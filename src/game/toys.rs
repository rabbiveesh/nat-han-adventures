//! The blast button (`M`) and stop signs (`@`).
//!
//! **The blast button**, with a speaker stack either side: land on it (a jump onto it, not a
//! walk) and the band BLASTS: [`BlastMusic`] tells the audio to play a shout chorus a key up,
//! flat out and louder for [`BLAST_SECS`], the blast throws Nat high ([`BLAST_SPEED`], the
//! toot back) and knocks every sploosh diaper near out of the sky. Landing on it again while
//! it blasts throws him again and keeps it going.
//!
//! **A stop sign**: touch it and the level stops right there. Nat skids to a halt ("STOP!"),
//! and a moment later it's cleared, like the goal.

use bevy::prelude::*;

use super::diapers::{Diaper, DiaperState, Splooshed};
use super::physics::{Body, Dead, Finished, PlayerControl};
use super::{ActiveLevel, GameSet, Groove, LevelEntity, LevelRun, Player, Pos, cell_floor};
use crate::events::{Landed, LevelCompleted, LevelStarted, ToyShout};
use crate::level::{BLAST_SECS, BLAST_SPEED, Level, TILE};
use crate::state::AppState;

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Blaster>()
        .register_type::<StopSign>()
        .init_resource::<Blasting>()
        .init_resource::<Stopping>()
        .add_message::<BlastMusic>()
        .add_message::<Blasted>()
        .add_message::<SignHit>()
        .add_systems(Update, calm_down)
        .add_systems(
            FixedUpdate,
            (press_button.after(super::physics::player_step).in_set(GameSet::Player), (stop_sign, stop_the_level).chain().in_set(GameSet::Interact)),
        );
}

/// A blast knocks diapers out of the sky this far away (px).
pub const BLAST_REACH: f32 = 12.0 * TILE;
/// The button: this wide (px), and Nat lands within this much of its middle to press it.
pub const BUTTON_HALF: f32 = 8.0;
/// A stop sign's box (half extents, px; its feet on its cell's floor).
pub const SIGN_HALF: Vec2 = Vec2::new(5.0, 14.0);
/// Nat skids this long (s) at a stop sign before the level's cleared.
pub const SKID_SECS: f32 = 1.2;

/// The blast button. Its transform is its cell's floor.
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct Blaster {
    /// Since it was last pressed (s; drives its look).
    pub since: f32,
    pub pressed: bool,
}

/// A stop sign. Its transform is its cell's floor.
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct StopSign;

/// The band's blasting for this many more seconds.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq)]
pub struct Blasting(pub f32);

/// The music blasts (`true`) or calms down (`false`): the audio plugin plays it.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlastMusic(pub bool);

/// A button pressed at `pos` (for the boom, the shake).
#[derive(Message, Debug, Clone, Copy)]
pub struct Blasted {
    pub pos: Vec2,
}

/// Nat hit a stop sign at `pos`.
#[derive(Message, Debug, Clone, Copy)]
pub struct SignHit {
    pub pos: Vec2,
}

/// Skidding at a stop sign: the level's cleared in this many seconds.
#[derive(Resource, Debug, Clone, Copy, Default)]
struct Stopping(Option<f32>);

/// Landing on a button: BLAST.
#[allow(clippy::too_many_arguments)]
fn press_button(
    time: Res<Time>,
    groove: Res<Groove>,
    mut landed: MessageReader<Landed>,
    mut nat: Query<(&Pos, &mut Body, &mut PlayerControl), (With<Player>, Without<Dead>)>,
    mut buttons: Query<(&mut Blaster, &Transform)>,
    mut diapers: Query<(&mut Diaper, &Pos), Without<Player>>,
    mut blasting: ResMut<Blasting>,
    mut music: MessageWriter<BlastMusic>,
    mut blasted: MessageWriter<Blasted>,
    mut splooshed: MessageWriter<Splooshed>,
    mut shout: MessageWriter<ToyShout>,
) {
    let dt = time.delta_secs();
    for (mut b, _) in &mut buttons {
        b.since += dt;
        b.pressed = b.since < 0.3;
    }
    let lands: Vec<Vec2> = landed.read().map(|l| l.pos).collect();
    let Ok((pos, mut body, mut ctl)) = nat.single_mut() else { return };
    for at in lands {
        let feet = at.y - body.half.y;
        let Some((mut b, tf)) = buttons
            .iter_mut()
            .find(|(_, tf)| (at.x - tf.translation.x).abs() <= BUTTON_HALF + body.half.x / 2.0 && (feet - tf.translation.y).abs() < 4.0)
        else {
            continue;
        };
        b.since = 0.0;
        b.pressed = true;
        let floor = tf.translation.truncate();
        body.vel.y = BLAST_SPEED * groove.jump_scale().max(1.0);
        body.on_ground = false;
        body.riding = None;
        ctl.has_toot = true;
        ctl.cut_armed = false;
        ctl.coyote = 0.0;
        if blasting.0 <= 0.0 {
            music.write(BlastMusic(true));
        }
        blasting.0 = BLAST_SECS;
        blasted.write(Blasted { pos: floor });
        shout.write(ToyShout { pos: floor + Vec2::Y * 28.0, text: "BLAST!" });
        for (mut d, dp) in &mut diapers {
            if d.deadly() && dp.0.distance(pos.0) <= BLAST_REACH {
                d.state = DiaperState::Splooshed { t: 0.0 };
                splooshed.write(Splooshed { pos: dp.0 });
            }
        }
        return;
    }
}

/// The blast runs out (and a level start or leaving play ends it at once).
fn calm_down(
    time: Res<Time>,
    state: Res<State<AppState>>,
    play: Option<Res<State<crate::state::PlayState>>>,
    mut started: MessageReader<LevelStarted>,
    mut blasting: ResMut<Blasting>,
    mut music: MessageWriter<BlastMusic>,
) {
    if blasting.0 <= 0.0 {
        started.clear();
        return;
    }
    let running = *state.get() == AppState::Playing && play.is_some_and(|p| *p.get() == crate::state::PlayState::Running);
    if running {
        blasting.0 -= time.delta_secs();
    }
    if started.read().count() > 0 || *state.get() != AppState::Playing || blasting.0 <= 0.0 {
        blasting.0 = 0.0;
        music.write(BlastMusic(false));
    }
}

fn overlap(a: Vec2, ah: Vec2, b: Vec2, bh: Vec2) -> bool {
    (a.x - b.x).abs() < ah.x + bh.x && (a.y - b.y).abs() < ah.y + bh.y
}

/// Touching a stop sign: Nat skids to a halt, and the level stops there.
fn stop_sign(
    mut commands: Commands,
    mut nat: Query<(Entity, &Pos, &mut Body), (With<Player>, Without<Dead>, Without<Finished>)>,
    signs: Query<&Transform, With<StopSign>>,
    encore: Option<Res<super::encore::Encore>>,
    mut stopping: ResMut<Stopping>,
    mut hit: MessageWriter<SignHit>,
    mut shout: MessageWriter<ToyShout>,
) {
    let Ok((e, pos, mut body)) = nat.single_mut() else { return };
    if encore.is_some() || stopping.0.is_some() {
        return;
    }
    for tf in &signs {
        let mid = tf.translation.truncate() + Vec2::Y * SIGN_HALF.y;
        if overlap(pos.0, body.half, mid, SIGN_HALF) {
            body.vel = Vec2::ZERO;
            commands.entity(e).insert(Finished);
            stopping.0 = Some(SKID_SECS);
            hit.write(SignHit { pos: pos.0 });
            shout.write(ToyShout { pos: mid + Vec2::Y * 22.0, text: "STOP!" });
            return;
        }
    }
}

/// ...and a moment later, cleared.
fn stop_the_level(
    time: Res<Time>,
    active: Res<ActiveLevel>,
    run: Res<LevelRun>,
    mut stopping: ResMut<Stopping>,
    mut completed: MessageWriter<LevelCompleted>,
    mut next: ResMut<NextState<AppState>>,
    mut started: MessageReader<LevelStarted>,
) {
    if started.read().count() > 0 {
        stopping.0 = None;
    }
    let Some(left) = stopping.0 else { return };
    let left = left - time.delta_secs();
    if left > 0.0 {
        stopping.0 = Some(left);
        return;
    }
    stopping.0 = None;
    completed.write(LevelCompleted { level: active.index, nuggets: run.nuggets, nuggets_total: run.nuggets_total, time_secs: run.time });
    next.set(AppState::LevelComplete);
}

pub(super) fn spawn_blaster(commands: &mut Commands, level: &Level, col: usize, row: usize) {
    commands.spawn((
        Name::new("Blaster"),
        LevelEntity,
        Blaster { since: 10.0, pressed: false },
        Transform::from_translation(cell_floor(level, col, row).extend(1.8)),
    ));
}

pub(super) fn spawn_sign(commands: &mut Commands, level: &Level, col: usize, row: usize) {
    commands.spawn((Name::new("StopSign"), LevelEntity, StopSign, Transform::from_translation(cell_floor(level, col, row).extend(1.8))));
}
