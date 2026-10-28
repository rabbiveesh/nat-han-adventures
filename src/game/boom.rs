//! The Boom (`B`): a round little flying pal who runs on beans. Parked, it bobs in its cell;
//! touch it and it catches Nat and flies wherever you steer (Left/Right/Up/Down; Down doesn't
//! call the band while you're aboard), until you hop off (Space/Z: Up, a jump key too, steers) or its tank runs dry. Nat
//! rides on top: the pair is one box ([`ride_box`]) against the tiles, and anything deadly the
//! Boom touches splats Nat (`hazards::check_hazards` counts the Boom's box as his).
//!
//! Its tank holds [`BOOM_FUEL`] s of flying. Beans (`:`) flown through top it up
//! ([`BEAN_FUEL`]); with [`BOOM_LOW`] s left it sputters, slower ([`BOOM_SPUTTER`]), and dry,
//! it drops Nat (with his toot) and drifts home. Hopping off, it flies home too; it can be
//! caught again once it's parked, and eaten beans come back each time it parks. A death
//! aboard sends it straight home. Fired up, it flies as much faster as Nat runs.

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::crushers::{BrickSmashed, CrusherBlock, smash};
use super::physics::{Body, Dead, Fall, Finished, PlayerControl, TileMood, step_body};
use super::{ActiveLevel, GameSet, Groove, LevelEntity, MovingPlatform, Player, Pos, PrevPos, tuning};
use crate::input::Action;
use crate::level::{BEAN_FUEL, BOOM_FUEL, BOOM_LOW, BOOM_SPEED, BOOM_SPUTTER};

pub(super) fn plugin(app: &mut App) {
    app.add_message::<BoomEvent>()
        .add_systems(FixedUpdate, (catch_nat, fly_boom, eat_beans).chain().in_set(GameSet::Player).after(super::physics::player_step))
        .add_systems(FixedUpdate, (drift_home, crash_aboard).in_set(GameSet::Interact).after(super::hazards::check_hazards));
}

/// The Boom's box (half extents, px): Nat sits on top of it.
pub const BOOM_HALF: Vec2 = Vec2::new(7.0, 5.0);
/// How fast it gets up to speed, and stops when you let go (px/s²).
pub const BOOM_ACCEL: f32 = 700.0;
/// How fast it flies home, empty (px/s).
pub const BOOM_HOME_SPEED: f32 = 90.0;
/// Dry, it sputters down this long (s) before it heads home.
pub const BOOM_DRY_FALL: f32 = 0.8;
/// Bean pickup box (px, square).
pub const BEAN_BOX: f32 = 10.0;

/// The Boom. `Pos` is its box's center.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Boom {
    /// Where it parks (its cell's center).
    pub home: Vec2,
    pub state: BoomState,
    /// Seconds of flying left in the tank.
    pub fuel: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub enum BoomState {
    /// Bobbing at home, waiting for Nat.
    Parked,
    /// Nat's aboard.
    Flying,
    /// Empty: sputtering down for `t` s (dry), then flying home.
    Home { dry: f32 },
}

/// On Nat while he rides the Boom `boom`: his flying velocity (px/s).
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Aboard {
    pub boom: Entity,
    pub vel: Vec2,
}

/// Beans (`:`): eaten while the Boom's out, back when it parks.
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct Bean {
    pub eaten: bool,
}

/// What the Boom did (for sounds and puffs).
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub enum BoomEvent {
    Caught(Vec2),
    Bean(Vec2),
    /// Ran dry and dropped Nat.
    Dry(Vec2),
    HoppedOff(Vec2),
}

/// The box of Nat on the Boom, for Nat's box center `nat`: (center, half extents).
pub fn ride_box(nat: Vec2) -> (Vec2, Vec2) {
    let nat_half = Vec2::new(tuning::PLAYER_SIZE.0, tuning::PLAYER_SIZE.1) / 2.0;
    let top = nat.y + nat_half.y;
    let bottom = nat.y - nat_half.y - 2.0 * BOOM_HALF.y;
    let half = Vec2::new(nat_half.x.max(BOOM_HALF.x), (top - bottom) / 2.0);
    (Vec2::new(nat.x, (top + bottom) / 2.0), half)
}

/// Where the Boom is under Nat at `nat`.
pub fn under(nat: Vec2) -> Vec2 {
    Vec2::new(nat.x, nat.y - tuning::PLAYER_SIZE.1 / 2.0 - BOOM_HALF.y)
}

/// The Boom's speed now: slower sputtering, faster fired up.
pub fn boom_speed(fuel: f32, run_scale: f32) -> f32 {
    BOOM_SPEED * run_scale * if fuel < BOOM_LOW { BOOM_SPUTTER } else { 1.0 }
}

fn overlap(a: Vec2, ah: Vec2, b: Vec2, bh: Vec2) -> bool {
    (a.x - b.x).abs() < ah.x + bh.x && (a.y - b.y).abs() < ah.y + bh.y
}

type Free = (With<Player>, Without<Dead>, Without<Finished>, Without<Aboard>);

/// Touching a parked Boom: it catches Nat, full tank.
fn catch_nat(
    mut commands: Commands,
    mut nat: Query<(Entity, &mut Pos, &mut PrevPos, &mut Body), Free>,
    mut booms: Query<(Entity, &mut Boom, &Pos), Without<Player>>,
    mut events: MessageWriter<BoomEvent>,
) {
    let Ok((e, mut pos, mut prev, mut body)) = nat.single_mut() else { return };
    for (boom_e, mut boom, bp) in &mut booms {
        if boom.state != BoomState::Parked || !overlap(pos.0, body.half, bp.0, BOOM_HALF) {
            continue;
        }
        boom.state = BoomState::Flying;
        boom.fuel = BOOM_FUEL;
        // Nat sits on top.
        let at = Vec2::new(bp.0.x, bp.0.y + BOOM_HALF.y + body.half.y);
        pos.0 = at;
        prev.0 = at;
        body.launch = 0.0;
        body.pad = 0.0;
        body.riding = None;
        commands.entity(e).insert(Aboard { boom: boom_e, vel: Vec2::new(body.vel.x, 0.0) });
        events.write(BoomEvent::Caught(bp.0));
        return;
    }
}

/// Steering the Boom with Nat aboard: the pair moves as one box; Jump hops off, a dry tank
/// drops him.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn fly_boom(
    mut commands: Commands,
    time: Res<Time>,
    mut active: ResMut<ActiveLevel>,
    groove: Res<Groove>,
    input: Single<&ActionState<Action>>,
    mut nat: Query<(Entity, &mut Pos, &mut Body, &mut PlayerControl, &mut Aboard), (With<Player>, Without<Dead>, Without<Finished>)>,
    mut booms: Query<(&mut Boom, &mut Pos), (Without<Player>, Without<MovingPlatform>, Without<CrusherBlock>)>,
    crushers: Query<(&CrusherBlock, &Pos), (Without<Player>, Without<Boom>)>,
    mut smashed: MessageWriter<BrickSmashed>,
    mut events: MessageWriter<BoomEvent>,
    mut jumped: MessageWriter<crate::events::Jumped>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let Ok((e, mut pos, mut body, mut ctl, mut aboard)) = nat.single_mut() else { return };
    let Ok((mut boom, mut bp)) = booms.get_mut(aboard.boom) else {
        commands.entity(e).remove::<Aboard>();
        return;
    };
    // (Up is a jump key too: Up steers, only a jump without it hops off.)
    let hop = input.just_pressed(&Action::Jump) && !input.pressed(&Action::Up);
    boom.fuel -= dt;
    if hop || boom.fuel <= 0.0 {
        boom.state = BoomState::Home { dry: if hop { 0.0 } else { BOOM_DRY_FALL } };
        commands.entity(e).remove::<Aboard>();
        *ctl = PlayerControl { facing: ctl.facing, ..PlayerControl::default() };
        body.on_ground = false;
        body.vel = aboard.vel;
        if hop {
            body.vel.y = tuning::JUMP_SPEED * groove.jump_scale();
            ctl.cut_armed = true;
            jumped.write(crate::events::Jumped { pos: pos.0, double: false });
            events.write(BoomEvent::HoppedOff(bp.0));
        } else {
            body.vel.y = body.vel.y.min(0.0);
            events.write(BoomEvent::Dry(bp.0));
        }
        return;
    }

    let axis = |plus: Action, minus: Action| input.pressed(&plus) as i32 as f32 - input.pressed(&minus) as i32 as f32;
    let steer = Vec2::new(axis(Action::Right, Action::Left), axis(Action::Up, Action::Down));
    if steer.x != 0.0 {
        ctl.facing = steer.x;
    }
    let target = steer.normalize_or_zero() * boom_speed(boom.fuel, groove.run_scale());
    let v = aboard.vel;
    let step = BOOM_ACCEL * dt;
    aboard.vel = if (target - v).length() <= step { target } else { v + (target - v).normalize() * step };

    // The pair as one box, weightless: the tiles stop it, a wall fan's gust pushes it, bricks
    // give way to its head.
    let (center, half) = ride_box(pos.0);
    let mut c = center;
    let mut pair = Body { vel: aboard.vel, half, on_ground: false, ..Body::player() };
    let fall = Fall { gravity: 0.0, max_fall: 10_000.0, tiles: TileMood::of(&groove) };
    let solids: Vec<(Vec2, Vec2)> = crushers.iter().map(|(b, p)| b.bounds(p.0)).collect();
    let contact = step_body(&active.level, &mut c, &mut pair, fall, &[], &[], &solids, dt);
    if contact.bricks.iter().any(Option::is_some) {
        smash(&mut active, contact.smashed(), &mut smashed);
    }
    // Stopped by what it bumped into.
    if contact.blocked_x {
        aboard.vel.x = 0.0;
    }
    if pair.vel.y == 0.0 {
        aboard.vel.y = 0.0;
    }
    pos.0 += c - center;
    bp.0 = under(pos.0);
    // Nat sits: drawn standing still, his box out of the physics' way.
    body.vel = Vec2::ZERO;
    body.on_ground = true;
    body.riding = None;
    body.launch = 0.0;
    body.pad = 0.0;
    ctl.has_toot = true;
}

/// Beans flown through on the Boom top up its tank.
fn eat_beans(
    nat: Query<(&Pos, &Aboard), With<Player>>,
    mut booms: Query<&mut Boom>,
    mut beans: Query<(&mut Bean, &mut Visibility, &Transform)>,
    mut events: MessageWriter<BoomEvent>,
) {
    let Ok((pos, aboard)) = nat.single() else { return };
    let Ok(mut boom) = booms.get_mut(aboard.boom) else { return };
    let (c, h) = ride_box(pos.0);
    for (mut bean, mut vis, tf) in &mut beans {
        let at = tf.translation.truncate();
        if bean.eaten || !overlap(c, h, at, Vec2::splat(BEAN_BOX / 2.0)) {
            continue;
        }
        bean.eaten = true;
        *vis = Visibility::Hidden;
        boom.fuel = (boom.fuel + BEAN_FUEL).min(BOOM_FUEL);
        events.write(BoomEvent::Bean(at));
    }
}

/// An empty Boom sputters down (dry), then flies home and parks; the beans come back.
fn drift_home(
    time: Res<Time>,
    clock: Res<super::SimClock>,
    mut booms: Query<(&mut Boom, &mut Pos)>,
    mut beans: Query<(&mut Bean, &mut Visibility)>,
) {
    let dt = time.delta_secs();
    let mut parked = false;
    for (mut boom, mut pos) in &mut booms {
        match boom.state {
            BoomState::Parked => pos.0 = boom.home + Vec2::new(0.0, (clock.time * 3.0).sin() * 2.0),
            BoomState::Flying => {}
            BoomState::Home { dry } if dry > 0.0 => {
                boom.state = BoomState::Home { dry: dry - dt };
                pos.0.y -= 60.0 * dt;
            }
            BoomState::Home { .. } => {
                let to = boom.home - pos.0;
                let step = BOOM_HOME_SPEED * dt;
                if to.length() <= step {
                    pos.0 = boom.home;
                    boom.state = BoomState::Parked;
                    parked = true;
                } else {
                    pos.0 += to.normalize() * step;
                }
            }
        }
    }
    if parked {
        for (mut bean, mut vis) in &mut beans {
            bean.eaten = false;
            *vis = Visibility::Inherited;
        }
    }
}

/// A death aboard: the Boom's back home at once, parked, the beans back.
fn crash_aboard(
    mut commands: Commands,
    nat: Query<(Entity, &Aboard), (With<Player>, With<Dead>)>,
    mut booms: Query<(&mut Boom, &mut Pos, &mut PrevPos)>,
    mut beans: Query<(&mut Bean, &mut Visibility)>,
) {
    let Ok((e, aboard)) = nat.single() else { return };
    commands.entity(e).remove::<Aboard>();
    if let Ok((mut boom, mut pos, mut prev)) = booms.get_mut(aboard.boom) {
        boom.state = BoomState::Parked;
        pos.0 = boom.home;
        prev.0 = boom.home;
    }
    for (mut bean, mut vis) in &mut beans {
        bean.eaten = false;
        *vis = Visibility::Inherited;
    }
}

/// The Boom parked in cell (col, row), and beans.
pub(super) fn spawn_boom(commands: &mut Commands, center: Vec2) {
    commands.spawn((
        Name::new("Boom"),
        LevelEntity,
        Boom { home: center, state: BoomState::Parked, fuel: BOOM_FUEL },
        Pos(center),
        PrevPos(center),
        Transform::from_translation(center.extend(4.5)),
    ));
}

pub(super) fn spawn_bean(commands: &mut Commands, center: Vec2) {
    commands.spawn((Name::new("Bean"), LevelEntity, Bean::default(), Visibility::default(), Transform::from_translation(center.extend(2.0))));
}

/// The Boom's box joins Nat's for hazards while he's aboard: (min, max) of the pair.
pub fn hazard_box(nat: Vec2) -> (Vec2, Vec2) {
    let (c, h) = ride_box(nat);
    (c - h, c + h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_boom_sits_under_nat() {
        let nat = Vec2::new(40.0, 100.0);
        let (c, h) = ride_box(nat);
        assert_eq!(c.y + h.y, nat.y + tuning::PLAYER_SIZE.1 / 2.0, "the pair's top is Nat's head");
        assert_eq!(under(nat).y - BOOM_HALF.y, c.y - h.y, "its bottom is the Boom's");
        assert!(boom_speed(1.0, 1.0) < boom_speed(BOOM_FUEL, 1.0), "sputtering's slower");
    }
}
