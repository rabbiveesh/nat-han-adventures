//! The story's encore (`game::encore`), played headless: the last level's goal starts an
//! encore lap instead of the results card; calls are free; the ending knocks Nat into the
//! throne, Han dives and misses, FLUSH, and then the level is complete.

use std::time::Duration;

use bevy::{input::InputPlugin, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use leafwing_input_manager::prelude::*;
use nat_han_adventures::{
    events::*,
    game::encore::{CONTINUED_SECS, ENDING_TIMEOUT, Encore, EncorePhase, FLUSH_SECS, LAP_SECS},
    game::sideb::CallMode,
    game::{Han, HanAnim, HanPose, LevelRun, Player, Pos},
    level::{LEVEL_COUNT, Level, Levels},
    state::{AppState, CurrentLevel},
};

const DT: f64 = 1.0 / 60.0;

const STAGE: &str = "name: Stage
---
..............................
..............................
..............................
..............................
.P...........G................
##############################
";

fn app(level: usize) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin, InputPlugin))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(DT)))
        .add_plugins(nat_han_adventures::gameplay)
        .insert_resource(Levels(vec![Level::parse(STAGE).expect("test level parses"); LEVEL_COUNT]))
        .insert_resource(CurrentLevel(level));
    for counter in [count::<LevelCompleted>, count::<CallMode>, count::<FinalHit>] {
        counter(&mut app);
    }
    app.insert_resource(Cues::default()).add_systems(Last, |mut r: MessageReader<EncoreCue>, mut c: ResMut<Cues>| c.0.extend(r.read().copied()));
    app.finish();
    app.cleanup();
    app.update();
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::Playing);
    app.update();
    app.update();
    app
}

#[derive(Resource, Default)]
struct Cues(Vec<EncoreCue>);

#[derive(Resource)]
struct Count<M>(usize, std::marker::PhantomData<M>);

fn count<M: Message>(app: &mut App) {
    app.insert_resource(Count::<M>(0, default()))
        .add_systems(Last, |mut r: MessageReader<M>, mut c: ResMut<Count<M>>| c.0 += r.read().count());
}

fn counted<M: Message>(app: &App) -> usize {
    app.world().resource::<Count<M>>().0
}

fn step(app: &mut App, secs: f32) {
    for _ in 0..(secs as f64 / DT).round() as usize {
        app.update();
    }
}

/// Steps until the encore is in `want` (at most `secs`).
fn until(app: &mut App, want: EncorePhase, secs: f32) {
    for _ in 0..(secs as f64 / DT).round() as usize {
        if phase(app) == Some(want) {
            return;
        }
        app.update();
    }
    assert_eq!(phase(app), Some(want));
}

fn state(app: &App) -> AppState {
    *app.world().resource::<State<AppState>>().get()
}

fn phase(app: &App) -> Option<EncorePhase> {
    app.world().get_resource::<Encore>().map(|e| e.phase)
}

fn pos<C: Component>(app: &mut App) -> Vec2 {
    app.world_mut().query_filtered::<&Pos, With<C>>().single(app.world()).unwrap().0
}

fn tap(app: &mut App, key: KeyCode) {
    key.press(app.world_mut());
    app.update();
    key.release(app.world_mut());
    app.update();
}

#[test]
fn other_levels_end_at_the_goal_as_ever() {
    let mut app = app(0);
    KeyCode::ArrowRight.press(app.world_mut());
    step(&mut app, 3.0);
    assert_eq!(state(&app), AppState::LevelComplete);
    assert!(phase(&app).is_none());
}

#[test]
fn the_last_goal_starts_an_encore_that_ends_in_the_throne() {
    let mut app = app(LEVEL_COUNT - 1);
    KeyCode::ArrowRight.press(app.world_mut());
    step(&mut app, 1.5);
    assert_eq!(state(&app), AppState::Playing, "no results card yet");
    assert_eq!(phase(&app), Some(EncorePhase::Lap));
    let time_at_goal = app.world().resource::<Encore>().time_at_goal;
    // Running on: the stage's edge stops Nat (the level's end).
    step(&mut app, 2.0);
    KeyCode::ArrowRight.release(app.world_mut());
    let stage = app.world().resource::<Encore>().stage;
    assert!(pos::<Player>(&mut app).x <= stage.1 + 0.01);
    // Calls are free: every Down is one.
    for _ in 0..4 {
        tap(&mut app, KeyCode::ArrowDown);
    }
    assert_eq!(counted::<CallMode>(&app), 4);
    assert_eq!(app.world().resource::<Encore>().calls, 4);
    // The lap ends: the band's asked for its ending; with no band here, the knock comes anyway.
    until(&mut app, EncorePhase::Ending, LAP_SECS);
    until(&mut app, EncorePhase::Knocked, ENDING_TIMEOUT + 0.1);
    let han_from = pos::<Han>(&mut app);
    step(&mut app, 3.0);
    // Nat's in the bowl, and Han landed short of it.
    let e = app.world().resource::<Encore>().clone();
    assert!(matches!(e.phase, EncorePhase::Flushing | EncorePhase::Continued), "{:?}", e.phase);
    assert!(pos::<Player>(&mut app).distance(e.bowl()) < 20.0);
    let han = pos::<Han>(&mut app);
    assert!((han.x - e.bowl().x).abs() >= 16.0, "Han misses: {han} vs bowl {}", e.bowl());
    assert!(han.distance(han_from) > 1.0, "Han dived");
    let anim = *app.world_mut().query_filtered::<&HanAnim, With<Han>>().single(app.world()).unwrap();
    assert_ne!(anim.pose, HanPose::Run);
    step(&mut app, FLUSH_SECS + CONTINUED_SECS);
    assert_eq!(state(&app), AppState::LevelComplete);
    assert_eq!(counted::<LevelCompleted>(&app), 1);
    assert_eq!(app.world().resource::<LevelRun>().time, time_at_goal, "the results count up to the goal");
    let cues = &app.world().resource::<Cues>().0;
    assert_eq!(cues.first(), Some(&EncoreCue::Start));
    assert!(cues.contains(&EncoreCue::End));
    assert!(cues.iter().any(|c| matches!(c, EncoreCue::Knock { .. })));
    assert!(cues.iter().any(|c| matches!(c, EncoreCue::Flush { .. })));
    assert!(app.world().get_resource::<Encore>().is_none());
}

#[test]
fn the_bands_last_chord_knocks_nat_over() {
    let mut app = app(LEVEL_COUNT - 1);
    KeyCode::ArrowRight.press(app.world_mut());
    step(&mut app, 1.5);
    KeyCode::ArrowRight.release(app.world_mut());
    until(&mut app, EncorePhase::Ending, LAP_SECS);
    step(&mut app, 0.5);
    assert_eq!(phase(&app), Some(EncorePhase::Ending), "waiting for the last chord");
    app.world_mut().write_message(FinalHit);
    until(&mut app, EncorePhase::Knocked, 0.1);
}
