//! Loading, restarting and unloading levels; the level timer; pause.

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::han::{HanAnim, HanMotion, HanTrail};
use super::physics::{Body, PlayerControl};
use super::{
    ActiveLevel, Checkpoint, Fly, GameSet, Goal, Groove, Han, LevelEntity, LevelRun, LevelTile,
    MovingPlatform, Nugget, Player, Pos, PrevPos, RestartLevel, SimClock, Spray, tuning,
};
use crate::events::HanSays;
use crate::input::Action;
use crate::level::{Level, Levels, TILE, ThingKind, Tile};
use crate::state::{AppState, CurrentLevel, PlayState};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Playing), enter_level)
        .add_systems(OnEnter(AppState::Title), unload_level)
        .add_systems(OnEnter(AppState::LevelSelect), unload_level)
        .add_systems(OnEnter(AppState::Victory), unload_level)
        .add_systems(
            Update,
            (
                level_hotkeys.run_if(in_state(PlayState::Running)),
                restart_level.run_if(in_state(AppState::Playing)),
            )
                .chain(),
        )
        .add_systems(FixedUpdate, tick_clocks.in_set(GameSet::Prepare));
}

fn enter_level(
    mut commands: Commands,
    levels: Res<Levels>,
    current: Res<CurrentLevel>,
    old: Query<Entity, With<LevelEntity>>,
    mut says: MessageWriter<HanSays>,
) {
    let level = load(&mut commands, &levels, current.0, &old);
    commands.write_message(crate::events::LevelStarted { level: current.0, restart: false });
    if !level.intro.is_empty() {
        says.write(HanSays { text: level.intro.clone() });
    }
}

fn unload_level(mut commands: Commands, old: Query<Entity, With<LevelEntity>>) {
    for e in &old {
        commands.entity(e).despawn();
    }
    commands.remove_resource::<ActiveLevel>();
    commands.insert_resource(Groove::default());
}

/// R restarts, Esc/Backspace pauses.
fn level_hotkeys(
    input: Single<&ActionState<Action>>,
    mut restart: MessageWriter<RestartLevel>,
    mut next: ResMut<NextState<PlayState>>,
) {
    if input.just_pressed(&Action::Restart) {
        restart.write(RestartLevel);
    } else if input.just_pressed(&Action::Back) {
        next.set(PlayState::Paused);
    }
}

fn restart_level(
    mut requests: MessageReader<RestartLevel>,
    mut commands: Commands,
    levels: Res<Levels>,
    active: Option<Res<ActiveLevel>>,
    current: Res<CurrentLevel>,
    old: Query<Entity, With<LevelEntity>>,
) {
    if requests.read().count() == 0 {
        return;
    }
    let index = active.map_or(current.0, |a| a.index);
    load(&mut commands, &levels, index, &old);
    commands.write_message(crate::events::LevelStarted { level: index, restart: true });
}

/// Despawn whatever is loaded and spawn level `index` fresh, resetting the run.
fn load(
    commands: &mut Commands,
    levels: &Levels,
    index: usize,
    old: &Query<Entity, With<LevelEntity>>,
) -> Level {
    for e in old {
        commands.entity(e).despawn();
    }
    let index = index.min(levels.0.len().saturating_sub(1));
    let level = levels.0[index].clone();

    commands.insert_resource(LevelRun {
        nuggets_total: level.nugget_count() as u32,
        ..default()
    });
    commands.insert_resource(SimClock::default());
    commands.insert_resource(super::OneJumps::default());
    commands.insert_resource(super::pickups::NuggetsAtRisk::default());
    // The band starts every level playing it straight (the audio plugin switches back at the
    // next bar line); physics don't wait for it.
    commands.insert_resource(Groove::default());
    commands.insert_resource(ActiveLevel { index, level: level.clone() });
    spawn_level(commands, &level);
    level
}

/// Where the player's box center goes when standing in cell (col, row).
pub(super) fn stand_pos(level: &Level, col: usize, row: usize) -> Vec2 {
    let c = level.tile_center(col, row);
    Vec2::new(c.x, c.y - TILE / 2.0 + tuning::PLAYER_SIZE.1 / 2.0)
}

/// Bottom-center of a cell.
fn cell_floor(level: &Level, col: usize, row: usize) -> Vec2 {
    level.tile_center(col, row) - Vec2::new(0.0, TILE / 2.0)
}

fn spawn_level(commands: &mut Commands, level: &Level) {
    // Tiles.
    for row in 0..level.height {
        for col in 0..level.width {
            let tile = level.tile(col as i32, row as i32);
            if tile == Tile::Empty {
                continue;
            }
            let above = level.tile(col as i32, row as i32 - 1);
            let top = match tile {
                Tile::Solid | Tile::Grease => !above.is_solid(),
                Tile::Liquid => above != Tile::Liquid,
                _ => true,
            };
            commands.spawn((
                Name::new("Tile"),
                LevelEntity,
                LevelTile { col, row, tile, top, world: level.world },
                Transform::from_translation(level.tile_center(col, row).extend(0.0)),
            ));
        }
    }

    // Things.
    let mut checkpoint_index = 0;
    for (i, thing) in level.things.iter().enumerate() {
        let center = level.tile_center(thing.col, thing.row);
        match thing.kind {
            ThingKind::Nugget => {
                commands.spawn((
                    Name::new("Nugget"),
                    LevelEntity,
                    Nugget,
                    Transform::from_translation(center.extend(2.0)),
                ));
            }
            ThingKind::Checkpoint => {
                commands.spawn((
                    Name::new("Checkpoint"),
                    LevelEntity,
                    Checkpoint { index: checkpoint_index, active: false },
                    Transform::from_translation(
                        cell_floor(level, thing.col, thing.row).extend(2.0),
                    ),
                ));
                checkpoint_index += 1;
            }
            ThingKind::Fly => {
                // Golden-ratio phases so neighbouring swarms don't move in lockstep.
                let phase = (i as f32 * 0.618_034).fract();
                commands.spawn((
                    Name::new("Fly"),
                    LevelEntity,
                    Fly { center, phase },
                    Pos(center),
                    PrevPos(center),
                    Transform::from_translation(center.extend(2.0)),
                ));
            }
            ThingKind::Spray => {
                commands.spawn((
                    Name::new("Spray"),
                    LevelEntity,
                    Spray { col: thing.col, on: false },
                    Transform::from_translation(
                        cell_floor(level, thing.col, thing.row).extend(2.0),
                    ),
                ));
            }
        }
    }

    for hint in &level.hints {
        commands.spawn((
            Name::new("HintSpot"),
            LevelEntity,
            super::HintSpot { center: level.tile_center(hint.col, hint.row), text: hint.text.clone(), said: false },
        ));
    }

    commands.spawn((
        Name::new("Goal"),
        LevelEntity,
        Goal,
        Transform::from_translation(cell_floor(level, level.goal.0, level.goal.1).extend(2.0)),
    ));

    // Moving platforms: Pos is the center of the platform's tiles.
    for def in &level.platforms {
        let left = level.tile_center(def.col, def.row);
        let base = left + Vec2::new((def.width as f32 - 1.0) * TILE / 2.0, 0.0);
        let platform = MovingPlatform {
            base,
            travel: Vec2::new(def.dx, def.dy) * TILE,
            period: def.period,
            phase: def.phase,
            width: def.width,
            kind: def.kind,
        };
        let pos = super::platforms::platform_pos(&platform, 0.0);
        commands.spawn((
            Name::new("MovingPlatform"),
            LevelEntity,
            platform,
            Pos(pos),
            PrevPos(pos),
            Transform::from_translation(pos.extend(1.0)),
        ));
    }

    // Player and Han.
    let start = stand_pos(level, level.start.0, level.start.1);
    commands.spawn((
        Name::new("Player"),
        LevelEntity,
        Player,
        Body::player(),
        PlayerControl::default(),
        Pos(start),
        PrevPos(start),
        Transform::from_translation(start.extend(5.0)),
    ));
    let han = super::han::behind(level, start, 1.0);
    commands.spawn((
        Name::new("Han"),
        LevelEntity,
        Han,
        HanTrail::default(),
        HanMotion::default(),
        HanAnim::default(),
        Pos(han),
        PrevPos(han),
        Transform::from_translation(han.extend(4.0)),
    ));
}

fn tick_clocks(
    time: Res<Time>,
    groove: Res<Groove>,
    mut clock: ResMut<SimClock>,
    mut run: ResMut<LevelRun>,
    mut prev: Query<(&Pos, &mut PrevPos)>,
) {
    let dt = time.delta_secs();
    clock.time += dt;
    clock.steps += 1;
    clock.platform_time += dt * groove.platform_rate();
    clock.fly_turns += dt * groove.fly_rate(super::hazards::FLY_PERIOD);
    run.time += dt;
    for (pos, mut prev) in &mut prev {
        prev.0 = pos.0;
    }
}
