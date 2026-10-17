//! Share links (`level::share`): a level comes out of its link and plays, Side A or B, and a
//! shared level is done once you're back at the title.

use std::time::Duration;

use bevy::{ecs::system::RunSystemOnce, input::InputPlugin, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use nat_han_adventures::{
    game::{AssistMode, GeneratedLevel, playtest, sideb::SideB},
    level::{LEVEL_SOURCES, share},
    state::AppState,
};

const DT: f64 = 1.0 / 60.0;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin, InputPlugin))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(DT)))
        .add_plugins(nat_han_adventures::gameplay);
    app.finish();
    app.cleanup();
    app.update();
    app
}

/// Open `link` as the game does (the web's `#lvl=...`, or SHARED LEVEL on the title).
fn open(app: &mut App, link: &str) -> Result<(), String> {
    let text = share::decode(link)?;
    let r = app.world_mut().run_system_once(move |mut c: Commands| playtest::play_shared(&mut c, text.clone())).unwrap();
    for _ in 0..3 {
        app.update();
    }
    r
}

fn state(app: &App) -> AppState {
    *app.world().resource::<State<AppState>>().get()
}

#[test]
fn a_shared_story_level_plays_then_the_title_puts_the_story_back() {
    let mut app = app();
    open(&mut app, &share::link(LEVEL_SOURCES[2])).unwrap();
    assert!(state(&app) == AppState::Playing, "{:?}", state(&app));
    assert_eq!(app.world().resource::<GeneratedLevel>().0.name, nat_han_adventures::level::Level::parse(LEVEL_SOURCES[2]).unwrap().name);
    assert_eq!(*app.world().resource::<AssistMode>(), AssistMode::Manual, "what you play is the level");
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::Title);
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(state(&app), AppState::Title, "a shared level doesn't close the game");
    assert!(!app.world().contains_resource::<GeneratedLevel>());
    assert!(!app.world().contains_resource::<playtest::Playtest>());
    assert_eq!(*app.world().resource::<AssistMode>(), AssistMode::Story);
}

#[test]
fn a_shared_side_b_level_plays_as_side_b_with_its_solution_line_aside() {
    let mut app = app();
    let text = nat_han_adventures::game::sideb::SIDE_B_SOURCES[0].replacen("---", "solution: 7/giant\n---", 1);
    open(&mut app, &share::link(&text)).unwrap();
    assert!(app.world().contains_resource::<SideB>());
    assert!(state(&app) == AppState::Playing);
}

#[test]
fn a_broken_link_says_why_and_plays_nothing() {
    let mut app = app();
    let bad = share::link("name: nope\n---\nnot a level\n");
    assert!(open(&mut app, &bad).is_err());
    assert!(open(&mut app, "https://rabbiveesh.github.io/nat-han-adventures/#lvl=1@@").is_err());
    assert!(!app.world().contains_resource::<playtest::Playtest>());
}
