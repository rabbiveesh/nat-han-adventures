//! The Boom (its puffs, its fuel gauge), beans, and the sploosh diapers (their flaps, their
//! drop, their shadow, the SPLOOSH): sprites, puffs and sounds.

use bevy::{prelude::*, sprite::Anchor};

use super::{FrameAnim, Particle, sprite};
use crate::art::{SpriteId, Sprites};
use crate::audio::Sfx;
use crate::events::PlaySfx;
use crate::game::boom::{Bean, Boom, BoomEvent, BoomState};
use crate::game::diapers::{Diaper, DiaperDropped, DiaperState, Splooshed};
use crate::game::{ActiveLevel, LevelEntity};
use crate::level::{BOOM_FUEL, BOOM_LOW, TILE};

pub(super) fn plugin(app: &mut App) {
    app.add_observer(boom_sprite).add_observer(bean_sprite).add_observer(diaper_sprite).add_systems(
        PostUpdate,
        (boom_looks, diaper_looks, sky_fx).in_set(super::VisualSet::Animate).run_if(resource_exists::<Sprites>),
    );
}

/// The gauge under the Boom: this wide at a full tank (px).
const GAUGE_W: f32 = 12.0;

#[derive(Component)]
struct FuelGauge;

#[derive(Component)]
struct DiaperShadow(Entity);

fn boom_sprite(add: On<Add, Boom>, sprites: Option<Res<Sprites>>, mut commands: Commands) {
    let Some(sprites) = sprites else { return };
    commands
        .entity(add.entity)
        .insert((sprite(&sprites, SpriteId::Boom), FrameAnim { id: SpriteId::Boom, fps: 8.0, offset: 0.0 }))
        .with_children(|p| {
            p.spawn((
                FuelGauge,
                Sprite::from_color(Color::srgb(0.6, 0.85, 0.35), Vec2::new(GAUGE_W, 2.0)),
                Anchor::CENTER_LEFT,
                Transform::from_xyz(-GAUGE_W / 2.0, -8.0, 0.1),
                Visibility::Hidden,
            ));
        });
}

fn bean_sprite(add: On<Add, Bean>, sprites: Option<Res<Sprites>>, mut commands: Commands) {
    let Some(sprites) = sprites else { return };
    commands.entity(add.entity).insert(sprite(&sprites, SpriteId::Bean));
}

fn diaper_sprite(add: On<Add, Diaper>, sprites: Option<Res<Sprites>>, mut commands: Commands) {
    let Some(sprites) = sprites else { return };
    commands.entity(add.entity).insert(sprite(&sprites, SpriteId::Diaper));
    commands.spawn((
        LevelEntity,
        DiaperShadow(add.entity),
        Sprite::from_color(Color::srgba(0.1, 0.05, 0.15, 0.35), Vec2::new(10.0, 2.0)),
        Transform::from_xyz(0.0, -1000.0, 1.5),
    ));
}

/// The gauge shows the tank while it flies (red when it sputters), and the Boom tilts as it
/// goes.
fn boom_looks(
    time: Res<Time>,
    booms: Query<(&Boom, &Children, &mut Transform), Without<FuelGauge>>,
    mut gauges: Query<(&mut Sprite, &mut Transform, &mut Visibility), With<FuelGauge>>,
) {
    let t = time.elapsed_secs();
    for (boom, children, mut tf) in booms {
        let flying = boom.state == BoomState::Flying;
        let low = boom.fuel < BOOM_LOW;
        // Sputtering: it shudders.
        tf.rotation = Quat::from_rotation_z(if flying && low { (t * 40.0).sin() * 0.12 } else { 0.0 });
        for &c in children {
            let Ok((mut s, mut gtf, mut vis)) = gauges.get_mut(c) else { continue };
            *vis = if flying { Visibility::Inherited } else { Visibility::Hidden };
            let k = (boom.fuel / BOOM_FUEL).clamp(0.0, 1.0);
            gtf.scale.x = k.max(0.05);
            gtf.translation.x = -GAUGE_W / 2.0;
            let blink = low && (t * 8.0) as i32 % 2 == 0;
            s.color = if low { if blink { Color::srgb(1.0, 0.3, 0.3) } else { Color::srgb(0.7, 0.15, 0.15) } } else { Color::srgb(0.6, 0.85, 0.35) };
        }
    }
}

/// Diapers flap (and hold still, wings up, as they fall), vanish once splooshed; each one's
/// shadow sits on the first thing below it.
fn diaper_looks(
    time: Res<Time>,
    sprites: Res<Sprites>,
    active: Option<Res<ActiveLevel>>,
    mut diapers: Query<(&Diaper, &GlobalTransform, &mut Sprite, &mut Visibility), Without<DiaperShadow>>,
    mut shadows: Query<(&DiaperShadow, &mut Transform, &mut Visibility), Without<Diaper>>,
) {
    let t = time.elapsed_secs();
    for (d, _, mut s, mut vis) in &mut diapers {
        let frame = match d.state {
            DiaperState::Falling { .. } => 2,
            _ => ((t + d.phase) * 8.0) as usize % 2,
        };
        let img = sprites.frame(SpriteId::Diaper, frame);
        if s.image != img {
            s.image = img;
        }
        let want = if d.deadly() { Visibility::Inherited } else { Visibility::Hidden };
        vis.set_if_neq(want);
    }
    let Some(active) = active else { return };
    let level = &active.level;
    for (shadow, mut tf, mut vis) in &mut shadows {
        let Ok((d, gtf, _, _)) = diapers.get(shadow.0) else { continue };
        let p = gtf.translation().truncate();
        // The first floor below it.
        let (col, mut row) = level.cell_at(p);
        let mut floor = None;
        while row < level.height as i32 {
            row += 1;
            let tile = level.tile(col, row);
            if tile.is_solid() || tile.is_one_way() || tile == crate::level::Tile::Liquid {
                floor = Some(level.tile_center(col as usize, row as usize).y + TILE / 2.0);
                break;
            }
        }
        match floor.filter(|_| d.deadly()) {
            Some(y) => {
                tf.translation.x = p.x;
                tf.translation.y = y + 1.0;
                // Bigger and darker the closer it gets.
                let near = (1.0 - (p.y - y) / (8.0 * TILE)).clamp(0.3, 1.0);
                tf.scale.x = near;
                vis.set_if_neq(Visibility::Inherited);
            }
            None => {
                vis.set_if_neq(Visibility::Hidden);
            }
        }
    }
}

fn sky_fx(
    mut commands: Commands,
    sprites: Res<Sprites>,
    mut boom: MessageReader<BoomEvent>,
    mut splooshed: MessageReader<Splooshed>,
    mut dropped: MessageReader<DiaperDropped>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    let puff = |commands: &mut Commands, at: Vec2| {
        commands.spawn((
            LevelEntity,
            Particle { vel: Vec2::new(0.0, -20.0), gravity: 0.0, life: 0.4, max_life: 0.4, fade: true },
            Sprite::from_image(sprites.get(SpriteId::TootPuff)),
            FrameAnim { id: SpriteId::TootPuff, fps: 10.0, offset: 0.0 },
            Transform::from_translation(at.extend(6.0)),
        ));
    };
    for e in boom.read() {
        match *e {
            BoomEvent::Caught(at) => {
                sfx.write(PlaySfx(Sfx::Boing));
                puff(&mut commands, at);
            }
            BoomEvent::Bean(at) => {
                // The Boom toots it straight out the back.
                sfx.write(PlaySfx(Sfx::Toot));
                puff(&mut commands, at);
            }
            BoomEvent::Dry(at) => {
                sfx.write(PlaySfx(Sfx::Sputter));
                puff(&mut commands, at);
            }
            BoomEvent::HoppedOff(at) => puff(&mut commands, at),
        }
    }
    for _ in dropped.read() {
        sfx.write(PlaySfx(Sfx::Whistle));
    }
    for s in splooshed.read() {
        sfx.write(PlaySfx(Sfx::Sploosh));
        // A cartoon splash: pale blue droplets, up and out.
        for k in 0..10 {
            let a = 0.35 + k as f32 / 9.0 * (std::f32::consts::PI - 0.7);
            let speed = 70.0 + (k * 37 % 50) as f32;
            commands.spawn((
                LevelEntity,
                Particle { vel: Vec2::new(a.cos(), a.sin()) * speed, gravity: 400.0, life: 0.5, max_life: 0.5, fade: true },
                Sprite { color: Color::srgb(0.75, 0.9, 1.0), ..Sprite::from_image(sprites.get(SpriteId::Particle)) },
                Transform::from_translation(s.pos.extend(6.0)).with_scale(Vec3::splat(1.5)),
            ));
        }
    }
}
