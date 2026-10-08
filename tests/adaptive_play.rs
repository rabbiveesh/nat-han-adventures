//! The adaptive engine wired into play (headless, like `tests/gameplay.rs`): story assists
//! rise with deaths beyond a level's expectation, fade with clean checkpoints, never go below
//! the floors; the levers reach physics and hazards; the band's freedom is decided on time.

use std::time::Duration;

use bevy::{input::InputPlugin, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use leafwing_input_manager::prelude::*;
use nat_han_adventures::{
    adapt::StoryAssist,
    events::*,
    game::{
        AssistMode, Assists, Body, Dead, HiddenRespawn, Player, Pos, PrevPos, RaftLife, StoryAssistState,
        adaptive::ENCOURAGE_LINES,
    },
    level::{Level, Levels, TILE},
    state::{AppState, CurrentLevel},
};

const DT: f64 = 1.0 / 60.0;
const JUMP: KeyCode = KeyCode::KeyZ;
const RIGHT: KeyCode = KeyCode::ArrowRight;

#[derive(Resource, Default)]
struct Heard {
    says: Vec<String>,
    freedom: usize,
    deaths: usize,
    respawns: Vec<Vec2>,
    toots: usize,
    ground_jumps: usize,
}

fn listen(
    mut heard: ResMut<Heard>,
    mut says: MessageReader<HanSays>,
    mut freedom: MessageReader<BandFreedom>,
    mut died: MessageReader<PlayerDied>,
    mut respawned: MessageReader<PlayerRespawned>,
    mut jumped: MessageReader<Jumped>,
) {
    heard.says.extend(says.read().map(|s| s.text.clone()));
    heard.freedom += freedom.read().count();
    heard.deaths += died.read().count();
    heard.respawns.extend(respawned.read().map(|r| r.pos));
    for j in jumped.read() {
        if j.double {
            heard.toots += 1;
        } else {
            heard.ground_jumps += 1;
        }
    }
}

fn app_with(levels: &[&str], setup: impl FnOnce(&mut World)) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin, InputPlugin))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(DT)))
        .add_plugins(nat_han_adventures::gameplay)
        .insert_resource(Levels(levels.iter().map(|l| Level::parse(l).expect("test level parses")).collect()))
        .init_resource::<Heard>()
        .add_systems(Last, listen);
    app.finish();
    app.cleanup();
    app.update();
    setup(app.world_mut());
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::Playing);
    app.update();
    app.update();
    app
}

fn app(level: &str) -> App {
    app_with(&[level], |_| {})
}

fn step(app: &mut App, secs: f32) {
    for _ in 0..(secs as f64 / DT).round() as usize {
        app.update();
    }
}

fn player(app: &mut App) -> Entity {
    app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap()
}

fn dial(app: &App) -> f32 {
    app.world().resource::<StoryAssistState>().0.assists
}

fn assists(app: &App) -> Assists {
    *app.world().resource::<Assists>()
}

/// Update until `done` (at most a simulated minute).
fn until(app: &mut App, mut done: impl FnMut(&App) -> bool) {
    for _ in 0..3600 {
        if done(app) {
            return;
        }
        app.update();
    }
    panic!("timed out");
}

fn heard(app: &App) -> &Heard {
    app.world().resource::<Heard>()
}

fn assert_above_floors(a: Assists) {
    let floor = Assists::default();
    assert!(a.coyote_mult >= floor.coyote_mult, "{a:?}");
    assert!(a.jump_buffer_mult >= floor.jump_buffer_mult, "{a:?}");
    assert!(a.hitbox_forgiveness_px >= floor.hitbox_forgiveness_px, "{a:?}");
    assert!(a.raft_life_mult >= floor.raft_life_mult, "{a:?}");
}

/// Walk right into a bottomless pit; every respawn walks right into it again (no stains).
const SPIKY: &str = "name: Pit
deaths: 0
---
........................
........................
..P....................G
#########.....##########
";

/// A flat run through six checkpoints.
const CHECKPOINTS: &str = "name: Checkpoints
---
..............................................
..............................................
..P....C....C....C....C....C....C............G
##############################################
";

#[test]
fn deaths_raise_assists_and_clean_checkpoints_fade_them() {
    let mut app = app_with(&[SPIKY, CHECKPOINTS], |_| {});
    assert_eq!(assists(&app), Assists::default());
    RIGHT.press(app.world_mut());
    let mut last = 0.0;
    for _ in 0..3600 {
        if heard(&app).deaths >= 6 {
            break;
        }
        app.update();
        assert!(dial(&app) >= last, "the dial only rises while dying");
        last = dial(&app);
        assert_above_floors(assists(&app));
    }
    app.update();
    let a = assists(&app);
    // 6 excess deaths × 0.08, plus one frustration (3 excess) × 0.15.
    assert!((dial(&app) - 0.63).abs() < 1e-4, "{}", dial(&app));
    assert!((a.coyote_mult - 1.63).abs() < 1e-4 && (a.jump_buffer_mult - (1.0 + 0.75 * 0.63)).abs() < 1e-4, "{a:?}");
    assert_eq!(a.hitbox_forgiveness_px, 2.0);
    assert!((a.raft_life_mult - 1.63).abs() < 1e-4);
    assert!(a.han_eagerness > 0.8, "{a:?}");
    assert_eq!(app.world().resource::<RaftLife>().scale, a.raft_life_mult);
    let encouraged: Vec<_> = heard(&app).says.iter().filter(|s| ENCOURAGE_LINES.contains(&s.as_str())).collect();
    assert_eq!(encouraged.len(), 1, "Han encourages once per segment: {:?}", heard(&app).says);

    // Next level: a clean run fades the dial at every checkpoint (0.12 each) down to nothing.
    RIGHT.release(app.world_mut());
    app.world_mut().resource_mut::<CurrentLevel>().0 = 1;
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::LevelSelect);
    app.update();
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::Playing);
    app.update();
    app.update();
    RIGHT.press(app.world_mut());
    let mut last = dial(&app);
    for _ in 0..600 {
        app.update();
        assert!(dial(&app) <= last, "clean play only lowers the dial");
        last = dial(&app);
        assert_above_floors(assists(&app));
    }
    assert_eq!(heard(&app).deaths, 6);
    assert_eq!(dial(&app), 0.0);
    let a = assists(&app);
    assert_eq!(Assists { han_eagerness: 0.5, ..a }, Assists::default());
    assert!(a.han_eagerness < 0.5, "cruising: Han is lazier ({a:?})");
}

const STAINS: &str = "name: Stains
deaths: 3
---
........................
........................
..P....................G
#########.....##########
";

#[test]
fn expected_deaths_never_count_against_you() {
    let mut app = app(STAINS);
    RIGHT.press(app.world_mut());
    until(&mut app, |a| heard(a).deaths >= 3);
    app.update();
    assert_eq!(dial(&app), 0.0);
    assert_eq!(assists(&app), Assists::default());
    until(&mut app, |a| heard(a).deaths >= 4);
    app.update();
    assert!((dial(&app) - 0.08).abs() < 1e-5);
}

#[test]
fn idling_after_a_death_gets_encouragement() {
    let mut app = app(SPIKY);
    RIGHT.press(app.world_mut());
    until(&mut app, |a| heard(a).deaths >= 1);
    RIGHT.release(app.world_mut());
    step(&mut app, 14.0);
    assert!((dial(&app) - 0.08).abs() < 1e-5);
    step(&mut app, 2.0);
    assert!((dial(&app) - 0.23).abs() < 1e-5, "{}", dial(&app));
    assert!(heard(&app).says.iter().any(|s| ENCOURAGE_LINES.contains(&s.as_str())));
}

const FORGIVE: &str = "name: Forgive
---
..........
..........
..P...^...G
##########
";

/// Nat stands still with his box 1.5 px into a spike tile (counting the normal forgiveness).
fn stand_near_spike(px: f32) -> bool {
    let mut app = app_with(&[FORGIVE], |w| {
        w.insert_resource(AssistMode::Manual);
        w.insert_resource(Assists { hitbox_forgiveness_px: px, ..default() });
    });
    step(&mut app, 0.5);
    let p = player(&mut app);
    let at = Vec2::new(6.0 * TILE - 2.5, app.world().get::<Pos>(p).unwrap().0.y);
    app.world_mut().get_mut::<Pos>(p).unwrap().0 = at;
    app.world_mut().get_mut::<PrevPos>(p).unwrap().0 = at;
    step(&mut app, 0.5);
    heard(&app).deaths > 0
}

#[test]
fn hazards_forgive_by_the_configured_px() {
    assert!(stand_near_spike(0.0), "1.5 px into the spikes kills");
    assert!(stand_near_spike(1.0), "1 px of forgiveness isn't enough");
    assert!(!stand_near_spike(2.0), "2 px forgives it");
    assert!(!stand_near_spike(3.0));
    assert!(stand_near_spike(-5.0), "negative forgiveness is ignored (never below the floor)");
}

const CAN: &str = "name: Can
---
..........
..........
..P...S...G
##########
";

/// Nat stands still beside a firing spray can (all cans fire from 1.5 s to 2.5 s), his box
/// 1.5 px into the can's body (counting the normal forgiveness), under its jet.
fn stand_near_can(px: f32) -> bool {
    let mut app = app_with(&[CAN], |w| {
        w.insert_resource(AssistMode::Manual);
        w.insert_resource(Assists { hitbox_forgiveness_px: px, ..default() });
    });
    step(&mut app, 1.6);
    assert_eq!(heard(&app).deaths, 0);
    let p = player(&mut app);
    // The can's body starts at 6 tiles + (16 - CAN_WIDTH) / 2 px.
    let can_left = 6.0 * TILE + (TILE - nat_han_adventures::game::CAN_WIDTH) / 2.0;
    let half = nat_han_adventures::game::tuning::PLAYER_SIZE.0 / 2.0;
    let at = Vec2::new(can_left + 1.5 - (half - nat_han_adventures::game::FORGIVE), app.world().get::<Pos>(p).unwrap().0.y);
    app.world_mut().get_mut::<Pos>(p).unwrap().0 = at;
    app.world_mut().get_mut::<PrevPos>(p).unwrap().0 = at;
    step(&mut app, 0.5);
    heard(&app).deaths > 0
}

#[test]
fn spray_cans_forgive_by_the_configured_px() {
    assert!(stand_near_can(0.0), "1.5 px into a firing can kills");
    assert!(stand_near_can(1.0), "1 px of forgiveness isn't enough");
    assert!(!stand_near_can(2.0), "2 px forgives it");
    assert!(stand_near_can(-5.0), "negative forgiveness is ignored");
}

const LEDGE: &str = "name: Ledge
---
..............................
..............................
..............................
..............................
.P............................
#####.........................
#####.........................
#####.......................G.
##############################
";

/// Walk off the ledge, press jump `late` seconds after leaving the ground: ground jump?
fn late_jump_is_a_ground_jump(coyote_mult: f32, late: f32) -> bool {
    let mut app = app_with(&[LEDGE], |w| {
        w.insert_resource(AssistMode::Manual);
        w.insert_resource(Assists { coyote_mult, ..default() });
    });
    RIGHT.press(app.world_mut());
    let p = player(&mut app);
    for _ in 0..120 {
        app.update();
        if !app.world().get::<Body>(p).unwrap().on_ground {
            break;
        }
    }
    step(&mut app, late);
    JUMP.press(app.world_mut());
    app.update();
    app.update();
    let h = heard(&app);
    assert_eq!(h.ground_jumps + h.toots, 1);
    h.ground_jumps == 1
}

#[test]
fn coyote_time_scales_with_the_assist() {
    assert!(late_jump_is_a_ground_jump(1.0, 0.05));
    assert!(!late_jump_is_a_ground_jump(1.0, 0.15), "past the normal coyote time: a toot");
    assert!(late_jump_is_a_ground_jump(2.0, 0.15), "doubled coyote time still jumps");
    assert!(!late_jump_is_a_ground_jump(1.5, 0.18), "1.5× coyote time runs out too");
    assert!(!late_jump_is_a_ground_jump(0.5, 0.15), "never below the floor");
    assert!(late_jump_is_a_ground_jump(0.5, 0.05), "never below the floor");
}

#[test]
fn the_band_decides_at_start_deaths_checkpoints_and_every_20s() {
    let mut app = app("name: Music
---
..................................
..................................
..P.....C...........^^^..........G
##################################
");
    app.update();
    assert_eq!(heard(&app).freedom, 1, "level start");
    RIGHT.press(app.world_mut());
    until(&mut app, |a| heard(a).deaths >= 1);
    app.update();
    assert_eq!(heard(&app).freedom, 3, "checkpoint, then the death");
    RIGHT.release(app.world_mut());
    step(&mut app, 19.0);
    assert_eq!(heard(&app).freedom, 3);
    step(&mut app, 2.0);
    assert_eq!(heard(&app).freedom, 4, "the periodic check");
}

/// No checkpoints: the start, a long walk, spikes near the goal.
const LONG: &str = "name: Long
---
....................................................
....................................................
..P.......................................^^^^....G.
####################################################
";

fn respawns_on_long(dial: f32) -> Vec2 {
    let mut app = app_with(&[LONG], |w| w.insert_resource(StoryAssistState(StoryAssist::new(dial))));
    RIGHT.press(app.world_mut());
    until(&mut app, |a| !heard(a).respawns.is_empty());
    heard(&app).respawns[0]
}

#[test]
fn a_high_dial_respawns_at_a_hidden_midway_point() {
    let start = respawns_on_long(0.0);
    assert!(start.x < 3.0 * TILE, "{start:?}");
    let mid = respawns_on_long(0.5);
    // Midway between the start (col 2) and the goal (col 50), clear of the spikes (col 42).
    assert!(mid.x > 25.0 * TILE && mid.x < 40.0 * TILE, "{mid:?}");
    assert_eq!(mid.y, start.y);
}

#[test]
fn hidden_respawn_resets_with_the_level() {
    let mut app = app_with(&[LONG], |w| w.insert_resource(StoryAssistState(StoryAssist::new(0.5))));
    RIGHT.press(app.world_mut());
    step(&mut app, 4.0);
    assert!(app.world().resource::<HiddenRespawn>().pos.is_some());
    app.world_mut().write_message(nat_han_adventures::game::RestartLevel);
    app.update();
    app.update();
    assert!(app.world().resource::<HiddenRespawn>().pos.is_none());
    assert!(app.world().resource::<HiddenRespawn>().armed);
    let p = player(&mut app);
    assert!(app.world().get::<Dead>(p).is_none());
}

const HINTED: &str = "name: Hinted
hint@6,2: Hop the brushes, Nat!
---
........................
........................
..P......^^^^^.........G
########################
";

#[test]
fn han_repeats_the_nearest_hint_after_repeated_deaths() {
    let hint = "Hop the brushes, Nat!";
    let count = |app: &App| heard(app).says.iter().filter(|s| *s == hint).count();
    // Low dial: the hint is said once, on approach.
    let mut app = app(HINTED);
    RIGHT.press(app.world_mut());
    until(&mut app, |a| heard(a).deaths >= 2);
    app.update();
    assert_eq!(count(&app), 1);
    // High dial: repeated deaths at the same spot bring it back (once per segment).
    let mut app = app_with(&[HINTED], |w| w.insert_resource(StoryAssistState(StoryAssist::new(0.6))));
    RIGHT.press(app.world_mut());
    until(&mut app, |a| heard(a).deaths >= 4);
    app.update();
    assert_eq!(count(&app), 2, "{:?}", heard(&app).says);
}
