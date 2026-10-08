//! Nuggets, checkpoints and the goal flag.

use bevy::prelude::*;

use super::physics::{Body, Dead, Finished};
use super::{ActiveLevel, Checkpoint, GameSet, Goal, LevelRun, Nugget, Player, Pos};
use crate::events::{CheckpointReached, HanSays, LevelCompleted, NuggetCollected};
use crate::level::TILE;
use crate::state::AppState;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<NuggetsAtRisk>().add_systems(
        FixedUpdate,
        (collect_nuggets, touch_checkpoints, touch_goal)
            .chain()
            .in_set(GameSet::Interact)
            .after(super::hazards::check_hazards)
            .before(super::hazards::respawn),
    );
}

/// Nugget pickup box (px, square, centered on the cell).
pub const NUGGET_BOX: f32 = 10.0;
/// Checkpoint box: this wide, one tile tall from the floor.
pub const CHECKPOINT_WIDTH: f32 = 12.0;
/// Goal flag box: this wide, two tiles tall from the floor.
pub const GOAL_WIDTH: f32 = 10.0;

/// Han's lines for checkpoints that have no `say:` in the level file.
pub const CHECKPOINT_QUIPS: &[&str] = &[
    "Two-ply! We're livin' in luxury now, Nat.",
    "Checkpoint! Tight as a fresh pipe fitting.",
    "Roll with it, Nat. Roll with it.",
    "Every roll's a checkpoint if you believe, Nat.",
    "Smells... fine here. Let's keep movin'.",
];

/// Nuggets picked up since the last checkpoint (where they were). Dying puts them back, so a
/// nugget line that fires up the band for a long gap is there again after a failed try.
#[derive(Resource, Debug, Clone, Default)]
pub struct NuggetsAtRisk(pub Vec<Vec2>);

fn overlap(a: Vec2, ah: Vec2, min: Vec2, max: Vec2) -> bool {
    a.x - ah.x < max.x && a.x + ah.x > min.x && a.y - ah.y < max.y && a.y + ah.y > min.y
}

type Alive = (With<Player>, Without<Dead>, Without<Finished>);

fn collect_nuggets(
    mut commands: Commands,
    player: Query<(&Pos, &Body), Alive>,
    nuggets: Query<(Entity, &Transform), With<Nugget>>,
    mut run: ResMut<LevelRun>,
    mut at_risk: ResMut<NuggetsAtRisk>,
    mut collected: MessageWriter<NuggetCollected>,
) {
    let Ok((pos, body)) = player.single() else { return };
    let h = Vec2::splat(NUGGET_BOX / 2.0);
    for (e, tf) in &nuggets {
        let c = tf.translation.truncate();
        if overlap(pos.0, body.half, c - h, c + h) {
            commands.entity(e).despawn();
            run.nuggets += 1;
            at_risk.0.push(c);
            collected.write(NuggetCollected { pos: c });
        }
    }
}

fn touch_checkpoints(
    active: Res<ActiveLevel>,
    player: Query<(&Pos, &Body), Alive>,
    mut checkpoints: Query<(&mut Checkpoint, &Transform)>,
    mut run: ResMut<LevelRun>,
    mut at_risk: ResMut<NuggetsAtRisk>,
    mut reached: MessageWriter<CheckpointReached>,
    mut says: MessageWriter<HanSays>,
) {
    let Ok((pos, body)) = player.single() else { return };
    for (mut cp, tf) in &mut checkpoints {
        if cp.active {
            continue;
        }
        let floor = tf.translation.truncate();
        let min = floor - Vec2::new(CHECKPOINT_WIDTH / 2.0, 0.0);
        let max = floor + Vec2::new(CHECKPOINT_WIDTH / 2.0, TILE);
        if !overlap(pos.0, body.half, min, max) {
            continue;
        }
        cp.active = true;
        at_risk.0.clear();
        // Later checkpoints win; touching an earlier one after a later one doesn't send you back.
        if run.checkpoint.is_none_or(|c| c < cp.index) {
            run.checkpoint = Some(cp.index);
        }
        reached.write(CheckpointReached { index: cp.index, pos: floor });
        let text = active.level.says.get(cp.index).cloned().unwrap_or_else(|| {
            CHECKPOINT_QUIPS[cp.index % CHECKPOINT_QUIPS.len()].to_string()
        });
        says.write(HanSays { text });
    }
}

fn touch_goal(
    mut commands: Commands,
    active: Res<ActiveLevel>,
    mut player: Query<(Entity, &Pos, &mut Body), Alive>,
    goal: Query<&Transform, With<Goal>>,
    run: Res<LevelRun>,
    mut completed: MessageWriter<LevelCompleted>,
    mut next: ResMut<NextState<AppState>>,
) {
    let Ok((entity, pos, mut body)) = player.single_mut() else { return };
    let Ok(goal) = goal.single() else { return };
    let floor = goal.translation.truncate();
    let min = floor - Vec2::new(GOAL_WIDTH / 2.0, 0.0);
    let max = floor + Vec2::new(GOAL_WIDTH / 2.0, 2.0 * TILE);
    if !overlap(pos.0, body.half, min, max) {
        return;
    }
    body.vel = Vec2::ZERO;
    commands.entity(entity).insert(Finished);
    completed.write(LevelCompleted {
        level: active.index,
        nuggets: run.nuggets,
        nuggets_total: run.nuggets_total,
        time_secs: run.time,
    });
    next.set(AppState::LevelComplete);
}
