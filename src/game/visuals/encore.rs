//! NAT ALONE's encore gag (`game::encore`, [`Gag::Grate`]): the ceiling over the stage (the
//! floor of the room above), its grate over the throne (broken once Nat's gone through), and the
//! shutter that slams over the hole.

use bevy::prelude::*;

use crate::art::{SpriteId, Sprites};
use crate::game::encore::{Encore, GRATE_WIDTH};
use crate::game::{ActiveLevel, LevelEntity};
use crate::level::TILE;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, (spawn_seam.run_if(resource_added::<Encore>), update_seam).chain().run_if(resource_exists::<Sprites>));
}

#[derive(Component)]
struct Grate;

#[derive(Component)]
struct Shutter;

/// The ceiling's tiles, the grate, and the shutter (hidden under the tiles right of the hole).
fn spawn_seam(mut commands: Commands, encore: Res<Encore>, active: Option<Res<ActiveLevel>>, sprites: Res<Sprites>) {
    let (Some(seam), Some(active)) = (encore.seam, active) else { return };
    let w = active.level.world;
    let y = seam.bottom + TILE / 2.0;
    let mut x = seam.x0 + TILE / 2.0;
    while x < seam.x1 {
        if (x - seam.hole).abs() > GRATE_WIDTH / 2.0 {
            commands.spawn((
                Name::new("Seam"),
                LevelEntity,
                Sprite::from_image(sprites.get(SpriteId::GroundTop(w))),
                Transform::from_translation(Vec3::new(x, y, 0.2)),
            ));
        }
        x += TILE;
    }
    // The grate: the wall grate on its side.
    commands.spawn((
        Name::new("SeamGrate"),
        LevelEntity,
        Grate,
        Sprite::from_image(sprites.get(SpriteId::Grate)),
        Transform::from_translation(Vec3::new(seam.hole, y, 0.15)).with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
    ));
    commands
        .spawn((Name::new("SeamShutter"), LevelEntity, Shutter, Transform::from_translation(Vec3::new(seam.hole + GRATE_WIDTH, y, 0.1)), Visibility::Hidden))
        .with_children(|s| {
            for dx in [-TILE / 2.0, TILE / 2.0] {
                s.spawn((Sprite::from_image(sprites.get(SpriteId::GroundTop(w))), Transform::from_xyz(dx, 0.0, 0.0)));
            }
        });
}

fn update_seam(
    encore: Option<Res<Encore>>,
    mut grate: Query<&mut Visibility, (With<Grate>, Without<Shutter>)>,
    mut shutter: Query<(&mut Transform, &mut Visibility), (With<Shutter>, Without<Grate>)>,
) {
    let Some(e) = encore else { return };
    let Some(seam) = e.seam else { return };
    for mut vis in &mut grate {
        vis.set_if_neq(if e.through { Visibility::Hidden } else { Visibility::Inherited });
    }
    let s = e.shutter();
    for (mut tf, mut vis) in &mut shutter {
        // It slides out from under the ceiling right of the hole.
        tf.translation.x = seam.hole + GRATE_WIDTH * (1.0 - s);
        vis.set_if_neq(if s > 0.0 { Visibility::Inherited } else { Visibility::Hidden });
    }
}
