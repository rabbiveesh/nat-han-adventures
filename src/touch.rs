//! Phone touch mode. The page (index.html) decides whether to show touch controls and marks
//! `<body class="touch">`; the game reads that to frame the camera for thumbs. On native,
//! `NATHAN_TOUCH=1` forces it (for testing).

use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.insert_resource(TouchMode(detect()));
}

/// True when playing with on-screen touch controls.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
#[reflect(Resource)]
pub struct TouchMode(pub bool);

#[cfg(target_arch = "wasm32")]
fn detect() -> bool {
    web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.body())
        .is_some_and(|b| b.class_list().contains("touch"))
}

#[cfg(not(target_arch = "wasm32"))]
fn detect() -> bool {
    std::env::var("NATHAN_TOUCH").is_ok_and(|v| v == "1")
}
