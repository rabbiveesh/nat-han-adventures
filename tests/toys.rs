//! The secret playground's toys, headless: the "helper" on his rope (his three tricks, and
//! jumping off), the blast button, stop signs and houses (the chimney, the chef).

use std::time::Duration;

use bevy::{input::InputPlugin, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use leafwing_input_manager::prelude::*;
use nat_han_adventures::{
    events::{DeathCause, PlayerDied},
    game::{
        Body, Player, Pos,
        diapers::Diaper,
        helper::{Helper, HelperState, OnRope, SNICKER, Trick},
        houses::Indoors,
        toybox::{self, TOY_COUNT},
        toys::Blasting,
    },
    level::{HOUSE_SHIFT, Level, Levels, TILE},
    state::AppState,
};

const DT: f64 = 1.0 / 60.0;
const SPACE: KeyCode = KeyCode::Space;
const RIGHT: KeyCode = KeyCode::ArrowRight;
const LEFT: KeyCode = KeyCode::ArrowLeft;
const UP: KeyCode = KeyCode::ArrowUp;

#[derive(Resource, Default)]
struct Deaths(Vec<DeathCause>);

fn app(level: &str) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin, InputPlugin))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(DT)))
        .add_plugins(nat_han_adventures::gameplay)
        .insert_resource(Levels(vec![Level::parse(level).expect("test level parses")]))
        .init_resource::<Deaths>()
        .add_systems(Last, |mut d: ResMut<Deaths>, mut r: MessageReader<PlayerDied>| d.0.extend(r.read().map(|m| m.cause)));
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

fn body(app: &mut App) -> Body {
    app.world_mut().query_filtered::<&Body, With<Player>>().single(app.world()).unwrap().clone()
}

fn on_rope(app: &mut App) -> bool {
    app.world_mut().query_filtered::<(), (With<Player>, With<OnRope>)>().iter(app.world()).count() == 1
}

fn helper(app: &mut App) -> Helper {
    *app.world_mut().query::<&Helper>().single(app.world()).unwrap()
}

fn deaths(app: &App) -> Vec<DeathCause> {
    app.world().resource::<Deaths>().0.clone()
}

fn state(app: &App) -> AppState {
    app.world().resource::<State<AppState>>().get().clone()
}

fn press(app: &mut App, k: KeyCode) {
    k.press(app.world_mut());
}

fn release(app: &mut App, k: KeyCode) {
    k.release(app.world_mut());
}

/// The helper up on the right edge of a ledge (cols 9-11), his rope hanging off its right
/// side over the floor, Nat under it; a pit of spikes beyond.
const LEDGE: &str = "name: Ledge
solo: nat
---
..............................................
..............................................
..............................................
...........H..................................
.........###..................................
.........###..................................
.........###..................................
.........###..................................
.........###..................................
.........###..................................
.........###P.................................
##############^^^^^^^^^^^^^^^#################G
";

/// Jump up to the rope's end (Nat's under it).
fn grab(app: &mut App) {
    let x = helper(app).end().x;
    let dir = if nat(app).x < x { RIGHT } else { LEFT };
    press(app, dir);
    for _ in 0..600 {
        step(app, 1.0 / 60.0);
        if (nat(app).x - x).abs() < 3.0 {
            break;
        }
    }
    release(app, dir);
    step(app, 0.1);
    press(app, SPACE);
    for _ in 0..60 {
        step(app, 1.0 / 60.0);
        if std::env::var("TOYS_TRACE").is_ok() {
            eprintln!("nat {} rope {} {:?}", nat(app), on_rope(app), helper(app).state);
        }
        if on_rope(app) {
            release(app, SPACE);
            step(app, 1.0 / 60.0);
            return;
        }
    }
    panic!("never grabbed the rope (Nat at {}, its end at {})", nat(app) / TILE, helper(app).end() / TILE);
}

fn ledge_app() -> App {
    let mut app = app(LEDGE);
    for _ in 0..18 {
        step(&mut app, 1.0 / 60.0);
        if std::env::var("TOYS_TRACE").is_ok() {
            eprintln!("start nat {} rope {} {:?}", nat(&mut app), on_rope(&mut app), helper(&mut app));
        }
    }
    app
}

#[test]
fn the_helpers_rope_hangs_off_the_open_side_of_his_ledge() {
    let mut app = ledge_app();
    let h = helper(&mut app);
    assert_eq!(h.side, 1.0, "right of the ledge");
    assert!(h.end().y > 1.0 * TILE && h.end().y < 3.0 * TILE, "down to just over the floor: {}", h.end() / TILE);
    assert_eq!(h.state, HelperState::Waiting);
}

#[test]
fn his_first_trick_drops_nat_after_a_snicker() {
    let mut app = ledge_app();
    grab(&mut app);
    let caught = nat(&mut app);
    eprintln!("{:?}", helper(&mut app));
    assert!(matches!(helper(&mut app).state, HelperState::Hauling { trick: Trick::Drop, .. }));
    let mut snickered = None;
    for i in 0..(6 * 60) {
        step(&mut app, 1.0 / 60.0);
        if snickered.is_none() && helper(&mut app).snickering() {
            snickered = Some(i);
        }
        if !on_rope(&mut app) {
            let s = snickered.expect("a snicker first");
            assert!(((i - s) as f32 / 60.0 - SNICKER).abs() < 0.05, "he lets go a snicker after");
            assert!(nat(&mut app).y > caught.y + 1.5 * TILE, "hauled up a way first");
            return;
        }
    }
    panic!("he never let go");
}

#[test]
fn a_jump_lets_go_of_the_rope_any_time() {
    let mut app = ledge_app();
    grab(&mut app);
    step(&mut app, 0.5);
    let y = nat(&mut app).y;
    press(&mut app, SPACE);
    step(&mut app, 0.1);
    release(&mut app, SPACE);
    assert!(!on_rope(&mut app));
    assert!(nat(&mut app).y > y, "a jump off it");
    assert_eq!(helper(&mut app).state, HelperState::Lowering);
    step(&mut app, 2.0);
    assert_eq!(helper(&mut app).state, HelperState::Waiting, "lowered again");
}

/// Grab, and wait out whatever he does; back on the floor.
fn ride_out(app: &mut App) {
    grab(app);
    for _ in 0..(8 * 60) {
        step(app, 1.0 / 60.0);
        if !on_rope(app) && body(app).on_ground {
            break;
        }
    }
    step(app, 2.0);
}

#[test]
fn the_fling_throws_nat_out_and_the_honest_one_hoists_him_up() {
    let mut app = ledge_app();
    ride_out(&mut app);
    assert!(deaths(&app).is_empty(), "a drop to the floor: {:?}", deaths(&app));
    // (Back under the rope.)
    // Second grab: the fling, out over the spikes.
    grab(&mut app);
    assert!(matches!(helper(&mut app).state, HelperState::Hauling { trick: Trick::Fling, .. }));
    let mut far = 0.0f32;
    for _ in 0..(8 * 60) {
        step(&mut app, 1.0 / 60.0);
        far = far.max(nat(&mut app).x);
        if std::env::var("TOYS_TRACE").is_ok() {
            eprintln!("fling {} rope {} {:?} {}", nat(&mut app) / TILE, on_rope(&mut app), helper(&mut app).state, helper(&mut app).angle);
        }
        if !deaths(&app).is_empty() {
            break;
        }
    }
    assert_eq!(deaths(&app).first(), Some(&DeathCause::Spikes), "flung onto the spikes: {} {:?}", far / TILE, nat(&mut app) / TILE);
    assert!(far > helper(&mut app).pivot.x + 3.0 * TILE, "flung out: {}", far / TILE);
    // Third: honest. (The respawn's at the start, under the rope.)
    step(&mut app, 2.0);
    grab(&mut app);
    assert!(matches!(helper(&mut app).state, HelperState::Hauling { trick: Trick::Honest, .. }));
    for _ in 0..(8 * 60) {
        step(&mut app, 1.0 / 60.0);
        if std::env::var("TOYS_TRACE").is_ok() {
            eprintln!("honest {} rope {} {:?} len {}", nat(&mut app) / TILE, on_rope(&mut app), helper(&mut app).state, helper(&mut app).len);
        }
        if !on_rope(&mut app) && body(&mut app).on_ground {
            break;
        }
    }
    let at = nat(&mut app);
    assert!(at.y > 7.0 * TILE && at.x < helper(&mut app).pivot.x, "up on the ledge: {}", at / TILE);
}

const BUTTON: &str = "name: Button
solo: nat
---
..............................................
..............................................
..............................................
..............................................
..............................................
..............................................
..............D...............................
..............................................
..............................................
..............................................
.P......M.....................................G
##############################################
";

#[test]
fn landing_on_the_button_blasts_and_walking_on_it_doesnt() {
    let mut app = app(BUTTON);
    step(&mut app, 0.3);
    // Walk over it.
    press(&mut app, RIGHT);
    step(&mut app, 0.6);
    release(&mut app, RIGHT);
    assert_eq!(app.world().resource::<Blasting>().0, 0.0, "a walk over it doesn't press it");
    // Back, and jump on it.
    press(&mut app, LEFT);
    step(&mut app, 0.25);
    release(&mut app, LEFT);
    step(&mut app, 0.3);
    let x = nat(&mut app).x;
    let button = 8.5 * TILE;
    let dir = if x < button { RIGHT } else { LEFT };
    press(&mut app, SPACE);
    press(&mut app, dir);
    let mut peak = 0.0f32;
    for _ in 0..90 {
        step(&mut app, 1.0 / 60.0);
        if (nat(&mut app).x - button).abs() < 3.0 {
            release(&mut app, dir);
        }
        peak = peak.max(nat(&mut app).y);
    }
    release(&mut app, SPACE);
    release(&mut app, dir);
    for _ in 0..120 {
        step(&mut app, 1.0 / 60.0);
        peak = peak.max(nat(&mut app).y);
    }
    assert!(app.world().resource::<Blasting>().0 > 0.0, "BLAST");
    assert!(peak > 8.0 * TILE, "thrown up high: {}", peak / TILE);
    let gone = app.world_mut().query::<&Diaper>().iter(app.world()).all(|d| !d.deadly());
    assert!(gone, "the diaper's knocked out of the sky");
}

const SIGN: &str = "name: Sign
solo: nat
---
..............................................
.P......@.....................................G
##############################################
";

#[test]
fn a_stop_sign_stops_the_level_right_there() {
    let mut app = app(SIGN);
    step(&mut app, 0.3);
    press(&mut app, RIGHT);
    step(&mut app, 0.8);
    assert_eq!(state(&app), AppState::Playing, "skidding first");
    step(&mut app, 1.2);
    assert_eq!(state(&app), AppState::LevelComplete, "cleared at the sign");
    assert!(nat(&mut app).x < 9.0 * TILE, "stopped at it");
}

const HOUSES: &str = "name: Houses
solo: nat
---
..............................................
..............................................
..............................................
..............................................
..............................................
..............................................
..............................................
..............................................
..............................................
..............................................
.P....A..........K..........a.................G
##############################################
";

fn walk_to(app: &mut App, col: f32) {
    let x = (col + 0.5) * TILE;
    let dir = if nat(app).x < x { RIGHT } else { LEFT };
    press(app, dir);
    for _ in 0..600 {
        step(app, 1.0 / 60.0);
        if (nat(app).x - x).abs() < 2.0 {
            break;
        }
    }
    release(app, dir);
    step(app, 0.3);
}

fn go_in(app: &mut App) {
    press(app, UP);
    step(app, 1.0 / 60.0);
    release(app, UP);
}

fn indoors(app: &mut App) -> bool {
    app.world_mut().query_filtered::<(), (With<Player>, With<Indoors>)>().iter(app.world()).count() == 1
}

#[test]
fn nobody_home_out_the_chimney_and_a_chef_makes_schnitzel() {
    let mut app = app(HOUSES);
    step(&mut app, 0.3);
    walk_to(&mut app, 6.0);
    let floor = nat(&mut app).y;
    go_in(&mut app);
    assert!(indoors(&mut app), "in at the door (not a jump)");
    assert!(nat(&mut app).y < floor + 2.0);
    let mut peak = 0.0f32;
    for _ in 0..(3 * 60) {
        step(&mut app, 1.0 / 60.0);
        peak = peak.max(nat(&mut app).y);
    }
    assert!(!indoors(&mut app));
    assert!(peak > floor + 8.0 * TILE, "out of the chimney, up high: {}", (peak - floor) / TILE);
    assert!(deaths(&app).is_empty());

    walk_to(&mut app, 17.0);
    go_in(&mut app);
    assert!(indoors(&mut app));
    step(&mut app, 1.5);
    assert_eq!(deaths(&app), vec![DeathCause::Schnitzel]);
}

#[test]
fn a_sometimes_house_has_the_chef_in_every_other_shift() {
    let mut app = app(HOUSES);
    step(&mut app, 0.3);
    walk_to(&mut app, 28.0);
    // Wait for the chef's shift, then go in.
    let clock = |app: &App| app.world().resource::<nat_han_adventures::game::SimClock>().time;
    while ((clock(&app) / HOUSE_SHIFT).floor() as i64) % 2 == 0 {
        step(&mut app, 1.0 / 60.0);
    }
    step(&mut app, 0.2);
    go_in(&mut app);
    step(&mut app, 1.5);
    assert_eq!(deaths(&app), vec![DeathCause::Schnitzel], "the chef's in");
    // Back to it (respawned at the start), out of the chef's shift.
    walk_to(&mut app, 28.0);
    while ((clock(&app) / HOUSE_SHIFT).floor() as i64) % 2 == 1 {
        step(&mut app, 1.0 / 60.0);
    }
    step(&mut app, 0.2);
    go_in(&mut app);
    step(&mut app, 1.5);
    assert_eq!(deaths(&app).len(), 1, "nobody home: out the chimney");
}

#[test]
fn every_playground_parses() {
    for n in 0..TOY_COUNT {
        let level = toybox::load(n);
        assert!(level.solo);
    }
}
