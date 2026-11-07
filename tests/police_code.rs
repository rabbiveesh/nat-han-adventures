//! The policeman (listen and nothing hurts you; don't, and a spike pops up in your way or a
//! cone comes down on your head) and the building code (the paper, the keypad, the secret
//! room and its mode), headless.

use std::time::Duration;

use bevy::{input::InputPlugin, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use leafwing_input_manager::prelude::*;
use nat_han_adventures::{
    events::{DeathCause, PlayerDied},
    game::{
        Player, Pos,
        code::{CodeDoor, CodeRead, ModeMusic, RoomPlaying},
        police::{Police, PoliceSpike},
    },
    level::{Arrow, Level, Levels, Order, POLICE_GRACE, RoomMode, TILE},
    state::AppState,
};

const DT: f64 = 1.0 / 60.0;
const SPACE: KeyCode = KeyCode::Space;
const RIGHT: KeyCode = KeyCode::ArrowRight;
const LEFT: KeyCode = KeyCode::ArrowLeft;
const UP: KeyCode = KeyCode::ArrowUp;
const DOWN: KeyCode = KeyCode::ArrowDown;

#[derive(Resource, Default)]
struct Seen {
    deaths: Vec<DeathCause>,
    codes: Vec<Vec<Arrow>>,
    modes: Vec<Option<RoomMode>>,
}

fn app(level: &str) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin, InputPlugin))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(DT)))
        .add_plugins(nat_han_adventures::gameplay)
        .insert_resource(Levels(vec![Level::parse(level).expect("test level parses")]))
        .init_resource::<Seen>()
        .add_systems(
            Last,
            |mut s: ResMut<Seen>, mut d: MessageReader<PlayerDied>, mut c: MessageReader<CodeRead>, mut m: MessageReader<ModeMusic>| {
                s.deaths.extend(d.read().map(|m| m.cause));
                s.codes.extend(c.read().map(|c| c.code.clone()));
                s.modes.extend(m.read().map(|m| m.0));
            },
        );
    app.finish();
    app.cleanup();
    app.update();
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::Playing);
    app.update();
    app.update();
    app
}

fn step(app: &mut App, secs: f32) {
    for _ in 0..(secs as f64 / DT).round() as usize {
        app.update();
    }
}

fn nat(app: &mut App) -> Vec2 {
    app.world_mut().query_filtered::<&Pos, With<Player>>().single(app.world()).unwrap().0
}

fn police(app: &mut App) -> Police {
    *app.world_mut().query::<&Police>().iter(app.world()).next().unwrap()
}

fn seen(app: &App) -> &Seen {
    app.world().resource::<Seen>()
}

fn tap(app: &mut App, k: KeyCode) {
    k.press(app.world_mut());
    step(app, 1.0 / 60.0);
    k.release(app.world_mut());
    step(app, 4.0 / 60.0);
}

/// Hold `k` until `until` (or 10 s).
fn hold_until(app: &mut App, k: KeyCode, mut until: impl FnMut(&mut App) -> bool) {
    k.press(app.world_mut());
    for _ in 0..600 {
        step(app, 1.0 / 60.0);
        if until(app) {
            break;
        }
    }
    k.release(app.world_mut());
}

fn wait_for(app: &mut App, mut until: impl FnMut(&mut App) -> bool) {
    for _ in 0..1200 {
        if until(app) {
            return;
        }
        step(app, 1.0 / 60.0);
    }
    panic!("never happened");
}

/// A policeman at col 12, his spikes at cols 15-19.
const BEAT: &str = "name: Beat
solo: nat
---
..............................................
..............................................
..P.........Q..yyyyy..........................G
##############################################
";

/// Walk up until he's on duty, and stop.
fn walk_up(app: &mut App) {
    step(app, 0.3);
    hold_until(app, RIGHT, |a| police(a).duty.is_some());
    step(app, POLICE_GRACE / 2.0);
}

#[test]
fn listen_to_the_police_and_his_spikes_fold() {
    let mut app = app(BEAT);
    walk_up(&mut app);
    assert_eq!(police(&mut app).order(), Some(Order::Stop));
    wait_for(&mut app, |a| police(a).order() == Some(Order::Go(1)));
    step(&mut app, 0.2);
    assert!(app.world_mut().query::<&PoliceSpike>().iter(app.world()).all(|s| s.up == 0.0), "folded while he says go");
    hold_until(&mut app, RIGHT, |a| nat(a).x > 21.0 * TILE);
    assert!(seen(&app).deaths.is_empty(), "{:?}", seen(&app).deaths);
}

#[test]
fn walk_on_when_he_says_stop_and_a_spike_pops_up() {
    let mut app = app(BEAT);
    step(&mut app, 0.3);
    hold_until(&mut app, RIGHT, |a| !a.world().resource::<Seen>().deaths.is_empty() || nat(a).x > 30.0 * TILE);
    assert_eq!(seen(&app).deaths, vec![DeathCause::Police]);
    assert!(nat(&mut app).x < 15.0 * TILE, "before his own spikes: {}", nat(&mut app).x / TILE);
}

#[test]
fn jump_when_he_says_stop_and_a_cone_comes_down() {
    let mut app = app(BEAT);
    walk_up(&mut app);
    step(&mut app, POLICE_GRACE);
    tap(&mut app, SPACE);
    step(&mut app, 1.0);
    assert_eq!(seen(&app).deaths, vec![DeathCause::Police]);
}

#[test]
fn go_back_when_he_says_go_and_a_spike_pops_up() {
    let mut app = app(BEAT);
    walk_up(&mut app);
    wait_for(&mut app, |a| police(a).order() == Some(Order::Go(1)));
    step(&mut app, POLICE_GRACE);
    hold_until(&mut app, LEFT, |a| !a.world().resource::<Seen>().deaths.is_empty());
    assert_eq!(seen(&app).deaths, vec![DeathCause::Police]);
}

#[test]
fn his_spikes_dont_come_up_under_someone_who_stopped_on_them() {
    let mut app = app(BEAT);
    walk_up(&mut app);
    wait_for(&mut app, |a| police(a).order() == Some(Order::Go(1)));
    hold_until(&mut app, RIGHT, |a| nat(a).x > 16.5 * TILE);
    // Stopped on them; he says stop.
    wait_for(&mut app, |a| police(a).order() == Some(Order::Stop));
    step(&mut app, 1.0);
    assert!(seen(&app).deaths.is_empty(), "{:?}", seen(&app).deaths);
}

/// The paper at col 5, the door at col 12, the wall, the room at col 16.
const CODE: &str = "name: Code
solo: nat
code: up down left right
mode: bossa
---
..............#...............
..............#...............
..P..?......&.#.$.............G
##############################
";

fn type_code(app: &mut App, keys: &[KeyCode]) {
    tap(app, UP);
    for &k in keys {
        tap(app, k);
    }
    step(app, 0.2);
}

fn door(app: &mut App) -> CodeDoor {
    *app.world_mut().query::<&CodeDoor>().iter(app.world()).next().unwrap()
}

#[test]
fn read_the_code_and_type_it_at_the_door() {
    let mut app = app(CODE);
    step(&mut app, 0.3);
    hold_until(&mut app, RIGHT, |a| nat(a).x > 6.0 * TILE);
    assert_eq!(seen(&app).codes, vec![vec![Arrow::Up, Arrow::Down, Arrow::Left, Arrow::Right]], "read on the paper");
    hold_until(&mut app, RIGHT, |a| nat(a).x > 12.4 * TILE);
    step(&mut app, 0.4);
    // Wrong: a buzz, and shut.
    type_code(&mut app, &[UP, UP, UP, UP]);
    assert!(!door(&mut app).open);
    assert!(nat(&mut app).x < 14.0 * TILE);
    // Right: open, through, and the bossa.
    step(&mut app, 0.6);
    type_code(&mut app, &[UP, DOWN, LEFT, RIGHT]);
    assert!(door(&mut app).open);
    step(&mut app, 1.0);
    assert!((nat(&mut app).x - 16.5 * TILE).abs() < 4.0, "in the secret room: {}", nat(&mut app).x / TILE);
    assert_eq!(seen(&app).modes, vec![Some(RoomMode::Bossa)]);
    assert_eq!(app.world().resource::<RoomPlaying>().0, Some(RoomMode::Bossa));
    assert!(seen(&app).deaths.is_empty());
}

#[test]
fn a_door_needs_its_code_and_room() {
    assert!(Level::parse("name: x\n---\nP&$G\n####\n").is_err(), "no code");
    assert!(Level::parse("name: x\ncode: up\n---\nP&.G\n####\n").is_err(), "no room");
    assert!(Level::parse("name: x\ncode: up\nmode: polka\n---\nP&$G\n####\n").is_err(), "no such mode");
    assert!(Level::parse("name: x\ncode: up\n---\nP&$G\n####\n").is_ok());
}
