//! Hand-rolled player physics: tile AABB collision (X then Y) against the level grid, one-way
//! tiles and moving platforms; coyote time, jump buffering, variable jump height, the toot.
//! The music's [`Groove`] scales gravity and run speed, can make landings bounce, and in the
//! waltz boosts a ground jump on ONE (whose toot is then a weak one); the laughing band's
//! phrases nudge them a little more ([`Groove::nudge`]).
//!
//! # Grease
//! Standing on grease (`_`, any grease tile under Nat's feet) Nat slips: no braking (ground
//! decel ×[`GREASE_DECEL`], i.e. none: he keeps sliding at his speed, and pushing the other way
//! does nothing), weak steering (ground accel ×[`GREASE_STEER`]), and no jumping off it (no
//! ground jump, no coyote jump after sliding off, no toot from the ground, no laughing-band
//! bounce). His feet are back to normal the moment he's in the air (a toot after sliding off a
//! ledge works). **Sweaty grip**: while the band is nervous ([`Groove::grip`], the 3+ deaths
//! mood) grease is just ground.
//!
//! # The body step
//! [`step_body`] is the shared physics of every body (Nat and Han): gravity, riding
//! [`Carrier`]s (moving platforms, rafts, Han's head), tile collision. Nat's controller
//! ([`PlayerControl`]: input, jumps, toots, grease, bounces) wraps it; Han's AI wraps it with
//! his own (`game::han`). Han's head is a one-way carrier for Nat ([`HanHead`]); jumping off it
//! is the plunger boost ([`BOOST_SPEED`], [`HanBoosted`]; in a band zone the weak one,
//! [`WEAK_BOOST_SPEED`]).

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::groove::{
    BOUNCE_MIN_SPEED, BOUNCE_RESTITUTION, BOUNCE_SPEED, JumpedOnOne, WALTZ_ONE_BOOST, WALTZ_ONE_TOOT_SPEED,
};
use super::{ActiveLevel, GameSet, Groove, MovingPlatform, Player, Pos, PrevPos, tuning::*};
use crate::events::{Jumped, Landed};
use crate::input::Action;
use crate::level::{Level, TILE, Tile};

pub(super) fn plugin(app: &mut App) {
    app.add_message::<HanBoosted>().add_systems(FixedUpdate, player_step.in_set(GameSet::Player));
}

/// Landing faster than this (px/s) writes [`Landed`].
pub const LAND_EVENT_SPEED: f32 = 100.0;
/// Ground deceleration on grease (×[`GROUND_DECEL`]): none, you keep sliding.
pub const GREASE_DECEL: f32 = 0.0;
/// Ground acceleration on grease (×[`GROUND_ACCEL`]): weak steering.
pub const GREASE_STEER: f32 = 0.2;

const EPS: f32 = 0.01;
/// Slack when deciding whether feet were above a one-way surface last step.
const ONE_WAY_SLACK: f32 = 0.5;

/// A physics box: velocity (px/s, y up), half extents, contact state.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct Body {
    pub vel: Vec2,
    pub half: Vec2,
    pub on_ground: bool,
    /// The moving platform stood on, if any.
    pub riding: Option<Entity>,
}

impl Body {
    pub fn player() -> Self {
        Self {
            vel: Vec2::ZERO,
            half: Vec2::new(PLAYER_SIZE.0, PLAYER_SIZE.1) / 2.0,
            on_ground: false,
            riding: None,
        }
    }
}

/// The player's controller state.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct PlayerControl {
    /// +1 facing right, -1 facing left.
    pub facing: f32,
    /// Time left to ground-jump after leaving the ground.
    pub coyote: f32,
    /// Time left on a buffered jump press.
    pub buffer: f32,
    /// The mid-air toot jump is available.
    pub has_toot: bool,
    /// Releasing jump will cut the rise.
    pub cut_armed: bool,
    /// In the air from a laughing-band landing bounce: still counts as grounded for jumping.
    pub bouncing: bool,
    /// The toot left after a waltz jump on ONE: a weak one ([`WALTZ_ONE_TOOT_SPEED`]).
    pub weak_toot: bool,
    /// Standing on grease (as of the last step's landing check).
    pub on_grease: bool,
    /// Last stood on Han's head in a band zone ([`HanHead::weak`]): a (coyote) jump now is the
    /// weak boost.
    pub weak_ground: bool,
    /// Seconds since the last landing (the seasick phrase's slippery landings).
    pub since_landing: f32,
    /// The rise (px/s) the last jump cut threw away, and how long ago: what Nat would be
    /// rising at now had he held jump (the overtones phrase's toot counts it).
    pub uncut: f32,
    pub since_cut: f32,
}

impl Default for PlayerControl {
    fn default() -> Self {
        Self {
            facing: 1.0,
            coyote: 0.0,
            buffer: 0.0,
            has_toot: true,
            cut_armed: false,
            bouncing: false,
            weak_toot: false,
            on_grease: false,
            weak_ground: false,
            since_landing: f32::INFINITY,
            uncut: 0.0,
            since_cut: 0.0,
        }
    }
}

/// Is any grease under the feet of a box centered at `pos` (half extents `half`)?
pub(super) fn grease_underfoot(level: &Level, pos: Vec2, half: Vec2) -> bool {
    let j = idx(pos.y - half.y - 0.5);
    let (xs, _) = cells(pos - half, pos + half);
    xs.into_iter().any(|i| tile_at(level, i, j) == Tile::Grease)
}

/// The player is splatted; respawns when `remaining` runs out. Frozen meanwhile.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Dead {
    pub remaining: f32,
}

/// Touched the goal: frozen for the results screen.
#[derive(Component, Debug, Clone, Copy, Default)]
pub(super) struct Finished;

/// Grid row of the tile whose world-space vertical index (0 = bottom row of the level) is `j`.
fn row_of(level: &Level, j: i32) -> i32 {
    level.height as i32 - 1 - j
}

fn idx(v: f32) -> i32 {
    (v / TILE).floor() as i32
}

/// Inclusive tile index ranges covered by the box [min, max] (touching edges don't count).
pub(super) fn cells(min: Vec2, max: Vec2) -> (std::ops::RangeInclusive<i32>, std::ops::RangeInclusive<i32>) {
    (idx(min.x + EPS)..=idx(max.x - EPS), idx(min.y + EPS)..=idx(max.y - EPS))
}

/// Tile at world-space tile indices (column, vertical index from the bottom).
pub(super) fn tile_at(level: &Level, i: i32, j: i32) -> Tile {
    level.tile(i, row_of(level, j))
}

/// Something a body can stand on from above and be carried by: a moving platform, a stain
/// raft, Han's head. Solid on top only (like `=`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Carrier {
    pub id: Entity,
    /// Center x and half width of the top surface.
    pub x: f32,
    pub half_w: f32,
    /// Height of the top surface now and at the start of the step.
    pub top: f32,
    pub prev_top: f32,
    /// How far it moved this step (riders move with it).
    pub carry: Vec2,
}

impl Carrier {
    /// A moving platform (or raft) at `pos` (its top tile row's center), `prev` a step ago.
    pub fn platform(id: Entity, pos: Vec2, prev: Vec2, width: usize) -> Self {
        Carrier {
            id,
            x: pos.x,
            half_w: width as f32 * TILE / 2.0,
            top: pos.y + TILE / 2.0,
            prev_top: prev.y + TILE / 2.0,
            carry: pos - prev,
        }
    }
}

/// Gravity for [`step_body`]: acceleration (px/s², downward) and terminal fall speed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fall {
    pub gravity: f32,
    pub max_fall: f32,
}

impl Fall {
    /// Nat's gravity under `groove`.
    pub fn of(groove: &Groove) -> Self {
        Fall { gravity: GRAVITY * groove.gravity_now(), max_fall: MAX_FALL * groove.fall_scale() }
    }
}

/// What [`step_body`] found.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Contact {
    /// Standing on something after the step.
    pub ground: bool,
    /// The carrier stood on, if any.
    pub riding: Option<Entity>,
    /// `body.on_ground` before the step.
    pub was_on_ground: bool,
    /// Downward speed before the step (px/s), for landing effects.
    pub fall_speed: f32,
    /// Ran into a wall (or a blocker) this step.
    pub blocked_x: bool,
}

/// One fixed step of the shared body physics (Nat and Han): gravity, being carried by the
/// carrier stood on, then tile AABB collision X then Y (solid tiles, one-way tiles from above),
/// then the carriers (one-way, from above). `blockers` are extra boxes (min, max) that stop
/// horizontal motion into them (a body already overlapping one isn't pushed). Sets
/// `body.on_ground`/`riding` and zeroes the vertical speed on landing; landing effects (events,
/// toot refresh, bounces) are the caller's.
pub fn step_body(
    level: &Level,
    pos: &mut Vec2,
    body: &mut Body,
    fall: Fall,
    carriers: &[Carrier],
    blockers: &[(Vec2, Vec2)],
    dt: f32,
) -> Contact {
    // --- Gravity.
    body.vel.y = (body.vel.y - fall.gravity * dt).max(-fall.max_fall);

    // --- Carried by what we stood on.
    let carry = body.riding.and_then(|e| carriers.iter().find(|c| c.id == e)).map_or(Vec2::ZERO, |c| c.carry);

    let half = body.half;
    let was_on_ground = body.on_ground;
    let fall_speed = -body.vel.y;
    let mut blocked_x = false;

    // --- X.
    let dx = body.vel.x * dt + carry.x;
    let x_before = pos.x;
    pos.x += dx;
    if dx != 0.0 {
        let (xs, ys) = cells(*pos - half, *pos + half);
        for j in ys {
            for i in xs.clone() {
                if !tile_at(level, i, j).is_solid() {
                    continue;
                }
                if dx > 0.0 {
                    pos.x = pos.x.min(i as f32 * TILE - half.x);
                } else {
                    pos.x = pos.x.max((i + 1) as f32 * TILE + half.x);
                }
                body.vel.x = 0.0;
                blocked_x = true;
            }
        }
        for &(min, max) in blockers {
            let overlaps = |x: f32| x + half.x > min.x && x - half.x < max.x;
            let y_overlaps = pos.y + half.y > min.y + EPS && pos.y - half.y < max.y - EPS;
            if !y_overlaps || overlaps(x_before) || !overlaps(pos.x) {
                continue;
            }
            pos.x = if dx > 0.0 { min.x - half.x } else { max.x + half.x };
            body.vel.x = 0.0;
            blocked_x = true;
        }
    }

    // --- Y.
    let bottom_before = pos.y - half.y + carry.y;
    let dy = body.vel.y * dt + carry.y;
    pos.y += dy;
    let mut ground = false;
    let mut riding = None;
    if dy != 0.0 {
        let (xs, ys) = cells(*pos - half, *pos + half);
        for j in ys {
            for i in xs.clone() {
                let tile = tile_at(level, i, j);
                let top = (j + 1) as f32 * TILE;
                match tile {
                    t if t.is_solid() && dy < 0.0 => {
                        pos.y = pos.y.max(top + half.y);
                        ground = true;
                    }
                    t if t.is_solid() => {
                        pos.y = pos.y.min(j as f32 * TILE - half.y);
                        body.vel.y = body.vel.y.min(0.0);
                    }
                    t if t.is_one_way()
                        && body.vel.y <= 0.0
                        && bottom_before - carry.y >= top - ONE_WAY_SLACK
                        && pos.y - half.y < top =>
                    {
                        pos.y = top + half.y;
                        ground = true;
                    }
                    _ => {}
                }
            }
        }
    }
    // Carriers (one-way).
    if body.vel.y <= 0.0 {
        for c in carriers {
            if pos.x + half.x <= c.x - c.half_w + EPS || pos.x - half.x >= c.x + c.half_w - EPS {
                continue;
            }
            let feet = pos.y - half.y;
            let before = if body.riding == Some(c.id) { bottom_before } else { bottom_before - carry.y };
            if before >= c.top.min(c.prev_top) - ONE_WAY_SLACK && feet <= c.top + EPS {
                pos.y = c.top + half.y;
                ground = true;
                riding = Some(c.id);
            }
        }
    }
    if ground {
        body.vel.y = 0.0;
    }
    body.on_ground = ground;
    body.riding = riding;
    Contact { ground, riding, was_on_ground, fall_speed, blocked_x }
}

/// Han's head as Nat sees it (written by Han's systems): a one-way carrier while `solid`;
/// jumping off it is the plunger boost while `boost`; while `block` (marching ahead) his body
/// also stops Nat walking into him from the side.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Reflect)]
#[reflect(Component)]
pub struct HanHead {
    pub solid: bool,
    pub boost: bool,
    pub block: bool,
    /// In a band zone (`Level::in_band_zone`): jumping off him (or off the coyote time after
    /// stepping off him) is the weak boost ([`WEAK_BOOST_SPEED`]), winded or not.
    pub weak: bool,
}

/// Nat jumped off Han's head: the plunger boost (`weak`: the feeble one of a band zone). Not a
/// [`Jumped`] (the band doesn't count it).
#[derive(Message, Debug, Clone, Copy)]
pub struct HanBoosted {
    pub pos: Vec2,
    pub weak: bool,
}

/// Upward speed of the weak boost, Han's in a band zone (px/s): Nat's feet rise 300²/2800 ≈ 32
/// px above Han's head, ≈ 46 px (2.9 tiles) above Han's floor: about a normal jump (51.6 px).
/// It refreshes the toot like any landing, and the toot adds ~39 px: ≤ 85 px with ideal timing,
/// under a perfect double jump from the ground (90.5 px) and 11 px under a 6-tile giant wall.
/// The validator proves no weak boost (from wherever Han stands in the zone), and no chain of
/// them, opens a band gate (`level::validate`).
pub const WEAK_BOOST_SPEED: f32 = 300.0;

/// Upward speed of the plunger boost (px/s): Nat's feet rise 560²/2800 = 112 px = 7 tiles above
/// Han's head (≈ 7.9 tiles above Han's floor), and the refreshed toot adds ~39 px: ≈ 10.3
/// tiles with ideal timing, ≥ 9.3 for a human. A *buddy ledge* is 9 tiles: out of Giant
/// Steps' reach (a perfect GS double jump tops out at ~8.7 tiles) yet a comfortable boost.
pub const BOOST_SPEED: f32 = 560.0;

/// Han's head as a carrier, if it's solid (`pos` is his box center).
pub fn han_carrier(id: Entity, pos: Vec2, prev: Vec2, head: &HanHead) -> Option<Carrier> {
    head.solid.then(|| Carrier {
        id,
        x: pos.x,
        half_w: PLAYER_SIZE.0 / 2.0,
        top: pos.y + PLAYER_SIZE.1 / 2.0,
        prev_top: prev.y + PLAYER_SIZE.1 / 2.0,
        carry: pos - prev,
    })
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn player_step(
    time: Res<Time>,
    active: Res<ActiveLevel>,
    groove: Res<Groove>,
    assists: Res<super::Assists>,
    input: Single<&ActionState<Action>>,
    mut player: Query<
        (&mut Pos, &mut Body, &mut PlayerControl),
        (With<Player>, Without<Dead>, Without<Finished>, Without<MovingPlatform>),
    >,
    platforms: Query<(Entity, &Pos, &PrevPos, &MovingPlatform), Without<Player>>,
    han: Query<(Entity, &Pos, &PrevPos, &HanHead), (Without<Player>, Without<MovingPlatform>)>,
    mut jumped: MessageWriter<Jumped>,
    mut landed: MessageWriter<Landed>,
    mut on_one: MessageWriter<JumpedOnOne>,
    mut boosted: MessageWriter<HanBoosted>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let level = &active.level;
    let Ok((mut pos, mut body, mut ctl)) = player.single_mut() else { return };

    // --- Input -> horizontal velocity.
    let axis = input.pressed(&Action::Right) as i32 as f32 - input.pressed(&Action::Left) as i32 as f32;
    if axis != 0.0 {
        ctl.facing = axis;
    }
    let run = groove.run_scale();
    let target = axis * RUN_SPEED * run;
    // Slipping on grease (no sweaty grip): can't brake, steers weakly, can't jump.
    let slipping = body.on_ground && ctl.on_grease && !groove.grip();
    ctl.since_landing += dt;
    ctl.since_cut += dt;
    let accel = run
        * if !body.on_ground {
            AIR_ACCEL
        } else if axis == 0.0 || axis * body.vel.x < 0.0 {
            GROUND_DECEL * if slipping { GREASE_DECEL } else { groove.landing_decel(ctl.since_landing) }
        } else {
            GROUND_ACCEL * if slipping { GREASE_STEER } else { 1.0 }
        };
    body.vel.x = move_towards(body.vel.x, target, accel * dt);

    // --- Jumping.
    if slipping {
        ctl.coyote = 0.0;
        ctl.has_toot = true;
        ctl.weak_toot = false;
    } else if body.on_ground || ctl.bouncing {
        ctl.coyote = assists.coyote_time();
        ctl.has_toot = true;
        ctl.weak_toot = false;
    } else {
        ctl.coyote -= dt;
    }
    let pressed = input.just_pressed(&Action::Jump);
    if pressed {
        ctl.buffer = assists.jump_buffer();
    } else {
        ctl.buffer -= dt;
    }
    let on_han = body
        .riding
        .and_then(|e| han.get(e).ok())
        .map(|(_, _, _, head)| *head);
    if ctl.buffer > 0.0 && ctl.coyote > 0.0 {
        // Off Han's head in a band zone (or just stepped off it): the weak boost, winded or not.
        let weak = on_han.map_or(!body.on_ground && ctl.weak_ground, |h| h.weak);
        if weak {
            body.vel.y = WEAK_BOOST_SPEED;
            ctl.has_toot = true;
            ctl.weak_toot = false;
            boosted.write(HanBoosted { pos: pos.0, weak: true });
        } else if on_han.is_some_and(|h| h.boost) {
            // The plunger boost: way up, and the toot's fresh again.
            body.vel.y = BOOST_SPEED;
            ctl.has_toot = true;
            ctl.weak_toot = false;
            boosted.write(HanBoosted { pos: pos.0, weak: false });
        } else {
            body.vel.y = JUMP_SPEED * groove.jump_scale();
            if groove.on_the_one() {
                // A waltz step on ONE: higher, golden, and its toot is a weak one.
                body.vel.y *= WALTZ_ONE_BOOST;
                ctl.weak_toot = true;
                on_one.write(JumpedOnOne { pos: pos.0 });
            }
            jumped.write(Jumped { pos: pos.0, double: false });
        }
        ctl.buffer = 0.0;
        ctl.coyote = 0.0;
        ctl.cut_armed = true;
        ctl.uncut = 0.0;
        ctl.bouncing = false;
        body.on_ground = false;
        body.riding = None;
    } else if pressed && ctl.has_toot && !slipping {
        let base = if ctl.weak_toot { WALTZ_ONE_TOOT_SPEED } else { DOUBLE_JUMP_SPEED };
        // Letting go of jump to toot cut the rise: the overtones count the rise he'd have had
        // (with the heaviest gravity there could have been, so never more).
        let heaviest = GRAVITY * groove.gravity_scale * (1.0 + super::groove::ALIEN_PULSE);
        let held = ctl.uncut - heaviest * ctl.since_cut;
        body.vel.y = groove.toot_speed(base, body.vel.y.max(held));
        ctl.uncut = 0.0;
        ctl.has_toot = false;
        ctl.weak_toot = false;
        ctl.buffer = 0.0;
        ctl.cut_armed = true;
        jumped.write(Jumped { pos: pos.0, double: true });
    }
    if ctl.cut_armed && !input.pressed(&Action::Jump) && body.vel.y > 0.0 {
        ctl.uncut = body.vel.y;
        ctl.since_cut = 0.0;
        body.vel.y *= JUMP_CUT;
        ctl.cut_armed = false;
    }
    if body.vel.y <= 0.0 {
        ctl.cut_armed = false;
    }

    // --- Move: gravity, carriers, collisions.
    let mut carriers: Vec<Carrier> =
        platforms.iter().map(|(e, p, prev, plat)| Carrier::platform(e, p.0, prev.0, plat.width)).collect();
    let mut blockers = Vec::new();
    for (e, p, prev, head) in &han {
        carriers.extend(han_carrier(e, p.0, prev.0, head));
        if head.block {
            let h = Vec2::new(PLAYER_SIZE.0, PLAYER_SIZE.1) / 2.0;
            blockers.push((p.0 - h, p.0 + h));
        }
    }
    let contact = step_body(level, &mut pos.0, &mut body, Fall::of(&groove), &carriers, &blockers, dt);
    let mut ground = contact.ground;
    let on_grease = ground && contact.riding.is_none() && grease_underfoot(level, pos.0, body.half);
    ctl.on_grease = on_grease;
    if ground {
        if !contact.was_on_ground && contact.fall_speed > LAND_EVENT_SPEED {
            landed.write(Landed { pos: pos.0, speed: contact.fall_speed });
        }
        if !contact.was_on_ground {
            ctl.since_landing = 0.0;
        }
        ctl.has_toot = true;
        ctl.weak_toot = false;
        ctl.bouncing = false;
        ctl.weak_ground = contact.riding.and_then(|e| han.get(e).ok()).is_some_and(|(_, _, _, h)| h.weak);
        // The laughing band: spring back up a little (lower each time, until it dies out).
        // Grease doesn't bounce (unless you've got grip): it would be a jump off grease. Nor
        // does a band zone's Han: a bounce plus the weak boost would be more than a jump.
        let slick = on_grease && !groove.grip();
        if groove.bouncy()
            && !groove.grip()
            && !slick
            && !ctl.weak_ground
            && !contact.was_on_ground
            && contact.fall_speed > BOUNCE_MIN_SPEED
        {
            body.vel.y = (contact.fall_speed * BOUNCE_RESTITUTION).min(BOUNCE_SPEED);
            ctl.bouncing = true;
            ground = false;
            body.riding = None;
        }
    }
    body.on_ground = ground;
}

pub fn move_towards(v: f32, target: f32, max_delta: f32) -> f32 {
    if (target - v).abs() <= max_delta { target } else { v + (target - v).signum() * max_delta }
}
