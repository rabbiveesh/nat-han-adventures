//! The crutch pit (`game::crutches`), played headless: a `crutchpit:` gap breaks Nat's legs
//! instead of splatting him; Han's plunger hauls him to the ledge before it, the hospital hands
//! him crutches, and on them he can't toot, walks slow and jumps small, till a death heals him.

use std::time::Duration;

use bevy::{input::InputPlugin, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use leafwing_input_manager::prelude::*;
use nat_han_adventures::{
    events::{HanSays, Jumped, PlayerDied},
    game::crutches::{CAREFUL_LINE, CrutchRescue, OnCrutches, RescuePhase},
    game::{Body, LevelRun, Player, Pos},
    level::{LEVEL_COUNT, Level, Levels, TILE},
    state::{AppState, CurrentLevel},
};

const DT: f64 = 1.0 / 60.0;

/// A crutch pit at 8-9, a flat run after it, and a spike to die on at the far end.
const PIT: &str = "name: Pit
crutchpit: 8 9
---
..............................
..............................
..............................
..............................
.P.........................^G.
########..####################
";

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin, InputPlugin))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(DT)))
        .add_plugins(nat_han_adventures::gameplay)
        .insert_resource(Levels(vec![Level::parse(PIT).expect("test level parses"); LEVEL_COUNT]))
        .insert_resource(CurrentLevel(0))
        .insert_resource(Log::default())
        .add_systems(Last, |mut r: MessageReader<HanSays>, mut l: ResMut<Log>| l.said.extend(r.read().map(|m| m.text.clone())))
        .add_systems(Last, |mut r: MessageReader<PlayerDied>, mut l: ResMut<Log>| l.died += r.read().count())
        .add_systems(Last, |mut r: MessageReader<Jumped>, mut l: ResMut<Log>| l.toots += r.read().filter(|j| j.double).count());
    app.finish();
    app.cleanup();
    app.update();
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::Playing);
    app.update();
    app.update();
    app
}

#[derive(Resource, Default)]
struct Log {
    said: Vec<String>,
    died: usize,
    toots: usize,
}

fn step(app: &mut App, secs: f32) {
    for _ in 0..(secs as f64 / DT).round() as usize {
        app.update();
    }
}

fn rescue(app: &App) -> Option<CrutchRescue> {
    app.world().get_resource::<CrutchRescue>().cloned()
}

fn nat(app: &mut App) -> (Vec2, Body, bool) {
    let mut q = app.world_mut().query_filtered::<(&Pos, &Body, Has<OnCrutches>), With<Player>>();
    let (p, b, c) = q.single(app.world()).unwrap();
    (p.0, b.clone(), c)
}

/// Runs into the pit and waits out the rescue.
fn break_legs(app: &mut App) {
    KeyCode::ArrowRight.press(app.world_mut());
    for _ in 0..240 {
        app.update();
        if rescue(app).is_some() {
            break;
        }
    }
    KeyCode::ArrowRight.release(app.world_mut());
    assert_eq!(rescue(app).map(|r| r.phase), Some(RescuePhase::Ouch), "landed in the crutch pit");
    for _ in 0..900 {
        if rescue(app).is_none() {
            return;
        }
        app.update();
    }
    panic!("the rescue never ended");
}

#[test]
fn the_crutch_pit_breaks_legs_not_nat() {
    let mut app = app();
    KeyCode::ArrowRight.press(app.world_mut());
    let mut phases = Vec::new();
    for _ in 0..900 {
        app.update();
        if let Some(r) = rescue(&app) {
            if phases.last() != Some(&r.phase) {
                phases.push(r.phase);
            }
            KeyCode::ArrowRight.release(app.world_mut());
        } else if !phases.is_empty() {
            break;
        }
    }
    use RescuePhase::*;
    assert_eq!(phases, vec![Ouch, Lower, Stuck, Haul, Hospital]);
    let (p, _, crutches) = nat(&mut app);
    assert!(crutches, "on crutches");
    assert!(p.x < 8.0 * TILE && p.y > TILE, "on the ledge before the pit: {p}");
    let log = app.world().resource::<Log>();
    assert_eq!(log.died, 0);
    assert_eq!(app.world().resource::<LevelRun>().deaths, 0);
    assert!(log.said.iter().any(|s| s == CAREFUL_LINE));
}

#[test]
fn on_crutches_no_toot_slow_and_small_jumps() {
    let mut app = app();
    break_legs(&mut app);
    // Slow: half speed at most.
    KeyCode::ArrowLeft.press(app.world_mut());
    step(&mut app, 0.6);
    let (_, b, _) = nat(&mut app);
    assert!(b.vel.x.abs() <= 75.0 + 0.1 && b.vel.x.abs() > 50.0, "hobbling: {}", b.vel.x);
    KeyCode::ArrowLeft.release(app.world_mut());
    step(&mut app, 0.5);
    // A jump, then a toot that doesn't come.
    let y0 = nat(&mut app).0.y;
    KeyCode::Space.press(app.world_mut());
    let mut top = y0;
    for i in 0..40 {
        app.update();
        if i == 18 {
            KeyCode::Space.release(app.world_mut());
            app.update();
            KeyCode::Space.press(app.world_mut());
        }
        top = top.max(nat(&mut app).0.y);
    }
    KeyCode::Space.release(app.world_mut());
    assert_eq!(app.world().resource::<Log>().toots, 0, "no tooting on crutches");
    // The ground jump's apex is ~51px; on crutches about 0.64 of it.
    assert!(top - y0 < 40.0 && top - y0 > 20.0, "a small hop: {}", top - y0);
}

#[test]
fn a_death_heals() {
    let mut app = app();
    break_legs(&mut app);
    assert!(nat(&mut app).2);
    // Past the pit (set down by hand), hobbling into the spike at the end.
    let at = Vec2::new(24.5 * TILE, TILE + Body::player().half.y);
    let mut q = app.world_mut().query_filtered::<(&mut Pos, &mut nat_han_adventures::game::PrevPos), With<Player>>();
    let (mut p, mut prev) = q.single_mut(app.world_mut()).unwrap();
    (p.0, prev.0) = (at, at);
    KeyCode::ArrowRight.press(app.world_mut());
    for _ in 0..600 {
        app.update();
        if app.world().resource::<Log>().died > 0 {
            break;
        }
    }
    KeyCode::ArrowRight.release(app.world_mut());
    assert_eq!(app.world().resource::<Log>().died, 1, "splat on the spike");
    step(&mut app, 1.5);
    assert!(!nat(&mut app).2, "healed by the respawn");
}
