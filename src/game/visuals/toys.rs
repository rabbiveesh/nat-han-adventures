//! The secret playground's toys on screen: the helper and his rope, the blast button (its
//! speakers pumping, notes flying while it blasts), stop signs, houses (the lit window, the
//! chimney smoke while the chef's in) and Nat the schnitzel; and their sounds.

use bevy::{prelude::*, sprite::Anchor};

use super::{Particle, sprite};
use crate::art::{SpriteId, Sprites};
use crate::audio::Sfx;
use crate::events::{DeathCause, PlayerDied, PlaySfx};
use crate::game::helper::{Helper, HelperState, RopeEvent};
use crate::game::houses::{CHIMNEY, House, HouseEvent};
use crate::game::toys::{Blasted, Blaster, Blasting, SignHit, StopSign};
use crate::game::{Dead, LevelEntity, Player, SimClock};

pub(super) fn plugin(app: &mut App) {
    app.add_observer(helper_sprite)
        .add_observer(blaster_sprite)
        .add_observer(sign_sprite)
        .add_observer(house_sprite)
        .add_systems(Update, unfry)
        .add_systems(
            PostUpdate,
            (helper_looks, blaster_looks, house_looks, toy_fx).in_set(super::VisualSet::Animate).run_if(resource_exists::<Sprites>),
        );
}

/// On Nat while he's a schnitzel (his splat's hidden: the schnitzel flies off instead).
#[derive(Component, Debug)]
pub struct Fried;

/// The helper's rope (its top at his hand).
#[derive(Component)]
struct Rope(Entity);

const ROPE: Color = Color::srgb(0.78, 0.62, 0.36);

fn helper_sprite(add: On<Add, Helper>, sprites: Option<Res<Sprites>>, mut commands: Commands) {
    let Some(sprites) = sprites else { return };
    commands.entity(add.entity).insert(sprite(&sprites, SpriteId::Helper));
    commands.spawn((LevelEntity, Rope(add.entity), Sprite::from_color(ROPE, Vec2::new(2.0, 1.0)), Anchor::TOP_CENTER, Transform::from_xyz(0.0, 0.0, 2.9)));
}

fn blaster_sprite(add: On<Add, Blaster>, sprites: Option<Res<Sprites>>, mut commands: Commands) {
    let Some(sprites) = sprites else { return };
    commands.entity(add.entity).insert((sprite(&sprites, SpriteId::Blaster), Anchor::BOTTOM_CENTER));
}

fn sign_sprite(add: On<Add, StopSign>, sprites: Option<Res<Sprites>>, mut commands: Commands) {
    let Some(sprites) = sprites else { return };
    commands.entity(add.entity).insert((sprite(&sprites, SpriteId::StopSign), Anchor::BOTTOM_CENTER));
}

/// The house's picture sits behind its door's floor point: its door's middle over it (the
/// art's door is a little right of the picture's middle, its floor 6 px up).
fn house_sprite(add: On<Add, House>, sprites: Option<Res<Sprites>>, mut commands: Commands) {
    let Some(sprites) = sprites else { return };
    commands.entity(add.entity).with_children(|p| {
        p.spawn((sprite(&sprites, SpriteId::House), Anchor::BOTTOM_CENTER, Transform::from_xyz(-3.0, -6.0, 0.0), HousePicture));
    });
}

#[derive(Component)]
struct HousePicture;

/// The helper calls (waving) while he waits and snickers before he lets go; his rope hangs
/// from his hand to its end.
fn helper_looks(
    time: Res<Time>,
    sprites: Res<Sprites>,
    mut helpers: Query<(Entity, &Helper, &mut Sprite), Without<Rope>>,
    mut ropes: Query<(&Rope, &mut Sprite, &mut Transform), Without<Helper>>,
) {
    let t = time.elapsed_secs();
    for (_, h, mut s) in &mut helpers {
        let frame = match h.state {
            HelperState::Snicker { .. } => 2,
            HelperState::Waiting if h.since_call < 1.2 => ((t * 4.0) as usize % 2) * 1,
            _ => 0,
        };
        let img = sprites.frame(SpriteId::Helper, frame);
        if s.image != img {
            s.image = img;
        }
        s.flip_x = h.side < 0.0;
    }
    for (rope, mut s, mut tf) in &mut ropes {
        let Ok((_, h, _)) = helpers.get(rope.0) else { continue };
        s.custom_size = Some(Vec2::new(2.0, h.len));
        tf.translation = h.pivot.extend(2.9);
        tf.rotation = Quat::from_rotation_z(h.angle);
    }
}

/// Pressed, it's down; while the band blasts, the speakers pump on every beat-ish.
fn blaster_looks(time: Res<Time>, sprites: Res<Sprites>, blasting: Res<Blasting>, mut buttons: Query<(&Blaster, &mut Sprite)>) {
    let t = time.elapsed_secs();
    for (b, mut s) in &mut buttons {
        let pump = blasting.0 > 0.0 && (t * 4.0) as i32 % 2 == 0;
        let img = sprites.frame(SpriteId::Blaster, usize::from(b.pressed || pump));
        if s.image != img {
            s.image = img;
        }
    }
}

/// The window's lit while the chef's in; smoke puffs out of the chimney.
fn house_looks(
    sprites: Res<Sprites>,
    clock: Option<Res<SimClock>>,
    time: Res<Time>,
    houses: Query<(&House, &Transform, &Children)>,
    mut pictures: Query<&mut Sprite, With<HousePicture>>,
    mut commands: Commands,
    mut smoke_at: Local<f32>,
) {
    let now = clock.map_or(0.0, |c| c.time);
    *smoke_at -= time.delta_secs();
    let puff = *smoke_at <= 0.0;
    if puff {
        *smoke_at = 0.25;
    }
    for (h, tf, children) in &houses {
        let home = h.chef_home(now);
        for &c in children {
            if let Ok(mut s) = pictures.get_mut(c) {
                let img = sprites.frame(SpriteId::House, usize::from(home));
                if s.image != img {
                    s.image = img;
                }
            }
        }
        if home && puff {
            let top = tf.translation.truncate() + CHIMNEY;
            let drift = (now * 1.7).sin() * 6.0;
            commands.spawn((
                LevelEntity,
                Particle { vel: Vec2::new(drift, 18.0), gravity: -4.0, life: 1.4, max_life: 1.4, fade: true },
                Sprite { color: Color::srgba(0.85, 0.85, 0.9, 0.8), ..Sprite::from_image(sprites.get(SpriteId::Wisp)) },
                Transform::from_translation(top.extend(-0.4)).with_scale(Vec3::splat(2.0)),
            ));
        }
    }
}

/// Sounds, puffs, notes and the schnitzel.
#[allow(clippy::too_many_arguments)]
fn toy_fx(
    mut commands: Commands,
    sprites: Res<Sprites>,
    time: Res<Time>,
    blasting: Res<Blasting>,
    buttons: Query<&Transform, With<Blaster>>,
    houses: Query<&Transform, With<House>>,
    player: Query<Entity, With<Player>>,
    mut rope: MessageReader<RopeEvent>,
    mut house: MessageReader<HouseEvent>,
    mut blasted: MessageReader<Blasted>,
    mut sign: MessageReader<SignHit>,
    mut died: MessageReader<PlayerDied>,
    mut sfx: MessageWriter<PlaySfx>,
    mut notes_at: Local<f32>,
) {
    let note = |commands: &mut Commands, at: Vec2, vel: Vec2, color: Color| {
        commands.spawn((
            LevelEntity,
            Particle { vel, gravity: -20.0, life: 1.0, max_life: 1.0, fade: true },
            Sprite { color, ..Sprite::from_image(sprites.get(SpriteId::Note)) },
            Transform::from_translation(at.extend(6.0)),
        ));
    };
    for e in rope.read() {
        match *e {
            RopeEvent::Grabbed(_) => sfx.write(PlaySfx(Sfx::Boing)),
            RopeEvent::Snicker(_) => sfx.write(PlaySfx(Sfx::Snicker)),
            RopeEvent::LetGo(_) => sfx.write(PlaySfx(Sfx::Whistle)),
            RopeEvent::Hoisted(_) => sfx.write(PlaySfx(Sfx::Checkpoint)),
            RopeEvent::JumpedOff(_) => continue,
        };
    }
    for e in house.read() {
        match *e {
            HouseEvent::In(_) => sfx.write(PlaySfx(Sfx::MenuMove)),
            HouseEvent::Sizzle(_) => sfx.write(PlaySfx(Sfx::Sizzle)),
            HouseEvent::Chimney(at) => {
                for k in 0..6 {
                    let a = k as f32 / 6.0 * std::f32::consts::TAU;
                    commands.spawn((
                        LevelEntity,
                        Particle { vel: Vec2::new(a.cos(), a.sin()) * 30.0, gravity: 0.0, life: 0.5, max_life: 0.5, fade: true },
                        Sprite { color: Color::srgba(0.3, 0.3, 0.32, 0.9), ..Sprite::from_image(sprites.get(SpriteId::Wisp)) },
                        Transform::from_translation(at.extend(6.0)).with_scale(Vec3::splat(2.0)),
                    ));
                }
                sfx.write(PlaySfx(Sfx::Foomp))
            }
        };
    }
    for b in blasted.read() {
        sfx.write(PlaySfx(Sfx::Blast));
        // A ring of notes out of the speakers.
        for k in 0..12 {
            let a = 0.2 + k as f32 / 11.0 * (std::f32::consts::PI - 0.4);
            note(&mut commands, b.pos + Vec2::Y * 8.0, Vec2::new(a.cos(), a.sin()) * 120.0, Color::srgb(1.0, 0.85, 0.3));
        }
    }
    // While it blasts, notes keep pouring out of the speakers.
    *notes_at -= time.delta_secs();
    if blasting.0 > 0.0 && *notes_at <= 0.0 {
        *notes_at = 0.15;
        let t = time.elapsed_secs();
        for tf in &buttons {
            for side in [-1.0f32, 1.0] {
                let at = tf.translation.truncate() + Vec2::new(side * 19.0, 10.0);
                let up = 50.0 + (t * 13.0).sin().abs() * 30.0;
                let color = if side < 0.0 { Color::srgb(0.6, 0.9, 1.0) } else { Color::srgb(1.0, 0.6, 0.85) };
                note(&mut commands, at, Vec2::new(side * 40.0, up), color);
            }
        }
    }
    for _ in sign.read() {
        sfx.write(PlaySfx(Sfx::Screech));
    }
    for d in died.read() {
        if d.cause != DeathCause::Schnitzel {
            continue;
        }
        if let Ok(e) = player.single() {
            commands.entity(e).insert(Fried);
        }
        // Out of the nearest door he flies, breaded, spinning.
        let door = houses.iter().map(|t| t.translation.truncate()).min_by(|a, b| a.distance(d.pos).total_cmp(&b.distance(d.pos))).unwrap_or(d.pos);
        commands.spawn((
            LevelEntity,
            Particle { vel: Vec2::new(70.0, 260.0), gravity: 600.0, life: 1.2, max_life: 1.2, fade: false },
            Spin(9.0),
            Sprite::from_image(sprites.get(SpriteId::Schnitzel)),
            Transform::from_translation((door + Vec2::Y * 10.0).extend(6.0)),
        ));
    }
}

/// A particle that spins as it goes (radians/s).
#[derive(Component)]
struct Spin(f32);

/// The schnitzel spins; and once Nat's back (respawned), he's not fried any more.
fn unfry(
    mut commands: Commands,
    time: Res<Time>,
    nat: Query<Entity, (With<Player>, With<Fried>, Without<Dead>)>,
    mut spins: Query<(&Spin, &mut Transform)>,
) {
    for e in &nat {
        commands.entity(e).remove::<Fried>();
    }
    for (s, mut tf) in &mut spins {
        tf.rotate_z(s.0 * time.delta_secs());
    }
}
