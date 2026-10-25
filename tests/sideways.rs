//! The gadget tiles laid sideways and upside down, headless: wall fans (`e` `w`) and ceiling
//! fans (`!`), wall cushions (`b` `d`) and ceiling cushions (`m`), wall brushes (`|`), in the
//! real game (Nat) and in the validator.

use std::time::Duration;

use bevy::{input::InputPlugin, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use leafwing_input_manager::prelude::*;
use nat_han_adventures::{
    events::{DeathCause, PlayerDied},
    game::{Body, Player, Pos},
    level::{CUSHION_FLING, CUSHION_HOLD, CUSHION_LIFT, Level, Levels, TILE, Tile, cushion_shot, validate},
    state::AppState,
};

const DT: f64 = 1.0 / 60.0;
const JUMP: KeyCode = KeyCode::KeyZ;
const LEFT: KeyCode = KeyCode::ArrowLeft;
const RIGHT: KeyCode = KeyCode::ArrowRight;

#[derive(Resource, Default)]
struct Deaths(Vec<DeathCause>);

fn game(level: &str) -> App {
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

fn deaths(app: &App) -> Vec<DeathCause> {
    app.world().resource::<Deaths>().0.clone()
}

/// Nat's top and lowest-left/rightmost x while `secs` pass.
fn track(app: &mut App, secs: f32) -> (f32, f32, f32) {
    let (mut top, mut lo, mut hi) = (f32::MIN, f32::MAX, f32::MIN);
    for _ in 0..(secs as f64 / DT).round() as usize {
        app.update();
        let p = nat(app);
        top = top.max(p.y);
        lo = lo.min(p.x);
        hi = hi.max(p.x);
    }
    (top, lo, hi)
}

const WIND: &str = "name: Wind
---
....................
....................
....................
....................
....................
...........P......G.
e...................
####################
";

#[test]
fn a_wall_fan_pushes_nat_along_its_gust_and_slows_him_against_it() {
    // Standing in the gust (a fan reaches 6 tiles: Nat at col 11 is out of it).
    let mut app = game(&WIND.replace("...........P......G.", "....P.............G."));
    step(&mut app, 0.2);
    let x0 = nat(&mut app).x;
    step(&mut app, 0.5);
    assert!(nat(&mut app).x > x0 + TILE, "blown right while standing still: {:.2} tiles", (nat(&mut app).x - x0) / TILE);
    // Out of it: still.
    let mut app = game(WIND);
    step(&mut app, 0.2);
    let x0 = nat(&mut app).x;
    step(&mut app, 0.5);
    assert_eq!(nat(&mut app).x, x0, "out of the gust, nothing moves Nat");
    // Walking left into it: still gets on, slower than a plain walk.
    let mut app = game(&WIND.replace("...........P......G.", ".....P............G."));
    step(&mut app, 0.2);
    let x0 = nat(&mut app).x;
    LEFT.press(app.world_mut());
    step(&mut app, 0.5);
    let went = (x0 - nat(&mut app).x) / TILE;
    assert!(went > 0.3 && went < 3.0, "slowed against the gust: {went:.2} tiles in 0.5 s");
}

const CEILING_FAN: &str = "name: Ceiling fan
---
....................
....................
....................
...!................
....................
....................
....................
....................
..P...........G.....
####################
";

#[test]
fn a_ceiling_fan_presses_nat_down() {
    let jump_top = |src: &str| {
        let mut app = game(src);
        step(&mut app, 0.2);
        let y0 = nat(&mut app).y;
        // Hop right under the fan, then jump.
        RIGHT.press(app.world_mut());
        step(&mut app, 0.1);
        RIGHT.release(app.world_mut());
        step(&mut app, 0.2);
        JUMP.press(app.world_mut());
        let (top, _, _) = track(&mut app, 0.6);
        JUMP.release(app.world_mut());
        (top - y0) / TILE
    };
    let plain = jump_top(&CEILING_FAN.replace('!', "#"));
    let pressed = jump_top(CEILING_FAN);
    assert!(plain > 3.0, "a plain jump rises over 3 tiles: {plain:.2}");
    assert!(pressed < plain - 1.0, "the downdraft keeps it lower: {pressed:.2} vs {plain:.2}");
}

const WALL_CUSHION: &str = "name: Wall cushion
---
.............................
.............................
.............................
.............................
.............................
.............................
.G......................P..d.
#############################
";

fn pad(app: &mut App) -> f32 {
    app.world_mut().query_filtered::<&Body, With<Player>>().single(app.world()).unwrap().pad
}

/// Run Nat right into the cushion; returns once it has caught him.
fn run_into_cushion(app: &mut App) {
    step(app, 0.2);
    RIGHT.press(app.world_mut());
    for _ in 0..60 {
        app.update();
        if pad(app) != 0.0 {
            return;
        }
    }
    panic!("never caught");
}

#[test]
fn a_wall_cushion_catches_nat_holds_him_then_slings_him_back() {
    let mut app = game(WALL_CUSHION);
    run_into_cushion(&mut app);
    assert_eq!(pad(&mut app), -1.0, "the cushion on his right slings him left");
    // Held still in it (Right still held: aimed into it)...
    let caught_at = nat(&mut app);
    let mut held = 0;
    while pad(&mut app) != 0.0 {
        assert_eq!(nat(&mut app), caught_at, "sunk in and held still");
        app.update();
        held += 1;
        assert!(held < 30, "and let go");
    }
    let secs = held as f32 / 60.0;
    assert!((CUSHION_HOLD - 0.02..CUSHION_HOLD + 0.04).contains(&secs), "held {secs:.2} s");
    RIGHT.release(app.world_mut());
    let (top, lo, _) = track(&mut app, 0.8);
    assert!(lo < caught_at.x - 2.0 * TILE, "slung back left: {:.2} tiles", (caught_at.x - lo) / TILE);
    assert!(top > caught_at.y + 2.0 * TILE, "and up");
    assert!(deaths(&app).is_empty());
}

/// Where the cushion slings Nat for `aim` held as it fires (-1 left, away from this one; +1
/// right, into it): (top, how far left he gets before he lands back at the cushion's height),
/// in tiles from where he was caught.
fn shot(aim: f32) -> (f32, f32) {
    let mut app = game(WALL_CUSHION);
    run_into_cushion(&mut app);
    RIGHT.release(app.world_mut());
    if aim < 0.0 {
        LEFT.press(app.world_mut());
    } else if aim > 0.0 {
        RIGHT.press(app.world_mut());
    }
    let at = nat(&mut app);
    while pad(&mut app) != 0.0 {
        app.update();
    }
    LEFT.release(app.world_mut());
    RIGHT.release(app.world_mut());
    let mut top = at.y;
    let mut prev = at;
    for _ in 0..180 {
        app.update();
        let p = nat(&mut app);
        top = top.max(p.y);
        if p.y <= at.y && prev.y > at.y {
            break;
        }
        prev = p;
    }
    assert!(deaths(&app).is_empty());
    ((top - at.y) / TILE, (at.x - nat(&mut app).x) / TILE)
}

#[test]
fn a_wall_cushion_aims_by_what_is_held_as_it_fires() {
    let plain = shot(0.0);
    let steep = shot(1.0);
    let long = shot(-1.0);
    let (away, up) = cushion_shot(0.0);
    assert_eq!((away, up), (CUSHION_FLING, CUSHION_LIFT));
    assert!(steep.0 > plain.0 + 2.0, "held into it: steep, {steep:?} vs plain {plain:?}");
    assert!(steep.1 < plain.1, "and short, {steep:?} vs plain {plain:?}");
    assert!(long.0 < plain.0, "held away: low, {long:?} vs plain {plain:?}");
    assert!(long.1 > plain.1 + 1.0, "and long, {long:?} vs plain {plain:?}");
}

#[test]
fn a_jump_fires_a_wall_cushion_early_and_the_toot_comes_back() {
    let mut app = game(WALL_CUSHION);
    run_into_cushion(&mut app);
    RIGHT.release(app.world_mut());
    let at = nat(&mut app);
    JUMP.press(app.world_mut());
    app.update();
    assert_eq!(pad(&mut app), 0.0, "fired at once");
    assert!(nat(&mut app).x < at.x, "already on his way");
    JUMP.release(app.world_mut());
    step(&mut app, 0.25);
    // The jump only fired it; the toot is still there.
    let before = nat(&mut app).y;
    let vy = |app: &mut App| app.world_mut().query_filtered::<&Body, With<Player>>().single(app.world()).unwrap().vel.y;
    let falling = vy(&mut app);
    JUMP.press(app.world_mut());
    app.update();
    JUMP.release(app.world_mut());
    assert!(vy(&mut app) > falling.max(0.0) + 100.0, "tooted ({falling:.0} -> {:.0} px/s)", vy(&mut app));
    step(&mut app, 0.2);
    assert!(nat(&mut app).y > before, "up again");
}

const CEILING_CUSHION: &str = "name: Ceiling cushion
---
....................
....................
....................
....................
....................
..m.................
....................
....................
..P...........G.....
####################
";

#[test]
fn a_ceiling_cushion_springs_a_bumped_head_back_down() {
    let fall_after_bump = |src: &str| {
        let mut app = game(src);
        step(&mut app, 0.2);
        JUMP.press(app.world_mut());
        let mut fastest = 0.0f32;
        let mut last = nat(&mut app).y;
        for _ in 0..30 {
            step(&mut app, 1.0 / 60.0);
            let y = nat(&mut app).y;
            fastest = fastest.max((last - y) * 60.0);
            last = y;
        }
        JUMP.release(app.world_mut());
        fastest
    };
    let plain = fall_after_bump(&CEILING_CUSHION.replace("..m.", "..#."));
    let sprung = fall_after_bump(CEILING_CUSHION);
    assert!(sprung > plain + 100.0, "slammed back down: {sprung:.0} px/s vs a plain bonk's {plain:.0}");
}

const BRUSHES: &str = "name: Brushes
---
....................
....................
....................
....................
..P..........|#...G.
####################
";

#[test]
fn wall_brushes_point_away_from_their_wall() {
    let level = Level::parse(BRUSHES).unwrap();
    assert_eq!(level.tile(13, 4), Tile::SpikesLeft, "the wall's on the right: they point left");
    assert_eq!(Level::parse(&BRUSHES.replace("|#", "#|")).unwrap().tile(14, 4), Tile::SpikesRight);
    assert!(Level::parse(&BRUSHES.replace("|#", "|.")).is_err(), "brushes need a wall beside them");
    // Run into them: splat. No stain (the tile stays).
    let mut app = game(BRUSHES);
    step(&mut app, 0.2);
    RIGHT.press(app.world_mut());
    JUMP.press(app.world_mut());
    step(&mut app, 0.2);
    JUMP.release(app.world_mut());
    step(&mut app, 1.4);
    assert_eq!(deaths(&app).first(), Some(&DeathCause::Spikes), "ran into the brushes");
}

fn reaches(src: &str) -> bool {
    let level = Level::parse(src).unwrap();
    validate::reachable_plain(&level, (level.goal.0 as i32, level.goal.1 as i32))
}

#[test]
fn the_validator_crosses_a_gap_on_a_tailwind_only() {
    // A 9-tile gap: too wide for a plain run and jump (and toot), not with the gust behind
    // (a wall fan floating over the edge, blowing over it).
    let gap = "name: Tailwind
---
....................
....................
....................
......e.............
......e.............
......e.............
..P..............G..
#######.........####
#######.........####
";
    assert!(reaches(gap), "the gust carries the jump over");
    assert!(!reaches(&gap.replace("......e", "......#")), "no gust, no way over");
}

#[test]
fn the_validator_rides_a_wall_cushion_over_a_gap() {
    // A 12-tile pit: run back into the cushion, get flung over it.
    let pit = "name: Fling
---
.........................
.........................
.........................
.........................
.........................
.........................
.G...............P...d...
#####............########
#####............########
";
    assert!(reaches(pit), "flung over the pit");
    assert!(!reaches(&pit.replace('d', "#")), "no cushion, no way over");
}

#[test]
fn the_validator_cant_climb_into_a_ceiling_fans_downdraft() {
    // A shelf a plain jump reaches, under a ceiling fan.
    let shelf = "name: Downdraft
---
....................
....................
!!!!!!!!!!!!!!!!!!!!
....................
......G.............
.....===............
....................
....................
....................
..P.................
####################
";
    assert!(reaches(&shelf.replace("!!!!!!!!!!!!!!!!!!!!", "....................")), "a plain jump onto the shelf");
    assert!(!reaches(shelf), "the downdraft keeps Nat off it");
}
