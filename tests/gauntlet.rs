//! The secret gadget gauntlet (`levels/secret/gauntlet.txt`): every gadget tile in one
//! hard level, and the scripted run through it that the demo video records
//! (`scripts/tours/gauntlet.timeline`). The run plays here the way the capture plays it (the
//! keyboard-message path, 30 fps, the music in lockstep driving the beat blocks and beat
//! crushers) and must reach the goal without a death.
//!
//! `GAUNTLET_TRACE=1` prints where Nat is every quarter second (for writing the timeline).
#![cfg(feature = "capture")]

use bevy::{input::InputPlugin, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use nat_han_adventures::{
    audio::{self, capture::AudioCapture},
    capture::{
        frame_duration,
        timeline::{Script, TimelinePlayer, play_timeline},
    },
    events::PlayerDied,
    game::{Groove, Player, Pos, crushers::CrusherBlock},
    level::{Level, Levels, TILE, Topic, validate},
    state::AppState,
};

const GAUNTLET: &str = include_str!("../levels/secret/gauntlet.txt");
const TOUR: &str = include_str!("../scripts/tours/gauntlet.timeline");
const FPS: u32 = 30;

#[derive(Resource, Default)]
struct Deaths(Vec<(u64, Vec2)>);

#[derive(Resource, Default)]
struct Frame(u64);

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
    let level = Level::parse(GAUNTLET).unwrap();
    let rows = level.height as f32;
    let timeline = Script::parse(TOUR).unwrap().compile(FPS);
    let wav = std::env::temp_dir().join(format!("nathan-gauntlet-{}.wav", std::process::id()));
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
    let trace = std::env::var("GAUNTLET_TRACE").is_ok();
    let mut done_at = None;
    let mut last_bar = None;
    for frame in 0..timeline.frames {
        app.world_mut().resource_mut::<Frame>().0 = frame;
        app.update();
        let state = app.world().resource::<State<AppState>>().get().clone();
        let bar = app.world().resource::<Groove>().clock.bar;
        if trace && last_bar != Some(bar) {
            eprintln!("t={:5.2} -- bar {bar}", frame as f32 / FPS as f32);
            last_bar = Some(bar);
        }
        if trace && frame % (FPS as u64 / 4) == 0 {
            let p = app.world_mut().query_filtered::<&Pos, With<Player>>().iter(app.world()).next().map(|p| p.0);
            if let Some(p) = p {
                eprintln!("t={:5.2} col {:6.2} row {:6.2} {state:?}", frame as f32 / FPS as f32, p.x / TILE, rows - p.y / TILE);
            }
        }
        if trace && frame % 3 == 0 {
            let p = app.world_mut().query_filtered::<&Pos, With<Player>>().iter(app.world()).next().map(|p| p.0);
            let near: Vec<_> = app
                .world_mut()
                .query::<(&CrusherBlock, &Pos)>()
                .iter(app.world())
                .filter(|(_, c)| p.is_some_and(|p| (p.x - c.0.x).abs() < 4.0 * TILE))
                .map(|(b, c)| format!("crusher@{},{} {:+.2}", b.def.col, b.def.row, (c.0 - b.home).length() / TILE))
                .collect();
            if !near.is_empty() {
                let p = p.unwrap();
                eprintln!("t={:5.2}   nat {:6.2},{:6.2}  {}", frame as f32 / FPS as f32, p.x / TILE, rows - p.y / TILE, near.join("  "));
            }
        }
        if done_at.is_none() && state == AppState::LevelComplete {
            done_at = Some(frame);
        }
    }
    let _ = std::fs::remove_file(&wav);
    let deaths = &app.world().resource::<Deaths>().0;
    let at = |f: u64| f as f32 / FPS as f32;
    assert!(deaths.is_empty(), "deaths: {:?}", deaths.iter().map(|(f, p)| (at(*f), p.x / TILE, rows - p.y / TILE)).collect::<Vec<_>>());
    let done = done_at.expect("the tour reaches the goal");
    assert!(timeline.frames - done < 6 * FPS as u64, "the tour ends soon after the goal (goal at {}s)", at(done));
}
