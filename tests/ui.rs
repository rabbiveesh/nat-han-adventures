//! Headless smoke test of the UI flow: gameplay + UI on MinimalPlugins (no window, no disk).

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use durhay::events::GusSays;
use durhay::game::{Gus, LevelRun};
use durhay::save::Progress;
use durhay::state::{AppState, CurrentLevel};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        StatesPlugin,
        bevy::input::InputPlugin,
        durhay::gameplay,
        durhay::ui::plugin,
    ));
    app.update();
    app
}

fn go(app: &mut App, s: AppState) {
    app.world_mut().resource_mut::<NextState<AppState>>().set(s);
    app.update();
    app.update();
}

#[test]
fn screens_cycle_without_panicking() {
    let mut app = app();
    for s in [
        AppState::Title,
        AppState::LevelSelect,
        AppState::Playing,
        AppState::LevelComplete,
        AppState::Victory,
        AppState::Title,
    ] {
        go(&mut app, s);
        assert_eq!(*app.world().resource::<State<AppState>>().get(), s);
    }
}

#[test]
fn level_complete_records_progress() {
    let mut app = app();
    go(&mut app, AppState::LevelSelect);
    app.world_mut().resource_mut::<CurrentLevel>().0 = 0;
    go(&mut app, AppState::Playing);
    *app.world_mut().resource_mut::<LevelRun>() =
        LevelRun { nuggets: 3, nuggets_total: 5, time: 42.0, checkpoint: None, deaths: 2 };
    go(&mut app, AppState::LevelComplete);
    let p = app.world().resource::<Progress>();
    assert_eq!(p.unlocked, 2);
    assert_eq!(p.best_nuggets[0], Some(3));
    assert_eq!(p.best_time[0], Some(42.0));
}

fn bubbles(app: &mut App) -> usize {
    app.world_mut().query_filtered::<(), With<Text2d>>().iter(app.world()).count()
}

#[test]
fn speech_bubble_follows_han_and_tolerates_his_absence() {
    let mut app = app();
    // No Han: the line is dropped quietly.
    app.world_mut().write_message(GusSays { text: "Anyone there?".into() });
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(bubbles(&mut app), 0);

    let han = app.world_mut().spawn((Gus, Transform::from_xyz(100.0, 50.0, 4.0))).id();
    app.world_mut().write_message(GusSays { text: "Mind the brushes, kid. They bite!".into() });
    app.update();
    app.update();
    assert_eq!(bubbles(&mut app), 1);

    // Han leaves: so does his bubble.
    app.world_mut().despawn(han);
    app.update();
    app.update();
    assert_eq!(bubbles(&mut app), 0);
}
