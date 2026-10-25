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
//! [`WEAK_BOOST_SPEED`]). Off Han clinging to a wall (a cling shaft) it's a **kick**: the boost
//! and [`KICK_CARRY`] away from the wall. While the band's fired up it's the **running throw** instead:
//! flatter ([`THROW_SPEED`]) and far, Nat keeping [`THROW_CARRY`] on top of his run the way he
//! keeps a platform's speed ([`Body::launch`]), towards where he's facing.
//!
//! # Platform carry
//! Jumping (or stepping) off a moving platform keeps its horizontal speed ([`Body::launch`],
//! at most [`LAUNCH_MAX`]) until Nat lands or hits a wall: air control steers on top of it.
//! In the waltz the platforms glide on ONE, fast, so a jump off one then is a long launch
//! (the waltz's gate, `level::validate`). Han's head gives none, and Han himself keeps none
//! (his AI plans his jumps without it).

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::groove::{
    BOUNCE_MIN_SPEED, BOUNCE_RESTITUTION, BOUNCE_SPEED, JumpedOnOne, WALTZ_ONE_BOOST, WALTZ_ONE_TOOT_SPEED,
};
use super::crushers::{BrickSmashed, CrusherBlock, Sprung, smash};
use super::{ActiveLevel, GameSet, Groove, MovingPlatform, Player, Pos, PrevPos, tuning::*};
use crate::events::{Jumped, Landed};
use crate::input::Action;
use crate::level::{
    BUMPER_LIFT, BUMPER_SPEED, CUSHION_HOLD, cushion_shot, FAN_LIFT, FAN_REACH, FAN_RISE, FIRED_UP_TILES, GIANT_CUSHION_BAT, GIANT_FAN_REACH,
    Level, SPRING_SPEED, TILE, Tile, WALTZ_SPRING_BOOST, WIND_SPEED,
};

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
    /// Horizontal speed (px/s) kept from the moving platform jumped or stepped off, on top of
    /// `vel.x` until landing or a wall (see "Platform carry" above).
    pub launch: f32,
    /// Caught in a wall cushion: the way it will sling the body (+1 right, -1 left; 0 not
    /// caught). Held still in it until `hold` runs out (in the waltz, until ONE) or `fire`.
    pub pad: f32,
    /// How long the cushion still holds (s).
    pub hold: f32,
    /// The aim held as the cushion fires (the controller's: -1 left, +1 right, 0 none).
    pub aim: f32,
    /// Fire the cushion now (a jump while caught).
    pub fire: bool,
}

impl Body {
    pub fn player() -> Self {
        Self {
            vel: Vec2::ZERO,
            half: Vec2::new(PLAYER_SIZE.0, PLAYER_SIZE.1) / 2.0,
            on_ground: false,
            riding: None,
            launch: 0.0,
            pad: 0.0,
            hold: 0.0,
            aim: 0.0,
            fire: false,
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
    /// Leaving it keeps its horizontal speed ([`Contact::launch`]): moving platforms, not
    /// Han's head.
    pub launches: bool,
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
            launches: true,
        }
    }
}

/// Gravity for [`step_body`]: acceleration (px/s², downward) and terminal fall speed, and how
/// the band bends the gadget tiles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fall {
    pub gravity: f32,
    pub max_fall: f32,
    pub tiles: TileMood,
}

impl Fall {
    /// Nat's gravity under `groove`.
    pub fn of(groove: &Groove) -> Self {
        Fall { gravity: GRAVITY * groove.gravity_now(), max_fall: MAX_FALL * groove.fall_scale(), tiles: TileMood::of(groove) }
    }
}

/// How the band bends the gadget tiles, for every body (one band): fired up, belts and wall
/// fans run [`FIRED_UP_TILES`] faster; Giant Steps, every fan blows [`GIANT_FAN_REACH`] tiles
/// far and a ceiling cushion bats you down [`GIANT_CUSHION_BAT`] as fast; the waltz, a trampoline catches you and springs you on the next ONE
/// ([`WALTZ_SPRING_BOOST`] faster), and a cushion or a bumper hit on ONE flings you that much
/// faster too.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileMood {
    /// Belt speed multiplier.
    pub belt: f32,
    /// Wall fans' gust multiplier.
    pub gust: f32,
    /// How far (empty tiles) fans blow.
    pub reach: i32,
    /// Trampolines hold whoever lands on them until ONE (the waltz).
    pub catch: bool,
    /// Spring multiplier now: [`WALTZ_SPRING_BOOST`] on ONE in the waltz, else 1.
    pub spring: f32,
    /// A ceiling cushion's bat down: [`GIANT_CUSHION_BAT`] in Giant Steps, else 1.
    pub bat: f32,
}

impl Default for TileMood {
    fn default() -> Self {
        TileMood { belt: 1.0, gust: 1.0, reach: FAN_REACH, catch: false, spring: 1.0, bat: 1.0 }
    }
}

impl TileMood {
    pub fn of(groove: &Groove) -> Self {
        let fired = if groove.fired_up() { FIRED_UP_TILES } else { 1.0 };
        TileMood {
            belt: fired,
            gust: fired,
            reach: if groove.giant_steps() { GIANT_FAN_REACH } else { FAN_REACH },
            catch: groove.waltz(),
            spring: if groove.on_the_one() { WALTZ_SPRING_BOOST } else { 1.0 },
            bat: if groove.giant_steps() { GIANT_CUSHION_BAT } else { 1.0 },
        }
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
    /// Left a moving platform into the air this step: its horizontal speed (px/s), for the
    /// caller to keep as [`Body::launch`] (Nat does, Han doesn't).
    pub launch: Option<f32>,
    /// Landed on a trampoline and sprang off it ([`SPRING_SPEED`]), or a cushion laid on a
    /// wall slung it away (out of its catch), or one on a ceiling sprang it back down.
    pub sprung: bool,
    /// Bricks (grid col, row) rising into them smashed this step: the caller breaks them
    /// (the body is already inside them).
    pub bricks: [Option<(i32, i32)>; 4],
    /// Ran or flew into a bumper's face and got batted back ([`BUMPER_SPEED`]).
    pub bumped: bool,
    /// Ran or flew into a wall cushion's face and sank into it ([`Body::pad`]).
    pub caught: bool,
}

impl Contact {
    pub fn smashed(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.bricks.iter().flatten().copied()
    }
}

/// Did a landing on the floor tiles `landed` (grid columns and their tiles, the box center's
/// column `center`) spring off a trampoline? Off the one under the middle of the box, or,
/// with nothing under the middle, if every tile underfoot is one. (`level::validate` lands
/// the same way.)
pub fn springs(landed: impl Iterator<Item = (i32, Tile)>, center: i32) -> bool {
    let (mut mid, mut any, mut other) = (None, false, false);
    for (c, t) in landed {
        if c == center {
            mid = Some(t == Tile::Trampoline);
        }
        any |= t == Tile::Trampoline;
        other |= t != Tile::Trampoline;
    }
    mid.unwrap_or(any && !other)
}

/// The conveyor belt's speed (px/s) under a body standing at `pos`: the tile under the middle
/// of its feet, else either corner's. 0 off a belt.
pub fn belt_underfoot(level: &Level, pos: Vec2, half: Vec2) -> f32 {
    let feet = pos.y - half.y - 1.0;
    [pos.x, pos.x - half.x + 0.5, pos.x + half.x - 0.5]
        .into_iter()
        .map(|x| tile_at(level, idx(x), idx(feet)).belt())
        .find(|&b| b != 0.0)
        .unwrap_or(0.0)
}

/// One fixed step of the shared body physics (Nat and Han): gravity, being carried by the
/// carrier stood on, then tile AABB collision X then Y (solid tiles, one-way tiles from above),
/// then the carriers (one-way, from above). `blockers` are extra boxes (min, max) that stop
/// horizontal motion into them (a body already overlapping one isn't pushed); `solids` (the
/// crushers' blocks) stop it too, and a head rising into their underside. Sets
/// `body.on_ground`/`riding` and zeroes the vertical speed on landing; landing effects (events,
/// toot refresh, bounces) are the caller's. [`Body::launch`] moves the body too, and is
/// dropped on landing or against a wall.
///
/// Rising into a brick smashes through it (`Contact::bricks`: the caller breaks them); landing
/// on a trampoline ([`springs`]) springs the body up at [`SPRING_SPEED`] (`Contact::sprung`).
/// A conveyor belt underfoot carries the body (and leaving it keeps its speed: `Contact::launch`),
/// a fan's updraft ([`Level::updraft`]) lifts it, and a bumper's face bats it back
/// (`Contact::bumped`, as a [`Body::launch`]).
#[allow(clippy::too_many_arguments)]
pub fn step_body(
    level: &Level,
    pos: &mut Vec2,
    body: &mut Body,
    fall: Fall,
    carriers: &[Carrier],
    blockers: &[(Vec2, Vec2)],
    solids: &[(Vec2, Vec2)],
    dt: f32,
) -> Contact {
    let mood = fall.tiles;
    // --- Caught in a wall cushion: held still, then slung (aimed by what's held as it fires).
    let mut slung = false;
    if body.pad != 0.0 {
        body.hold -= dt;
        let ready = if mood.catch { mood.spring > 1.0 } else { body.hold <= 0.0 };
        if !ready && !body.fire {
            body.vel = Vec2::ZERO;
            body.launch = 0.0;
            body.on_ground = false;
            body.riding = None;
            return Contact { fall_speed: 0.0, ..Contact::default() };
        }
        let (away, up) = cushion_shot(body.aim * body.pad);
        body.launch = body.pad * away * mood.spring;
        body.vel = Vec2::new(0.0, up * mood.spring);
        body.pad = 0.0;
        body.hold = 0.0;
        body.fire = false;
        slung = true;
    }
    // --- Gravity (and a fan's updraft against it, or a ceiling fan's downdraft with it).
    body.vel.y = (body.vel.y - fall.gravity * dt).max(-fall.max_fall);
    let wind = level.wind_within(idx(pos.x), row_of(level, idx(pos.y)), mood.reach);
    if wind.1 > 0 && body.vel.y < FAN_RISE {
        body.vel.y = (body.vel.y + FAN_LIFT * dt).min(FAN_RISE);
    }
    if wind.1 < 0 && body.vel.y > -FAN_RISE {
        body.vel.y = (body.vel.y - FAN_LIFT * dt).max(-FAN_RISE);
    }
    // A wall fan's gust pushes along (like a belt, in the air too).
    let gust = wind.0 as f32 * WIND_SPEED * mood.gust;

    // --- Carried by what we stood on.
    let carrier = body.riding.and_then(|e| carriers.iter().find(|c| c.id == e));
    let carry = carrier.map_or(Vec2::ZERO, |c| c.carry);

    let half = body.half;
    let was_on_ground = body.on_ground;
    // A conveyor belt underfoot (just jumped off it too: its speed comes along).
    let belt = if body.riding.is_none() && (body.vel.y >= 0.0 || was_on_ground) { belt_underfoot(level, *pos, half) * mood.belt } else { 0.0 };
    let fall_speed = -body.vel.y;
    let mut blocked_x = false;

    // --- X.
    let dx = (body.vel.x + body.launch + belt + gust) * dt + carry.x;
    let x_before = pos.x;
    pos.x += dx;
    let mut bumped = 0.0;
    let mut flung = 0.0;
    if dx != 0.0 {
        let (xs, ys) = cells(*pos - half, *pos + half);
        for j in ys {
            for i in xs.clone() {
                let tile = tile_at(level, i, j);
                if !tile.is_solid() {
                    continue;
                }
                // A bumper's face, run into: batted back.
                if tile.bumper() == -dx.signum() {
                    bumped = tile.bumper();
                }
                // A wall cushion's face: caught in it.
                if tile.fling() == -dx.signum() {
                    flung = tile.fling();
                }
                if dx > 0.0 {
                    pos.x = pos.x.min(i as f32 * TILE - half.x);
                } else {
                    pos.x = pos.x.max((i + 1) as f32 * TILE + half.x);
                }
                body.vel.x = 0.0;
                body.launch = 0.0;
                blocked_x = true;
            }
        }
        for &(min, max) in blockers.iter().chain(solids) {
            let overlaps = |x: f32| x + half.x > min.x && x - half.x < max.x;
            let y_overlaps = pos.y + half.y > min.y + EPS && pos.y - half.y < max.y - EPS;
            if !y_overlaps || overlaps(x_before) || !overlaps(pos.x) {
                continue;
            }
            pos.x = if dx > 0.0 { min.x - half.x } else { max.x + half.x };
            body.vel.x = 0.0;
            body.launch = 0.0;
            blocked_x = true;
        }
    }
    if bumped != 0.0 {
        body.launch = bumped * BUMPER_SPEED * mood.spring;
        body.vel.y = body.vel.y.max(BUMPER_LIFT * mood.spring);
        body.on_ground = false;
    }
    if flung != 0.0 && !slung {
        body.pad = flung;
        body.hold = CUSHION_HOLD;
        body.aim = 0.0;
        body.fire = false;
        body.vel = Vec2::ZERO;
        body.launch = 0.0;
        body.on_ground = false;
        body.riding = None;
        return Contact { was_on_ground, fall_speed, blocked_x, caught: true, ..Contact::default() };
    }
    // A ceiling cushion bumped into: back down.
    let mut bonked = false;

    // --- Y.
    let bottom_before = pos.y - half.y + carry.y;
    let dy = body.vel.y * dt + carry.y;
    pos.y += dy;
    let mut ground = false;
    let mut riding = None;
    let mut bricks = [None; 4];
    let mut landed: [Option<(i32, Tile)>; 4] = [None; 4];
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
                        if let Some(slot) = landed.iter_mut().find(|s| s.is_none()) {
                            *slot = Some((i, t));
                        }
                    }
                    Tile::Brick => {
                        // Smashed through from below: the caller breaks it.
                        if let Some(slot) = bricks.iter_mut().find(|s| s.is_none()) {
                            *slot = Some((i, row_of(level, j)));
                        }
                    }
                    t if t.is_solid() => {
                        pos.y = pos.y.min(j as f32 * TILE - half.y);
                        body.vel.y = body.vel.y.min(0.0);
                        bonked |= t == Tile::CushionDown;
                    }
                    t if t.is_one_way()
                        && body.vel.y <= 0.0
                        && bottom_before - carry.y >= top - ONE_WAY_SLACK
                        && pos.y - half.y < top =>
                    {
                        pos.y = top + half.y;
                        ground = true;
                        if let Some(slot) = landed.iter_mut().find(|s| s.is_none()) {
                            *slot = Some((i, t));
                        }
                    }
                    _ => {}
                }
            }
        }
        // A head rising into a solid box's underside.
        if dy > 0.0 {
            let head_before = pos.y - dy + half.y;
            for &(min, max) in solids {
                let x_overlaps = pos.x + half.x > min.x + EPS && pos.x - half.x < max.x - EPS;
                if x_overlaps && head_before <= min.y + ONE_WAY_SLACK && pos.y + half.y > min.y {
                    pos.y = min.y - half.y;
                    body.vel.y = body.vel.y.min(0.0);
                }
            }
        }
    }
    if bonked {
        body.vel.y = -SPRING_SPEED * mood.spring * mood.bat;
    }
    // A trampoline underfoot: up we go (in the waltz, it holds you till ONE).
    let sprung = ground && springs(landed.iter().flatten().copied(), idx(pos.x)) && (!mood.catch || mood.spring > 1.0);
    if sprung {
        body.vel.y = SPRING_SPEED * mood.spring;
        body.launch = 0.0;
        body.on_ground = false;
        body.riding = None;
        return Contact { was_on_ground, fall_speed, blocked_x, sprung, bricks, ..Contact::default() };
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
        body.launch = 0.0;
    }
    // (Slung by a cushion: its launch, not the belt's or the platform's.)
    let launch = (!ground && !blocked_x && !slung)
        .then_some(carrier)
        .flatten()
        .filter(|c| c.launches)
        .map(|c| c.carry.x / dt)
        .or((!ground && !blocked_x && !slung && belt != 0.0).then_some(belt));
    body.on_ground = ground;
    body.riding = riding;
    Contact {
        ground,
        riding,
        was_on_ground,
        fall_speed,
        blocked_x,
        launch,
        sprung: slung || bonked,
        bricks,
        bumped: bumped != 0.0,
        caught: false,
    }
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
    /// Horizontal speed a boost off him adds, kept like a platform's (px/s, signed; 0 for
    /// none): clinging to a wall (a cling shaft, nervous) [`KICK_CARRY`] away from it; on the
    /// trapeze (the waltz) his swing's speed.
    pub kick: f32,
}

/// Nat jumped off Han's head: the plunger boost (`weak`: the feeble one of a band zone;
/// `throw`: the fired-up band's running throw). Not a [`Jumped`] (the band doesn't count it).
#[derive(Message, Debug, Clone, Copy)]
pub struct HanBoosted {
    pub pos: Vec2,
    pub weak: bool,
    pub throw: bool,
    /// The laughing band's boing: Nat landed on him and sprang off, no jump needed.
    pub boing: bool,
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
/// tiles with ideal timing, ≥ 9.3 for a human. In Giant Steps' low gravity it's the
/// *mega-boost*: 560²/1820 ≈ 172 px over Han's head, ≈ 15 tiles over his floor with the toot.
/// A *mega wall* is 11 tiles: out of Giant Steps' reach (a perfect GS double jump tops out at
/// ~8.7 tiles) and the plain boost's, a comfortable mega-boost (by a mega wall Han's boost is
/// full strength only while Giant Steps plays: `Level::weak_boost`).
pub const BOOST_SPEED: f32 = 560.0;

/// Upward speed of the fired-up band's running throw off Han's head (px/s): flatter than the
/// boost, about a jump...
pub const THROW_SPEED: f32 = 380.0;
/// ...and the horizontal speed it adds on top of Nat's run (px/s, kept like a platform's speed
/// until he lands or hits a wall: [`Body::launch`]).
pub const THROW_CARRY: f32 = 120.0;

/// The horizontal speed a kick off Han clinging to a wall adds, away from it (px/s, kept like a
/// platform's speed until Nat lands or hits a wall: [`Body::launch`]).
pub const KICK_CARRY: f32 = 90.0;

/// Han's head as a carrier, if it's solid (`pos` is his box center).
pub fn han_carrier(id: Entity, pos: Vec2, prev: Vec2, head: &HanHead) -> Option<Carrier> {
    head.solid.then(|| Carrier {
        id,
        x: pos.x,
        half_w: PLAYER_SIZE.0 / 2.0,
        top: pos.y + PLAYER_SIZE.1 / 2.0,
        prev_top: prev.y + PLAYER_SIZE.1 / 2.0,
        carry: pos - prev,
        launches: false,
    })
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn player_step(
    time: Res<Time>,
    mut active: ResMut<ActiveLevel>,
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
    crushers: Query<(Entity, &CrusherBlock, &Pos, &PrevPos), (Without<Player>, Without<MovingPlatform>)>,
    mut smashed: MessageWriter<BrickSmashed>,
    mut sprung: MessageWriter<Sprung>,
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

    // --- Caught in a wall cushion: aiming it (Left/Right), and a jump fires it.
    let caught = body.pad != 0.0;
    if caught {
        body.aim = axis;
        body.fire |= input.just_pressed(&Action::Jump);
    }

    // --- Jumping.
    if caught {
        ctl.buffer = 0.0;
        ctl.coyote = 0.0;
    } else if slipping {
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
    let pressed = input.just_pressed(&Action::Jump) && !caught;
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
            boosted.write(HanBoosted { pos: pos.0, weak: true, throw: false, boing: false });
        } else if on_han.is_some_and(|h| h.boost) && groove.fired_up() {
            // The running throw: flat and far, and the toot's fresh again.
            body.vel.y = THROW_SPEED;
            body.launch = ctl.facing * THROW_CARRY;
            ctl.has_toot = true;
            ctl.weak_toot = false;
            boosted.write(HanBoosted { pos: pos.0, weak: false, throw: true, boing: false });
        } else if on_han.is_some_and(|h| h.boost) {
            // The plunger boost: way up, and the toot's fresh again (off Han clinging to a wall,
            // a kick away from it too; off his trapeze, his swing).
            body.vel.y = BOOST_SPEED;
            let kick = on_han.map_or(0.0, |h| h.kick);
            if kick != 0.0 {
                body.launch = kick;
            }
            ctl.has_toot = true;
            ctl.weak_toot = false;
            boosted.write(HanBoosted { pos: pos.0, weak: false, throw: false, boing: false });
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
        // Still riding for this step: `step_body` carries the jump off with the platform and
        // hands over its speed.
        body.on_ground = false;
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
    let mut solids = Vec::new();
    for (e, block, p, prev) in &crushers {
        carriers.push(block.carrier(e, p.0, prev.0));
        solids.push(block.bounds(p.0));
    }
    let contact = step_body(level, &mut pos.0, &mut body, Fall::of(&groove), &carriers, &blockers, &solids, dt);
    if contact.bricks.iter().any(Option::is_some) {
        smash(&mut active, contact.smashed(), &mut smashed);
    }
    let level = &active.level;
    if contact.sprung || contact.bumped {
        // Off a trampoline (or a bumper): way up, and the toot's back.
        ctl.has_toot = true;
        ctl.weak_toot = false;
        ctl.cut_armed = false;
        ctl.bouncing = false;
        ctl.coyote = 0.0;
        sprung.write(Sprung { pos: pos.0, han: false });
    }
    if let Some(v) = contact.launch {
        body.launch = v.clamp(-LAUNCH_MAX, LAUNCH_MAX);
    }
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
        let was_bouncing = ctl.bouncing;
        ctl.bouncing = false;
        ctl.weak_ground = contact.riding.and_then(|e| han.get(e).ok()).is_some_and(|(_, _, _, h)| h.weak);
        // The laughing band's boing: dropping onto Han's head (where he'd boost) springs Nat
        // off it at the full boost, no jump needed (fired up, a jump off him is the throw).
        // Once: coming back down from a bounce, it's the plain bounce.
        let springy = contact.riding.and_then(|e| han.get(e).ok()).map(|(_, _, _, h)| *h).filter(|h| h.boost && !h.weak);
        if let Some(h) = springy
            && groove.bouncy()
            && !groove.fired_up()
            && !was_bouncing
            && !contact.was_on_ground
            && contact.fall_speed > BOUNCE_MIN_SPEED
        {
            body.vel.y = BOOST_SPEED;
            if h.kick != 0.0 {
                body.launch = h.kick;
            }
            ctl.bouncing = true;
            boosted.write(HanBoosted { pos: pos.0, weak: false, throw: false, boing: true });
            body.on_ground = false;
            body.riding = None;
            return;
        }
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
