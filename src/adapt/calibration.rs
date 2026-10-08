//! The disguised placement test: the first free-play rooms ("Han checks your plumbing").
//!
//! Probe rooms exercise [`CALIBRATION_SKILL`]. The first is at [`START_BAND`]; a clean clear
//! moves the next probe up [`UP_STEP`] bands, a struggle down [`DOWN_STEP`]. It stops as soon as
//! ability is bracketed (a clean and a struggle), or the player bottoms out / tops out, and
//! after [`MAX_PROBES`] rooms at most. Then [`Calibration::placement`] picks the starting bands,
//! spread and assists. Time-to-clear (relative to the room's par) can only *raise* the
//! placement (a fast clean clear) or widen the spread; slow never counts against anyone.

use super::skill::{Band, MAX_BAND, MIN_BAND, Skill, clamp_band};

pub const CALIBRATION_SKILL: Skill = Skill::Precision;
pub const START_BAND: Band = 3;
pub const UP_STEP: i32 = 2;
pub const DOWN_STEP: i32 = 1;
pub const MIN_PROBES: usize = 2;
pub const MAX_PROBES: usize = 3;
/// A clean clear in under this fraction of par counts as fast.
pub const FAST_RATIO: f32 = 0.75;
/// Clean clears slower than this fraction of par count as careful (narrower spread, never lower).
pub const SLOW_RATIO: f32 = 1.4;
/// Starting assists when no probe was cleared.
pub const NO_CLEAN_ASSISTS: f32 = 0.4;
/// Starting assists when some probe was a struggle.
pub const SOME_STRUGGLE_ASSISTS: f32 = 0.15;

/// One probe room's result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Probe {
    pub band: Band,
    pub clean: bool,
    /// time / par, when the room had a par.
    pub time_ratio: Option<f32>,
}

/// Where calibration landed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// Starting center for [`CALIBRATION_SKILL`]; other skills start one lower (min 1),
    /// since they haven't been seen yet.
    pub band: Band,
    pub spread: f32,
    pub assists: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Calibration {
    pub probes: Vec<Probe>,
    /// Set once the placement has been applied to the profile.
    pub done: bool,
}

impl Calibration {
    /// Calibration that's already over (for profiles set up directly, e.g. in tests).
    pub fn finished() -> Self {
        Calibration { probes: Vec::new(), done: true }
    }

    /// Band of the next probe room.
    pub fn next_band(&self) -> Band {
        match self.probes.last() {
            None => START_BAND,
            Some(p) => clamp_band(p.band as i32 + if p.clean { UP_STEP } else { -DOWN_STEP }),
        }
    }

    pub fn record(&self, probe: Probe) -> Self {
        let mut probes = self.probes.clone();
        probes.push(probe);
        Calibration { probes, done: self.done }
    }

    /// Enough probes to place the player.
    pub fn complete(&self) -> bool {
        let n = self.probes.len();
        if n >= MAX_PROBES {
            return true;
        }
        if n < MIN_PROBES {
            return false;
        }
        let bracketed = self.probes.iter().any(|p| p.clean) && self.probes.iter().any(|p| !p.clean);
        let last = self.probes[n - 1];
        let at_floor = !last.clean && last.band <= MIN_BAND;
        let at_ceiling = last.clean && last.band >= MAX_BAND;
        bracketed || at_floor || at_ceiling
    }

    /// Starting dials from the probes so far.
    pub fn placement(&self) -> Placement {
        let cleans: Vec<&Probe> = self.probes.iter().filter(|p| p.clean).collect();
        let lowest_struggle = self.probes.iter().filter(|p| !p.clean).map(|p| p.band).min();
        let mut band = cleans.iter().map(|p| p.band).max().unwrap_or(MIN_BAND);
        let ratios: Vec<f32> = cleans.iter().filter_map(|p| p.time_ratio).collect();
        let avg_ratio = if ratios.is_empty() { None } else { Some(ratios.iter().sum::<f32>() / ratios.len() as f32) };
        let fast = avg_ratio.is_some_and(|r| r < FAST_RATIO);
        let slow = avg_ratio.is_some_and(|r| r > SLOW_RATIO);
        // Flying through: start one higher, but never at or above a band they struggled at.
        if fast && band < MAX_BAND && lowest_struggle.is_none_or(|s| band + 1 < s) {
            band += 1;
        }
        let spread = if fast { 0.6 } else if slow { 0.35 } else { 0.5 };
        let assists = if cleans.is_empty() {
            NO_CLEAN_ASSISTS
        } else if lowest_struggle.is_some() {
            SOME_STRUGGLE_ASSISTS
        } else {
            0.0
        };
        Placement { band, spread, assists }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe(band: Band, clean: bool) -> Probe {
        Probe { band, clean, time_ratio: Some(1.0) }
    }

    #[test]
    fn walks_up_two_down_one() {
        let c = Calibration::default();
        assert_eq!(c.next_band(), 3);
        let c = c.record(probe(3, true));
        assert_eq!(c.next_band(), 5);
        let c = c.record(probe(5, false));
        assert_eq!(c.next_band(), 4);
    }

    #[test]
    fn floor_and_ceiling_clamp() {
        let c = Calibration::default().record(probe(1, false));
        assert_eq!(c.next_band(), 1);
        let c = Calibration::default().record(probe(10, true));
        assert_eq!(c.next_band(), 10);
    }
}
