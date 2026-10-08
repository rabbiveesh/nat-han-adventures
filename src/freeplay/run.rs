//! A free-play run in the game: starting it, generating rooms ahead of Nat (off the main thread
//! where there are threads, an attempt at a time), streaming them into the loaded level, the
//! adaptive engine's room events, room restarts, and the end.

use std::collections::VecDeque;
use std::ops::Range;

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, futures::check_ready};
use leafwing_input_manager::prelude::*;

use super::canvas::STAND;
use super::course::{COLS_PER_ROOM, Course, ENDLESS_ROOMS};
use super::dice::{Dice, mix, stream};
use super::generate::{Job, Room, RoomPlan, WORLD_STREAM, generate};
use super::templates::unlocked_skills;
use crate::adapt::frustration::IDLE_SECS;
use crate::adapt::{AdaptEvent, FrustrationSignal, RoomResult, Skill, next_room, reduce};
use crate::events::{Jumped, LevelCompleted, PlayerDied};
use crate::game::{
    ActiveLevel, AdaptiveProfile, AssistMode, Checkpoint, Dead, Fly, GeneratedLevel, Goal, HintSpot, LevelEntity,
    LevelRun, LevelTile, MovingPlatform, Nugget, Player, Pos, RestartLevel, Spray, Stain, cell_floor, spawn_region,
};
use crate::input::Action;
use crate::level::TILE;
use crate::save::Progress;
use crate::state::{AppState, PlayState};

/// Rooms in a fixed-length run.
pub const FIXED_ROOMS: u32 = 8;
/// Columns of a new room spawned per frame (a room is a few hundred tile entities).
pub const SPAWN_COLS_PER_FRAME: usize = 16;
/// Rooms kept loaded behind the one Nat is in; older ones are sealed off and unloaded.
pub const ROOMS_BEHIND: usize = 2;
/// Han's line when a free-play run starts.
pub const INTRO: &str = "Free play! New pipes every time, Nat.";
/// Dice stream for the adaptive engine's room picks.
pub const CHOOSE_STREAM: u64 = 4;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<FreePlaySettings>()
        .add_message::<StartFreePlay>()
        .add_systems(Update, start_run)
        .add_systems(OnEnter(AppState::Title), end_run)
        .add_systems(OnEnter(AppState::LevelSelect), end_run)
        .add_systems(OnEnter(AppState::FreePlaySetup), end_run)
        .add_systems(
            Update,
            (track_rooms, room_stats, restart_room, finish_run, poll_generation, stream_in)
                .chain()
                .run_if(in_state(AppState::Playing).and_then(resource_exists::<FreePlayRun>)),
        );
}

/// Free play's choices, kept for the session.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct FreePlaySettings {
    pub endless: bool,
    /// The seed of the last run (for "replay").
    pub last_seed: Option<u32>,
}

impl Default for FreePlaySettings {
    fn default() -> Self {
        FreePlaySettings { endless: false, last_seed: None }
    }
}

/// Start a run (the setup screen, tests): generates the first room, loads the course and
/// switches to [`AppState::Playing`].
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct StartFreePlay {
    pub seed: u32,
    pub endless: bool,
}

/// A fresh random seed.
pub fn random_seed() -> u32 {
    use rand::Rng;
    rand::rng().random_range(0..=super::dice::SEED_MAX)
}

/// What one room's play looked like so far.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RoomStats {
    /// [`LevelRun::time`] when it started.
    pub started: f32,
    pub deaths: u32,
    pub toots: u32,
    pub first_death: Option<f32>,
    /// Idle seconds since the last death (until input), and the longest.
    pub idle: Option<f32>,
    pub max_idle: f32,
    pub idle_posted: bool,
}

/// The run in progress.
#[derive(Resource, Debug, Clone)]
pub struct FreePlayRun {
    pub seed: u32,
    pub endless: bool,
    pub world: u8,
    pub course: Course,
    /// Every room generated, in course order (`course.rooms[i]` is where `rooms[i]` went).
    pub rooms: Vec<Room>,
    /// The room Nat is in (crossed its checkpoint), if any yet.
    pub current: Option<usize>,
    /// Rooms finished.
    pub cleared: u32,
    /// The goal was reached.
    pub finished: bool,
    pub unlocked: Vec<Skill>,
    pub stats: RoomStats,
    /// Every adaptive event posted, in order (tests, the debug dump).
    pub events: Vec<AdaptEvent>,
    room_id_base: u64,
    /// Rooms planned but not yet generated (at most one).
    planned: Option<RoomPlan>,
    spawn_queue: VecDeque<Range<usize>>,
}

impl FreePlayRun {
    /// Rooms in the run (`None`: endless).
    pub fn total(&self) -> Option<u32> {
        (!self.endless).then_some(FIXED_ROOMS)
    }

    /// The HUD's room counter ("ROOM 3/8", "ROOM 12"). Never a difficulty.
    pub fn room_label(&self) -> String {
        let n = self.current.map_or(1, |k| k + 1);
        match self.total() {
            Some(t) => format!("ROOM {n}/{t}"),
            None => format!("ROOM {n}"),
        }
    }

    pub fn seed_text(&self) -> String {
        super::dice::seed_text(self.seed)
    }

    fn room_id(&self, k: usize) -> u64 {
        self.room_id_base + k as u64
    }

    /// Room `index` is the run's last.
    fn is_last(&self, index: usize) -> bool {
        match self.total() {
            Some(t) => index + 1 >= t as usize,
            None => index + 1 >= ENDLESS_ROOMS,
        }
    }

    fn seen(&self, skill: Skill) -> bool {
        self.rooms.iter().any(|r| r.plan.request.skill == skill)
    }

    /// The plan for room `index`, from the profile as it is now.
    fn plan(&self, profile: &crate::adapt::PlayerProfile, index: usize) -> RoomPlan {
        let mut d = Dice::new(stream(self.seed, CHOOSE_STREAM, index as u64));
        let request = next_room(profile, &self.unlocked, d.rng());
        RoomPlan::new(self.seed, index as u32, request, self.world, self.seen(request.skill), profile.calibrating())
    }
}

/// The generation in flight: one attempt per task.
#[derive(Resource)]
struct GenTask(Task<(Job, Option<Room>)>);

fn spawn_attempt(commands: &mut Commands, mut job: Job) {
    let task = AsyncComputeTaskPool::get().spawn(async move {
        let room = job.step();
        (job, room)
    });
    commands.insert_resource(GenTask(task));
}

/// The run's world (tiles, backdrop, music), by seed.
pub fn world_for(seed: u32) -> u8 {
    1 + (stream(seed, WORLD_STREAM, 0) % 5) as u8
}

/// Set up a run: its first room generated and stitched in. Returns the run and the level to load.
pub fn begin(seed: u32, endless: bool, profile: &mut crate::adapt::PlayerProfile, unlocked_levels: usize) -> (FreePlayRun, crate::level::Level) {
    let world = world_for(seed);
    let rooms = if endless { ENDLESS_ROOMS } else { FIXED_ROOMS as usize };
    let (course, mut level) = Course::new(world, rooms);
    level.intro = INTRO.to_string();
    let unlocked = unlocked_skills(unlocked_levels);
    let mut run = FreePlayRun {
        seed,
        endless,
        world,
        course,
        rooms: Vec::new(),
        current: None,
        cleared: 0,
        finished: false,
        unlocked: unlocked.clone(),
        stats: RoomStats::default(),
        events: Vec::new(),
        room_id_base: mix(seed as u64 ^ (profile.rooms_played as u64) << 32) & !0xFFFF,
        planned: None,
        spawn_queue: VecDeque::new(),
    };
    for s in unlocked {
        post(&mut run, profile, AdaptEvent::SkillUnlocked(s));
    }
    let room = generate(seed, run.plan(profile, 0));
    let last = run.is_last(0);
    run.course.add(&mut level, &room.level, last);
    run.rooms.push(room);
    (run, level)
}

fn post(run: &mut FreePlayRun, profile: &mut crate::adapt::PlayerProfile, event: AdaptEvent) {
    run.events.push(event.clone());
    *profile = reduce(std::mem::take(profile), event);
}

#[allow(clippy::too_many_arguments)]
fn start_run(
    mut commands: Commands,
    mut requests: MessageReader<StartFreePlay>,
    mut profile: ResMut<AdaptiveProfile>,
    progress: Res<Progress>,
    mut settings: ResMut<FreePlaySettings>,
    mut mode: ResMut<AssistMode>,
    mut next: ResMut<NextState<AppState>>,
) {
    let Some(req) = requests.read().last().copied() else { return };
    let (run, level) = begin(req.seed, req.endless, &mut profile.0, progress.unlocked);
    settings.last_seed = Some(req.seed);
    settings.endless = req.endless;
    if *mode != AssistMode::Manual {
        *mode = AssistMode::FreePlay;
    }
    commands.insert_resource(GeneratedLevel(level));
    commands.insert_resource(run);
    commands.remove_resource::<GenTask>();
    next.set(AppState::Playing);
}

/// Leaving free play: story mode gets its level source and its assists back.
fn end_run(mut commands: Commands, run: Option<Res<FreePlayRun>>, mut mode: ResMut<AssistMode>) {
    if run.is_none() {
        return;
    }
    commands.remove_resource::<FreePlayRun>();
    commands.remove_resource::<GeneratedLevel>();
    commands.remove_resource::<GenTask>();
    if *mode == AssistMode::FreePlay {
        *mode = AssistMode::Story;
    }
}

fn result_of(stats: &RoomStats, now: f32) -> RoomResult {
    RoomResult {
        deaths: stats.deaths,
        time_secs: (now - stats.started).max(0.0),
        toots: stats.toots,
        idle_after_death_secs: stats.max_idle,
        first_death_secs: stats.first_death,
    }
}

type Static = Or<(With<LevelTile>, With<Nugget>, With<Checkpoint>, With<Fly>, With<Spray>, With<Stain>, With<MovingPlatform>)>;

/// Nat crossed into a new room: the last one is finished, this one starts, the one after it is
/// planned, and rooms far behind are sealed off and unloaded.
#[allow(clippy::too_many_arguments)]
fn track_rooms(
    mut commands: Commands,
    mut run: ResMut<FreePlayRun>,
    mut profile: ResMut<AdaptiveProfile>,
    mut active: ResMut<ActiveLevel>,
    level_run: Res<LevelRun>,
    player: Query<&Pos, With<Player>>,
    old: Query<(Entity, &Transform), (With<LevelEntity>, Static)>,
    hints: Query<(Entity, &HintSpot)>,
) {
    let Ok(pos) = player.single() else { return };
    let col = (pos.0.x / TILE).floor().max(0.0) as usize;
    let next = run.current.map_or(0, |k| k + 1);
    let Some(room) = run.course.rooms.get(next).copied() else { return };
    if col < room.start_col() {
        return;
    }
    let now = level_run.time;
    let run = &mut *run;
    if run.current.is_some() {
        let result = result_of(&run.stats, now);
        post(run, &mut profile.0, AdaptEvent::RoomFinished(result));
        run.cleared += 1;
    }
    run.current = Some(next);
    run.stats = RoomStats { started: now, ..default() };
    let r = &run.rooms[next];
    let started = AdaptEvent::RoomStarted {
        room_id: run.room_id(next),
        skills: vec![(r.plan.request.skill, r.plan.request.band)],
        expected_deaths: r.expected_deaths,
        par_secs: Some(r.par_secs),
    };
    post(run, &mut profile.0, started);
    if !run.is_last(next) && run.planned.is_none() && run.rooms.len() == next + 1 {
        let plan = run.plan(&profile.0, next + 1);
        spawn_attempt(&mut commands, Job::new(run.seed, plan.clone()));
        run.planned = Some(plan);
    }
    // Seal the pipe behind the rooms still loaded, and unload everything before it.
    if next > ROOMS_BEHIND {
        let keep = next - ROOMS_BEHIND;
        let level = &mut active.bypass_change_detection().level;
        for (c, r) in run.course.seal(level, keep) {
            spawn_tile(&mut commands, level, c, r);
        }
        let cut = run.course.rooms[keep].col0 as f32 * TILE;
        for (e, tf) in &old {
            if tf.translation.x < cut {
                commands.entity(e).despawn();
            }
        }
        for (e, h) in &hints {
            if h.center.x < cut {
                commands.entity(e).despawn();
            }
        }
    }
}

fn spawn_tile(commands: &mut Commands, level: &crate::level::Level, col: usize, row: usize) {
    let tile = level.tile(col as i32, row as i32);
    let top = !level.tile(col as i32, row as i32 - 1).is_solid();
    commands.spawn((
        Name::new("Tile"),
        LevelEntity,
        LevelTile { col, row, tile, top, world: level.world },
        Transform::from_translation(level.tile_center(col, row).extend(0.0)),
    ));
}

/// Deaths, toots and idling for the room in progress; idling too long after a death is a
/// frustration signal, posted the moment it happens.
fn room_stats(
    time: Res<Time<Real>>,
    play: Res<State<PlayState>>,
    mut run: ResMut<FreePlayRun>,
    mut profile: ResMut<AdaptiveProfile>,
    level_run: Res<LevelRun>,
    input: Option<Single<&ActionState<Action>>>,
    mut died: MessageReader<PlayerDied>,
    mut jumped: MessageReader<Jumped>,
) {
    let run = &mut *run;
    let deaths = died.read().count() as u32;
    let toots = jumped.read().filter(|j| j.double).count() as u32;
    if run.current.is_none() {
        return;
    }
    let st = &mut run.stats;
    st.toots += toots;
    if deaths > 0 {
        st.deaths += deaths;
        st.first_death.get_or_insert(level_run.time - st.started);
        st.idle = Some(-crate::game::tuning::RESPAWN_DELAY);
    }
    if *play.get() != PlayState::Running {
        return;
    }
    let Some(idle) = st.idle else { return };
    if input.is_some_and(|i| !i.get_pressed().is_empty()) && idle > 0.0 {
        st.idle = None;
        return;
    }
    let idle = idle + time.delta_secs();
    st.idle = Some(idle);
    st.max_idle = st.max_idle.max(idle);
    if idle > IDLE_SECS && !st.idle_posted {
        st.idle_posted = true;
        post(run, &mut profile.0, AdaptEvent::Frustrated(FrustrationSignal::IdleAfterDeath));
    }
}

/// R / the pause menu's RESTART in free play: back to the room's checkpoint (a restart of the
/// room for the adaptive engine), not the whole run.
fn restart_room(
    mut commands: Commands,
    mut requests: MessageReader<RestartLevel>,
    mut run: ResMut<FreePlayRun>,
    mut profile: ResMut<AdaptiveProfile>,
    player: Query<(Entity, Has<Dead>), With<Player>>,
) {
    if requests.read().count() == 0 {
        return;
    }
    if let Ok((e, dead)) = player.single()
        && !dead
    {
        commands.entity(e).insert(Dead { remaining: 0.0 });
    }
    let run = &mut *run;
    let Some(k) = run.current else { return };
    let r = &run.rooms[k];
    let again = AdaptEvent::RoomStarted {
        room_id: run.room_id(k),
        skills: vec![(r.plan.request.skill, r.plan.request.band)],
        expected_deaths: r.expected_deaths,
        par_secs: Some(r.par_secs),
    };
    post(run, &mut profile.0, again);
}

/// The goal flag: the last room is finished.
fn finish_run(
    mut completed: MessageReader<LevelCompleted>,
    mut run: ResMut<FreePlayRun>,
    mut profile: ResMut<AdaptiveProfile>,
    level_run: Res<LevelRun>,
) {
    if completed.read().count() == 0 || run.finished {
        return;
    }
    let run = &mut *run;
    run.finished = true;
    if run.current.is_some() {
        let result = result_of(&run.stats, level_run.time);
        post(run, &mut profile.0, AdaptEvent::RoomFinished(result));
        run.cleared += 1;
    }
}

/// A finished attempt: stitch the room in (or try again).
#[allow(clippy::too_many_arguments)]
fn poll_generation(
    mut commands: Commands,
    task: Option<ResMut<GenTask>>,
    mut run: ResMut<FreePlayRun>,
    mut active: ResMut<ActiveLevel>,
    mut level_run: ResMut<LevelRun>,
    tiles: Query<(Entity, &LevelTile)>,
    mut goal: Query<&mut Transform, With<Goal>>,
) {
    let Some(mut task) = task else { return };
    let Some((job, room)) = check_ready(&mut task.0) else { return };
    commands.remove_resource::<GenTask>();
    let Some(room) = room else {
        spawn_attempt(&mut commands, job);
        return;
    };
    let run = &mut *run;
    run.planned = None;
    let index = run.rooms.len();
    let level = &mut active.bypass_change_detection().level;
    let last = run.is_last(index) || !run.course.fits(level, room.level.width + COLS_PER_ROOM);
    let (placed, uncapped) = run.course.add(level, &room.level, last);
    for (e, t) in &tiles {
        if uncapped.contains(&(t.col, t.row)) {
            commands.entity(e).despawn();
        }
    }
    if let Some(cap) = run.course.cap {
        for r in super::canvas::PIPE_ROOF + 1..=STAND {
            spawn_tile(&mut commands, level, cap, r);
        }
    }
    if last {
        for mut tf in &mut goal {
            let at = cell_floor(level, level.goal.0, level.goal.1);
            tf.translation.x = at.x;
            tf.translation.y = at.y;
        }
    }
    level_run.nuggets_total += room.level.nugget_count() as u32;
    let cols = placed.cols();
    let mut c = cols.start;
    while c < cols.end {
        let e = (c + SPAWN_COLS_PER_FRAME).min(cols.end);
        run.spawn_queue.push_back(c..e);
        c = e;
    }
    info!(
        "free play: room {} ({}, band {}) ready in {} attempt(s), {:.1} ms{}",
        index + 1,
        room.plan.template().name,
        room.plan.request.band,
        room.attempts,
        room.micros as f64 / 1000.0,
        if room.fallback { " (fallback)" } else { "" }
    );
    run.rooms.push(room);
}

/// Spawn the newest room a few columns per frame.
fn stream_in(mut commands: Commands, mut run: ResMut<FreePlayRun>, active: Res<ActiveLevel>) {
    if let Some(cols) = run.spawn_queue.pop_front() {
        spawn_region(&mut commands, &active.level, cols);
    }
}
