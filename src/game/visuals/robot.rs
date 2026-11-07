//! The robot pit's wrong game (`game::robot`): Sparky's meadow, Sparky, the answer pads, the
//! chest, the whirlpool (and Nat swirling down it), and Sparky's antenna in the pit.

use bevy::prelude::*;
use bevy::sprite::Anchor;

use super::{CharacterSprite, FrameAnim, VisualSet};
use crate::art::{SpriteId, Sprites};
use crate::game::robot::{AnswerPad, PitAntenna, RobotRoom, RoomChest, RoomPhase, RoomTile, Sparky, Whirlpool, FLUSH_SECS};
use crate::game::Player;

pub(super) fn plugin(app: &mut App) {
    app.add_observer(room_tile)
        .add_observer(sparky)
        .add_observer(pad)
        .add_observer(chest)
        .add_observer(whirlpool)
        .add_observer(antenna)
        .add_systems(
            PostUpdate,
            (update_room, swirl.run_if(resource_exists::<RobotRoom>))
                .after(VisualSet::Animate)
                .before(VisualSet::Camera)
                .run_if(resource_exists::<Sprites>),
        );
}

fn room_tile(add: On<Add, RoomTile>, q: Query<&RoomTile>, sprites: Option<Res<Sprites>>, mut commands: Commands) {
    let (Some(sprites), Ok(tile)) = (sprites, q.get(add.entity)) else { return };
    let id = match tile.0 {
        'T' => SpriteId::RobotTree,
        ',' => SpriteId::RobotPath,
        'f' => SpriteId::RobotFlowers,
        'R' => SpriteId::RobotRoof,
        'H' => SpriteId::RobotWall,
        'W' => SpriteId::RobotWindow,
        'D' => SpriteId::RobotDoor,
        'F' => SpriteId::RobotFence,
        '~' => SpriteId::RobotWater,
        _ => SpriteId::RobotGrass,
    };
    let mut e = commands.entity(add.entity);
    e.insert(Sprite::from_image(sprites.get(id)));
    if id == SpriteId::RobotWater {
        e.insert(FrameAnim { id, fps: 2.0, offset: 0.0 });
    }
}

fn sparky(add: On<Add, Sparky>, sprites: Option<Res<Sprites>>, mut commands: Commands) {
    let Some(sprites) = sprites else { return };
    // (His middle's the tile's: feet at its bottom.)
    commands.entity(add.entity).insert(Visibility::default()).with_child((
        Sprite::from_image(sprites.get(SpriteId::Sparky)),
        Anchor::BOTTOM_CENTER,
        Transform::from_xyz(0.0, -8.0, 0.0),
        SparkyBody,
    ));
}

#[derive(Component)]
struct SparkyBody;

fn pad(add: On<Add, AnswerPad>, sprites: Option<Res<Sprites>>, mut commands: Commands) {
    let Some(sprites) = sprites else { return };
    commands.entity(add.entity).insert(Sprite::from_image(sprites.get(SpriteId::AnswerPad)));
}

fn chest(add: On<Add, RoomChest>, sprites: Option<Res<Sprites>>, mut commands: Commands) {
    let Some(sprites) = sprites else { return };
    commands.entity(add.entity).insert(Sprite::from_image(sprites.get(SpriteId::RobotChest)));
}

fn whirlpool(add: On<Add, Whirlpool>, sprites: Option<Res<Sprites>>, mut commands: Commands) {
    let Some(sprites) = sprites else { return };
    commands.entity(add.entity).insert((Sprite::from_image(sprites.get(SpriteId::Whirlpool)), Visibility::Hidden));
}

fn antenna(add: On<Add, PitAntenna>, sprites: Option<Res<Sprites>>, mut commands: Commands) {
    let Some(sprites) = sprites else { return };
    commands
        .entity(add.entity)
        .insert((Sprite::from_image(sprites.get(SpriteId::PitAntenna)), FrameAnim { id: SpriteId::PitAntenna, fps: 2.5, offset: 0.0 }));
}

#[allow(clippy::type_complexity)]
fn update_room(
    time: Res<Time>,
    sprites: Res<Sprites>,
    mut pads: Query<(&AnswerPad, &mut Sprite), (Without<RoomChest>, Without<Whirlpool>, Without<SparkyBody>)>,
    mut chests: Query<(&RoomChest, &mut Sprite), (Without<Whirlpool>, Without<SparkyBody>)>,
    mut pools: Query<(&Whirlpool, &mut Sprite, &mut Visibility), Without<SparkyBody>>,
    mut body: Query<(&mut Sprite, &mut Transform), With<SparkyBody>>,
) {
    let t = time.elapsed_secs();
    for (p, mut s) in &mut pads {
        s.image = sprites.frame(SpriteId::AnswerPad, p.used as usize);
    }
    for (c, mut s) in &mut chests {
        s.image = sprites.frame(SpriteId::RobotChest, c.open as usize);
    }
    for (w, mut s, mut v) in &mut pools {
        v.set_if_neq(if w.open { Visibility::Inherited } else { Visibility::Hidden });
        s.image = sprites.frame(SpriteId::Whirlpool, (t * 8.0) as usize);
    }
    // Sparky bobs (like his game's) and blinks now and then.
    for (mut s, mut tf) in &mut body {
        tf.translation.y = -8.0 + ((t * 3.0).sin() * 1.5).round();
        s.image = sprites.frame(SpriteId::Sparky, ((t * 1.3).fract() > 0.92) as usize);
    }
}

/// Down the whirlpool: Nat spins and shrinks away.
fn swirl(room: Res<RobotRoom>, nat: Query<&Children, With<Player>>, mut kids: Query<&mut Transform, With<CharacterSprite>>) {
    let Ok(children) = nat.single() else { return };
    let k = if room.phase == RoomPhase::Flush { (room.t / FLUSH_SECS).min(1.0) } else { 0.0 };
    for &c in children {
        if let Ok(mut tf) = kids.get_mut(c) {
            if k > 0.0 {
                tf.scale = Vec3::splat(1.0 - k);
                tf.rotation = Quat::from_rotation_z(k * k * 25.0);
            } else if tf.rotation != Quat::IDENTITY {
                tf.rotation = Quat::IDENTITY;
            }
        }
    }
}
