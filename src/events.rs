//! Gameplay messages. The simulation writes them; UI, audio and FX read them.

use bevy::prelude::*;

use crate::adapt::Skill;

pub fn plugin(app: &mut App) {
    app.add_message::<NuggetCollected>()
        .add_message::<Jumped>()
        .add_message::<Landed>()
        .add_message::<PlayerDied>()
        .add_message::<PlayerRespawned>()
        .add_message::<CheckpointReached>()
        .add_message::<LevelCompleted>()
        .add_message::<HanSays>()
        .add_message::<HintRepeated>()
        .add_message::<PlaySfx>()
        .add_message::<LevelStarted>()
        .add_message::<BandFreedom>()
        .add_message::<CallBand>()
        .add_message::<crate::audio::MusicStarted>()
        .add_message::<crate::audio::MusicChanged>();
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
    pub cause: DeathCause,
    /// The skill the death was a failure at (`crate::game::hazards::death_skill`).
    pub skill: Skill,
    /// A death at something the adaptive engine rates easy for the player (never a designed
    /// death: a stain pit, a grease chute): the band laughs.
    pub slip: bool,
}

impl PlayerDied {
    /// A plain fall into the pit (tests, tools).
    pub fn at(pos: Vec2) -> Self {
        PlayerDied { pos, cause: DeathCause::Pit, skill: Skill::Precision, slip: false }
    }
}

/// What splatted Nat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum DeathCause {
    /// Fell out of the bottom of the level.
    Pit,
    /// Spikes (toilet brushes), floor or ceiling.
    Spikes,
    Sewage,
    Fly,
    Spray,
    /// Squashed by a crusher.
    Crusher,
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

/// Han the plumber pipes up: show `text` in a speech bubble above him for a few seconds.
#[derive(Message, Debug, Clone)]
pub struct HanSays {
    pub text: String,
}

/// Han repeats a hint after repeated deaths near it (the adaptive `han_hint` lever): its
/// lesson cards come up again (`ui::lesson`), since his bark alone teaches nothing.
#[derive(Message, Debug, Clone)]
pub struct HintRepeated {
    pub topics: Vec<crate::level::Topic>,
}

/// The player calls the band (Down): it plays the mode whose counter is full and was ticked
/// last (`audio::director::Band::ready`), if any.
#[derive(Message, Debug, Clone, Copy)]
pub struct CallBand;

/// Play a one-shot sound effect (menus etc.; gameplay sounds are driven by the messages above).
#[derive(Message, Debug, Clone, Copy)]
pub struct PlaySfx(pub crate::audio::Sfx);

/// A level was (re)loaded: entered from a menu or the previous level (`restart: false`), or
/// restarted from scratch (`restart: true`).
#[derive(Message, Debug, Clone, Copy)]
pub struct LevelStarted {
    pub level: usize,
    pub restart: bool,
}

/// How freely the band should play (each musician's freedom and the dynamics, 0..1), decided
/// by the adaptive engine from how the player is doing (`game::adaptive`). The audio plugin
/// passes it to the live engine.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct BandFreedom {
    pub lead: f32,
    pub comp: f32,
    pub bass: f32,
    pub drums: f32,
    pub dynamics: f32,
}
