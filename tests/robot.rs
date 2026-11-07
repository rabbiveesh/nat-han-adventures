//! The robot pit (`game::robot`), played headless: a `robotpit:` gap drops Nat into Sparky's
//! game instead of splatting him; he walks top-down, answers on a pad (a wrong one only drains
//! Sparky's battery), and the whirlpool flushes him back to the ledge before the pit.

use std::time::Duration;

use bevy::{input::InputPlugin, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use leafwing_input_manager::prelude::*;
use nat_han_adventures::{
    events::{HanSays, PlayerDied},
    game::robot::{AnswerPad, BACK_LINE, BATTERY, RobotRoom, RoomPhase, TopDown, Whirlpool},
    game::{LevelRun, Player, Pos},
    level::{LEVEL_COUNT, Level, Levels, TILE},
    state::{AppState, CurrentLevel},
};

const DT: f64 = 1.0 / 60.0;

const PIT: &str = "name: Pit
robotpit: 8 9
---
....................
....................
....................
....................
.P................G.
########..##########
";

/// A plain gap, no robot in it.
const PLAIN: &str = "name: Plain
---
....................
....................
....................
....................
.P................G.
########..##########
";

fn app(src: &str) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin, InputPlugin))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(DT)))
        .add_plugins(nat_han_adventures::gameplay)
        .insert_resource(Levels(vec![Level::parse(src).expect("test level parses"); LEVEL_COUNT]))
        .insert_resource(CurrentLevel(0))
        .insert_resource(Said::default())
        .insert_resource(Died::default())
        .add_systems(Last, |mut r: MessageReader<HanSays>, mut s: ResMut<Said>| s.0.extend(r.read().map(|m| m.text.clone())))
        .add_systems(Last, |mut r: MessageReader<PlayerDied>, mut d: ResMut<Died>| d.0 += r.read().count());
    app.finish();
    app.cleanup();
    app.update();
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::Playing);
    app.update();
    app.update();
    app
}

#[derive(Resource, Default)]
struct Said(Vec<String>);

#[derive(Resource, Default)]
struct Died(usize);

fn step(app: &mut App, secs: f32) {
    for _ in 0..(secs as f64 / DT).round() as usize {
        app.update();
    }
}

fn room(app: &App) -> Option<RobotRoom> {
    app.world().get_resource::<RobotRoom>().cloned()
}

fn nat(app: &mut App) -> Vec2 {
    app.world_mut().query_filtered::<&Pos, With<Player>>().single(app.world()).unwrap().0
}

/// Walks Nat (top-down) to `to`: along x first, then y. Paths in the room are clear.
fn walk_to(app: &mut App, to: Vec2) {
    for axis in [0, 1] {
        let gap = |app: &mut App| {
            let d = to - nat(app);
            if axis == 0 { d.x } else { d.y }
        };
        let d = gap(app);
        if d.abs() < 1.0 {
            continue;
        }
        let key = match (axis, d > 0.0) {
            (0, true) => KeyCode::ArrowRight,
            (0, false) => KeyCode::ArrowLeft,
            (_, true) => KeyCode::ArrowUp,
            (_, false) => KeyCode::ArrowDown,
        };
        key.press(app.world_mut());
        for _ in 0..600 {
            app.update();
            if gap(app).abs() < 1.0 || gap(app).signum() != d.signum() {
                break;
            }
        }
        key.release(app.world_mut());
        app.update();
    }
}

/// Walks Nat through room cells (each a straight line from the last).
fn go(app: &mut App, cells: &[(usize, usize)]) {
    for &(c, r) in cells {
        let to = room(app).unwrap().cell_center(c, r);
        walk_to(app, to);
    }
}

/// The room cell of the pad showing `value`.
fn pad_cell(app: &mut App, value: u32) -> (usize, usize) {
    let mut q = app.world_mut().query::<(&AnswerPad, &Transform)>();
    let at = q.iter(app.world()).find(|(p, _)| p.value == value).map(|(_, t)| t.translation.truncate()).expect("that pad");
    let (c, r) = room(app).unwrap().cell_at(at);
    (c as usize, r as usize)
}

/// Down the row between Nat's landing and the pads, to a pad.
fn to_pad(app: &mut App, value: u32) {
    let (c, r) = pad_cell(app, value);
    let here = room(app).unwrap().cell_at(nat(app));
    go(app, &[(here.0 as usize, r - 1), (c, r - 1), (c, r)]);
}

/// From a pad, round the pond to the whirlpool (open on the right, past the pond's corner).
fn to_pool(app: &mut App) {
    let mut q = app.world_mut().query::<(&Whirlpool, &Transform)>();
    let at = q.single(app.world()).unwrap().1.translation.truncate();
    let (c, r) = room(app).unwrap().cell_at(at);
    let (c, r) = (c as usize, r as usize);
    let here = room(app).unwrap().cell_at(nat(app));
    go(app, &[(here.0 as usize, r + 2), (c + 2, r + 2), (c + 2, r), (c, r)]);
}

/// Runs Nat off the ledge into the pit, and on into the room till the question's up.
fn fall_in(app: &mut App) {
    KeyCode::ArrowRight.press(app.world_mut());
    for _ in 0..240 {
        app.update();
        if room(app).is_some() {
            break;
        }
    }
    KeyCode::ArrowRight.release(app.world_mut());
    assert!(room(app).is_some(), "fell into the wrong game");
    for _ in 0..1200 {
        if room(app).unwrap().phase == RoomPhase::Ask {
            return;
        }
        app.update();
    }
    panic!("the question never came: {:?}", room(app).map(|r| r.phase));
}

#[test]
fn a_plain_gap_still_splats() {
    let mut app = app(PLAIN);
    KeyCode::ArrowRight.press(app.world_mut());
    step(&mut app, 2.5);
    assert!(app.world().resource::<Died>().0 >= 1);
    assert!(room(&app).is_none());
}

#[test]
fn the_robot_pit_is_a_round_trip_not_a_death() {
    let mut app = app(PIT);
    fall_in(&mut app);
    let r = room(&app).unwrap();
    assert_eq!(r.visit, 1);
    // Nat's in the room, far below the level, walking top-down.
    let p = nat(&mut app);
    assert!(p.y < -10.0 * TILE, "down in the room: {p}");
    assert!(app.world_mut().query_filtered::<(), (With<Player>, With<TopDown>)>().iter(app.world()).count() == 1);

    // A wrong pad: Sparky's battery drains, the pad greys out, the question comes back.
    let q = r.question();
    let wrong = *q.pads.iter().find(|&&v| v != q.answer).unwrap();
    to_pad(&mut app, wrong);
    let r = room(&app).unwrap();
    assert_eq!(r.phase, RoomPhase::Wrong);
    assert_eq!(r.battery, BATTERY - 1);
    step(&mut app, 2.5);
    assert_eq!(room(&app).unwrap().phase, RoomPhase::Ask);
    // Standing on it again does nothing.
    step(&mut app, 0.2);
    assert_eq!(room(&app).unwrap().battery, BATTERY - 1);

    // The right one opens the whirlpool home.
    to_pad(&mut app, q.answer);
    assert_eq!(room(&app).unwrap().phase, RoomPhase::Open);
    to_pool(&mut app);
    assert_eq!(room(&app).unwrap().phase, RoomPhase::Flush);
    step(&mut app, 2.0);

    // Back on the ledge before the pit, no death counted; Han wants to know.
    assert!(room(&app).is_none());
    let p = nat(&mut app);
    assert!(p.x < 8.0 * TILE && p.y > 0.0, "back on the ledge: {p}");
    assert_eq!(app.world().resource::<Died>().0, 0);
    assert_eq!(app.world().resource::<LevelRun>().deaths, 0);
    assert!(app.world().resource::<Said>().0.iter().any(|s| s == BACK_LINE));
    assert!(app.world_mut().query_filtered::<(), With<AnswerPad>>().iter(app.world()).next().is_none(), "the room's gone");
    // And the game plays on: across the gap to the goal is still the way.
    step(&mut app, 0.5);
    assert_eq!(*app.world().resource::<State<AppState>>().get(), AppState::Playing);
}

#[test]
fn a_flat_battery_still_lets_nat_home() {
    let mut app = app(PIT);
    fall_in(&mut app);
    let q = room(&app).unwrap().question();
    let wrongs: Vec<u32> = q.pads.iter().copied().filter(|&v| v != q.answer).collect();
    for (i, v) in wrongs.iter().enumerate() {
        to_pad(&mut app, *v);
        if i + 1 < wrongs.len() {
            step(&mut app, 2.5);
        }
    }
    // Both wrong: Sparky's flat, and the whirlpool opens anyway (no prize). Never stranded.
    let r = room(&app).unwrap();
    assert_eq!((r.phase, r.battery), (RoomPhase::Open, 0));
    to_pool(&mut app);
    step(&mut app, 2.0);
    assert!(room(&app).is_none());
}

#[test]
fn the_next_visit_asks_something_else() {
    let mut app = app(PIT);
    fall_in(&mut app);
    let first = room(&app).unwrap().question();
    app.world_mut().write_message(nat_han_adventures::game::RestartLevel);
    step(&mut app, 0.5);
    assert!(room(&app).is_none(), "a restart leaves the wrong game");
    fall_in(&mut app);
    let r = room(&app).unwrap();
    assert_eq!(r.visit, 2);
    assert_ne!(r.question(), first);
}
