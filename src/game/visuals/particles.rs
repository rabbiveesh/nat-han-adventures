//! Cheap particles: short-lived tinted 2x2 sprites with velocity, gravity and fade, plus the
//! toot puff cloud, the trail of golden notes Nat leaves while the band plays Giant Steps, and
//! the golden sparkle of a waltz jump on ONE.

use bevy::prelude::*;

use super::FrameAnim;
use crate::art::{SpriteId, Sprites};
use crate::events::{CheckpointReached, Jumped, Landed, NuggetCollected, PlayerDied};
use crate::game::{Body, Dead, Groove, JumpedOnOne, LevelEntity, Player};
use crate::state::PlayState;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Rng>().init_resource::<NoteTrail>().add_systems(
        Update,
        (spawn_fx.run_if(resource_exists::<Sprites>), note_trail.run_if(resource_exists::<Sprites>), update_particles)
            .chain()
            .run_if(not(in_state(PlayState::Paused))),
    );
}

/// A short-lived effect sprite.
#[derive(Component, Debug, Clone, Copy)]
pub struct Particle {
    pub vel: Vec2,
    pub gravity: f32,
    pub life: f32,
    pub max_life: f32,
    /// Fade alpha out over the lifetime.
    pub fade: bool,
}

/// Tiny xorshift so effects don't need a global RNG.
#[derive(Resource)]
struct Rng(u32);

impl Default for Rng {
    fn default() -> Self {
        Self(0x9e37_79b9)
    }
}

impl Rng {
    fn f(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f()
    }
}

const MAX_PARTICLES: usize = 400;

struct Burst<'a> {
    count: usize,
    colors: &'a [Color],
    speed: (f32, f32),
    /// Direction range in radians.
    angle: (f32, f32),
    gravity: f32,
    life: (f32, f32),
    /// Square size in px (the sprite is 2x2).
    size: f32,
}

#[allow(clippy::too_many_arguments)]
fn spawn_fx(
    mut commands: Commands,
    sprites: Res<Sprites>,
    mut rng: ResMut<Rng>,
    existing: Query<(), With<Particle>>,
    mut jumped: MessageReader<Jumped>,
    mut landed: MessageReader<Landed>,
    mut died: MessageReader<PlayerDied>,
    mut nuggets: MessageReader<NuggetCollected>,
    mut checkpoints: MessageReader<CheckpointReached>,
    mut on_one: MessageReader<JumpedOnOne>,
) {
    use std::f32::consts::PI;
    let mut budget = MAX_PARTICLES.saturating_sub(existing.iter().count());
    let feet = Vec2::new(0.0, -7.0);
    let mut burst = |commands: &mut Commands, rng: &mut Rng, at: Vec2, b: Burst| {
        for _ in 0..b.count.min(budget) {
            let a = rng.range(b.angle.0, b.angle.1);
            let v = Vec2::from_angle(a) * rng.range(b.speed.0, b.speed.1);
            let life = rng.range(b.life.0, b.life.1);
            let color = b.colors[(rng.f() * b.colors.len() as f32) as usize % b.colors.len()];
            commands.spawn((
                LevelEntity,
                Particle { vel: v, gravity: b.gravity, life, max_life: life, fade: true },
                Sprite { image: sprites.get(SpriteId::Particle), color, ..default() },
                Transform::from_translation(at.extend(6.0)).with_scale(Vec3::splat(b.size / 2.0)),
            ));
        }
        budget = budget.saturating_sub(b.count);
    };

    for j in jumped.read() {
        if j.double {
            commands.spawn((
                LevelEntity,
                Particle { vel: Vec2::new(0.0, -20.0), gravity: 0.0, life: 0.4, max_life: 0.4, fade: true },
                Sprite::from_image(sprites.get(SpriteId::TootPuff)),
                FrameAnim { id: SpriteId::TootPuff, fps: 10.0, offset: 0.0 },
                Transform::from_translation((j.pos + Vec2::new(0.0, -10.0)).extend(6.0)),
            ));
            burst(&mut commands, &mut rng, j.pos + feet, Burst {
                count: 10,
                colors: &[Color::srgb(0.55, 0.8, 0.3), Color::srgb(0.75, 0.9, 0.4), Color::srgb(0.4, 0.65, 0.25)],
                speed: (20.0, 70.0),
                angle: (-PI * 0.9, -PI * 0.1),
                gravity: -60.0,
                life: (0.3, 0.6),
                size: 2.0,
            });
        } else {
            burst(&mut commands, &mut rng, j.pos + feet, Burst {
                count: 4,
                colors: &[Color::srgb(0.75, 0.68, 0.55)],
                speed: (15.0, 40.0),
                angle: (PI * 0.05, PI * 0.95),
                gravity: 100.0,
                life: (0.2, 0.35),
                size: 2.0,
            });
        }
    }
    for j in on_one.read() {
        // A ring of gold around the feet, and a few glints that rise with Nat.
        burst(&mut commands, &mut rng, j.pos + feet, Burst {
            count: 14,
            colors: &NOTE_COLORS,
            speed: (40.0, 90.0),
            angle: (0.0, 2.0 * PI),
            gravity: 0.0,
            life: (0.3, 0.55),
            size: 2.0,
        });
        burst(&mut commands, &mut rng, j.pos, Burst {
            count: 6,
            colors: &[Color::srgb(1.0, 1.0, 0.75)],
            speed: (60.0, 120.0),
            angle: (PI * 0.35, PI * 0.65),
            gravity: 60.0,
            life: (0.4, 0.7),
            size: 2.0,
        });
    }
    for l in landed.read() {
        let n = if l.speed > 300.0 { 10 } else { 6 };
        burst(&mut commands, &mut rng, l.pos + feet, Burst {
            count: n,
            colors: &[Color::srgb(0.75, 0.68, 0.55), Color::srgb(0.6, 0.55, 0.45)],
            speed: (20.0, 60.0),
            angle: (0.0, PI),
            gravity: 200.0,
            life: (0.2, 0.4),
            size: 2.0,
        });
    }
    for d in died.read() {
        burst(&mut commands, &mut rng, d.pos, Burst {
            count: 28,
            colors: &[Color::srgb(0.45, 0.28, 0.12), Color::srgb(0.6, 0.38, 0.17), Color::srgb(0.33, 0.2, 0.08)],
            speed: (60.0, 180.0),
            angle: (0.0, 2.0 * PI),
            gravity: 500.0,
            life: (0.4, 0.8),
            size: 2.0,
        });
    }
    for n in nuggets.read() {
        burst(&mut commands, &mut rng, n.pos, Burst {
            count: 10,
            colors: &[Color::srgb(1.0, 0.85, 0.2), Color::srgb(1.0, 1.0, 0.7), Color::srgb(0.95, 0.65, 0.1)],
            speed: (30.0, 80.0),
            angle: (0.0, 2.0 * PI),
            gravity: 0.0,
            life: (0.25, 0.5),
            size: 2.0,
        });
    }
    for c in checkpoints.read() {
        burst(&mut commands, &mut rng, c.pos + Vec2::new(0.0, 12.0), Burst {
            count: 24,
            colors: &[Color::WHITE, Color::srgb(0.95, 0.93, 0.85), Color::srgb(0.85, 0.85, 0.8)],
            speed: (60.0, 150.0),
            angle: (PI * 0.15, PI * 0.85),
            gravity: 250.0,
            life: (0.6, 1.1),
            size: 4.0,
        });
    }
}

/// Seconds between trail notes while airborne in Giant Steps.
const NOTE_EVERY: f32 = 0.07;
const NOTE_COLORS: [Color; 3] = [Color::srgb(1.0, 0.85, 0.2), Color::srgb(1.0, 0.95, 0.55), Color::srgb(0.95, 0.65, 0.1)];

/// Time until the next trail note.
#[derive(Resource, Default)]
struct NoteTrail(f32);

/// Giant Steps: Nat's jumps leave a trail of little golden eighth notes that drift up and fade.
fn note_trail(
    mut commands: Commands,
    time: Res<Time>,
    sprites: Res<Sprites>,
    groove: Option<Res<Groove>>,
    mut trail: ResMut<NoteTrail>,
    mut rng: ResMut<Rng>,
    player: Query<(&Transform, &Body), (With<Player>, Without<Dead>)>,
    existing: Query<(), With<Particle>>,
) {
    let airborne = player.single().ok().filter(|(_, b)| !b.on_ground);
    let (Some((tf, _)), true) = (airborne, groove.is_some_and(|g| g.giant_steps())) else {
        trail.0 = 0.0;
        return;
    };
    trail.0 -= time.delta_secs();
    if trail.0 > 0.0 || existing.iter().count() >= MAX_PARTICLES {
        return;
    }
    trail.0 = NOTE_EVERY;
    let at = tf.translation.truncate() + Vec2::new(rng.range(-5.0, 5.0), rng.range(-6.0, 2.0));
    let color = NOTE_COLORS[(rng.f() * 3.0) as usize % 3];
    let life = rng.range(0.5, 0.8);
    commands.spawn((
        LevelEntity,
        Particle { vel: Vec2::new(rng.range(-8.0, 8.0), rng.range(10.0, 25.0)), gravity: 0.0, life, max_life: life, fade: true },
        Sprite { image: sprites.get(SpriteId::Note), color, ..default() },
        Transform::from_translation(at.extend(6.0)),
    ));
}

fn update_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Particle, &mut Transform, &mut Sprite)>,
) {
    let dt = time.delta_secs();
    for (e, mut p, mut tf, mut sprite) in &mut q {
        p.life -= dt;
        if p.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        p.vel.y -= p.gravity * dt;
        tf.translation += (p.vel * dt).extend(0.0);
        if p.fade {
            let a = (p.life / p.max_life).clamp(0.0, 1.0);
            sprite.color.set_alpha(a);
        }
    }
}
