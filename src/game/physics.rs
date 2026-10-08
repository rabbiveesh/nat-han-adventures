//! Hand-rolled player physics: tile AABB collision (X then Y) against the level grid, one-way
//! tiles and moving platforms; coyote time, jump buffering, variable jump height, the toot.
//! The music's [`Groove`] scales gravity and run speed, and can make landings bounce.

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::groove::{BOUNCE_MIN_SPEED, BOUNCE_RESTITUTION, BOUNCE_SPEED};
use super::{ActiveLevel, GameSet, Groove, MovingPlatform, Player, Pos, PrevPos, tuning::*};
use crate::events::{Jumped, Landed};
use crate::input::Action;
use crate::level::{Level, TILE, Tile};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(FixedUpdate, player_step.in_set(GameSet::Player));
}

/// Landing faster than this (px/s) writes [`Landed`].
pub const LAND_EVENT_SPEED: f32 = 100.0;

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
}

impl Default for PlayerControl {
    fn default() -> Self {
        Self { facing: 1.0, coyote: 0.0, buffer: 0.0, has_toot: true, cut_armed: false, bouncing: false }
    }
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

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn player_step(
    time: Res<Time>,
    active: Res<ActiveLevel>,
    groove: Res<Groove>,
    input: Single<&ActionState<Action>>,
    mut player: Query<
        (&mut Pos, &mut Body, &mut PlayerControl),
        (With<Player>, Without<Dead>, Without<Finished>, Without<MovingPlatform>),
    >,
    platforms: Query<(Entity, &Pos, &PrevPos, &MovingPlatform), Without<Player>>,
    mut jumped: MessageWriter<Jumped>,
    mut landed: MessageWriter<Landed>,
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
    let target = axis * RUN_SPEED * groove.speed_scale;
    let accel = groove.speed_scale
        * if !body.on_ground {
            AIR_ACCEL
        } else if axis == 0.0 || axis * body.vel.x < 0.0 {
            GROUND_DECEL
        } else {
            GROUND_ACCEL
        };
    body.vel.x = move_towards(body.vel.x, target, accel * dt);

    // --- Jumping.
    if body.on_ground || ctl.bouncing {
        ctl.coyote = COYOTE_TIME;
        ctl.has_toot = true;
    } else {
        ctl.coyote -= dt;
    }
    let pressed = input.just_pressed(&Action::Jump);
    if pressed {
        ctl.buffer = JUMP_BUFFER;
    } else {
        ctl.buffer -= dt;
    }
    if ctl.buffer > 0.0 && ctl.coyote > 0.0 {
        body.vel.y = JUMP_SPEED;
        ctl.buffer = 0.0;
        ctl.coyote = 0.0;
        ctl.cut_armed = true;
        ctl.bouncing = false;
        body.on_ground = false;
        body.riding = None;
        jumped.write(Jumped { pos: pos.0, double: false });
    } else if pressed && ctl.has_toot {
        body.vel.y = DOUBLE_JUMP_SPEED;
        ctl.has_toot = false;
        ctl.buffer = 0.0;
        ctl.cut_armed = true;
        jumped.write(Jumped { pos: pos.0, double: true });
    }
    if ctl.cut_armed && !input.pressed(&Action::Jump) && body.vel.y > 0.0 {
        body.vel.y *= JUMP_CUT;
        ctl.cut_armed = false;
    }
    if body.vel.y <= 0.0 {
        ctl.cut_armed = false;
    }

    // --- Gravity.
    body.vel.y = (body.vel.y - GRAVITY * groove.gravity_scale * dt).max(-MAX_FALL * groove.fall_scale());

    // --- Carried by the platform we stood on.
    let carry = body
        .riding
        .and_then(|e| platforms.get(e).ok())
        .map_or(Vec2::ZERO, |(_, p, prev, _)| p.0 - prev.0);

    let half = body.half;
    let was_on_ground = body.on_ground;
    let fall_speed = -body.vel.y;

    // --- X.
    let dx = body.vel.x * dt + carry.x;
    pos.0.x += dx;
    if dx != 0.0 {
        let (xs, ys) = cells(pos.0 - half, pos.0 + half);
        for j in ys {
            for i in xs.clone() {
                if tile_at(level, i, j) != Tile::Solid {
                    continue;
                }
                if dx > 0.0 {
                    pos.0.x = pos.0.x.min(i as f32 * TILE - half.x);
                } else {
                    pos.0.x = pos.0.x.max((i + 1) as f32 * TILE + half.x);
                }
                body.vel.x = 0.0;
            }
        }
    }

    // --- Y.
    let bottom_before = pos.0.y - half.y + carry.y;
    let dy = body.vel.y * dt + carry.y;
    pos.0.y += dy;
    let mut ground = false;
    let mut riding = None;
    if dy != 0.0 {
        let (xs, ys) = cells(pos.0 - half, pos.0 + half);
        for j in ys {
            for i in xs.clone() {
                let tile = tile_at(level, i, j);
                let top = (j + 1) as f32 * TILE;
                match tile {
                    Tile::Solid if dy < 0.0 => {
                        pos.0.y = pos.0.y.max(top + half.y);
                        ground = true;
                    }
                    Tile::Solid => {
                        pos.0.y = pos.0.y.min(j as f32 * TILE - half.y);
                        body.vel.y = body.vel.y.min(0.0);
                    }
                    Tile::OneWay
                        if body.vel.y <= 0.0
                            && bottom_before - carry.y >= top - ONE_WAY_SLACK
                            && pos.0.y - half.y < top =>
                    {
                        pos.0.y = top + half.y;
                        ground = true;
                    }
                    _ => {}
                }
            }
        }
    }
    // Moving platforms (one-way).
    if body.vel.y <= 0.0 {
        for (e, p, prev, plat) in &platforms {
            let half_w = plat.width as f32 * TILE / 2.0;
            if pos.0.x + half.x <= p.0.x - half_w + EPS || pos.0.x - half.x >= p.0.x + half_w - EPS {
                continue;
            }
            let top_now = p.0.y + TILE / 2.0;
            let top_prev = prev.0.y + TILE / 2.0;
            let feet = pos.0.y - half.y;
            let before = if body.riding == Some(e) { bottom_before } else { bottom_before - carry.y };
            if before >= top_now.min(top_prev) - ONE_WAY_SLACK && feet <= top_now + EPS {
                pos.0.y = top_now + half.y;
                ground = true;
                riding = Some(e);
            }
        }
    }
    if ground {
        body.vel.y = 0.0;
        if !was_on_ground && fall_speed > LAND_EVENT_SPEED {
            landed.write(Landed { pos: pos.0, speed: fall_speed });
        }
        ctl.has_toot = true;
        ctl.bouncing = false;
        // The laughing band: spring back up a little (lower each time, until it dies out).
        if groove.bounce && !was_on_ground && fall_speed > BOUNCE_MIN_SPEED {
            body.vel.y = (fall_speed * BOUNCE_RESTITUTION).min(BOUNCE_SPEED);
            ctl.bouncing = true;
            ground = false;
            riding = None;
        }
    }
    body.on_ground = ground;
    body.riding = riding;
}

fn move_towards(v: f32, target: f32, max_delta: f32) -> f32 {
    if (target - v).abs() <= max_delta { target } else { v + (target - v).signum() * max_delta }
}
