//! Menus and HUD: title, level select, HUD (+ level intro card, toasts), pause, results,
//! victory credits, and Han's world-space speech bubble.
//!
//! All screens are bevy_ui laid out in *virtual pixels*: [`UiScale`] maps 216 virtual px to the
//! window height, like the game camera. Text is Press Start 2P at multiples of 8px.

use bevy::prelude::*;
use bevy::text::{FontSmoothing, FontSize};
use bevy::window::PrimaryWindow;

use crate::art::{SpriteId, Sprites};
use crate::audio::Sfx;
use crate::events::PlaySfx;

mod bubble;
mod hud;
mod level_select;
mod pause;
mod results;
mod title;
mod victory;

/// The game's name ("Nat Han Adventures"), shown on the title screen and in the credits,
/// split in two lines so it fits the 24px title font.
pub const GAME_TITLE: &str = "NAT HAN";
pub const GAME_SUBTITLE: &str = "ADVENTURES";
pub const GAME_TAGLINE: &str = "a plumber's tale";
/// Player-facing names of the hero (the poo) and his plumber sidekick.
pub const HERO_NAME: &str = "Nat";
pub const SIDEKICK_NAME: &str = "Han";

/// Virtual screen height in UI px (matches the game camera's `FixedVertical` height).
pub const VIRTUAL_HEIGHT: f32 = 216.0;

/// World names, by world number - 1.
pub const WORLD_NAMES: [&str; 5] = ["BATHROOM", "PIPES", "SEWER", "SEPTIC FEST", "TREATMENT PLANT"];

pub fn world_name(world: u8) -> &'static str {
    WORLD_NAMES.get((world as usize).wrapping_sub(1)).copied().unwrap_or("???")
}

/// The 8-bit palette: warm browns, gold accents, cream text, dark outlines.
pub mod palette {
    use bevy::prelude::Color;
    pub const INK: Color = Color::srgb(0.10, 0.07, 0.05);
    pub const SHADOW: Color = Color::srgba(0.06, 0.03, 0.02, 0.9);
    pub const CREAM: Color = Color::srgb(1.0, 0.94, 0.80);
    pub const DIM_CREAM: Color = Color::srgb(0.70, 0.60, 0.48);
    pub const GOLD: Color = Color::srgb(1.0, 0.78, 0.18);
    pub const DARK_GOLD: Color = Color::srgb(0.72, 0.48, 0.08);
    pub const BROWN: Color = Color::srgb(0.42, 0.26, 0.14);
    pub const DARK_BROWN: Color = Color::srgb(0.24, 0.14, 0.08);
    pub const PANEL: Color = Color::srgba(0.20, 0.12, 0.07, 0.94);
    pub const LOCKED: Color = Color::srgb(0.16, 0.11, 0.08);
    pub const GREEN: Color = Color::srgb(0.55, 0.85, 0.35);
    pub const OVERLAY: Color = Color::srgba(0.05, 0.03, 0.02, 0.6);
}

pub fn plugin(app: &mut App) {
    // Real window = the real game: load/save progress. (Headless tests on MinimalPlugins
    // never touch disk / localStorage.)
    if app.is_plugin_added::<bevy::window::WindowPlugin>() {
        app.add_plugins(crate::save::persistence_plugin);
    }

    let font = app
        .world_mut()
        .get_resource_mut::<Assets<Font>>()
        .map(|mut fonts| {
            fonts.add(Font::from_bytes(
                include_bytes!("../../assets/fonts/PressStart2P-Regular.ttf").to_vec(),
            ))
        })
        .unwrap_or_default();
    app.insert_resource(UiFont(font))
        .add_systems(Update, (scale_ui, ensure_camera, blink, tick_toasts))
        .add_plugins((
            title::plugin,
            level_select::plugin,
            hud::plugin,
            pause::plugin,
            results::plugin,
            victory::plugin,
            bubble::plugin,
        ));
}

/// Press Start 2P, embedded in the binary.
#[derive(Resource, Debug, Clone)]
pub struct UiFont(pub Handle<Font>);

/// Text style in the house font. `size` in virtual px (use multiples of 8).
pub fn font(ui: &UiFont, size: f32) -> TextFont {
    TextFont {
        font: ui.0.clone().into(),
        font_size: FontSize::Px(size),
        font_smoothing: FontSmoothing::None,
        ..default()
    }
}

/// A UI text with the house font and a chunky drop shadow.
pub fn label(ui: &UiFont, text: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text),
        font(ui, size),
        TextColor(color),
        TextShadow { offset: Vec2::splat((size / 8.0).max(1.0)), color: palette::SHADOW },
    )
}

/// A sprite from [`Sprites`] as a UI image, `size` virtual px square-ish.
pub fn icon(sprites: Option<&Sprites>, id: SpriteId, w: f32, h: f32) -> impl Bundle {
    (
        ImageNode::new(sprites.map(|s| s.get(id)).unwrap_or_default()),
        Node { width: px(w), height: px(h), ..default() },
    )
}

/// A bordered panel (dark outline, brown fill, gold inner rim) around `node`.
pub fn panel(node: Node) -> impl Bundle {
    (
        Node { border: UiRect::all(px(2.0)), ..node },
        BackgroundColor(palette::PANEL),
        BorderColor::all(palette::DARK_GOLD),
        Outline::new(px(2.0), px(0.0), palette::INK),
    )
}

/// A centered column with padding, for [`panel`].
pub fn column(padding: f32, gap: f32) -> Node {
    Node {
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        padding: UiRect::all(px(padding)),
        row_gap: px(gap),
        ..default()
    }
}

/// A full-screen root node (absolute, covering the window).
pub fn fullscreen() -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: percent(100.0),
        height: percent(100.0),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        ..default()
    }
}

/// `mm:ss`, or `h:mm:ss` for the very patient.
pub fn format_time(secs: f32) -> String {
    let t = secs.max(0.0) as u32;
    let (h, m, s) = (t / 3600, t / 60 % 60, t % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m:02}:{s:02}") }
}

/// Blinks (toggles visibility) with the given period in seconds.
#[derive(Component, Debug, Clone, Copy)]
pub struct Blink(pub f32);

fn blink(time: Res<Time>, mut q: Query<(&Blink, &mut Visibility)>) {
    for (b, mut v) in &mut q {
        let on = (time.elapsed_secs() / b.0).fract() < 0.6;
        v.set_if_neq(if on { Visibility::Inherited } else { Visibility::Hidden });
    }
}

/// 216 virtual px tall, whatever the window.
fn scale_ui(window: Option<Single<&Window, With<PrimaryWindow>>>, scale: Option<ResMut<UiScale>>) {
    let (Some(w), Some(mut scale)) = (window, scale) else { return };
    let h = w.resolution.height();
    if h > 0.0 {
        let s = h / VIRTUAL_HEIGHT;
        if (scale.0 - s).abs() > 1e-4 {
            scale.0 = s;
        }
    }
}

/// UI needs a camera. The game's visuals spawn the real one; this fills in only while there is
/// none (e.g. if menus run before any level camera exists) and steps aside once one appears.
#[derive(Component)]
struct FallbackCamera;

fn ensure_camera(
    mut commands: Commands,
    window: Option<Single<(), With<PrimaryWindow>>>,
    cams: Query<(Entity, Has<FallbackCamera>), With<Camera>>,
) {
    if window.is_none() {
        return;
    }
    let others = cams.iter().filter(|(_, fb)| !fb).count();
    let fallbacks: Vec<Entity> = cams.iter().filter(|(_, fb)| *fb).map(|(e, _)| e).collect();
    if others == 0 && fallbacks.is_empty() {
        commands.spawn((Name::new("UiFallbackCamera"), FallbackCamera, Camera2d));
    } else if others > 0 {
        for e in fallbacks {
            commands.entity(e).despawn();
        }
    }
}

/// A short message popping up near the top of the screen, gone after `ttl` seconds.
#[derive(Component, Debug)]
pub struct Toast {
    pub ttl: f32,
}

pub fn spawn_toast(commands: &mut Commands, ui: &UiFont, text: impl Into<String>, ttl: f32) {
    commands
        .spawn((
            Name::new("Toast"),
            Toast { ttl },
            Node {
                position_type: PositionType::Absolute,
                bottom: px(24.0),
                width: percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            GlobalZIndex(20),
        ))
        .with_children(|t| {
            t.spawn(panel(Node { padding: UiRect::axes(px(8.0), px(4.0)), ..default() }))
                .with_child(label(ui, text, 8.0, palette::GOLD));
        });
}

fn tick_toasts(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Toast)>) {
    for (e, mut t) in &mut q {
        t.ttl -= time.delta_secs();
        if t.ttl <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}

/// Menu navigation pressed this frame: x = right - left, y = down - up.
pub fn nav(a: &leafwing_input_manager::prelude::ActionState<crate::input::Action>) -> IVec2 {
    use crate::input::Action::*;
    let p = |x| a.just_pressed(&x) as i32;
    IVec2::new(p(Right) - p(Left), p(Down) - p(Up))
}

/// Menu sounds.
pub fn sfx(w: &mut MessageWriter<PlaySfx>, s: Sfx) {
    w.write(PlaySfx(s));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_format() {
        assert_eq!(format_time(0.0), "00:00");
        assert_eq!(format_time(65.9), "01:05");
        assert_eq!(format_time(3725.0), "1:02:05");
        assert_eq!(format_time(-3.0), "00:00");
    }

    #[test]
    fn worlds() {
        assert_eq!(world_name(1), "BATHROOM");
        assert_eq!(world_name(5), "TREATMENT PLANT");
        assert_eq!(world_name(0), "???");
        assert_eq!(world_name(9), "???");
    }
}
