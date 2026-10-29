//! The crutch pit (`game::crutches`): the WET FLOOR sign, Han's plunger on its rope, Nat flattened
//! in the pit and dangling on the haul, and his crutches while he's on them.

use bevy::prelude::*;
use bevy::sprite::Anchor;

use super::{CharacterSprite, VisualSet};
use crate::art::{SpriteId, Sprites};
use crate::game::crutches::{CrutchRescue, OnCrutches, ROPE_FROM, RescuePhase, RescuePlunger, WetFloorSign};
use crate::game::{Body, Player, tuning};

const ROPE: Color = Color::srgb(0.55, 0.36, 0.16);

pub(super) fn plugin(app: &mut App) {
    app.add_observer(sign)
        .add_observer(plunger)
        .add_observer(put_on_crutches)
        .add_observer(take_off_crutches)
        .add_systems(
            PostUpdate,
            (hobble, (flatten_or_dangle).run_if(resource_exists::<CrutchRescue>))
                .after(VisualSet::Animate)
                .before(VisualSet::Camera)
                .run_if(resource_exists::<Sprites>),
        );
}

fn sign(add: On<Add, WetFloorSign>, sprites: Option<Res<Sprites>>, mut commands: Commands) {
    let Some(sprites) = sprites else { return };
    commands.entity(add.entity).insert(Sprite::from_image(sprites.get(SpriteId::WetFloor)));
}

fn plunger(add: On<Add, RescuePlunger>, sprites: Option<Res<Sprites>>, mut commands: Commands) {
    let Some(sprites) = sprites else { return };
    commands.entity(add.entity).insert(Visibility::default()).with_children(|p| {
        p.spawn((Sprite::from_image(sprites.get(SpriteId::RescuePlunger)), Anchor::BOTTOM_CENTER, Transform::default()));
        // The rope, on up off the screen.
        p.spawn((Sprite::from_color(ROPE, Vec2::new(1.0, ROPE_FROM * 2.0)), Anchor::BOTTOM_CENTER, Transform::from_xyz(0.5, 15.0, -0.1)));
    });
}

/// The crutches (and casts), drawn over Nat like his own sprite.
#[derive(Component)]
struct CrutchSprite;

fn put_on_crutches(add: On<Add, OnCrutches>, sprites: Option<Res<Sprites>>, mut commands: Commands) {
    let Some(sprites) = sprites else { return };
    commands.entity(add.entity).with_child((
        CrutchSprite,
        Sprite::from_image(sprites.get(SpriteId::Crutches)),
        Anchor::BOTTOM_CENTER,
        Transform::from_xyz(0.0, -tuning::PLAYER_SIZE.1 / 2.0, 0.1),
    ));
}

fn take_off_crutches(remove: On<Remove, OnCrutches>, children: Query<&Children>, crutches: Query<(), With<CrutchSprite>>, mut commands: Commands) {
    let Ok(kids) = children.get(remove.entity) else { return };
    for &c in kids {
        if crutches.contains(c) {
            commands.entity(c).despawn();
        }
    }
}

/// The crutches swing as Nat hobbles (and hide while he's splatted).
#[allow(clippy::type_complexity)]
fn hobble(
    time: Res<Time>,
    sprites: Res<Sprites>,
    nat: Query<(&Body, &Children, Has<crate::game::Dead>), (With<Player>, With<OnCrutches>)>,
    mut crutches: Query<(&mut Sprite, &mut Visibility), With<CrutchSprite>>,
) {
    for (body, kids, dead) in &nat {
        for &c in kids {
            if let Ok((mut s, mut v)) = crutches.get_mut(c) {
                v.set_if_neq(if dead { Visibility::Hidden } else { Visibility::Inherited });
                let moving = body.vel.x.abs() > 10.0 || !body.on_ground;
                s.image = sprites.frame(SpriteId::Crutches, if moving { (time.elapsed_secs() * 5.0) as usize } else { 0 });
            }
        }
    }
}

/// Flat in the pit; stretched, dangling from the plunger on the haul.
fn flatten_or_dangle(rescue: Res<CrutchRescue>, nat: Query<&Children, With<Player>>, mut kids: Query<&mut CharacterSprite>) {
    let squash = match rescue.phase {
        RescuePhase::Ouch | RescuePhase::Lower => Vec2::new(1.35, 0.6),
        RescuePhase::Stuck | RescuePhase::Haul => Vec2::new(0.85, 1.2),
        RescuePhase::Hospital => return,
    };
    for children in &nat {
        for &c in children {
            if let Ok(mut cs) = kids.get_mut(c) {
                cs.squash = squash;
            }
        }
    }
}
