//! World -1 (Nat alone; pausing mid-toot opens a corrupted menu that rewrites the tile he
//! took off from), headless.

use std::time::Duration;

use leafwing_input_manager::prelude::*;
use bevy::{ecs::system::RunSystemOnce, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use nat_han_adventures::{
    audio::AudioOutput,
    game::{
        ActiveLevel, Han,
        minus1::{self, Freeze, GlitchMenu},
    },
    level::Tile,
    state::PlayState,
};

const DT: f64 = 1.0 / 60.0;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        StatesPlugin,
        bevy::input::InputPlugin,
        nat_han_adventures::gameplay,
        nat_han_adventures::audio::plugin,
    ))
    .insert_resource(AudioOutput::Headless)
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(DT)));
    app.update();
    app.world_mut().run_system_once(|mut c: Commands| minus1::start(&mut c, 0)).unwrap();
    app.update();
    app.update();
    app
}

fn tap(app: &mut App, k: KeyCode) {
    k.press(app.world_mut());
    app.update();
    k.release(app.world_mut());
    app.update();
}

fn steps(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn menu(app: &App) -> Option<GlitchMenu> {
    app.world().get_resource::<GlitchMenu>().cloned()
}

fn wait_menu(app: &mut App) -> GlitchMenu {
    for _ in 0..120 {
        if let Some(m) = menu(app) {
            return m;
        }
        app.update();
    }
    panic!("no corrupted menu");
}

fn count(app: &App, t: Tile) -> usize {
    app.world().resource::<ActiveLevel>().level.tiles.iter().filter(|&&x| x == t).count()
}

fn paused(app: &App) -> bool {
    *app.world().resource::<State<PlayState>>().get() == PlayState::Paused
}

#[test]
fn arrival_crashes_and_spikes_rewrite_the_ground_then_restart_is_clean() {
    let mut app = app();
    let m = wait_menu(&mut app);
    assert_eq!((m.freeze, m.target), (Freeze::Arrival, Tile::Solid), "uninitialized takeoff reads ground");
    steps(&mut app, 2);
    assert!(paused(&app));
    assert_eq!(app.world_mut().query_filtered::<(), With<Han>>().iter(app.world()).count(), 0, "no Han in World -1");

    let ground = count(&app, Tile::Solid);
    assert!(ground > 0);
    tap(&mut app, KeyCode::ArrowDown); // the spikes entry
    tap(&mut app, KeyCode::Enter);
    assert_eq!(count(&app, Tile::Solid), 0, "all the ground is spikes now");
    assert_eq!(count(&app, Tile::SpikesUp), ground);
    assert!(menu(&app).is_none());
    steps(&mut app, 2);
    assert!(!paused(&app));

    tap(&mut app, KeyCode::KeyR);
    steps(&mut app, 2);
    assert_eq!(count(&app, Tile::Solid), ground, "restart reloads the clean level");
    assert_eq!(wait_menu(&mut app).freeze, Freeze::Arrival, "and the arrival crash comes back");
    tap(&mut app, KeyCode::Enter); // R3$UM#: nothing written
    assert_eq!(count(&app, Tile::Solid), ground);
}

#[test]
fn pausing_mid_toot_opens_the_corrupted_menu_aimed_at_the_takeoff_tile() {
    let mut app = app();
    wait_menu(&mut app);
    tap(&mut app, KeyCode::Enter);
    steps(&mut app, 90); // land on the ground

    // A plain pause on the ground is the plain pause menu.
    tap(&mut app, KeyCode::Escape);
    steps(&mut app, 2);
    assert!(paused(&app) && menu(&app).is_none());
    // (The plain menu is UI, not loaded here: resume by hand.)
    app.world_mut().resource_mut::<NextState<PlayState>>().set(PlayState::Running);
    steps(&mut app, 2);
    assert!(!paused(&app));

    KeyCode::Space.press(app.world_mut());
    steps(&mut app, 8);
    KeyCode::Space.release(app.world_mut());
    steps(&mut app, 4);
    tap(&mut app, KeyCode::Space); // toot
    steps(&mut app, 3);
    tap(&mut app, KeyCode::Escape);
    let m = menu(&app).expect("mid-toot pause is corrupted");
    assert_eq!((m.freeze, m.target), (Freeze::MidToot, Tile::Solid));
    for _ in 0..3 {
        tap(&mut app, KeyCode::ArrowDown); // the blank entry
    }
    tap(&mut app, KeyCode::Enter);
    assert_eq!(count(&app, Tile::Solid), 0, "the ground is gone");
}


#[test]
fn every_world_minus_one_level_parses() {
    for n in 0..minus1::MINUS_ONE_SOURCES.len() {
        minus1::load(n);
    }
}

#[test]
fn splats_are_paint_rewriting_spikes_takes_them_too() {
    let (mut level, _) = minus1::load(2);
    let at = level.tiles.iter().position(|&t| t == Tile::SpikesUp).unwrap();
    level.tiles[at] = Tile::StainUp;
    assert!(minus1::rewrite(&mut level, Tile::SpikesUp, Tile::OneWay));
    assert!(!level.tiles.iter().any(|&t| matches!(t, Tile::SpikesUp | Tile::StainUp)));
    assert_eq!(level.tiles[at], Tile::OneWay);
}
