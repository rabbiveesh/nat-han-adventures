//! Moving platforms ping-pong between their grid position and +(dx, dy) tiles with a cosine ease,
//! on their own clock ([`SimClock::platform_time`]): sim time, or lilting pulses in the waltz.

use std::f32::consts::TAU;

use bevy::prelude::*;

use super::{GameSet, MovingPlatform, Pos, SimClock};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(FixedUpdate, move_platforms.in_set(GameSet::World));
}

/// Where `platform` is at sim time `t`.
pub fn platform_pos(platform: &MovingPlatform, t: f32) -> Vec2 {
    let f = (1.0 - (TAU * (t / platform.period + platform.phase)).cos()) / 2.0;
    platform.base + platform.travel * f
}

fn move_platforms(clock: Res<SimClock>, mut q: Query<(&MovingPlatform, &mut Pos)>) {
    for (platform, mut pos) in &mut q {
        pos.0 = platform_pos(platform, clock.platform_time);
    }
}
