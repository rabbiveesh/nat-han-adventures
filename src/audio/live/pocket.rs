//! Where each player sits against the beat: nobody in a real band plays on the grid. Measured
//! on 34 hand-annotated jazz trio recordings (the Jazz Trio Database's manual annotations,
//! 1956-2015): the drums are the time; the bass sits about 4 ms behind them on average but on
//! either side from tune to tune (Mingus's "rotary perception": the beat is a circle to play
//! anywhere in); the piano about 9 ms behind; and every note lands within 10-20 ms of its spot.
//! Soloists lay back on the beat and meet the drums on the offbeats (Friberg & Sundström 2002),
//! the more so the slower the tune: across the 456 solos of the Weimar Jazz Database the
//! soloist's on-beat notes sit about 4-5 ms behind below 200 bpm, 2 ms at 200-260 and on the
//! beat above, and their scatter narrows with tempo too (Miles: 10 ms behind on ballads, a
//! hair ahead at 260+).
//!
//! Each note's offset has three parts:
//! - the player's *spot*, wandering slowly: a mean, and a drift to either side over tens of
//!   seconds (the bass the most, the drums hardly at all) with a small faster sway;
//! - the *attack*: the lead's lay-back on the beat (its offbeats meet the drums), by tempo;
//! - *jitter*: each attack its own few ms, seeded, wider at slow tempos.
//!
//! A note's end moves only with the spot, and so does a tied note's start, so ties and slurs
//! stay joined. In a soli the section sits with the lead (it breathes together). Straight feels
//! (bossa, samba, rock, funk) sit tighter than swing. Pure, no allocation.

use super::feel::Feel;
use crate::audio::tuning;

/// Per channel, in ms (positive: behind the beat): the spot's mean and how far it drifts to
/// either side (about one standard deviation), and the attacks' jitter (likewise). Pulse 1
/// (lead), pulse 2 (comp), triangle (bass), noise (drums), pulse 3 (the soli's tenor).
const SEATS: [Seat; 5] = [
    Seat { mean: 0.0, drift: 4.0, jitter: 6.0 },
    Seat { mean: 9.0, drift: 5.0, jitter: 7.0 },
    Seat { mean: 4.0, drift: 7.0, jitter: 6.0 },
    Seat { mean: 0.0, drift: 1.5, jitter: 3.0 },
    Seat { mean: 0.0, drift: 4.0, jitter: 6.0 },
];

/// The lead's lay-back on the beat, ms (its offbeats meet the drums): this much at slow tempos,
/// easing to none at [`LAY_BACK_GONE`] bpm.
const LAY_BACK: f64 = 6.0;
const LAY_BACK_FULL: f64 = 140.0;
const LAY_BACK_GONE: f64 = 270.0;

/// The jitter's width at `bpm` against the seat's (at 160 bpm): wider on ballads, tighter
/// when it burns (the soloists' scatter in the recordings: 41, 23, 17, 14 ms by tempo).
fn jitter_scale(bpm: f64) -> f64 {
    (160.0 / bpm.max(1.0)).sqrt().clamp(0.75, 1.4)
}

/// No attack lands further than this from its spot, ms.
const MAX_JITTER: f64 = 14.0;

#[derive(Debug, Clone, Copy)]
struct Seat {
    mean: f64,
    drift: f64,
    jitter: f64,
}

/// One note's place, for [`start`] / [`end`].
#[derive(Debug, Clone, Copy)]
pub struct Player {
    pub ch: u8,
    pub sr: u32,
    pub seed: u64,
    /// The tune's tempo (the lead lays back less, and everyone scatters less, as it rises).
    pub bpm: f32,
    /// The lead's lay-back against the average soloist's (1: as measured across them all; the
    /// soloist's own, [`super::soloist::Soloist::lay_back`]).
    pub lay: f32,
    /// 0..1: scales it all (0: on the grid).
    pub loose: f32,
    pub feel: Feel,
    /// In a soli: the section sits with the lead.
    pub soli: bool,
}

impl Player {
    /// The seat this channel sits in (in a soli, the section's voices take the lead's).
    fn seat(&self) -> (usize, Seat) {
        let ch = self.ch.min(4) as usize;
        let k = if self.soli && matches!(ch, 1 | 4) { 0 } else { ch };
        (k, SEATS[k])
    }

    fn scale(&self) -> f64 {
        self.loose as f64 * if self.feel == Feel::Swing { 1.0 } else { 0.5 }
    }

    /// The spot at absolute sample `t`, ms.
    fn spot(&self, t: u64) -> f64 {
        let (k, seat) = self.seat();
        let secs = t as f64 / self.sr as f64;
        let wave = |i: usize, period: f64| {
            let phase = unit(tuning::salt(self.seed ^ 0x9_0C4E7, k, i, 0));
            (std::f64::consts::TAU * (secs / period + phase)).sin()
        };
        // Over tens of seconds to either side (two slow waves, ~1 sd together), and a sway.
        let drift = 1.15 * (0.75 * wave(0, 31.0) + 0.55 * wave(1, 13.0)) + 0.25 * wave(2, 2.3);
        seat.mean + seat.drift * drift
    }

    /// The sample a note written at `t` starts on: `beat` its beat (song beats, for the lead's
    /// lay-back), `tied` if it carries on from the note before (no attack of its own).
    pub fn start(&self, t: u64, beat: f64, tied: bool) -> u64 {
        if self.loose <= 0.0 {
            return t;
        }
        let mut ms = self.spot(t);
        if !tied {
            let (k, seat) = self.seat();
            let on_beat = (beat - beat.round()).abs() < 0.05;
            let bpm = self.bpm as f64;
            if k == 0 && on_beat {
                ms += LAY_BACK * self.lay as f64 * ((LAY_BACK_GONE - bpm) / (LAY_BACK_GONE - LAY_BACK_FULL)).clamp(0.0, 1.0);
            }
            // Triangular, about `jitter` ms to a standard deviation.
            let h = tuning::salt(self.seed ^ 0x717, k, (t / 8) as usize, 1);
            let (a, b) = (unit(h), unit(h.rotate_left(32).wrapping_mul(0x9E37_79B9_7F4A_7C15)));
            let width = seat.jitter * jitter_scale(bpm) * 6f64.sqrt() / 2.0;
            ms += ((a + b - 1.0) * width).clamp(-MAX_JITTER, MAX_JITTER);
        }
        self.shift(t, ms * self.scale())
    }

    /// The sample a note written to end at `t` ends on (the spot alone).
    pub fn end(&self, t: u64) -> u64 {
        if self.loose <= 0.0 {
            return t;
        }
        self.shift(t, self.spot(t) * self.scale())
    }

    /// `t` moved by `ms` (never before 0).
    fn shift(&self, t: u64, ms: f64) -> u64 {
        t.saturating_add_signed((ms / 1000.0 * self.sr as f64).round() as i64)
    }
}

fn unit(h: u64) -> f64 {
    (h >> 11) as f64 / (1u64 << 53) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 32_000;

    fn player(ch: u8) -> Player {
        Player { ch, sr: SR, seed: 3, bpm: 160.0, lay: 1.0, loose: 1.0, feel: Feel::Swing, soli: false }
    }

    /// Mean and standard deviation of a channel's attacks against the grid, ms (`beat`: the
    /// notes' beat phase).
    fn stats(ch: u8, beat: f64) -> (f64, f64) {
        stats_at(ch, beat, 160.0)
    }

    fn stats_at(ch: u8, beat: f64, bpm: f32) -> (f64, f64) {
        let p = Player { bpm, ..player(ch) };
        let d: Vec<f64> = (0..4000u64).map(|k| k * 7919).map(|t| (p.start(t, beat, false) as f64 - t as f64) / 32.0).collect();
        let m = d.iter().sum::<f64>() / d.len() as f64;
        (m, (d.iter().map(|x| (x - m).powi(2)).sum::<f64>() / d.len() as f64).sqrt())
    }

    #[test]
    fn players_sit_where_the_recordings_put_them() {
        // The drums on the beat, the bass a little behind them, the comp further; the lead
        // lays back on the beat and meets the drums on the offbeats.
        let (drums, bass, comp) = (stats(3, 0.0), stats(2, 0.0), stats(1, 0.0));
        let (on, off) = (stats(0, 1.0), stats(0, 1.5));
        assert!(drums.0.abs() < 1.5 && drums.1 < 5.0, "drums {drums:?}");
        assert!(bass.0 > 1.0 && bass.0 < 8.0 && bass.1 > 6.0, "bass {bass:?}");
        assert!(comp.0 > 6.0 && comp.0 < 12.0, "comp {comp:?}");
        assert!(on.0 > off.0 + 4.0, "lead on {on:?} off {off:?}");
        for ch in 0..5 {
            assert_eq!(Player { loose: 0.0, ..player(ch) }.start(12345, 0.0, false), 12345, "tight is tight");
        }
    }

    #[test]
    fn the_lead_lays_back_less_as_it_burns() {
        let (slow, mid, fast) = (stats_at(0, 1.0, 120.0), stats_at(0, 1.0, 230.0), stats_at(0, 1.0, 290.0));
        assert!(slow.0 > mid.0 + 2.0 && mid.0 > fast.0 + 1.0, "{slow:?} {mid:?} {fast:?}");
        assert!(fast.0.abs() < 1.5, "on the beat when it burns: {fast:?}");
        assert!(slow.1 > fast.1 * 1.15, "wider on ballads: {slow:?} {fast:?}");
    }

    #[test]
    fn the_bass_sits_on_either_side_of_the_beat() {
        let p = player(2);
        let spots: Vec<f64> = (0..600u64).map(|s| (p.end(s * SR as u64) as f64 - (s * SR as u64) as f64) / 32.0).collect();
        assert!(spots.iter().any(|&x| x < -2.0) && spots.iter().any(|&x| x > 8.0), "{:?}", &spots[..40]);
    }

    #[test]
    fn ties_stay_joined_and_the_spot_moves_slowly() {
        for ch in 0..5 {
            let p = player(ch);
            for k in 1..2000u64 {
                let t = k * 2500;
                // A tied note starts where the note before it ends.
                assert_eq!(p.start(t, 0.5, true), p.end(t));
                // The spot moves under a ms between notes a sixteenth apart (~78 ms at 192).
                let (a, b) = (p.end(t) as i64 - t as i64, p.end(t + 2500) as i64 - (t + 2500) as i64);
                assert!((a - b).abs() <= 32, "{ch}: {a} {b}");
                assert!((p.start(t, 0.0, false) as i64 - t as i64).abs() <= 32 * 40, "within 40 ms");
            }
        }
    }
}
