//! The gadget tiles' art: crushers' blocks (steel, a face, hazard stripes on the slamming side),
//! crumbling platforms cracking and gone, smashed bricks vanishing, trampolines squashing,
//! beat blocks there and gone with the bars, fans puffing wisps up their updraft (twice as far
//! under Giant Steps, faster when fired up: the tiles answer the band), and a secret's grate
//! with Han behind it (waving once Nat's found him).

use bevy::prelude::*;

use crate::art::{SpriteId, Sprites};
use crate::game::crushers::{BrickSmashed, Crumble, CrusherBlock, Sprung};
use crate::game::secret::HanGlimpse;
use crate::game::{ActiveLevel, Groove, LevelEntity, LevelTile};
use crate::level::{CrushDir, FAN_REACH, FIRED_UP_TILES, GIANT_FAN_REACH, TILE, Tile};

pub(super) fn plugin(app: &mut App) {
    app.add_observer(crusher_sprite).add_observer(glimpse_sprite).add_systems(
        PostUpdate,
        (crumble_sprites, smashed_bricks, squash_trampolines, beat_sprites, fan_wisps, glimpse_wave)
            .in_set(super::VisualSet::Animate)
            .run_if(resource_exists::<Sprites>),
    );
}

/// A crusher's block: steel in every cell, stripes along the slamming side, the face in the
/// middle.
fn crusher_sprite(add: On<Add, CrusherBlock>, q: Query<&CrusherBlock>, sprites: Option<Res<Sprites>>, mut commands: Commands) {
    let (Some(sprites), Ok(b)) = (sprites, q.get(add.entity)) else { return };
    let (w, h) = (b.def.w as i32, b.def.h as i32);
    let dir = b.def.dir;
    commands.entity(add.entity).insert(Visibility::default()).with_children(|c| {
        for r in 0..h {
            for k in 0..w {
                let at = Vec2::new((k as f32 - (w as f32 - 1.0) / 2.0) * TILE, ((h as f32 - 1.0) / 2.0 - r as f32) * TILE);
                c.spawn((Sprite::from_image(sprites.get(SpriteId::CrusherBody)), Transform::from_translation(at.extend(0.0))));
                let face = match dir {
                    CrushDir::Down => r == h - 1,
                    CrushDir::Left => k == 0,
                    CrushDir::Right => k == w - 1,
                };
                if face {
                    let turn = match dir {
                        CrushDir::Down => 0.0,
                        CrushDir::Left => -std::f32::consts::FRAC_PI_2,
                        CrushDir::Right => std::f32::consts::FRAC_PI_2,
                    };
                    c.spawn((
                        Sprite::from_image(sprites.get(SpriteId::CrusherStripes)),
                        Transform::from_translation(at.extend(0.05)).with_rotation(Quat::from_rotation_z(turn)),
                    ));
                }
            }
        }
        // The face, in the middle.
        c.spawn((Sprite::from_image(sprites.get(SpriteId::CrusherFace)), Transform::from_xyz(0.0, 0.0, 0.1)));
    });
}

/// The dark room behind the grate.
const GLIMPSE_ROOM: Color = Color::srgb(0.07, 0.08, 0.11);

/// Han glimpsed behind a grate: the dark room, Han facing the open side, the bars over both.
fn glimpse_sprite(
    add: On<Add, HanGlimpse>,
    q: Query<&Transform, With<HanGlimpse>>,
    active: Option<Res<ActiveLevel>>,
    sprites: Option<Res<Sprites>>,
    mut commands: Commands,
) {
    let (Some(sprites), Some(active), Ok(tf)) = (sprites, active, q.get(add.entity)) else { return };
    // Face the side Nat finds him from (the open one).
    let (c, r) = ((tf.translation.x / TILE).floor() as i32, active.level.height as i32 - 1 - (tf.translation.y / TILE).floor() as i32);
    let facing_left = !active.level.tile(c - 1, r).is_solid();
    commands.entity(add.entity).insert(Visibility::default()).with_children(|g| {
        g.spawn((Sprite::from_color(GLIMPSE_ROOM, Vec2::new(14.0, 30.0)), Transform::from_xyz(0.0, TILE / 2.0, 0.0)));
        g.spawn((
            GlimpsedHan,
            Sprite { image: sprites.get(SpriteId::HanIdle), flip_x: facing_left, ..default() },
            Transform::from_xyz(0.0, 0.0, 0.1),
        ));
        g.spawn((Sprite::from_image(sprites.get(SpriteId::Grate)), Transform::from_xyz(0.0, TILE / 2.0, 0.2)));
    });
}

/// The Han sprite behind a grate.
#[derive(Component)]
struct GlimpsedHan;

/// Han idles behind the grate; found, he waves his arms (the goalkeeper pose, flapping).
fn glimpse_wave(
    time: Res<Time>,
    sprites: Res<Sprites>,
    glimpses: Query<(&HanGlimpse, &Children)>,
    mut han: Query<&mut Sprite, With<GlimpsedHan>>,
) {
    for (g, children) in &glimpses {
        for c in children {
            let Ok(mut sprite) = han.get_mut(*c) else { continue };
            let t = time.elapsed_secs();
            let image = if g.found {
                sprites.frame(SpriteId::HanIntercept, (t * 6.0) as usize % 2)
            } else {
                sprites.frame(SpriteId::HanIdle, (t * 2.0) as usize % sprites.frames(SpriteId::HanIdle).len())
            };
            if sprite.image != image {
                sprite.image = image;
            }
        }
    }
}

/// Crumbling platforms: shaking while they crack, gone once they've fallen.
fn crumble_sprites(time: Res<Time>, sprites: Res<Sprites>, mut q: Query<(&Crumble, &LevelTile, &mut Sprite, &mut Visibility, &mut Transform)>) {
    for (state, tile, mut sprite, mut vis, mut tf) in &mut q {
        let frame = matches!(state, Crumble::Cracking(_)) as usize;
        let image = sprites.frame(SpriteId::Crumble, frame);
        if sprite.image != image {
            sprite.image = image;
        }
        vis.set_if_neq(if matches!(state, Crumble::Fallen(_)) { Visibility::Hidden } else { Visibility::Inherited });
        let shake = if matches!(state, Crumble::Cracking(_)) { (time.elapsed_secs() * 60.0).sin().signum() } else { 0.0 };
        let x = (tile.col as f32 + 0.5) * TILE + shake;
        if tf.translation.x != x {
            tf.translation.x = x;
        }
    }
}

/// A smashed brick's tile goes.
fn smashed_bricks(mut commands: Commands, mut smashed: MessageReader<BrickSmashed>, tiles: Query<(Entity, &LevelTile)>) {
    for s in smashed.read() {
        for (e, t) in &tiles {
            if (t.col, t.row) == (s.col, s.row) && t.tile == Tile::Brick {
                commands.entity(e).despawn();
            }
        }
    }
}

/// A trampoline that just sprang someone squashes a moment.
#[derive(Component)]
struct Squashed(f32);

fn squash_trampolines(
    mut commands: Commands,
    time: Res<Time>,
    sprites: Res<Sprites>,
    mut sprung: MessageReader<Sprung>,
    mut q: Query<(Entity, &LevelTile, &Transform, &mut Sprite, Option<&mut Squashed>)>,
) {
    let hits: Vec<Vec2> = sprung.read().map(|s| s.pos).collect();
    for (e, t, tf, mut sprite, squashed) in &mut q {
        if !matches!(t.tile, Tile::Trampoline | Tile::CushionRight | Tile::CushionLeft | Tile::CushionDown) {
            continue;
        }
        let center = Vec2::new((t.col as f32 + 0.5) * TILE, 0.0);
        // A trampoline squashes under whoever sprang off it (by column); a cushion laid
        // sideways or upside down when they're right by it.
        let hit = if t.tile == Tile::Trampoline {
            hits.iter().any(|p| (p.x - center.x).abs() < TILE)
        } else {
            let at = tf.translation.truncate();
            hits.iter().any(|p| (*p - at).length() < TILE * 1.6)
        };
        let left = match squashed {
            _ if hit => {
                commands.entity(e).insert(Squashed(0.15));
                0.15
            }
            Some(mut s) => {
                s.0 -= time.delta_secs();
                s.0
            }
            None => 0.0,
        };
        let image = sprites.frame(SpriteId::Trampoline, (left > 0.0) as usize);
        if sprite.image != image {
            sprite.image = image;
        }
    }
}

/// Beat blocks show what the level says they are: there (frame 0) or gone (an outline).
fn beat_sprites(active: Option<Res<ActiveLevel>>, sprites: Res<Sprites>, mut q: Query<(&LevelTile, &mut Sprite)>) {
    let Some(active) = active else { return };
    if !active.is_changed() {
        return;
    }
    for (t, mut sprite) in &mut q {
        if !t.tile.is_beat() {
            continue;
        }
        let now = active.level.tile(t.col as i32, t.row as i32);
        let id = if matches!(t.tile, Tile::BeatRed | Tile::BeatRedOff) { SpriteId::BeatRed } else { SpriteId::BeatBlue };
        let image = sprites.frame(id, (!now.is_solid()) as usize);
        if sprite.image != image {
            sprite.image = image;
        }
    }
}

/// Fans puff wisps up their updraft, every so often.
fn fan_wisps(
    mut commands: Commands,
    time: Res<Time>,
    active: Option<Res<ActiveLevel>>,
    sprites: Res<Sprites>,
    groove: Option<Res<Groove>>,
    fans: Query<&LevelTile>,
    mut clock: Local<f32>,
) {
    let Some(active) = active else { return };
    *clock += time.delta_secs();
    if *clock < 0.12 {
        return;
    }
    *clock = 0.0;
    let level = &active.level;
    let t = time.elapsed_secs();
    let far = groove.as_ref().is_some_and(|g| g.giant_steps());
    let max = if far { GIANT_FAN_REACH } else { FAN_REACH };
    let speed = if groove.as_ref().is_some_and(|g| g.fired_up()) { 90.0 * FIRED_UP_TILES } else { 90.0 };
    for f in &fans {
        let Some((x, y)) = f.tile.fan() else { continue };
        let (c, r) = (f.col as i32, f.row as i32);
        // How far its gust reaches (grid rows run down).
        let reach = (1..=max)
            .take_while(|k| {
                let w = level.wind_within(c + x * k, r - y * k, max);
                (x != 0 && w.0.signum() == x) || (y != 0 && w.1.signum() == y)
            })
            .count() as f32;
        if reach == 0.0 {
            continue;
        }
        // A cheap jitter: no two fans (or puffs) alike.
        let j = ((t * 12.9898 + f.col as f32 * 78.233).sin() * 43758.545).fract();
        let dir = Vec2::new(x as f32, y as f32);
        let at = level.tile_center(f.col, f.row) + dir.perp() * (j - 0.5) * 12.0 + dir * TILE / 2.0;
        let life = reach * TILE / speed;
        commands.spawn((
            LevelEntity,
            super::particles::Particle { vel: dir * speed, gravity: 0.0, life, max_life: life, fade: true },
            Sprite { image: sprites.get(SpriteId::Wisp), color: Color::srgba(0.8, 0.97, 1.0, 1.0), ..default() },
            Transform::from_translation(at.extend(1.5)),
        ));
    }
}
