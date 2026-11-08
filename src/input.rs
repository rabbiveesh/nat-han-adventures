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
    /// Menus, and in a level: call the band (`events::CallBand`).
    Down,
    Jump,
    /// Restart the level from the start (R).
    Restart,
    /// Pause in game / back out of menus (Esc, Backspace).
    Back,
    /// Menu select (Enter, Space).
    Confirm,
    /// Side B: play the set list's first, second or third call at once (1, 2, 3; on a phone,
    /// the left, middle and right thirds of the screen).
    Call1,
    Call2,
    Call3,
}

impl Action {
    /// Side B's direct calls, in set list order.
    pub const CALLS: [Action; 3] = [Action::Call1, Action::Call2, Action::Call3];
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
        (Call1, KeyCode::Digit1),
        (Call2, KeyCode::Digit2),
        (Call3, KeyCode::Digit3),
    ])
}

fn spawn_input(mut commands: Commands) {
    commands.spawn((Name::new("GlobalInput"), GlobalInput, input_map()));
}
