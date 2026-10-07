//! The simulation: level spawning, player physics, Gus, platforms, hazards, nuggets,
//! checkpoints, the goal and the camera.

use bevy::prelude::*;

/// Physics and feel tuning. Units: pixels and seconds; a tile is 16px. Level design relies on
/// these (see `tests/levels.rs`): single jump clears ~3 tiles up / ~4 tiles across,
/// double jump ~5 tiles up / ~7 across.
pub mod tuning {
    pub const GRAVITY: f32 = 1400.0;
    pub const MAX_FALL: f32 = 420.0;
    pub const RUN_SPEED: f32 = 150.0;
    pub const GROUND_ACCEL: f32 = 1400.0;
    pub const GROUND_DECEL: f32 = 1800.0;
    pub const AIR_ACCEL: f32 = 1000.0;
    /// Initial upward speed of the ground jump (apex ~51px ≈ 3.2 tiles).
    pub const JUMP_SPEED: f32 = 380.0;
    /// Upward speed set by the mid-air toot jump (adds ~39px ≈ 2.4 tiles).
    pub const DOUBLE_JUMP_SPEED: f32 = 330.0;
    /// Releasing jump while rising multiplies vertical speed by this (variable jump height).
    pub const JUMP_CUT: f32 = 0.5;
    /// Can still ground-jump this long after walking off a ledge.
    pub const COYOTE_TIME: f32 = 0.1;
    /// A jump pressed this long before landing still fires on landing.
    pub const JUMP_BUFFER: f32 = 0.12;
    /// Player collision box (centered on the transform).
    pub const PLAYER_SIZE: (f32, f32) = (12.0, 14.0);
    /// Delay between splat and respawn.
    pub const RESPAWN_DELAY: f32 = 0.8;
}

pub fn plugin(app: &mut App) {
    app.init_resource::<crate::level::Levels>()
        .init_resource::<LevelRun>()
        .add_message::<RestartLevel>();
}

/// The hero. Exactly one while a level is loaded.
#[derive(Component, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct Player;

/// Gus the plumber, who follows the player around. Exactly one while a level is loaded.
#[derive(Component, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct Gus;

/// Everything spawned for the current level; despawned when the level unloads.
#[derive(Component, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct LevelEntity;

/// Stats for the level in progress (read by the HUD).
#[derive(Resource, Debug, Clone, Default, Reflect)]
#[reflect(Resource)]
pub struct LevelRun {
    pub nuggets: u32,
    pub nuggets_total: u32,
    /// Seconds of play (not counting pause).
    pub time: f32,
    /// Index of the last checkpoint reached, if any.
    pub checkpoint: Option<usize>,
    pub deaths: u32,
}

/// Ask the game to restart the current level from scratch (R key, pause menu).
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct RestartLevel;

/// Presentation half of the game: sprites, animation, camera, particles.
pub fn visuals_plugin(_app: &mut App) {}
