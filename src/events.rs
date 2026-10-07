//! Gameplay messages. The simulation writes them; UI, audio and FX read them.

use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_message::<NuggetCollected>()
        .add_message::<Jumped>()
        .add_message::<Landed>()
        .add_message::<PlayerDied>()
        .add_message::<PlayerRespawned>()
        .add_message::<CheckpointReached>()
        .add_message::<LevelCompleted>()
        .add_message::<GusSays>()
        .add_message::<PlaySfx>();
}

/// Picked up a golden nugget (the coin) at `pos`.
#[derive(Message, Debug, Clone, Copy)]
pub struct NuggetCollected {
    pub pos: Vec2,
}

/// The player jumped. `double` is the mid-air "toot" jump.
#[derive(Message, Debug, Clone, Copy)]
pub struct Jumped {
    pub pos: Vec2,
    pub double: bool,
}

/// Touched ground after falling at `speed` px/s.
#[derive(Message, Debug, Clone, Copy)]
pub struct Landed {
    pub pos: Vec2,
    pub speed: f32,
}

/// Splat. Respawn at the last checkpoint follows after a short delay.
#[derive(Message, Debug, Clone, Copy)]
pub struct PlayerDied {
    pub pos: Vec2,
}

#[derive(Message, Debug, Clone, Copy)]
pub struct PlayerRespawned {
    pub pos: Vec2,
}

/// First touch of checkpoint number `index` (0-based, in level order) at `pos`.
#[derive(Message, Debug, Clone, Copy)]
pub struct CheckpointReached {
    pub index: usize,
    pub pos: Vec2,
}

/// Touched the goal flag. The app moves to [`crate::state::AppState::LevelComplete`].
#[derive(Message, Debug, Clone, Copy)]
pub struct LevelCompleted {
    pub level: usize,
    pub nuggets: u32,
    pub nuggets_total: u32,
    pub time_secs: f32,
}

/// Gus the plumber pipes up: show `text` in a speech bubble above him for a few seconds.
#[derive(Message, Debug, Clone)]
pub struct GusSays {
    pub text: String,
}

/// Play a one-shot sound effect (menus etc.; gameplay sounds are driven by the messages above).
#[derive(Message, Debug, Clone, Copy)]
pub struct PlaySfx(pub crate::audio::Sfx);
