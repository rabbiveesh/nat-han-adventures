//! **World -1** (prototype): Nat alone in the broken memory under the cartridge, glitching his
//! way out. No Han.
//!
//! The pen is a bug: **pausing mid-toot** opens a *corrupted* pause menu ([`GlitchMenu`]):
//! junk entries, some of them tiles leaking in. Picking one rewrites the tile Nat took off from
//! (his last footing, [`MinusOne::takeoff`]) into the picked tile, everywhere in the level, in
//! both look and behaviour (the grid in [`ActiveLevel`] is what physics reads and what's drawn).
//! Take off from ground, pick spikes: all the ground is toilet brushes.
//!
//! - **The arrival**: every load drops Nat in from the sky and the game "crashes" into the
//!   corrupted menu on its own, so the first glitch happens *to* the player. The takeoff
//!   register is uninitialized then: it reads ground.
//! - **Restart** (R) is never glitchy: it reloads the level's clean original, menu open or not.
//!
//! Memory safety: there's no memory trick here at all. The "corruption" is an ordinary edit of
//! the level grid through safe Rust; the bug only exists in the fiction.

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::lifecycle::spawn_tiles;
use super::physics::Body;
use super::{ActiveLevel, GameSet, GeneratedLevel, Han, LevelTile, Player, Pos, RestartLevel, SimClock, Stain};
use crate::events::{CheckpointReached, Jumped};
use crate::input::Action;
use crate::level::{Level, Tile};
use crate::state::{AppState, PlayState};

pub static MINUS_ONE_SOURCES: [&str; 3] = [
    include_str!("../../levels/minus-1/m1.txt"),
    include_str!("../../levels/minus-1/m2.txt"),
    include_str!("../../levels/minus-1/m3.txt"),
];

/// A pause this long after a toot (still in the air) is a mid-toot pause.
pub const TOOT_WINDOW: f32 = 0.6;
/// The arrival crash: this long after a load.
pub const ARRIVAL_SECS: f32 = 0.25;
/// On the goal this long, then back to the title.
pub const ESCAPED_SECS: f32 = 3.0;

/// One line of the corrupted menu: its junk text, and the tile it writes (`None`: nothing,
/// the menu just closes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    pub text: &'static str,
    pub tile: Option<Tile>,
}

/// The corrupted menu's entries, top to bottom.
pub const ENTRIES: [Entry; 6] = [
    Entry { text: "R3$UM#", tile: None },
    Entry { text: "^^^^^^", tile: Some(Tile::SpikesUp) },
    Entry { text: "~~~~~~", tile: Some(Tile::Liquid) },
    Entry { text: "      ", tile: Some(Tile::Empty) },
    Entry { text: "======", tile: Some(Tile::OneWay) },
    Entry { text: "#_#_#_", tile: Some(Tile::Grease) },
];

/// World -1 is on. Exists while a World -1 level is being played.
#[derive(Resource, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct MinusOne {
    pub level: usize,
    /// The tile Nat last stood on (uninitialized: the level's `crash:` garbage).
    pub takeoff: Tile,
    /// What the takeoff register holds before Nat stands anywhere (`crash:`, default ground).
    pub crash: Tile,
    /// Seconds since the last toot (large: none lately).
    pub since_toot: f32,
    /// The arrival crash happened for this load.
    pub arrived: bool,
    pub writes: u32,
    /// Seconds since Nat reached the goal (then back to the title, [`ESCAPED_SECS`]).
    pub escaped: Option<f32>,
}

impl MinusOne {
    pub fn new(level: usize, crash: Tile) -> Self {
        MinusOne {
            level,
            takeoff: crash,
            crash,
            since_toot: f32::INFINITY,
            arrived: false,
            writes: 0,
            escaped: None,
        }
    }
}

/// Why the game froze into the corrupted menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum Freeze {
    Arrival,
    MidToot,
    /// A checkpoint touched in mid-air: "SAVING..." (don't turn off the power).
    Saving,
}

/// The corrupted menu is up (with [`PlayState::Paused`]; the real pause menu stays away).
#[derive(Resource, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct GlitchMenu {
    pub cursor: usize,
    pub freeze: Freeze,
    /// The tile a pick rewrites.
    pub target: Tile,
}

/// Every `from` tile in the level became `to` (for the visuals' glitch flash).
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileRewrite {
    pub from: Tile,
    pub to: Tile,
}

/// What a `crash:` word (and a level file's tile names) stands for.
pub fn tile_of(word: &str) -> Option<Tile> {
    Some(match word {
        "ground" => Tile::Solid,
        "oneway" => Tile::OneWay,
        "spikes" => Tile::SpikesUp,
        "sewage" => Tile::Liquid,
        "grease" => Tile::Grease,
        _ => return None,
    })
}

/// A World -1 level: the usual format (`level` module docs) plus `crash: <tile>`, the garbage
/// in the takeoff register when the level loads (what the arrival crash aims at).
pub fn load(n: usize) -> (Level, Tile) {
    parse(MINUS_ONE_SOURCES[n]).unwrap_or_else(|e| panic!("World -1 level {}: {e}", n + 1))
}

pub fn parse(src: &str) -> Result<(Level, Tile), String> {
    let mut crash = Tile::Solid;
    let mut rest = String::new();
    let mut in_header = true;
    for line in src.lines() {
        if line.trim() == "---" {
            in_header = false;
        }
        match line.split_once(':') {
            Some((k, v)) if in_header && k.trim() == "crash" => {
                crash = tile_of(v.trim()).ok_or_else(|| format!("no such tile `{}`", v.trim()))?;
            }
            _ => {
                rest.push_str(line);
                rest.push('\n');
            }
        }
    }
    Ok((Level::parse(&rest)?, crash))
}

/// Start World -1 level `n` (0-based).
pub fn start(commands: &mut Commands, n: usize) {
    let (level, crash) = load(n);
    commands.insert_resource(MinusOne::new(n, crash));
    commands.insert_resource(GeneratedLevel(level));
    commands.insert_resource(super::AssistMode::Manual);
    commands.queue(|w: &mut World| w.resource_mut::<NextState<AppState>>().set(AppState::Playing));
}

pub(super) fn plugin(app: &mut App) {
    app.add_message::<TileRewrite>()
        .register_type::<MinusOne>()
        .register_type::<GlitchMenu>()
        .add_observer(no_han)
        .add_observer(fresh_load)
        .add_systems(
            Update,
            (
                pause_mid_toot.run_if(in_state(PlayState::Running)),
                escape.before(super::lifecycle::restart_level),
                menu_input.run_if(in_state(PlayState::Paused).and_then(resource_exists::<GlitchMenu>)),
            )
                .run_if(resource_exists::<MinusOne>),
        )
        .add_systems(
            FixedUpdate,
            (track, arrival, saving).chain().in_set(GameSet::Follow).run_if(resource_exists::<MinusOne>),
        );
    for s in [AppState::Title, AppState::LevelSelect, AppState::FreePlaySetup] {
        app.add_systems(OnEnter(s), leave);
    }
    app.add_systems(Startup, dev_start);
}

/// `NATHAN_MINUS_ONE=1 cargo run`, or `?minus1=1` on the web.
fn dev_start(mut commands: Commands) {
    if let Some(n) = asked_for() {
        start(&mut commands, n.saturating_sub(1).min(MINUS_ONE_SOURCES.len() - 1));
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn asked_for() -> Option<usize> {
    std::env::var("NATHAN_MINUS_ONE").ok()?.parse().ok()
}

#[cfg(target_arch = "wasm32")]
fn asked_for() -> Option<usize> {
    let search = web_sys::window()?.location().search().ok()?;
    search.trim_start_matches('?').split('&').find_map(|kv| kv.strip_prefix("minus1=")?.parse().ok())
}

fn leave(mut commands: Commands, minus: Option<Res<MinusOne>>, mut mode: ResMut<super::AssistMode>) {
    if minus.is_some() {
        commands.remove_resource::<MinusOne>();
        commands.remove_resource::<GlitchMenu>();
        commands.remove_resource::<GeneratedLevel>();
        *mode = super::AssistMode::Story;
    }
}

/// Nat's alone down here.
fn no_han(add: On<Add, Han>, minus: Option<Res<MinusOne>>, mut commands: Commands) {
    if minus.is_some() {
        commands.entity(add.entity).despawn();
    }
}

/// A (re)load: the registers are garbage again, and the arrival crash comes back.
fn fresh_load(_: On<Add, Player>, minus: Option<ResMut<MinusOne>>, mut commands: Commands) {
    if let Some(mut m) = minus {
        *m = MinusOne { writes: m.writes, ..MinusOne::new(m.level, m.crash) };
        commands.remove_resource::<GlitchMenu>();
    }
}

/// On the goal a moment, then the next World -1 level (after the last: the title).
fn escape(
    time: Res<Time>,
    mut minus: ResMut<MinusOne>,
    mut commands: Commands,
    mut next: ResMut<NextState<AppState>>,
    mut restart: MessageWriter<RestartLevel>,
) {
    let Some(t) = &mut minus.escaped else { return };
    *t += time.delta_secs();
    if *t < ESCAPED_SECS {
        return;
    }
    let n = minus.level + 1;
    if n < MINUS_ONE_SOURCES.len() {
        let (level, crash) = load(n);
        *minus = MinusOne::new(n, crash);
        commands.insert_resource(GeneratedLevel(level));
        restart.write(RestartLevel);
    } else {
        minus.escaped = None;
        next.set(AppState::Title);
    }
}

/// Nat's footing (the takeoff register) and the toot clock.
fn track(
    time: Res<Time>,
    mut minus: ResMut<MinusOne>,
    active: Res<ActiveLevel>,
    mut jumped: MessageReader<Jumped>,
    nat: Query<(&Pos, &Body), With<Player>>,
) {
    minus.since_toot += time.delta_secs();
    if jumped.read().any(|j| j.double) {
        minus.since_toot = 0.0;
    }
    let Ok((pos, body)) = nat.single() else { return };
    if body.on_ground {
        if let Some(t) = footing(&active.level, pos.0, body) {
            minus.takeoff = t;
        }
    }
}

/// The tile under Nat's feet, if it's something (not a moving platform's empty cell). A splat
/// is only paint on the spikes: standing on one, Nat's footing is the spikes under it.
fn footing(level: &Level, pos: Vec2, body: &Body) -> Option<Tile> {
    let (c, r) = level.cell_at(pos - Vec2::new(0.0, body.half.y + 0.5));
    let t = match level.tile(c, r) {
        Tile::StainUp => Tile::SpikesUp,
        Tile::StainDown => Tile::SpikesDown,
        t => t,
    };
    Some(t).filter(|&t| t != Tile::Empty && r >= 0 && (r as usize) < level.height)
}

/// A checkpoint touched in mid-air freezes the game to save, and the save screen is the same
/// corrupted menu.
fn saving(
    mut reached: MessageReader<CheckpointReached>,
    minus: Res<MinusOne>,
    nat: Query<&Body, With<Player>>,
    mut commands: Commands,
    mut next: ResMut<NextState<PlayState>>,
) {
    let airborne = nat.single().is_ok_and(|b| !b.on_ground);
    if reached.read().count() > 0 && airborne {
        commands.insert_resource(GlitchMenu { cursor: 0, freeze: Freeze::Saving, target: minus.takeoff });
        next.set(PlayState::Paused);
    }
}

fn arrival(
    clock: Res<SimClock>,
    mut minus: ResMut<MinusOne>,
    mut commands: Commands,
    mut next: ResMut<NextState<PlayState>>,
) {
    if !minus.arrived && clock.time >= ARRIVAL_SECS {
        minus.arrived = true;
        commands.insert_resource(GlitchMenu { cursor: 0, freeze: Freeze::Arrival, target: minus.takeoff });
        next.set(PlayState::Paused);
    }
}

/// Pause mid-toot: the menu comes up corrupted (the plain pause handles every other pause).
fn pause_mid_toot(
    input: Single<&ActionState<Action>>,
    minus: Res<MinusOne>,
    nat: Query<&Body, With<Player>>,
    mut commands: Commands,
) {
    let airborne = nat.single().is_ok_and(|b| !b.on_ground);
    if input.just_pressed(&Action::Back) && airborne && minus.since_toot < TOOT_WINDOW {
        commands.insert_resource(GlitchMenu { cursor: 0, freeze: Freeze::MidToot, target: minus.takeoff });
    }
}

fn menu_input(
    input: Single<&ActionState<Action>>,
    mut menu: ResMut<GlitchMenu>,
    mut minus: ResMut<MinusOne>,
    mut active: ResMut<ActiveLevel>,
    tiles: Query<Entity, With<LevelTile>>,
    stains: Query<(Entity, &Stain)>,
    mut commands: Commands,
    mut next: ResMut<NextState<PlayState>>,
    mut restart: MessageWriter<RestartLevel>,
    mut rewrites: MessageWriter<TileRewrite>,
) {
    let n = ENTRIES.len();
    if input.just_pressed(&Action::Up) {
        menu.cursor = (menu.cursor + n - 1) % n;
    }
    if input.just_pressed(&Action::Down) {
        menu.cursor = (menu.cursor + 1) % n;
    }
    let close = |commands: &mut Commands, next: &mut NextState<PlayState>| {
        commands.remove_resource::<GlitchMenu>();
        next.set(PlayState::Running);
    };
    if input.just_pressed(&Action::Restart) {
        restart.write(RestartLevel);
        close(&mut commands, &mut next);
    } else if input.just_pressed(&Action::Back) {
        close(&mut commands, &mut next);
    } else if input.just_pressed(&Action::Confirm) {
        if let Some(to) = ENTRIES[menu.cursor].tile {
            let from = menu.target;
            if rewrite(&mut active.level, from, to) {
                for e in &tiles {
                    commands.entity(e).despawn();
                }
                // Splats on rewritten spikes go with them.
                for (e, st) in &stains {
                    if active.level.tile(st.col as i32, st.row as i32) != st.tile {
                        commands.entity(e).despawn();
                    }
                }
                let w = active.level.width;
                spawn_tiles(&mut commands, &active.level, 0..w);
                minus.writes += 1;
                rewrites.write(TileRewrite { from, to });
            }
        }
        close(&mut commands, &mut next);
    }
}

/// Every `from` cell becomes `to` (spikes take their splats with them: the paint goes with the
/// tile). False if nothing changed.
pub fn rewrite(level: &mut Level, from: Tile, to: Tile) -> bool {
    if from == to {
        return false;
    }
    let paint = from.stained();
    let mut any = false;
    for t in level.tiles.iter_mut().filter(|t| **t == from || Some(**t) == paint) {
        *t = to;
        any = true;
    }
    any
}
