//! Music hook: how hard the player is pushing, as a mood the band can follow.

use super::profile::{PlayerProfile, RecentRoom};
use super::skill::Skill;
use super::window::Outcome;

/// How the band should feel. Struggling → calmer and sparser; flying → hotter and looser.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BandMood {
    /// 0 = calm, gentle; 1 = hot, driving.
    pub intensity: f32,
    /// 0 = sparse, tight to the tune; 1 = busy, improvising.
    pub freedom: f32,
}

impl BandMood {
    pub const NEUTRAL: BandMood = BandMood { intensity: 0.5, freedom: 0.5 };
}

/// Recency weight per room back (the latest room counts most).
pub const MOOD_DECAY: f32 = 0.7;

/// The mood from the profile's recent rooms ([`PlayerProfile::recent`]), its assists and its
/// spreads. Neutral with no history.
pub fn band_mood(profile: &PlayerProfile) -> BandMood {
    band_mood_from(profile, &profile.recent)
}

/// [`band_mood`] over an explicit list of recent rooms (oldest first).
pub fn band_mood_from(profile: &PlayerProfile, recent: &[RecentRoom]) -> BandMood {
    if recent.is_empty() {
        return BandMood::NEUTRAL;
    }
    let (mut clean, mut pace, mut toots, mut wsum, mut psum) = (0.0, 0.0, 0.0, 0.0, 0.0);
    let mut w = 1.0;
    for r in recent.iter().rev() {
        let c = match r.outcome {
            Outcome::Clean => 1.0,
            Outcome::Careless => 0.7,
            Outcome::Struggle => (0.3 - 0.1 * r.excess_deaths as f32).max(0.0),
        };
        clean += w * c;
        toots += w * (r.toots_per_min / 20.0).min(1.0);
        wsum += w;
        if let Some(t) = r.time_ratio {
            // 0.5×par → 1, 1.5×par → 0.
            pace += w * (1.5 - t).clamp(0.0, 1.0);
            psum += w;
        }
        w *= MOOD_DECAY;
    }
    let clean = clean / wsum;
    let toots = toots / wsum;
    let pace = if psum > 0.0 { pace / psum } else { 0.5 };
    let spread = Skill::ALL.iter().map(|s| profile.skill(*s).spread).sum::<f32>() / Skill::COUNT as f32;
    let calm = profile.assists + if profile.frustrated() { 0.2 } else { 0.0 };
    BandMood {
        intensity: (0.1 + 0.5 * clean + 0.3 * pace + 0.1 * toots - 0.3 * calm).clamp(0.0, 1.0),
        freedom: (0.05 + 0.45 * clean + 0.3 * spread + 0.2 * toots - 0.3 * calm).clamp(0.0, 1.0),
    }
}
