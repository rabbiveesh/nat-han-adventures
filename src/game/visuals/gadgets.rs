//! The gadget tiles' art: crushers' blocks (steel, a face, hazard stripes on the slamming side),
//! crumbling platforms cracking and gone, smashed bricks vanishing, trampolines squashing.

use bevy::prelude::*;

use crate::art::{SpriteId, Sprites};
use crate::game::crushers::{BrickSmashed, Crumble, CrusherBlock, Sprung};
use crate::game::LevelTile;
use crate::level::{CrushDir, TILE, Tile};

pub(super) fn plugin(app: &mut App) {
    app.add_observer(crusher_sprite).add_systems(
        PostUpdate,
        (crumble_sprites, smashed_bricks, squash_trampolines)
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
