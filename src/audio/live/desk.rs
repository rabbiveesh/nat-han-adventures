//! The mixing desk over the band: what the game does to the music as a whole, after the
//! voices. The *steer* (Side B steers Han with it) and the *hit* (Side B's drum stab, played by
//! the band: on its beat the band drops out under a stab of the chord that's sounding, comp,
//! bass and lead together, with a kick and a crash).
//!
//! The steer is one of two sounds ([`Steer`]): the stereo **balance** (Han follows the sound),
//! or the **record**: forward as it plays, held (the needle dragged to a stop: a tape stop), or
//! scratched backwards (the band's last few seconds, played in reverse; Han moonwalks back).
//! Either way the band plays on underneath: let go and it's where it would have been.
//!
//! Game-free like the rest of the engine: [`Input::SetBalance`], [`Input::SetSteer`] and
//! [`Input::Hit`] ([`super::engine::Input`]) drive it.

use kira::Frame;

use crate::audio::synth::{midi_to_hz, pulse, triangle};

/// The balance glides to where it's sent in this long (s): about a beat at Side B's 192 bpm,
/// the game's turn ([`crate::game::sideb`]) glides the same.
pub const BALANCE_GLIDE_SECS: f32 = 0.3;
/// Panned hard over, the far side keeps this much (phones and laptops are near mono: the
/// near side gets a lift too, so it's heard as a move, not only a placement).
const FAR_SIDE: f32 = 0.2;
const NEAR_LIFT: f32 = 1.15;

/// The band under a stab: down to this, for the stab's hold, then back up over the release.
const DUCK: f32 = 0.18;
/// A stab's hold and release (s), its tones' and its crash's decay.
const HOLD_SECS: f32 = 0.11;
const RELEASE_SECS: f32 = 0.16;
const TONE_DECAY_SECS: f32 = 0.16;
const CRASH_DECAY_SECS: f32 = 0.55;
const KICK_SECS: f32 = 0.09;
/// Levels: each stab tone, the bass, the crash, the kick.
const TONE_GAIN: f32 = 0.16;
const BASS_GAIN: f32 = 0.34;
const CRASH_GAIN: f32 = 0.22;
const KICK_GAIN: f32 = 0.55;
/// How many stabs can be scheduled or ringing at once.
const MAX_STABS: usize = 4;

/// What the steer sounds like.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Steer {
    /// The stereo balance.
    #[default]
    Balance,
    /// The record: 1 forward, 0 held, -1 scratched back.
    Record,
}

/// The record's memory (s) of what the band played: as far back as a scratch goes.
const RECORD_SECS: f32 = 4.0;
/// How fast the record's speed changes (speed per s: a hand on the platter).
pub const RECORD_HAND: f32 = 7.0;
/// Let go, the needle spins forward onto the band as it is in this long (s).
const ZIP_SECS: f32 = 0.07;

/// One hit: at sample `at`, these tones (Hz) over this bass.
#[derive(Debug, Clone, Copy)]
struct Stab {
    at: u64,
    tones: [f32; 4],
    n: usize,
    bass: Option<f32>,
    phases: [f32; 5],
    kick_phase: f32,
}

/// The desk: balance and stabs.
pub struct Desk {
    sr: f32,
    balance: f32,
    target: f32,
    stabs: Vec<Stab>,
    noise: u32,
    steer: Steer,
    /// The record: what the band played (a ring, `head` its newest), where the needle is (in
    /// samples back from the newest) and how fast it's going (1 forward, -1 back).
    tape: Vec<Frame>,
    head: usize,
    filled: usize,
    back: f32,
    speed: f32,
}

impl Desk {
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate as f32;
        Desk {
            sr,
            balance: 0.0,
            target: 0.0,
            stabs: Vec::with_capacity(MAX_STABS),
            noise: 1,
            steer: Steer::Balance,
            tape: vec![Frame::ZERO; (RECORD_SECS * sr) as usize],
            head: 0,
            filled: 0,
            back: 0.0,
            speed: 1.0,
        }
    }

    /// Which sound the steer makes (the balance's value is the record's speed then).
    pub fn set_steer(&mut self, s: Steer) {
        self.steer = s;
    }

    /// Glide the balance to `b` (-1 hard left .. 1 hard right).
    pub fn set_balance(&mut self, b: f32) {
        self.target = if b.is_finite() { b.clamp(-1.0, 1.0) } else { 0.0 };
    }

    /// Where the balance is now.
    pub fn balance(&self) -> f32 {
        self.balance
    }

    /// A stab at sample `at` of these notes (MIDI; the bass is the lowest).
    pub fn hit(&mut self, at: u64, mut notes: Vec<u8>) {
        if self.stabs.len() >= MAX_STABS {
            self.stabs.remove(0);
        }
        notes.sort_unstable();
        notes.dedup();
        let bass = notes.first().map(|&n| midi_to_hz(lower_into(n, 36, 48)));
        // The voicing: the rest, up into the horns' register, top four.
        let mut up: Vec<u8> = notes.iter().skip(1).map(|&n| lower_into(n, 60, 72)).collect();
        if up.is_empty() {
            up = notes.iter().map(|&n| lower_into(n, 60, 72)).collect();
        }
        up.sort_unstable();
        up.dedup();
        let mut tones = [0.0; 4];
        let n = up.len().min(4);
        for (t, &m) in tones.iter_mut().zip(up.iter().rev().take(4)) {
            *t = midi_to_hz(m);
        }
        self.stabs.push(Stab { at, tones, n, bass, phases: [0.0; 5], kick_phase: 0.0 });
    }

    /// Run the desk over the band's mix of samples `t0..t0 + out.len()`.
    pub fn process(&mut self, out: &mut [Frame], t0: u64) {
        let glide = 2.0 / (BALANCE_GLIDE_SECS * self.sr);
        if self.steer == Steer::Record {
            self.record(out);
            return self.stabs(out, t0, |_| (1.0, 1.0));
        }
        let quiet = self.stabs.is_empty() && self.balance == self.target && self.balance == 0.0;
        if quiet {
            return;
        }
        let target = self.target;
        let mut b = self.balance;
        self.stabs(out, t0, |_| {
            let g = gains(b);
            if b != target {
                b = if (target - b).abs() <= glide { target } else { b + glide * (target - b).signum() };
            }
            g
        });
        self.balance = b;
    }

    /// The record over the band: kept, and played back at the needle (forward at speed 1 it's
    /// the band as it plays).
    fn record(&mut self, out: &mut [Frame]) {
        let n = self.tape.len();
        let hand = RECORD_HAND / self.sr;
        for f in out.iter_mut() {
            self.head = (self.head + 1) % n;
            self.tape[self.head] = *f;
            self.filled = (self.filled + 1).min(n - 2);
            let want = self.target;
            // Let go forward: the needle drops back on the band as it is (a quick zip forward).
            if want > 0.5 && self.speed > 0.0 {
                // (A spin forward over the gap, in `ZIP_SECS`.)
                if self.speed <= 1.0 && self.back > 0.0 {
                    self.speed = 1.0 + self.back / (ZIP_SECS * self.sr);
                }
                self.back = (self.back - (self.speed - 1.0)).max(0.0);
                if self.back <= 0.0 {
                    self.speed = 1.0;
                }
            } else {
                self.speed += (want - self.speed).clamp(-hand, hand);
                // (Each sample the band moves on one: the needle falls back by what it lags.)
                self.back = (self.back + 1.0 - self.speed).clamp(0.0, self.filled as f32);
                if self.back >= self.filled as f32 {
                    // Out of record: it holds there.
                    self.speed = self.speed.max(0.0);
                }
            }
            if self.back == 0.0 && self.speed == 1.0 {
                continue;
            }
            let at = self.back.floor();
            let frac = self.back - at;
            let i = (self.head + n - at as usize) % n;
            let j = (i + n - 1) % n;
            let s = self.tape[i] * (1.0 - frac) + self.tape[j] * frac;
            // (Slow, it's quieter: the platter barely turning.)
            *f = s * self.speed.abs().min(1.0).sqrt();
        }
    }

    /// Stabs over `out`, then each sample's left/right gains from `pan`.
    fn stabs(&mut self, out: &mut [Frame], t0: u64, mut pan: impl FnMut(usize) -> (f32, f32)) {
        for (i, f) in out.iter_mut().enumerate() {
            let t = t0 + i as u64;
            let mut duck = 1.0f32;
            let mut add = 0.0f32;
            for s in &mut self.stabs {
                if t < s.at {
                    continue;
                }
                let secs = (t - s.at) as f32 / self.sr;
                let d = if secs < HOLD_SECS {
                    DUCK
                } else {
                    DUCK + (1.0 - DUCK) * ((secs - HOLD_SECS) / RELEASE_SECS).min(1.0)
                };
                duck = duck.min(d);
                // The horns: a bright stab that dies away, the bass an octave under.
                let env = (-secs / TONE_DECAY_SECS).exp() * (secs * 400.0).min(1.0);
                for k in 0..s.n {
                    let dt = s.tones[k] / self.sr;
                    add += pulse(s.phases[k], dt, 0.25) * TONE_GAIN * env;
                    s.phases[k] = (s.phases[k] + dt).fract();
                }
                if let Some(b) = s.bass {
                    add += triangle(s.phases[4]) * BASS_GAIN * (-secs / (TONE_DECAY_SECS * 1.5)).exp();
                    s.phases[4] = (s.phases[4] + b / self.sr).fract();
                }
                // The kick (a falling sine) and the crash (noise).
                if secs < KICK_SECS {
                    let hz = 150.0 * (1.0 - secs / KICK_SECS) + 45.0;
                    add += (s.kick_phase * std::f32::consts::TAU).sin() * KICK_GAIN * (1.0 - secs / KICK_SECS);
                    s.kick_phase = (s.kick_phase + hz / self.sr).fract();
                }
                self.noise ^= self.noise << 13;
                self.noise ^= self.noise >> 17;
                self.noise ^= self.noise << 5;
                let white = (self.noise as f32 / u32::MAX as f32) * 2.0 - 1.0;
                add += white * CRASH_GAIN * (-secs / CRASH_DECAY_SECS).exp();
            }
            let (l, r) = pan(i);
            f.left = (f.left * duck + add) * l;
            f.right = (f.right * duck + add) * r;
        }
        let sr = self.sr;
        let end = t0 + out.len() as u64;
        self.stabs.retain(|s| end < s.at || ((end - s.at) as f32 / sr) < CRASH_DECAY_SECS * 5.0);
    }
}

/// Left and right gains at balance `b`.
pub fn gains(b: f32) -> (f32, f32) {
    let side = |x: f32| if x > 0.0 { 1.0 + (NEAR_LIFT - 1.0) * x } else { 1.0 + (FAR_SIDE - 1.0) * -x };
    (side(-b), side(b))
}

/// `n` moved by octaves into `lo..hi` (or as near as it goes).
fn lower_into(mut n: u8, lo: u8, hi: u8) -> u8 {
    while n >= hi && n >= 12 {
        n -= 12;
    }
    while n < lo {
        n += 12;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hard_over_keeps_a_little_of_the_far_side() {
        let (l, r) = gains(1.0);
        assert!((l - FAR_SIDE).abs() < 1e-6 && r > 1.0, "{l} {r}");
        assert_eq!(gains(0.0), (1.0, 1.0));
    }

    #[test]
    fn a_stab_ducks_the_band_and_plays_on_its_sample() {
        let mut d = Desk::new(32_000);
        d.hit(1000, vec![48, 64, 67, 71]);
        let mut out = vec![Frame { left: 0.5, right: 0.5 }; 2000];
        d.process(&mut out, 0);
        assert_eq!(out[999].left, 0.5, "nothing before it");
        let after: f32 = out[1100..1900].iter().map(|f| (f.left - 0.5 * DUCK).abs()).sum();
        assert!(after > 1.0, "the stab sounds over the ducked band");
    }

    #[test]
    fn the_record_plays_back_what_the_band_played_backwards() {
        let mut d = Desk::new(1000);
        d.set_steer(Steer::Record);
        d.set_balance(1.0);
        // A rising ramp: forward, it's as played.
        let ramp = |t0: usize| (t0..t0 + 500).map(|t| Frame { left: t as f32, right: t as f32 }).collect::<Vec<_>>();
        let mut a = ramp(0);
        d.process(&mut a, 0);
        assert_eq!(a[499].left, 499.0);
        d.set_balance(-1.0);
        let mut b = ramp(500);
        d.process(&mut b, 500);
        // Slowed to a stop and back: by the end it's going down, and below where it was.
        assert!(b[499].left < b[400].left, "backwards: {} then {}", b[400].left, b[499].left);
        assert!(b[499].left < 499.0);
        // Let go: back on the band as it plays.
        d.set_balance(1.0);
        let mut c = ramp(1000);
        d.process(&mut c, 1000);
        assert_eq!(c[499].left, 1499.0);
    }

    #[test]
    fn the_balance_glides() {
        let mut d = Desk::new(32_000);
        d.set_balance(1.0);
        let mut out = vec![Frame { left: 1.0, right: 1.0 }; 32_000];
        d.process(&mut out, 0);
        assert!(out[100].left > 0.9, "not at once");
        assert!((out[31_000].left - FAR_SIDE).abs() < 1e-3, "there within the glide");
        assert_eq!(d.balance(), 1.0);
    }
}
