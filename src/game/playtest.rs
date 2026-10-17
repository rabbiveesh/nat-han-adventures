//! Playtesting a level file: `NATHAN_LEVEL=levels/custom/mine.txt cargo run` (native; a number
//! there is still a story level), or the
//! level editor's Play button. The file is played straight away (Side B if it has a `setlist:`
//! line), with no adaptive assists ([`AssistMode::Manual`]: what you play is the level), and is
//! reloaded whenever it changes on disk, so saving in the editor restarts it in the game.
//! Reaching the goal prints a `playtest: cleared ...` line (the editor reads it) and Jump or R
//! plays again; quitting to the title closes the game.
//!
//! A share link's level (`level::share`: the web game's `#lvl=...`, or SHARED LEVEL on the
//! title) plays the same way, from the link's text; quitting it goes back to the title.

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::sideb::{self, SideB, SideBLevel};
use super::{AssistMode, GeneratedLevel, LevelRun, RestartLevel};
use crate::events::{HanSays, LevelCompleted, PlayerDied};
use crate::input::Action;
use crate::level::Level;
use crate::state::{AppState, CurrentLevel};

/// A level file being playtested, or a shared level being played. Exists while it is.
#[derive(Resource, Debug, Clone)]
pub struct Playtest {
    pub source: Source,
    /// The file's text as last loaded.
    pub text: String,
    poll: f32,
}

/// Where a playtest's level came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// A file, reloaded when it changes.
    File(std::path::PathBuf),
    /// A share link.
    Shared,
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

/// Play a share link's level (its text: `level::share::decode`), or say why not.
pub fn play_shared(commands: &mut Commands, text: String) -> Result<(), String> {
    let loaded = load_text(&text)?;
    commands.insert_resource(Playtest { source: Source::Shared, text, poll: 0.0 });
    start(commands, loaded);
    Ok(())
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Startup, (from_env, from_link))
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
        commands.insert_resource(Playtest { source: Source::File(path.into()), text, poll: 0.0 });
        start(&mut commands, loaded);
    }
}

/// The web game opened from a share link (`#lvl=...`): play its level (a broken link says so,
/// and the title comes up as usual).
#[allow(unused_mut, unused_variables)]
fn from_link(mut commands: Commands) {
    #[cfg(target_arch = "wasm32")]
    {
        let Some(hash) = web_sys::window().and_then(|w| w.location().hash().ok()) else { return };
        if !hash.contains(crate::level::share::KEY) {
            return;
        }
        if let Err(e) = crate::level::share::decode(&hash).and_then(|text| play_shared(&mut commands, text)) {
            crate::web::alert(&format!("Can't play that shared level: {e}"));
        }
    }
}

/// Play `loaded` from the top.
fn start(commands: &mut Commands, loaded: Loaded) {
    match loaded {
        Loaded::SideA(level) => {
            commands.remove_resource::<SideB>();
            commands.insert_resource(GeneratedLevel(level));
            commands.insert_resource(AssistMode::Manual);
            commands.insert_resource(CurrentLevel(0));
            commands.queue(|w: &mut World| w.resource_mut::<NextState<AppState>>().set(AppState::Playing));
        }
        Loaded::SideB(b) => sideb::start_level(commands, 0, b),
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
        let Source::File(path) = pt.source.clone() else { return };
        let Ok(text) = std::fs::read_to_string(&path) else { return };
        if text == pt.text {
            return;
        }
        pt.text = text.clone();
        match load_text(&text) {
            Ok(loaded) => {
                println!("playtest: reloaded {}", path.display());
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
                println!("playtest: {} doesn't parse: {e}", path.display());
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

/// Back at the title: a playtest closes the game; a shared level's done, and the story gets its
/// levels and assists back (Side B's are `sideb::leave`'s).
fn quit(mut commands: Commands, pt: Res<Playtest>, mut exit: MessageWriter<AppExit>) {
    if pt.source != Source::Shared {
        exit.write(AppExit::Success);
        return;
    }
    commands.remove_resource::<Playtest>();
    commands.remove_resource::<GeneratedLevel>();
    commands.insert_resource(AssistMode::Story);
    #[cfg(target_arch = "wasm32")]
    crate::web::clear_hash();
}

/// A Side B level's `solution:` line (the editor's: `sideb_solve`) is no part of the level.
fn strip_solution(text: &str) -> String {
    sideb::split_solution(text).0
}
