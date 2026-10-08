//! Choosing what comes next: which skill, at which band, with how much help.

use rand::Rng;

use super::assist::AssistLevers;
use super::calibration::CALIBRATION_SKILL;
use super::profile::PlayerProfile;
use super::skill::{Band, MAX_BAND, MIN_BAND, Skill};

/// What the free-play room generator should build next.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoomRequest {
    /// The skill the room is about.
    pub skill: Skill,
    /// How hard that skill should be, 1..=10.
    pub band: Band,
    /// Silent help to build in / apply while the room is played.
    pub assists: AssistLevers,
}

/// Share of picks that go to strengths (the rest to growth areas)...
pub const STRENGTH_SHARE: f32 = 0.6;
/// ...and while frustrated.
pub const FRUSTRATED_STRENGTH_SHARE: f32 = 0.8;
/// Clean rate assumed for a skill with no history.
pub const UNKNOWN_RATE: f32 = 0.5;

/// Probability of each band 1..=10 (index 0 = band 1) for a skill centered at `center` with
/// `spread` (0..1). The center gets `0.9 − 0.6·spread`; the rest goes to ±1 (`0.05 + 0.15·s`),
/// ±2 (`0.1·s − 0.005`) and ±3 (`0.1·(s − 0.5)`), split evenly between the sides. A side that
/// falls off the 1..=10 range folds back one step toward the center. Then normalized to sum
/// to 1 (so the center really gets ~95% at spread 0, ~47% at spread 1). Same as robot-game's
/// `band_distribution`.
pub fn band_distribution(center: Band, spread: f32) -> [f32; 10] {
    let s = spread.clamp(0.0, 1.0);
    let center = center.clamp(MIN_BAND, MAX_BAND) as i32;
    let offsets = [
        (0, 0.9 - 0.6 * s),
        (1, 0.05 + 0.15 * s),
        (2, (0.1 * s - 0.005).max(0.0)),
        (3, (0.1 * (s - 0.5)).max(0.0)),
    ];
    let mut raw = [0.0f32; 10];
    let fold = |b: i32, toward: i32| -> usize {
        let b = if (MIN_BAND as i32..=MAX_BAND as i32).contains(&b) { b } else { b - toward };
        (b.clamp(MIN_BAND as i32, MAX_BAND as i32) - 1) as usize
    };
    for (d, w) in offsets {
        if d == 0 {
            raw[(center - 1) as usize] += w;
        } else {
            raw[fold(center + d, 1)] += w / 2.0;
            raw[fold(center - d, -1)] += w / 2.0;
        }
    }
    let total: f32 = raw.iter().sum();
    for v in &mut raw {
        *v /= total;
    }
    raw
}

/// Draw a band from a [`band_distribution`].
pub fn sample_band(dist: &[f32; 10], rng: &mut impl Rng) -> Band {
    let r: f32 = rng.random();
    let mut acc = 0.0;
    for (i, p) in dist.iter().enumerate() {
        acc += p;
        if r < acc {
            return (i + 1) as Band;
        }
    }
    MAX_BAND
}

/// Pick the skill for the next room among `unlocked`: rank by clean rate (no history =
/// [`UNKNOWN_RATE`]), the top half are strengths, the rest growth areas; pick a strength with
/// [`STRENGTH_SHARE`] (or [`FRUSTRATED_STRENGTH_SHARE`] while frustrated), then uniformly within
/// the group. Nothing unlocked → [`Skill::Precision`].
pub fn pick_skill(profile: &PlayerProfile, unlocked: &[Skill], rng: &mut impl Rng) -> Skill {
    let mut ranked: Vec<(Skill, f32)> = Vec::new();
    for &s in unlocked {
        if !ranked.iter().any(|(r, _)| *r == s) {
            ranked.push((s, profile.skill(s).window.clean_rate().0.unwrap_or(UNKNOWN_RATE)));
        }
    }
    match ranked.len() {
        0 => return Skill::Precision,
        1 => return ranked[0].0,
        _ => {}
    }
    // Stable: equal rates keep `unlocked` order.
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    let (strengths, growth) = ranked.split_at(ranked.len().div_ceil(2));
    let share = if profile.frustrated() { FRUSTRATED_STRENGTH_SHARE } else { STRENGTH_SHARE };
    let group = if rng.random::<f32>() < share { strengths } else { growth };
    group[rng.random_range(0..group.len())].0
}

/// The next room for free play. During calibration that's the next placement probe.
pub fn next_room(profile: &PlayerProfile, unlocked: &[Skill], rng: &mut impl Rng) -> RoomRequest {
    let assists = profile.levers();
    if profile.calibrating() {
        let skill = if unlocked.is_empty() || unlocked.contains(&CALIBRATION_SKILL) {
            CALIBRATION_SKILL
        } else {
            unlocked[0]
        };
        return RoomRequest { skill, band: profile.calibration.next_band(), assists };
    }
    let skill = pick_skill(profile, unlocked, rng);
    let st = profile.skill(skill);
    let band = sample_band(&band_distribution(st.center, st.spread), rng);
    RoomRequest { skill, band, assists }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_robot_game_weights_mid_range() {
        // Raw weights 0.3 / 0.2 / 0.095 / 0.05 (sum 0.645), normalized.
        let d = band_distribution(5, 1.0);
        let t = 0.645;
        assert!((d[4] - 0.3 / t).abs() < 1e-5);
        assert!((d[3] - 0.1 / t).abs() < 1e-5 && (d[5] - 0.1 / t).abs() < 1e-5);
        assert!((d[2] - 0.0475 / t).abs() < 1e-5);
        assert!((d[1] - 0.025 / t).abs() < 1e-5);
    }
}
