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

pub mod adapt;
pub mod art;
pub mod audio;
#[cfg(feature = "capture")]
pub mod capture;
pub mod debug;
pub mod events;
pub mod freeplay;
pub mod game;
pub mod input;
pub mod level;
pub mod save;
pub mod state;
pub mod touch;
pub mod ui;
#[cfg(target_arch = "wasm32")]
pub mod web;
#[cfg(not(target_arch = "wasm32"))]
pub mod workspace;

use bevy::prelude::*;

/// The simulation: input, levels, player physics, hazards, progression. Runs without a window.
pub fn gameplay(app: &mut App) {
    app.add_plugins((
        state::plugin,
        input::plugin,
        events::plugin,
        save::plugin,
        game::plugin,
        freeplay::plugin,
    ));
}

/// Everything that only matters when there's a screen and speakers.
pub fn presentation(app: &mut App) {
    app.add_plugins((art::plugin, game::visuals_plugin, ui::plugin, audio::plugin, debug::plugin, touch::plugin));
}

/// The game, windowed: `cargo run` (and the level editor's playtest, `NATHAN_LEVEL=<file>`,
/// which also turns off remote control: the editor may run several).
pub fn run_game() {
    // `NATHAN_CAPTURE=<dir>`: record a deterministic video (see `nat_han_adventures::capture`).
    #[cfg(feature = "capture")]
    let capture = crate::capture::CaptureConfig::from_env()
        .map(|c| c.unwrap_or_else(|e| panic!("NATHAN_CAPTURE: {e}")));

    let window = Window {
        title: "Nat Han Adventures".into(),
        // Web: render into the page's canvas and keep arrows/space from scrolling it.
        canvas: Some("#game".into()),
        fit_canvas_to_parent: true,
        prevent_default_event_handling: true,
        ..default()
    };
    #[cfg(feature = "capture")]
    let window = if capture.is_some() { crate::capture::capture_window(window) } else { window };
    let plugins = DefaultPlugins
        .set(WindowPlugin { primary_window: Some(window), ..default() })
        // Chunky pixels: never blur our 16x16 sprites.
        .set(ImagePlugin::default_nearest());
    #[cfg(feature = "capture")]
    let plugins = if capture.is_some() { crate::capture::adjust_plugins(plugins) } else { plugins };

    let mut app = App::new();
    app.add_plugins((plugins, crate::gameplay, crate::presentation))
        .insert_resource(ClearColor(Color::srgb_u8(0x1a, 0x12, 0x0c)));

    // No remote control while capturing: the timeline is the only input.
    #[cfg(feature = "capture")]
    let remote = capture.is_none() && crate::game::playtest::file_from_env().is_none();
    #[cfg(not(feature = "capture"))]
    let remote = crate::game::playtest::file_from_env().is_none();
    #[cfg(feature = "brp")]
    if remote {
        app.add_plugins(bevy_brp_extras::BrpExtrasPlugin);
    }
    let _ = remote;
    #[cfg(feature = "capture")]
    if let Some(c) = capture {
        app.add_plugins(crate::capture::CapturePlugin(c));
    }

    app.run();
}
