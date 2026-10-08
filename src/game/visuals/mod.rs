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

use super::han::{HanAnim, HanBrain, HanMode, HanPose, SprayPlug};
use super::physics::{Body, Dead, PlayerControl};
use super::{
    Checkpoint, Fly, Goal, Groove, Han, LevelEntity, LevelTile, MovingPlatform, Nugget, Player, Pos,
    PrevPos, Spray, Stain, tuning,
};
use crate::art::{SpriteId, Sprites};
use crate::events::{Jumped, Landed};
use crate::level::{Level, PlatformKind, TILE, Tile, Topic};
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
    .add_observer(stain_sprite)
    .add_observer(side_stain_sprite)
    .add_systems(PostUpdate, fade_side_stain_sprites.run_if(resource_exists::<Sprites>))
    .add_observer(player_sprite)
    .add_observer(han_sprite)
    .add_observer(gate_markers)
    .init_resource::<MarkersDrawn>()
    .add_systems(Update, new_gate_markers.run_if(resource_exists::<Sprites>))
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
            han_fx,
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

/// Dev shortcut: `NATHAN_LEVEL=3 cargo run` starts straight in level 3.
#[cfg(not(target_arch = "wasm32"))]
fn dev_jump_to_level(
    mut current: ResMut<crate::state::CurrentLevel>,
    mut next: ResMut<NextState<crate::state::AppState>>,
) {
    if let Some(n) = std::env::var("NATHAN_LEVEL").ok().and_then(|v| v.parse::<usize>().ok()) {
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
    /// Drawn size (the laughing band's melting phrase shrinks Nat), eased.
    pub size: f32,
}

impl Default for CharacterSprite {
    fn default() -> Self {
        Self { squash: Vec2::ONE, size: 1.0 }
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
        Tile::Grease if t.top => (SpriteId::Grease(w), false),
        Tile::Grease => (SpriteId::GroundFill(w), false),
        Tile::StainUp => (SpriteId::StainUp, false),
        Tile::StainDown => (SpriteId::StainDown, false),
    };
    let mut e = commands.entity(add.entity);
    e.insert(sprite(&sprites, id));
    if anim {
        e.insert(FrameAnim { id, fps: 4.0, offset: t.col as f32 * 0.13 });
    }
}

fn nugget_sprite(
    add: On<Add, Nugget>,
    q: Query<(&Transform, Has<crate::game::GhostNugget>)>,
    sprites: Option<Res<Sprites>>,
    mut commands: Commands,
) {
    let Some(sprites) = sprites else { return };
    let (offset, ghost) = q.get(add.entity).map_or((0.0, false), |(t, g)| (t.translation.x * 0.011, g));
    let mut s = sprite(&sprites, SpriteId::Nugget);
    if ghost {
        // Already counted: faded.
        s.color = Color::srgba(1.0, 1.0, 1.0, 0.4);
    }
    commands.entity(add.entity).insert((
        s,
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

/// A splat stain, drawn over the spikes it covers.
fn stain_sprite(add: On<Add, Stain>, q: Query<&Stain>, sprites: Option<Res<Sprites>>, mut commands: Commands) {
    let (Some(sprites), Ok(s)) = (sprites, q.get(add.entity)) else { return };
    let id = if s.tile == Tile::StainDown { SpriteId::StainDown } else { SpriteId::StainUp };
    commands.entity(add.entity).insert(sprite(&sprites, id));
}

/// A side splat: the stain turned onto the spike tile's face Nat ran into, fading as it expires.
fn side_stain_sprite(
    add: On<Add, crate::game::SideStain>,
    q: Query<&crate::game::SideStain>,
    sprites: Option<Res<Sprites>>,
    mut commands: Commands,
) {
    let (Some(sprites), Ok(s)) = (sprites, q.get(add.entity)) else { return };
    let _ = s; // (Rotation onto the face is applied by `fade_side_stain_sprites`.)
    commands.entity(add.entity).insert(sprite(&sprites, SpriteId::StainUp));
}

fn fade_side_stain_sprites(mut q: Query<(&crate::game::SideStain, &mut Sprite, &mut Transform)>) {
    for (s, mut sprite, mut tf) in &mut q {
        sprite.color = Color::srgba(1.0, 1.0, 1.0, (s.life / crate::game::SIDE_STAIN_LIFE).clamp(0.0, 1.0));
        tf.rotation = Quat::from_rotation_z(-s.side * std::f32::consts::FRAC_PI_2);
    }
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
        PlatformKind::Raft => SpriteId::StainRaft,
    };
    if p.kind == PlatformKind::Raft && p.width > 1 {
        // Han's big raft: his splatted overalls, three segments.
        let width = p.width;
        commands.entity(add.entity).insert(Visibility::default()).with_children(|c| {
            for i in 0..width {
                let x = (i as f32 - (width as f32 - 1.0) / 2.0) * TILE;
                let frame = if i == 0 { 0 } else if i + 1 == width { 2 } else { 1 };
                c.spawn((Sprite::from_image(sprites.frame(SpriteId::HanRaft, frame)), Transform::from_xyz(x, 0.0, 0.0)));
            }
        });
        return;
    }
    if p.kind == PlatformKind::Raft {
        // One bobbing blob.
        commands.entity(add.entity).insert(Visibility::default()).with_child((
            Sprite::from_image(sprites.get(id)),
            FrameAnim { id, fps: 2.0, offset: p.base.x * 0.01 },
            Transform::default(),
        ));
        return;
    }
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

/// What a gate mark looks like, and where (world center of the overlay, z): giant walls get
/// gold music-staff trim on their face and a note emblem; buddy ledges red plunger-handle
/// notches on their face and Han's yellow plumber's tape along the top; shield rows a
/// "PLUMBERS ONLY" sign over their start.
pub fn gate_decor(level: &Level) -> Vec<(SpriteId, Vec2, f32)> {
    marks_decor(level, &level.gates)
}

/// [`gate_decor`] for some of `level`'s marks.
fn marks_decor(level: &Level, marks: &[crate::level::GateMark]) -> Vec<(SpriteId, Vec2, f32)> {
    let mut out = Vec::new();
    for m in marks {
        // The faces: solid cells in the mark with open air beside them, inside the mark.
        let mut faces = Vec::new();
        for c in m.c0..=m.c1 + 1 {
            for r in m.r0 - 1..=m.r1 {
                let open = |cc: i32| m.contains((cc, r)) && !level.tile(cc, r).is_solid();
                if level.tile(c, r).is_solid() && c >= 0 && r >= 0 && (open(c - 1) || open(c + 1)) {
                    faces.push((c, r));
                }
            }
        }
        let at = |(c, r): (i32, i32)| level.tile_center(c.max(0) as usize, r.max(0) as usize);
        match m.topic {
            Topic::Giant => {
                for &f in &faces {
                    out.push((SpriteId::GiantTrim, at(f), 0.4));
                }
                if let Some(&mid) = faces.get(faces.len() / 2) {
                    out.push((SpriteId::GiantEmblem, at(mid), 0.45));
                }
            }
            Topic::Boost => {
                for &(c, r) in &faces {
                    out.push((SpriteId::LedgeNotch, at((c, r)), 0.4));
                    if !level.tile(c, r - 1).is_solid() {
                        out.push((SpriteId::PlumberTape, at((c, r)), 0.45));
                    }
                }
            }
            Topic::Shield => {
                let p = at((m.c0, m.r0)) + Vec2::new(0.0, TILE);
                out.push((SpriteId::PlumbersOnly, p, 0.6));
            }
            _ => {}
        }
    }
    out
}

/// Gate marks of the loaded level that have their markers.
#[derive(Resource, Default)]
struct MarkersDrawn(usize);

/// Gate markers, drawn when a level's Han arrives (once per level load).
fn gate_markers(
    _add: On<Add, Han>,
    active: Option<Res<super::ActiveLevel>>,
    sprites: Option<Res<Sprites>>,
    mut drawn: ResMut<MarkersDrawn>,
    mut commands: Commands,
) {
    let (Some(active), Some(sprites)) = (active, sprites) else { return };
    spawn_markers(&mut commands, &sprites, gate_decor(&active.level));
    drawn.0 = active.level.gates.len();
}

/// ...and for the marks added since (free play's rooms, stitched in as Nat goes; once this
/// level's Han is here, so the count is this level's).
fn new_gate_markers(
    active: Option<Res<super::ActiveLevel>>,
    run: Option<Res<crate::freeplay::FreePlayRun>>,
    han: Query<(), With<Han>>,
    sprites: Res<Sprites>,
    mut drawn: ResMut<MarkersDrawn>,
    mut commands: Commands,
) {
    let (Some(active), Some(_)) = (active, run) else { return };
    if han.is_empty() {
        return;
    }
    let marks = &active.level.gates;
    if marks.len() > drawn.0 {
        spawn_markers(&mut commands, &sprites, marks_decor(&active.level, &marks[drawn.0..]));
        drawn.0 = marks.len();
    }
}

fn spawn_markers(commands: &mut Commands, sprites: &Sprites, decor: Vec<(SpriteId, Vec2, f32)>) {
    for (id, at, z) in decor {
        commands.spawn((
            Name::new("GateMarker"),
            LevelEntity,
            sprite(sprites, id),
            Transform::from_translation(at.extend(z)),
        ));
    }
}

/// The plunger boost and Han's sewage splat: a puff, a sound.
fn han_fx(
    mut commands: Commands,
    sprites: Res<Sprites>,
    mut boosted: MessageReader<super::HanBoosted>,
    han: Query<(&HanBrain, &Pos), With<Han>>,
    mut sfx: MessageWriter<crate::events::PlaySfx>,
    mut was_sinking: Local<bool>,
) {
    for b in boosted.read() {
        sfx.write(crate::events::PlaySfx(crate::audio::Sfx::Jump));
        commands.spawn((
            LevelEntity,
            Particle { vel: Vec2::new(0.0, -30.0), gravity: 0.0, life: 0.4, max_life: 0.4, fade: true },
            Sprite::from_image(sprites.get(SpriteId::TootPuff)),
            FrameAnim { id: SpriteId::TootPuff, fps: 10.0, offset: 0.0 },
            Transform::from_translation((b.pos + Vec2::new(0.0, -12.0)).extend(6.0)),
        ));
    }
    let sinking = han.single().is_ok_and(|(b, _)| matches!(b.mode, HanMode::Sinking { .. }));
    if sinking && !*was_sinking {
        sfx.write(crate::events::PlaySfx(crate::audio::Sfx::Splat));
    }
    *was_sinking = sinking;
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
    mut han: Query<(&HanAnim, &Children, &mut Visibility), With<Han>>,
    mut kids: Query<(&mut Sprite, &mut CharacterSprite, &mut Transform)>,
    mut was_jumping: Local<bool>,
) {
    let t = time.elapsed_secs();
    for (anim, children, mut vis) in &mut han {
        let want = if anim.hidden { Visibility::Hidden } else { Visibility::Inherited };
        if *vis != want {
            *vis = want;
        }
        let jumping = matches!(anim.pose, HanPose::Jump | HanPose::Paddle);
        for &child in children {
            let Ok((mut sprite, mut cs, mut tf)) = kids.get_mut(child) else { continue };
            if jumping && !*was_jumping {
                cs.squash = Vec2::new(0.8, 1.2);
            } else if !jumping && *was_jumping {
                cs.squash = Vec2::new(1.25, 0.8);
            }
            let (id, frame) = match anim.pose {
                HanPose::Idle => (SpriteId::HanIdle, (t * 3.0) as usize),
                HanPose::Run => (SpriteId::HanRun, (t * 12.0) as usize),
                HanPose::Jump => (SpriteId::HanJump, (t * 10.0) as usize),
                HanPose::Braced => (SpriteId::HanBraced, (t * 4.0) as usize),
                HanPose::Intercept => (SpriteId::HanIntercept, (t * 8.0) as usize),
                HanPose::March => (SpriteId::HanMarch, (t * 9.0) as usize),
                HanPose::Parachute => (SpriteId::HanParachute, (t * 2.0) as usize),
                HanPose::Winded => (SpriteId::HanWinded, (t * 3.0) as usize),
                HanPose::Splat => (SpriteId::HanSplat, (t * 3.0) as usize % 3),
                HanPose::Paddle => (SpriteId::HanPaddle, (t * 8.0) as usize),
                HanPose::Roll => (SpriteId::HanRoll, (t * 14.0) as usize),
            };
            let img = sprites.frame(id, frame);
            if sprite.image != img {
                sprite.image = img;
            }
            sprite.flip_x = anim.facing_left;
            // The nervous band: he trembles. A jet or a fly: a comedic wobble.
            tf.translation.x = if anim.tremble && (t * 30.0) as i32 % 2 == 0 { 1.0 } else { 0.0 };
            let wobble = anim.wobble * 0.35 * (t * 28.0).sin();
            tf.rotation = Quat::from_rotation_z(wobble);
        }
        *was_jumping = jumping;
    }
}

fn relax_squash(
    time: Res<Time>,
    groove: Option<Res<Groove>>,
    player: Query<&Children, With<Player>>,
    mut q: Query<(Entity, &mut CharacterSprite, &mut Transform)>,
) {
    let k = 1.0 - (-time.delta_secs() * 14.0).exp();
    // Melting (the laughing band's Carlos alpha phrase): Nat shrinks, slowly.
    let melt = 1.0 - (-time.delta_secs() * 3.0).exp();
    let nat_size = groove.map_or(1.0, |g| g.nat_size());
    for (e, mut cs, mut tf) in &mut q {
        if player.iter().any(|c| c.contains(&e)) {
            cs.size += (nat_size - cs.size) * melt;
        }
        cs.squash = cs.squash.lerp(Vec2::ONE, k);
        tf.scale = (cs.squash * cs.size).extend(1.0);
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
    sprays: Query<(&Spray, &Children, &Transform, Option<&SprayPlug>), Or<(Changed<Spray>, With<SprayPlug>)>>,
    mut jets: Query<(&mut Visibility, &Transform), (With<JetSegment>, Without<Spray>)>,
) {
    for (spray, children, tf, plug) in &sprays {
        // Han's body stops the jet: segments above where it's cut off don't show.
        let base = tf.translation.truncate() + Vec2::new(0.0, TILE);
        let top = if spray.on { SprayPlug::jet(plug, base).map(|(_, hi)| hi.y) } else { None };
        for &c in children {
            if let Ok((mut vis, seg)) = jets.get_mut(c) {
                let bottom = tf.translation.y + seg.translation.y - TILE / 2.0;
                let want = if top.is_some_and(|t| t > bottom + 4.0) { Visibility::Inherited } else { Visibility::Hidden };
                if *vis != want {
                    *vis = want;
                }
            }
        }
    }
}
