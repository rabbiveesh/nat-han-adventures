//! Input actions via leafwing. Arrows or WASD; Space/W/Up/Z jump. The [`ActionState`] lives
//! on one global entity (leafwing 0.21 has no resource form): query it with
//! `Single<&ActionState<Action>>`.

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_plugins(InputManagerPlugin::<Action>::default())
        .add_systems(Startup, spawn_input);
}

#[derive(Actionlike, Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum Action {
    Left,
    Right,
    /// Menus only (gameplay jump is [`Action::Jump`]).
    Up,
    Down,
    Jump,
    /// Restart the level from the start (R).
    Restart,
    /// Pause in game / back out of menus (Esc, Backspace).
    Back,
    /// Menu select (Enter, Space).
    Confirm,
}

/// Marker for the entity holding the global [`ActionState<Action>`].
#[derive(Component)]
pub struct GlobalInput;

pub fn input_map() -> InputMap<Action> {
    use Action::*;
    InputMap::new([
        (Left, KeyCode::ArrowLeft),
        (Left, KeyCode::KeyA),
        (Right, KeyCode::ArrowRight),
        (Right, KeyCode::KeyD),
        (Up, KeyCode::ArrowUp),
        (Up, KeyCode::KeyW),
        (Down, KeyCode::ArrowDown),
        (Down, KeyCode::KeyS),
        (Jump, KeyCode::Space),
        (Jump, KeyCode::ArrowUp),
        (Jump, KeyCode::KeyW),
        (Jump, KeyCode::KeyZ),
        (Restart, KeyCode::KeyR),
        (Back, KeyCode::Escape),
        (Back, KeyCode::Backspace),
        (Confirm, KeyCode::Enter),
        (Confirm, KeyCode::Space),
    ])
}

fn spawn_input(mut commands: Commands) {
    commands.spawn((Name::new("GlobalInput"), GlobalInput, input_map()));
}
