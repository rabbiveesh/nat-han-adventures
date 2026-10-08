//! Alternative tunings for the synth ("laughing band" candidates), auditioned with
//! `cargo run --release --example tunings`. Each [`Tuning`] is a pure function from a MIDI note to
//! Hz ([`Tuning::hz`]), given the voice's *anchor*: a MIDI note on the song's tonic
//! ([`super::Song::key`]) that keeps its equal-tempered pitch. Octave-equivalent tunings don't
//! care which octave the anchor is in; the two non-octave ones (alpha, Bohlen–Pierce) do, so the
//! synth anchors each voice at the tonic nearest the middle of its own range ([`anchor_tonic`]):
//! every voice's tonic is still an exact equal-tempered octave of every other voice's tonic,
//! but the bass doesn't drift off into subsonics (or up into the melody).
//!
//! # The tunings (cents above the anchor tonic, per chromatic step `d` = semitones above it)
//! - [`Tuning::Equal`]: 12-TET, `100·d`.
//! - [`Tuning::Just`]: the existing 5-limit just intonation ([`theory::JI_RATIOS`]).
//! - [`Tuning::CarlosAlpha`]: Wendy Carlos's α scale, `78·d`, no octaves at all: a 12-TET
//!   octave shrinks to 936 cents, so melodies keep their exact shape at 78% size and every
//!   "octave" is a quarter-tone-ish flat major seventh.
//! - [`Tuning::BohlenPierce`]: 13 equal steps per tritave (3:1, 1901.96¢, ≈146.30¢/step). Each
//!   12-TET octave becomes one tritave, and the 12 pitch classes map monotonically onto 12 of
//!   the 13 steps (see [`BP_STEPS`]), so contour and the whole/half-step pattern survive:
//!
//!   | pc   | C | C# | D | Eb | E | F | F# | G | Ab | A  | Bb | B  | (C') |
//!   |------|---|----|---|----|---|---|----|---|----|----|----|----|------|
//!   | step | 0 | 1  | 2 | 3  | 4 | 5 | 6  | 7 | 9  | 10 | 11 | 12 | (13) |
//!
//!   The Lambda mode is steps `0 2 3 4 6 7 9 10 12`. The major-scale degrees land on Lambda
//!   steps 0 2 4 7 10 12 (proportionally nearest), except the 4th: no monotonic map can put all
//!   seven diatonic degrees on Lambda *and* give the chromatic notes their own steps (F would
//!   need step 6, leaving nothing between F and G for F#), so F takes the non-Lambda step 5.
//!   Chromatic notes take the remaining neighbour steps; only step 8 is unused.
//! - [`Tuning::Tet7`]: 7-TET (171.43¢ steps), octaves kept. Each pitch class goes to the 7-TET
//!   step nearest its 12-TET pitch, `round(pc·7/12)` with the tritone tie going down: every
//!   diatonic degree lands on its own step (C D E F G A B = 0..6) and alterations go to the
//!   nearer neighbour (C#→D, Eb→E, F#→F, Ab→A, Bb→B), so the tune is "diatonic" but every
//!   interval is neutral.
//! - [`Tuning::Harmonic`]: otonal ratios from the harmonic series, octave-equivalent:
//!   `1, 17/16, 9/8, 19/16, 5/4, 11/8, 23/16, 3/2, 13/8, 27/16, 7/4, 15/8`. The tritone is
//!   23/16 (628.27¢, a very sharp #4, rather than 45/32 or 11/8, which is already the 4th); the
//!   major 6th is the Pythagorean 27/16 (905.87¢) since 13/8 is taken by the minor 6th, so 6 and
//!   b6 stay distinct and in order. The 4th (11/8, 551.32¢) is the headline weirdness.
//! - [`Tuning::Drunk`]: 12-TET, but every note-on is off by a random ±40¢ (deterministic per
//!   song + channel + event + arpeggio tone, see [`drunk_cents`]), and the whole pitched mix
//!   wobbles ±15¢ at 0.5 Hz ([`Tuning::wobble`]). Drums aren't pitched, so they don't wobble.

use super::theory::{self, et_hz};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Tuning {
    #[default]
    Equal,
    Just,
    CarlosAlpha,
    BohlenPierce,
    Tet7,
    Harmonic,
    Drunk,
}

/// Wendy Carlos α: cents per chromatic step.
pub const ALPHA_CENTS: f64 = 78.0;
/// Bohlen–Pierce: equal steps per tritave.
pub const BP_DIVISIONS: f64 = 13.0;
/// 12-TET pitch class (semitones above the tonic) → BP step (see the module docs).
pub const BP_STEPS: [i32; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12];
/// 12-TET pitch class → 7-TET step.
pub const TET7_STEPS: [i32; 12] = [0, 1, 1, 2, 2, 3, 3, 4, 5, 5, 6, 6];
/// Harmonic-series ratios, per semitone above the tonic.
pub const HARMONIC_RATIOS: [f64; 12] = [
    1.0,
    17.0 / 16.0,
    9.0 / 8.0,
    19.0 / 16.0,
    5.0 / 4.0,
    11.0 / 8.0,
    23.0 / 16.0,
    3.0 / 2.0,
    13.0 / 8.0,
    27.0 / 16.0,
    7.0 / 4.0,
    15.0 / 8.0,
];
/// Drunk: max random offset per note, in cents.
pub const DRUNK_CENTS: f64 = 40.0;
/// Drunk: wobble depth (cents) and rate (Hz).
pub const WOBBLE_CENTS: f64 = 15.0;
pub const WOBBLE_HZ: f64 = 0.5;

impl Tuning {
    pub const ALL: [Tuning; 7] = [
        Tuning::Equal,
        Tuning::Just,
        Tuning::CarlosAlpha,
        Tuning::BohlenPierce,
        Tuning::Tet7,
        Tuning::Harmonic,
        Tuning::Drunk,
    ];

    /// Lower-case name for files.
    pub fn slug(self) -> &'static str {
        match self {
            Tuning::Equal => "equal",
            Tuning::Just => "ji",
            Tuning::CarlosAlpha => "alpha",
            Tuning::BohlenPierce => "bp",
            Tuning::Tet7 => "tet7",
            Tuning::Harmonic => "harmonic",
            Tuning::Drunk => "drunk",
        }
    }

    /// Frequency of MIDI `note`, given the voice's `anchor` (a MIDI note on the song's tonic, which
    /// keeps its equal-tempered pitch; only its pitch class matters for octave-equivalent tunings)
    /// and a `salt` that identifies the note-on (only [`Tuning::Drunk`] uses it).
    pub fn hz(self, note: u8, anchor: u8, salt: u64) -> f64 {
        let d = note as i32 - anchor as i32;
        let (oct, pc) = (d.div_euclid(12), d.rem_euclid(12) as usize);
        // The tonic at or below the note (for the octave-equivalent tunings).
        let tonic_below = || et_hz((note as i32 - pc as i32) as f64);
        let a = et_hz(anchor as f64);
        match self {
            Tuning::Equal => et_hz(note as f64),
            Tuning::Just => theory::note_hz(note, Some(anchor % 12)),
            Tuning::CarlosAlpha => a * 2f64.powf(d as f64 * ALPHA_CENTS / 1200.0),
            Tuning::BohlenPierce => {
                let steps = oct * BP_DIVISIONS as i32 + BP_STEPS[pc];
                a * 3f64.powf(steps as f64 / BP_DIVISIONS)
            }
            Tuning::Tet7 => tonic_below() * 2f64.powf(TET7_STEPS[pc] as f64 / 7.0),
            Tuning::Harmonic => tonic_below() * HARMONIC_RATIOS[pc],
            Tuning::Drunk => et_hz(note as f64) * 2f64.powf(drunk_cents(salt) / 1200.0),
        }
    }

    /// Does this tuning wobble over time (see [`Tuning::wobble`])?
    pub fn wobbles(self) -> bool {
        self == Tuning::Drunk
    }

    /// Frequency multiplier at `t` seconds into the song (1.0 unless [`Tuning::wobbles`]).
    pub fn wobble(self, t: f64) -> f64 {
        if !self.wobbles() {
            return 1.0;
        }
        let cents = WOBBLE_CENTS * (std::f64::consts::TAU * WOBBLE_HZ * t).sin();
        2f64.powf(cents / 1200.0)
    }
}

/// The MIDI note on tonic pitch class `key` nearest to `center` (ties go down).
pub fn anchor_tonic(key: u8, center: f64) -> u8 {
    let c = center.round() as i32;
    let below = c - (c - key as i32).rem_euclid(12);
    let anchor = if (c - below) <= 6 { below } else { below + 12 };
    anchor.clamp(0, 127) as u8
}

/// Drunk detune for a note-on: uniform-ish in ±[`DRUNK_CENTS`], a pure function of `salt`.
pub fn drunk_cents(salt: u64) -> f64 {
    let x = splitmix(salt);
    let unit = (x >> 11) as f64 / (1u64 << 53) as f64; // [0, 1)
    (unit * 2.0 - 1.0) * DRUNK_CENTS
}

/// A note-on's salt from the song (`song` = e.g. [`hash_str`] of its title), channel, event index
/// and arpeggio tone.
pub fn salt(song: u64, channel: usize, event: usize, tone: usize) -> u64 {
    splitmix(splitmix(splitmix(song ^ channel as u64) ^ event as u64) ^ tone as u64)
}

/// FNV-1a: a stable hash of a string (std's hasher isn't guaranteed stable).
pub fn hash_str(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3))
}

fn splitmix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::theory::cents;

    /// Cents of `note` above the anchor in tuning `t`.
    fn c(t: Tuning, anchor: u8, note: u8) -> f64 {
        cents(et_hz(anchor as f64), t.hz(note, anchor, 0))
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn the_anchor_never_moves() {
        for t in Tuning::ALL.into_iter().filter(|&t| t != Tuning::Drunk) {
            for anchor in [48, 60, 65, 70] {
                assert!(close(c(t, anchor, anchor), 0.0), "{t:?}");
            }
        }
    }

    #[test]
    fn equal_and_just() {
        for d in 0..24 {
            assert!(close(c(Tuning::Equal, 60, 60 + d), 100.0 * d as f64));
        }
        // Same as the existing JI path: E above F (key 5) is 5/4.
        assert!(close(c(Tuning::Just, 65, 69), 386.31));
        assert_eq!(Tuning::Just.hz(64, 60, 0), theory::note_hz(64, Some(0)));
        assert!(close(c(Tuning::Just, 60, 70), 1017.60)); // 9/5
    }

    #[test]
    fn alpha_is_78_cents_per_semitone_without_octaves() {
        assert!(close(c(Tuning::CarlosAlpha, 60, 72), 936.0));
        assert!(close(c(Tuning::CarlosAlpha, 60, 61), 78.0));
        assert!(close(c(Tuning::CarlosAlpha, 60, 48), -936.0));
        assert!(close(c(Tuning::CarlosAlpha, 60, 84), 1872.0));
        // Note-to-note intervals are always multiples of 78 cents.
        let t = Tuning::CarlosAlpha;
        assert!(close(cents(t.hz(63, 60, 0), t.hz(70, 60, 0)), 7.0 * 78.0));
    }

    #[test]
    fn bohlen_pierce_steps_and_tritave() {
        let step = 1200.0 * 3f64.log2() / 13.0;
        assert!(close(step, 146.30));
        let t = Tuning::BohlenPierce;
        assert!(close(c(t, 60, 72), 1901.96)); // a 12-TET octave is a tritave (13 steps)
        assert!(close(c(t, 60, 48), -1901.96));
        for (pc, &s) in BP_STEPS.iter().enumerate() {
            assert!(close(c(t, 60, 60 + pc as u8), s as f64 * step), "pc {pc}");
        }
        // Monotonic: the contour of any melody survives.
        let all: Vec<f64> = (36..96).map(|n| t.hz(n, 60, 0)).collect();
        assert!(all.windows(2).all(|w| w[1] > w[0]));
        // Diatonic degrees on Lambda, except the 4th.
        const LAMBDA: [i32; 9] = [0, 2, 3, 4, 6, 7, 9, 10, 12];
        for pc in [0, 2, 4, 7, 9, 11] {
            assert!(LAMBDA.contains(&BP_STEPS[pc]), "pc {pc}");
        }
    }

    #[test]
    fn tet7_steps() {
        let step = 1200.0 / 7.0;
        assert!(close(step, 171.43));
        let t = Tuning::Tet7;
        // Diatonic degrees 0..6, in any key, and octaves kept.
        for (deg, pc) in [0, 2, 4, 5, 7, 9, 11].into_iter().enumerate() {
            assert!(close(c(t, 62, 62 + pc), deg as f64 * step));
            assert!(close(c(t, 62, 74 + pc), 1200.0 + deg as f64 * step));
        }
        assert!(close(c(t, 60, 70), 6.0 * step)); // Bb → the 7th step
        assert!(close(c(t, 60, 61), step)); // C# → D
    }

    #[test]
    fn harmonic_ratios() {
        let t = Tuning::Harmonic;
        let want = [0.0, 104.96, 203.91, 297.51, 386.31, 551.32, 628.27, 701.96, 840.53, 905.87, 968.83, 1088.27];
        for (pc, w) in want.into_iter().enumerate() {
            assert!(close(c(t, 60, 60 + pc as u8), w), "pc {pc}: {}", c(t, 60, 60 + pc as u8));
            // Octave-equivalent, and the anchor's octave doesn't matter.
            assert!(close(c(t, 60, 84 + pc as u8), 2400.0 + w));
            assert!(close(cents(et_hz(48.0), t.hz(48 + pc as u8, 72, 0)), w));
        }
    }

    #[test]
    fn drunk_offsets_are_bounded_and_deterministic() {
        let song = hash_str("Sweet Georgia Brown");
        let mut lo: f64 = 0.0;
        let mut hi: f64 = 0.0;
        for ch in 0..3 {
            for ev in 0..2000 {
                let s = salt(song, ch, ev, 0);
                let off = cents(et_hz(64.0), Tuning::Drunk.hz(64, 60, s));
                assert!(off.abs() <= DRUNK_CENTS + 1e-9, "{off}");
                assert_eq!(Tuning::Drunk.hz(64, 60, s), Tuning::Drunk.hz(64, 60, salt(song, ch, ev, 0)));
                lo = lo.min(off);
                hi = hi.max(off);
            }
        }
        // It really is drunk: offsets use the whole range.
        assert!(lo < -35.0 && hi > 35.0, "{lo} {hi}");
        assert_ne!(salt(song, 0, 1, 0), salt(song, 1, 0, 0));
        assert_ne!(salt(song, 0, 0, 0), salt(hash_str("other"), 0, 0, 0));
        // The wobble: ±15 cents at 0.5 Hz, peak at t = 0.5s.
        assert!(close(1200.0 * Tuning::Drunk.wobble(0.5).log2(), 15.0));
        assert!(close(1200.0 * Tuning::Drunk.wobble(1.5).log2(), -15.0));
        assert_eq!(Tuning::Harmonic.wobble(0.5), 1.0);
    }

    #[test]
    fn every_song_renders_cleanly_in_every_tuning() {
        use crate::audio::{Filters, Music, demo, songs, synth};
        let mut all: Vec<_> = Music::ALL.into_iter().map(songs::song).collect();
        all.push(demo::demo_song());
        for song in &all {
            for t in Tuning::ALL {
                let r = synth::render_song_tuned(song, Filters::default(), t).unwrap();
                let peak = r.frames.iter().fold(0.0f32, |m, f| {
                    assert!(f.left.is_finite() && f.right.is_finite(), "{} {t:?}", song.title);
                    m.max(f.left.abs()).max(f.right.abs())
                });
                assert!(peak <= 1.0 && peak > 0.05, "{} {t:?}: peak {peak}", song.title);
            }
        }
    }

    #[test]
    fn rendered_pitch_follows_the_tuning() {
        use crate::audio::{Filters, Song, synth};
        // C4 then C5 on pulse 1; measure the second note.
        let s = Song {
            title: "t",
            bpm: 120.0,
            swing: 0.0,
            looping: true,
            pulse1: "o4 c2 o5 c2",
            pulse2: "",
            triangle: "",
            noise: "",
            key: 0,
            chords: "",
        };
        let freq = |x: &[f32]| {
            let ups: Vec<usize> = (1..x.len()).filter(|&i| x[i - 1] < 0.0 && x[i] >= 0.0).collect();
            let (a, b) = (ups[0], *ups.last().unwrap());
            (ups.len() - 1) as f64 * synth::SAMPLE_RATE as f64 / (b - a) as f64
        };
        for (t, want) in [(Tuning::Equal, 523.25), (Tuning::CarlosAlpha, 261.63 * 2f64.powf(0.78)), (Tuning::BohlenPierce, 261.63 * 3.0)] {
            let r = synth::render_song_tuned(&s, Filters::default(), t).unwrap();
            let x: Vec<f32> = r.frames.iter().map(|f| f.left).collect();
            // The voice's mean note is 66, so its anchor is C4 (tie goes down).
            let f = freq(&x[34_000..60_000]);
            assert!((f - want).abs() / want < 0.003, "{t:?}: {f} vs {want}");
        }
    }

    #[test]
    fn anchors() {
        assert_eq!(anchor_tonic(0, 60.0), 60);
        assert_eq!(anchor_tonic(10, 60.0), 58);
        assert_eq!(anchor_tonic(5, 60.0), 65);
        assert_eq!(anchor_tonic(6, 60.0), 54); // tie goes down
        assert_eq!(anchor_tonic(10, 40.0), 34);
    }
}
