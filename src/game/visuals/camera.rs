//! The game camera (follows the player with look-ahead, clamped to the level) and the parallax
//! backdrop.

use bevy::camera::ScalingMode;
use bevy::prelude::*;

use super::VisualSet;
use crate::art::{SpriteId, Sprites};
use crate::game::{ActiveLevel, Groove, LevelEntity, Nudge, Player, PlayerControl, SEASICK_ROLL, SEASICK_ROLL_HZ};
use crate::level::TILE;
use crate::touch::TouchMode;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn_camera).add_systems(
        PostUpdate,
        (
            follow_player,
            // Once per load (a reload despawns them first): a splat stain changes the level too,
            // and must not stack up another backdrop and margin.
            (
                spawn_backdrop.run_if(not(any_with_component::<Backdrop>)),
                spawn_margins.run_if(not(any_with_component::<MarginTile>)),
            )
                .run_if(resource_exists::<Sprites>.and_then(resource_exists_and_changed::<ActiveLevel>)),
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
/// Touch mode: how far (px) the camera may show past the level's left/right edges and below its
/// floor, so Nat isn't pinned into a bottom corner under a thumb; and how much higher Nat sits.
const TOUCH_MARGIN: Vec2 = Vec2::new(6.0 * TILE, 3.0 * TILE);
const TOUCH_LIFT: f32 = 14.0;
/// Decorative ground drawn past the level's edges (walls left/right, earth under solid floor).
const MARGIN_COLS: i32 = 7;
const MARGIN_ROWS: i32 = 4;
/// Backdrop scroll speed relative to the camera.
const PARALLAX: f32 = 0.3;
/// Backdrop art is 256x144; drawn at this scale to fill the 216px view.
const BACKDROP_SCALE: f32 = VIEW_HEIGHT / 144.0;
const BACKDROP_COPIES: usize = 4;

/// The one game camera. `look` is the smoothed look-ahead offset.
#[derive(Component, Debug, Default)]
pub struct GameCamera {
    pub look: f32,
    /// The laughing band's wobble offset currently applied (kept out of the smoothing).
    pub wobble: Vec2,
    /// Wobble strength 0..1 (eases in and out with the bouncy groove).
    pub giggle: f32,
    /// Roll strength 0..1 (eases in and out with the laughing band's seasick phrase).
    pub seasick: f32,
}

/// Laughing-band camera wobble: amplitude (px) and frequency (Hz).
const WOBBLE_PX: f32 = 1.5;
const WOBBLE_HZ: f32 = 2.3;

#[derive(Component)]
struct Backdrop(usize);

#[derive(Component)]
struct MarginTile;

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

#[allow(clippy::type_complexity)]
fn follow_player(
    time: Res<Time>,
    groove: Option<Res<Groove>>,
    touch: Option<Res<TouchMode>>,
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
    let touch = touch.is_some_and(|t| t.0);
    let lift = if touch { 16.0 + TOUCH_LIFT } else { 16.0 };
    let target = ptf.translation.truncate() + Vec2::new(gc.look, lift);
    let mut p = tf.translation.truncate() - gc.wobble;
    p.x += (target.x - p.x) * k(6.0);
    p.y += (target.y - p.y) * k(4.0);

    let size = active.level.size_px();
    let clamp = |v: f32, half: f32, size: f32, margin: f32| {
        if size + 2.0 * margin <= half * 2.0 { size / 2.0 } else { v.clamp(half - margin, size - half + margin) }
    };
    let margin = if touch { TOUCH_MARGIN } else { Vec2::ZERO };
    p.x = clamp(p.x, half.x, size.x, margin.x);
    // (Only below the floor: the top keeps its edge.)
    p.y = if size.y + margin.y <= half.y * 2.0 {
        size.y / 2.0
    } else {
        p.y.clamp(half.y - margin.y, size.y - half.y)
    };
    let bouncy = groove.as_ref().is_some_and(|g| g.bouncy());
    gc.giggle = (gc.giggle + if bouncy { dt } else { -dt }).clamp(0.0, 1.0);
    let t = time.elapsed_secs() * std::f32::consts::TAU * WOBBLE_HZ;
    gc.wobble = gc.giggle * WOBBLE_PX * Vec2::new((t * 0.5).sin(), t.sin());
    tf.translation.x = p.x + gc.wobble.x;
    tf.translation.y = p.y + gc.wobble.y;
    // Seasick (the laughing band's 7-TET phrase): the world rolls a little.
    let seasick = groove.is_some_and(|g| g.nudge() == Some(Nudge::Seasick));
    gc.seasick = (gc.seasick + if seasick { dt } else { -dt }).clamp(0.0, 1.0);
    let roll = gc.seasick * SEASICK_ROLL * (time.elapsed_secs() * std::f32::consts::TAU * SEASICK_ROLL_HZ).sin();
    tf.rotation = Quat::from_rotation_z(roll);
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
    cam: Query<(&Transform, &GameCamera), Without<Backdrop>>,
    mut q: Query<(&Backdrop, &mut Transform)>,
) {
    let Ok((cam, gc)) = cam.single() else { return };
    // A rolling camera sees past the backdrop's top and bottom at the corners: grow it a bit.
    let scale = 1.0 + gc.seasick * 0.06;
    let w = 256.0 * BACKDROP_SCALE;
    let c = cam.translation;
    // Tiles scroll at PARALLAX of the camera speed; wrap so copies always cover the view.
    let shift = (c.x * PARALLAX).rem_euclid(w);
    let first = c.x - shift - w * ((BACKDROP_COPIES as f32 - 1.0) / 2.0).floor() + w / 2.0;
    for (b, mut tf) in &mut q {
        tf.translation.x = first + b.0 as f32 * w;
        tf.translation.y = c.y;
        tf.scale = Vec3::new(scale, scale, 1.0);
    }
}

/// Ground past the level's edges, so a camera allowed past them (touch mode) never shows a
/// void: solid walls left and right (as the physics treats them) and earth under solid floor
/// columns (pits stay open).
fn spawn_margins(mut commands: Commands, sprites: Res<Sprites>, active: Res<ActiveLevel>) {
    let level = &active.level;
    let (w, h) = (level.width as i32, level.height as i32);
    let fill = sprites.get(SpriteId::GroundFill(level.world));
    let mut put = |col: i32, row: i32| {
        let x = col as f32 * TILE + TILE / 2.0;
        let y = (h - 1 - row) as f32 * TILE + TILE / 2.0;
        commands.spawn((
            Name::new("MarginTile"),
            MarginTile,
            LevelEntity,
            Sprite::from_image(fill.clone()),
            Transform::from_xyz(x, y, -1.0),
        ));
    };
    for row in 0..h + MARGIN_ROWS {
        for k in 1..=MARGIN_COLS {
            put(-k, row);
            put(w - 1 + k, row);
        }
    }
    for col in 0..w {
        if level.tile(col, h - 1).is_solid() {
            for row in h..h + MARGIN_ROWS {
                put(col, row);
            }
        }
    }
}
