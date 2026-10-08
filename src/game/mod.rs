//! The simulation: level spawning, player physics, Han, platforms, hazards, nuggets,
//! checkpoints, the goal and the camera.
//!
//! The simulation runs in `FixedUpdate` at 60 Hz (deterministic). Simulated entities carry a
//! [`Pos`] (authoritative position) and [`PrevPos`] (position at the start of the step);
//! `Transform` is synced from `Pos` after every step, and the visuals interpolate between the
//! two so motion is smooth at any refresh rate.

use bevy::prelude::*;

use crate::level::{Level, PlatformKind};
use crate::state::PlayState;

pub mod adaptive;
mod groove;
mod han;
mod hazards;
mod lifecycle;
mod physics;
mod pickups;
mod platforms;
mod visuals;

pub mod assist;
pub use adaptive::{AdaptiveProfile, AssistMode, HiddenRespawn, StoryAssistState};
pub use assist::Assists;
pub use groove::{
    BOUNCE_MIN_SPEED, BOUNCE_RESTITUTION, BOUNCE_SPEED, BeatClock, FIRED_UP_SPEED, GIANT_STEPS_GRAVITY,
    GIANT_STEPS_SPEED, Groove, JumpedOnOne, NERVOUS_TIME, OneJumps, WALTZ_ONE_BOOST, WALTZ_ONE_LINE,
    WALTZ_ONE_LINE_EVERY, WALTZ_ONE_TOOT_SPEED, WALTZ_ONE_WINDOW, WALTZ_SPRAY_BARS,
};
pub use hazards::{
    FLY_PERIOD, FORGIVE, RAFT_LIFE_FLOOR, RAFT_SINK, RAFT_SINK_DEPTH, SPRAY_CYCLE, SPRAY_HEIGHT, SPRAY_ON, SPRAY_WIDTH, RaftLife, raft_sink,
    spray_on,
};
pub use platforms::platform_pos;
pub use lifecycle::{GeneratedLevel, cell_floor, spawn_region, stand_pos};
pub use han::{DEATH_LINES, GRIP_LINE, HAN_DELAY_STEPS, HanAnim, HanMotion, HanPose, HanTrail, NERVOUS_LINE};
pub use pickups::CHECKPOINT_QUIPS;
pub use physics::{Body, Dead, PlayerControl};
pub use visuals::{CharacterSprite, FrameAnim, GameCamera, Particle, VisualSet};

/// Physics and feel tuning. Units: pixels and seconds; a tile is 16px. Level design relies on
/// these (see `tests/levels.rs`): single jump clears ~3 tiles up / ~4 tiles across,
/// double jump ~5 tiles up (with perfect timing) / ~7 across. The music bends them (see
/// [`Groove`]): Giant Steps makes a 6-tile "giant wall" climbable, a fired-up band an
/// 11-tile "long gap" jumpable.
pub mod tuning {
    pub const GRAVITY: f32 = 1400.0;
    pub const MAX_FALL: f32 = 420.0;
    pub const RUN_SPEED: f32 = 150.0;
    pub const GROUND_ACCEL: f32 = 1400.0;
    pub const GROUND_DECEL: f32 = 1800.0;
    pub const AIR_ACCEL: f32 = 1000.0;
    /// Initial upward speed of the ground jump (apex ~51px ≈ 3.2 tiles).
    pub const JUMP_SPEED: f32 = 380.0;
    /// Upward speed set by the mid-air toot jump (adds ~39px ≈ 2.4 tiles). A perfectly timed
    /// double jump (toot at the apex) reaches ~5.3 tiles; no double jump clears a 6-tile wall
    /// unless the band plays Giant Steps (see [`Groove`]).
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

/// Simulation rate.
pub const FIXED_HZ: f64 = 60.0;

pub fn plugin(app: &mut App) {
    assist::plugin(app);
    app.init_resource::<crate::level::Levels>()
        .init_resource::<LevelRun>()
        .init_resource::<SimClock>()
        .add_message::<RestartLevel>()
        .insert_resource(Time::<Fixed>::from_hz(FIXED_HZ))
        .configure_sets(
            FixedUpdate,
            (
                GameSet::Prepare,
                GameSet::World,
                GameSet::Player,
                GameSet::Interact,
                GameSet::Follow,
                GameSet::Sync,
            )
                .chain()
                .run_if(in_state(PlayState::Running).and_then(resource_exists::<ActiveLevel>)),
        )
        .add_systems(FixedUpdate, sync_transforms.in_set(GameSet::Sync))
        .add_plugins((
            groove::plugin,
            lifecycle::plugin,
            physics::plugin,
            platforms::plugin,
            hazards::plugin,
            pickups::plugin,
            han::plugin,
            adaptive::plugin,
        ));
}

/// Order of the fixed-step simulation. Everything only runs while [`PlayState::Running`].
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameSet {
    /// Tick clocks, remember previous positions.
    Prepare,
    /// Moving platforms, flies, spray jets.
    World,
    /// Player input + physics.
    Player,
    /// Hazards, nuggets, checkpoints, goal, death/respawn.
    Interact,
    /// Han.
    Follow,
    /// Copy [`Pos`] into `Transform`.
    Sync,
}

/// Simulated position (world pixels, y up) of a moving entity: the player's/Han's box center,
/// a platform's center, a fly's center. Authoritative; `Transform` follows it.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Reflect)]
#[reflect(Component)]
pub struct Pos(pub Vec2);

/// [`Pos`] at the start of the current fixed step (for interpolation and platform carrying).
/// Equal to `Pos` right after a teleport.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Reflect)]
#[reflect(Component)]
pub struct PrevPos(pub Vec2);

/// Seconds of simulated (running, unpaused) time since the level (re)started. Drives platforms,
/// flies and sprays deterministically.
#[derive(Resource, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Resource)]
pub struct SimClock {
    pub time: f32,
    pub steps: u64,
    /// The moving platforms' clock: `time`, except that while the band waltzes it runs in
    /// lilting pulses (see [`Groove::platform_rate`]).
    pub platform_time: f32,
    /// Orbits the flies have flown (one per [`FLY_PERIOD`], or one per bar in the waltz).
    pub fly_turns: f32,
}

/// The level currently loaded (a copy of `Levels[CurrentLevel]` taken when it was spawned).
/// Physics queries this grid directly. Exists while level entities exist.
#[derive(Resource, Debug, Clone)]
pub struct ActiveLevel {
    pub index: usize,
    pub level: Level,
}

/// A static tile of the level grid (for drawing; physics uses [`ActiveLevel`]).
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct LevelTile {
    pub col: usize,
    pub row: usize,
    pub tile: crate::level::Tile,
    /// The tile above is a different kind (draw the surface variant: grass top / liquid top).
    pub top: bool,
    pub world: u8,
}

/// A moving platform (one-way, carries riders). `Pos` is the center of its top tile row.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct MovingPlatform {
    /// `Pos` at phase 0 (the grid position).
    pub base: Vec2,
    /// Travel in pixels (y up).
    pub travel: Vec2,
    pub period: f32,
    pub phase: f32,
    /// Width in tiles.
    pub width: usize,
    pub kind: PlatformKind,
}

/// A golden nugget waiting to be picked up.
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct Nugget;

/// A nugget that came back after a death: picking it up again still fires up the band
/// ([`crate::events::NuggetCollected`]) but doesn't count toward the level total twice.
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct GhostNugget;

/// Toilet-paper-holder checkpoint number `index` (level order). `active` once touched.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Checkpoint {
    pub index: usize,
    pub active: bool,
}

/// The goal flag (a plunger with a flag, two tiles tall; the entity sits at the bottom-center
/// of its `G` cell, so the art is anchored bottom-center).
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct Goal;

/// A fly circling `center`. `Pos` is its current position.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Fly {
    pub center: Vec2,
    /// Phase offset in turns (0..1).
    pub phase: f32,
}

/// Air-freshener can at the bottom-center of its cell; its jet fires 3 tiles up while `on`.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Spray {
    pub col: usize,
    pub on: bool,
}

/// A splat stain on what were spikes at (col, row) (the grid in [`ActiveLevel`] already says
/// [`Tile::StainUp`](crate::level::Tile)/`StainDown`; this entity is for drawing it).
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Stain {
    pub col: usize,
    pub row: usize,
    pub tile: crate::level::Tile,
}

/// A stain raft (with a one-tile [`MovingPlatform`] that stays put): floats [`RaftLife::secs`] (at least [`RAFT_LIFE_FLOOR`])
/// seconds after the splat in liquid that made it, then sinks.
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct Raft {
    pub age: f32,
}

/// A `hint@` spot: Han says `text` the first time Nat comes within
/// [`HINT_RADIUS`](crate::level::HINT_RADIUS) of `center` (once per level visit).
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct HintSpot {
    pub center: Vec2,
    pub text: String,
    pub said: bool,
}

fn sync_transforms(mut q: Query<(&Pos, &mut Transform)>) {
    for (pos, mut tf) in &mut q {
        tf.translation.x = pos.0.x;
        tf.translation.y = pos.0.y;
    }
}

/// The hero. Exactly one while a level is loaded.
#[derive(Component, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct Player;

/// Han the plumber, who follows the player around. Exactly one while a level is loaded.
#[derive(Component, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct Han;

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
pub fn visuals_plugin(app: &mut App) {
    visuals::plugin(app);
}
