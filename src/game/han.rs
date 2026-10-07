//! Han the plumber follows the player like Tails in Sonic 2: he replays the player's
//! recorded path a fraction of a second later, so he takes the same jumps. The path only
//! grows while the player moves, so he waits a short way behind when they stop.

use std::collections::VecDeque;

use bevy::prelude::*;

use super::physics::{Body, Dead};
use super::{ActiveLevel, GameSet, Han, Player, Pos, PrevPos};
use crate::level::{Level, TILE, Tile};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(FixedUpdate, follow.in_set(GameSet::Follow));
}

/// Han replays the player's position this many fixed steps (at 60 Hz) late.
pub const HAN_DELAY_STEPS: usize = 21;
/// Farther than this from the player and he pops right next to them.
pub const HAN_POP_DISTANCE: f32 = 12.0 * TILE;
/// How fast Han can close a gap (px/s), e.g. after a respawn.
pub const HAN_CATCH_UP_SPEED: f32 = 600.0;
/// Han never stands closer than this (horizontally) to the player, so he stays visible even when
/// the player only hops in place.
pub const HAN_MIN_GAP: f32 = 14.0;
/// Speed (px/s) above which he counts as running.
const RUN_THRESHOLD: f32 = 20.0;

/// What Han says when the player splats (every 3rd death).
pub const DEATH_LINES: &[&str] = &[
    "Don't worry, Nat. I've unclogged worse.",
    "Nat! That's not how plumbing works!",
    "Shake it off, Nat. Gravity's just a big drain.",
    "Ooh. I'm puttin' that one on the invoice.",
    "Happens to the best of us. Mostly to you, Nat.",
    "Nat, you go AROUND the pointy stuff.",
];

/// The player's recent path: (position, on ground), oldest first.
#[derive(Component, Debug, Clone, Default)]
pub struct HanTrail(pub VecDeque<(Vec2, bool)>);

/// Han's velocity over the last step (px/s), derived from motion.
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct HanMotion {
    pub vel: Vec2,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
pub enum HanPose {
    #[default]
    Idle,
    Run,
    Jump,
}

/// For visuals: what Han is doing and which way he faces.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Reflect)]
#[reflect(Component)]
pub struct HanAnim {
    pub pose: HanPose,
    pub facing_left: bool,
}

/// A spot just behind `at` (facing `facing`, ±1), or `at` itself if that's inside a wall.
pub fn behind(level: &Level, at: Vec2, facing: f32) -> Vec2 {
    let p = at - Vec2::new(facing * 18.0, 0.0);
    let (col, row) = level.cell_at(p);
    if level.tile(col, row) == Tile::Solid { at } else { p }
}

#[allow(clippy::type_complexity)]
fn follow(
    time: Res<Time>,
    active: Res<ActiveLevel>,
    player: Query<(&Pos, &Body, Has<Dead>, &super::PlayerControl), (With<Player>, Without<Han>)>,
    mut han: Query<(&mut Pos, &mut PrevPos, &mut HanTrail, &mut HanMotion, &mut HanAnim), With<Han>>,
) {
    let dt = time.delta_secs();
    let Ok((ppos, pbody, dead, ctl)) = player.single() else { return };
    let Ok((mut pos, mut prev, mut trail, mut motion, mut anim)) = han.single_mut() else {
        return;
    };

    let mut grounded = true;
    if !dead {
        // Only record actual movement: when the player stands still, Han stops a few steps
        // behind instead of walking right into them (and vanishing behind their sprite).
        let moved = trail.0.back().is_none_or(|(last, _)| last.distance(ppos.0) > 0.5);
        if moved {
            trail.0.push_back((ppos.0, pbody.on_ground));
        }
        if pos.0.distance(ppos.0) > HAN_POP_DISTANCE {
            pos.0 = behind(&active.level, ppos.0, ctl.facing);
            prev.0 = pos.0;
            trail.0.clear();
        }
    }
    // Keep going past the delay if he'd otherwise freeze mid-jump (the player stopped right
    // after landing): finish the arc down to the ground.
    let mid_air = anim.pose == HanPose::Jump;
    if trail.0.len() > HAN_DELAY_STEPS || (mid_air && !trail.0.is_empty()) {
        let (target, on_ground) = trail.0.pop_front().unwrap();
        grounded = on_ground;
        let to = target - pos.0;
        let max = HAN_CATCH_UP_SPEED * dt;
        pos.0 += if to.length() > max { to.normalize() * max } else { to };
    }

    let dx = pos.0.x - ppos.0.x;
    if dx.abs() < HAN_MIN_GAP {
        let side = if dx.abs() > 0.5 { dx.signum() } else { -ctl.facing };
        let x = ppos.0.x + side * HAN_MIN_GAP;
        let (col, row) = active.level.cell_at(Vec2::new(x, pos.0.y));
        if active.level.tile(col, row) != Tile::Solid {
            pos.0.x = x;
        }
    }

    motion.vel = if dt > 0.0 { (pos.0 - prev.0) / dt } else { Vec2::ZERO };
    let pose = if !grounded {
        HanPose::Jump
    } else if motion.vel.x.abs() > RUN_THRESHOLD {
        HanPose::Run
    } else {
        HanPose::Idle
    };
    let facing_left = if motion.vel.x.abs() > 1.0 { motion.vel.x < 0.0 } else { anim.facing_left };
    let new = HanAnim { pose, facing_left };
    if *anim != new {
        *anim = new;
    }
}
