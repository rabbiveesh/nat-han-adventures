//! Side B's encore: what happens at B10's goal instead of the cheer.
//!
//! Han's found Nat's hat, and the band takes an encore: the same lap as the story's (free
//! calls, Down or Jump; the band all out, a shout chorus a key up) with Han dancing on the
//! stage (struts to the beat, hops on every call, floats in Giant Steps, rolls when melting).
//! On the Basie ending's last chord he hops up on the throne for a celebratory plunge: GLUG,
//! GLUG, and he yanks the plunger out with Nat stuck to the cup. "NAT!!" Nat slips off, and
//! the throne slurps him back down, deeper. "...oops." To be continued (into NAT ALONE).

use bevy::prelude::*;

use super::{CONTINUED_SECS, ENDING_TIMEOUT, Encore, EncorePhase, Finale, LAP_SECS, THRONE_OFFSET};
use crate::events::{EncoreCue, FinalHit, HanSays};
use crate::game::physics::{Body, Finished};
use crate::game::{ActiveLevel, Groove, Han, HanAnim, HanPose, LevelRun, Nudge, Player, Pos, cell_floor};
use crate::level::{TILE, Tile};
use crate::state::AppState;

/// Han's strut across the stage (px/s, before the music's run speed).
const STRUT: f32 = 3.5 * TILE;
/// How high he hops to the beat (px), and in Giant Steps (over two beats).
const BEAT_HOP: f32 = 5.0;
const GIANT_HOP: f32 = 24.0;
/// A call kicks him up this high (px) for this long (s).
const CALL_HOP: f32 = 18.0;
const CALL_HOP_SECS: f32 = 0.6;
/// The hop up onto the throne (s), and how high it arcs (px).
pub const HOP_SECS: f32 = 0.7;
const HOP_ARC: f32 = 30.0;
/// The plunges: how many, and each one's length (s).
pub const PLUNGES: u32 = 4;
pub const PLUNGE_SECS: f32 = 0.4;
/// Nat stuck on the cup, wiggling (s).
pub const YANKED_SECS: f32 = 1.4;
/// Nat slides off the cup into the bowl (s), then the throne slurps him down (s).
const SLIDE_SECS: f32 = 0.35;
pub const SUCK_SECS: f32 = 2.0;
/// Where Han stands to plunge, from the throne's bottom-center: on the seat's far side, facing
/// the bowl (his plunger's cup over it).
const SEAT: Vec2 = Vec2::new(13.0, 19.0);
/// The yanked plunger's cup, from Han's feet (facing left): up high in front of him.
const CUP: Vec2 = Vec2::new(-6.0, 20.0);
/// How deep Nat goes (px): deeper than the story's flush.
const SINK: f32 = 22.0;

/// Han's dance in the lap.
#[derive(Debug, Clone, Copy, Default, Reflect)]
pub struct Dance {
    x: f32,
    dir: f32,
    /// Seconds since the last call (its hop), if any.
    since_call: Option<f32>,
    /// Where the hop onto the throne started.
    from: Vec2,
}

impl Dance {
    pub(super) fn called(&mut self) {
        self.since_call = Some(0.0);
        // (A turn on every call: he spins round to the band.)
        self.dir = -self.dir;
    }
}

/// Is this the last Side B level (its goal starts the encore)? Not a free play room.
pub fn is_finale(level: usize, room: bool) -> bool {
    !room && level + 1 == crate::game::sideb::SIDE_B_COUNT
}

/// Start Han's encore at the goal (`han`: where he is).
pub fn start(commands: &mut Commands, active: &ActiveLevel, run: &LevelRun, han: Vec2, cues: &mut MessageWriter<EncoreCue>) {
    let level = &active.level;
    let (gc, gr) = level.goal;
    let goal = cell_floor(level, gc, gr);
    // The stage: the goal's floor, as far left as it runs with headroom (no drips), to the
    // level's end.
    let floor_row = gr as i32 + 1;
    let clear = |c: i32| level.tile(c, gr as i32) == Tile::Empty && level.tile(c, gr as i32 - 1) == Tile::Empty;
    let mut left = gc as i32;
    while left > 0 && level.tile(left - 1, floor_row).is_solid() && clear(left - 1) {
        left -= 1;
    }
    let half = Body::player().half.x;
    commands.insert_resource(Encore {
        finale: Finale::SideB,
        phase: EncorePhase::Lap,
        t: 0.0,
        calls: 0,
        stage: (left as f32 * TILE + half, level.size_px().x - half),
        throne: goal + Vec2::new(THRONE_OFFSET, 0.0),
        time_at_goal: run.time,
        knock: None,
        dance: Dance { x: han.x, dir: 1.0, since_call: None, from: han },
    });
    cues.write(EncoreCue::Start);
}

impl Encore {
    /// Where Han stands to plunge (his feet).
    pub fn seat(&self) -> Vec2 {
        self.throne + SEAT
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn run(
    time: Res<Time>,
    groove: Res<Groove>,
    mut encore: ResMut<Encore>,
    mut hits: MessageReader<FinalHit>,
    mut han: Query<(&mut Pos, &mut Body, &mut HanAnim), (With<Han>, Without<Player>)>,
    mut nat: Query<(Entity, &mut Pos, &mut Transform, Has<Finished>), (With<Player>, Without<Han>)>,
    mut commands: Commands,
    mut cues: MessageWriter<EncoreCue>,
    mut says: MessageWriter<HanSays>,
    mut next: ResMut<NextState<AppState>>,
) {
    let dt = time.delta_secs();
    let hit = hits.read().count() > 0;
    let Ok((mut hp, mut hb, mut anim)) = han.single_mut() else { return };
    let e = &mut *encore;
    e.t += dt;
    hb.vel = Vec2::ZERO;
    anim.hidden = false;
    anim.tremble = false;
    // (Where his center is when his feet are at `f`.)
    let stand = |b: &Body, f: Vec2| f + Vec2::new(0.0, b.half.y);
    let floor = e.throne.y;
    let seat = e.seat();
    match e.phase {
        EncorePhase::Lap | EncorePhase::Ending => {
            let d = &mut e.dance;
            if d.dir == 0.0 {
                d.dir = 1.0;
            }
            d.x += d.dir * STRUT * groove.run_scale() * dt;
            if d.x > e.stage.1 || d.x < e.stage.0 {
                d.x = d.x.clamp(e.stage.0, e.stage.1);
                d.dir = -d.dir;
            }
            let c = groove.clock;
            let melting = groove.nudge() == Some(Nudge::Melting);
            let beat = if groove.giant_steps() {
                let p = ((c.beat % 2) as f32 + c.phase) / 2.0;
                GIANT_HOP * 4.0 * p * (1.0 - p)
            } else if melting {
                0.0
            } else {
                BEAT_HOP * 4.0 * c.phase * (1.0 - c.phase)
            };
            let kick = d.since_call.map_or(0.0, |s| {
                let s = (s / CALL_HOP_SECS).min(1.0);
                CALL_HOP * 4.0 * s * (1.0 - s)
            });
            if let Some(s) = d.since_call.as_mut() {
                *s += dt;
            }
            let hop = beat + kick;
            hp.0 = stand(&hb, Vec2::new(d.x, floor + hop));
            anim.facing_left = d.dir < 0.0;
            anim.pose = if melting {
                HanPose::Roll
            } else if hop > 3.0 && groove.giant_steps() {
                HanPose::Paddle
            } else if hop > 3.0 {
                HanPose::Jump
            } else {
                HanPose::Run
            };
            if e.phase == EncorePhase::Lap && e.t >= LAP_SECS {
                e.phase = EncorePhase::Ending;
                e.t = 0.0;
                cues.write(EncoreCue::End);
            } else if e.phase == EncorePhase::Ending && (hit || e.t >= ENDING_TIMEOUT) {
                // The last chord: up on the throne for a celebratory plunge.
                e.dance.from = hp.0;
                e.phase = EncorePhase::Hop;
                e.t = 0.0;
                cues.write(EncoreCue::Knock { pos: hp.0 });
                says.write(HanSays { text: "Plunge!".into() });
            }
        }
        EncorePhase::Hop => {
            let s = (e.t / HOP_SECS).min(1.0);
            let to = stand(&hb, seat);
            hp.0 = e.dance.from.lerp(to, s) + Vec2::new(0.0, HOP_ARC * 4.0 * s * (1.0 - s));
            anim.facing_left = s >= 0.5 || to.x < e.dance.from.x;
            anim.pose = HanPose::Jump;
            if s >= 1.0 {
                e.phase = EncorePhase::Plunging;
                e.t = 0.0;
                cues.write(EncoreCue::Plunge { pos: e.bowl() });
            }
        }
        EncorePhase::Plunging => {
            hp.0 = stand(&hb, seat);
            anim.facing_left = true;
            anim.pose = HanPose::Plunge;
            let n = (e.t / PLUNGE_SECS) as u32;
            let was = ((e.t - dt) / PLUNGE_SECS).max(0.0) as u32;
            if n >= PLUNGES {
                // GLUG... POP: up comes the plunger, and Nat with it.
                e.phase = EncorePhase::Yanked;
                e.t = 0.0;
                cues.write(EncoreCue::Pop { pos: e.bowl() });
                says.write(HanSays { text: "NAT!!".into() });
            } else if n > was {
                cues.write(EncoreCue::Plunge { pos: e.bowl() });
            }
        }
        EncorePhase::Yanked => {
            hp.0 = stand(&hb, seat);
            anim.facing_left = true;
            anim.pose = HanPose::Yank;
            if e.t >= YANKED_SECS {
                e.phase = EncorePhase::Sucked;
                e.t = 0.0;
            }
        }
        EncorePhase::Sucked => {
            hp.0 = stand(&hb, seat);
            anim.facing_left = true;
            anim.pose = HanPose::Intercept;
            if e.t >= SUCK_SECS {
                e.phase = EncorePhase::Continued;
                e.t = 0.0;
                says.write(HanSays { text: "...oops.".into() });
            }
        }
        EncorePhase::Continued => {
            hp.0 = stand(&hb, seat);
            anim.facing_left = true;
            anim.pose = HanPose::Idle;
            if e.t >= CONTINUED_SECS {
                // (Leaving for the level select clears Side B away: `sideb::leave`.)
                next.set(AppState::LevelSelect);
            }
        }
        EncorePhase::Knocked | EncorePhase::Flushing => {}
    }
    // Nat: hidden in the beacon's spot until the plunger brings him up.
    let Ok((nat, mut np, mut tf, finished)) = nat.single_mut() else { return };
    if !finished {
        commands.entity(nat).insert(Finished);
    }
    let half = Body::player().half;
    let cup = seat + CUP + Vec2::new(0.0, half.y);
    match e.phase {
        EncorePhase::Yanked => {
            np.0 = cup;
            tf.translation.z = 5.0;
            tf.rotation = Quat::from_rotation_z((e.t * 14.0).sin() * 0.35);
            tf.scale = Vec3::ONE;
        }
        EncorePhase::Sucked => {
            let bowl = e.bowl();
            if e.t < SLIDE_SECS {
                let s = e.t / SLIDE_SECS;
                np.0 = cup.lerp(bowl, s) + Vec2::new(0.0, 6.0 * 4.0 * s * (1.0 - s));
                tf.rotation = Quat::from_rotation_z(s * 3.0);
            } else {
                if e.t - dt < SLIDE_SECS {
                    tf.translation.z = 1.4;
                    cues.write(EncoreCue::Flush { pos: bowl });
                }
                let s = ((e.t - SLIDE_SECS) / (SUCK_SECS - SLIDE_SECS)).min(1.0);
                np.0 = bowl - Vec2::new(0.0, SINK * s * s);
                tf.rotation = Quat::from_rotation_z(3.0 + e.t * (6.0 + 24.0 * s));
                tf.scale = Vec3::splat((1.0 - s * s).max(0.0));
            }
        }
        EncorePhase::Continued => tf.scale = Vec3::ZERO,
        _ => {}
    }
}
