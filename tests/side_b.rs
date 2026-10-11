//! Side B (Han alone, the player calls the band from a set list), headless: the real gameplay
//! and the band (headless audio), Han walking Side B's levels on his own.

use std::time::Duration;

use bevy::{ecs::system::RunSystemOnce, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use leafwing_input_manager::prelude::*;
use nat_han_adventures::{
    audio::{AudioOutput, Harmony},
    events::HanSays,
    game::{
        Groove, Han, Pos,
        sideb::{self, Phase, SideB},
    },
    level::TILE,
    state::AppState,
};

const DT: f64 = 1.0 / 60.0;

#[derive(Resource, Default)]
struct Heard(Vec<String>);

fn app(level: usize) -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        StatesPlugin,
        bevy::input::InputPlugin,
        nat_han_adventures::gameplay,
        nat_han_adventures::audio::plugin,
    ))
    .insert_resource(AudioOutput::Headless)
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(DT)))
    .init_resource::<Heard>()
    .add_systems(Last, |mut h: ResMut<Heard>, mut s: MessageReader<HanSays>| h.0.extend(s.read().map(|m| m.text.clone())));
    app.update();
    app.world_mut().run_system_once(move |mut c: Commands| sideb::start(&mut c, level)).unwrap();
    app.update();
    app.update();
    app
}

fn side(app: &App) -> &SideB {
    app.world().resource::<SideB>()
}

fn han(app: &mut App) -> Vec2 {
    app.world_mut().query_filtered::<&Pos, With<Han>>().single(app.world()).unwrap().0
}

fn col(app: &mut App) -> f32 {
    han(app).x / TILE
}

fn trace(app: &mut App, t: f32) {
    if std::env::var("SIDE_B_TRACE").is_ok() {
        let (p, s, g) = (han(app), side(app).clone(), app.world().resource::<Groove>().harmony);
        eprintln!("t={t:5.2} han=({:.1},{:.1}) target={:?} frontier={} phase={:?} groove={g:?}", p.x / TILE, p.y / TILE, s.target, s.frontier, s.phase);
    }
}

/// Step until `done` (or `secs` pass); the seconds it took.
fn until(app: &mut App, secs: f32, mut done: impl FnMut(&mut App) -> bool) -> Option<f32> {
    let n = (secs as f64 / DT).round() as usize;
    for i in 0..n {
        app.update();
        if i % 15 == 0 {
            trace(app, i as f32 * DT as f32);
        }
        if done(app) {
            return Some(i as f32 * DT as f32);
        }
    }
    None
}

fn tap(app: &mut App, k: KeyCode) {
    k.press(app.world_mut());
    app.update();
    k.release(app.world_mut());
    app.update();
}

fn at_frontier(app: &mut App) -> bool {
    let s = side(app);
    s.frontier && s.barked.is_some() && s.barked == s.target
}

#[test]
fn level_one_parses_with_its_set_list() {
    let b = sideb::SideBLevel::load(0);
    assert_eq!(b.setlist, vec![(Harmony::Coltrane, 1), (Harmony::Quartal, 1)]);
}

#[test]
fn han_walks_to_the_wall_and_asks_for_music() {
    let mut app = app(0);
    assert_eq!(*app.world().resource::<State<AppState>>().get(), AppState::Playing);
    until(&mut app, 8.0, at_frontier).expect("Han stops at the giant wall");
    assert!((17.0..20.5).contains(&col(&mut app)), "at the wall's foot: col {}", col(&mut app));
    assert!(app.world().resource::<Heard>().0.iter().any(|s| s.contains("Too tall")));
}

#[test]
fn the_right_calls_get_han_to_the_goal() {
    let mut app = app(0);
    until(&mut app, 8.0, at_frontier).expect("Han stops at the giant wall");
    tap(&mut app, KeyCode::ArrowDown); // GIANT STEPS
    assert_eq!(side(&app).left, vec![0, 1]);
    until(&mut app, 25.0, |a| col(a) > 27.0 && a.world().resource::<Groove>().harmony == Harmony::Original)
        .expect("over the wall, and Giant Steps over");
    until(&mut app, 8.0, at_frontier).expect("Han stops at the long gap");
    tap(&mut app, KeyCode::ArrowRight);
    tap(&mut app, KeyCode::ArrowDown); // FIRED UP
    until(&mut app, 15.0, |a| matches!(side(a).phase, Phase::Cleared { .. })).expect("Han reaches the goal");
    assert_eq!(side(&app).rewinds, 0);
}

#[test]
fn spending_the_set_list_badly_rewinds_the_level() {
    let mut app = app(0);
    until(&mut app, 1.0, |_| false);
    // Both calls at the start: fired up overrides Giant Steps before the wall.
    tap(&mut app, KeyCode::ArrowDown);
    tap(&mut app, KeyCode::ArrowRight);
    tap(&mut app, KeyCode::ArrowDown);
    assert_eq!(side(&app).calls_left(), 0);
    until(&mut app, 40.0, |a| side(a).rewinds == 1).expect("stuck with nothing left: the level rewinds");
    assert_eq!(side(&app).left, vec![1, 1], "the set list comes back");
    assert!(col(&mut app) < 5.0, "Han's back at the start");
}

#[test]
fn han_hops_spike_alley_on_his_own() {
    let mut app = app(1);
    until(&mut app, 12.0, at_frontier).expect("Han gets through the spikes to the giant wall");
    assert!((34.0..38.5).contains(&col(&mut app)), "at the wall's foot: col {}", col(&mut app));
    assert_eq!(side(&app).rewinds, 0);
}

#[test]
fn giant_steps_too_early_strands_han_under_the_ceiling_spikes() {
    let mut app = app(1);
    until(&mut app, 6.0, |a| col(a) > 5.0).expect("Han heads into the alley");
    tap(&mut app, KeyCode::ArrowDown);
    // Low gravity: every hop over the next pit floats him into the ceiling spikes, so he waits.
    until(&mut app, 6.0, |a| a.world().resource::<Groove>().harmony == Harmony::Coltrane).unwrap();
    until(&mut app, 4.0, |_| false);
    assert!((15.0..18.5).contains(&col(&mut app)), "waiting short of the second pit: col {}", col(&mut app));
    // It wears off, he walks on, and at the wall there's nothing left to call: rewind.
    until(&mut app, 40.0, |a| side(a).rewinds == 1).expect("stuck at the wall, the level rewinds");
}

#[test]
fn spikes_kill_han_and_the_level_rewinds() {
    let mut app = app(1);
    until(&mut app, 1.0, |_| false);
    tap(&mut app, KeyCode::ArrowDown);
    assert_eq!(side(&app).calls_left(), 0);
    // Drop him on the first pit's spikes.
    let level = app.world().resource::<nat_han_adventures::game::ActiveLevel>().level.clone();
    let spot = level.tile_center(11, 11);
    let mut q = app.world_mut().query_filtered::<(&mut Pos, &mut nat_han_adventures::game::PrevPos, &mut nat_han_adventures::game::Body), With<Han>>();
    let (mut p, mut pp, mut body) = q.single_mut(app.world_mut()).unwrap();
    (p.0, pp.0, body.vel) = (spot, spot, Vec2::ZERO);
    until(&mut app, 0.2, |a| matches!(side(a).phase, Phase::Rewind { .. })).expect("spikes are deadly on Side B");
    assert!(app.world().resource::<Heard>().0.iter().any(|s| s == sideb::DEATH_LINE));
    until(&mut app, 3.0, |a| side(a).rewinds == 1).expect("the level rewinds");
    assert_eq!(side(&app).calls_left(), 1, "with the set list back");
}
