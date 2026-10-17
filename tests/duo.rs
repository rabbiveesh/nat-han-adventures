//! The double game (`game::duo`), headless: Nat played by a timeline (the capture's format,
//! `scripts/tours/duo-1.timeline`, the same one the video records), Han running by himself.
//! `DUO_TRACE=1` prints where everyone is, every half second.
#![cfg(feature = "capture")]

use std::time::Duration;

use bevy::{input::InputPlugin, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use leafwing_input_manager::prelude::*;
use nat_han_adventures::{
    capture::timeline::Script,
    events::{HanSays, PlayerDied},
    game::{
        Han, Player, Pos,
        duo::{self, DeathRule, Duo},
    },
    level::TILE,
    state::AppState,
};

const DT: f64 = 1.0 / 60.0;
const TOUR: &str = include_str!("../scripts/tours/duo-1.timeline");

#[derive(Resource, Default, Debug)]
struct Log {
    nat_deaths: Vec<(f32, Vec2)>,
    says: Vec<(f32, String)>,
    t: f32,
}

fn log(mut l: ResMut<Log>, mut d: MessageReader<PlayerDied>, mut s: MessageReader<HanSays>) {
    let t = l.t;
    let deaths: Vec<_> = d.read().map(|d| (t, d.pos)).collect();
    l.nat_deaths.extend(deaths);
    let says: Vec<_> = s.read().map(|s| (t, s.text.clone())).collect();
    l.says.extend(says);
    l.t += DT as f32;
}

fn app(rule: DeathRule) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin, InputPlugin))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(DT)))
        .add_plugins((nat_han_adventures::gameplay, nat_han_adventures::audio::plugin))
        .insert_resource(nat_han_adventures::audio::AudioOutput::Headless)
        .init_resource::<Log>()
        .add_systems(Last, log);
    app.finish();
    app.cleanup();
    // As `NATHAN_DUO=1` does at startup (so the timeline lines up with the video's frames).
    duo::start(&mut app.world_mut().commands(), 0, rule);
    app.world_mut().flush();
    app
}

fn pos<C: Component>(app: &mut App) -> Vec2 {
    app.world_mut().query_filtered::<&Pos, With<C>>().single(app.world()).map_or(Vec2::NAN, |p| p.0)
}

fn cell(p: Vec2) -> String {
    format!("({:.1},{:.1})", p.x / TILE, 18.0 - p.y / TILE)
}

/// Play the tour; returns the app at its end.
fn play(rule: DeathRule) -> App {
    let mut app = app(rule);
    let tl = Script::parse(TOUR).expect("tour parses").compile(60);
    let trace = std::env::var("DUO_TRACE").is_ok();
    let trace_every = std::env::var("DUO_TRACE").ok().and_then(|v| v.parse().ok()).unwrap_or(30);
    let mut next = 0;
    for f in 0..tl.frames + 60 {
        while next < tl.events.len() && tl.events[next].frame == f {
            let e = tl.events[next];
            if e.pressed {
                e.key.press(app.world_mut());
            } else {
                e.key.release(app.world_mut());
            }
            next += 1;
        }
        app.update();
        if trace && f % trace_every == 0 {
            let (n, h) = (pos::<Player>(&mut app), pos::<Han>(&mut app));
            let d = app.world().get_resource::<Duo>().cloned();
            let state = *app.world().resource::<State<AppState>>().get();
            println!(
                "t={:5.1} nat={} han={} view={:.1} han_down={:?} {state:?} {:?}",
                f as f32 / 60.0,
                cell(n),
                cell(h),
                d.as_ref().map_or(0.0, |d| d.view_x / TILE),
                d.as_ref().and_then(|d| d.han_down),
                app.world().resource::<nat_han_adventures::game::Groove>().harmony
            );
            if std::env::var("DUO_BRAIN").is_ok() {
                let b =
                    app.world_mut().query_filtered::<&nat_han_adventures::game::HanBrain, With<Han>>().single(app.world()).unwrap().clone();
                println!("    mode={:?} charge={:?}", b.mode, b.charge);
            }
        }
        if *app.world().resource::<State<AppState>>().get() == AppState::LevelComplete {
            break;
        }
    }
    if trace {
        let l = app.world().resource::<Log>();
        println!("nat deaths: {:?}", l.nat_deaths.iter().map(|(t, p)| format!("{t:.1}s {}", cell(*p))).collect::<Vec<_>>());
        println!("says: {:?}", l.says);
    }
    app
}

/// The tour clears the level with separate deaths, Han never dying, Nat never dying.
#[test]
fn the_tour_clears_upstairs_downstairs() {
    let mut app = play(DeathRule::Separate);
    let state = *app.world().resource::<State<AppState>>().get();
    let deaths = app.world().resource::<Log>().nat_deaths.len();
    let han_deaths = app.world().get_resource::<Duo>().map_or(0, |d| d.han_deaths);
    assert_eq!(state, AppState::LevelComplete, "Nat at {}", cell(pos::<Player>(&mut app)));
    assert_eq!((deaths, han_deaths), (0, 0));
}

/// Without a call, Han dies at his first gate (the pit), and comes back where he jumped.
#[test]
fn han_needs_the_call() {
    let mut app = app(DeathRule::Separate);
    KeyCode::ArrowRight.press(app.world_mut());
    let mut died = false;
    for _ in 0..60 * 12 {
        app.update();
        died |= app.world().resource::<Duo>().han_deaths > 0;
    }
    assert!(died, "Han at {}", cell(pos::<Han>(&mut app)));
}

/// Both rewinding: a death sends both back (Nat left behind: Han's back at his start too).
#[test]
fn both_rewind_together() {
    let mut app = app(DeathRule::Both);
    for _ in 0..60 * 4 {
        app.update();
    }
    assert!(!app.world().resource::<Log>().nat_deaths.is_empty(), "Nat left behind");
    for _ in 0..60 * 2 {
        app.update();
    }
    let h = pos::<Han>(&mut app);
    assert!(h.x < 12.0 * TILE, "han {}", cell(h));
}
