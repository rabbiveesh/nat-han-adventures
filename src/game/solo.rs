//! **NAT ALONE**, the gadget world: ten levels of Nat without Han (`solo: nat`), built from the
//! gadget tiles, laid sideways and upside down too: wall and ceiling fans, cushions,
//! brushes, ducks, beat blocks and crushers (`levels/solo/`). Each level's run is scripted in
//! `scripts/tours/` and played in `tests/showcase.rs`.
//!
//! Wired like Side B: the level is a [`GeneratedLevel`] (the story's levels, saves and assist
//! dial aren't touched; [`AssistMode::Manual`]) and [`SoloRun`] says which one. The results
//! card records it in [`Progress`](crate::save::Progress)'s `solo_*` and goes on to the next.
//! The third world: NAT ALONE shows on the title once Side B's cleared. Other ways in: Down
//! Down Up Up on the title, `?solo=1` on the web, `NATHAN_SOLO=1 cargo run`.

use bevy::prelude::*;

use super::{AssistMode, GeneratedLevel};
use crate::level::Level;
use crate::state::{AppState, Shelf};

/// How many NAT ALONE levels there are.
pub const SOLO_COUNT: usize = 10;

/// NAT ALONE's levels, in play order.
pub static SOLO_SOURCES: [&str; SOLO_COUNT] = [
    include_str!("../../levels/solo/breezy.txt"),
    include_str!("../../levels/solo/bouncy.txt"),
    include_str!("../../levels/solo/spout.txt"),
    include_str!("../../levels/solo/brush.txt"),
    include_str!("../../levels/solo/tilt.txt"),
    include_str!("../../levels/solo/wind.txt"),
    include_str!("../../levels/solo/disco.txt"),
    include_str!("../../levels/solo/spin.txt"),
    include_str!("../../levels/solo/pingpong.txt"),
    include_str!("../../levels/solo/flush.txt"),
];

/// The NAT ALONE level being played (0-based).
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Resource)]
pub struct SoloRun(pub usize);

/// Level `n`'s name, from its header.
pub fn name(n: usize) -> &'static str {
    SOLO_SOURCES[n].lines().find_map(|l| l.strip_prefix("name:")).map_or("???", str::trim)
}

/// The first NAT ALONE level (0-based) whose map has `tile` in it: the one that teaches it.
pub fn teaches(tile: char) -> Option<usize> {
    SOLO_SOURCES.iter().position(|src| src.split_once("\n---\n").is_some_and(|(_, map)| map.contains(tile)))
}

/// Level `n`, parsed.
pub fn load(n: usize) -> Level {
    Level::parse(SOLO_SOURCES[n]).unwrap_or_else(|e| panic!("NAT ALONE level {}: {e}", n + 1))
}

/// Start NAT ALONE level `n` (0-based): its resources, then [`AppState::Playing`] (from the
/// results card too: `Playing` is entered afresh).
pub fn start(commands: &mut Commands, n: usize) {
    commands.insert_resource(SoloRun(n));
    commands.insert_resource(GeneratedLevel(load(n)));
    commands.insert_resource(AssistMode::Manual);
    commands.insert_resource(Shelf::Solo);
    commands.queue(|w: &mut World| w.resource_mut::<NextState<AppState>>().set(AppState::Playing));
}

pub(super) fn plugin(app: &mut App) {
    app.register_type::<SoloRun>();
    for s in [AppState::Title, AppState::LevelSelect, AppState::FreePlaySetup] {
        app.add_systems(OnEnter(s), leave);
    }
    app.add_systems(Startup, dev_start);
}

/// Leaving NAT ALONE: the story gets its levels and assists back.
fn leave(mut commands: Commands, solo: Option<Res<SoloRun>>, mut mode: ResMut<AssistMode>) {
    if solo.is_some() {
        commands.remove_resource::<SoloRun>();
        commands.remove_resource::<GeneratedLevel>();
        *mode = AssistMode::Story;
    }
}

/// `NATHAN_SOLO=n cargo run`, or `?solo=n` on the web: straight into NAT ALONE's level n.
fn dev_start(mut commands: Commands) {
    if let Some(n) = asked_for() {
        start(&mut commands, n.saturating_sub(1).min(SOLO_COUNT - 1));
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn asked_for() -> Option<usize> {
    std::env::var("NATHAN_SOLO").ok()?.parse().ok()
}

#[cfg(target_arch = "wasm32")]
fn asked_for() -> Option<usize> {
    let search = web_sys::window()?.location().search().ok()?;
    search.trim_start_matches('?').split('&').find_map(|kv| kv.strip_prefix("solo=")?.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_level_parses_and_is_nat_alone() {
        for n in 0..SOLO_COUNT {
            let level = load(n);
            assert!(level.solo, "{} has no `solo: nat`", name(n));
            assert_eq!(level.name, name(n));
        }
    }

    #[test]
    fn every_sideways_tile_is_taught() {
        for tile in ['e', 'w', '!', 'b', 'd', 'm', '|'] {
            assert!(teaches(tile).is_some(), "no NAT ALONE level has {tile}");
        }
        assert_eq!(teaches('e'), Some(0), "Breezy Does It opens with the wall fans");
    }
}
