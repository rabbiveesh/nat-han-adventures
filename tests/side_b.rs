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
        sideb::{self, Fate, Phase, SetCall, SideB},
    },
    level::TILE,
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
        let gr = app.world().resource::<Groove>();
        let (b, ph) = (gr.bounce, gr.phrase);
        let melt = app.world().resource::<nat_han_adventures::audio::plugin::Director>().melt_until;
        let half = app.world_mut().query_filtered::<&nat_han_adventures::game::Body, With<Han>>().single(app.world()).unwrap().half;
        eprintln!("   bounce={b} phrase={ph:?} melt_until={melt:?} half={half:?}");
        eprintln!("t={t:5.2} han=({:.2},{:.2}) target={:?} frontier={} charging={:?} landings={} cx={:.2} phase={:?} groove={g:?}", p.x / TILE, p.y / TILE, s.target, s.frontier, s.charging, s.charge_landings, s.charge_x / TILE, s.phase);
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

fn rewound(app: &mut App) -> bool {
    matches!(side(app).phase, Phase::Rewind { .. })
}

fn fate(app: &App) -> Option<Fate> {
    side(app).fate
}

fn said(app: &App, line: &str) -> bool {
    app.world().resource::<Heard>().0.iter().any(|s| s == line)
}

#[test]
fn level_one_parses_with_its_set_list() {
    let b = sideb::SideBLevel::load(0);
    assert_eq!(b.setlist, vec![(SetCall::Mode(Harmony::Coltrane), 1), (SetCall::Mode(Harmony::Quartal), 1)]);
}

#[test]
fn with_no_music_han_charges_the_wall_and_knocks_himself_out() {
    let mut app = app(0);
    until(&mut app, 10.0, rewound).expect("Han charges the giant wall and comes off worst");
    assert_eq!(fate(&app), Some(Fate::Bonk));
    assert!(said(&app, "I can climb that. Probably!"));
    assert!(said(&app, Fate::Bonk.line()));
    until(&mut app, 3.0, |a| side(a).rewinds == 1).expect("the level rewinds");
    assert!(col(&mut app) < 5.0, "Han's back at the start");
}

#[test]
fn the_right_calls_in_time_get_han_to_the_goal() {
    let mut app = app(0);
    until(&mut app, 0.5, |_| false);
    tap(&mut app, KeyCode::ArrowDown); // GIANT STEPS, before he gets to the wall
    assert_eq!(side(&app).left, vec![0, 1]);
    until(&mut app, 10.0, |a| col(a) > 21.0).expect("up the wall");
    tap(&mut app, KeyCode::ArrowRight);
    tap(&mut app, KeyCode::ArrowDown); // FIRED UP, on top of the wall
    until(&mut app, 15.0, |a| matches!(side(a).phase, Phase::Cleared { .. }) || rewound(a)).unwrap();
    assert!(matches!(side(&app).phase, Phase::Cleared { .. }), "cleared, not {:?}", fate(&app));
}

#[test]
fn no_fired_up_for_the_gap_is_a_long_way_down() {
    let mut app = app(0);
    until(&mut app, 0.5, |_| false);
    tap(&mut app, KeyCode::ArrowDown);
    until(&mut app, 12.0, rewound).expect("Han leaps the gap anyway");
    assert_eq!(fate(&app), Some(Fate::Pit));
}

#[test]
fn han_hops_spike_alley_then_bonks_the_wall() {
    let mut app = app(1);
    until(&mut app, 12.0, rewound).unwrap();
    assert_eq!(fate(&app), Some(Fate::Bonk), "through the spikes, then the wall");
}

#[test]
fn giant_steps_in_spike_alley_floats_han_into_the_ceiling() {
    let mut app = app(1);
    until(&mut app, 6.0, |a| col(a) > 5.0).expect("Han heads into the alley");
    tap(&mut app, KeyCode::ArrowDown);
    until(&mut app, 12.0, rewound).unwrap();
    assert_eq!(fate(&app), Some(Fate::Spikes));
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
    until(&mut app, 0.2, rewound).expect("spikes are deadly on Side B");
    assert_eq!(fate(&app), Some(Fate::Spikes));
    until(&mut app, 3.0, |a| side(a).rewinds == 1).expect("the level rewinds");
    assert_eq!(side(&app).calls_left(), 1, "with the set list back");
}

#[test]
fn melting_han_ducks_through_the_drip_pipe() {
    let mut app = app(2);
    until(&mut app, 0.3, |_| false);
    tap(&mut app, KeyCode::ArrowDown); // MELT
    until(&mut app, 15.0, |a| matches!(side(a).phase, Phase::Cleared { .. }) || rewound(a)).unwrap();
    assert!(matches!(side(&app).phase, Phase::Cleared { .. }), "cleared, not {:?}", fate(&app));
}

#[test]
fn full_size_han_loses_his_cap_in_the_drip_pipe() {
    let mut app = app(2);
    until(&mut app, 10.0, rewound).unwrap();
    assert_eq!(fate(&app), Some(Fate::Spikes));
    assert!(said(&app, "Duck and run! Duck and run!"));
}
