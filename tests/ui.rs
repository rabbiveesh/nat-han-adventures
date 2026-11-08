//! Headless smoke test of the UI flow: gameplay + UI on MinimalPlugins (no window, no disk).

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use nat_han_adventures::events::HanSays;
use nat_han_adventures::game::{Han, LevelRun};
use nat_han_adventures::save::Progress;
use nat_han_adventures::state::{AppState, CurrentLevel};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        StatesPlugin,
        bevy::input::InputPlugin,
        nat_han_adventures::gameplay,
        nat_han_adventures::ui::plugin,
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
    *app.world_mut().resource_mut::<LevelRun>() = LevelRun {
        nuggets: 3,
        nuggets_total: 5,
        time: 42.0,
        checkpoint: None,
        deaths: 2,
    };
    go(&mut app, AppState::LevelComplete);
    let p = app.world().resource::<Progress>();
    assert_eq!(p.unlocked, 2);
    assert_eq!(p.best_nuggets[0], Some(3));
    assert_eq!(p.best_time[0], Some(42.0));
}

fn bubbles(app: &mut App) -> usize {
    app.world_mut()
        .query_filtered::<(), With<Text2d>>()
        .iter(app.world())
        .count()
}

#[test]
fn speech_bubble_follows_han_and_tolerates_his_absence() {
    let mut app = app();
    // No Han: the line is dropped quietly.
    app.world_mut().write_message(HanSays {
        text: "Anyone there?".into(),
    });
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(bubbles(&mut app), 0);

    let han = app
        .world_mut()
        .spawn((Han, Transform::from_xyz(100.0, 50.0, 4.0)))
        .id();
    app.world_mut().write_message(HanSays {
        text: "Mind the brushes, kid. They bite!".into(),
    });
    app.update();
    app.update();
    assert_eq!(bubbles(&mut app), 1);

    // Han leaves: so does his bubble.
    app.world_mut().despawn(han);
    app.update();
    app.update();
    assert_eq!(bubbles(&mut app), 0);
}

/// No freezing: reaching a gate's hint spot teaches its counter (once per save) with a hint
/// line under the strip while the game runs on; the line goes once the counter fills (or
/// after `HINT_SECS`).
#[test]
fn a_hint_spot_teaches_its_counter_with_a_line() {
    use bevy::time::TimeUpdateStrategy;
    use nat_han_adventures::game::HintSpot;
    use nat_han_adventures::level::Topic;
    use nat_han_adventures::save::BandCounter;
    use nat_han_adventures::state::PlayState;
    use nat_han_adventures::ui::{HINT_SECS, Lesson, Lessons};

    let mut app = app();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_secs_f64(1.0 / 60.0),
    ));
    go(&mut app, AppState::LevelSelect);
    app.world_mut().resource_mut::<CurrentLevel>().0 = 0;
    go(&mut app, AppState::Playing);
    let play_state = |app: &App| *app.world().resource::<State<PlayState>>().get();
    // Nat hears the giant wall's hint.
    let mut q = app.world_mut().query::<&mut HintSpot>();
    let mut spot = q
        .iter_mut(app.world_mut())
        .find(|h| h.topics.contains(&Topic::Giant))
        .expect("level 1 teaches the giant wall");
    spot.said = true;
    app.update();
    app.update();
    assert_eq!(
        app.world().resource::<Lessons>().hint.map(|h| h.lesson),
        Some(Lesson::Counter(BandCounter::Horn))
    );
    assert!(app.world().resource::<Progress>().taught[BandCounter::Horn.index()]);
    // Not frozen: play time runs on.
    let t = app.world().resource::<LevelRun>().time;
    for _ in 0..60 {
        app.update();
    }
    assert_eq!(play_state(&app), PlayState::Running);
    assert!(app.world().resource::<LevelRun>().time > t + 0.5);
    // Nobody fills the horn: the line goes by itself.
    for _ in 0..((HINT_SECS + 1.0) * 60.0) as usize {
        app.update();
    }
    assert_eq!(app.world().resource::<Lessons>().hint, None);
}
