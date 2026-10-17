//! The story's encore: what happens at level 10's goal instead of the results card.
//!
//! Nat reaches the Golden Throne and the band takes an encore: a short lap on the stage
//! ([`LAP_SECS`]) where every call is free (Down cycles Giant Steps, fired up, melting) and the
//! band goes all out, a shout chorus a key up. The more Nat calls, the bigger it gets. Then the
//! band plays a Basie ending, and on its last chord the cymbal crash knocks Nat flying into the
//! throne. Han dives with his plunger and misses by a hair. FLUSH. To be continued (Side B).
//!
//! The band and the FX follow [`EncoreCue`]s; the knock waits for the band's [`FinalHit`]
//! (with no band, after [`ENDING_TIMEOUT`]).

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::physics::{Body, Dead, Finished};
use super::sideb::{CallMode, SetCall};
use super::{ActiveLevel, GameSet, Han, HanAnim, HanPose, LevelRun, Player, Pos, cell_floor};
use crate::audio::Harmony;
use crate::events::{EncoreCue, FinalHit, HanSays, LevelCompleted};
use crate::input::Action;
use crate::level::{LEVEL_COUNT, TILE};
use crate::state::{AppState, PlayState};

/// How long the encore lap lasts before the band plays its ending.
pub const LAP_SECS: f32 = 12.0;
/// With no band to hear (tests), the final hit comes this long after the ending was asked for.
pub const ENDING_TIMEOUT: f32 = 8.0;
/// Seconds in the throne before Nat's gone.
pub const FLUSH_SECS: f32 = 2.0;
/// "To be continued" holds this long before the results card.
pub const CONTINUED_SECS: f32 = 3.5;
/// Gravity of the knock's arc (px/s²): far floatier than Nat's own, so it reads (and stays on
/// screen).
const KNOCK_GRAVITY: f32 = 380.0;
/// How high above the higher end the knock's arc peaks (px), and how much more per call.
const KNOCK_PEAK: f32 = 56.0;
const KNOCK_PEAK_PER_CALL: f32 = 3.0;
/// The throne sits this far right of the goal flag (`visuals`: drawn there).
pub const THRONE_OFFSET: f32 = TILE * 1.5;
/// Where Nat lands in the throne, from its bottom-center: over the bowl, just in.
const BOWL: Vec2 = Vec2::new(7.0, 22.0);
/// How deep he sinks while it flushes (px).
const SINK: f32 = 16.0;
/// What the encore's calls cycle through.
const CALLS: [SetCall; 3] = [SetCall::Mode(Harmony::Coltrane), SetCall::Mode(Harmony::Quartal), SetCall::Melt];

/// The encore in progress (only on the story's last level).
#[derive(Resource, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct Encore {
    pub phase: EncorePhase,
    /// Seconds in this phase.
    pub t: f32,
    /// Calls in the lap.
    pub calls: u32,
    /// The stage: Nat stays between these x's (px) until the knock.
    pub stage: (f32, f32),
    /// The throne's bottom-center.
    pub throne: Vec2,
    /// The level's time when Nat reached the goal (the results count that, not the encore).
    pub time_at_goal: f32,
    knock: Option<Knock>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum EncorePhase {
    /// The encore lap: free calls, the band all out.
    Lap,
    /// The band plays its ending; waiting for the last chord.
    Ending,
    /// Flying into the throne (Han diving after him).
    Knocked,
    /// In the throne: FLUSH.
    Flushing,
    /// "To be continued...", then the results card.
    Continued,
}

/// The knock's arc, and Han's dive.
#[derive(Debug, Clone, Copy, Reflect)]
struct Knock {
    from: Vec2,
    vel: Vec2,
    secs: f32,
    han_from: Vec2,
    han_to: Vec2,
}

impl Encore {
    /// Where Nat lands, in the bowl.
    pub fn bowl(&self) -> Vec2 {
        self.throne + BOWL
    }
}

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Encore>()
        .add_systems(FixedUpdate, run_encore.in_set(GameSet::Follow).run_if(resource_exists::<Encore>))
        .add_systems(Update, call_in_encore.run_if(in_state(PlayState::Running).and_then(resource_exists::<Encore>)))
        .add_systems(OnExit(AppState::Playing), |mut commands: Commands| commands.remove_resource::<Encore>());
}

/// Is `index` the level that ends in the encore (the story's last; not free play or Side B)?
pub fn is_finale(index: usize, story: bool) -> bool {
    story && index + 1 == LEVEL_COUNT
}

/// Start the encore at the goal (`goal`: the flag's bottom-center).
pub(super) fn start(commands: &mut Commands, active: &ActiveLevel, run: &LevelRun, cues: &mut MessageWriter<EncoreCue>, says: &mut MessageWriter<HanSays>) {
    let level = &active.level;
    let (gc, gr) = level.goal;
    let goal = cell_floor(level, gc, gr);
    // The stage: the floor the goal stands on, as far left as it runs, to the level's end.
    let floor_row = gr as i32 + 1;
    let mut left = gc as i32;
    while left > 0 && level.tile(left - 1, floor_row).is_solid() && !level.tile(left - 1, gr as i32).is_solid() {
        left -= 1;
    }
    let half = Body::player().half.x;
    commands.insert_resource(Encore {
        phase: EncorePhase::Lap,
        t: 0.0,
        calls: 0,
        stage: (left as f32 * TILE + half, level.size_px().x - half),
        throne: goal + Vec2::new(THRONE_OFFSET, 0.0),
        time_at_goal: run.time,
        knock: None,
    });
    cues.write(EncoreCue::Start);
    says.write(HanSays { text: "Encore!".into() });
}

/// Down in the lap: the next call, free.
fn call_in_encore(
    input: Single<&ActionState<Action>>,
    mut encore: ResMut<Encore>,
    player: Query<&Pos, With<Player>>,
    mut calls: MessageWriter<CallMode>,
    mut cues: MessageWriter<EncoreCue>,
) {
    if encore.phase != EncorePhase::Lap || !input.just_pressed(&Action::Down) {
        return;
    }
    calls.write(CallMode(CALLS[encore.calls as usize % CALLS.len()]));
    encore.calls += 1;
    let pos = player.single().map_or(Vec2::ZERO, |p| p.0);
    cues.write(EncoreCue::Call { pos, hype: encore.calls });
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn run_encore(
    time: Res<Time>,
    mut commands: Commands,
    mut encore: ResMut<Encore>,
    active: Res<ActiveLevel>,
    mut run: ResMut<LevelRun>,
    mut hits: MessageReader<FinalHit>,
    mut player: Query<(Entity, &mut Pos, &mut Body, &mut Transform), (With<Player>, Without<Han>)>,
    mut han: Query<(&mut Pos, &mut Body, &mut HanAnim), (With<Han>, Without<Player>)>,
    dead: Query<(), (With<Player>, With<Dead>)>,
    mut cues: MessageWriter<EncoreCue>,
    mut says: MessageWriter<HanSays>,
    mut completed: MessageWriter<LevelCompleted>,
    mut next: ResMut<NextState<AppState>>,
) {
    let dt = time.delta_secs();
    let hit = hits.read().count() > 0;
    let Ok((nat, mut pos, mut body, mut tf)) = player.single_mut() else { return };
    let e = &mut *encore;
    e.t += dt;
    match e.phase {
        EncorePhase::Lap | EncorePhase::Ending => {
            // The stage has edges.
            if pos.0.x < e.stage.0 || pos.0.x > e.stage.1 {
                pos.0.x = pos.0.x.clamp(e.stage.0, e.stage.1);
                body.vel.x = 0.0;
            }
            if e.phase == EncorePhase::Lap && e.t >= LAP_SECS {
                e.phase = EncorePhase::Ending;
                e.t = 0.0;
                cues.write(EncoreCue::End);
            } else if e.phase == EncorePhase::Ending && (hit || e.t >= ENDING_TIMEOUT) && dead.is_empty() {
                // The last chord: the cymbal knocks Nat into the throne.
                let from = pos.0;
                let to = e.bowl();
                let g = KNOCK_GRAVITY;
                let peak = from.y.max(to.y) + KNOCK_PEAK + KNOCK_PEAK_PER_CALL * e.calls.min(10) as f32;
                let vy = (2.0 * g * (peak - from.y)).sqrt();
                let secs = (vy + (vy * vy - 2.0 * g * (to.y - from.y)).max(0.0).sqrt()) / g;
                let vel = Vec2::new((to.x - from.x) / secs, vy);
                // Han dives for him from wherever he is, and lands a hair short of the bowl.
                let (han_from, side) = han.single().map_or((from, -1.0), |(p, _, _)| (p.0, if p.0.x < to.x { -1.0 } else { 1.0 }));
                let han_y = e.throne.y + Body::player().half.y;
                e.knock = Some(Knock { from, vel, secs, han_from, han_to: Vec2::new(to.x + side * 20.0, han_y) });
                e.phase = EncorePhase::Knocked;
                e.t = 0.0;
                commands.entity(nat).insert(Finished);
                body.vel = Vec2::ZERO;
                cues.write(EncoreCue::Knock { pos: from });
                says.write(HanSays { text: "NAT!!".into() });
            }
        }
        EncorePhase::Knocked => {
            let Some(k) = e.knock else { return };
            let t = e.t.min(k.secs);
            pos.0 = k.from + k.vel * t - Vec2::new(0.0, 0.5 * KNOCK_GRAVITY * t * t);
            tf.rotation = Quat::from_rotation_z(-k.vel.x.signum() * t * 9.0);
            if e.t >= k.secs {
                e.phase = EncorePhase::Flushing;
                e.t = 0.0;
                // Behind the throne from here: he sinks into the bowl.
                tf.translation.z = 1.4;
                cues.write(EncoreCue::Flush { pos: pos.0 });
            }
        }
        EncorePhase::Flushing => {
            let s = (e.t / FLUSH_SECS).min(1.0);
            pos.0 = e.bowl() - Vec2::new(0.0, SINK * s * s);
            tf.rotation = Quat::from_rotation_z(-e.t * (6.0 + 18.0 * s));
            tf.scale = Vec3::splat((1.0 - s * s).max(0.0));
            if e.t >= FLUSH_SECS {
                e.phase = EncorePhase::Continued;
                e.t = 0.0;
                says.write(HanSays { text: "...Nat?".into() });
            }
        }
        EncorePhase::Continued => {
            if e.t >= CONTINUED_SECS {
                run.time = e.time_at_goal;
                completed.write(LevelCompleted {
                    level: active.index,
                    nuggets: run.nuggets,
                    nuggets_total: run.nuggets_total,
                    time_secs: run.time,
                });
                next.set(AppState::LevelComplete);
            }
        }
    }
    // Han's dive: a low leap after Nat that lands a hair too late and too short.
    if let (Some(k), Ok((mut hp, mut hb, mut anim))) = (e.knock, han.single_mut()) {
        let start = 0.15;
        let dive = (k.secs + 0.12 - start).max(0.1);
        let since = match e.phase {
            EncorePhase::Knocked => e.t,
            _ => f32::INFINITY,
        };
        let s = ((since - start) / dive).clamp(0.0, 1.0);
        hb.vel = Vec2::ZERO;
        anim.facing_left = k.han_to.x < k.han_from.x;
        anim.hidden = false;
        if since < start {
            hp.0 = k.han_from;
            anim.pose = HanPose::Braced;
        } else if s < 1.0 {
            hp.0 = k.han_from.lerp(k.han_to, s) + Vec2::new(0.0, 28.0 * 4.0 * s * (1.0 - s));
            anim.pose = HanPose::Intercept;
        } else {
            hp.0 = k.han_to;
            anim.pose = if e.phase == EncorePhase::Continued { HanPose::Idle } else { HanPose::Braced };
        }
    }
}
