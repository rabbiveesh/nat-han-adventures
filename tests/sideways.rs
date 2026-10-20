//! The gadget tiles laid sideways and upside down, headless: wall fans (`e` `w`) and ceiling
//! fans (`!`), wall cushions (`b` `d`) and ceiling cushions (`m`), wall brushes (`|`), in the
//! real game (Nat) and in the validator.

use std::time::Duration;

use bevy::{input::InputPlugin, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use leafwing_input_manager::prelude::*;
use nat_han_adventures::{
    events::{DeathCause, PlayerDied},
    game::{Player, Pos, tuning},
    level::{Level, Levels, TILE, Tile, validate},
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

#[test]
fn a_wall_cushion_flings_nat_far_back() {
    let mut app = game(WALL_CUSHION);
    step(&mut app, 0.2);
    let x0 = nat(&mut app).x;
    RIGHT.press(app.world_mut());
    step(&mut app, 0.3);
    RIGHT.release(app.world_mut());
    let (top, lo, _) = track(&mut app, 0.8);
    assert!(lo < x0 - 6.0 * TILE, "flung far left: {:.2} tiles", (x0 - lo) / TILE);
    assert!(top > x0.min(f32::MAX) * 0.0 + tuning::PLAYER_SIZE.1, "and up off the floor a little");
    assert!(deaths(&app).is_empty());
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
