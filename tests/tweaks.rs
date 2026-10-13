//! Per-tile physics tweaks (`level::tweak`) in the real gameplay, headless: a trampoline floor
//! springs Nat up, ice keeps him sliding, a belt carries him, low gravity floats him; and the
//! level editor's document round-trips every level file unchanged.

use std::time::Duration;

use bevy::{input::InputPlugin, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use nat_han_adventures::{
    game::{Body, Player, Pos},
    level::{Level, Levels, TILE, doc::LevelDoc},
    state::AppState,
};

const DT: f64 = 1.0 / 60.0;

fn app(level: &str) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin, InputPlugin))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(DT)))
        .add_plugins(nat_han_adventures::gameplay)
        .insert_resource(Levels(vec![Level::parse(level).expect("test level parses")]));
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

fn nat(app: &mut App) -> (Vec2, Vec2) {
    let (p, b) = app.world_mut().query_filtered::<(&Pos, &Body), With<Player>>().single(app.world()).unwrap();
    (p.0, b.vel)
}

/// Nat dropped from high over a floor: the highest he gets after the first landing (px).
fn rebound(layer: &str) -> f32 {
    let src = format!(
        "name: T\ntweak a: bounce=1\n---\n........................\n..P.....................\n........................\n........................\n........................\n........................\n........................\n........................\n.....................G..\n########################\n---\n{layer}"
    );
    let mut app = app(&src);
    // Fall to the floor.
    let mut landed = false;
    let mut best: f32 = 0.0;
    for _ in 0..240 {
        app.update();
        let (p, v) = nat(&mut app);
        if !landed && v.y.abs() < 1.0 && p.y < 2.0 * TILE {
            landed = true;
        }
        if p.y < 2.0 * TILE && v.y > 0.0 {
            landed = true;
        }
        if landed {
            best = best.max(p.y);
        }
    }
    best
}

#[test]
fn a_bounce_tweak_springs_nat_back_up() {
    let plain = rebound("");
    let spring = rebound(&("\n".repeat(9) + &"a".repeat(24) + "\n"));
    assert!(spring > plain + 3.0 * TILE, "a trampoline floor springs him well up: {spring} vs {plain}");
}

#[test]
fn a_belt_carries_nat() {
    let src = "name: T\ntweak a: belt=4\ntweak b: gravity=0.3\n---\n..........................\n..........................\n..........................\n..........................\n..P..................G....\n##########################\n---\n..........................\n..........................\n..........................\n..........................\n..........................\naaaaaaaaaaaa..............\n";
    let mut app = app(src);
    step(&mut app, 0.3);
    let (a, _) = nat(&mut app);
    step(&mut app, 1.0);
    let (b, _) = nat(&mut app);
    assert!(b.x - a.x > 3.0 * TILE, "the belt carries him: {} px", b.x - a.x);
}

#[test]
fn levels_round_trip_through_the_editor_doc() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("levels");
    let mut n = 0;
    for dir in [root.clone(), root.join("side-b"), root.join("custom")] {
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "txt") {
                let text = std::fs::read_to_string(&p).unwrap();
                let doc = LevelDoc::parse(&text).unwrap();
                assert_eq!(doc.text(), text, "{} round-trips", p.display());
                n += 1;
            }
        }
    }
    assert!(n >= 17);
}

/// How far Nat falls in 0.4 s from high up, with the air painted `layer`.
fn fall(layer: &str) -> f32 {
    let rows = "..P.................\n".to_string() + &"....................\n".repeat(12) + "..................G.\n####################\n";
    let mut app = app(&format!("name: T\ntweak b: gravity=0.3\n---\n{rows}---\n{layer}"));
    let (a, _) = nat(&mut app);
    step(&mut app, 0.4);
    a.y - nat(&mut app).0.y
}

#[test]
fn low_gravity_floats() {
    let plain = fall("");
    let floaty = fall(&"bbbbbbbbbbbbbbbbbbbb\n".repeat(14));
    assert!(floaty < plain * 0.5, "low gravity: fell {floaty} px vs {plain}");
}
