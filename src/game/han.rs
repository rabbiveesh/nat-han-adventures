//! Han the plumber follows the player like Tails in Sonic 2: he replays the player's
//! recorded path a fraction of a second later, so he takes the same jumps. The path only
//! grows while the player moves, so he waits a short way behind when they stop.

use std::collections::VecDeque;

use bevy::prelude::*;

use super::physics::{Body, Dead};
use super::{ActiveLevel, GameSet, Han, MovingPlatform, Player, Pos, PrevPos, tuning::*};
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
    /// Downward speed while he's off the replayed path with nothing under his feet (px/s).
    pub fall: f32,
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

/// Half of Han's height: his feet are this far below his position (same box as the player).
const HAN_HALF_HEIGHT: f32 = PLAYER_SIZE.1 / 2.0;
/// Half his width, for standing on ledges.
const HAN_HALF_WIDTH: f32 = PLAYER_SIZE.0 / 2.0;

/// The top of whatever Han's feet would rest on when sinking from `at` by up to `drop` px:
/// solid or one-way tiles, or a moving platform (center + half width). `None` = nothing there.
fn support(level: &Level, platforms: &[(Vec2, f32)], at: Vec2, drop: f32) -> Option<f32> {
    let feet = at.y - HAN_HALF_HEIGHT;
    let reach = feet - drop - 0.5;
    let mut best: Option<f32> = None;
    for x in [at.x - HAN_HALF_WIDTH + 1.0, at.x + HAN_HALF_WIDTH - 1.0] {
        // Tile tops between the feet and how far he sinks this step.
        let (col, row_feet) = level.cell_at(Vec2::new(x, feet + 0.5));
        let (_, row_reach) = level.cell_at(Vec2::new(x, reach));
        for row in row_feet..=row_reach {
            if matches!(level.tile(col, row), Tile::Solid | Tile::OneWay) {
                let top = level.tile_center(0, row.max(0) as usize).y + TILE / 2.0;
                if top <= feet + 0.5 && top >= reach {
                    best = Some(best.map_or(top, |b: f32| b.max(top)));
                }
            }
        }
    }
    for &(p, half_w) in platforms {
        let top = p.y + TILE / 2.0;
        // A platform may have risen since the player stood where Han replays: snap up a little.
        if (at.x - p.x).abs() < half_w + HAN_HALF_WIDTH && top <= feet + 4.0 && top >= reach {
            best = Some(best.map_or(top, |b: f32| b.max(top)));
        }
    }
    best
}

#[allow(clippy::type_complexity)]
fn follow(
    time: Res<Time>,
    active: Res<ActiveLevel>,
    player: Query<(&Pos, &Body, Has<Dead>, &super::PlayerControl), (With<Player>, Without<Han>)>,
    mut han: Query<(&mut Pos, &mut PrevPos, &mut HanTrail, &mut HanMotion, &mut HanAnim), With<Han>>,
    platforms: Query<(&Pos, &MovingPlatform), (Without<Han>, Without<Player>)>,
    mut last_recorded: Local<Option<Vec2>>,
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
        // (Remembered across frames: once he's used up the trail, a player standing still must
        // not keep feeding him their own spot, or he walks into them.)
        let last = trail.0.back().map(|(p, _)| *p).or(*last_recorded);
        if last.is_none_or(|last| last.distance(ppos.0) > 0.5) {
            trail.0.push_back((ppos.0, pbody.on_ground));
            *last_recorded = Some(ppos.0);
        }
        if pos.0.distance(ppos.0) > HAN_POP_DISTANCE {
            pos.0 = behind(&active.level, ppos.0, ctl.facing);
            prev.0 = pos.0;
            trail.0.clear();
        }
    }
    // Keep going past the delay if he'd otherwise freeze mid-jump (the player stopped right
    // after landing): finish the arc down to the ground.
    // (Not while he's falling on his own: that's his physics, not the player's jump.)
    let mid_air = anim.pose == HanPose::Jump && motion.fall == 0.0;
    if trail.0.len() > HAN_DELAY_STEPS || (mid_air && !trail.0.is_empty()) {
        let (target, on_ground) = trail.0.pop_front().unwrap();
        grounded = on_ground;
        let to = target - pos.0;
        let max = HAN_CATCH_UP_SPEED * dt;
        pos.0 += if to.length() > max { to.normalize() * max } else { to };
    }

    // Keep a gap only when both stand at the same height: pushing him sideways while he
    // replays a drop could shove him into a ledge or leave him hanging in the air.
    let dx = pos.0.x - ppos.0.x;
    if grounded && pbody.on_ground && (pos.0.y - ppos.0.y).abs() < 2.0 && dx.abs() < HAN_MIN_GAP {
        let side = if dx.abs() > 0.5 { dx.signum() } else { -ctl.facing };
        let x = ppos.0.x + side * HAN_MIN_GAP;
        let (col, row) = active.level.cell_at(Vec2::new(x, pos.0.y));
        if active.level.tile(col, row) != Tile::Solid {
            pos.0.x = x;
        }
    }

    // He should be standing but nothing's under him (popped in mid-air, or pushed off a
    // ledge): fall like Nat would and land on whatever is below.
    let plats: Vec<(Vec2, f32)> =
        platforms.iter().map(|(p, m)| (p.0, m.width as f32 * TILE / 2.0)).collect();
    let level = &active.level;
    if grounded && support(level, &plats, pos.0, 0.0).is_none() {
        motion.fall = (motion.fall + GRAVITY * dt).min(MAX_FALL);
        let drop = motion.fall * dt;
        match support(level, &plats, pos.0, drop) {
            Some(top) => {
                pos.0.y = top + HAN_HALF_HEIGHT;
                motion.fall = 0.0;
            }
            None => {
                pos.0.y -= drop;
                grounded = false;
            }
        }
    } else {
        motion.fall = 0.0;
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
