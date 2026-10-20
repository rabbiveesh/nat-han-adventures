//! The story's encore: what happens at level 10's goal instead of the results card.
//!
//! Nat reaches the Golden Throne and the band takes an encore: a short lap on the stage
//! ([`LAP_SECS`]) where every call is free (Down cycles Giant Steps, fired up, melting) and the
//! band goes all out, a shout chorus a key up. The more Nat calls, the bigger it gets. Then the
//! band plays a Basie ending, and on its last chord the cymbal crash knocks Nat flying into the
//! throne. Han dives with his plunger and misses by a hair. FLUSH. To be continued (Side B).
//!
//! NAT ALONE ends the same way, with its own gag ([`Gag::Grate`]): The Grand Flush's goal
//! starts the lap under a ceiling (the floor of the room above, a grate in it over the
//! throne), and the last chord's rubber duck bounces Nat sky high, up through the grate. Han
//! pops out of the throne below ("Nat?!"), they see each other through the hole, and a shutter
//! SLAMs over it: Nat upstairs, Han downstairs. To be continued (World 4, the double game).
//!
//! The band and the FX follow [`EncoreCue`]s; the knock waits for the band's [`FinalHit`]
//! (with no band, after [`ENDING_TIMEOUT`]).

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::physics::{Body, Dead, Finished};
use super::secret::HanGlimpse;
use super::sideb::{CallMode, SetCall};
use super::solo::{SOLO_COUNT, SoloRun};
use super::{ActiveLevel, GameSet, Han, HanAnim, HanPose, LevelEntity, LevelRun, Player, Pos, PrevPos, cell_floor};
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
/// NAT ALONE's gag: the ceiling's underside sits this high over the stage's floor (px).
pub const SEAM_RISE: f32 = TILE * 8.0;
/// The grate (and the hole it leaves) is this wide (px).
pub const GRATE_WIDTH: f32 = TILE * 2.0;
/// The duck's bounce peaks this far over the ceiling's top (px).
const BOING_OVER: f32 = TILE * 3.5;
/// Nat lands this far right of the hole's middle (px), looking down into it.
const PERCH: f32 = TILE * 1.25;
/// Seconds upstairs, looking down at Han, before the shutter.
pub const UPSTAIRS_SECS: f32 = 2.8;
/// Into the upstairs: when Han pops out of the throne, and how long his hop takes (s).
const HAN_POPS: f32 = 0.35;
const HAN_HOP: f32 = 0.55;
/// Han stands this far left of the hole's middle (px), looking up.
const HAN_STANDS: f32 = TILE * 0.75;
/// The shutter slides over the hole in this long (s); the slam holds this long after (s).
pub const SHUT_SECS: f32 = 0.25;
pub const SLAM_SECS: f32 = 1.4;
/// What the encore's calls cycle through.
const CALLS: [SetCall; 3] = [SetCall::Mode(Harmony::Coltrane), SetCall::Mode(Harmony::Quartal), SetCall::Melt];

/// The encore in progress (only on the story's last level).
#[derive(Resource, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct Encore {
    pub gag: Gag,
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
    /// [`Gag::Grate`]: the ceiling over the stage.
    pub seam: Option<Seam>,
    /// Up through the grate (it's broken: a hole).
    pub through: bool,
    knock: Option<Knock>,
}

/// How the encore goes wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum Gag {
    /// The story (World 1): the cymbal knocks Nat into the throne, FLUSH (into Side B).
    Throne,
    /// NAT ALONE (World 3): the duck bounces Nat up through the grate, the shutter slams between
    /// him and Han (into World 4).
    Grate,
}

/// NAT ALONE's ceiling over the encore's stage: the floor of the room above.
#[derive(Debug, Clone, Copy, Reflect)]
pub struct Seam {
    /// Its underside and its top (y, px).
    pub bottom: f32,
    pub top: f32,
    /// It spans these x's (px).
    pub x0: f32,
    pub x1: f32,
    /// The grate's middle (x, px): over the throne.
    pub hole: f32,
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
    /// [`Gag::Grate`]: Nat's upstairs; Han pops out of the throne below and sees him.
    Upstairs,
    /// [`Gag::Grate`]: the shutter slams over the hole.
    Slam,
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

    /// Where Nat lands upstairs (his middle): beside the hole.
    pub fn perch(&self) -> Option<Vec2> {
        self.seam.map(|s| Vec2::new(s.hole + PERCH, s.top + Body::player().half.y))
    }

    /// Where Han ends up downstairs (his middle): under the hole, looking up.
    pub fn han_spot(&self) -> Option<Vec2> {
        self.seam.map(|s| Vec2::new(s.hole - HAN_STANDS, self.throne.y + Body::player().half.y))
    }

    /// How far the shutter has slid over the hole (0 open, 1 shut).
    pub fn shutter(&self) -> f32 {
        match self.phase {
            EncorePhase::Slam => (self.t / SHUT_SECS).min(1.0),
            EncorePhase::Continued if self.gag == Gag::Grate => 1.0,
            _ => 0.0,
        }
    }

    /// Where the camera looks, if not at Nat: upstairs and down at once, once he's up there.
    pub fn focus(&self) -> Option<Vec2> {
        let s = self.seam?;
        matches!(self.phase, EncorePhase::Upstairs | EncorePhase::Slam | EncorePhase::Continued)
            .then(|| Vec2::new(s.hole, (self.throne.y + s.top + 2.0 * Body::player().half.y) / 2.0))
    }
}

/// Han, just for NAT ALONE's encore gag (he's not in its levels): a picture with poses, no
/// brain or body. Pos is his middle, like Han's.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct StageHan;

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Encore>()
        .register_type::<StageHan>()
        .add_systems(FixedUpdate, run_encore.in_set(GameSet::Follow).run_if(resource_exists::<Encore>))
        .add_systems(Update, call_in_encore.run_if(in_state(PlayState::Running).and_then(resource_exists::<Encore>)))
        .add_systems(OnExit(AppState::Playing), |mut commands: Commands| commands.remove_resource::<Encore>());
}

/// Is `index` the level that ends in the encore (the story's last; not free play or Side B)?
pub fn is_finale(index: usize, story: bool) -> bool {
    story && index + 1 == LEVEL_COUNT
}

/// Is this NAT ALONE's last level (its own encore, [`Gag::Grate`])?
pub fn is_solo_finale(solo: Option<&SoloRun>) -> bool {
    solo.is_some_and(|s| s.0 + 1 == SOLO_COUNT)
}

/// Start the encore at the goal (`goal`: the flag's bottom-center).
pub(super) fn start(
    commands: &mut Commands,
    active: &ActiveLevel,
    run: &LevelRun,
    gag: Gag,
    cues: &mut MessageWriter<EncoreCue>,
    says: &mut MessageWriter<HanSays>,
) {
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
    let throne = goal + Vec2::new(THRONE_OFFSET, 0.0);
    let seam = (gag == Gag::Grate).then(|| Seam {
        bottom: goal.y + SEAM_RISE,
        top: goal.y + SEAM_RISE + TILE,
        x0: left as f32 * TILE,
        x1: level.size_px().x,
        hole: throne.x,
    });
    commands.insert_resource(Encore {
        gag,
        seam,
        through: false,
        phase: EncorePhase::Lap,
        t: 0.0,
        calls: 0,
        stage: (left as f32 * TILE + half, level.size_px().x - half),
        throne,
        time_at_goal: run.time,
        knock: None,
    });
    cues.write(EncoreCue::Start);
    // (No Han to say it in NAT ALONE: the words on screen do.)
    if gag == Gag::Throne {
        says.write(HanSays { text: "Encore!".into() });
    }
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
    mut stage_han: Query<(&mut Pos, &mut HanAnim), (With<StageHan>, Without<Player>, Without<Han>)>,
    glimpses: Query<Entity, With<HanGlimpse>>,
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
            // NAT ALONE's stage has a ceiling (however high Giant Steps jumps).
            if let Some(seam) = e.seam
                && pos.0.y + body.half.y > seam.bottom
            {
                pos.0.y = seam.bottom - body.half.y;
                body.vel.y = body.vel.y.min(0.0);
            }
            if e.phase == EncorePhase::Lap && e.t >= LAP_SECS {
                e.phase = EncorePhase::Ending;
                e.t = 0.0;
                cues.write(EncoreCue::End);
            } else if e.phase == EncorePhase::Ending && (hit || e.t >= ENDING_TIMEOUT) && dead.is_empty() && let Some(seam) = e.seam {
                // NAT ALONE's last chord: a rubber duck pops up under Nat and bounces him sky
                // high, up through the grate.
                let from = pos.0;
                let g = KNOCK_GRAVITY;
                let land = e.perch().unwrap_or(from);
                let vy = (2.0 * g * (seam.top + BOING_OVER - from.y)).sqrt();
                let secs = (vy + (vy * vy - 2.0 * g * (land.y - from.y)).max(0.0).sqrt()) / g;
                e.knock = Some(Knock { from, vel: Vec2::new(0.0, vy), secs, han_from: land, han_to: land });
                e.phase = EncorePhase::Knocked;
                e.t = 0.0;
                commands.entity(nat).insert(Finished);
                body.vel = Vec2::ZERO;
                cues.write(EncoreCue::Knock { pos: from });
                cues.write(EncoreCue::Boing { pos: from - Vec2::new(0.0, body.half.y) });
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
        EncorePhase::Knocked if e.gag == Gag::Grate => {
            let (Some(k), Some(seam), Some(land)) = (e.knock, e.seam, e.perch()) else { return };
            let t = e.t.min(k.secs);
            let g = KNOCK_GRAVITY;
            let y = k.from.y + k.vel.y * t - 0.5 * g * t * t;
            // Over to the hole's middle before his head reaches the ceiling, then over to where
            // he lands.
            let half = Body::player().half.y;
            let up = (k.vel.y - (k.vel.y * k.vel.y - 2.0 * g * (seam.bottom - half - k.from.y)).max(0.0).sqrt()) / g;
            let smooth = |a: f32| a * a * (3.0 - 2.0 * a);
            let x = if t < up {
                k.from.x + (seam.hole - k.from.x) * smooth((t / up.max(0.01)).min(1.0))
            } else {
                seam.hole + (land.x - seam.hole) * smooth(((t - up) / (k.secs - up).max(0.01)).min(1.0))
            };
            pos.0 = Vec2::new(x, y);
            tf.rotation = Quat::from_rotation_z(-t * 11.0 * (1.0 - (t / k.secs)));
            if !e.through && y + half >= seam.bottom {
                e.through = true;
                cues.write(EncoreCue::Through { pos: Vec2::new(seam.hole, (seam.bottom + seam.top) / 2.0) });
            }
            if e.t >= k.secs {
                pos.0 = land;
                tf.rotation = Quat::IDENTITY;
                e.phase = EncorePhase::Upstairs;
                e.t = 0.0;
                // Han's here now: no other picture of him to talk over (the level's secret).
                for g in &glimpses {
                    commands.entity(g).despawn();
                }
                let at = e.bowl();
                commands.spawn((
                    Name::new("StageHan"),
                    LevelEntity,
                    StageHan,
                    HanAnim { pose: HanPose::Jump, facing_left: false, hidden: true, ..default() },
                    Pos(at),
                    PrevPos(at),
                    Transform::from_translation(at.extend(1.45)),
                ));
            }
        }
        EncorePhase::Upstairs => {
            // Nat hops on the spot up there, peering down the hole.
            if let Some(land) = e.perch() {
                let hop = (e.t * 7.0).sin().abs() * 3.0;
                pos.0 = land + Vec2::new(0.0, hop);
            }
            if let (Ok((mut hp, mut anim)), Some(spot)) = (stage_han.single_mut(), e.han_spot()) {
                let s = ((e.t - HAN_POPS) / HAN_HOP).clamp(0.0, 1.0);
                anim.hidden = e.t < HAN_POPS;
                anim.facing_left = false;
                let from = e.bowl();
                hp.0 = from.lerp(spot, s) + Vec2::new(0.0, 30.0 * 4.0 * s * (1.0 - s));
                anim.pose = if s < 1.0 { HanPose::Jump } else { HanPose::Braced };
                if e.t >= HAN_POPS + HAN_HOP && e.t - dt < HAN_POPS + HAN_HOP {
                    says.write(HanSays { text: "Nat?!".into() });
                }
            }
            if e.t >= UPSTAIRS_SECS {
                e.phase = EncorePhase::Slam;
                e.t = 0.0;
            }
        }
        EncorePhase::Slam => {
            if let Some(seam) = e.seam
                && e.t >= SHUT_SECS
                && e.t - dt < SHUT_SECS
            {
                cues.write(EncoreCue::Slam { pos: Vec2::new(seam.hole, seam.top) });
                says.write(HanSays { text: "NAT!!".into() });
                if let Ok((_, mut anim)) = stage_han.single_mut() {
                    anim.pose = HanPose::Intercept;
                    anim.wobble = 0.6;
                }
            }
            if let Some(land) = e.perch() {
                pos.0 = land;
            }
            if e.t >= SLAM_SECS {
                e.phase = EncorePhase::Continued;
                e.t = 0.0;
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
    if let (Gag::Throne, Some(k), Ok((mut hp, mut hb, mut anim))) = (e.gag, e.knock, han.single_mut()) {
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
