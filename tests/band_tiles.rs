//! The gadget tiles answer the band, headless: fired up, belts and wall fans run faster; under
//! Giant Steps every fan blows twice as far; in the waltz a trampoline catches you and springs
//! you on ONE, higher, and a cushion or duck hit on ONE flings you harder. In the real game
//! (Nat) and in the validator, whose call gates can need a mode and a tile together.

use std::time::Duration;

use bevy::{input::InputPlugin, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use leafwing_input_manager::prelude::*;
use nat_han_adventures::{
    audio::{Harmony, waltz::WALTZ_BPM},
    events::{DeathCause, PlayerDied},
    game::{BeatClock, Groove, Player, Pos},
    level::{
        BELT_SPEED, FAN_REACH, FIRED_UP_TILES, GIANT_FAN_REACH, Level, Levels, TILE,
        validate::{Gate, Options, Physics, Report, check_with},
    },
    state::AppState,
};

const DT: f64 = 1.0 / 60.0;
const RIGHT: KeyCode = KeyCode::ArrowRight;

#[derive(Resource, Default)]
struct Deaths(Vec<DeathCause>);

/// The game on `level`, the band playing `harmony`.
fn game(level: &str, harmony: Harmony) -> App {
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
    *app.world_mut().resource_mut::<Groove>() = Groove::of(harmony);
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

/// Nat's highest point over `secs`.
fn top(app: &mut App, secs: f32) -> f32 {
    let mut top = f32::MIN;
    for _ in 0..(secs as f64 / DT).round() as usize {
        app.update();
        top = top.max(nat(app).y);
    }
    top
}

// ─── Fired up: belts and wall fans run faster ───────────────────────────────

const BELT: &str = "name: Belt
---
................................
................................
................................
................................
..P.............................
#}}}}}}}}}}}}}}}}}}}}}}}}}}}}}G#
";

#[test]
fn fired_up_belts_run_faster() {
    let carried = |harmony| {
        let mut app = game(BELT, harmony);
        step(&mut app, 0.2);
        let x0 = nat(&mut app).x;
        step(&mut app, 1.0);
        nat(&mut app).x - x0
    };
    let (plain, fired) = (carried(Harmony::Original), carried(Harmony::Quartal));
    assert!((plain - BELT_SPEED).abs() < 3.0, "a belt carries {plain:.1} px/s");
    assert!((fired - BELT_SPEED * FIRED_UP_TILES).abs() < 3.0, "fired up, {fired:.1} px/s");
}

const WIND: &str = "name: Wind
---
....................
....................
....................
....................
....................
....P.............G.
e...................
####################
";

#[test]
fn fired_up_wall_fans_blow_harder() {
    let blown = |harmony| {
        // From where he lands, a quarter second: still in the gust.
        let mut app = game(WIND, harmony);
        let x0 = nat(&mut app).x;
        step(&mut app, 0.3);
        nat(&mut app).x - x0
    };
    let (plain, fired) = (blown(Harmony::Original), blown(Harmony::Quartal));
    assert!(plain > TILE, "a gust blows Nat along: {plain:.1} px");
    assert!(fired > plain * 1.3, "fired up, harder: {fired:.1} px vs {plain:.1}");
}

// ─── Giant Steps: fans blow twice as far ────────────────────────────────────

#[test]
fn giant_steps_fans_reach_twice_as_far() {
    let mut rows = vec!["..............".to_string(); 18];
    rows.push("....P........G".into());
    rows.push("....*.........".into());
    rows.push("##############".into());
    let src = format!("name: Fan\n---\n{}\n", rows.join("\n"));
    let rise = |harmony| {
        let mut app = game(&src, harmony);
        // Standing on the fan's tile (it floats Nat off it at once).
        let floor = 2.0 * TILE;
        (top(&mut app, 4.0) - floor) / TILE
    };
    let (plain, giant) = (rise(Harmony::Original), rise(Harmony::Coltrane));
    assert!((plain - FAN_REACH as f32).abs() < 1.5, "a fan floats Nat ~{FAN_REACH} tiles: {plain:.2}");
    assert!((giant - GIANT_FAN_REACH as f32).abs() < 1.5, "under Giant Steps ~{GIANT_FAN_REACH}: {giant:.2}");
    // A wall fan's gust reaches as far: Nat 11 tiles from it.
    let far = WIND.replace("....P.............G.", "...........P......G.");
    let pushed = |harmony| {
        let mut app = game(&far, harmony);
        let x0 = nat(&mut app).x;
        step(&mut app, 0.5);
        nat(&mut app).x - x0
    };
    assert_eq!(pushed(Harmony::Original), 0.0, "out of an everyday gust");
    assert!(pushed(Harmony::Coltrane) > TILE, "in a giant one");
}

// ─── The waltz: trampolines catch you and spring you on ONE ─────────────────

/// The waltz at `beats` beats in.
fn waltz_at(app: &mut App, beats: f64) {
    let secs = 60.0 / WALTZ_BPM as f64;
    *app.world_mut().resource_mut::<Groove>() = Groove::of(Harmony::Waltz).at(BeatClock::at(beats, secs, 3));
}

fn spring_level() -> String {
    let mut rows = vec!["........................".to_string(); 22];
    rows.push("..P....................G".into());
    rows.push("#####TTTT###############".into());
    format!("name: Spring\n---\n{}\n", rows.join("\n"))
}

#[test]
fn a_waltz_trampoline_holds_nat_till_one_then_springs_him_higher() {
    // The everyday spring.
    let mut app = game(&spring_level(), Harmony::Original);
    step(&mut app, 0.3);
    let floor = nat(&mut app).y;
    RIGHT.press(app.world_mut());
    while nat(&mut app).x < 5.5 * TILE {
        app.update();
    }
    RIGHT.release(app.world_mut());
    let plain = (top(&mut app, 2.5) - floor) / TILE;
    assert!(plain > 5.8, "a trampoline springs ~6.5 tiles: {plain:.2}");

    // The waltz, off ONE: caught, standing on it.
    let mut app = game(&spring_level(), Harmony::Waltz);
    let beat = 60.0 / WALTZ_BPM as f64;
    let mut beats = 1.0;
    waltz_at(&mut app, beats);
    step(&mut app, 0.3);
    RIGHT.press(app.world_mut());
    while nat(&mut app).x < 5.5 * TILE {
        beats += DT / beat;
        waltz_at(&mut app, beats);
        app.update();
    }
    RIGHT.release(app.world_mut());
    assert!(beats < 2.0, "on before ONE ({beats:.2} beats)");
    let mut held = 0.0f32;
    while beats < 2.6 {
        beats += DT / beat;
        waltz_at(&mut app, beats);
        app.update();
        held = held.max(nat(&mut app).y - floor);
    }
    assert!(held < 1.0, "caught off ONE, Nat stays put ({held:.1} px up)");
    let x = nat(&mut app).x / TILE;
    assert!((5.0..9.0).contains(&x), "on the trampolines (col {x:.1})");
    // On to the next ONE: sprung, higher than the everyday spring.
    let mut high = 0.0f32;
    for _ in 0..240 {
        beats += DT / beat;
        waltz_at(&mut app, beats);
        app.update();
        high = high.max(nat(&mut app).y - floor);
    }
    let high = high / TILE;
    assert!(high > plain * 1.6, "sprung on ONE: {high:.2} tiles vs {plain:.2}");
    assert!(deaths(&app).is_empty());
}

const DUCK: &str = "name: Duck
---
....................
....................
....................
....................
....................
....................
....................
..G.................
..........P..(......
####################
";

#[test]
fn a_duck_hit_on_one_bats_harder() {
    let batted = |beats: f64| {
        let mut app = game(DUCK, Harmony::Waltz);
        waltz_at(&mut app, beats);
        step(&mut app, 0.2);
        let start = nat(&mut app);
        RIGHT.press(app.world_mut());
        let mut high = f32::MIN;
        for _ in 0..30 {
            app.update();
            high = high.max(nat(&mut app).y);
        }
        RIGHT.release(app.world_mut());
        high = high.max(top(&mut app, 0.7));
        (high - start.y) / TILE
    };
    // The clock held still: on ONE all the way, or off it.
    let (off, on) = (batted(1.5), batted(0.0));
    assert!(off > 1.0, "batted up: {off:.2} tiles");
    assert!(on > off * 1.5, "harder on ONE: {on:.2} vs {off:.2}");
}

// ─── The level checker ──────────────────────────────────────────────────────

fn run_check(src: &str) -> Report {
    static PHYS: std::sync::OnceLock<Physics> = std::sync::OnceLock::new();
    check_with(&Level::parse(src).unwrap(), &Options::default(), PHYS.get_or_init(Physics::new))
}

fn gates(r: &Report) -> Vec<Gate> {
    r.gates.iter().map(|g| g.0).collect()
}

/// A wall `h` tall at col 20 with the goal on top, a runway before it, `foot` at its foot.
fn wall(h: usize, foot: char) -> String {
    let w = 42;
    let rows_n = (h + 8).max(14);
    let mut rows: Vec<Vec<char>> = vec![vec!['.'; w]; rows_n];
    let floor = rows_n - 2;
    for c in 0..w {
        rows[floor][c] = '#';
        rows[floor + 1][c] = '#';
    }
    for r in floor - h..floor {
        for c in 20..w {
            rows[r][c] = '#';
        }
    }
    rows[floor][19] = foot;
    // The nuggets a full check wants, along the floor.
    for c in 3..19 {
        rows[floor - 1][c] = 'o';
    }
    rows[floor - 1][2] = 'P';
    rows[floor - 1][8] = 'C';
    rows[floor - h - 1][25] = 'G';
    let rows: Vec<String> = rows.iter().map(|r| r.iter().collect()).collect();
    format!("name: Wall\nsolo: nat\n---\n{}\n", rows.join("\n"))
}

/// A bottomless gap `w` wide from col 24, a run-up of `run` tiles before it under a line of
/// nuggets (to fire the band up).
fn gap(w: usize, run: char) -> String {
    let width = 24 + w + 16;
    let mut rows: Vec<Vec<char>> = vec![vec!['.'; width]; 14];
    for r in 12..14 {
        for c in 0..width {
            rows[r][c] = if (24..24 + w).contains(&c) { '.' } else { '#' };
        }
    }
    for c in 12..24 {
        rows[12][c] = run;
    }
    for c in 3..19 {
        rows[11][c] = 'o';
    }
    rows[11][2] = 'P';
    rows[11][8] = 'C';
    rows[11][width - 3] = 'G';
    let rows: Vec<String> = rows.iter().map(|r| r.iter().collect()).collect();
    format!("name: Gap\nsolo: nat\n---\n{}\n", rows.join("\n"))
}

/// Reached, and how: the gates crossed (the goal unreachable: `None`).
fn crossed(src: &str) -> Option<Vec<Gate>> {
    let r = run_check(src);
    let errs: Vec<_> = r.errs.iter().filter(|e| !e.contains("nugget") && !e.contains("level is only")).collect();
    if errs.iter().any(|e| e.contains("goal at")) {
        return None;
    }
    assert!(errs.is_empty(), "{errs:#?}");
    Some(gates(&r))
}

#[test]
fn the_validator_knows_a_fired_up_belt_jumps_further() {
    // Off plain floor, 11 is as far as the fired-up band jumps; off a belt going your way, 11
    // is an everyday jump and 12 to 15 need the band fired up.
    assert_eq!(crossed(&gap(11, '#')), Some(vec![Gate::LongGap]));
    assert_eq!(crossed(&gap(12, '#')), None);
    assert_eq!(crossed(&gap(11, '}')), Some(vec![]));
    for w in [12, 15] {
        assert_eq!(crossed(&gap(w, '}')), Some(vec![Gate::LongGap]), "{w} wide off a belt");
    }
}

#[test]
fn the_validator_knows_giant_steps_fans_blow_further() {
    // A wall 9 to 13 tall: too tall for Giant Steps' jump or an everyday fan, not for Giant
    // Steps over a fan.
    for h in [9, 13] {
        assert_eq!(crossed(&wall(h, '#')), None, "{h} tall, no fan");
        assert_eq!(crossed(&wall(h, '*')), Some(vec![Gate::GiantWall]), "{h} tall over a fan");
    }
    assert_eq!(crossed(&wall(8, '*')), Some(vec![]), "an everyday fan floats Nat up 8");
}

#[test]
fn the_validator_knows_a_waltz_spring_on_one() {
    // A trampoline at the foot of a wall: Giant Steps' low gravity springs you up to 13, the
    // waltz's spring on ONE to 14, where nothing else gets.
    assert_eq!(crossed(&wall(14, 'T')), Some(vec![Gate::SpringOnOne]));
    assert_eq!(crossed(&wall(14, '#')), None);
    assert_eq!(crossed(&wall(15, 'T')), None);
}
