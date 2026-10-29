//! **THE SECRET PLAYGROUND**: a hidden world of silly toys, one playground level each
//! (`levels/toybox/`): the Boom, sploosh diapers, the "helper" on his rope, the blast button,
//! stop signs, schnitzel houses, the policeman and the building code. No Han, no saves to beat: every level's open, and
//! nothing's recorded but that you played.
//!
//! Wired like NAT ALONE (`super::solo`): the level is a [`GeneratedLevel`] (the story's levels
//! and assist dial aren't touched; [`AssistMode::Manual`]) and [`ToyRun`] says which one. The
//! way in: Up Down Up Down on the title (it's never listed there), `?toybox=1` on the web,
//! `NATHAN_TOYBOX=1 cargo run`.

use bevy::prelude::*;

use super::{AssistMode, GeneratedLevel};
use crate::level::Level;
use crate::state::{AppState, Shelf};

/// How many playgrounds there are.
pub const TOY_COUNT: usize = 8;

/// The playgrounds, in play order.
pub static TOY_SOURCES: [&str; TOY_COUNT] = [
    include_str!("../../levels/toybox/boom.txt"),
    include_str!("../../levels/toybox/sploosh.txt"),
    include_str!("../../levels/toybox/helper.txt"),
    include_str!("../../levels/toybox/blast.txt"),
    include_str!("../../levels/toybox/stop.txt"),
    include_str!("../../levels/toybox/houses.txt"),
    include_str!("../../levels/toybox/police.txt"),
    include_str!("../../levels/toybox/code.txt"),
];

/// The playground being played (0-based).
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Resource)]
pub struct ToyRun(pub usize);

/// Playground `n`'s name, from its header.
pub fn name(n: usize) -> &'static str {
    TOY_SOURCES[n].lines().find_map(|l| l.strip_prefix("name:")).map_or("???", str::trim)
}

/// Playground `n`, parsed.
pub fn load(n: usize) -> Level {
    Level::parse(TOY_SOURCES[n]).unwrap_or_else(|e| panic!("playground {}: {e}", n + 1))
}

/// Start playground `n` (0-based): its resources, then [`AppState::Playing`].
pub fn start(commands: &mut Commands, n: usize) {
    commands.insert_resource(ToyRun(n));
    commands.insert_resource(GeneratedLevel(load(n)));
    commands.insert_resource(AssistMode::Manual);
    commands.insert_resource(Shelf::Toybox);
    commands.queue(|w: &mut World| w.resource_mut::<NextState<AppState>>().set(AppState::Playing));
}

pub(super) fn plugin(app: &mut App) {
    app.register_type::<ToyRun>();
    for s in [AppState::Title, AppState::LevelSelect, AppState::FreePlaySetup] {
        app.add_systems(OnEnter(s), leave);
    }
    app.add_systems(Startup, dev_start);
}

/// Leaving the playground: the story gets its levels and assists back.
fn leave(mut commands: Commands, toy: Option<Res<ToyRun>>, mut mode: ResMut<AssistMode>) {
    if toy.is_some() {
        commands.remove_resource::<ToyRun>();
        commands.remove_resource::<GeneratedLevel>();
        *mode = AssistMode::Story;
    }
}

/// `NATHAN_TOYBOX=n cargo run`, or `?toybox=n` on the web: straight into playground n.
fn dev_start(mut commands: Commands) {
    if let Some(n) = asked_for() {
        start(&mut commands, n.saturating_sub(1).min(TOY_COUNT - 1));
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn asked_for() -> Option<usize> {
    std::env::var("NATHAN_TOYBOX").ok()?.parse().ok()
}

#[cfg(target_arch = "wasm32")]
fn asked_for() -> Option<usize> {
    let search = web_sys::window()?.location().search().ok()?;
    search.trim_start_matches('?').split('&').find_map(|kv| kv.strip_prefix("toybox=")?.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_playground_parses_and_is_nat_alone() {
        for n in 0..TOY_COUNT {
            let level = load(n);
            assert!(level.solo, "{} has no `solo: nat`", name(n));
            assert_eq!(level.name, name(n));
        }
    }
}
