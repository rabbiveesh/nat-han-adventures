//! The gadget tiles' art: crushers' blocks (steel, a face, hazard stripes on the slamming side),
//! crumbling platforms cracking and gone, smashed bricks vanishing, trampolines squashing,
//! beat blocks there and gone with the bars, fans puffing wisps up their updraft.

use bevy::prelude::*;

use crate::art::{SpriteId, Sprites};
use crate::game::crushers::{BrickSmashed, Crumble, CrusherBlock, Sprung};
use crate::game::{ActiveLevel, LevelEntity, LevelTile};
use crate::level::{CrushDir, TILE, Tile};

pub(super) fn plugin(app: &mut App) {
    app.add_observer(crusher_sprite).add_systems(
        PostUpdate,
        (crumble_sprites, smashed_bricks, squash_trampolines, beat_sprites, fan_wisps)
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
    mut q: Query<(Entity, &LevelTile, &mut Sprite, Option<&mut Squashed>)>,
) {
    let hits: Vec<Vec2> = sprung.read().map(|s| s.pos).collect();
    for (e, t, mut sprite, squashed) in &mut q {
        if t.tile != Tile::Trampoline {
            continue;
        }
        let x = (t.col as f32 + 0.5) * TILE;
        let hit = hits.iter().any(|p| (p.x - x).abs() < TILE);
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
    fans: Query<&LevelTile>,
    mut clock: Local<f32>,
) {
    let Some(active) = active else { return };
    *clock += time.delta_secs();
    if *clock < 0.18 {
        return;
    }
    *clock = 0.0;
    let level = &active.level;
    let t = time.elapsed_secs();
    for f in &fans {
        if f.tile != Tile::Fan {
            continue;
        }
        let (c, r) = (f.col as i32, f.row as i32);
        let reach = (1..).take_while(|k| level.updraft(c, r - k)).count() as f32;
        if reach == 0.0 {
            continue;
        }
        // A cheap jitter: no two fans (or puffs) alike.
        let j = ((t * 12.9898 + f.col as f32 * 78.233).sin() * 43758.545).fract();
        let at = level.tile_center(f.col, f.row) + Vec2::new((j - 0.5) * 12.0, TILE / 2.0);
        let speed = 90.0;
        let life = reach * TILE / speed;
        commands.spawn((
            LevelEntity,
            super::particles::Particle { vel: Vec2::new(0.0, speed), gravity: 0.0, life, max_life: life, fade: true },
            Sprite { image: sprites.get(SpriteId::Wisp), color: Color::srgba(0.85, 0.95, 1.0, 0.8), ..default() },
            Transform::from_translation(at.extend(1.5)),
        ));
    }
}
