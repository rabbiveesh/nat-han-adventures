//! The game camera (follows the player with look-ahead, clamped to the level) and the parallax
//! backdrop.

use bevy::camera::ScalingMode;
use bevy::prelude::*;

use super::VisualSet;
use crate::art::{SpriteId, Sprites};
use crate::game::{ActiveLevel, LevelEntity, Player, PlayerControl};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn_camera).add_systems(
        PostUpdate,
        (
            follow_player,
            spawn_backdrop.run_if(resource_exists::<Sprites>.and_then(resource_exists_and_changed::<ActiveLevel>)),
            scroll_backdrop,
        )
            .chain()
            .in_set(VisualSet::Camera),
    );
}

/// Virtual screen height in pixels (13.5 tiles).
pub const VIEW_HEIGHT: f32 = 216.0;
/// How far ahead (px) of the player the camera looks in the facing direction.
const LOOK_AHEAD: f32 = 32.0;
/// Backdrop scroll speed relative to the camera.
const PARALLAX: f32 = 0.3;
/// Backdrop art is 256x144; drawn at this scale to fill the 216px view.
const BACKDROP_SCALE: f32 = VIEW_HEIGHT / 144.0;
const BACKDROP_COPIES: usize = 4;

/// The one game camera. `look` is the smoothed look-ahead offset.
#[derive(Component, Debug, Default)]
pub struct GameCamera {
    pub look: f32,
}

#[derive(Component)]
struct Backdrop(usize);

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Name::new("GameCamera"),
        Camera2d,
        GameCamera::default(),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical { viewport_height: VIEW_HEIGHT },
            ..OrthographicProjection::default_2d()
        }),
    ));
}

fn follow_player(
    time: Res<Time>,
    active: Option<Res<ActiveLevel>>,
    player: Query<(&Transform, &PlayerControl, Ref<Player>), Without<GameCamera>>,
    mut cam: Query<(&mut Transform, &mut GameCamera, &Projection)>,
) {
    let (Some(active), Ok((ptf, ctl, added))) = (active, player.single()) else { return };
    let Ok((mut tf, mut gc, proj)) = cam.single_mut() else { return };
    let half = match proj {
        Projection::Orthographic(o) if o.area.width() > 0.0 => o.area.half_size(),
        _ => Vec2::new(VIEW_HEIGHT * 16.0 / 9.0, VIEW_HEIGHT) / 2.0,
    };
    let snap = added.is_added();
    let dt = time.delta_secs();
    let k = |rate: f32| if snap { 1.0 } else { 1.0 - (-dt * rate).exp() };

    gc.look += (ctl.facing * LOOK_AHEAD - gc.look) * k(2.5);
    let target = ptf.translation.truncate() + Vec2::new(gc.look, 16.0);
    let mut p = tf.translation.truncate();
    p.x += (target.x - p.x) * k(6.0);
    p.y += (target.y - p.y) * k(4.0);

    let size = active.level.size_px();
    let clamp = |v: f32, half: f32, size: f32| {
        if size <= half * 2.0 { size / 2.0 } else { v.clamp(half, size - half) }
    };
    p.x = clamp(p.x, half.x, size.x);
    p.y = clamp(p.y, half.y, size.y);
    tf.translation.x = p.x;
    tf.translation.y = p.y;
}

fn spawn_backdrop(mut commands: Commands, sprites: Res<Sprites>, active: Res<ActiveLevel>) {
    let size = Vec2::new(256.0, 144.0) * BACKDROP_SCALE;
    for i in 0..BACKDROP_COPIES {
        commands.spawn((
            Name::new("Backdrop"),
            LevelEntity,
            Backdrop(i),
            Sprite {
                image: sprites.get(SpriteId::Backdrop(active.level.world)),
                custom_size: Some(size),
                ..default()
            },
            Transform::from_xyz(0.0, 0.0, -100.0),
        ));
    }
}

fn scroll_backdrop(
    cam: Query<&Transform, (With<GameCamera>, Without<Backdrop>)>,
    mut q: Query<(&Backdrop, &mut Transform)>,
) {
    let Ok(cam) = cam.single() else { return };
    let w = 256.0 * BACKDROP_SCALE;
    let c = cam.translation;
    // Tiles scroll at PARALLAX of the camera speed; wrap so copies always cover the view.
    let shift = (c.x * PARALLAX).rem_euclid(w);
    let first = c.x - shift - w * ((BACKDROP_COPIES as f32 - 1.0) / 2.0).floor() + w / 2.0;
    for (b, mut tf) in &mut q {
        tf.translation.x = first + b.0 as f32 * w;
        tf.translation.y = c.y;
    }
}
