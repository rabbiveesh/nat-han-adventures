//! Han and the hazards: he plugs spray jets with his body, bats flies away, and sinks in
//! sewage (leaving his big raft). None of it hurts him except the sewage, and none of it is
//! Nat's death.

use bevy::prelude::*;

use super::brain::{HanBrain, HanMode};
use super::{FLY_LINE, HAN_PLUG_LINGER, HAN_RAFT_WIDTH, HAN_SINK_TIME, HanAnim, JET_LINES};
use crate::events::HanSays;
use crate::game::{
    ActiveLevel, Assists, Body, Fly, Han, LevelEntity, MovingPlatform, Pos, PrevPos, Raft,
    SPRAY_HEIGHT, SPRAY_WIDTH, SimClock, Spray,
};
use crate::level::buddy::HALF;
use crate::level::{PlatformKind, TILE, Tile};

/// A spray can's jet as Han's body changes it (on the can's entity). `top`: while Han stands in
/// the jet, it stops at his feet (world y); `linger`: s it stays plugged after he's gone.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Reflect)]
#[reflect(Component)]
pub struct SprayPlug {
    pub top: Option<f32>,
    pub linger: f32,
}

impl SprayPlug {
    /// The jet's danger box (min, max) for a can whose jet starts at `base` (world, the top of
    /// its cell), or `None` while it's plugged.
    pub fn jet(plug: Option<&SprayPlug>, base: Vec2) -> Option<(Vec2, Vec2)> {
        let top = base.y + SPRAY_HEIGHT;
        let top = match plug {
            Some(p) if p.linger > 0.0 && p.top.is_none() => return None,
            Some(p) => p.top.map_or(top, |t| t.min(top)),
            None => top,
        };
        (top > base.y).then(|| (base - Vec2::new(SPRAY_WIDTH / 2.0, 0.0), Vec2::new(base.x + SPRAY_WIDTH / 2.0, top)))
    }
}

/// A fly swarm that bounced off Han: it circles the other way (`dir`), from where it was.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component)]
pub struct FlySpin {
    pub dir: f32,
    /// Phase offset (turns) keeping the fly where it was when it turned.
    pub offset: f32,
    /// Can't bounce again until this is 0 (s).
    pub cooldown: f32,
}

impl Default for FlySpin {
    fn default() -> Self {
        FlySpin { dir: 1.0, offset: 0.0, cooldown: 0.0 }
    }
}

/// Han's big raft (a [`Raft`] [`HAN_RAFT_WIDTH`] tiles wide that floats
/// [`HAN_RAFT_LIFE_FLOOR`](super::HAN_RAFT_LIFE_FLOOR) s × the assist).
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct HanRaft;

/// Seconds between Han's jet/fly quips.
const QUIP_EVERY: f32 = 6.0;

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn han_hazards(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<SimClock>,
    active: Res<ActiveLevel>,
    mut han: Query<(&Pos, &Body, &mut HanBrain, &mut HanAnim), With<Han>>,
    mut sprays: Query<(Entity, &Spray, &Transform, Option<&mut SprayPlug>)>,
    mut flies: Query<(Entity, &Pos, Option<&mut FlySpin>), (With<Fly>, Without<Han>)>,
    mut says: MessageWriter<HanSays>,
    mut quip: Local<f32>,
) {
    let dt = time.delta_secs();
    *quip -= dt;
    let Ok((pos, _body, mut brain, mut anim)) = han.single_mut() else { return };
    anim.wobble = (anim.wobble - dt).max(0.0);
    let present = !matches!(brain.mode, HanMode::Sinking { .. } | HanMode::Gone { .. });
    let (min, max) = (pos.0 - HALF, pos.0 + HALF);

    // Spray jets stop at his body, and sputter a moment after he's past.
    let mut hit = false;
    for (e, spray, tf, plug) in &mut sprays {
        let base = tf.translation.truncate() + Vec2::new(0.0, TILE);
        let inside = present
            && max.x > base.x - SPRAY_WIDTH / 2.0
            && min.x < base.x + SPRAY_WIDTH / 2.0
            && max.y > base.y
            && min.y < base.y + SPRAY_HEIGHT;
        let new = match plug.as_deref() {
            _ if inside => SprayPlug { top: Some(min.y), linger: HAN_PLUG_LINGER },
            Some(p) => SprayPlug { top: None, linger: (p.linger - dt).max(0.0) },
            None => continue,
        };
        hit |= inside && spray.on;
        match plug {
            Some(mut p) => {
                if *p != new {
                    *p = new;
                }
            }
            None => {
                commands.entity(e).insert(new);
            }
        }
    }
    // Flies bounce off him.
    let mut batted = false;
    for (e, fpos, spin) in &mut flies {
        let mut s = spin.as_deref().copied().unwrap_or_default();
        s.cooldown = (s.cooldown - dt).max(0.0);
        let near = present && (fpos.0 - pos.0).abs().cmple(HALF + 4.0).all();
        if near && s.cooldown == 0.0 {
            // Keep the angle where it is: turns·dir + offset is unchanged when dir flips.
            let turns = clock.fly_turns;
            s.offset += 2.0 * s.dir * turns;
            s.dir = -s.dir;
            s.cooldown = 0.5;
            batted = true;
        }
        match spin {
            Some(mut sp) => {
                if *sp != s {
                    *sp = s;
                }
            }
            None if near => {
                commands.entity(e).insert(s);
            }
            None => {}
        }
    }
    if hit || batted {
        anim.wobble = 0.6;
        if *quip <= 0.0 {
            *quip = QUIP_EVERY;
            let line = if batted { FLY_LINE } else { JET_LINES[(clock.steps as usize / 7) % JET_LINES.len()] };
            says.write(HanSays { text: line.to_string() });
        }
    }

    // Sewage: he splats and sinks, leaving his big raft.
    if !present {
        return;
    }
    let level = &active.level;
    let (fmin, fmax) = (min + 2.0, max - 2.0);
    let mut splat: Option<(i32, i32)> = None;
    for c in [(fmin.x / TILE).floor() as i32, (fmax.x / TILE).floor() as i32] {
        let (_, r) = level.cell_at(Vec2::new(0.0, fmin.y));
        if level.tile(c, r) == Tile::Liquid {
            splat.get_or_insert((c, r));
        }
    }
    let Some((c, r)) = splat else { return };
    let dir = if anim.facing_left { -1 } else { 1 };
    brain.mode = HanMode::Sinking { t: HAN_SINK_TIME };
    brain.path.clear();
    brain.exec = None;
    let mut top = r;
    while top > 0 && level.tile(c, top - 1) == Tile::Liquid {
        top -= 1;
    }
    // The raft runs from the splat on in the way he was going (shifted back to stay on the pool).
    let surface = |k: i32| level.tile(k, top) == Tile::Liquid && level.tile(k, top - 1) != Tile::Liquid;
    let w = HAN_RAFT_WIDTH as i32;
    let mut c0 = if dir > 0 { c } else { c - w + 1 };
    for _ in 0..w {
        if (c0..c0 + w).all(surface) {
            break;
        }
        if !surface(c0 + w - 1) {
            c0 -= 1;
        } else {
            c0 += 1;
        }
    }
    let left = level.tile_center(c0.max(0) as usize, top as usize);
    let center = left + Vec2::new((w as f32 - 1.0) * TILE / 2.0, 0.0);
    commands.spawn((
        Name::new("HanRaft"),
        LevelEntity,
        Raft::default(),
        HanRaft,
        MovingPlatform {
            base: center,
            travel: Vec2::ZERO,
            period: 1.0,
            phase: 0.0,
            width: HAN_RAFT_WIDTH,
            kind: PlatformKind::Raft,
        },
        Pos(center),
        PrevPos(center),
        Transform::from_translation(center.extend(1.0)),
    ));
}

/// How long a raft floats: Nat's [`RaftLife`](crate::game::RaftLife), Han's
/// [`HAN_RAFT_LIFE_FLOOR`](super::HAN_RAFT_LIFE_FLOOR) × the assist (never below the floor).
pub fn han_raft_life(assists: &Assists) -> f32 {
    super::HAN_RAFT_LIFE_FLOOR * assists.raft_life_mult.max(1.0)
}
