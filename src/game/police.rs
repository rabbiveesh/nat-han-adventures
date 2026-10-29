//! The policeman (`Q`) and his spikes (`y`).
//!
//! He walks his beat. With Nat near ([`POLICE_REACH`]) he stops, blows his whistle and gives
//! orders, round and round from when Nat came near ([`POLICE_ORDERS`]): "STOP!" and "THIS
//! WAY!" with an arrow (mostly on, now and then back). Each new order gives Nat
//! [`POLICE_GRACE`] to do it. Listen to the police and nothing hurts you: his spikes fold
//! down while he says go (and never come up under someone standing on them). Don't listen
//! (move when he says stop, or go the other way) and a spike pops up right where Nat was about
//! to step; in the air (a jump, the Boom), a traffic cone drops on his head, following him.
//! So you have to listen to the police.

use bevy::prelude::*;

use super::boom::Aboard;
use super::hazards::Doomed;
use super::physics::{Body, Dead, Finished, tile_at};
use super::{ActiveLevel, GameSet, LevelEntity, Player, Pos, cell_floor};
use crate::events::{DeathCause, ToyShout};
use crate::level::{Level, Order, POLICE_GRACE, POLICE_ORDERS, POLICE_REACH, POLICE_ROWS, TILE};

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Police>()
        .register_type::<PoliceSpike>()
        .add_message::<PoliceEvent>()
        .add_systems(FixedUpdate, patrol.in_set(GameSet::World))
        .add_systems(
            FixedUpdate,
            (obey, (fold_spikes, pop_spikes, drop_cones), police_harm).chain().in_set(GameSet::Interact).before(super::hazards::check_hazards),
        );
}

/// He walks this far either side of his post (px)...
const BEAT: f32 = 2.0 * TILE;
/// ...this fast (px/s).
const WALK: f32 = 24.0;
/// He goes off duty once Nat's this far (tiles across).
const OFF_DUTY: f32 = POLICE_REACH + 4.0;
/// Moving faster than this (px/s) is moving.
const MOVING: f32 = 30.0;
/// A popped spike: this far in front of Nat (px past his front), up this fast (s), there this long (s).
const POP_AHEAD: f32 = 6.0;
const POP_RISE: f32 = 0.05;
const POP_LIFE: f32 = 1.4;
/// A cone: dropped from this high over Nat's head (px), falling under this gravity (px/s²),
/// steering over him at up to this speed (px/s); it sits where it lands this long (s).
const CONE_HIGH: f32 = 72.0;
const CONE_GRAVITY: f32 = 1400.0;
const CONE_STEER: f32 = 260.0;
const CONE_REST: f32 = 1.0;
/// A police spike folds or comes up in this long (s); it's deadly more than half up.
const FOLD_SECS: f32 = 0.12;
/// The spikes' box (half extents, px; the bottom half of the tile, like `^`).
const SPIKE_HALF: Vec2 = Vec2::new(7.0, 4.0);
const CONE_HALF: Vec2 = Vec2::new(6.0, 7.0);

/// A policeman. His transform is his feet.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Police {
    /// His post (his cell's floor).
    pub post: Vec2,
    /// Walking this way (+1 right, -1 left), off duty.
    pub walk: f32,
    /// Giving orders: how long (s) since Nat came near.
    pub duty: Option<f32>,
    /// Since the order now given (s).
    pub since_order: f32,
    /// Nat's not listened to this order (he's been dealt with).
    pub dealt: bool,
    /// Facing (+1 right, -1 left).
    pub facing: f32,
}

impl Police {
    /// The order he's giving now (`None`: off duty).
    pub fn order(&self) -> Option<Order> {
        self.duty.map(|t| order_at(t).0)
    }
}

/// The order `t` s into a policeman's duty, and which in [`POLICE_ORDERS`] it is.
pub fn order_at(t: f32) -> (Order, usize) {
    let round: f32 = POLICE_ORDERS.iter().map(|o| o.1).sum();
    let mut t = t.rem_euclid(round);
    for (i, &(o, len)) in POLICE_ORDERS.iter().enumerate() {
        if t < len {
            return (o, i);
        }
        t -= len;
    }
    (POLICE_ORDERS[0].0, 0)
}

/// A police spike: `up` 0 (folded) .. 1 (up). Its transform is its cell's floor.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct PoliceSpike {
    pub up: f32,
}

/// A spike popped up in front of someone who didn't listen (`t` s ago). Its transform is its foot.
#[derive(Component, Debug, Clone, Copy)]
pub struct PoppedSpike {
    pub t: f32,
}

/// A traffic cone dropped on someone who didn't listen. `rest`: it's landed, this long ago.
#[derive(Component, Debug, Clone, Copy)]
pub struct Cone {
    pub vel: Vec2,
    pub rest: Option<f32>,
}

/// What a policeman did (for sounds).
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub enum PoliceEvent {
    /// A new order, blown on his whistle (`pos`: him).
    Order(Vec2),
    /// A spike popped up (`pos`: its foot).
    Pop(Vec2),
    /// A cone's dropped.
    Cone(Vec2),
    /// A cone came down on Nat's head.
    Bonk(Vec2),
}

fn nat_near(post: Vec2, nat: Vec2, across: f32) -> bool {
    (nat.x - post.x).abs() < across * TILE && (nat.y - post.y).abs() < POLICE_ROWS * TILE
}

/// On his beat, or on duty: walking, or giving orders.
fn patrol(
    time: Res<Time>,
    nat: Query<&Pos, (With<Player>, Without<Dead>, Without<Finished>, Without<Police>)>,
    mut police: Query<(&mut Police, &mut Transform)>,
    mut events: MessageWriter<PoliceEvent>,
    mut shout: MessageWriter<ToyShout>,
) {
    let dt = time.delta_secs();
    let nat = nat.single().ok().map(|p| p.0);
    for (mut p, mut tf) in &mut police {
        let at = tf.translation.truncate();
        let near = nat.is_some_and(|n| nat_near(p.post, n, if p.duty.is_some() { OFF_DUTY } else { POLICE_REACH }));
        let Some(n) = nat.filter(|_| near) else {
            p.duty = None;
            // Walk the beat.
            let mut x = at.x + p.walk * WALK * dt;
            if (x - p.post.x).abs() > BEAT {
                p.walk = -p.walk;
                x = p.post.x + BEAT * (x - p.post.x).signum();
            }
            p.facing = p.walk;
            tf.translation.x = x;
            continue;
        };
        let before = p.duty.map(|t| order_at(t).1);
        let t = p.duty.map_or(0.0, |t| t + dt);
        p.duty = Some(t);
        p.facing = if n.x < at.x { -1.0 } else { 1.0 };
        let (order, i) = order_at(t);
        if before != Some(i) {
            p.since_order = 0.0;
            p.dealt = false;
            events.write(PoliceEvent::Order(at));
            let text = match order {
                Order::Stop => "STOP!",
                Order::Go(d) if d > 0 => "THIS WAY! →",
                Order::Go(_) => "← THIS WAY!",
            };
            shout.write(ToyShout { pos: at + Vec2::Y * 46.0, text });
        } else {
            p.since_order += dt;
        }
    }
}

/// Did Nat listen? If not: a spike pops up where he's headed, or (in the air) a cone drops.
#[allow(clippy::type_complexity)]
fn obey(
    mut commands: Commands,
    nat: Query<(&Pos, &Body, Has<Aboard>), (With<Player>, Without<Dead>, Without<Finished>)>,
    mut police: Query<(&mut Police, &Transform)>,
    mut events: MessageWriter<PoliceEvent>,
    mut shout: MessageWriter<ToyShout>,
) {
    let Ok((pos, body, aboard)) = nat.single() else { return };
    for (mut p, tf) in &mut police {
        let Some(order) = p.order() else { continue };
        if p.dealt || p.since_order < POLICE_GRACE {
            continue;
        }
        let vx = body.vel.x + body.launch;
        let flying = aboard || !body.on_ground;
        let wrong = match order {
            Order::Stop => flying || vx.abs() > MOVING,
            Order::Go(d) => vx * f32::from(d) < -MOVING,
        };
        if !wrong {
            continue;
        }
        p.dealt = true;
        shout.write(ToyShout { pos: tf.translation.truncate() + Vec2::Y * 46.0, text: "HEY!" });
        if flying {
            let at = pos.0 + Vec2::Y * (body.half.y + CONE_HIGH);
            commands.spawn((Name::new("Cone"), LevelEntity, Cone { vel: Vec2::ZERO, rest: None }, Transform::from_translation(at.extend(5.5))));
            events.write(PoliceEvent::Cone(at));
        } else {
            let ahead = vx.signum() * (body.half.x + POP_AHEAD + SPIKE_HALF.x);
            let foot = Vec2::new(pos.0.x + ahead, pos.0.y - body.half.y);
            commands.spawn((Name::new("PoppedSpike"), LevelEntity, PoppedSpike { t: 0.0 }, Transform::from_translation(foot.extend(2.0))));
            events.write(PoliceEvent::Pop(foot));
        }
    }
}

/// Police spikes fold while the nearest policeman says go, and come up again after (never
/// under someone standing on them).
fn fold_spikes(
    time: Res<Time>,
    nat: Query<(&Pos, &Body), (With<Player>, Without<Dead>)>,
    police: Query<(&Police, &Transform)>,
    mut spikes: Query<(&mut PoliceSpike, &Transform), Without<Police>>,
) {
    let step = time.delta_secs() / FOLD_SECS;
    let nat = nat.single().ok();
    for (mut s, tf) in &mut spikes {
        let at = tf.translation.truncate();
        let cop = police.iter().min_by(|a, b| a.1.translation.truncate().distance(at).total_cmp(&b.1.translation.truncate().distance(at)));
        let fold = cop.is_some_and(|(p, _)| matches!(p.order(), Some(Order::Go(_))));
        let over = nat.is_some_and(|(p, b)| overlap(p.0, b.half, at + Vec2::Y * TILE / 2.0, Vec2::new(TILE / 2.0, TILE / 2.0)));
        if fold {
            s.up = (s.up - step).max(0.0);
        } else if !over || s.up > 0.0 {
            s.up = (s.up + step).min(1.0);
        }
    }
}

/// Popped spikes go back down after a while.
fn pop_spikes(mut commands: Commands, time: Res<Time>, mut popped: Query<(Entity, &mut PoppedSpike)>) {
    for (e, mut s) in &mut popped {
        s.t += time.delta_secs();
        if s.t > POP_LIFE {
            commands.entity(e).despawn();
        }
    }
}

/// Cones fall, steering over Nat's head, and sit a moment where they land.
fn drop_cones(
    mut commands: Commands,
    time: Res<Time>,
    active: Res<ActiveLevel>,
    nat: Query<&Pos, (With<Player>, Without<Dead>)>,
    mut cones: Query<(Entity, &mut Cone, &mut Transform)>,
) {
    let dt = time.delta_secs();
    let nat = nat.single().ok().map(|p| p.0);
    for (e, mut c, mut tf) in &mut cones {
        if let Some(r) = c.rest.as_mut() {
            *r += dt;
            if *r > CONE_REST {
                commands.entity(e).despawn();
            }
            continue;
        }
        let at = tf.translation.truncate();
        c.vel.y -= CONE_GRAVITY * dt;
        c.vel.x = nat.map_or(0.0, |n| ((n.x - at.x) / dt).clamp(-CONE_STEER, CONE_STEER));
        let mut next = at + c.vel * dt;
        let bottom = next.y - CONE_HALF.y;
        let (i, j) = ((next.x / TILE).floor() as i32, (bottom / TILE).floor() as i32);
        // (Over the top of the grid it's falling through the sky, not the ceiling.)
        let below = if j >= active.level.height as i32 { crate::level::Tile::Empty } else { tile_at(&active.level, i, j) };
        if below.is_solid() || below == crate::level::Tile::OneWay || bottom < 0.0 {
            next.y = (j + 1) as f32 * TILE + CONE_HALF.y;
            c.rest = Some(0.0);
        }
        tf.translation.x = next.x;
        tf.translation.y = next.y;
    }
}

fn overlap(a: Vec2, ah: Vec2, b: Vec2, bh: Vec2) -> bool {
    (a.x - b.x).abs() < ah.x + bh.x && (a.y - b.y).abs() < ah.y + bh.y
}

/// Up spikes, popped spikes and falling cones get Nat.
#[allow(clippy::type_complexity)]
fn police_harm(
    mut commands: Commands,
    nat: Query<(Entity, &Pos, &Body), (With<Player>, Without<Dead>, Without<Finished>)>,
    spikes: Query<(&PoliceSpike, &Transform)>,
    popped: Query<(&PoppedSpike, &Transform)>,
    cones: Query<(&Cone, &Transform)>,
    mut events: MessageWriter<PoliceEvent>,
) {
    let Ok((e, pos, body)) = nat.single() else { return };
    // A little forgiving, like the tiles.
    let half = body.half - Vec2::splat(2.0);
    let spike = |foot: Vec2| foot + Vec2::Y * SPIKE_HALF.y;
    let hit = spikes.iter().any(|(s, tf)| s.up > 0.5 && overlap(pos.0, half, spike(tf.translation.truncate()), SPIKE_HALF))
        || popped.iter().any(|(s, tf)| s.t > POP_RISE && overlap(pos.0, half, spike(tf.translation.truncate()), SPIKE_HALF));
    let bonk = cones.iter().any(|(c, tf)| c.rest.is_none() && overlap(pos.0, half, tf.translation.truncate(), CONE_HALF));
    if bonk {
        events.write(PoliceEvent::Bonk(pos.0));
    }
    if hit || bonk {
        commands.entity(e).insert(Doomed(DeathCause::Police));
    }
}

pub(super) fn spawn_police(commands: &mut Commands, level: &Level, col: usize, row: usize) {
    let post = cell_floor(level, col, row);
    commands.spawn((
        Name::new("Police"),
        LevelEntity,
        Police { post, walk: 1.0, duty: None, since_order: 0.0, dealt: false, facing: 1.0 },
        Transform::from_translation(post.extend(3.0)),
    ));
}

pub(super) fn spawn_spike(commands: &mut Commands, level: &Level, col: usize, row: usize) {
    commands.spawn((Name::new("PoliceSpike"), LevelEntity, PoliceSpike { up: 1.0 }, Transform::from_translation(cell_floor(level, col, row).extend(1.9))));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_orders_go_round() {
        assert_eq!(order_at(0.0), (Order::Stop, 0));
        assert_eq!(order_at(1.7).0, Order::Go(1));
        assert_eq!(order_at(4.1).0, Order::Stop);
        assert_eq!(order_at(5.7).0, Order::Go(-1));
        assert_eq!(order_at(6.7).0, Order::Stop);
    }
}
