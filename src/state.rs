//! App flow: Title -> LevelSelect -> Playing -> LevelComplete -> (LevelSelect | next level),
//! and Victory after the last level. Free play: Title -> FreePlaySetup -> Playing ->
//! LevelComplete (its results) -> FreePlaySetup | Title.

use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.init_state::<AppState>()
        .init_resource::<CurrentLevel>()
        .add_sub_state::<PlayState>();
}

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Reflect)]
pub enum AppState {
    #[default]
    Title,
    LevelSelect,
    /// Free play's setup screen (endless or 8 rooms, the seed).
    FreePlaySetup,
    /// A level is loaded and being played (including paused / dying).
    Playing,
    /// Goal reached: results card for the current level. Level entities still exist (frozen).
    LevelComplete,
    /// Beat level 10: credits.
    Victory,
}

/// Fine-grained state while [`AppState::Playing`].
#[derive(SubStates, Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Reflect)]
#[source(AppState = AppState::Playing)]
pub enum PlayState {
    #[default]
    Running,
    Paused,
}

/// Index (0-based) into [`crate::level::LEVELS`] of the level being played / selected.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
#[reflect(Resource)]
pub struct CurrentLevel(pub usize);
