//! A 2D platformer about a brave little poo, its plumber sidekick Han, and ten levels
//! of increasingly unsanitary terrain.
//!
//! Split into `gameplay` (headless-testable simulation) and `presentation`
//! (rendering-only: sprites, UI, FX, audio). Tests build an app from `gameplay` alone.
//!
//! Shared contracts every module builds on:
//! - [`state`]: app states and the current level.
//! - [`input`]: the [`input::Action`] enum (arrows/WASD/space) on a global entity.
//! - [`events`]: gameplay messages (nugget collected, died, ...) that UI/audio/FX react to.
//! - [`level`]: the ASCII level format and its parser.
//! - [`save`]: persistent progress (unlocked levels, best nugget counts).

pub mod art;
pub mod audio;
pub mod debug;
pub mod events;
pub mod game;
pub mod input;
pub mod level;
pub mod save;
pub mod state;
pub mod ui;

use bevy::prelude::*;

/// The simulation: input, levels, player physics, hazards, progression. Runs without a window.
pub fn gameplay(app: &mut App) {
    app.add_plugins((
        state::plugin,
        input::plugin,
        events::plugin,
        save::plugin,
        game::plugin,
    ));
}

/// Everything that only matters when there's a screen and speakers.
pub fn presentation(app: &mut App) {
    app.add_plugins((art::plugin, game::visuals_plugin, ui::plugin, audio::plugin, debug::plugin));
}
