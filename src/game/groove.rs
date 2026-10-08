//! The groove: the music that's playing bends the physics.
//!
//! The band director (`crate::audio::director`) reharmonizes the soundtrack from how the player
//! plays; the audio plugin writes [`Groove`] at the exact moment a new version starts sounding
//! (when a track starts, and at the bar line where a reharmonization switches in), so what you
//! hear and how Nat moves are always in sync. Level start and restart reset it to normal.
//! Headless tests set it directly.
//!
//! | music | trigger | physics | gates |
//! |---|---|---|---|
//! | original | – | normal | – |
//! | Coltrane ("GIANT STEPS!") | 5 toots in 20s | gravity ×[`GIANT_STEPS_GRAVITY`], run ×[`GIANT_STEPS_SPEED`] | giant walls (6 tiles tall) |
//! | quartal ("FIRED UP") | 4 quick nuggets | run ×[`FIRED_UP_SPEED`] | long gaps (11 tiles) |
//! | melodic minor ("NERVOUS") | 3 deaths | game time ×[`NERVOUS_TIME`] (slow motion) | none: an assist |
//! | + laughing band (tuning medley) | 2 deaths at one checkpoint | landings bounce | none: comedy |
//!
//! Giant Steps slows the run so its longer air time doesn't also clear long gaps: each gate
//! opens in exactly one mode (`tests/levels.rs` checks it).
//!
//! Adding a mode (e.g. a waltz with a beat to jump on): give [`Groove`] the new knob, set it in
//! [`Groove::new`] from the music, and read it in the physics; nothing else needs to know.
//! [`Groove::tuning`] carries the laughing band's tuning so physics can later follow the medley
//! phrase by phrase (the audio side would update it at each phrase line).

use bevy::prelude::*;

use crate::audio::{Filters, Harmony, tuning::Tuning};

/// Gravity multiplier under Giant Steps (jumps ~1.5x higher: a single jump ~4.8 tiles, a
/// quick 0.1s/0.1s double-tap ~6.2 tiles, a perfect double jump ~8.3 tiles).
pub const GIANT_STEPS_GRAVITY: f32 = 0.65;
/// Run speed multiplier under Giant Steps: keeps the horizontal reach of a jump about normal.
pub const GIANT_STEPS_SPEED: f32 = 0.65;
/// Run speed multiplier while the band is fired up (quartal).
pub const FIRED_UP_SPEED: f32 = 1.35;
/// Game speed while the band is nervous (melodic minor).
pub const NERVOUS_TIME: f32 = 0.8;
/// Upward speed of the laughing-band landing bounce, at most (~1 tile).
pub const BOUNCE_SPEED: f32 = 210.0;
/// Bounce speed per landing speed (each bounce is lower until it dies out).
pub const BOUNCE_RESTITUTION: f32 = 0.6;
/// Landings slower than this don't bounce.
pub const BOUNCE_MIN_SPEED: f32 = 150.0;

/// How the music currently playing bends the physics.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct Groove {
    /// The harmony the band is playing (for the UI and effects).
    pub harmony: Harmony,
    /// Gravity multiplier; max fall speed scales by its square root (floaty, not just slow).
    pub gravity_scale: f32,
    /// Run speed multiplier (ground and air acceleration scale with it).
    pub speed_scale: f32,
    /// Game time multiplier (slow motion below 1). The music keeps real time.
    pub time_scale: f32,
    /// Landings spring Nat back up a little.
    pub bounce: bool,
    /// The tuning the band plays in ([`Tuning::Medley`] for the laughing band). Physics don't
    /// read it yet.
    #[reflect(ignore)]
    pub tuning: Tuning,
}

impl Default for Groove {
    fn default() -> Self {
        Self::new(Filters::default())
    }
}

impl Groove {
    /// The physics that go with the music `filters`.
    pub fn new(filters: Filters) -> Self {
        let plain = Groove {
            harmony: filters.harmony,
            gravity_scale: 1.0,
            speed_scale: 1.0,
            time_scale: 1.0,
            bounce: filters.just_intonation,
            tuning: if filters.just_intonation { Tuning::Medley } else { Tuning::Equal },
        };
        match filters.harmony {
            Harmony::Original => plain,
            Harmony::Coltrane => {
                Groove { gravity_scale: GIANT_STEPS_GRAVITY, speed_scale: GIANT_STEPS_SPEED, ..plain }
            }
            Harmony::Quartal => Groove { speed_scale: FIRED_UP_SPEED, ..plain },
            Harmony::MelodicMinor => Groove { time_scale: NERVOUS_TIME, ..plain },
        }
    }

    /// The physics of a harmony, without the laughing band.
    pub fn of(harmony: Harmony) -> Self {
        Self::new(Filters { harmony, just_intonation: false })
    }

    pub fn giant_steps(&self) -> bool {
        self.harmony == Harmony::Coltrane
    }

    /// Max fall speed multiplier.
    pub fn fall_scale(&self) -> f32 {
        self.gravity_scale.sqrt()
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Groove>().add_systems(Update, apply_time_scale);
}

/// Slow motion: the game's virtual clock follows [`Groove::time_scale`].
fn apply_time_scale(groove: Res<Groove>, mut time: ResMut<Time<Virtual>>) {
    if time.relative_speed() != groove.time_scale {
        time.set_relative_speed(groove.time_scale);
    }
}
