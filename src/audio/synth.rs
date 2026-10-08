//! The NES-flavoured synth's building blocks, and offline rendering.
//!
//! Voices (one per MML channel, each monophonic; streamed by [`super::live::voice`]):
//! - pulse 1 / pulse 2: band-limited (PolyBLEP) pulse waves with 4 duty cycles, a short
//!   attack/release ramp so notes never click, a gentle decay, and (pulse 1 only) a delayed
//!   vibrato on long notes. Pulse 1 sits slightly left, pulse 2 slightly right.
//! - triangle: the NES's 32-step (4-bit) stepped triangle, for that gritty bass.
//! - noise: drums built from a 15-bit LFSR noise generator plus pitch-swept triangle blips.
//!
//! The sound effects ([`super::sfx`]) are built from the same blocks.
//!
//! # Offline
//! [`render_song_with`] and friends render a song to PCM by running the live engine
//! ([`super::live::Engine`], at freedom 0) for as long as it takes, so there's one
//! implementation of the music: a looping song is rendered exactly one loop long, its second
//! pass (whose start has the end's drum tails ringing over it, so the loop is seamless); a
//! one-shot rings out and ends at zero. The examples and tests use it; the game plays the engine
//! live.

use kira::Frame;

use super::live::instrument::Kit;
use super::live::{Engine, EngineConfig, Input, song::SongFile};
use super::mml::{Event, Track};
use super::tuning::Tuning;
use super::{Filters, Harmony};

/// The engine's sample rate (and of every render). 32 kHz leaves plenty of headroom above
/// the highest notes and hats.
pub const SAMPLE_RATE: u32 = 32_000;
const SR: f32 = SAMPLE_RATE as f32;

// Mix levels (linear). Pulse waves at narrow duty peak at 1.75 (they're DC-centred).
pub const PULSE_GAIN: f32 = 0.15;
pub const TRIANGLE_GAIN: f32 = 0.30;
pub const NOISE_GAIN: f32 = 0.30;
/// Stereo placement of the pulses: -1 left .. 1 right. Kept small: mostly centred.
pub const PULSE1_PAN: f32 = -0.2;
pub const PULSE2_PAN: f32 = 0.2;
/// Note edges, in seconds.
pub const ATTACK: f32 = 0.002;
pub const RELEASE: f32 = 0.008;
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

/// Arpeggio speed: one chord tone per NES frame.
pub const ARP_STEP: f32 = 1.0 / 60.0;

/// Render a song as written (no filters).
pub fn render_song(song: &SongFile) -> Result<Rendered, String> {
    render_song_with(song, Filters::default(), 0)
}

/// Render a song through `filters`; `seed` varies the generated accompaniment. Errors if the
/// song can't take the harmony (no chord chart).
pub fn render_song_with(song: &SongFile, filters: Filters, seed: u64) -> Result<Rendered, String> {
    render(song, filters, seed, None)
}

/// Render a song through `filters` in an alternative [`Tuning`] (which replaces
/// `filters.just_intonation`; [`Tuning::Medley`] is the same retuning).
pub fn render_song_tuned(song: &SongFile, filters: Filters, tuning: Tuning) -> Result<Rendered, String> {
    render(song, filters, 0, Some(tuning))
}

/// Run the engine offline (see the module docs).
fn render(song: &SongFile, filters: Filters, seed: u64, tuning: Option<Tuning>) -> Result<Rendered, String> {
    let mut e = Engine::with_config(song, SAMPLE_RATE, EngineConfig { seed, ..EngineConfig::default() })?;
    if !e.can_play(filters.harmony) {
        return Err(format!("song \"{}\" has no chord chart to reharmonize", song.title));
    }
    e.post(Input::SetFilters(filters));
    if tuning.is_some() {
        e.post(Input::ForceTuning(tuning));
    }
    let shape = match e.waltz_shape() {
        Some(w) if filters.harmony == Harmony::Waltz => w,
        _ => e.shape(),
    };
    let len = shape.len as usize;
    let fill = |e: &mut Engine, n: usize| {
        let mut out = vec![Frame::ZERO; n];
        for chunk in out.chunks_mut(4096) {
            e.fill(chunk);
        }
        out
    };
    if song.looping {
        fill(&mut e, len);
        return Ok(Rendered { frames: fill(&mut e, len), sample_rate: SAMPLE_RATE, looping: true });
    }
    // A one-shot rings out, then the trailing silence goes (keeping a few ms) and it ends at zero.
    let mut out = fill(&mut e, len + (TAIL * SR) as usize);
    let last = out.iter().rposition(|f| f.left.abs().max(f.right.abs()) > 1e-4).unwrap_or(0);
    out.truncate((last + 64).min(out.len()));
    fade_out(&mut out, 0.005);
    Ok(Rendered { frames: out, sample_rate: SAMPLE_RATE, looping: false })
}

/// Left/right gains for a pan position.
pub fn pan(p: f32, gain: f32) -> (f32, f32) {
    (gain * (1.0 - p), gain * (1.0 + p))
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

/// Advance an oscillator phase (0..1) by `dt`.
#[inline]
pub fn advance(phase: &mut f32, dt: f32) {
    *phase += dt;
    if *phase >= 1.0 {
        *phase -= phase.floor();
    }
}

/// Smooth-ish -1..1 triangle LFO.
#[inline]
pub fn triangle_lfo(phase: f32) -> f32 {
    4.0 * (phase - 0.5).abs() - 1.0
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

    /// Clock the register `clock_hz / SAMPLE_RATE` times (fractionally) and return the current
    /// bit as ±1.
    #[inline]
    pub fn next(&mut self, clock_hz: f32, short: bool) -> f32 {
        self.next_at(clock_hz, short, SR)
    }

    /// [`Lfsr::next`] at sample rate `sr`.
    #[inline]
    pub fn next_at(&mut self, clock_hz: f32, short: bool, sr: f32) -> f32 {
        self.acc += clock_hz / sr;
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            let tap = if short { 6 } else { 1 };
            let fb = (self.reg ^ (self.reg >> tap)) & 1;
            self.reg = (self.reg >> 1) | (fb << 14);
        }
        if self.reg & 1 == 0 { 1.0 } else { -1.0 }
    }
}

/// The built-in open hat's length, in seconds.
pub const MAX_DRUM_SECS: f32 = 0.3;

/// Natural length of each drum of the built-in kit, in seconds.
pub fn drum_len(d: super::mml::Drum) -> f32 {
    kit_drum_len(d, &Kit::DEFAULT)
}

/// Natural length of a drum of `kit`, in seconds.
pub fn kit_drum_len(d: super::mml::Drum, kit: &Kit) -> f32 {
    use super::mml::Drum;
    match d {
        Drum::Kick => kit.kick.len,
        Drum::Snare => kit.snare.len,
        Drum::ClosedHat => kit.hat.len,
        Drum::OpenHat => kit.ohat.len,
        Drum::Crash => kit.crash.len,
    }
}

/// Synthesize one hit of the built-in kit into `buf` at sample rate `sr` (its length is the
/// hit's length; ends at zero).
pub fn drum(d: super::mml::Drum, buf: &mut [f32], lfsr: &mut Lfsr, sr: f32) {
    drum_kit(d, &Kit::DEFAULT, buf, lfsr, sr)
}

/// [`drum`] with `kit`'s parameters ([`super::live::instrument::Kit`]).
pub fn drum_kit(d: super::mml::Drum, kit: &Kit, buf: &mut [f32], lfsr: &mut Lfsr, sr: f32) {
    use super::mml::Drum;
    let n = buf.len();
    let decay = |tau: f32| (-1.0 / (tau * sr)).exp();
    let mut phase = 0.0f32;
    let mut prev = 0.0f32;
    match d {
        Drum::Kick => {
            // Pitch-dropping stepped triangle "boomp" plus a tiny noise click.
            let k = &kit.kick;
            let (k_amp, k_pitch) = (decay(k.amp_tau), decay(k.pitch_tau));
            let (mut a, mut p) = (1.0f32, 1.0f32);
            for (i, s) in buf.iter_mut().enumerate() {
                let f = k.base_hz + k.sweep_hz * p;
                advance(&mut phase, f / sr);
                let click = if i < (0.004 * sr) as usize { lfsr.next_at(12_000.0, false, sr) * k.click } else { 0.0 };
                *s = (triangle(phase) * 1.1 + click) * a;
                a *= k_amp;
                p *= k_pitch;
            }
        }
        Drum::Snare => {
            let sn = &kit.snare;
            let (k_noise, k_tone) = (decay(sn.noise_tau), decay(sn.tone_tau));
            let (mut a, mut t) = (1.0f32, 1.0f32);
            for s in buf.iter_mut() {
                advance(&mut phase, sn.tone_hz / sr);
                *s = lfsr.next_at(sn.noise_hz, sn.short, sr) * a * 0.75 + triangle(phase) * t * 0.6;
                a *= k_noise;
                t *= k_tone;
            }
        }
        Drum::ClosedHat | Drum::OpenHat | Drum::Crash => {
            let m = match d {
                Drum::ClosedHat => &kit.hat,
                Drum::OpenHat => &kit.ohat,
                _ => &kit.crash,
            };
            let k = decay(m.tau);
            let mut a = if d == Drum::Crash { 0.7f32 } else { 0.55f32 };
            for s in buf.iter_mut() {
                let x = lfsr.next_at(m.clock_hz, m.short, sr);
                // First difference: a crude high-pass, keeps hats thin and bright.
                *s = (x - prev) * 0.5 * a;
                prev = x;
                a *= k;
            }
        }
    }
    // Start from zero (a 1ms ramp keeps the attack punchy without a pop)...
    let ramp = ((0.001 * sr) as usize).min(n);
    for (i, s) in buf.iter_mut().take(ramp).enumerate() {
        *s *= i as f32 / ramp as f32;
    }
    // ...and end exactly at zero.
    let ramp = ((0.003 * sr) as usize).min(n);
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
    use crate::audio::mml::{self, Channel};
    use crate::audio::tuning::{self, Medley};

    fn song(bpm: f32, swing: f32, looping: bool, p1: &str, tri: &str, noise: &str) -> SongFile {
        SongFile::from_mml("test", bpm, swing, looping, 0, "", [p1, "", tri, noise]).unwrap()
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
        // Rendered: "r8 c8 r8 c8" at 60bpm (1 beat = 1s) with triplet swing: each c8 is an
        // off-beat 8th, so it starts at 1/2 + 1/6 of its beat.
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
        let at = |src: &str| {
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
        let e = SongFile::from_mml("test", 120.0, 0.0, true, 0, "", ["c", "", "", "z"]).unwrap_err();
        assert!(e.contains("noise") && e.contains("test"), "{e}");
    }

    #[test]
    fn arpeggios_cycle_the_chord_tones() {
        // A C major arpeggio on pulse 2 for 4s; each ARP_STEP window holds one chord tone.
        let s = SongFile::from_mml("test", 60.0, 0.0, true, 0, "", ["", "o4 {c e g}1", "", ""]).unwrap();
        let x = left(&render_song(&s).unwrap());
        let step = (ARP_STEP * SR) as usize;
        let want = [261.63, 329.63, 392.0];
        for j in 1..60 {
            let f = freq(&x[j * step + 20..(j + 1) * step - 20]);
            let w = want[j % 3];
            assert!((f - w).abs() / w < 0.02, "step {j}: {f} vs {w}");
        }
    }

    #[test]
    fn just_intonation_retunes_rendered_notes() {
        // E4 in C, justly tuned: 5/4 above an equal-tempered C4. (The laughing band filter plays
        // the medley now; JI can still be forced.)
        let s = song(120.0, 0.0, true, "e1", "", "");
        let f = freq(&left(&render_song_tuned(&s, Filters::default(), Tuning::Just).unwrap())[3200..60000]);
        assert!((f - 261.626 * 1.25).abs() < 0.5, "{f}");
        let f = freq(&left(&render_song(&s).unwrap())[3200..60000]);
        assert!((f - 329.63).abs() < 0.5, "{f}");
    }

    #[test]
    fn medley_notes_follow_their_phrase() {
        // One long note per phrase on pulse 2 (no vibrato), 6 phrases: each is measured
        // against its phrase's tuning (± the medley's drunk detune and wobble).
        let s = SongFile::from_mml("test", 240.0, 0.0, true, 0, "", ["", "v12 @2 [o4 e1 e1 e1 e1]6", "", ""]).unwrap();
        let r = render_song_tuned(&s, Filters::default(), Tuning::Medley).unwrap();
        let m = Medley::new(tuning::hash_str("test"), s.beats(), Some(r.duration_secs() as f64));
        assert_eq!(m.phrases(), 6);
        let anchor = tuning::anchor_tonic(0, 64.0);
        let x: Vec<f32> = r.frames.iter().map(|f| f.left).collect();
        let bar = (4.0 * 60.0 / 240.0 * SR) as usize;
        let mut heard = std::collections::HashSet::new();
        for k in 0..6 {
            // The phrase's first note.
            let t = m.tuning(k);
            heard.insert(t);
            let want = t.hz(64, anchor, 0);
            let f = freq(&x[k * 4 * bar + 400..k * 4 * bar + bar - 400]) as f64;
            let off = 1200.0 * (f / want).log2();
            assert!(off.abs() < tuning::MEDLEY_DRUNK_CENTS + tuning::MEDLEY_WOBBLE_CENTS + 2.0, "phrase {k} {t:?}: {f} vs {want}");
        }
        assert_eq!(heard.len(), 5, "every tuning in the first five phrases");
    }

    #[test]
    fn reharmonizing_needs_a_matching_chart() {
        let f = Filters { harmony: Harmony::Coltrane, just_intonation: false };
        let e = render_song_with(&song(120.0, 0.0, true, "c1", "", ""), f, 0).unwrap_err();
        assert!(e.contains("no chord chart"), "{e}");
        let e = SongFile::from_mml("test", 120.0, 0.0, true, 0, "| C | G7 |", ["c1", "", "", ""]).unwrap_err();
        assert!(e.contains("2 bars") && e.contains("4 beats"), "{e}");
        let e = SongFile::from_mml("test", 120.0, 0.0, true, 0, "| C | Gx |", ["c1 c1", "", "", ""]).unwrap_err();
        assert!(e.contains("bar 2") && e.contains("Gx"), "{e}");
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
