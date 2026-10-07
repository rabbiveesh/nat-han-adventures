//! A small NES-flavoured synth: renders a [`Song`] to stereo PCM ([`Frame`]s).
//!
//! Voices (one per MML channel, each monophonic):
//! - pulse 1 / pulse 2: band-limited (PolyBLEP) pulse waves with 4 duty cycles, a short
//!   attack/release ramp so notes never click, a gentle decay, and (pulse 1 only) a delayed
//!   vibrato on long notes. Pulse 1 sits slightly left, pulse 2 slightly right.
//! - triangle: the NES's 32-step (4-bit) stepped triangle, for that gritty bass.
//! - noise: drums built from a 15-bit LFSR noise generator plus pitch-swept triangle blips.
//!
//! Timing: MML beats are swung (see [`apply_swing`]) and converted to sample positions; each event is
//! rendered independently into the mix, with oscillator phase carried across notes. Looping
//! songs are rendered exactly one loop long, and anything ringing past the loop end (drum tails)
//! is wrapped around onto the start, so the loop is seamless.

use bevy_kira_audio::prelude::Frame;

use super::Song;
use super::mml::{self, Channel, Drum, Event, EventKind, Track};

/// Output sample rate. 32 kHz keeps memory and render time down (a 60s song is ~15 MB of
/// frames) while leaving plenty of headroom above the highest notes and hats.
pub const SAMPLE_RATE: u32 = 32_000;
const SR: f32 = SAMPLE_RATE as f32;

// Mix levels (linear). Pulse waves at narrow duty peak at 1.75 (they're DC-centred).
const PULSE_GAIN: f32 = 0.15;
const TRIANGLE_GAIN: f32 = 0.30;
const NOISE_GAIN: f32 = 0.30;
/// Stereo placement of the pulses: -1 left .. 1 right. Kept small: mostly centred.
const PULSE1_PAN: f32 = -0.2;
const PULSE2_PAN: f32 = 0.2;
/// Note edges, in seconds.
const ATTACK: f32 = 0.002;
const RELEASE: f32 = 0.008;
/// Extra time rendered after a non-looping song's last note.
const TAIL: f32 = 0.4;

/// A rendered piece of audio.
#[derive(Debug, Clone)]
pub struct Rendered {
    pub frames: Vec<Frame>,
    pub sample_rate: u32,
    /// Loop the whole thing (it's exactly one loop long).
    pub looping: bool,
}

impl Rendered {
    pub fn duration_secs(&self) -> f32 {
        self.frames.len() as f32 / self.sample_rate as f32
    }
}

/// Swing a track: an 8th note (or 8th rest) that starts on an off-beat 8th position starts
/// `swing` 8ths late, and the event right before it is lengthened to meet it. Nothing else moves:
/// 16ths, dotted rhythms, quarters and every downbeat stay put, and since each event is placed
/// from its own unswung time, no error ever accumulates.
pub fn apply_swing(track: &Track, swing: f32) -> Track {
    const EPS: f64 = 1e-6;
    if swing == 0.0 {
        return track.clone();
    }
    let delay = swing.clamp(0.0, 0.9) as f64 * 0.5;
    let swung = |e: &Event| {
        let frac = e.start - e.start.floor();
        (frac - 0.5).abs() < EPS && (e.dur - 0.5).abs() < EPS
    };
    let ev = &track.events;
    let start = |e: &Event| if swung(e) { e.start + delay } else { e.start };
    let events = ev
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let end = e.start + e.dur;
            let end = match ev.get(i + 1) {
                Some(next) if (next.start - end).abs() < EPS => start(next),
                _ => end,
            };
            let s = start(e);
            Event { start: s, dur: end - s, ..*e }
        })
        .collect();
    Track { events, length: track.length }
}

pub fn midi_to_hz(note: u8) -> f32 {
    440.0 * 2f32.powf((note as f32 - 69.0) / 12.0)
}

/// Render a song. Errors name the song and channel of the bad MML.
pub fn render_song(song: &Song) -> Result<Rendered, String> {
    let parse = |name: &str, src: &str, ch: Channel| {
        mml::parse(src, ch).map_err(|e| format!("song \"{}\", {name}: {e}", song.title))
    };
    let p1 = parse("pulse1", song.pulse1, Channel::Melodic)?;
    let p2 = parse("pulse2", song.pulse2, Channel::Melodic)?;
    let tri = parse("triangle", song.triangle, Channel::Melodic)?;
    let noise = parse("noise", song.noise, Channel::Drums)?;

    let beats = [&p1, &p2, &tri, &noise].iter().map(|t| t.length).fold(0.0, f64::max);
    if beats <= 0.0 {
        return Err(format!("song \"{}\" is empty", song.title));
    }
    if !song.bpm.is_finite() || song.bpm <= 0.0 {
        return Err(format!("song \"{}\": bpm must be positive", song.title));
    }
    let timing = Timing { samples_per_beat: SR as f64 * 60.0 / song.bpm as f64 };
    let [p1, p2, tri, noise] = [p1, p2, tri, noise].map(|t| apply_swing(&t, song.swing));
    let len = timing.at(beats);
    let tail = (TAIL * SR) as usize;
    let mut out = vec![Frame::ZERO; len + tail];

    render_pulse(&mut out, &p1, &timing, pan(PULSE1_PAN, PULSE_GAIN), true);
    render_pulse(&mut out, &p2, &timing, pan(PULSE2_PAN, PULSE_GAIN), false);
    render_triangle(&mut out, &tri, &timing, pan(0.0, TRIANGLE_GAIN));
    render_drums(&mut out, &noise, &timing, pan(0.0, NOISE_GAIN));

    if song.looping {
        // Wrap whatever rings past the loop point onto the start.
        let (head, spill) = out.split_at_mut(len);
        for (i, f) in spill.iter().enumerate() {
            head[i % len] += *f;
        }
        out.truncate(len);
    } else {
        // Trim trailing silence (keeping a few ms), then make sure it ends at zero.
        let last = out.iter().rposition(|f| f.left.abs().max(f.right.abs()) > 1e-4).unwrap_or(0);
        out.truncate((last + 64).min(out.len()));
        fade_out(&mut out, 0.005);
    }
    master(&mut out);
    Ok(Rendered { frames: out, sample_rate: SAMPLE_RATE, looping: song.looping })
}

struct Timing {
    samples_per_beat: f64,
}

impl Timing {
    /// Sample index of a beat time.
    fn at(&self, beats: f64) -> usize {
        (beats * self.samples_per_beat).round() as usize
    }

    fn span(&self, e: &Event) -> (usize, usize) {
        (self.at(e.start), self.at(e.start + e.dur))
    }
}

/// Left/right gains for a pan position.
fn pan(p: f32, gain: f32) -> (f32, f32) {
    (gain * (1.0 - p), gain * (1.0 + p))
}

/// The note events of a track, each with whether the next one slurs out of it.
fn notes(track: &Track) -> impl Iterator<Item = (&Event, u8, bool)> {
    let ev = &track.events;
    ev.iter().enumerate().filter_map(move |(i, e)| match e.kind {
        EventKind::Note(n) => {
            let slur_out = ev.get(i + 1).is_some_and(|next| {
                next.tie && matches!(next.kind, EventKind::Note(_)) && (next.start - (e.start + e.dur)).abs() < 1e-9
            });
            Some((e, n, slur_out))
        }
        _ => None,
    })
}

/// Linear attack/release ramps, so note starts and ends never click.
#[derive(Clone, Copy)]
struct Edges {
    attack: f32,
    release: f32,
}

impl Edges {
    fn new(e: &Event, slur_out: bool, n: usize) -> Self {
        // Short notes get proportionally shorter ramps.
        let a = if e.tie { 0.0 } else { (ATTACK * SR).min(n as f32 / 4.0).max(1.0) };
        let r = if slur_out { 0.0 } else { (RELEASE * SR).min(n as f32 / 3.0).max(1.0) };
        Edges { attack: a, release: r }
    }

    #[inline]
    fn gain(&self, i: usize, n: usize) -> f32 {
        let mut g = 1.0;
        if self.attack > 0.0 {
            g *= (i as f32 / self.attack).min(1.0);
        }
        if self.release > 0.0 {
            g *= ((n - i) as f32 / self.release).min(1.0);
        }
        g
    }
}

/// PolyBLEP residual for a discontinuity at phase 0 (`t` in [0, 1), `dt` phase step).
#[inline]
fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let x = t / dt;
        x + x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + x + x + 1.0
    } else {
        0.0
    }
}

pub const DUTIES: [f32; 4] = [0.125, 0.25, 0.5, 0.75];

/// Band-limited pulse wave, DC-free (mean 0): high for `duty` of the cycle.
#[inline]
pub fn pulse(phase: f32, dt: f32, duty: f32) -> f32 {
    let mut v = if phase < duty { 1.0 } else { -1.0 };
    v += poly_blep(phase, dt);
    let mut t2 = phase - duty;
    if t2 < 0.0 {
        t2 += 1.0;
    }
    v -= poly_blep(t2, dt);
    v - (2.0 * duty - 1.0)
}

/// The NES triangle: 32 steps, 4-bit, in -1..=1.
#[inline]
pub fn triangle(phase: f32) -> f32 {
    let step = (phase * 32.0) as u32 & 31;
    let level = if step < 16 { 15 - step } else { step - 16 };
    level as f32 / 7.5 - 1.0
}

#[inline]
fn advance(phase: &mut f32, dt: f32) {
    *phase += dt;
    if *phase >= 1.0 {
        *phase -= phase.floor();
    }
}

fn render_pulse(out: &mut [Frame], track: &Track, timing: &Timing, (gl, gr): (f32, f32), vibrato: bool) {
    let mut phase = 0.0f32;
    // Gentle decay towards 65% over ~a second, like a soft NES envelope.
    let decay = (-1.0 / (0.8 * SR)).exp();
    let vib_delay = (0.18 * SR) as usize;
    let vib_ramp = 0.25 * SR;
    let vib_rate = 5.5 / SR;
    for (e, note, slur_out) in notes(track) {
        let (s0, s1) = timing.span(e);
        let n = s1.saturating_sub(s0);
        if n == 0 || e.volume == 0 {
            continue;
        }
        let edges = Edges::new(e, slur_out, n);
        let base_dt = midi_to_hz(note) / SR;
        let duty = DUTIES[e.duty as usize];
        let amp = e.volume as f32 / 15.0;
        let mut env = 1.0f32;
        let mut lfo = 0.0f32;
        for (i, f) in out[s0..s1].iter_mut().enumerate() {
            let mut dt = base_dt;
            if vibrato && i > vib_delay {
                let depth = (((i - vib_delay) as f32) / vib_ramp).min(1.0) * 0.006;
                advance(&mut lfo, vib_rate);
                dt *= 1.0 + depth * (triangle_lfo(lfo));
            }
            let v = pulse(phase, dt, duty) * amp * (0.65 + 0.35 * env) * edges.gain(i, n);
            env *= decay;
            advance(&mut phase, dt);
            f.left += v * gl;
            f.right += v * gr;
        }
    }
}

/// Smooth-ish -1..1 triangle LFO.
#[inline]
fn triangle_lfo(phase: f32) -> f32 {
    4.0 * (phase - 0.5).abs() - 1.0
}

fn render_triangle(out: &mut [Frame], track: &Track, timing: &Timing, (gl, gr): (f32, f32)) {
    let mut phase = 0.0f32;
    for (e, note, slur_out) in notes(track) {
        let (s0, s1) = timing.span(e);
        let n = s1.saturating_sub(s0);
        if n == 0 || e.volume == 0 {
            continue;
        }
        let edges = Edges::new(e, slur_out, n);
        let dt = midi_to_hz(note) / SR;
        let amp = e.volume as f32 / 15.0;
        for (i, f) in out[s0..s1].iter_mut().enumerate() {
            let v = triangle(phase) * amp * edges.gain(i, n);
            advance(&mut phase, dt);
            f.left += v * gl;
            f.right += v * gr;
        }
    }
}

/// The NES noise generator: a 15-bit LFSR. Short mode (tap 6) gives a metallic buzz.
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
    pub fn with_seed(seed: u16) -> Self {
        Lfsr { reg: seed.max(1) & 0x7fff, acc: 0.0 }
    }

    /// Clock the register `clock_hz / SR` times (fractionally) and return the current bit as ±1.
    #[inline]
    pub fn next(&mut self, clock_hz: f32, short: bool) -> f32 {
        self.acc += clock_hz / SR;
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            let tap = if short { 6 } else { 1 };
            let fb = (self.reg ^ (self.reg >> tap)) & 1;
            self.reg = (self.reg >> 1) | (fb << 14);
        }
        if self.reg & 1 == 0 { 1.0 } else { -1.0 }
    }
}

/// Natural length of each drum, in seconds.
fn drum_len(d: Drum) -> f32 {
    match d {
        Drum::Kick => 0.16,
        Drum::Snare => 0.2,
        Drum::ClosedHat => 0.05,
        Drum::OpenHat => 0.3,
    }
}

fn render_drums(out: &mut [Frame], track: &Track, timing: &Timing, (gl, gr): (f32, f32)) {
    let mut lfsr = Lfsr::default();
    let hits: Vec<_> =
        track.events.iter().filter(|e| matches!(e.kind, EventKind::Drum(_)) && e.volume > 0).collect();
    for (k, e) in hits.iter().enumerate() {
        let EventKind::Drum(d) = e.kind else { unreachable!() };
        let s0 = timing.at(e.start);
        let mut n = (drum_len(d) * SR) as usize;
        // The open hat is choked by the next hit (with a short fade).
        let choke = (d == Drum::OpenHat)
            .then(|| hits.get(k + 1).map(|next| timing.at(next.start).saturating_sub(s0)))
            .flatten();
        if let Some(c) = choke {
            n = n.min(c + (0.004 * SR) as usize);
        }
        let n = n.min(out.len().saturating_sub(s0));
        let amp = e.volume as f32 / 15.0;
        let mut buf = vec![0.0f32; n];
        drum(d, &mut buf, &mut lfsr);
        let fade_from = choke.unwrap_or(usize::MAX);
        for (i, (f, v)) in out[s0..s0 + n].iter_mut().zip(buf).enumerate() {
            let mut v = v * amp;
            if i >= fade_from {
                v *= 1.0 - (i - fade_from) as f32 / (0.004 * SR);
            }
            f.left += v * gl;
            f.right += v * gr;
        }
    }
}

/// Synthesize one drum hit into `buf` (its length is the hit's length; ends at zero).
pub fn drum(d: Drum, buf: &mut [f32], lfsr: &mut Lfsr) {
    let n = buf.len();
    let decay = |tau: f32| (-1.0 / (tau * SR)).exp();
    let mut phase = 0.0f32;
    let mut prev = 0.0f32;
    match d {
        Drum::Kick => {
            // Pitch-dropping stepped triangle "boomp" plus a tiny noise click.
            let (k_amp, k_pitch) = (decay(0.07), decay(0.025));
            let (mut a, mut p) = (1.0f32, 1.0f32);
            for (i, s) in buf.iter_mut().enumerate() {
                let f = 48.0 + 130.0 * p;
                advance(&mut phase, f / SR);
                let click = if i < (0.004 * SR) as usize { lfsr.next(12_000.0, false) * 0.3 } else { 0.0 };
                *s = (triangle(phase) * 1.1 + click) * a;
                a *= k_amp;
                p *= k_pitch;
            }
        }
        Drum::Snare => {
            let (k_noise, k_tone) = (decay(0.055), decay(0.03));
            let (mut a, mut t) = (1.0f32, 1.0f32);
            for s in buf.iter_mut() {
                advance(&mut phase, 185.0 / SR);
                *s = lfsr.next(18_000.0, false) * a * 0.75 + triangle(phase) * t * 0.6;
                a *= k_noise;
                t *= k_tone;
            }
        }
        Drum::ClosedHat | Drum::OpenHat => {
            let tau = if d == Drum::ClosedHat { 0.012 } else { 0.07 };
            let k = decay(tau);
            let mut a = 0.55f32;
            for s in buf.iter_mut() {
                let x = lfsr.next(220_000.0, false);
                // First difference: a crude high-pass, keeps hats thin and bright.
                *s = (x - prev) * 0.5 * a;
                prev = x;
                a *= k;
            }
        }
    }
    // Start from zero (a 1ms ramp keeps the attack punchy without a pop)...
    let ramp = ((0.001 * SR) as usize).min(n);
    for (i, s) in buf.iter_mut().take(ramp).enumerate() {
        *s *= i as f32 / ramp as f32;
    }
    // ...and end exactly at zero.
    let ramp = ((0.003 * SR) as usize).min(n);
    for i in 0..ramp {
        buf[n - 1 - i] *= i as f32 / ramp as f32;
    }
}

/// Fade the last `secs` of the buffer linearly to zero.
pub fn fade_out(out: &mut [Frame], secs: f32) {
    let n = out.len();
    let ramp = ((secs * SR) as usize).min(n);
    for i in 0..ramp {
        out[n - 1 - i] *= i as f32 / ramp as f32;
    }
}

/// Soft knee above 0.8, hard ceiling at 1.0: catches the rare stacked peak without
/// squashing the whole mix.
#[inline]
pub fn soft_clip(x: f32) -> f32 {
    const KNEE: f32 = 0.8;
    let a = x.abs();
    if a <= KNEE {
        x
    } else {
        let over = (a - KNEE) / (1.0 - KNEE);
        (KNEE + (1.0 - KNEE) * over.tanh()).min(1.0).copysign(x)
    }
}

/// Final limiter over the mix.
pub fn master(out: &mut [Frame]) {
    for f in out {
        f.left = soft_clip(f.left);
        f.right = soft_clip(f.right);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(bpm: f32, swing_amt: f32, looping: bool, p1: &'static str, tri: &'static str, noise: &'static str) -> Song {
        Song { title: "test", bpm, swing: swing_amt, looping, pulse1: p1, pulse2: "", triangle: tri, noise, key: 0, chords: "" }
    }

    fn left(r: &Rendered) -> Vec<f32> {
        r.frames.iter().map(|f| f.left).collect()
    }

    /// Frequency from upward zero crossings over a window.
    fn freq(x: &[f32]) -> f32 {
        let ups: Vec<usize> = (1..x.len()).filter(|&i| x[i - 1] < 0.0 && x[i] >= 0.0).collect();
        let (a, b) = (ups[0], *ups.last().unwrap());
        (ups.len() - 1) as f32 * SR / (b - a) as f32
    }

    #[test]
    fn pulse_c4_is_middle_c() {
        // One 2s whole note at 120bpm; ignore the edges.
        let r = render_song(&song(120.0, 0.0, true, "c1", "", "")).unwrap();
        let x = left(&r);
        assert_eq!(x.len(), 2 * SAMPLE_RATE as usize);
        let f = freq(&x[3200..60000]);
        assert!((f - 261.63).abs() < 0.5, "{f}");
        // Triangle too, and an octave up with `>`.
        let r = render_song(&song(120.0, 0.0, true, "", "o3 a1", "")).unwrap();
        let f = freq(&left(&r)[3200..60000]);
        assert!((f - 220.0).abs() < 0.5, "{f}");
    }

    #[test]
    fn vibrato_is_subtle() {
        // Pulse 1 vibrato kicks in late; pitch stays within a few cents on average.
        let r = render_song(&song(60.0, 0.0, true, "a1", "", "")).unwrap();
        let f = freq(&left(&r)[0..4 * SAMPLE_RATE as usize - 1000]);
        assert!((f - 440.0).abs() < 2.0, "{f}");
    }

    /// Sample index where the signal first becomes audible after `from`.
    fn onset(x: &[f32], from: usize) -> usize {
        from + x[from..].iter().position(|v| v.abs() > 0.01).unwrap()
    }

    #[test]
    fn swing_delays_offbeat_eighths() {
        let t = mml::parse("g8. g16 d8 b-8 c8 d8 e4 r8 f8 g16 a16 b8", Channel::Melodic).unwrap();
        let sw = apply_swing(&t, 1.0 / 3.0);
        let starts: Vec<f64> = sw.events.iter().map(|e| e.start).collect();
        let d = 1.0 / 6.0;
        let want = [0.0, 0.75, 1.0, 1.5 + d, 2.0, 2.5 + d, 3.0, 4.0, 4.5 + d, 5.0, 5.25, 5.5 + d];
        for (got, want) in starts.iter().zip(want) {
            assert!((got - want).abs() < 1e-6, "{starts:?}");
        }
        // Contiguous events stay contiguous; the total length doesn't change.
        for w in sw.events.windows(2) {
            assert!((w[0].start + w[0].dur - w[1].start).abs() < 1e-9);
        }
        assert_eq!(sw.length, t.length);
        // Rendered: "c8 r8 c8 r8" at 60bpm (1 beat = 1s) with triplet swing: the second note
        // starts at beat 1 (on-beat), and an off-beat one would start at 1/3 + 1/3 ...
        let r = render_song(&song(60.0, 1.0 / 3.0, true, "r8 c8 r8 c8", "", "")).unwrap();
        let x = left(&r);
        let sr = SAMPLE_RATE as usize;
        let first = onset(&x, 0);
        assert!((first as i64 - (2 * sr / 3) as i64).abs() < 10, "{first}");
        let second = onset(&x, sr);
        assert!((second as i64 - (sr + 2 * sr / 3) as i64).abs() < 10, "{second}");
    }

    #[test]
    fn ties_have_no_gap_and_separate_notes_do() {
        // Two separate c8s dip to (near) zero between them; a tie doesn't.
        let at = |src: &'static str| {
            let r = render_song(&song(60.0, 0.0, true, src, "", "")).unwrap();
            let x = left(&r);
            let mid = SAMPLE_RATE as usize / 2;
            x[mid - 40..mid + 40].iter().fold(0.0f32, |m, v| m.max(v.abs()))
        };
        let separate = at("c8 c8 r4");
        let tied = at("c8&c8 r4");
        let slurred = at("c8&d8 r4");
        assert!(separate < tied * 0.8, "{separate} vs {tied}");
        assert!(slurred > tied * 0.9, "{slurred} vs {tied}");
    }

    #[test]
    fn repeats_and_length() {
        // [c8 r8]2 = 2 beats; triangle 3 beats: loop is the longest (3 beats at 120bpm = 1.5s).
        let r = render_song(&song(120.0, 0.0, true, "[c8 r8]2", "c2.", "")).unwrap();
        assert_eq!(r.frames.len(), 48_000);
        // Non-looping songs ring out and end at zero.
        let r = render_song(&song(120.0, 0.0, false, "c4", "", "k4")).unwrap();
        assert!(r.frames.len() > 16_000 && r.frames.len() < 16_000 + (TAIL * SR) as usize + 64);
        let last = r.frames.last().unwrap();
        assert_eq!((last.left, last.right), (0.0, 0.0));
    }

    #[test]
    fn drums_wrap_around_the_loop() {
        // A kick on the very last 16th rings past the loop end; its tail lands at the start.
        let r = render_song(&song(120.0, 0.0, true, "", "", "r2. r8. k16")).unwrap();
        let x = left(&r);
        assert!(x[..1000].iter().any(|v| v.abs() > 0.01));
    }

    #[test]
    fn pans_are_light() {
        let r = render_song(&song(120.0, 0.0, true, "c1", "", "")).unwrap();
        let (l, rr) = r.frames.iter().fold((0.0f32, 0.0f32), |(a, b), f| (a.max(f.left.abs()), b.max(f.right.abs())));
        assert!(l > rr && rr > 0.5 * l, "{l} {rr}");
    }

    #[test]
    fn errors_name_the_channel() {
        let e = render_song(&song(120.0, 0.0, true, "c", "", "x")).unwrap_err();
        assert!(e.contains("noise") && e.contains("test"), "{e}");
    }

    #[test]
    fn soft_clip_never_exceeds_one() {
        for x in [-5.0, -1.0, -0.81, 0.0, 0.5, 0.8, 0.9, 1.0, 3.0, 100.0] {
            let y = soft_clip(x);
            assert!(y.abs() <= 1.0 && y.signum() == x.signum() || x == 0.0);
        }
        assert_eq!(soft_clip(0.5), 0.5);
    }
}
