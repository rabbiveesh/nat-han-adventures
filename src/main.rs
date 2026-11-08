use bevy::prelude::*;

fn main() {
    // `NATHAN_CAPTURE=<dir>`: record a deterministic video (see `nat_han_adventures::capture`).
    #[cfg(feature = "capture")]
    let capture = nat_han_adventures::capture::CaptureConfig::from_env()
        .map(|c| c.unwrap_or_else(|e| panic!("NATHAN_CAPTURE: {e}")));

    let window = Window {
        title: "Nat Han Adventures".into(),
        // Web: render into the page's canvas and keep arrows/space from scrolling it.
        canvas: Some("#game".into()),
        fit_canvas_to_parent: true,
        prevent_default_event_handling: true,
        ..default()
    };
    #[cfg(feature = "capture")]
    let window = if capture.is_some() { nat_han_adventures::capture::capture_window(window) } else { window };
    let plugins = DefaultPlugins
        .set(WindowPlugin { primary_window: Some(window), ..default() })
        // Chunky pixels: never blur our 16x16 sprites.
        .set(ImagePlugin::default_nearest());
    #[cfg(feature = "capture")]
    let plugins = if capture.is_some() { nat_han_adventures::capture::adjust_plugins(plugins) } else { plugins };

    let mut app = App::new();
    app.add_plugins((plugins, nat_han_adventures::gameplay, nat_han_adventures::presentation))
        .insert_resource(ClearColor(Color::srgb_u8(0x1a, 0x12, 0x0c)));

    // No remote control while capturing: the timeline is the only input.
    #[cfg(feature = "capture")]
    let remote = capture.is_none();
    #[cfg(not(feature = "capture"))]
    let remote = true;
    #[cfg(feature = "brp")]
    if remote {
        app.add_plugins(bevy_brp_extras::BrpExtrasPlugin);
    }
    let _ = remote;
    #[cfg(feature = "capture")]
    if let Some(c) = capture {
        app.add_plugins(nat_han_adventures::capture::CapturePlugin(c));
    }

    app.run();
}
