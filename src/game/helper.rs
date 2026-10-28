//! The "helper" (`H`): a shifty guy on a ledge with a rope dangling off it. "HELP! HELP! I'LL
//! HELP YOU! COME UP HERE!" Grab the rope's end and he hauls Nat up, swinging it about, and
//! he lets go when he likes. His tricks go round in order, one a grab ([`TRICKS`]; a level's
//! second helper starts one further along, its third two):
//! - [`Trick::Drop`]: partway up he snickers, and lets go.
//! - [`Trick::Fling`]: he swings it hard and wide, snickers, and lets go at the bottom of a
//!   swing, out away from his ledge: Nat flies.
//! - [`Trick::Honest`]: all the way up, and he hoists Nat onto the ledge. "SEE? I HELPED!"
//!
//! The snicker comes [`SNICKER`] s before he lets go: time enough to jump off (Jump lets go,
//! with a jump and the swing's speed; the toot's back for a dropped Nat too). After a trick he
//! lowers the rope again and waits for the next grab. Nat on the rope is out of the physics'
//! hands ([`OnRope`]): the rope's end carries him, the tiles still stop him, and hazards are
//! as deadly as ever.

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::physics::{Body, Dead, Fall, Finished, PlayerControl, TileMood, step_body};
use super::{ActiveLevel, GameSet, Groove, LevelEntity, Player, Pos, PrevPos, tuning};
use crate::events::{Jumped, ToyShout};
use crate::input::Action;
use crate::level::{HELPER_HAUL, HELPER_ROPE, Level, TILE, ThingKind};

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Helper>()
        .add_message::<RopeEvent>()
        .add_systems(FixedUpdate, (grab_rope, swing_rope).chain().in_set(GameSet::Player).after(super::physics::player_step))
        .add_systems(FixedUpdate, dropped_dead.in_set(GameSet::Interact).after(super::hazards::check_hazards));
}

/// His tricks, in the order he plays them (one a grab, then round again).
pub const TRICKS: [Trick; 3] = [Trick::Drop, Trick::Fling, Trick::Honest];
/// He snickers this long (s) before he lets go.
pub const SNICKER: f32 = 0.6;
/// The swing's period (s).
pub const SWING_PERIOD: f32 = 1.6;
/// The shortest the rope gets (px): his hands.
pub const ROPE_MIN: f32 = 14.0;
/// He lowers the rope again this fast (px/s) after a trick.
const LOWER_SPEED: f32 = 120.0;
/// A Nat who just let go can't grab the rope again for this long (s).
const REGRAB: f32 = 0.5;
/// Nat grabs the rope within this far (px) of it.
const GRAB_REACH: f32 = 7.0;
/// He calls out to a Nat this close (px), every few seconds.
const CALL_REACH: f32 = 7.0 * TILE;
const CALL_EVERY: f32 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum Trick {
    Drop,
    Fling,
    Honest,
}

#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub enum HelperState {
    /// The rope hangs at full length, waiting.
    Waiting,
    /// Nat's on it: `t` s into the trick.
    Hauling { trick: Trick, t: f32 },
    /// Snickering (`t` s of it left), still holding: then he lets go.
    Snicker { trick: Trick, t: f32 },
    /// Lowering the rope back down.
    Lowering,
}

/// The helper. `Pos` is where he stands (his box's middle, on his ledge).
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Helper {
    /// The rope's top: his hand, just past the ledge's edge.
    pub pivot: Vec2,
    /// Which way the rope hangs off his ledge (+1 right, -1 left).
    pub side: f32,
    /// The rope at full length (px).
    pub rope: f32,
    /// The rope now (px).
    pub len: f32,
    /// Its swing (radians from straight down).
    pub angle: f32,
    pub state: HelperState,
    /// Grabs so far this level visit (his next trick is `TRICKS[(first + grabs) % 3]`).
    pub grabs: u32,
    /// Where in [`TRICKS`] he starts (the level's helpers in reading order: 0, 1, 2, 0...).
    pub first: u32,
    /// Since he last called out (s).
    pub since_call: f32,
    /// Since Nat last let go (s).
    pub since_release: f32,
}

impl Helper {
    /// Where the rope's end is.
    pub fn end(&self) -> Vec2 {
        self.pivot + self.len * Vec2::new(self.angle.sin(), -self.angle.cos())
    }

    /// His trick on this grab.
    pub fn next_trick(&self) -> Trick {
        TRICKS[(self.first + self.grabs) as usize % TRICKS.len()]
    }

    /// Is he snickering (about to let go)?
    pub fn snickering(&self) -> bool {
        matches!(self.state, HelperState::Snicker { .. })
    }
}

/// On Nat while he hangs on the helper's rope.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct OnRope {
    pub helper: Entity,
    /// His velocity last step (px/s): what he flies off with.
    pub vel: Vec2,
    /// Seconds he's hung on (a jump that grabbed it doesn't let go of it).
    pub held: f32,
}

/// What happened on a rope (for sounds).
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub enum RopeEvent {
    Grabbed(Vec2),
    /// He snickers: he's about to let go.
    Snicker(Vec2),
    /// He let go of Nat (a trick).
    LetGo(Vec2),
    /// Nat jumped off.
    JumpedOff(Vec2),
    /// The honest one: hoisted onto the ledge.
    Hoisted(Vec2),
}

/// How far the rope swings `t` s into `trick` (radians), and the swing angle then.
pub fn swing(trick: Trick, t: f32, side: f32) -> f32 {
    let amp = match trick {
        Trick::Drop => (0.12 + 0.1 * t).min(0.45),
        Trick::Fling => (0.2 + 0.35 * t).min(1.1),
        Trick::Honest => 0.12,
    };
    // Out away from the ledge and back to hang straight down (the ledge's wall is on the
    // other side): fastest half way out.
    side * amp * (1.0 - (t * std::f32::consts::TAU / SWING_PERIOD).cos()) / 2.0
}

/// How fast he hauls in for `trick` (px/s): the fling doesn't (it's all swing).
pub fn haul(trick: Trick) -> f32 {
    if trick == Trick::Fling { 0.0 } else { HELPER_HAUL }
}

/// When (s into the trick) he starts to snicker: `None` for the honest one. The fling waits
/// for the bottom of a swing, heading out: its snicker ends there.
pub fn snicker_at(trick: Trick, rope: f32) -> Option<f32> {
    match trick {
        // Half way up.
        Trick::Drop => Some(((rope - ROPE_MIN) * 0.5 / HELPER_HAUL).max(0.4)),
        // The third time it swings out, half way, at its fastest; less the snicker.
        Trick::Fling => Some(2.25 * SWING_PERIOD - SNICKER),
        Trick::Honest => None,
    }
}

/// The helper standing in cell (col, row), his rope: hanging off whichever side of his ledge
/// is open (right if both), down to a tile and a bit above the floor below (at most
/// [`HELPER_ROPE`] tiles).
pub fn rope_of(level: &Level, col: usize, row: usize) -> (Vec2, f32, f32) {
    let (c, r) = (col as i32, row as i32);
    let open = |dc: i32| !level.tile(c + dc, r + 1).is_solid() && !level.tile(c + dc, r).is_solid();
    let side = if open(1) || !open(-1) { 1.0 } else { -1.0 };
    let center = level.tile_center(col, row);
    let pivot = Vec2::new(center.x + side * (TILE / 2.0 + 3.0), center.y + 2.0);
    let rc = c + side as i32;
    let mut floor = r + 1;
    while floor < level.height as i32 && !level.tile(rc, floor).is_solid() && !level.tile(rc, floor).is_one_way() {
        floor += 1;
    }
    let floor_y = (level.height as i32 - floor) as f32 * TILE;
    let len = (pivot.y - floor_y - 1.4 * TILE).clamp(ROPE_MIN, HELPER_ROPE * TILE);
    (pivot, side, len)
}

/// Nat's middle when his hands are at the rope's end `end`.
fn hanging_at(end: Vec2) -> Vec2 {
    end - Vec2::new(0.0, tuning::PLAYER_SIZE.1 / 2.0 - 3.0)
}

fn dist_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let k = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    p.distance(a + ab * k)
}

type Free = (With<Player>, Without<Dead>, Without<Finished>, Without<OnRope>, Without<super::boom::Aboard>);

/// Touching a waiting rope (its lower half): Nat grabs it, and the helper's trick starts.
fn grab_rope(
    mut commands: Commands,
    time: Res<Time>,
    nat: Query<(Entity, &Pos, &Body), Free>,
    mut helpers: Query<(Entity, &mut Helper, &Pos), Without<Player>>,
    mut events: MessageWriter<RopeEvent>,
    mut shout: MessageWriter<ToyShout>,
) {
    let dt = time.delta_secs();
    let nat = nat.single().ok();
    let mut grabbed = false;
    for (he, mut h, hp) in &mut helpers {
        h.since_release += dt;
        h.since_call += dt;
        let Some((e, pos, body)) = nat else { continue };
        if h.state == HelperState::Waiting && h.since_call >= CALL_EVERY && pos.0.distance(hp.0) < CALL_REACH {
            h.since_call = 0.0;
            let line = if h.grabs == 0 { "HELP! HELP!" } else { "COME UP HERE!" };
            shout.write(ToyShout { pos: hp.0 + Vec2::Y * 16.0, text: line });
        }
        if grabbed || h.state != HelperState::Waiting || h.since_release < REGRAB {
            continue;
        }
        let (top, end) = (h.pivot.lerp(h.end(), 0.5), h.end());
        // His hands (the top of his box) near the rope's lower half.
        let hands = pos.0 + Vec2::Y * (body.half.y - 3.0);
        if dist_to_segment(hands, top, end).min(dist_to_segment(pos.0, top, end)) > GRAB_REACH {
            continue;
        }
        // He takes up the slack to where Nat grabbed.
        h.len = (h.pivot.y - hands.y).clamp(ROPE_MIN, h.rope);
        h.angle = 0.0;
        let trick = h.next_trick();
        h.grabs += 1;
        h.state = HelperState::Hauling { trick, t: 0.0 };
        h.since_call = 0.0;
        commands.entity(e).insert(OnRope { helper: he, vel: body.vel, held: 0.0 });
        events.write(RopeEvent::Grabbed(h.end()));
        shout.write(ToyShout { pos: hp.0 + Vec2::Y * 16.0, text: "I'LL HELP YOU!" });
        grabbed = true;
    }
}

/// The helper's side of things: hauling Nat in and swinging him about, his snicker, letting
/// go; Nat's: a jump lets go. Ropes with nobody on them sway, or come back down.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn swing_rope(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<super::SimClock>,
    active: Res<ActiveLevel>,
    groove: Res<Groove>,
    input: Single<&ActionState<Action>>,
    mut nat: Query<(Entity, &mut Pos, &mut Body, &mut PlayerControl, &mut OnRope), (With<Player>, Without<Dead>, Without<Finished>)>,
    mut helpers: Query<(Entity, &mut Helper, &Pos), Without<Player>>,
    mut events: MessageWriter<RopeEvent>,
    mut shout: MessageWriter<ToyShout>,
    mut jumped: MessageWriter<Jumped>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let mut on = nat.single_mut().ok();
    for (he, mut h, hp) in &mut helpers {
        let riding = on.as_ref().is_some_and(|n| n.4.helper == he);
        match h.state {
            HelperState::Waiting => {
                // A lazy sway.
                h.angle = 0.08 * (clock.time * std::f32::consts::TAU / (SWING_PERIOD * 1.5)).sin();
                continue;
            }
            HelperState::Lowering => {
                h.angle *= 1.0 - (4.0 * dt).min(1.0);
                h.len = (h.len + LOWER_SPEED * dt).min(h.rope);
                if h.len >= h.rope {
                    h.state = HelperState::Waiting;
                }
                continue;
            }
            _ if !riding => {
                h.state = HelperState::Lowering;
                continue;
            }
            _ => {}
        }
        let Some((e, pos, body, ctl, rope)) = on.as_mut() else { continue };
        let let_go = |ctl: &mut PlayerControl, body: &mut Body, rope: &OnRope, commands: &mut Commands| {
            commands.entity(*e).remove::<OnRope>();
            *ctl = PlayerControl { facing: ctl.facing, ..PlayerControl::default() };
            body.on_ground = false;
            body.vel = rope.vel;
        };
        rope.held += dt;
        // Nat lets go: a jump off it (not the jump that caught it).
        if input.just_pressed(&Action::Jump) && rope.held > 0.1 {
            let_go(ctl, body, rope, &mut commands);
            body.vel.y = tuning::JUMP_SPEED * groove.jump_scale() * 0.9 + body.vel.y.max(0.0) * 0.5;
            ctl.cut_armed = true;
            jumped.write(Jumped { pos: pos.0, double: false });
            events.write(RopeEvent::JumpedOff(pos.0));
            h.state = HelperState::Lowering;
            h.since_release = 0.0;
            continue;
        }
        let (trick, t) = match h.state {
            HelperState::Hauling { trick, t } | HelperState::Snicker { trick, t } => (trick, t),
            _ => unreachable!(),
        };
        match h.state {
            HelperState::Hauling { .. } => {
                let t = t + dt;
                h.len = (h.len - haul(trick) * dt).max(ROPE_MIN);
                h.state = HelperState::Hauling { trick, t };
                if snicker_at(trick, h.rope).is_some_and(|at| t >= at) {
                    h.state = HelperState::Snicker { trick, t: SNICKER };
                    events.write(RopeEvent::Snicker(hp.0));
                    shout.write(ToyShout { pos: hp.0 + Vec2::Y * 16.0, text: "HEHEHE..." });
                } else if trick == Trick::Honest && h.len <= ROPE_MIN {
                    // Up and over, onto the ledge behind him, with a little hop.
                    let_go(ctl, body, rope, &mut commands);
                    pos.0 = Vec2::new(hp.0.x - h.side * TILE * 0.6, hp.0.y - TILE / 2.0 + body.half.y + 1.0);
                    body.vel = Vec2::new(0.0, 160.0);
                    h.state = HelperState::Lowering;
                    h.since_release = 0.0;
                    events.write(RopeEvent::Hoisted(pos.0));
                    shout.write(ToyShout { pos: hp.0 + Vec2::Y * 16.0, text: "SEE? I HELPED!" });
                    continue;
                }
            }
            HelperState::Snicker { t: left, .. } => {
                let left = left - dt;
                if trick == Trick::Drop {
                    // (He stops hauling to giggle.)
                } else {
                    h.len = (h.len - haul(trick) * dt).max(ROPE_MIN);
                }
                if left <= 0.0 {
                    let_go(ctl, body, rope, &mut commands);
                    h.state = HelperState::Lowering;
                    h.since_release = 0.0;
                    events.write(RopeEvent::LetGo(pos.0));
                    shout.write(ToyShout { pos: hp.0 + Vec2::Y * 16.0, text: "OOPS!" });
                    continue;
                }
                h.state = HelperState::Snicker { trick, t: left };
            }
            _ => {}
        }
        // The swing's clock runs on through the snicker.
        let swing_t = match h.state {
            HelperState::Hauling { t, .. } => t,
            HelperState::Snicker { t: left, .. } => snicker_at(trick, h.rope).unwrap_or(0.0) + SNICKER - left,
            _ => t,
        };
        h.angle = swing(trick, swing_t, h.side);
        // Carry Nat to the rope's end; the tiles have their say.
        let target = hanging_at(h.end());
        let vel = (target - pos.0) / dt;
        let mut c = pos.0;
        let mut b = Body { vel, half: body.half, on_ground: false, ..Body::player() };
        let fall = Fall { gravity: 0.0, max_fall: 10_000.0, tiles: TileMood::of(&groove) };
        step_body(&active.level, &mut c, &mut b, fall, &[], &[], &[], dt);
        rope.vel = (c - pos.0) / dt;
        pos.0 = c;
        body.vel = Vec2::ZERO;
        body.on_ground = false;
        body.riding = None;
        body.launch = 0.0;
        body.pad = 0.0;
        ctl.has_toot = true;
    }
}

/// A death on the rope: the helper lowers it again.
fn dropped_dead(mut commands: Commands, nat: Query<(Entity, &OnRope), (With<Player>, With<Dead>)>, mut helpers: Query<&mut Helper>) {
    let Ok((e, rope)) = nat.single() else { return };
    commands.entity(e).remove::<OnRope>();
    if let Ok(mut h) = helpers.get_mut(rope.helper) {
        h.state = HelperState::Lowering;
        h.since_release = 0.0;
    }
}

/// The helper standing in cell (col, row).
pub(super) fn spawn_helper(commands: &mut Commands, level: &Level, col: usize, row: usize) {
    let (pivot, side, rope) = rope_of(level, col, row);
    let at = level.tile_center(col, row);
    let first = level.things.iter().filter(|t| t.kind == ThingKind::Helper).position(|t| (t.col, t.row) == (col, row)).unwrap_or(0) as u32;
    commands.spawn((
        Name::new("Helper"),
        LevelEntity,
        Helper {
            pivot,
            side,
            rope,
            len: rope,
            angle: 0.0,
            state: HelperState::Waiting,
            grabs: 0,
            first,
            since_call: CALL_EVERY,
            since_release: REGRAB,
        },
        Pos(at),
        PrevPos(at),
        Transform::from_translation(at.extend(3.0)),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fling_lets_go_at_its_fastest_heading_out() {
        let at = snicker_at(Trick::Fling, 100.0).unwrap() + SNICKER;
        let speed = |t: f32| swing(Trick::Fling, t + 0.005, 1.0) - swing(Trick::Fling, t - 0.005, 1.0);
        assert!(speed(at) > 0.0, "heading out (right) off a ledge whose rope hangs right");
        assert!(speed(at) >= speed(at - 0.05) && speed(at) >= speed(at + 0.05), "at its fastest");
        for k in 0..100 {
            assert!(swing(Trick::Fling, k as f32 * 0.05, 1.0) >= 0.0, "never back into the ledge");
        }
    }
}
