//! Persistent progress: which levels are unlocked and the best nugget haul per level.
//! Stored in `localStorage` on the web and a small file on native.
//! (The UI module owns load/store and updating this on level completion.)

use bevy::prelude::*;

use crate::level::LEVEL_COUNT;

pub fn plugin(app: &mut App) {
    app.init_resource::<Progress>();
}

#[derive(Resource, Debug, Clone, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct Progress {
    /// Levels `0..unlocked` are playable. Always >= 1.
    pub unlocked: usize,
    /// Best nugget count per level, `None` if never completed.
    pub best_nuggets: [Option<u32>; LEVEL_COUNT],
    /// Best completion time per level, seconds.
    pub best_time: [Option<f32>; LEVEL_COUNT],
}

impl Default for Progress {
    fn default() -> Self {
        Self { unlocked: 1, best_nuggets: [None; LEVEL_COUNT], best_time: [None; LEVEL_COUNT] }
    }
}
