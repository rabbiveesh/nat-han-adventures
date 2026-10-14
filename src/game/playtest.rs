//! Playtesting a level file: `NATHAN_LEVEL=levels/custom/mine.txt cargo run` (native; a number
//! there is still a story level), or the
//! level editor's Play button. The file is played straight away (Side B if it has a `setlist:`
//! line), with no adaptive assists ([`AssistMode::Manual`]: what you play is the level), and is
//! reloaded whenever it changes on disk, so saving in the editor restarts it in the game.
//! Reaching the goal prints a `playtest: cleared ...` line (the editor reads it) and Jump or R
//! plays again; quitting to the title closes the game.

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::sideb::{self, SideB, SideBLevel};
use super::{AssistMode, GeneratedLevel, LevelRun, RestartLevel};
use crate::events::{HanSays, LevelCompleted, PlayerDied};
use crate::input::Action;
use crate::level::Level;
use crate::state::{AppState, CurrentLevel};

/// A level file being playtested. Exists while it is.
#[derive(Resource, Debug, Clone)]
pub struct Playtest {
    pub path: std::path::PathBuf,
    /// The file's text as last loaded.
    pub text: String,
    poll: f32,
}

/// What a playtest file holds.
pub enum Loaded {
    SideA(Level),
    SideB(SideBLevel),
}

/// Parse a level file: Side B if it has a `setlist:` line.
pub fn load_text(text: &str) -> Result<Loaded, String> {
    let side_b = text.lines().take_while(|l| l.trim() != "---").any(|l| l.split_once(':').is_some_and(|(k, _)| k.trim() == "setlist"));
    if side_b { SideBLevel::parse(&strip_solution(text)).map(Loaded::SideB) } else { Level::parse(text).map(Loaded::SideA) }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Startup, from_env)
        .add_systems(Update, (reload, again.run_if(in_state(AppState::LevelComplete))).run_if(resource_exists::<Playtest>))
        .add_systems(OnEnter(AppState::LevelComplete), cleared.run_if(resource_exists::<Playtest>))
        .add_systems(OnEnter(AppState::Title), quit.run_if(resource_exists::<Playtest>))
        .add_systems(Update, count_deaths.run_if(resource_exists::<Playtest>));
}

/// The level file `NATHAN_LEVEL` names (native; a number there is a story level instead:
/// `game::visuals`).
pub fn file_from_env() -> Option<String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("NATHAN_LEVEL").ok().filter(|v| v.parse::<usize>().is_err())
    }
    #[cfg(target_arch = "wasm32")]
    None
}

/// `NATHAN_LEVEL=<file>`.
#[allow(unused_mut, unused_variables)]
fn from_env(mut commands: Commands) {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(path) = file_from_env() {
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("NATHAN_LEVEL={path}: {e}"));
        let loaded = load_text(&text).unwrap_or_else(|e| panic!("NATHAN_LEVEL={path}: {e}"));
        commands.insert_resource(Playtest { path: path.into(), text, poll: 0.0 });
        start(&mut commands, loaded);
    }
}

/// Play `loaded` from the top.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
fn start(commands: &mut Commands, loaded: Loaded) {
    match loaded {
        Loaded::SideA(level) => {
            commands.remove_resource::<SideB>();
            commands.insert_resource(GeneratedLevel(level));
            commands.insert_resource(AssistMode::Manual);
            commands.insert_resource(CurrentLevel(0));
            commands.queue(|w: &mut World| w.resource_mut::<NextState<AppState>>().set(AppState::Playing));
        }
        Loaded::SideB(b) => sideb::start_with(commands, 0, b),
    }
}

/// The file changed on disk: load it again and start over (a broken file is skipped, with a
/// note on the console).
fn reload(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut pt: ResMut<Playtest>,
    state: Res<State<AppState>>,
    mut restart: MessageWriter<RestartLevel>,
    mut says: MessageWriter<HanSays>,
) {
    pt.poll -= time.delta_secs();
    if pt.poll > 0.0 {
        return;
    }
    pt.poll = 0.4;
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Ok(text) = std::fs::read_to_string(&pt.path) else { return };
        if text == pt.text {
            return;
        }
        pt.text = text.clone();
        match load_text(&text) {
            Ok(loaded) => {
                println!("playtest: reloaded {}", pt.path.display());
                let playing = *state.get() == AppState::Playing;
                match loaded {
                    Loaded::SideA(level) if playing => {
                        commands.insert_resource(GeneratedLevel(level));
                        restart.write(RestartLevel);
                    }
                    Loaded::SideB(b) if playing => {
                        commands.insert_resource(SideB::new(0, b.setlist));
                        commands.insert_resource(GeneratedLevel(b.level));
                        restart.write(RestartLevel);
                    }
                    loaded => start(&mut commands, loaded),
                }
            }
            Err(e) => {
                println!("playtest: {} doesn't parse: {e}", pt.path.display());
                says.write(HanSays { text: "Huh? Broken!".into() });
            }
        }
    }
    let _ = (&mut commands, &state, &mut restart, &mut says);
}

#[derive(Resource, Default)]
struct Deaths(u32);

fn count_deaths(mut died: MessageReader<PlayerDied>, mut restarted: MessageReader<RestartLevel>, mut d: Local<u32>, mut commands: Commands) {
    if restarted.read().count() > 0 {
        *d = 0;
    }
    *d += died.read().count() as u32;
    commands.insert_resource(Deaths(*d));
}

/// Nat reached the goal (Side A).
fn cleared(run: Res<LevelRun>, deaths: Option<Res<Deaths>>, mut done: MessageReader<LevelCompleted>, mut says: MessageWriter<HanSays>) {
    done.clear();
    println!(
        "playtest: cleared time={:.1}s nuggets={}/{} splats={}",
        run.time,
        run.nuggets,
        run.nuggets_total,
        deaths.map_or(0, |d| d.0)
    );
    says.write(HanSays { text: "Flushed it! Again?".into() });
}

/// Jump or R after clearing: play it again.
fn again(input: Single<&ActionState<Action>>, mut next: ResMut<NextState<AppState>>) {
    if input.just_pressed(&Action::Jump) || input.just_pressed(&Action::Restart) || input.just_pressed(&Action::Confirm) {
        next.set(AppState::Playing);
    }
}

fn quit(mut exit: MessageWriter<AppExit>) {
    exit.write(AppExit::Success);
}

/// A Side B level's `solution:` line (the editor's: `sideb_solve`) is no part of the level.
fn strip_solution(text: &str) -> String {
    #[cfg(not(target_arch = "wasm32"))]
    {
        super::sideb_solve::split_solution(text).0
    }
    #[cfg(target_arch = "wasm32")]
    {
        text.to_string()
    }
}
