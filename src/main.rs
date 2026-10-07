use bevy::prelude::*;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Nat Han Adventures".into(),
                    // Web: render into the page's canvas and keep arrows/space from scrolling it.
                    canvas: Some("#game".into()),
                    fit_canvas_to_parent: true,
                    prevent_default_event_handling: true,
                    ..default()
                }),
                ..default()
            })
            // Chunky pixels: never blur our 16x16 sprites.
            .set(ImagePlugin::default_nearest()),
        bevy_kira_audio::AudioPlugin,
        nat_han_adventures::gameplay,
        nat_han_adventures::presentation,
    ))
    .insert_resource(ClearColor(Color::srgb_u8(0x1a, 0x12, 0x0c)));

    #[cfg(feature = "brp")]
    app.add_plugins(bevy_brp_extras::BrpExtrasPlugin);

    app.run();
}
