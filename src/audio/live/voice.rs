//! The NES voices, streaming: two pulses, the 4-bit triangle and the noise drums, rendering
//! committed [`NoteEvent`]s block by block.
//!
//! Every per-sample operation is the offline renderer's ([`crate::audio::synth`]), in the same
//! order, so for the same events the output is bit-identical: tone voices are monophonic, keep
//! their oscillator phase across notes (reset at each loop pass, as a one-loop render would),
//! and resolve pitch at note-on through the note's [`Tuning`] (the medley's phrase tuning
//! included); drum hits are synthesized whole at their onset (choke and all) from one shared
//! LFSR, reset at each loop pass. The mix is summed channel by channel, then drum hits in
//! order, then whatever rings over from the previous pass (like the offline loop wrap).
//!
//! Consolidation note: `advance`, `triangle_lfo`, `Edges`, `pan`, the drum synthesis and the
//! LFSR are copies of private (or sample-rate-fixed) items in `synth.rs`, taking the sample
//! rate as a parameter.

use std::collections::VecDeque;

use kira::Frame;

use crate::audio::mml::{Arp, Drum};
use crate::audio::synth::{ARP_STEP, DUTIES, pulse, triangle};
use crate::audio::tuning::{Medley, Tuning, Wobble};

// Mix levels, panning and note edges: as `synth.rs`.
const PULSE_GAIN: f32 = 0.15;
const TRIANGLE_GAIN: f32 = 0.30;
const NOISE_GAIN: f32 = 0.30;
const PULSE1_PAN: f32 = -0.2;
const PULSE2_PAN: f32 = 0.2;
const ATTACK: f32 = 0.002;
const RELEASE: f32 = 0.008;

/// Longest a drum rings, in seconds (the open hat).
const MAX_DRUM_SECS: f32 = 0.3;
/// Drum hits that can ring at once.
const DRUM_VOICES: usize = 24;
/// Committed events a channel can hold (several bars' worth).
pub const QUEUE: usize = 1024;

/// What a [`NoteEvent`] plays.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sound {
    Note(u8),
    /// A chord as a fast arpeggio.
    Arp(Arp),
    Drum(Drum),
}

impl Sound {
    pub fn notes(&self) -> &[u8] {
        match self {
            Sound::Note(n) => std::slice::from_ref(n),
            Sound::Arp(a) => a.notes(),
            Sound::Drum(_) => &[],
        }
    }
}

/// One committed note or drum hit, in absolute samples.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoteEvent {
    /// 0 pulse1, 1 pulse2, 2 triangle, 3 noise.
    pub ch: u8,
    /// Absolute bar it was committed in.
    pub bar: u64,
    /// First and one-past-last sample.
    pub start: u64,
    pub end: u64,
    pub sound: Sound,
    /// 0..=15.
    pub volume: u8,
    /// Pulse duty 0..=3.
    pub duty: u8,
    /// Slurred from the previous note (no attack).
    pub tie: bool,
    /// Slurs into the next note (no release). Set by the voice bank when the next note arrives.
    pub slur_out: bool,
    /// Dynamics, on top of `volume` (exactly 1.0 when the dynamics dial is at 0).
    pub gain: f32,
    /// Start in song beats (within the loop pass, in its shape's beats).
    pub beat: f64,
    /// Start in 4/4 song beats, for the medley's phrase (the waltz's beats unwarped).
    pub phrase_beat: f64,
    /// Played in the waltz's shape (its medley's wobble fits the waltz's loop).
    pub waltz: bool,
    /// Absolute sample where this event's loop pass starts.
    pub loop_start: u64,
    /// Tuning salt of the note-on ([`crate::audio::tuning::salt`]; arpeggio tone `j` adds `j`).
    pub salt: u64,
    pub tuning: Tuning,
    /// The voice's tuning anchor ([`crate::audio::tuning::anchor_tonic`]).
    pub anchor: u8,
    /// Unique, increasing (set when committed).
    pub seq: u64,
}

#[inline]
fn advance(phase: &mut f32, dt: f32) {
    *phase += dt;
    if *phase >= 1.0 {
        *phase -= phase.floor();
    }
}

#[inline]
fn triangle_lfo(phase: f32) -> f32 {
    4.0 * (phase - 0.5).abs() - 1.0
}

fn pan(p: f32, gain: f32) -> (f32, f32) {
    (gain * (1.0 - p), gain * (1.0 + p))
}

/// The NES noise LFSR, at any sample rate.
#[derive(Debug, Clone, Copy)]
pub struct Lfsr {
    reg: u16,
    acc: f32,
}

impl Default for Lfsr {
    fn default() -> Self {
        Lfsr { reg: 1, acc: 0.0 }
    }
}

impl Lfsr {
    #[inline]
    fn next(&mut self, clock_hz: f32, sr: f32) -> f32 {
        self.acc += clock_hz / sr;
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            let fb = (self.reg ^ (self.reg >> 1)) & 1;
            self.reg = (self.reg >> 1) | (fb << 14);
        }
        if self.reg & 1 == 0 { 1.0 } else { -1.0 }
    }
}

/// Natural length of each drum, in seconds.
pub fn drum_len(d: Drum) -> f32 {
    match d {
        Drum::Kick => 0.16,
        Drum::Snare => 0.2,
        Drum::ClosedHat => 0.05,
        Drum::OpenHat => MAX_DRUM_SECS,
    }
}

/// One drum hit into `buf` (its length is the hit's), like `synth::drum`.
pub fn drum(d: Drum, buf: &mut [f32], lfsr: &mut Lfsr, sr: f32) {
    let n = buf.len();
    let decay = |tau: f32| (-1.0 / (tau * sr)).exp();
    let mut phase = 0.0f32;
    let mut prev = 0.0f32;
    match d {
        Drum::Kick => {
            let (k_amp, k_pitch) = (decay(0.07), decay(0.025));
            let (mut a, mut p) = (1.0f32, 1.0f32);
            for (i, s) in buf.iter_mut().enumerate() {
                let f = 48.0 + 130.0 * p;
                advance(&mut phase, f / sr);
                let click = if i < (0.004 * sr) as usize { lfsr.next(12_000.0, sr) * 0.3 } else { 0.0 };
                *s = (triangle(phase) * 1.1 + click) * a;
                a *= k_amp;
                p *= k_pitch;
            }
        }
        Drum::Snare => {
            let (k_noise, k_tone) = (decay(0.055), decay(0.03));
            let (mut a, mut t) = (1.0f32, 1.0f32);
            for s in buf.iter_mut() {
                advance(&mut phase, 185.0 / sr);
                *s = lfsr.next(18_000.0, sr) * a * 0.75 + triangle(phase) * t * 0.6;
                a *= k_noise;
                t *= k_tone;
            }
        }
        Drum::ClosedHat | Drum::OpenHat => {
            let tau = if d == Drum::ClosedHat { 0.012 } else { 0.07 };
            let k = decay(tau);
            let mut a = 0.55f32;
            for s in buf.iter_mut() {
                let x = lfsr.next(220_000.0, sr);
                *s = (x - prev) * 0.5 * a;
                prev = x;
                a *= k;
            }
        }
    }
    let ramp = ((0.001 * sr) as usize).min(n);
    for (i, s) in buf.iter_mut().take(ramp).enumerate() {
        *s *= i as f32 / ramp as f32;
    }
    let ramp = ((0.003 * sr) as usize).min(n);
    for i in 0..ramp {
        buf[n - 1 - i] *= i as f32 / ramp as f32;
    }
}

/// A note sounding on a tone voice.
#[derive(Debug, Clone, Copy)]
struct Active {
    ev: NoteEvent,
    /// Length in samples.
    n: usize,
    /// Sample within the loop where it started (for the wobble's clock).
    s0: u64,
    dts: [f32; Arp::MAX],
    k: usize,
    amp: f32,
    attack: f32,
    /// The release ramp if the note doesn't slur on.
    release: f32,
    wobble: Option<Wobble>,
    env: f32,
    lfo: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Pulse { vibrato: bool },
    Triangle,
}

/// One monophonic pulse / triangle voice.
struct ToneVoice {
    kind: Kind,
    gain: (f32, f32),
    phase: f32,
    /// Loop pass of the last note-on (the phase restarts each pass).
    pass_start: Option<u64>,
    queue: VecDeque<NoteEvent>,
    active: Option<Active>,
    /// seq and end of the last event pushed (to set its `slur_out`).
    last: Option<(u64, u64)>,
}

impl ToneVoice {
    fn new(kind: Kind, gain: (f32, f32)) -> Self {
        ToneVoice { kind, gain, phase: 0.0, pass_start: None, queue: VecDeque::with_capacity(QUEUE), active: None, last: None }
    }

    fn push(&mut self, ev: NoteEvent) {
        // The note before slurs into this one if this one is a tied single note right after it
        // (the offline renderer's rule).
        if let Some((seq, end)) = self.last
            && ev.tie
            && matches!(ev.sound, Sound::Note(_))
            && ev.start == end
        {
            if let Some(prev) = self.queue.back_mut().filter(|p| p.seq == seq) {
                prev.slur_out = true;
            } else if let Some(a) = self.active.as_mut().filter(|a| a.ev.seq == seq) {
                a.ev.slur_out = true;
            }
        }
        self.last = Some((ev.seq, ev.end));
        if self.queue.len() < self.queue.capacity() {
            self.queue.push_back(ev);
        }
    }

    fn note_on(&mut self, ev: NoteEvent, medleys: &[Medley; 2], sr: f32) {
        let medley = &medleys[ev.waltz as usize];
        if self.pass_start != Some(ev.loop_start) {
            self.pass_start = Some(ev.loop_start);
            self.phase = 0.0;
        }
        let n = (ev.end - ev.start) as usize;
        if n == 0 || ev.volume == 0 {
            return;
        }
        let notes = ev.sound.notes();
        let mut dts = [0.0f32; Arp::MAX];
        for (j, (dt, &note)) in dts.iter_mut().zip(notes).enumerate() {
            let salt = ev.salt.wrapping_add(j as u64);
            let hz = match ev.tuning {
                Tuning::Medley => medley.hz(note, ev.anchor, ev.phrase_beat, salt),
                t => t.hz(note, ev.anchor, salt),
            };
            *dt = (hz / sr as f64) as f32;
        }
        let wobble = match ev.tuning {
            Tuning::Medley => Some(medley.wobble()),
            t => t.wobble_shape(),
        };
        let mut amp = ev.volume as f32 / 15.0;
        if ev.gain != 1.0 {
            amp *= ev.gain;
        }
        let attack = if ev.tie { 0.0 } else { (ATTACK * sr).min(n as f32 / 4.0).max(1.0) };
        let release = (RELEASE * sr).min(n as f32 / 3.0).max(1.0);
        self.active = Some(Active {
            ev,
            n,
            s0: ev.start - ev.loop_start,
            dts,
            k: notes.len(),
            amp,
            attack,
            release,
            wobble,
            env: 1.0,
            lfo: 0.0,
        });
    }

    /// Add this voice into `out` (samples `t0..t0 + out.len()`).
    fn render(&mut self, out: &mut [Frame], t0: u64, medleys: &[Medley; 2], sr: f32) {
        let end = t0 + out.len() as u64;
        let mut t = t0;
        while t < end {
            if self.active.is_none() {
                match self.queue.front() {
                    Some(e) if e.start < end => {
                        let e = self.queue.pop_front().unwrap();
                        t = t.max(e.start);
                        self.note_on(e, medleys, sr);
                        continue;
                    }
                    _ => return,
                }
            }
            let a = self.active.as_mut().unwrap();
            let stop = end.min(a.ev.start + a.n as u64);
            let i0 = (t - a.ev.start) as usize;
            let frames = &mut out[(t - t0) as usize..(stop - t0) as usize];
            Self::tone(a, self.kind, self.gain, &mut self.phase, frames, i0, sr);
            if stop == a.ev.start + a.n as u64 {
                self.active = None;
            }
            t = stop;
        }
    }

    /// Samples `i0..` of the note into `frames` (the offline `Tone::render`, resumable).
    #[inline]
    fn tone(a: &mut Active, kind: Kind, (gl, gr): (f32, f32), phase: &mut f32, frames: &mut [Frame], i0: usize, sr: f32) {
        let n = a.n;
        let attack = a.attack;
        let release = if a.ev.slur_out { 0.0 } else { a.release };
        let edge = |i: usize| {
            let mut g = 1.0;
            if attack > 0.0 {
                g *= (i as f32 / attack).min(1.0);
            }
            if release > 0.0 {
                g *= ((n - i) as f32 / release).min(1.0);
            }
            g
        };
        let s0 = a.s0;
        let wob = a.wobble;
        let wobble = |i: usize| wob.map_or(1.0, |w| w.at((s0 + i as u64) as f64 / sr as f64)) as f32;
        let wobbles = wob.is_some();
        let k = a.k;
        let dts = a.dts;
        let step = ((ARP_STEP * sr) as usize).max(1);
        let amp = a.amp;
        match kind {
            Kind::Pulse { vibrato } => {
                let decay = (-1.0 / (0.8 * sr)).exp();
                let vib_delay = (0.18 * sr) as usize;
                let vib_ramp = 0.25 * sr;
                let vib_rate = 5.5 / sr;
                let vibrato = vibrato && k == 1;
                let duty = DUTIES[a.ev.duty as usize];
                for (j, f) in frames.iter_mut().enumerate() {
                    let i = i0 + j;
                    let mut dt = if k == 1 { dts[0] } else { dts[(i / step) % k] };
                    if wobbles {
                        dt *= wobble(i);
                    }
                    if vibrato && i > vib_delay {
                        let depth = (((i - vib_delay) as f32) / vib_ramp).min(1.0) * 0.006;
                        advance(&mut a.lfo, vib_rate);
                        dt *= 1.0 + depth * (triangle_lfo(a.lfo));
                    }
                    let v = pulse(*phase, dt, duty) * amp * (0.65 + 0.35 * a.env) * edge(i);
                    a.env *= decay;
                    advance(phase, dt);
                    f.left += v * gl;
                    f.right += v * gr;
                }
            }
            Kind::Triangle => {
                for (j, f) in frames.iter_mut().enumerate() {
                    let i = i0 + j;
                    let mut dt = if k == 1 { dts[0] } else { dts[(i / step) % k] };
                    if wobbles {
                        dt *= wobble(i);
                    }
                    let v = triangle(*phase) * amp * edge(i);
                    advance(phase, dt);
                    f.left += v * gl;
                    f.right += v * gr;
                }
            }
        }
    }
}

/// A drum hit ringing: its samples, already scaled (volume, choke fade).
struct Hit {
    start: u64,
    loop_start: u64,
    buf: Vec<f32>,
}

/// The noise channel: hits start in order and ring out on their own.
struct Drums {
    gain: (f32, f32),
    queue: VecDeque<NoteEvent>,
    ringing: Vec<Hit>,
    spare: Vec<Vec<f32>>,
    lfsr: Lfsr,
    pass_start: Option<u64>,
}

impl Drums {
    fn new(sr: f32) -> Self {
        let cap = (MAX_DRUM_SECS * sr) as usize + 1;
        Drums {
            gain: pan(0.0, NOISE_GAIN),
            queue: VecDeque::with_capacity(QUEUE),
            ringing: Vec::with_capacity(DRUM_VOICES),
            spare: (0..DRUM_VOICES).map(|_| Vec::with_capacity(cap)).collect(),
            lfsr: Lfsr::default(),
            pass_start: None,
        }
    }

    fn push(&mut self, ev: NoteEvent) {
        if self.queue.len() < self.queue.capacity() {
            self.queue.push_back(ev);
        }
    }

    /// Synthesize the hits that start before `end`, in order.
    fn start_hits(&mut self, end: u64, sr: f32) {
        while self.queue.front().is_some_and(|e| e.start < end) {
            let e = self.queue.pop_front().unwrap();
            let Sound::Drum(d) = e.sound else { continue };
            if self.pass_start != Some(e.loop_start) {
                self.pass_start = Some(e.loop_start);
                self.lfsr = Lfsr::default();
            }
            let Some(mut buf) = self.spare.pop() else { continue };
            let mut n = (drum_len(d) * sr) as usize;
            // The open hat is choked by the next hit of the same pass (with a short fade).
            let choke = (d == Drum::OpenHat)
                .then(|| self.queue.front().filter(|next| next.loop_start == e.loop_start).map(|next| next.start.saturating_sub(e.start) as usize))
                .flatten();
            if let Some(c) = choke {
                n = n.min(c + (0.004 * sr) as usize);
            }
            buf.clear();
            buf.resize(n.min(buf.capacity()), 0.0);
            drum(d, &mut buf, &mut self.lfsr, sr);
            let mut amp = e.volume as f32 / 15.0;
            if e.gain != 1.0 {
                amp *= e.gain;
            }
            let fade_from = choke.unwrap_or(usize::MAX);
            for (i, s) in buf.iter_mut().enumerate() {
                let mut v = *s * amp;
                if i >= fade_from {
                    v *= 1.0 - (i - fade_from) as f32 / (0.004 * sr);
                }
                *s = v;
            }
            self.ringing.push(Hit { start: e.start, loop_start: e.loop_start, buf });
        }
    }

    /// Add the hits of loop pass `pass` into `out` (samples `t0..`), the others into `spill`.
    fn render(&mut self, out: &mut [Frame], spill: &mut [Frame], t0: u64, pass: u64) -> bool {
        let (gl, gr) = self.gain;
        let end = t0 + out.len() as u64;
        let mut spilled = false;
        for h in &self.ringing {
            let from = h.start.max(t0);
            let to = end.min(h.start + h.buf.len() as u64);
            if from >= to {
                continue;
            }
            let dst = if h.loop_start == pass {
                &mut out[(from - t0) as usize..(to - t0) as usize]
            } else {
                spilled = true;
                &mut spill[(from - t0) as usize..(to - t0) as usize]
            };
            for (f, &v) in dst.iter_mut().zip(&h.buf[(from - h.start) as usize..]) {
                f.left += v * gl;
                f.right += v * gr;
            }
        }
        // Free the hits that have rung out.
        let spare = &mut self.spare;
        self.ringing.retain_mut(|h| {
            let alive = h.start + (h.buf.len() as u64) > end;
            if !alive {
                spare.push(std::mem::take(&mut h.buf));
            }
            alive
        });
        spilled
    }
}

/// All the voices.
pub struct VoiceBank {
    sr: f32,
    tones: [ToneVoice; 3],
    drums: Drums,
    /// The medley for each shape: the straight one's, and the waltz's (the same phrases; the
    /// wobble fitted to its loop).
    medleys: [Medley; 2],
    spill: Vec<Frame>,
}

/// Frames rendered in one go (the caller splits longer blocks).
pub const MAX_SEGMENT: usize = 1024;

impl VoiceBank {
    pub fn new(sample_rate: u32, medleys: [Medley; 2]) -> Self {
        let sr = sample_rate as f32;
        VoiceBank {
            sr,
            tones: [
                ToneVoice::new(Kind::Pulse { vibrato: true }, pan(PULSE1_PAN, PULSE_GAIN)),
                ToneVoice::new(Kind::Pulse { vibrato: false }, pan(PULSE2_PAN, PULSE_GAIN)),
                ToneVoice::new(Kind::Triangle, pan(0.0, TRIANGLE_GAIN)),
            ],
            drums: Drums::new(sr),
            medleys,
            spill: vec![Frame::ZERO; MAX_SEGMENT],
        }
    }

    /// Queue a committed event (per channel, in start order).
    pub fn push(&mut self, ev: NoteEvent) {
        match ev.ch {
            0..=2 => self.tones[ev.ch as usize].push(ev),
            _ => self.drums.push(ev),
        }
    }

    /// Committed events not started yet.
    pub fn pending(&self) -> impl Iterator<Item = &NoteEvent> {
        self.tones.iter().flat_map(|v| v.queue.iter()).chain(self.drums.queue.iter())
    }

    /// Mix samples `t0..t0 + out.len()` (at most [`MAX_SEGMENT`], all in the loop pass that
    /// starts at `pass`) into the zeroed `out`. Not mastered.
    pub fn render(&mut self, out: &mut [Frame], t0: u64, pass: u64) {
        debug_assert!(out.len() <= MAX_SEGMENT);
        for v in &mut self.tones {
            v.render(out, t0, &self.medleys, self.sr);
        }
        self.drums.start_hits(t0 + out.len() as u64, self.sr);
        let spill = &mut self.spill[..out.len()];
        spill.fill(Frame::ZERO);
        if self.drums.render(out, spill, t0, pass) {
            for (f, s) in out.iter_mut().zip(spill.iter()) {
                *f += *s;
            }
        }
    }

    /// Nothing queued or sounding.
    pub fn idle(&self) -> bool {
        self.tones.iter().all(|v| v.queue.is_empty() && v.active.is_none()) && self.drums.queue.is_empty() && self.drums.ringing.is_empty()
    }
}
