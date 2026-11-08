//! Han and the hazards: he plugs spray jets with his body, bats flies away, and sinks in
//! sewage (leaving his big raft). None of it hurts him except the sewage, and none of it is
//! Nat's death.

use bevy::prelude::*;

use super::brain::{HanBrain, HanMode};
use super::{
    FLY_LINE, HAN_PLUG_LINGER, HAN_RAFT_WIDTH, HAN_SINK_TIME, HAN_SQUASH_RESPAWN, HanAnim,
    JET_LINES, SQUASH_LINE,
};
use leafwing_input_manager::prelude::*;

use crate::events::{HanSays, NuggetCollected};
use crate::game::crushers::{CrusherBlock, squashed};
use crate::game::{
    ActiveLevel, Assists, Body, CAN_HEIGHT, CAN_WIDTH, Dead, Fly, Han, LevelEntity, MovingPlatform,
    Player, Pos, PrevPos, Raft, SPRAY_HEIGHT, SPRAY_WIDTH, SimClock, Spray,
};
use crate::input::Action;
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
        (top > base.y).then(|| {
            (
                base - Vec2::new(SPRAY_WIDTH / 2.0, 0.0),
                Vec2::new(base.x + SPRAY_WIDTH / 2.0, top),
            )
        })
    }

    /// The can's body box (min, max) for a can standing at `floor` (world, the bottom-center
    /// of its cell), while its jet is live at the nozzle: not while it's plugged, nor while
    /// Han stands over it (the jet then stops at his feet, at or below the nozzle).
    pub fn can(plug: Option<&SprayPlug>, floor: Vec2) -> Option<(Vec2, Vec2)> {
        Self::jet(plug, floor + Vec2::new(0.0, TILE))?;
        Some((
            floor - Vec2::new(CAN_WIDTH / 2.0, 0.0),
            floor + Vec2::new(CAN_WIDTH / 2.0, CAN_HEIGHT),
        ))
    }

    /// Everything that's deadly about a firing can standing at `floor`: its body and its jet.
    pub fn danger(plug: Option<&SprayPlug>, floor: Vec2) -> [Option<(Vec2, Vec2)>; 2] {
        [
            Self::can(plug, floor),
            Self::jet(plug, floor + Vec2::new(0.0, TILE)),
        ]
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
        FlySpin {
            dir: 1.0,
            offset: 0.0,
            cooldown: 0.0,
        }
    }
}

/// Han's big raft (a [`Raft`] [`HAN_RAFT_WIDTH`] tiles wide that floats
/// [`HAN_RAFT_LIFE_FLOOR`](super::HAN_RAFT_LIFE_FLOOR) s × the assist).
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct HanRaft;

/// Han the pool float: as Nat runs up to a pool, Han cannonballs in and floats on his back,
/// creeping across ([`FLOAT_CREEP`]) until Nat hops aboard. Then Nat steers him like a little
/// boat: Left/Right paddle him either way ([`FLOAT_SPEED`]), nothing held and he stops; while Nat
/// hops (teeny hops: the pools have low spikes) he kicks to stay under him. He can
/// only kick for [`FLOAT_FUEL`] s; nuggets grabbed aboard top him up ([`FLOAT_REFUEL`]). Low, he
/// shivers; dry, he sinks (GLUB) and whoever's on him goes in. Nobody aboard at the far
/// bank (or he was left behind) and he climbs out after [`FLOAT_BEACHED`] s; he's back by
/// parachute and cannonballs in again on the next run-up.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct HanFloat {
    /// The way he cannonballed in (+1 right, -1 left): where he creeps before Nat's aboard.
    pub dir: f32,
    /// Velocity now (px/s, + right).
    pub vel: f32,
    /// The surface row of the pool.
    pub row: i32,
    /// The surface height (world y of the raft's center when afloat).
    pub surface: f32,
    /// Nat has been aboard.
    pub boarded: bool,
    /// Kicks left (s).
    pub fuel: f32,
    /// How far he's sunk (px).
    pub sunk: f32,
    /// Time with nobody aboard since Nat left him, or at the far bank unboarded (s).
    pub idle: f32,
}

impl HanFloat {
    pub fn new(dir: f32, row: i32, surface: f32) -> Self {
        HanFloat {
            dir,
            vel: 0.0,
            row,
            surface,
            boarded: false,
            fuel: FLOAT_FUEL,
            sunk: 0.0,
            idle: 0.0,
        }
    }
}

/// Drift before Nat's aboard (px/s): slow enough to hop on.
pub const FLOAT_CREEP: f32 = 24.0;
/// Paddling speed with Nat aboard (px/s).
pub const FLOAT_SPEED: f32 = 100.0;
/// Nat hopping within this far (px) across of him: he kicks to stay under Nat, up to
/// [`FLOAT_CHASE`] px/s.
pub const FLOAT_TRACK: f32 = 2.0 * TILE;
pub const FLOAT_CHASE: f32 = 170.0;
/// How quickly the float changes speed (px/s²).
pub const FLOAT_ACCEL: f32 = 500.0;
/// Kicks in a full float (s; it only drains with Nat aboard), and per nugget grabbed aboard.
pub const FLOAT_FUEL: f32 = 8.0;
pub const FLOAT_REFUEL: f32 = 2.5;
/// Below this much fuel (s) he shivers (a warning); dry, he sinks at [`FLOAT_SINK`] px/s and
/// is gone at [`FLOAT_GONE`] px under.
pub const FLOAT_LOW: f32 = 2.0;
pub const FLOAT_SINK: f32 = 24.0;
pub const FLOAT_GONE: f32 = 14.0;
/// Nobody aboard this long (s) and Han climbs out.
pub const FLOAT_BEACHED: f32 = 1.5;
/// Han's bark as he dives in, and as he sinks.
pub const CANNONBALL_LINE: &str = "Cannonball!";
pub const GLUB_LINE: &str = "Glub!";

/// Han's floats: steered by Nat aboard, fuelled by nuggets, and sunk when dry.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(in crate::game) fn drift_floats(
    mut commands: Commands,
    time: Res<Time>,
    active: Res<ActiveLevel>,
    input: Option<Single<&ActionState<Action>>>,
    player: Query<(&Pos, &Body), (With<Player>, Without<Dead>, Without<HanFloat>)>,
    mut floats: Query<(Entity, &mut HanFloat, &mut MovingPlatform, &mut Pos)>,
    mut han: Query<&mut HanBrain, With<Han>>,
    mut nuggets: MessageReader<NuggetCollected>,
    mut says: MessageWriter<HanSays>,
) {
    let dt = time.delta_secs();
    let grabbed = nuggets.read().count() as f32;
    if dt <= 0.0 {
        return;
    }
    let level = &active.level;
    let axis = input.map_or(0.0, |i| {
        i.pressed(&Action::Right) as i32 as f32 - i.pressed(&Action::Left) as i32 as f32
    });
    let riding = player.single().ok().and_then(|(_, b)| b.riding);
    // Nat in the air (hopping off him): where, and how fast across.
    let nat = player
        .single()
        .ok()
        .filter(|(_, b)| !b.on_ground)
        .map(|(p, b)| (p.0.x, b.vel.x));
    for (e, mut f, mut p, mut pos) in &mut floats {
        let aboard = riding == Some(e);
        f.boarded |= aboard;
        if aboard {
            f.fuel = (f.fuel + grabbed * FLOAT_REFUEL).min(FLOAT_FUEL);
        }
        let dry = f.fuel <= 0.0;
        let target = if dry {
            0.0
        } else if aboard {
            axis * FLOAT_SPEED
        } else if let Some((nx, nvx)) = nat.filter(|_| f.boarded) {
            // Nat's hopping (over spikes, for a nugget): Han kicks to stay under him.
            let dx = nx - pos.0.x;
            if dx.abs() < FLOAT_TRACK {
                (nvx + dx * 4.0).clamp(-FLOAT_CHASE, FLOAT_CHASE)
            } else {
                0.0
            }
        } else if f.boarded {
            0.0
        } else {
            f.dir * FLOAT_CREEP
        };
        f.vel = crate::game::physics::move_towards(f.vel, target, FLOAT_ACCEL * dt);
        // Over the pool only: he stops at either bank.
        let half = p.width as f32 * TILE / 2.0;
        let mut x = p.base.x + f.vel * dt;
        let mut at_bank = false;
        for side in [-1.0f32, 1.0] {
            let edge = x + side * half;
            let col = ((edge - side * 0.01) / TILE).floor() as i32;
            if level.tile(col, f.row) != Tile::Liquid {
                let bank = if side > 0.0 {
                    col as f32 * TILE
                } else {
                    (col + 1) as f32 * TILE
                };
                x = bank - side * half;
                if f.vel * side > 0.0 {
                    f.vel = 0.0;
                }
                at_bank |= f.vel == 0.0 && (target * side > 0.0 || target == 0.0);
            }
        }
        p.base.x = x;
        // Kicking costs fuel while Nat's aboard; low, he settles; dry, he sinks.
        if f.boarded && aboard && !dry {
            f.fuel -= dt;
            if f.fuel <= 0.0 {
                says.write(HanSays {
                    text: GLUB_LINE.to_string(),
                });
            }
        }
        // (Low on kicks, he shivers: `visuals`. Any lower in the muck and Nat's feet are in it.)
        if dry {
            f.sunk += FLOAT_SINK * dt;
        }
        pos.0 = Vec2::new(x, f.surface - f.sunk);
        p.base = pos.0;
        // Nobody aboard: once Nat's left him, or he's crossed without Nat, he climbs out.
        let left_alone = !aboard && (f.boarded || (at_bank && f.vel == 0.0));
        f.idle = if left_alone { f.idle + dt } else { 0.0 };
        if f.sunk >= FLOAT_GONE || f.idle >= FLOAT_BEACHED {
            commands.entity(e).despawn();
            continue;
        }
        // Han's out of the picture while he's the float.
        if let Ok(mut brain) = han.single_mut()
            && let HanMode::Gone { t } = brain.mode
        {
            brain.mode = HanMode::Gone { t: t.max(0.5) };
        }
    }
}

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
    crushers: Query<(&CrusherBlock, &Pos), Without<Han>>,
    nav: Option<Res<super::brain::HanNav>>,
) {
    let side_b = nav.is_some_and(|n| n.side_b);
    let dt = time.delta_secs();
    *quip -= dt;
    let Ok((pos, _body, mut brain, mut anim)) = han.single_mut() else {
        return;
    };
    anim.wobble = (anim.wobble - dt).max(0.0);
    let present = !matches!(brain.mode, HanMode::Sinking { .. } | HanMode::Gone { .. });
    let (min, max) = (pos.0 - HALF, pos.0 + HALF);

    // Spray jets stop at his body, and sputter a moment after he's past.
    let mut hit = false;
    for (e, spray, tf, plug) in &mut sprays {
        // His body over the can or in its jet (the can is no wider than the jet).
        let floor = tf.translation.truncate();
        let base = floor + Vec2::new(0.0, TILE);
        let inside = present
            && max.x > base.x - SPRAY_WIDTH.max(CAN_WIDTH) / 2.0
            && min.x < base.x + SPRAY_WIDTH.max(CAN_WIDTH) / 2.0
            && max.y > floor.y
            && min.y < base.y + SPRAY_HEIGHT;
        // Escorting Nat through: the jets he's plugged stay plugged.
        let escorting = matches!(brain.mode, HanMode::Ahead { .. });
        let new = match plug.as_deref() {
            _ if inside => SprayPlug {
                top: Some(min.y),
                linger: HAN_PLUG_LINGER,
            },
            Some(p) if escorting && p.linger > 0.0 => SprayPlug {
                top: None,
                linger: HAN_PLUG_LINGER,
            },
            Some(p) => SprayPlug {
                top: None,
                linger: (p.linger - dt).max(0.0),
            },
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
            let line = if batted {
                FLY_LINE
            } else {
                JET_LINES[(clock.steps as usize / 7) % JET_LINES.len()]
            };
            says.write(HanSays {
                text: line.to_string(),
            });
        }
    }

    if !present {
        return;
    }
    // A crusher: flat as a pancake, and back by parachute.
    if crushers
        .iter()
        .any(|(b, p)| squashed(pos.0, HALF, b.bounds(p.0)))
    {
        brain.mode = HanMode::Gone {
            t: HAN_SQUASH_RESPAWN,
        };
        brain.path.clear();
        brain.exec = None;
        says.write(HanSays {
            text: SQUASH_LINE.to_string(),
        });
        return;
    }

    // Sewage: he splats and sinks, leaving his big raft.
    let level = &active.level;
    let (fmin, fmax) = (min + 2.0, max - 2.0);
    let mut splat: Option<(i32, i32)> = None;
    for c in [
        (fmin.x / TILE).floor() as i32,
        (fmax.x / TILE).floor() as i32,
    ] {
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
    let surface =
        |k: i32| level.tile(k, top) == Tile::Liquid && level.tile(k, top - 1) != Tile::Liquid;
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
    let raft = commands
        .spawn((
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
        ))
        .id();
    if !side_b {
        commands
            .entity(raft)
            .insert(HanFloat::new(dir as f32, top, center.y));
    }
}

/// How long a raft floats: Nat's [`RaftLife`](crate::game::RaftLife), Han's
/// [`HAN_RAFT_LIFE_FLOOR`](super::HAN_RAFT_LIFE_FLOOR) × the assist (never below the floor).
pub fn han_raft_life(assists: &Assists) -> f32 {
    super::HAN_RAFT_LIFE_FLOOR * assists.raft_life_mult.max(1.0)
}
