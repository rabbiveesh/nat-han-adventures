//! **Side B** (prototype): Nat's gone, Han goes after him alone, and the player is the band.
//!
//! Han walks the level on his own AI, heading for the goal (his nav graph, `level::nav`, with
//! Nat's physics: [`HanPhys::side_b`], one toot). The player never moves him: each level hands
//! out a *set list* of calls (`setlist: giant=1 gap=1` in the level header), Left/Right picks
//! one, Down calls it ([`CallMode`]), and the band plays that mode as it does when Nat calls
//! it. Han takes whatever route is quickest under the physics now, so calling a mode steers him.
//!
//! - **Han heads for the goal**, or, when no route gets there, for the reachable cell nearest
//!   it (the *frontier*), and barks what blocks him there.
//! - **Han can die**: spikes, sewage, the pit. A death rewinds the whole level (the set list
//!   comes back with it): there are no checkpoints on Side B.
//! - **Stuck rewinds too**: at the frontier with no mode playing and nothing left on the set
//!   list, he gives up and the level rewinds.
//!
//! How it's wired: the level is a [`GeneratedLevel`] (so the story's levels and saves aren't
//! touched), and Nat stays in the world as an invisible, frozen *beacon* (`Finished`: no input,
//! no hazards, no pickups) that this module parks where Han should go. Han's brain follows it
//! as it follows Nat, with no gap, no parachute and none of his Nat reflexes (`HanNav::side_b`).

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::han::{HanBrain, HanMode, HanNav, nav_cell, stand};
use super::physics::{Body, Finished, PlayerControl};
use super::{ActiveLevel, GameSet, GeneratedLevel, Goal, Groove, Han, Player, Pos, RestartLevel, SimClock};
use crate::audio::Harmony;
use crate::events::HanSays;
use crate::input::Action;
use crate::level::buddy::HALF;
use crate::level::validate::Map;
use crate::level::{Level, TILE, Tile, Topic};
use crate::state::{AppState, PlayState};

/// Side B's levels, in play order.
pub static SIDE_B_SOURCES: [&str; 2] =
    [include_str!("../../levels/side-b/b1.txt"), include_str!("../../levels/side-b/b2.txt")];

/// A level rewinds this long after Han's death (the needle scratch).
pub const REWIND_SECS: f32 = 1.5;
/// Han gives up this long after he's stuck with nothing left to call.
pub const STUCK_SECS: f32 = 2.0;
/// The goal reached: this long on the cheer, then back to the title.
pub const CLEARED_SECS: f32 = 4.0;
/// Cells a frontier search may simulate per step.
const SEARCH_BUDGET: usize = 120;

pub const DEATH_LINE: &str = "Ow! Union's hearin' about this. Again!";
pub const STUCK_LINE: &str = "Outta music. Take it from the top!";
pub const CLEARED_LINE: &str = "Nat's hat! He went thataway!";

/// Ask the band for a mode by name (Side B's set list). Plays it at once, charged or not.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallMode(pub Harmony);

/// What a set-list call plays, by its header word.
pub fn mode_of(word: &str) -> Option<Harmony> {
    match word {
        "giant" => Some(Harmony::Coltrane),
        "gap" => Some(Harmony::Quartal),
        _ => None,
    }
}

/// The set list's name for a mode, on screen.
pub fn call_name(h: Harmony) -> &'static str {
    match h {
        Harmony::Coltrane => "GIANT STEPS",
        Harmony::Quartal => "FIRED UP",
        Harmony::Waltz => "WALTZ",
        _ => "?",
    }
}

/// A Side B level: the level, and its set list.
#[derive(Debug, Clone)]
pub struct SideBLevel {
    pub level: Level,
    pub setlist: Vec<(Harmony, u32)>,
}

impl SideBLevel {
    /// A level file with Side B's own header line (`setlist: giant=1 gap=1`) on top of the
    /// usual format (`level` module docs).
    pub fn parse(src: &str) -> Result<SideBLevel, String> {
        let mut setlist = Vec::new();
        let mut rest = String::new();
        let mut in_header = true;
        for line in src.lines() {
            if line.trim() == "---" {
                in_header = false;
            }
            match line.split_once(':') {
                Some((k, v)) if in_header && k.trim() == "setlist" => {
                    for item in v.split_whitespace() {
                        let (word, n) = item.split_once('=').ok_or_else(|| format!("bad set list item `{item}`"))?;
                        let h = mode_of(word).ok_or_else(|| format!("no such call `{word}`"))?;
                        let n = n.parse().map_err(|_| format!("bad count `{n}`"))?;
                        setlist.push((h, n));
                    }
                }
                _ => {
                    rest.push_str(line);
                    rest.push('\n');
                }
            }
        }
        Ok(SideBLevel { level: Level::parse(&rest)?, setlist })
    }

    pub fn load(n: usize) -> SideBLevel {
        SideBLevel::parse(SIDE_B_SOURCES[n]).unwrap_or_else(|e| panic!("side B level {}: {e}", n + 1))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub enum Phase {
    Playing,
    /// Han died (or gave up): the level rewinds in `t` s.
    Rewind { t: f32 },
    /// Han reached the goal.
    Cleared { t: f32 },
}

/// Side B is on: the level's set list and how the run's going. Exists while a Side B level is
/// being played.
#[derive(Resource, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct SideB {
    pub level: usize,
    #[reflect(ignore)]
    pub setlist: Vec<(Harmony, u32)>,
    /// Calls left, per set list entry.
    pub left: Vec<u32>,
    pub selected: usize,
    pub phase: Phase,
    /// Where Han's heading (a cell), and whether it's the frontier (no route to the goal).
    pub target: Option<(i32, i32)>,
    pub frontier: bool,
    pub stuck: f32,
    /// The frontier cell he last barked at (once per cell per try).
    pub barked: Option<(i32, i32)>,
    pub rewinds: u32,
    pub calls: u32,
}

impl SideB {
    pub fn new(level: usize, setlist: Vec<(Harmony, u32)>) -> SideB {
        let left = setlist.iter().map(|s| s.1).collect();
        SideB {
            level,
            setlist,
            left,
            selected: 0,
            phase: Phase::Playing,
            target: None,
            frontier: false,
            stuck: 0.0,
            barked: None,
            rewinds: 0,
            calls: 0,
        }
    }

    /// Back to the top: the set list refilled.
    fn rewound(&mut self) {
        *self = SideB { rewinds: self.rewinds + 1, selected: self.selected, ..SideB::new(self.level, self.setlist.clone()) };
    }

    pub fn calls_left(&self) -> u32 {
        self.left.iter().sum()
    }
}

/// Start Side B level `n` (0-based): the run's resources, then [`AppState::Playing`].
pub fn start(commands: &mut Commands, n: usize) {
    let b = SideBLevel::load(n);
    commands.insert_resource(SideB::new(n, b.setlist));
    commands.insert_resource(GeneratedLevel(b.level));
    commands.insert_resource(super::AssistMode::Manual);
    commands.queue(|w: &mut World| w.resource_mut::<NextState<AppState>>().set(AppState::Playing));
}

pub(super) fn plugin(app: &mut App) {
    app.add_message::<CallMode>()
        .register_type::<SideB>()
        .add_systems(
            Update,
            (
                choose_and_call.run_if(in_state(PlayState::Running)),
                refill_on_restart.run_if(in_state(AppState::Playing)),
            )
                .run_if(resource_exists::<SideB>),
        )
        .add_systems(
            FixedUpdate,
            (
                beacon.in_set(GameSet::Prepare).after(super::lifecycle::tick_clocks),
                han_fate.in_set(GameSet::Interact),
            )
                .run_if(resource_exists::<SideB>),
        );
    for s in [AppState::Title, AppState::LevelSelect, AppState::FreePlaySetup] {
        app.add_systems(OnEnter(s), leave);
    }
    #[cfg(not(target_arch = "wasm32"))]
    app.add_systems(Startup, dev_start);
}

/// Dev shortcut: `NATHAN_SIDE_B=1 cargo run` starts straight in Side B's level 1.
#[cfg(not(target_arch = "wasm32"))]
fn dev_start(mut commands: Commands) {
    if let Some(n) = std::env::var("NATHAN_SIDE_B").ok().and_then(|v| v.parse::<usize>().ok()) {
        start(&mut commands, n.saturating_sub(1).min(SIDE_B_SOURCES.len() - 1));
    }
}

/// Leaving Side B: the story gets its levels and assists back.
fn leave(mut commands: Commands, side: Option<Res<SideB>>, mut mode: ResMut<super::AssistMode>) {
    if side.is_some() {
        commands.remove_resource::<SideB>();
        commands.remove_resource::<GeneratedLevel>();
        *mode = super::AssistMode::Story;
    }
}

/// Left/Right picks a call on the set list, Down plays it.
fn choose_and_call(input: Single<&ActionState<Action>>, mut side: ResMut<SideB>, mut calls: MessageWriter<CallMode>) {
    let n = side.setlist.len();
    if n == 0 {
        return;
    }
    if input.just_pressed(&Action::Left) {
        side.selected = (side.selected + n - 1) % n;
    }
    if input.just_pressed(&Action::Right) {
        side.selected = (side.selected + 1) % n;
    }
    if input.just_pressed(&Action::Down) && side.phase == Phase::Playing {
        let k = side.selected;
        if side.left[k] > 0 {
            side.left[k] -= 1;
            side.calls += 1;
            calls.write(CallMode(side.setlist[k].0));
        }
    }
}

fn refill_on_restart(mut restarts: MessageReader<RestartLevel>, mut side: ResMut<SideB>) {
    if restarts.read().count() > 0 {
        side.rewound();
    }
}

/// Where Han should head now: the goal, or the frontier. Nat (the beacon) is parked there.
#[allow(clippy::type_complexity)]
fn beacon(
    mut commands: Commands,
    active: Res<ActiveLevel>,
    groove: Res<Groove>,
    nav: Option<ResMut<HanNav>>,
    mut side: ResMut<SideB>,
    mut nat: Query<(Entity, &mut Pos, &mut Body, &mut PlayerControl, Has<Finished>), (With<Player>, Without<Han>)>,
    han: Query<(&Pos, &Body), (With<Han>, Without<Player>)>,
) {
    let Ok((e, mut pos, mut body, mut ctl, finished)) = nat.single_mut() else { return };
    if !finished {
        commands.entity(e).insert(Finished);
    }
    let level = &active.level;
    let Some(mut nav) = nav else { return };
    if !nav.side_b {
        *nav = HanNav::side_b();
    }
    let Ok((hpos, hbody)) = han.single() else { return };
    // Re-plan from where he stands (mid-air he's carrying out a move already).
    if hbody.on_ground && side.phase == Phase::Playing {
        let map = Map::for_han(level);
        let from = nav_cell(level, hpos.0);
        let goal = (level.goal.0 as i32, level.goal.1 as i32);
        let (route, nearest) = nav.for_groove(&groove).route_or_nearest(&map, from, goal, SEARCH_BUDGET);
        match route {
            crate::level::nav::Route::Found(_) => {
                side.target = Some(goal);
                side.frontier = false;
            }
            crate::level::nav::Route::NoRoute => {
                side.target = Some(nearest);
                side.frontier = true;
            }
            crate::level::nav::Route::Budget => {}
        }
    }
    let Some(t) = side.target else { return };
    pos.0 = stand(level, t);
    body.vel = Vec2::ZERO;
    body.on_ground = true;
    body.riding = None;
    ctl.facing = if t.0 as f32 * TILE >= hpos.0.x { 1.0 } else { -1.0 };
}

/// What Han says when he's stopped at the frontier: the gate right ahead of him, or (none
/// there, a mode playing) the music that's in his way.
pub fn bark_line(level: &Level, at: (i32, i32), playing: bool) -> &'static str {
    let near = |g: &&crate::level::GateMark| (g.c0 - 3..=g.c1 + 3).contains(&at.0);
    let gate = level.gates.iter().find(near);
    if gate.is_none() && playing {
        return "Whoa! Not with THIS music on!";
    }
    match gate.map(|g| g.topic) {
        Some(Topic::Giant) => "Too tall for these legs. Music, please!",
        Some(Topic::Gap) => "Too far to jump. Music, please!",
        Some(Topic::Waltz) => "Can't get across. Gimme a waltz!",
        _ => "Hmm. I need some music here.",
    }
}

/// Is Han's box in a spike tile's deadly half?
fn on_spikes(level: &Level, pos: Vec2) -> bool {
    let shrink = Vec2::splat(super::FORGIVE);
    let (min, max) = (pos - HALF + shrink, pos + HALF - shrink);
    let (c0, r1) = level.cell_at(min);
    let (c1, r0) = level.cell_at(max);
    for r in r0..=r1 {
        for c in c0..=c1 {
            let t = level.tile(c, r);
            if !matches!(t, Tile::SpikesUp | Tile::SpikesDown) || c < 0 || r < 0 {
                continue;
            }
            let center = level.tile_center(c as usize, r as usize);
            let (lo, hi) = if t == Tile::SpikesUp {
                (center.y - TILE / 2.0, center.y)
            } else {
                (center.y, center.y + TILE / 2.0)
            };
            let x_in = max.x > center.x - TILE / 2.0 && min.x < center.x + TILE / 2.0;
            if x_in && max.y > lo && min.y < hi {
                return true;
            }
        }
    }
    false
}

/// The goal, a death, a stuck Han; and the rewind or the cheer when their time's up.
#[allow(clippy::too_many_arguments)]
fn han_fate(
    time: Res<Time>,
    active: Res<ActiveLevel>,
    groove: Res<Groove>,
    clock: Res<SimClock>,
    mut side: ResMut<SideB>,
    mut han: Query<(&Pos, &Body, &mut HanBrain), With<Han>>,
    goal: Query<&Transform, With<Goal>>,
    mut says: MessageWriter<HanSays>,
    mut restart: MessageWriter<RestartLevel>,
    mut next: ResMut<NextState<AppState>>,
) {
    let dt = time.delta_secs();
    let level = &active.level;
    let say = |says: &mut MessageWriter<HanSays>, t: &str| {
        says.write(HanSays { text: t.to_string() });
    };
    match side.phase {
        Phase::Rewind { t } => {
            if t - dt <= 0.0 {
                restart.write(RestartLevel);
            } else {
                side.phase = Phase::Rewind { t: t - dt };
            }
            return;
        }
        Phase::Cleared { t } => {
            if t - dt <= 0.0 {
                next.set(AppState::Title);
            } else {
                side.phase = Phase::Cleared { t: t - dt };
            }
            return;
        }
        Phase::Playing => {}
    }
    let Ok((pos, body, mut brain)) = han.single_mut() else { return };
    // (A moment's grace at the top, while the level settles.)
    if clock.time < 0.1 {
        return;
    }
    if let Ok(g) = goal.single() {
        let floor = g.translation.truncate();
        let (min, max) = (floor - Vec2::new(5.0, 0.0), floor + Vec2::new(5.0, 2.0 * TILE));
        if pos.0.x + HALF.x > min.x && pos.0.x - HALF.x < max.x && pos.0.y + HALF.y > min.y && pos.0.y - HALF.y < max.y {
            side.phase = Phase::Cleared { t: CLEARED_SECS };
            say(&mut says, CLEARED_LINE);
            return;
        }
    }
    let sunk = matches!(brain.mode, HanMode::Sinking { .. } | HanMode::Gone { .. });
    let fell = pos.0.y + HALF.y < 0.0;
    let spiked = on_spikes(level, pos.0);
    if sunk || fell || spiked {
        if spiked {
            brain.mode = HanMode::Sinking { t: 30.0 };
        }
        side.phase = Phase::Rewind { t: REWIND_SECS };
        say(&mut says, DEATH_LINE);
        return;
    }
    // At the frontier: bark once, and with nothing left to call, give up.
    let here = nav_cell(level, pos.0);
    let at_frontier = side.frontier && body.on_ground && side.target.is_some_and(|t| (t.0 - here.0).abs() <= 1 && t.1 == here.1);
    if at_frontier && side.barked != side.target {
        side.barked = side.target;
        say(&mut says, bark_line(level, here, groove.harmony != Harmony::Original));
    }
    let nothing_playing = groove.harmony == Harmony::Original;
    if at_frontier && nothing_playing && side.calls_left() == 0 {
        side.stuck += dt;
        if side.stuck >= STUCK_SECS {
            side.phase = Phase::Rewind { t: REWIND_SECS };
            say(&mut says, STUCK_LINE);
        }
    } else {
        side.stuck = 0.0;
    }
}
