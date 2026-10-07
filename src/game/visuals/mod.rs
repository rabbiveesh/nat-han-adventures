//! Presentation for the simulation: sprites for every level entity, animation, squash & stretch,
//! interpolation between fixed steps, the camera, the backdrop and particles.
//!
//! Gameplay entities never require a `Sprite`; sprites are attached here by observers when the
//! gameplay components are added (and only if the [`Sprites`] resource exists).

mod camera;
mod particles;

use bevy::prelude::*;
use bevy::sprite::Anchor;

pub use camera::GameCamera;
pub use particles::Particle;

use super::han::{HanAnim, HanPose};
use super::physics::{Body, Dead, PlayerControl};
use super::{
    Checkpoint, Fly, Goal, Han, LevelEntity, LevelTile, MovingPlatform, Nugget, Player, Pos,
    PrevPos, Spray, tuning,
};
use crate::art::{SpriteId, Sprites};
use crate::events::{Jumped, Landed};
use crate::level::{PlatformKind, TILE, Tile};
use crate::state::PlayState;

/// Ordering of the presentation systems in `PostUpdate` (all before transform propagation).
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VisualSet {
    Interpolate,
    Animate,
    Camera,
}

pub(super) fn plugin(app: &mut App) {
    app.configure_sets(
        PostUpdate,
        (VisualSet::Interpolate, VisualSet::Animate, VisualSet::Camera)
            .chain()
            .before(bevy::transform::TransformSystems::Propagate),
    )
    .add_observer(tile_sprite)
    .add_observer(nugget_sprite)
    .add_observer(checkpoint_sprite)
    .add_observer(goal_sprite)
    .add_observer(fly_sprite)
    .add_observer(spray_sprite)
    .add_observer(platform_sprite)
    .add_observer(player_sprite)
    .add_observer(han_sprite)
    .add_systems(PostUpdate, interpolate.in_set(VisualSet::Interpolate))
    .add_systems(
        PostUpdate,
        (
            frame_anims,
            animate_player,
            animate_han,
            relax_squash,
            update_checkpoints,
            update_sprays,
        )
            .chain()
            .in_set(VisualSet::Animate)
            .run_if(resource_exists::<Sprites>),
    );
    camera::plugin(app);
    particles::plugin(app);
    #[cfg(not(target_arch = "wasm32"))]
    app.add_systems(Startup, dev_jump_to_level);
}

/// Dev shortcut: `DURHAY_LEVEL=3 cargo run` starts straight in level 3.
#[cfg(not(target_arch = "wasm32"))]
fn dev_jump_to_level(
    mut current: ResMut<crate::state::CurrentLevel>,
    mut next: ResMut<NextState<crate::state::AppState>>,
) {
    if let Some(n) = std::env::var("DURHAY_LEVEL").ok().and_then(|v| v.parse::<usize>().ok()) {
        current.0 = n.saturating_sub(1);
        next.set(crate::state::AppState::Playing);
    }
}

/// Loops through the frames of `id` at `fps`.
#[derive(Component, Debug, Clone, Copy)]
pub struct FrameAnim {
    pub id: SpriteId,
    pub fps: f32,
    /// Seconds added to the clock, so identical things don't animate in lockstep.
    pub offset: f32,
}

/// The sprite child of the player / Han. Scaled for squash & stretch around the feet.
#[derive(Component, Debug, Clone, Copy)]
pub struct CharacterSprite {
    pub squash: Vec2,
}

impl Default for CharacterSprite {
    fn default() -> Self {
        Self { squash: Vec2::ONE }
    }
}

/// A jet segment child of a spray can.
#[derive(Component, Debug, Clone, Copy)]
struct JetSegment;

fn sprite(sprites: &Sprites, id: SpriteId) -> Sprite {
    Sprite::from_image(sprites.get(id))
}

fn tile_sprite(
    add: On<Add, LevelTile>,
    q: Query<&LevelTile>,
    sprites: Option<Res<Sprites>>,
    mut commands: Commands,
) {
    let (Some(sprites), Ok(t)) = (sprites, q.get(add.entity)) else { return };
    let w = t.world;
    let (id, anim) = match t.tile {
        Tile::Empty => return,
        Tile::Solid if t.top => (SpriteId::GroundTop(w), false),
        Tile::Solid => (SpriteId::GroundFill(w), false),
        Tile::OneWay => (SpriteId::OneWay(w), false),
        Tile::SpikesUp => (SpriteId::SpikesUp, false),
        Tile::SpikesDown => (SpriteId::SpikesDown, false),
        Tile::Liquid if t.top => (SpriteId::LiquidTop(w), true),
        Tile::Liquid => (SpriteId::LiquidFill(w), false),
    };
    let mut e = commands.entity(add.entity);
    e.insert(sprite(&sprites, id));
    if anim {
        e.insert(FrameAnim { id, fps: 4.0, offset: t.col as f32 * 0.13 });
    }
}

fn nugget_sprite(
    add: On<Add, Nugget>,
    q: Query<&Transform>,
    sprites: Option<Res<Sprites>>,
    mut commands: Commands,
) {
    let Some(sprites) = sprites else { return };
    let offset = q.get(add.entity).map_or(0.0, |t| t.translation.x * 0.011);
    commands.entity(add.entity).insert((
        sprite(&sprites, SpriteId::Nugget),
        FrameAnim { id: SpriteId::Nugget, fps: 8.0, offset },
    ));
}

fn checkpoint_sprite(add: On<Add, Checkpoint>, sprites: Option<Res<Sprites>>, mut commands: Commands) {
    let Some(sprites) = sprites else { return };
    commands
        .entity(add.entity)
        .insert((sprite(&sprites, SpriteId::CheckpointOff), Anchor::BOTTOM_CENTER));
}

fn goal_sprite(
    add: On<Add, Goal>,
    q: Query<&Transform>,
    sprites: Option<Res<Sprites>>,
    mut commands: Commands,
) {
    let Some(sprites) = sprites else { return };
    commands.entity(add.entity).insert((
        sprite(&sprites, SpriteId::GoalFlag),
        Anchor::BOTTOM_CENTER,
        FrameAnim { id: SpriteId::GoalFlag, fps: 5.0, offset: 0.0 },
    ));
    // The throne, just right of the flag (decorative).
    if let Ok(tf) = q.get(add.entity) {
        let at = tf.translation.truncate() + Vec2::new(TILE * 1.5, 0.0);
        commands.spawn((
            Name::new("Throne"),
            LevelEntity,
            sprite(&sprites, SpriteId::Throne),
            Anchor::BOTTOM_CENTER,
            Transform::from_translation(at.extend(1.5)),
        ));
    }
}

fn fly_sprite(
    add: On<Add, Fly>,
    q: Query<&Fly>,
    sprites: Option<Res<Sprites>>,
    mut commands: Commands,
) {
    let Some(sprites) = sprites else { return };
    let offset = q.get(add.entity).map_or(0.0, |f| f.phase);
    commands.entity(add.entity).insert((
        sprite(&sprites, SpriteId::Fly),
        FrameAnim { id: SpriteId::Fly, fps: 16.0, offset },
    ));
}

fn spray_sprite(add: On<Add, Spray>, sprites: Option<Res<Sprites>>, mut commands: Commands) {
    let Some(sprites) = sprites else { return };
    commands
        .entity(add.entity)
        .insert((sprite(&sprites, SpriteId::SprayCan), Anchor::BOTTOM_CENTER))
        .with_children(|p| {
            for k in 1..=3 {
                p.spawn((
                    JetSegment,
                    sprite(&sprites, SpriteId::SprayJet),
                    FrameAnim { id: SpriteId::SprayJet, fps: 12.0, offset: k as f32 * 0.1 },
                    Transform::from_xyz(0.0, k as f32 * TILE + TILE / 2.0, 0.1),
                    Visibility::Hidden,
                ));
            }
        });
}

fn platform_sprite(
    add: On<Add, MovingPlatform>,
    q: Query<&MovingPlatform>,
    sprites: Option<Res<Sprites>>,
    mut commands: Commands,
) {
    let (Some(sprites), Ok(p)) = (sprites, q.get(add.entity)) else { return };
    let id = match p.kind {
        PlatformKind::Tp => SpriteId::PlatformTp,
        PlatformKind::Duck => SpriteId::PlatformDuck,
        PlatformKind::Plunger => SpriteId::PlatformPlunger,
    };
    let width = p.width;
    commands.entity(add.entity).insert(Visibility::default()).with_children(|c| {
        for i in 0..width {
            let x = (i as f32 - (width as f32 - 1.0) / 2.0) * TILE;
            // Alternate frames so long platforms don't look stamped.
            c.spawn((Sprite::from_image(sprites.frame(id, i)), Transform::from_xyz(x, 0.0, 0.0)));
        }
    });
}

/// Characters' sprite child: feet at the bottom of the collision box, anchored bottom-center
/// so squash & stretch keeps the feet planted.
fn character_child(sprites: &Sprites, id: SpriteId) -> impl Bundle {
    (
        CharacterSprite::default(),
        sprite(sprites, id),
        Anchor::BOTTOM_CENTER,
        Transform::from_xyz(0.0, -tuning::PLAYER_SIZE.1 / 2.0, 0.0),
    )
}

fn player_sprite(add: On<Add, Player>, sprites: Option<Res<Sprites>>, mut commands: Commands) {
    let Some(sprites) = sprites else { return };
    let child = character_child(&sprites, SpriteId::PooIdle);
    commands.entity(add.entity).insert(Visibility::default()).with_child(child);
}

fn han_sprite(add: On<Add, Han>, sprites: Option<Res<Sprites>>, mut commands: Commands) {
    let Some(sprites) = sprites else { return };
    let child = character_child(&sprites, SpriteId::HanIdle);
    commands.entity(add.entity).insert(Visibility::default()).with_child(child);
}

/// Draw simulated entities between their last two fixed-step positions.
fn interpolate(
    fixed: Res<Time<Fixed>>,
    state: Option<Res<State<PlayState>>>,
    mut q: Query<(&Pos, &PrevPos, &mut Transform)>,
) {
    let running = state.is_some_and(|s| *s.get() == PlayState::Running);
    let a = if running { fixed.overstep_fraction() } else { 1.0 };
    for (pos, prev, mut tf) in &mut q {
        let p = prev.0.lerp(pos.0, a);
        tf.translation.x = p.x;
        tf.translation.y = p.y;
    }
}

fn frame_anims(time: Res<Time>, sprites: Res<Sprites>, mut q: Query<(&FrameAnim, &mut Sprite)>) {
    let t = time.elapsed_secs();
    for (anim, mut sprite) in &mut q {
        let frames = sprites.frames(anim.id);
        if frames.len() < 2 {
            continue;
        }
        let i = (((t + anim.offset) * anim.fps) as usize) % frames.len();
        if sprite.image != frames[i] {
            sprite.image = frames[i].clone();
        }
    }
}

/// Seconds the splat stays visible before the player vanishes until respawn.
const SPLAT_SHOW: f32 = 0.5;

#[allow(clippy::type_complexity)]
fn animate_player(
    time: Res<Time>,
    sprites: Res<Sprites>,
    player: Query<(&Body, &PlayerControl, Option<&Dead>, &Children), With<Player>>,
    mut kids: Query<(&mut Sprite, &mut CharacterSprite, &mut Visibility)>,
    mut jumped: MessageReader<Jumped>,
    mut landed: MessageReader<Landed>,
) {
    let Ok((body, ctl, dead, children)) = player.single() else {
        jumped.clear();
        landed.clear();
        return;
    };
    let t = time.elapsed_secs();
    for &child in children {
        let Ok((mut sprite, mut cs, mut vis)) = kids.get_mut(child) else { continue };
        for _ in jumped.read() {
            cs.squash = Vec2::new(0.75, 1.3);
        }
        for l in landed.read() {
            let k = (l.speed / tuning::MAX_FALL).clamp(0.3, 1.0);
            cs.squash = Vec2::new(1.0 + 0.4 * k, 1.0 - 0.35 * k);
        }
        let (id, frame) = if let Some(dead) = dead {
            let since = tuning::RESPAWN_DELAY - dead.remaining;
            *vis = if since < SPLAT_SHOW { Visibility::Inherited } else { Visibility::Hidden };
            cs.squash = Vec2::ONE;
            let n = sprites.frames(SpriteId::PooSplat).len().max(1);
            (SpriteId::PooSplat, ((since * 14.0) as usize).min(n - 1))
        } else {
            *vis = Visibility::Inherited;
            if !body.on_ground {
                (if body.vel.y > 0.0 { SpriteId::PooJump } else { SpriteId::PooFall }, (t * 10.0) as usize)
            } else if body.vel.x.abs() > 10.0 {
                (SpriteId::PooRun, (t * 12.0) as usize)
            } else {
                (SpriteId::PooIdle, (t * 3.0) as usize)
            }
        };
        let img = sprites.frame(id, frame);
        if sprite.image != img {
            sprite.image = img;
        }
        sprite.flip_x = ctl.facing < 0.0;
    }
}

fn animate_han(
    time: Res<Time>,
    sprites: Res<Sprites>,
    han: Query<(&HanAnim, &Children), With<Han>>,
    mut kids: Query<(&mut Sprite, &mut CharacterSprite)>,
    mut was_jumping: Local<bool>,
) {
    let t = time.elapsed_secs();
    for (anim, children) in &han {
        let jumping = anim.pose == HanPose::Jump;
        for &child in children {
            let Ok((mut sprite, mut cs)) = kids.get_mut(child) else { continue };
            if jumping && !*was_jumping {
                cs.squash = Vec2::new(0.8, 1.2);
            } else if !jumping && *was_jumping {
                cs.squash = Vec2::new(1.25, 0.8);
            }
            let (id, frame) = match anim.pose {
                HanPose::Idle => (SpriteId::HanIdle, (t * 3.0) as usize),
                HanPose::Run => (SpriteId::HanRun, (t * 12.0) as usize),
                HanPose::Jump => (SpriteId::HanJump, (t * 10.0) as usize),
            };
            let img = sprites.frame(id, frame);
            if sprite.image != img {
                sprite.image = img;
            }
            sprite.flip_x = anim.facing_left;
        }
        *was_jumping = jumping;
    }
}

fn relax_squash(time: Res<Time>, mut q: Query<(&mut CharacterSprite, &mut Transform)>) {
    let k = 1.0 - (-time.delta_secs() * 14.0).exp();
    for (mut cs, mut tf) in &mut q {
        cs.squash = cs.squash.lerp(Vec2::ONE, k);
        tf.scale = cs.squash.extend(1.0);
    }
}

fn update_checkpoints(
    sprites: Res<Sprites>,
    mut q: Query<(&Checkpoint, &mut Sprite), Changed<Checkpoint>>,
) {
    for (cp, mut sprite) in &mut q {
        sprite.image =
            sprites.get(if cp.active { SpriteId::CheckpointOn } else { SpriteId::CheckpointOff });
    }
}

fn update_sprays(
    sprays: Query<(&Spray, &Children), Changed<Spray>>,
    mut jets: Query<&mut Visibility, With<JetSegment>>,
) {
    for (spray, children) in &sprays {
        for &c in children {
            if let Ok(mut vis) = jets.get_mut(c) {
                *vis = if spray.on { Visibility::Inherited } else { Visibility::Hidden };
            }
        }
    }
}
