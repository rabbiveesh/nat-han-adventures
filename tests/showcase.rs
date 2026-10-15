//! The showcase levels and the scripted runs their demo videos record:
//! - the secret gadget gauntlet (`levels/secret/gauntlet.txt`, `scripts/tours/gauntlet.timeline`):
//!   every gadget tile in one hard level, beaten without a death;
//! - Tilt! (`levels/solo/tilt.txt`, `scripts/tours/tilt.timeline`): Nat alone (`solo: nat`) in
//!   a pinball machine of rubber ducks with no ground, two wrong doors and then the right one;
//! - Up the Spout! (`levels/solo/spout.txt`, `scripts/tours/spout.timeline`): Nat alone up a
//!   tower: a duck shaft through soap bricks past a trap door, fans, a beat bridge and a duck
//!   chimney to toot up.
//!
//! The runs play here the way the capture plays them (the keyboard-message path, 30 fps, the
//! music in lockstep driving the beat blocks and beat crushers).
//!
//! For writing a timeline: `SHOWCASE_TRACE=1` prints where Nat is every quarter second, the
//! bar lines and the crushers near him; `SHOWCASE_LEVEL=<file>` / `SHOWCASE_TOUR=<file>` play
//! another level / timeline in `the_tour_beats_the_gauntlet_without_a_death` (`SHOWCASE_EVERY=n`:
//! trace every n frames), and
//! `SHOWCASE_CHECK=<file>` prints the validator's report on a level file.
#![cfg(feature = "capture")]

use bevy::{input::InputPlugin, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use nat_han_adventures::{
    audio::{self, capture::AudioCapture},
    capture::{
        frame_duration,
        timeline::{Script, TimelinePlayer, play_timeline},
    },
    events::PlayerDied,
    game::{Groove, Han, Player, Pos, crushers::CrusherBlock},
    level::{Level, Levels, TILE, Topic, validate},
    state::AppState,
};

const GAUNTLET: &str = include_str!("../levels/secret/gauntlet.txt");
const GAUNTLET_TOUR: &str = include_str!("../scripts/tours/gauntlet.timeline");
const TILT: &str = include_str!("../levels/solo/tilt.txt");
const TILT_TOUR: &str = include_str!("../scripts/tours/tilt.timeline");
const SPOUT: &str = include_str!("../levels/solo/spout.txt");
const SPOUT_TOUR: &str = include_str!("../scripts/tours/spout.timeline");
const FPS: u32 = 30;

#[derive(Resource, Default)]
struct Deaths(Vec<(u64, Vec2)>);

#[derive(Resource, Default)]
struct Frame(u64);

/// What a run did: its deaths (s, col, row), when it reached the goal (s), whether Han was in
/// the level.
struct Run {
    deaths: Vec<(f32, f32, f32)>,
    goal: Option<f32>,
    end: f32,
    han: bool,
}

fn play(level: &str, tour: &str) -> Run {
    let level = Level::parse(level).unwrap();
    let rows = level.height as f32;
    let timeline = Script::parse(tour).unwrap().compile(FPS);
    let wav = std::env::temp_dir().join(format!("nathan-showcase-{}-{}.wav", std::process::id(), level.name.len()));
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin, InputPlugin, nat_han_adventures::gameplay))
        .insert_resource(TimeUpdateStrategy::ManualDuration(frame_duration(FPS).unwrap()))
        .insert_resource(Levels(vec![level]))
        .init_resource::<Deaths>()
        .init_resource::<Frame>()
        .add_systems(Last, |mut d: ResMut<Deaths>, f: Res<Frame>, mut r: MessageReader<PlayerDied>| {
            d.0.extend(r.read().map(|m| (f.0, m.pos)))
        });
    app.add_plugins(audio::plugin).insert_resource(AudioCapture::new(&wav, FPS).unwrap());
    app.finish();
    app.cleanup();
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::Playing);
    app.update();
    app.world_mut().resource_mut::<AudioCapture>().recording = true;
    app.insert_resource(TimelinePlayer::new(timeline.clone()))
        .add_systems(PreUpdate, play_timeline.before(bevy::input::InputSystems));
    let trace = std::env::var("SHOWCASE_TRACE").is_ok();
    let every = std::env::var("SHOWCASE_EVERY").ok().and_then(|e| e.parse().ok()).unwrap_or(FPS as u64 / 4);
    let secs = |f: u64| f as f32 / FPS as f32;
    let mut goal = None;
    let mut last_bar = None;
    let mut han = false;
    for frame in 0..timeline.frames {
        app.world_mut().resource_mut::<Frame>().0 = frame;
        app.update();
        han |= app.world_mut().query_filtered::<(), With<Han>>().iter(app.world()).next().is_some();
        let state = app.world().resource::<State<AppState>>().get().clone();
        if goal.is_none() && state == AppState::LevelComplete {
            goal = Some(secs(frame));
        }
        if !trace {
            continue;
        }
        let bar = app.world().resource::<Groove>().clock.bar;
        if last_bar != Some(bar) {
            eprintln!("t={:5.2} -- bar {bar}", secs(frame));
            last_bar = Some(bar);
        }
        let Some(p) = app.world_mut().query_filtered::<&Pos, With<Player>>().iter(app.world()).next().map(|p| p.0) else { continue };
        if frame % every == 0 {
            eprintln!("t={:5.2} col {:6.2} row {:6.2} {state:?}", secs(frame), p.x / TILE, rows - p.y / TILE);
        }
        if frame % 3 == 0 {
            let near: Vec<_> = app
                .world_mut()
                .query::<(&CrusherBlock, &Pos)>()
                .iter(app.world())
                .filter(|(_, c)| (p.x - c.0.x).abs() < 4.0 * TILE)
                .map(|(b, c)| format!("crusher@{},{} {:+.2}", b.def.col, b.def.row, (c.0 - b.home).length() / TILE))
                .collect();
            if !near.is_empty() {
                eprintln!("t={:5.2}   nat {:6.2},{:6.2}  {}", secs(frame), p.x / TILE, rows - p.y / TILE, near.join("  "));
            }
        }
    }
    let _ = std::fs::remove_file(&wav);
    let deaths = app.world().resource::<Deaths>().0.iter().map(|(f, p)| (secs(*f), p.x / TILE, rows - p.y / TILE)).collect();
    Run { deaths, goal, end: secs(timeline.frames), han }
}

#[test]
fn the_gauntlet_is_a_valid_level_with_every_gadget() {
    let level = Level::parse(GAUNTLET).unwrap();
    let report = validate::check(&level, &validate::Options { dump: false });
    assert!(report.errs.is_empty(), "{:#?}", report.errs);
    let topics: Vec<_> = report.lessons.iter().map(|l| l.topic).collect();
    for t in Topic::GADGETS {
        assert!(topics.contains(&t), "no {t:?} in the gauntlet: {topics:?}");
    }
}

#[test]
fn the_tour_beats_the_gauntlet_without_a_death() {
    let file = |var: &str, default: &str| std::env::var(var).map(|p| std::fs::read_to_string(p).unwrap()).unwrap_or(default.to_string());
    let run = play(&file("SHOWCASE_LEVEL", GAUNTLET), &file("SHOWCASE_TOUR", GAUNTLET_TOUR));
    assert!(run.deaths.is_empty(), "deaths (s, col, row): {:?}", run.deaths);
    let goal = run.goal.expect("the tour reaches the goal");
    assert!(run.end - goal < 6.0, "the tour ends soon after the goal (goal at {goal}s)");
}

/// Tilt! passes the full check: the level checker steers through the ducks ([`validate`]'s
/// pinball search) to the plunger and to the lures in the trap shaft and back out (a fixed
/// jump can't), and Nat alone needs no intro, checkpoint or Han.
#[test]
fn tilt_is_a_valid_level() {
    let level = Level::parse(TILT).unwrap();
    let report = validate::check(&level, &validate::Options { dump: false });
    assert!(report.errs.is_empty(), "{:#?}", report.errs);
}

/// Tilt!: no ground but the launch perch and the plunger's pedestal, only ducks. The tour
/// climbs A into its spikes, then takes the wide shaft's first door (spikes again), then sinks
/// to the bottom door and out onto the plunger. Nat alone: no Han.
#[test]
fn the_tour_finds_the_way_out_of_tilt_on_the_third_try() {
    let level = Level::parse(TILT).unwrap();
    assert!(level.solo);
    let run = play(TILT, TILT_TOUR);
    assert!(!run.han, "a solo level has no Han");
    let cols: Vec<_> = run.deaths.iter().map(|d| d.1 as i32).collect();
    assert!(matches!(cols[..], [7..=9, 35..=37]), "into A's spikes, then the first door's (s, col, row): {:?}", run.deaths);
    let goal = run.goal.expect("the third try reaches the goal");
    assert!(run.end - goal < 6.0, "the tour ends soon after the goal (goal at {goal}s)");
}

/// Up the Spout! passes the full check (the checker steers up both duck shafts).
#[test]
fn the_spout_is_a_valid_level() {
    let level = Level::parse(SPOUT).unwrap();
    assert!(level.solo);
    let report = validate::check(&level, &validate::Options { dump: false });
    assert!(report.errs.is_empty(), "{:#?}", report.errs);
}

/// Up the Spout!: the shaft carries Nat out of the trap door first, the second try toots past
/// it and climbs the rest of the tower.
#[test]
fn the_tour_climbs_the_spout_on_the_second_try() {
    let run = play(SPOUT, SPOUT_TOUR);
    let cols: Vec<_> = run.deaths.iter().map(|d| d.1 as i32).collect();
    assert!(matches!(cols[..], [1..=6]), "into the trap door's brushes (s, col, row): {:?}", run.deaths);
    let goal = run.goal.expect("the second try reaches the goal");
    assert!(run.end - goal < 6.0, "the tour ends soon after the goal (goal at {goal}s)");
}

#[test]
fn check_a_level_file() {
    let Ok(path) = std::env::var("SHOWCASE_CHECK") else { return };
    let level = Level::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    let report = validate::check(&level, &validate::Options { dump: std::env::var("SHOWCASE_DUMP").is_ok() });
    if let Some(d) = report.dump {
        eprintln!("{d}");
    }
    eprintln!("{:#?}", report.errs);
}
