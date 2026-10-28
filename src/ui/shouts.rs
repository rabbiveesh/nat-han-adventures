//! Words shouted out in a level ([`ToyShout`]: the helper's "HELP! HELP!", a stop sign's
//! "STOP!", a house's "SIZZLE!"): they pop up where they're said, in the house font with a
//! shadow, float up a little and fade.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::text::{FontSize, FontSmoothing};
use bevy::window::PrimaryWindow;

use super::{UiFont, VIRTUAL_HEIGHT, palette::INK};
use crate::events::ToyShout;
use crate::state::AppState;

/// A shout stays this long (s)...
const LIFE: f32 = 1.4;
/// ...rising this fast (px/s).
const RISE: f32 = 10.0;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (spawn_shouts, float_shouts).chain());
}

#[derive(Component)]
struct Shout {
    age: f32,
}

fn spawn_shouts(
    mut commands: Commands,
    mut shouts: MessageReader<ToyShout>,
    font: Option<Res<UiFont>>,
    window: Option<Single<&Window, With<PrimaryWindow>>>,
) {
    let Some(font) = font else {
        shouts.clear();
        return;
    };
    // Rendered at the window's scale, then shrunk, so it's crisp.
    let k = window.map_or(1.0, |w| (w.resolution.height() / VIRTUAL_HEIGHT).ceil().max(1.0));
    for s in shouts.read() {
        let text = |color: Color, at: Vec2, z: f32| {
            (
                Text2d::new(s.text),
                TextFont { font: font.0.clone().into(), font_size: FontSize::Px(8.0 * k), font_smoothing: FontSmoothing::None, ..default() },
                TextColor(color),
                Anchor::BOTTOM_CENTER,
                Transform::from_translation(at.extend(z)).with_scale(Vec3::splat(1.0 / k)),
            )
        };
        commands
            .spawn((Name::new("Shout"), Shout { age: 0.0 }, DespawnOnExit(AppState::Playing), Transform::from_translation(s.pos.extend(10.0)), Visibility::Inherited))
            .with_children(|p| {
                p.spawn(text(INK, Vec2::new(1.0, -1.0), 0.0));
                p.spawn(text(Color::srgb(1.0, 0.95, 0.6), Vec2::ZERO, 0.1));
            });
    }
}

fn float_shouts(mut commands: Commands, time: Res<Time>, mut shouts: Query<(Entity, &mut Shout, &mut Transform, &Children)>, mut colors: Query<&mut TextColor>) {
    let dt = time.delta_secs();
    for (e, mut s, mut tf, children) in &mut shouts {
        s.age += dt;
        if s.age >= LIFE {
            commands.entity(e).despawn();
            continue;
        }
        tf.translation.y += RISE * dt;
        let alpha = (1.0 - (s.age - LIFE * 0.6).max(0.0) / (LIFE * 0.4)).clamp(0.0, 1.0);
        for &c in children {
            if let Ok(mut col) = colors.get_mut(c) {
                col.0.set_alpha(alpha);
            }
        }
    }
}
