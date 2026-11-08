//! Level complete card, over the frozen level. Records progress (and so saves) on entry.

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::{palette::*, *};
use crate::game::solo::{self, SOLO_COUNT, SoloRun};
use crate::game::{GeneratedLevel, LevelRun};
use crate::input::Action;
use crate::level::{LEVEL_COUNT, Levels};
use crate::save::{Progress, RecordOutcome};
use crate::state::{AppState, CurrentLevel};

const HEADERS: [&str; 5] = ["FLUSHED IT!", "WHAT A RELIEF!", "PLOP-TASTIC!", "SMELLS LIKE VICTORY", "DOOTY CALLS!"];

pub fn plugin(app: &mut App) {
    // Free play has its own card (`crate::freeplay::ui`) and records nothing here.
    let story = not(resource_exists::<crate::freeplay::FreePlayRun>);
    app.add_systems(OnEnter(AppState::LevelComplete), (record, spawn).chain().run_if(story.clone()))
        .add_systems(Update, input.run_if(in_state(AppState::LevelComplete).and_then(story)));
}

/// The result of the run just finished, for the card.
#[derive(Resource, Debug, Clone, Default)]
pub struct LastResult {
    pub level: usize,
    /// A NAT ALONE level (`game::solo`), not the story's.
    pub solo: bool,
    pub run: LevelRun,
    pub outcome: RecordOutcome,
}

/// Update [`Progress`] from the finished run (the persistence plugin saves it on change).
pub fn record_run(progress: &mut Progress, level: usize, run: &LevelRun) -> RecordOutcome {
    progress.record(level, run.nuggets, run.time)
}

fn record(
    mut commands: Commands,
    mut progress: ResMut<Progress>,
    current: Res<CurrentLevel>,
    run: Res<LevelRun>,
    solo: Option<Res<SoloRun>>,
) {
    let result = match solo {
        Some(s) => LastResult { level: s.0, solo: true, run: run.clone(), outcome: progress.record_solo(s.0, run.nuggets, run.time) },
        None => LastResult { level: current.0, solo: false, run: run.clone(), outcome: record_run(&mut progress, current.0, &run) },
    };
    commands.insert_resource(result);
}

fn spawn(
    mut commands: Commands,
    font: Res<UiFont>,
    sprites: Option<Res<Sprites>>,
    levels: Res<Levels>,
    generated: Option<Res<GeneratedLevel>>,
    result: Res<LastResult>,
) {
    let f = &*font;
    let r = &*result;
    let level = if r.solo { generated.as_deref().map(|g| &g.0) } else { levels.0.get(r.level) };
    let total = r.run.nuggets_total.max(level.map_or(0, |l| l.nugget_count() as u32));
    let name = level.map_or_else(|| "???".into(), |l| l.name.to_uppercase());
    let last = r.level + 1 >= if r.solo { SOLO_COUNT } else { LEVEL_COUNT };
    let what = if r.solo { "NAT ALONE" } else { "LEVEL" };
    commands
        .spawn((
            Name::new("ResultsCard"),
            DespawnOnExit(AppState::LevelComplete),
            fullscreen(),
            BackgroundColor(OVERLAY),
            GlobalZIndex(10),
        ))
        .with_children(|root| {
            root.spawn(panel(Node { min_width: px(240.0), padding: UiRect::axes(px(16.0), px(12.0)), ..column(16.0, 8.0) }))
                .with_children(|p| {
                    p.spawn(label(f, HEADERS[r.level % HEADERS.len()], 16.0, GOLD));
                    p.spawn((label(f, format!("{what} {}: {name}", r.level + 1), 8.0, CREAM), Node { margin: UiRect::bottom(px(8.0)), ..default() }));
                    let row = |p: &mut ChildSpawnerCommands, text: String, best: bool, with_icon: bool| {
                        p.spawn(Node { column_gap: px(8.0), align_items: AlignItems::Center, ..default() })
                            .with_children(|r| {
                                if with_icon {
                                    r.spawn(icon(sprites.as_deref(), SpriteId::IconNugget, 8.0, 8.0));
                                }
                                r.spawn(label(f, text, 8.0, CREAM));
                                if best {
                                    r.spawn((label(f, "NEW BEST!", 8.0, GREEN), Blink(0.5)));
                                }
                            });
                    };
                    row(p, format!("NUGGETS {}/{total}", r.run.nuggets), r.outcome.new_best_nuggets, true);
                    row(p, format!("TIME {}", format_time(r.run.time)), r.outcome.new_best_time, false);
                    let splats = match r.run.deaths {
                        0 => "SPLATS 0  SPOTLESS!".to_string(),
                        n => format!("SPLATS {n}"),
                    };
                    row(p, splats, false, false);
                    if total > 0 && r.run.nuggets >= total {
                        p.spawn(label(f, "EVERY LAST NUGGET!", 8.0, GOLD));
                    }
                    if r.outcome.unlocked_next && !last {
                        p.spawn(label(f, format!("{what} {} UNLOCKED!", r.level + 2), 8.0, GREEN));
                    }
                    let prompt = match (last, r.solo) {
                        (true, true) => "ENTER:ALL FLUSHED!",
                        (true, false) => "ENTER:THE THRONE AWAITS",
                        _ => "ENTER:NEXT  ESC:LEVELS",
                    };
                    p.spawn((label(f, prompt, 8.0, DIM_CREAM), Node { margin: UiRect::top(px(8.0)), ..default() }));
                });
        });
}

fn input(
    action: Single<&ActionState<Action>>,
    mut current: ResMut<CurrentLevel>,
    mut next: ResMut<NextState<AppState>>,
    mut sfx_w: MessageWriter<PlaySfx>,
    solo: Option<Res<SoloRun>>,
    mut commands: Commands,
) {
    if action.just_pressed(&Action::Confirm) {
        sfx(&mut sfx_w, Sfx::MenuSelect);
        // NAT ALONE: on to its next level, or back to its shelf after the last.
        if let Some(s) = solo {
            if s.0 + 1 < SOLO_COUNT {
                solo::start(&mut commands, s.0 + 1);
            } else {
                next.set(AppState::LevelSelect);
            }
        } else if current.0 + 1 >= LEVEL_COUNT {
            next.set(AppState::Victory);
        } else {
            current.0 += 1;
            next.set(AppState::Playing);
        }
    } else if action.just_pressed(&Action::Back) {
        sfx(&mut sfx_w, Sfx::MenuMove);
        next.set(AppState::LevelSelect);
    }
}
