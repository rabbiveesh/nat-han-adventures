//! **Side B** (prototype): Nat's gone, Han goes after him alone, and the player is the band.
//!
//! Han walks the level on his own AI, heading for the goal (his nav graph, `level::nav`, with
//! Nat's physics: [`HanPhys::side_b`], one toot). The player never moves him: each level has a
//! *set list* of calls (`setlist: giant gap melt` in the level header), empty at the start:
//! every nugget Han grabs is one more of the call that's picked (Left/Right), and Down or Jump
//! calls it ([`CallMode`]); the band plays that mode as it does when Nat calls it. Han takes
//! whatever route is quickest under the physics now, so calling a mode steers him, and the
//! levels fork: over a pipe or under it, on the deck for its nuggets or through the tunnel.
//!
//! - **Han heads for the goal**, or, when no route gets there, for the reachable cell nearest
//!   it (the *frontier*), and from there he **charges**: runs on, jumping at walls and edges,
//!   tooting at the top. Without the right music playing by then, that's comically fatal.
//! - **Han can die**: spikes, sewage, the pit, and knocking himself out on a wall he can't
//!   climb. A death rewinds the whole level (the set list comes back with it): there are no
//!   checkpoints on Side B.
//! - **Melting** (`melt` on the set list): the laughing band, held on its melting phrase. Han
//!   shrinks to [`MELT_SIZE`] and ducks under ceiling spikes he'd otherwise lose his cap to,
//!   but if it wears off while he's under them, he grows back into them.
//!
//! How it's wired: the level is a [`GeneratedLevel`] (so the story's levels and saves aren't
//! touched), and Nat stays in the world as an invisible, frozen *beacon* (`Finished`: no input,
//! no hazards, no pickups) that this module parks where Han should go. Han's brain follows it
//! as it follows Nat, with no gap, no parachute and none of his Nat reflexes (`HanNav::side_b`).

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::han::{HanBrain, HanMode, HanNav, nav_cell, stand};
use super::physics::{Body, Finished, PlayerControl};
use super::{ActiveLevel, GameSet, GeneratedLevel, Goal, Groove, Han, Nugget, Player, Pos, PrevPos, RestartLevel, SimClock};
use crate::audio::Harmony;
use crate::events::HanSays;
use crate::input::Action;
use crate::level::buddy::HALF;
use crate::level::validate::Map;
use crate::level::{Level, TILE, Tile, Topic};
use crate::state::{AppState, PlayState};

/// Side B's levels, in play order.
/// How many Side B levels there are.
pub const SIDE_B_COUNT: usize = 10;

pub static SIDE_B_SOURCES: [&str; SIDE_B_COUNT] = [
    include_str!("../../levels/side-b/b1.txt"),
    include_str!("../../levels/side-b/b2.txt"),
    include_str!("../../levels/side-b/b3.txt"),
    include_str!("../../levels/side-b/b4.txt"),
    include_str!("../../levels/side-b/b5.txt"),
    include_str!("../../levels/side-b/b6.txt"),
    include_str!("../../levels/side-b/b7.txt"),
    include_str!("../../levels/side-b/b8.txt"),
    include_str!("../../levels/side-b/b9.txt"),
    include_str!("../../levels/side-b/b10.txt"),
];

/// A level rewinds this long after Han's death (the needle scratch).
pub const REWIND_SECS: f32 = 2.0;
/// The goal reached: this long on the cheer, then back to the title.
pub const CLEARED_SECS: f32 = 4.0;
/// Cells a frontier search may simulate per step (~0.15 ms each natively, several times that
/// in a browser on a slow laptop, where the music renders on the same thread): a big search
/// spreads over a few frames rather than dropping one.
const SEARCH_BUDGET: usize = 8;

/// Han's size while melting (Nat's melting is only drawn, and less: on Side B he really
/// shrinks, enough to duck under a row of ceiling spikes).
pub const MELT_SIZE: f32 = 0.65;
/// How long a melting call holds (s).
pub const MELT_SECS: f32 = HOLD_SECS;

/// How long a set-list call holds the band: a few bars, so the calls have to be timed (the
/// story's calls hold for 20 s).
pub const HOLD_SECS: f32 = 6.0;
/// Charging this long without getting anywhere: he's done for.
pub const CHARGE_GIVE_UP: f32 = 8.0;

pub const CLEARED_LINE: &str = "Nat's hat!";
/// A free play Side B room cleared: Han tags Nat back in (after this long).
pub const ROOM_CLEARED_LINE: &str = "Tag, Nat!";
pub const ROOM_CLEARED_SECS: f32 = 1.5;

/// How Han met his end (each has its line).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum Fate {
    Spikes,
    Pit,
    Sewage,
    /// Knocked out on a wall he charged at.
    Bonk,
    /// Charged about for ages and got nowhere.
    Pooped,
    /// Flattened by a crusher.
    Squashed,
}

impl Fate {
    pub fn line(self) -> &'static str {
        match self {
            Fate::Spikes => "OW! Pointy!",
            Fate::Pit => "Naaaat!",
            Fate::Sewage => "Glub.",
            Fate::Bonk => "BONK!",
            Fate::Pooped => "Pooped.",
            Fate::Squashed => "Pancake!",
        }
    }
}

/// One entry of a set list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum SetCall {
    /// A band mode (Giant Steps, fired up).
    Mode(Harmony),
    /// The laughing band, held on its melting phrase ([`MELT_SECS`]).
    Melt,
}

/// Ask the band for a set-list call. Plays it at once, charged or not.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallMode(pub SetCall);

/// What a set-list call plays, by its header word.
pub fn call_of(word: &str) -> Option<SetCall> {
    match word {
        "giant" => Some(SetCall::Mode(Harmony::Coltrane)),
        "gap" => Some(SetCall::Mode(Harmony::Quartal)),
        "melt" => Some(SetCall::Melt),
        _ => None,
    }
}

/// Where a call sits on every set list: Giant Steps, fired up, melting.
fn set_list_rank(c: SetCall) -> u8 {
    match c {
        SetCall::Mode(Harmony::Coltrane) => 0,
        SetCall::Mode(Harmony::Quartal) => 1,
        SetCall::Mode(_) => 2,
        SetCall::Melt => 3,
    }
}

/// A Side B level: the level, and its set list.
#[derive(Debug, Clone)]
pub struct SideBLevel {
    pub level: Level,
    pub setlist: Vec<(SetCall, u32)>,
}

impl SideBLevel {
    /// A level file with Side B's own header line (`setlist: giant gap melt`, or `giant=1` to
    /// start with one) on top of the usual format (`level` module docs).
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
                        // `giant` (none to start: Han's nuggets bring them) or `giant=2`.
                        let (word, n) = item.split_once('=').unwrap_or((item, "0"));
                        let h = call_of(word).ok_or_else(|| format!("no such call `{word}`"))?;
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
        // Always in the same order, whatever the header says: the strip mustn't give the
        // solution's order away.
        setlist.sort_by_key(|&(c, _)| set_list_rank(c));
        Ok(SideBLevel { level: Level::parse(&rest)?, setlist })
    }

    /// Level `n`'s name (its `name:` header), without parsing the rest.
    pub fn name(n: usize) -> &'static str {
        SIDE_B_SOURCES[n].lines().find_map(|l| l.strip_prefix("name:")).map_or("???", str::trim)
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
    pub setlist: Vec<(SetCall, u32)>,
    /// Calls left, per set list entry.
    pub left: Vec<u32>,
    pub selected: usize,
    pub phase: Phase,
    /// Where Han's heading (a cell), and whether it's the frontier (no route to the goal).
    pub target: Option<(i32, i32)>,
    pub frontier: bool,
    /// Charging since this long, from this x, with this many landings since.
    pub charging: Option<f32>,
    pub charge_x: f32,
    pub charge_landings: u32,
    pub was_grounded: bool,
    /// How the last try ended.
    pub fate: Option<Fate>,
    pub rewinds: u32,
    pub calls: u32,
    /// Nuggets Han's grabbed this try (each one more call).
    pub refills: u32,
    /// A free play Side B room (`crate::freeplay`) rather than a Side B level: where it is.
    #[reflect(ignore)]
    pub room: Option<AloneRoom>,
}

/// A free play Side B room in the course (cells and columns of the loaded level): Nat waits in
/// its entry pipe while Han runs it, a rewind puts Han back at its start (the rest of the run
/// stays as it is), and Han reaching its goal hands the run back to Nat there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AloneRoom {
    pub start: (usize, usize),
    pub goal: (usize, usize),
    pub cols: (usize, usize),
}

impl SideB {
    /// The band's tune for this level (or room).
    pub fn tune(&self) -> crate::audio::Music {
        crate::audio::Music::side_b(self.level, self.room.is_some())
    }

    pub fn new(level: usize, setlist: Vec<(SetCall, u32)>) -> SideB {
        let left = setlist.iter().map(|s| s.1).collect();
        SideB {
            level,
            setlist,
            left,
            selected: 0,
            phase: Phase::Playing,
            target: None,
            frontier: false,
            charging: None,
            charge_x: 0.0,
            charge_landings: 0,
            was_grounded: true,
            fate: None,
            rewinds: 0,
            calls: 0,
            refills: 0,
            room: None,
        }
    }

    /// Back to the top: the set list refilled.
    fn rewound(&mut self) {
        *self = SideB {
            rewinds: self.rewinds + 1,
            selected: self.selected,
            fate: self.fate,
            room: self.room,
            ..SideB::new(self.level, self.setlist.clone())
        };
    }

    pub fn calls_left(&self) -> u32 {
        self.left.iter().sum()
    }
}

/// Start Side B level `n` (0-based): the run's resources, then [`AppState::Playing`].
pub fn start(commands: &mut Commands, n: usize) {
    start_level(commands, n, SideBLevel::load(n));
}

/// Start `b` as Side B level `n` (tests: a free play Side B room on its own).
pub fn start_level(commands: &mut Commands, n: usize, b: SideBLevel) {
    commands.insert_resource(SideB::new(n, b.setlist));
    commands.insert_resource(GeneratedLevel(b.level));
    commands.insert_resource(super::AssistMode::Manual);
    commands.insert_resource(crate::state::Shelf::SideB);
    commands.queue(|w: &mut World| w.resource_mut::<NextState<AppState>>().set(AppState::Playing));
}

/// Hand free play over to Han for a Side B room (room `k` of the run): Nat waits in the pipe
/// (the beacon, hidden), Han comes through it to the room's start, the band starts the tune
/// over (`audio::plugin`), and the set list starts empty.
pub fn start_room(world: &mut World, k: usize, room: AloneRoom, setlist: Vec<(SetCall, u32)>) {
    world.insert_resource(SideB { room: Some(room), ..SideB::new(k, setlist) });
    world.insert_resource(HanNav::side_b());
    han_to_start(world, room);
}

/// Han back at the room's start, as he came in.
fn han_to_start(world: &mut World, room: AloneRoom) {
    let Some(level) = world.get_resource::<ActiveLevel>().map(|a| a.level.clone()) else { return };
    let at = super::han::spawn_spot(&level, super::stand_pos(&level, room.start.0, room.start.1));
    let mut q = world.query_filtered::<(&mut Pos, &mut PrevPos, &mut Body, &mut HanBrain), With<Han>>();
    if let Ok((mut pos, mut prev, mut body, mut brain)) = q.single_mut(world) {
        pos.0 = at;
        prev.0 = at;
        *body = Body { on_ground: true, ..Body::player() };
        *brain = HanBrain::default();
    }
}

/// A rewind in a free play Side B room: Han back at its start, its nuggets back.
fn rewind_room(world: &mut World, room: AloneRoom) {
    han_to_start(world, room);
    let Some(level) = world.get_resource::<ActiveLevel>().map(|a| a.level.clone()) else { return };
    let (x0, x1) = (room.cols.0 as f32 * TILE, room.cols.1 as f32 * TILE);
    let mut q = world.query_filtered::<(Entity, &Transform), With<Nugget>>();
    let old: Vec<Entity> = q.iter(world).filter(|(_, tf)| (x0..x1).contains(&tf.translation.x)).map(|(e, _)| e).collect();
    for e in old {
        world.despawn(e);
    }
    for t in level.things.iter().filter(|t| t.kind == crate::level::ThingKind::Nugget && (room.cols.0..room.cols.1).contains(&t.col)) {
        world.spawn(super::lifecycle::nugget(&level, t.col, t.row));
    }
}

/// Han's cleared the room: Nat comes out of the pipe where Han is, and the run's his again.
fn end_room(world: &mut World) {
    world.remove_resource::<SideB>();
    world.insert_resource(HanNav::default());
    let mut han = world.query_filtered::<(&Pos, &mut HanBrain), With<Han>>();
    let Ok((at, mut brain)) = han.single_mut(world).map(|(p, b)| (p.0, b)) else { return };
    *brain = HanBrain::default();
    let mut nat = world.query_filtered::<(Entity, &mut Pos, &mut PrevPos, &mut Body, Option<&mut Visibility>), With<Player>>();
    let Ok((e, mut pos, mut prev, mut body, vis)) = nat.single_mut(world) else { return };
    pos.0 = at;
    prev.0 = at;
    *body = Body { on_ground: true, ..Body::player() };
    if let Some(mut vis) = vis {
        *vis = Visibility::Inherited;
    }
    world.entity_mut(e).remove::<Finished>();
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
                (beacon, melt).chain().in_set(GameSet::Prepare).after(super::lifecycle::tick_clocks),
                (han_fate, han_grabs_nuggets).chain().in_set(GameSet::Interact),
            )
                .run_if(resource_exists::<SideB>),
        );
    for s in [AppState::Title, AppState::LevelSelect, AppState::FreePlaySetup] {
        app.add_systems(OnEnter(s), leave);
    }
    app.add_systems(Startup, dev_start);
}

/// Hidden ways in (Side B is a prototype): `NATHAN_SIDE_B=1 cargo run`, or `?sideb=1` on the
/// web, start straight in Side B's level 1 (and the title's Up Up Down Down: `ui::title`).
fn dev_start(mut commands: Commands) {
    if let Some(n) = asked_for() {
        start(&mut commands, n.saturating_sub(1).min(SIDE_B_SOURCES.len() - 1));
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn asked_for() -> Option<usize> {
    std::env::var("NATHAN_SIDE_B").ok()?.parse().ok()
}

#[cfg(target_arch = "wasm32")]
fn asked_for() -> Option<usize> {
    let search = web_sys::window()?.location().search().ok()?;
    search.trim_start_matches('?').split('&').find_map(|kv| kv.strip_prefix("sideb=")?.parse().ok())
}

/// Leaving Side B: the story gets its levels and assists back.
fn leave(mut commands: Commands, side: Option<Res<SideB>>, mut mode: ResMut<super::AssistMode>) {
    if side.is_some() {
        commands.remove_resource::<SideB>();
        commands.remove_resource::<GeneratedLevel>();
        *mode = super::AssistMode::Story;
    }
}

/// Left/Right picks a call on the set list, Down or Jump plays it (Han jumps by himself, and
/// on a phone the jump zone is the big button).
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
    let call = input.just_pressed(&Action::Down) || input.just_pressed(&Action::Jump);
    if call && side.phase == Phase::Playing {
        let k = side.selected;
        if side.left[k] > 0 {
            side.left[k] -= 1;
            side.calls += 1;
            calls.write(CallMode(side.setlist[k].0));
        }
    }
}

fn refill_on_restart(mut commands: Commands, mut restarts: MessageReader<RestartLevel>, mut side: ResMut<SideB>) {
    if restarts.read().count() > 0 {
        side.rewound();
        if let Some(room) = side.room {
            commands.queue(move |w: &mut World| rewind_room(w, room));
        }
    }
}

/// Where Han should head now: the goal, or the frontier. Nat (the beacon) is parked there.
/// At the frontier, Han charges.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn beacon(
    mut commands: Commands,
    time: Res<Time>,
    active: Res<ActiveLevel>,
    groove: Res<Groove>,
    nav: Option<ResMut<HanNav>>,
    mut side: ResMut<SideB>,
    mut nat: Query<(Entity, &mut Pos, &mut Body, &mut PlayerControl, Has<Finished>), (With<Player>, Without<Han>)>,
    mut han: Query<(&Pos, &Body, &mut HanBrain), (With<Han>, Without<Player>)>,
    mut says: MessageWriter<HanSays>,
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
    let Ok((hpos, hbody, mut brain)) = han.single_mut() else { return };
    let goal = side.room.map_or(level.goal, |r| r.goal);
    let goal = (goal.0 as i32, goal.1 as i32);
    let dir = if goal.0 as f32 * TILE + TILE / 2.0 >= hpos.0.x { 1.0 } else { -1.0 };
    // Re-plan from where he stands (mid-air he's carrying out a move already).
    if hbody.on_ground && side.phase == Phase::Playing {
        let map = Map::for_han(level);
        // (Where he stands: by his feet, however small he's melted.)
        let from = nav_cell(level, hpos.0 + Vec2::Y * (HALF.y - hbody.half.y));
        let (route, nearest) = nav.for_groove(&groove).route_or_nearest(&map, from, goal, SEARCH_BUDGET);
        match route {
            crate::level::nav::Route::Found(_) => {
                side.target = Some(goal);
                side.frontier = false;
                side.charging = None;
            }
            crate::level::nav::Route::NoRoute => {
                side.target = Some(nearest);
                side.frontier = true;
            }
            crate::level::nav::Route::Budget => {}
        }
        // Got somewhere since he charged (up the wall, over the gap): that charge is over.
        if side.charging.is_some() && (hpos.0.x - side.charge_x).abs() > 3.0 * TILE {
            side.charging = None;
        }
        // At the frontier: charge!
        let there = side.target.is_some_and(|t| (t.0 - from.0) * dir as i32 <= 1 && t.1 == from.1);
        if side.frontier && there && side.charging.is_none() {
            side.charging = Some(0.0);
            side.charge_x = hpos.0.x;
            side.charge_landings = 0;
            says.write(HanSays { text: charge_line(level, from, dir).to_string() });
        }
    }
    if let Some(t) = &mut side.charging {
        *t += time.delta_secs();
    }
    // He always just goes for it (`han::brain::charge`); the charge at the frontier is when he
    // yells about it, and how long he keeps it up.
    brain.charge = (side.phase == Phase::Playing).then_some(dir);
    if hbody.on_ground && !side.was_grounded && side.charging.is_some() {
        side.charge_landings += 1;
    }
    side.was_grounded = hbody.on_ground;
    let Some(t) = side.target else { return };
    pos.0 = stand(level, t);
    body.vel = Vec2::ZERO;
    body.on_ground = true;
    body.riding = None;
    ctl.facing = dir;
}

/// Han's charge barks (he's nearly silent, as everywhere: `level::is_bark`).
pub const DUCK_LINE: &str = "Duck!";
pub const CLIMB_LINE: &str = "Up we go!";
pub const LEAP_LINE: &str = "HYAAA!";
pub const GO_LINE: &str = "Here goes!";

/// What Han yells as he charges: at the gate right ahead of him, or ceiling spikes, or anything.
pub fn charge_line(level: &Level, at: (i32, i32), dir: f32) -> &'static str {
    let d = if dir < 0.0 { -1 } else { 1 };
    if (1..=4).any(|k| level.tile(at.0 + k * d, at.1) == Tile::SpikesDown) {
        return DUCK_LINE;
    }
    let near = |g: &&crate::level::GateMark| (g.c0 - 3..=g.c1 + 3).contains(&at.0);
    match level.gates.iter().find(near).map(|g| g.topic) {
        Some(Topic::Giant) => CLIMB_LINE,
        Some(Topic::Gap) => LEAP_LINE,
        _ => GO_LINE,
    }
}

/// Melting: Han shrinks (feet planted) while the band's on its melting phrase, and grows
/// back after.
fn melt(time: Res<Time>, groove: Res<Groove>, mut han: Query<(&mut Pos, &mut Body), With<Han>>) {
    let Ok((mut pos, mut body)) = han.single_mut() else { return };
    let size = if groove.nudge() == Some(super::Nudge::Melting) { MELT_SIZE } else { 1.0 };
    let full = Body::player().half;
    let want = full * size;
    let step = full.y * 4.0 * time.delta_secs();
    let new = Vec2::new(
        crate::game::move_towards(body.half.x, want.x, step),
        crate::game::move_towards(body.half.y, want.y, step),
    );
    if new != body.half {
        pos.0.y += new.y - body.half.y;
        body.half = new;
    }
}

/// A nugget Han grabs is one more of the call that's picked on the set list: steer the cursor
/// before he gets there.
fn han_grabs_nuggets(
    mut commands: Commands,
    // (Gone the moment Han clears a free play Side B room: `end_room`.)
    side: Option<ResMut<SideB>>,
    han: Query<(&Pos, &Body), With<Han>>,
    nuggets: Query<(Entity, &Transform), With<super::Nugget>>,
    mut sfx: MessageWriter<crate::events::PlaySfx>,
) {
    let Some(mut side) = side else { return };
    if side.phase != Phase::Playing || side.left.is_empty() {
        return;
    }
    let Ok((pos, body)) = han.single() else { return };
    let h = super::pickups::NUGGET_BOX / 2.0;
    for (e, tf) in &nuggets {
        let c = tf.translation.truncate();
        if (pos.0.x - c.x).abs() < body.half.x + h && (pos.0.y - c.y).abs() < body.half.y + h {
            commands.entity(e).despawn();
            let k = side.selected;
            side.left[k] += 1;
            side.refills += 1;
            sfx.write(crate::events::PlaySfx(crate::audio::Sfx::Nugget));
        }
    }
}

/// Is a box at `pos` (half extents `half`) in a spike tile's deadly half?
fn on_spikes(level: &Level, pos: Vec2, half: Vec2) -> bool {
    let shrink = Vec2::splat(super::FORGIVE);
    let (min, max) = (pos - half + shrink, pos + half - shrink);
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

/// The goal, or how Han met his end; and the rewind or the cheer when their time's up.
#[allow(clippy::too_many_arguments)]
fn han_fate(
    mut commands: Commands,
    time: Res<Time>,
    active: Res<ActiveLevel>,
    clock: Res<SimClock>,
    mut side: ResMut<SideB>,
    mut han: Query<(&Pos, &Body, &mut HanBrain), With<Han>>,
    goal: Query<&Transform, With<Goal>>,
    crushers: Query<(&super::crushers::CrusherBlock, &Pos), Without<Han>>,
    mut says: MessageWriter<HanSays>,
    mut restart: MessageWriter<RestartLevel>,
    mut next: ResMut<NextState<AppState>>,
    progress: Option<ResMut<crate::save::Progress>>,
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
            if t - dt <= 0.0 && side.room.is_some() {
                commands.queue(end_room);
            } else if t - dt <= 0.0 {
                // On to the next Side B level (the strip and the level redraw), or the title
                // after the last.
                let n = side.level + 1;
                if n < SIDE_B_SOURCES.len() {
                    commands.queue(move |w: &mut World| {
                        let b = SideBLevel::load(n);
                        w.insert_resource(SideB::new(n, b.setlist));
                        w.insert_resource(GeneratedLevel(b.level));
                        w.write_message(RestartLevel);
                    });
                } else {
                    next.set(AppState::LevelSelect);
                }
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
    let floor = match side.room {
        Some(r) => Some(super::cell_floor(level, r.goal.0, r.goal.1)),
        None => goal.single().ok().map(|g| g.translation.truncate()),
    };
    if let Some(floor) = floor {
        let (min, max) = (floor - Vec2::new(5.0, 0.0), floor + Vec2::new(5.0, 2.0 * TILE));
        if pos.0.x + HALF.x > min.x && pos.0.x - HALF.x < max.x && pos.0.y + HALF.y > min.y && pos.0.y - HALF.y < max.y {
            if side.room.is_some() {
                side.phase = Phase::Cleared { t: ROOM_CLEARED_SECS };
                say(&mut says, ROOM_CLEARED_LINE);
                return;
            }
            side.phase = Phase::Cleared { t: CLEARED_SECS };
            if let Some(mut progress) = progress {
                progress.record_side_b(side.level, side.rewinds);
            }
            say(&mut says, CLEARED_LINE);
            return;
        }
    }
    let flat = crushers.iter().any(|(b, p)| super::crushers::squashed(pos.0, body.half, b.bounds(p.0)));
    let fate = if flat {
        Some(Fate::Squashed)
    } else if matches!(brain.mode, HanMode::Sinking { .. } | HanMode::Gone { .. }) {
        Some(if pos.0.y + HALF.y < 0.0 { Fate::Pit } else { Fate::Sewage })
    } else if pos.0.y + body.half.y < 0.0 {
        Some(Fate::Pit)
    } else if on_spikes(level, pos.0, body.half) {
        Some(Fate::Spikes)
    } else if side.charge_landings >= 2 && (pos.0.x - side.charge_x).abs() < 2.5 * TILE {
        Some(Fate::Bonk)
    } else if side.charging.is_some_and(|t| t > CHARGE_GIVE_UP) {
        Some(Fate::Pooped)
    } else {
        None
    };
    if let Some(f) = fate {
        if matches!(f, Fate::Spikes | Fate::Bonk | Fate::Pooped | Fate::Squashed) {
            brain.mode = HanMode::Sinking { t: 30.0 };
        }
        brain.charge = None;
        side.fate = Some(f);
        side.phase = Phase::Rewind { t: REWIND_SECS };
        say(&mut says, f.line());
    }
}
