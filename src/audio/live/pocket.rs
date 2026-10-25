//! Where each player sits against the beat: nobody in a real band plays on the grid. The
//! bass sits a touch on top of the beat, the drums are the time, the comp a hair behind, the
//! lead lays back, and each wanders a few ms around their spot, slowly (a feel, not a mistake).
//!
//! The offset is a smooth function of time for each channel, so a note's end and the next
//! note's start move together (ties and slurs stay joined) and the order of a channel's notes
//! never changes. In a soli the voices sit with the lead (the section breathes together).
//! Straight feels (bossa, samba, rock, funk) sit tighter than swing. Pure, no allocation.

use super::feel::Feel;
use crate::audio::tuning;

/// (Spot, wander) per channel in seconds: pulse 1 (lead), pulse 2 (comp), triangle (bass),
/// noise (drums), pulse 3 (the soli's tenor). Positive is behind the beat.
const SEATS: [(f64, f64); 5] = [(0.012, 0.006), (0.004, 0.004), (-0.005, 0.003), (0.0, 0.002), (0.008, 0.005)];

/// How slowly each player wanders: two waves, in seconds per cycle.
const WAVES: [f64; 2] = [2.3, 0.9];

/// The offset, in samples, of a note of channel `ch` written at absolute sample `t` (`sr`
/// samples a second; `loose` 0..1 scales it all; `soli`: the section sits with the lead).
pub fn offset(ch: u8, t: u64, sr: u32, seed: u64, loose: f32, feel: Feel, soli: bool) -> i64 {
    if loose <= 0.0 {
        return 0;
    }
    let ch = ch.min(4) as usize;
    // The section's voices share the lead's spot and their own small wander.
    let seat = if soli && matches!(ch, 1 | 4) { SEATS[0].0 } else { SEATS[ch].0 };
    let wander = SEATS[ch].1;
    let secs = t as f64 / sr as f64;
    let wave = |k: usize| {
        let phase = (tuning::salt(seed ^ 0x9_0C4E7, ch, k, 0) >> 11) as f64 / (1u64 << 53) as f64;
        (std::f64::consts::TAU * (secs / WAVES[k] + phase)).sin()
    };
    let swing = if feel == Feel::Swing { 1.0 } else { 0.5 };
    let s = (seat + wander * (0.6 * wave(0) + 0.4 * wave(1))) * swing * loose as f64;
    (s * sr as f64).round() as i64
}

/// `t` moved by `offset` (never before 0).
#[inline]
pub fn shift(t: u64, offset: i64) -> u64 {
    (t as i64 + offset).max(0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn players_sit_where_they_should() {
        let mean = |ch: u8| (0..1000).map(|k| offset(ch, k * 997, 32_000, 3, 1.0, Feel::Swing, false)).sum::<i64>() as f64 / 1000.0 / 32.0;
        // In ms: the lead laid back, the bass on top, the drums on it.
        assert!(mean(0) > 8.0 && mean(0) < 16.0, "{}", mean(0));
        assert!(mean(2) < -2.0, "{}", mean(2));
        assert!(mean(3).abs() < 1.0, "{}", mean(3));
        for ch in 0..5 {
            for k in 0..1000 {
                assert!(offset(ch, k * 331, 32_000, 3, 1.0, Feel::Swing, false).abs() <= 32 * 20, "within 20 ms");
            }
            assert_eq!(offset(ch, 12345, 32_000, 3, 0.0, Feel::Swing, false), 0, "tight is tight");
        }
    }

    #[test]
    fn the_wander_is_slow() {
        // Notes a sixteenth apart (at 192 bpm, ~78 ms) move by a few ms at most against each other.
        let sr = 32_000;
        let step = (0.078 * sr as f64) as u64;
        for k in 0..2000 {
            let (a, b) = (offset(0, k * step, sr, 9, 1.0, Feel::Swing, false), offset(0, (k + 1) * step, sr, 9, 1.0, Feel::Swing, false));
            assert!((a - b).abs() <= 3 * 32, "{a} {b}");
        }
    }
}
