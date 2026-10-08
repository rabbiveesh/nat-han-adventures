//! Level complete card, over the frozen level. Records progress (and so saves) on entry.

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::{palette::*, *};
use crate::game::LevelRun;
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
) {
    let outcome = record_run(&mut progress, current.0, &run);
    commands.insert_resource(LastResult { level: current.0, run: run.clone(), outcome });
}

fn spawn(mut commands: Commands, font: Res<UiFont>, sprites: Option<Res<Sprites>>, levels: Res<Levels>, result: Res<LastResult>) {
    let f = &*font;
    let r = &*result;
    let total = r.run.nuggets_total.max(levels.0.get(r.level).map_or(0, |l| l.nugget_count() as u32));
    let name = levels.0.get(r.level).map_or_else(|| "???".into(), |l| l.name.to_uppercase());
    let last = r.level + 1 >= LEVEL_COUNT;
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
                    p.spawn((label(f, format!("LEVEL {}: {name}", r.level + 1), 8.0, CREAM), Node { margin: UiRect::bottom(px(8.0)), ..default() }));
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
                        p.spawn(label(f, format!("LEVEL {} UNLOCKED!", r.level + 2), 8.0, GREEN));
                    }
                    let prompt = if last { "ENTER:THE THRONE AWAITS" } else { "ENTER:NEXT  ESC:LEVELS" };
                    p.spawn((label(f, prompt, 8.0, DIM_CREAM), Node { margin: UiRect::top(px(8.0)), ..default() }));
                });
        });
}

fn input(
    action: Single<&ActionState<Action>>,
    mut current: ResMut<CurrentLevel>,
    mut next: ResMut<NextState<AppState>>,
    mut sfx_w: MessageWriter<PlaySfx>,
) {
    if action.just_pressed(&Action::Confirm) {
        sfx(&mut sfx_w, Sfx::MenuSelect);
        if current.0 + 1 >= LEVEL_COUNT {
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
