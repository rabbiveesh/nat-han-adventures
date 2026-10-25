//! The NES voices, streaming: two pulses, the 4-bit triangle and the noise drums (and a third
//! pulse, as on Konami's VRC6 expansion chip, that only the soli's tenor plays), rendering
//! committed [`NoteEvent`]s block by block.
//!
//! The building blocks (oscillators, the LFSR, the drum synthesis, levels) are
//! [`crate::audio::synth`]'s. Tone voices are monophonic, keep their oscillator phase across
//! notes (reset at each loop pass, as a one-loop render would), and resolve pitch at note-on
//! through the note's [`Tuning`] (the medley's phrase tuning included); drum hits are
//! synthesized whole at their onset (choke and all) from one shared LFSR, reset at each loop
//! pass. The mix is summed channel by channel, then drum hits in order, then whatever rings
//! over from the previous pass (so a looping song's second pass is a seamless loop: what the
//! offline renderer keeps).
//!
//! # Instruments and effects
//! Each note plays its [`NoteEvent::inst`] ([`super::instrument`]): the macros step once per
//! 60 Hz frame, counted in samples from the note-on (`frame = i · 60 / rate`, integer), so
//! they're sample-accurate and blind to the block size. The note's [`Fx`] (the band's
//! ornaments: slides, fall-offs, vibrato, duty sweeps, the wah) step on the same frames. A
//! note with neither renders exactly as the voices always did (every extra factor is skipped
//! or exactly 1). A release part (`/` in a sequence) plays on after the note's end, until the
//! next note starts (it ducks out over the usual release ramp just before) or slurs on.
//!
//! Nothing here allocates once built: instrument tables and drum buffers are made up front.

use std::collections::VecDeque;

use kira::Frame;

use super::instrument::{Instruments, Kit, MAX_HIT_SECS, Tone, Wave};
use crate::audio::mml::{Arp, Drum};
use crate::audio::synth::{
    ARP_STEP, ATTACK, DUTIES, Lfsr, NOISE_GAIN, PULSE_GAIN, PULSE1_PAN, PULSE2_PAN, PULSE3_PAN, RELEASE, TRIANGLE_GAIN, advance, drum_kit,
    kit_drum_len, pan, pulse, triangle, triangle_lfo,
};
use crate::audio::tuning::{Medley, Tuning, Wobble};

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

/// Per-note effects (the band's chiptune ornaments), stepped per 60 Hz frame like the
/// instrument macros. [`Fx::NONE`] changes nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Fx {
    /// Start this many semitones off the note (negative: below) and slide onto it over
    /// `slide_frames`.
    pub slide: i8,
    pub slide_frames: u8,
    /// Fall off by this many semitones (negative: down) over the note's last `fall_frames`.
    pub fall: i8,
    pub fall_frames: u8,
    /// Vibrato this many cents deep (0: the instrument's own), from 0.1 s in.
    pub vib: u8,
    /// Sweep the duty a step every this many frames (0: off). Pulse waves only.
    pub sweep: u8,
    /// The plunger "wah": the volume and duty open and close about 3 times a second.
    pub wah: bool,
    /// One breath with the note it slurs from (a tie right after it): the instrument's macros,
    /// vibrato and fade carry on where that note left them instead of starting over, so a
    /// grace note, mordent or turn and the note it ornaments sound as one gesture.
    pub legato: bool,
}

impl Fx {
    pub const NONE: Fx = Fx { slide: 0, slide_frames: 0, fall: 0, fall_frames: 0, vib: 0, sweep: 0, wah: false, legato: false };

    pub fn is_none(&self) -> bool {
        *self == Fx::NONE
    }
}

/// One committed note or drum hit, in absolute samples.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoteEvent {
    /// 0 pulse1, 1 pulse2, 2 triangle, 3 noise, 4 pulse3 (the soli's tenor).
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
    /// Instrument: 0 the channel's built-in, `k` the song's `k`-th ([`Instruments`]).
    pub inst: u8,
    /// Ornament effects.
    pub fx: Fx,
    /// Unique, increasing (set when committed).
    pub seq: u64,
}

/// A vibrato in samples.
#[derive(Debug, Clone, Copy)]
struct Vib {
    delay: usize,
    ramp: f32,
    rate: f32,
    depth: f32,
}

/// A note sounding on a tone voice.
#[derive(Debug, Clone, Copy)]
struct Active {
    ev: NoteEvent,
    /// Length in samples (to the note-off).
    n: usize,
    /// Release samples after the note-off (0 without a release part).
    tail: usize,
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
    /// The gentle decay per sample (`fade`), if any.
    decay: Option<f32>,
    vib: Option<Vib>,
    wave: Wave,
    /// Macros or effects: something changes per frame.
    modulated: bool,
    inst: Tone,
    /// Sample (from the note-on) of the next frame boundary.
    next_frame: usize,
    /// Frames in the held part.
    held_frames: u32,
    vol_mul: f32,
    duty: f32,
    pitch_mul: f32,
    /// Samples of the legato line before this note ([`Fx::legato`]): the instrument's macros
    /// and vibrato count from the line's first note-on.
    carry: usize,
    /// The slide and fall-off, per sample ([`Bend::at`]).
    bend: Bend,
}

/// Where a legato line got to when its last note ended: the next note picks it up.
#[derive(Debug, Clone, Copy)]
struct Line {
    /// The sample the note's held part ended at.
    end: u64,
    inst: u8,
    carry: usize,
    lfo: f32,
    env: f32,
}

/// A slide onto the note and a fall off its end, glided sample by sample (eased: a slide
/// arrives quickly and settles, a fall drops away faster and faster) rather than stepped
/// per frame, so neither sounds like a staircase.
#[derive(Debug, Clone, Copy, Default)]
struct Bend {
    slide: f32,
    slide_len: f32,
    fall: f32,
    fall_from: usize,
    fall_len: f32,
}

impl Bend {
    fn new(fx: &Fx, n: usize, sr: f32) -> Self {
        let frames = |f: u8| f as f32 * sr / 60.0;
        let fall_len = frames(fx.fall_frames.max(1)).min(n as f32);
        Bend {
            slide: fx.slide as f32 * 100.0,
            slide_len: frames(fx.slide_frames).max(1.0),
            fall: fx.fall as f32 * 100.0,
            fall_from: n.saturating_sub(fall_len as usize),
            fall_len: fall_len.max(1.0),
        }
    }

    fn is_none(&self) -> bool {
        self.slide == 0.0 && self.fall == 0.0
    }

    /// The pitch factor at sample `i` of the note (held part `n` long; past it, the fall holds).
    #[inline]
    fn at(&self, i: usize) -> f32 {
        let mut cents = 0.0;
        if self.slide != 0.0 && (i as f32) < self.slide_len {
            let q = 1.0 - i as f32 / self.slide_len;
            cents += self.slide * q * q;
        }
        if self.fall != 0.0 && i >= self.fall_from {
            let p = ((i - self.fall_from) as f32 / self.fall_len).min(1.0);
            cents += self.fall * p * p;
        }
        if cents == 0.0 { 1.0 } else { (cents / 1200.0).exp2() }
    }
}

impl Active {
    /// The frame values at sample `i` (a frame boundary).
    fn frame(&mut self, i: usize, sr: u64) {
        let held = i < self.n;
        // The held part counts from the legato line's first note-on, the release from the note-off.
        let (base, from) = if held { (0, self.carry) } else { (self.n, 0) };
        let f = ((i - base + from) as u64 * 60 / sr) as u32;
        let mut next = (base + ((f as u64 + 1) * sr).div_ceil(60) as usize).saturating_sub(from);
        if held {
            next = next.min(self.n);
        }
        self.next_frame = next.max(i + 1);
        // Frames since the line's note-on (for sequences without a release), and since this
        // note's own note-on (for the effects).
        let fh = if held { f } else { self.held_frames + f };
        let fx_h = fh.saturating_sub((self.carry as u64 * 60 / sr) as u32);
        let at = |s: &super::instrument::Seq| if held || s.release().is_none() { s.held(fh) } else { s.released(f) };
        let fx = self.ev.fx;
        let mut vol = self.inst.vol.as_ref().map_or(1.0, |s| at(s).clamp(0, 15) as f32 / 15.0);
        let mut duty = self.inst.duty.as_ref().map_or(self.ev.duty as usize, |s| at(s).clamp(0, 3) as usize);
        if fx.sweep > 0 {
            // Up and back down: 0 1 2 3 2 1 0 ...
            const SWEEP: [usize; 6] = [0, 1, 2, 3, 2, 1];
            duty = SWEEP[(duty + (fx_h / fx.sweep as u32) as usize) % 6];
        }
        let cents = self.inst.pitch.as_ref().map_or(0.0, |s| at(s) as f32);
        if fx.wah {
            // Open and close every 18 frames (3.3 Hz): loud and bright, then muted and thin.
            let ph = (fx_h % 18) as f32 / 18.0;
            let open = 0.5 - 0.5 * (std::f32::consts::TAU * ph).cos();
            vol *= 0.35 + 0.65 * open;
            duty = if open > 0.55 { 2 } else { 0 };
        }
        self.vol_mul = vol;
        self.duty = DUTIES[duty];
        self.pitch_mul = if cents == 0.0 { 1.0 } else { 2f32.powf(cents / 1200.0) };
    }
}

/// One monophonic pulse / triangle voice.
struct ToneVoice {
    ch: usize,
    gain: (f32, f32),
    phase: f32,
    /// Loop pass of the last note-on (the phase restarts each pass).
    pass_start: Option<u64>,
    queue: VecDeque<NoteEvent>,
    active: Option<Active>,
    /// seq and end of the last event pushed (to set its `slur_out`).
    last: Option<(u64, u64)>,
    /// Instrument `k` (0: the built-in).
    insts: Vec<Tone>,
    /// The legato line the last note left off ([`Fx::legato`]).
    line: Option<Line>,
}

impl ToneVoice {
    fn new(ch: usize, gain: (f32, f32), instruments: &Instruments) -> Self {
        let n = instruments.defs.len() + 1;
        let insts = (0..n).map(|k| instruments.tone(ch, k as u8)).collect();
        ToneVoice { ch, gain, phase: 0.0, pass_start: None, queue: VecDeque::with_capacity(QUEUE), active: None, last: None, insts, line: None }
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
        let line = self.line.take().filter(|l| ev.fx.legato && ev.tie && l.end == ev.start && l.inst == ev.inst);
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
        let inst = self.insts.get(ev.inst as usize).copied().unwrap_or(self.insts[0]);
        let wave = if self.ch == 2 { Wave::Triangle } else { inst.wave.unwrap_or(Wave::Pulse) };
        let k = notes.len();
        let vib = if ev.fx.vib > 0 {
            let depth = 2f32.powf(ev.fx.vib as f32 / 1200.0) - 1.0;
            Some(Vib { delay: (0.1 * sr) as usize, ramp: 0.12 * sr, rate: 6.0 / sr, depth })
        } else {
            inst.vib.map(|v| Vib { delay: (v.delay * sr) as usize, ramp: v.ramp * sr, rate: v.rate / sr, depth: v.depth })
        }
        .filter(|_| k == 1);
        let sr_u = sr as u64;
        let tail = (inst.release_frames() as u64 * sr_u).div_ceil(60) as usize;
        let bend = Bend::new(&ev.fx, n, sr);
        let modulated = inst.has_macros() || !ev.fx.is_none();
        let carry = line.map_or(0, |l| l.carry);
        self.active = Some(Active {
            ev,
            n,
            tail,
            s0: ev.start - ev.loop_start,
            dts,
            k,
            amp,
            attack,
            release,
            wobble,
            env: line.map_or(1.0, |l| l.env),
            lfo: line.map_or(0.0, |l| l.lfo),
            decay: inst.fade.map(|tau| (-1.0 / (tau * sr)).exp()),
            vib,
            wave,
            modulated,
            inst,
            next_frame: 0,
            held_frames: ((n + carry) as u64 * 60 / sr_u) as u32,
            vol_mul: 1.0,
            duty: DUTIES[ev.duty as usize & 3],
            pitch_mul: 1.0,
            carry,
            bend,
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
            let next_start = self.queue.front().map(|e| e.start);
            let a = self.active.as_mut().unwrap();
            let held_end = a.ev.start + a.n as u64;
            let mut a_end = held_end + if a.ev.slur_out { 0 } else { a.tail as u64 };
            // A release tail stops where the next note starts.
            if a_end > held_end
                && let Some(s) = next_start
                && s < a_end
            {
                a_end = s.max(held_end);
            }
            let a_end = a_end.max(t);
            let stop = end.min(a_end);
            if stop > t {
                let i0 = (t - a.ev.start) as usize;
                let frames = &mut out[(t - t0) as usize..(stop - t0) as usize];
                Self::tone(a, self.gain, &mut self.phase, frames, i0, sr, (a_end - a.ev.start) as usize);
            }
            if stop == a_end {
                let a = self.active.take().unwrap();
                self.line = a.ev.slur_out.then_some(Line { end: held_end, inst: a.ev.inst, carry: a.carry + a.n, lfo: a.lfo, env: a.env });
            }
            t = stop;
        }
    }

    /// Samples `i0..` of the note into `frames` (the offline `Tone::render`, resumable). The
    /// note (and its tail) ends at sample `n_end`.
    #[inline]
    fn tone(a: &mut Active, (gl, gr): (f32, f32), phase: &mut f32, frames: &mut [Frame], i0: usize, sr: f32, n_end: usize) {
        let n = n_end;
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
        let vib = a.vib;
        let carry = a.carry;
        let bend = a.bend;
        let bends = !bend.is_none();
        let decay = a.decay;
        let modulated = a.modulated;
        let sr_u = sr as u64;
        let pulse_wave = a.wave == Wave::Pulse;
        for (j, f) in frames.iter_mut().enumerate() {
            let i = i0 + j;
            if modulated && i >= a.next_frame {
                a.frame(i, sr_u);
            }
            let mut dt = if k == 1 { dts[0] } else { dts[(i / step) % k] };
            if wobbles {
                dt *= wobble(i);
            }
            if modulated {
                dt *= a.pitch_mul;
            }
            if bends {
                dt *= bend.at(i);
            }
            if let Some(vb) = vib
                && i + carry > vb.delay
            {
                let depth = (((i + carry - vb.delay) as f32) / vb.ramp).min(1.0) * vb.depth;
                advance(&mut a.lfo, vb.rate);
                dt *= 1.0 + depth * (triangle_lfo(a.lfo));
            }
            let mut v = if pulse_wave { pulse(*phase, dt, a.duty) } else { triangle(*phase) } * amp;
            if let Some(d) = decay {
                v *= 0.65 + 0.35 * a.env;
                a.env *= d;
            }
            v *= edge(i);
            if modulated {
                v *= a.vol_mul;
            }
            advance(phase, dt);
            f.left += v * gl;
            f.right += v * gr;
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
    /// Kit `k` (0: the built-in).
    kits: Vec<Kit>,
}

impl Drums {
    fn new(sr: f32, instruments: &Instruments) -> Self {
        let cap = (MAX_HIT_SECS * sr) as usize + 1;
        Drums {
            gain: pan(0.0, NOISE_GAIN),
            queue: VecDeque::with_capacity(QUEUE),
            ringing: Vec::with_capacity(DRUM_VOICES),
            spare: (0..DRUM_VOICES).map(|_| Vec::with_capacity(cap)).collect(),
            lfsr: Lfsr::default(),
            pass_start: None,
            kits: (0..=instruments.defs.len()).map(|k| instruments.kit(k as u8)).collect(),
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
            let kit = self.kits.get(e.inst as usize).copied().unwrap_or(Kit::DEFAULT);
            let mut n = (kit_drum_len(d, &kit) * sr) as usize;
            // The open hat is choked by the next hit of the same pass (with a short fade).
            let choke = (d == Drum::OpenHat)
                .then(|| self.queue.front().filter(|next| next.loop_start == e.loop_start).map(|next| next.start.saturating_sub(e.start) as usize))
                .flatten();
            if let Some(c) = choke {
                n = n.min(c + (0.004 * sr) as usize);
            }
            buf.clear();
            buf.resize(n.min(buf.capacity()), 0.0);
            drum_kit(d, &kit, &mut buf, &mut self.lfsr, sr);
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
    tones: [ToneVoice; 4],
    drums: Drums,
    /// The medley for each shape: the straight one's, and the waltz's (the same phrases; the
    /// wobble fitted to its loop).
    medleys: [Medley; 2],
    spill: Vec<Frame>,
}

/// Frames rendered in one go (the caller splits longer blocks).
pub const MAX_SEGMENT: usize = 1024;

impl VoiceBank {
    /// The voices for a song with `instruments` (an empty table: the built-ins only).
    pub fn new(sample_rate: u32, medleys: [Medley; 2], instruments: &Instruments) -> Self {
        let sr = sample_rate as f32;
        VoiceBank {
            sr,
            tones: [
                ToneVoice::new(0, pan(PULSE1_PAN, PULSE_GAIN), instruments),
                ToneVoice::new(1, pan(PULSE2_PAN, PULSE_GAIN), instruments),
                ToneVoice::new(2, pan(0.0, TRIANGLE_GAIN), instruments),
                ToneVoice::new(4, pan(PULSE3_PAN, PULSE_GAIN), instruments),
            ],
            drums: Drums::new(sr, instruments),
            medleys,
            spill: vec![Frame::ZERO; MAX_SEGMENT],
        }
    }

    /// Queue a committed event (per channel, in start order).
    pub fn push(&mut self, ev: NoteEvent) {
        match ev.ch {
            0..=2 => self.tones[ev.ch as usize].push(ev),
            4 => self.tones[3].push(ev),
            _ => self.drums.push(ev),
        }
    }

    /// Scale channel `ch`'s output by `gain` (0 mutes; 1 is as written), from the next sample.
    pub fn set_channel_gain(&mut self, ch: usize, gain: f32) {
        let g = if gain.is_finite() { gain.clamp(0.0, 4.0) } else { 0.0 };
        let (l, r) = match ch {
            0 => pan(PULSE1_PAN, PULSE_GAIN),
            1 => pan(PULSE2_PAN, PULSE_GAIN),
            2 => pan(0.0, TRIANGLE_GAIN),
            _ => pan(0.0, NOISE_GAIN),
        };
        match ch {
            0..=2 => self.tones[ch].gain = (l * g, r * g),
            _ => self.drums.gain = (l * g, r * g),
        }
        // The third pulse is the comp's: it follows pulse 2.
        if ch == 1 {
            let (l, r) = pan(PULSE3_PAN, PULSE_GAIN);
            self.tones[3].gain = (l * g, r * g);
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

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;
    /// Samples in a 60 Hz frame.
    const F: u64 = 800;

    fn note(start: u64, end: u64, n: u8, inst: u8, tie: bool, legato: bool, seq: u64) -> NoteEvent {
        NoteEvent {
            ch: 0,
            bar: 0,
            start,
            end,
            sound: Sound::Note(n),
            volume: 15,
            duty: 2,
            tie,
            slur_out: false,
            gain: 1.0,
            beat: 0.0,
            phrase_beat: 0.0,
            waltz: false,
            loop_start: 0,
            salt: 0,
            tuning: Tuning::Equal,
            anchor: 60,
            inst,
            fx: Fx { legato, ..Fx::NONE },
            seq,
        }
    }

    /// Pulse 1's output for `events`, `len` samples.
    fn play(events: &[NoteEvent], len: usize) -> Vec<f32> {
        let inst = Instruments::parse("swell : vol 3 3 3 3 3 3 3 3 3 3 15\npulse1 = default swell").unwrap();
        let m = Medley::new(0, 16.0, Some(8.0));
        let mut bank = VoiceBank::new(SR, [m.clone(), m], &inst);
        for e in events {
            bank.push(*e);
        }
        let mut out = vec![Frame::ZERO; len];
        let mut t = 0;
        for chunk in out.chunks_mut(MAX_SEGMENT) {
            bank.render(chunk, t, 0);
            t += chunk.len() as u64;
        }
        out.iter().map(|f| f.left).collect()
    }

    fn peak(x: &[f32]) -> f32 {
        x.iter().fold(0.0f32, |m, v| m.max(v.abs()))
    }

    #[test]
    fn a_legato_note_carries_on_the_breath_a_plain_slur_starts_over() {
        // A grace note for 12 frames (its swell is full from frame 10), then the main note.
        let swell = 1;
        let grace = note(0, 12 * F, 71, swell, false, false, 1);
        let at = 12 * F + 100..13 * F - 100;
        let restarts = play(&[grace, note(12 * F, 40 * F, 72, swell, true, false, 2)], 40 * F as usize);
        let carries = play(&[grace, note(12 * F, 40 * F, 72, swell, true, true, 2)], 40 * F as usize);
        let (r, c) = (peak(&restarts[at.start as usize..at.end as usize]), peak(&carries[at.start as usize..at.end as usize]));
        assert!(c > 4.0 * r, "the main note's first frame: legato {c}, plain slur {r}");
        // And the full swell where both have got there.
        let late = 30 * F as usize..31 * F as usize;
        assert!((peak(&restarts[late.clone()]) - peak(&carries[late])).abs() < 1e-3);
    }

    #[test]
    fn a_legato_line_breaks_on_a_new_attack_or_another_instrument() {
        let swell = 1;
        let grace = note(0, 12 * F, 71, swell, false, false, 1);
        let first = 12 * F as usize + 100..13 * F as usize - 100;
        let plain = play(&[grace, note(12 * F, 40 * F, 72, swell, false, true, 2)], 40 * F as usize);
        let after_default = play(&[note(0, 12 * F, 71, 0, false, false, 1), note(12 * F, 40 * F, 72, swell, true, true, 2)], 40 * F as usize);
        let restarts = play(&[grace, note(12 * F, 40 * F, 72, swell, true, false, 2)], 40 * F as usize);
        let r = peak(&restarts[first.clone()]);
        assert!((peak(&plain[first.clone()]) - r).abs() < 0.2 * r, "an attacked note starts its swell over");
        assert!((peak(&after_default[first.clone()]) - r).abs() < 0.2 * r, "a new instrument starts its swell over");
    }

    #[test]
    fn slides_and_falls_glide_sample_by_sample() {
        let fx = Fx { slide: -2, slide_frames: 4, fall: -3, fall_frames: 6, ..Fx::NONE };
        let n = 30 * F as usize;
        let b = Bend::new(&fx, n, SR as f32);
        let cents = |i: usize| 1200.0 * b.at(i).log2();
        assert!((cents(0) + 200.0).abs() < 0.01, "starts two semitones under: {}", cents(0));
        // Rising every sample of the slide (no frame-long steps), smoothly, and arriving.
        for i in 1..4 * F as usize {
            let d = cents(i) - cents(i - 1);
            assert!(d > 0.0 && d < 0.2, "sample {i}: {d} cents");
        }
        assert_eq!(b.at(4 * F as usize), 1.0);
        assert_eq!(b.at(20 * F as usize), 1.0);
        // The fall: falling all through its last 6 frames (from a standstill), down three semitones.
        let from = n - 6 * F as usize;
        for i in from + 100..n {
            assert!(cents(i) < cents(i - 1), "sample {i}");
        }
        assert!((cents(n - 1) + 300.0).abs() < 1.0, "{}", cents(n - 1));
    }
}
