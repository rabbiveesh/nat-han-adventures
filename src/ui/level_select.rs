//! Level select: a 5x2 grid of levels; details of the highlighted one below.
//! Hidden cheat: U unlocks everything.

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::{palette::*, *};
use crate::input::Action;
use crate::level::{LEVEL_COUNT, Levels};
use crate::save::Progress;
use crate::state::{AppState, CurrentLevel};

const COLS: usize = 5;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::LevelSelect), init_cursor).add_systems(
        Update,
        (
            spawn.run_if(not(any_with_component::<LevelSelectScreen>)),
            (input, cheat, highlight).chain(),
        )
            .chain()
            .run_if(in_state(AppState::LevelSelect)),
    );
}

/// The highlighted level.
#[derive(Resource, Debug, Clone, Copy, Default)]
struct Cursor(usize);

#[derive(Component)]
struct LevelSelectScreen;

#[derive(Component)]
struct Cell(usize);

#[derive(Component)]
enum Detail {
    World,
    Name,
    Best,
}

fn init_cursor(mut commands: Commands, current: Res<CurrentLevel>, progress: Res<Progress>) {
    // Start on the last level played, or the newest unlocked one.
    let i = if current.0 > 0 { current.0 } else { progress.unlocked - 1 };
    commands.insert_resource(Cursor(i.min(LEVEL_COUNT - 1)));
}

fn spawn(
    mut commands: Commands,
    font: Res<UiFont>,
    sprites: Option<Res<Sprites>>,
    progress: Res<Progress>,
    levels: Res<Levels>,
) {
    let sprites = sprites.as_deref();
    let f = &*font;
    commands
        .spawn((
            Name::new("LevelSelect"),
            LevelSelectScreen,
            DespawnOnExit(AppState::LevelSelect),
            fullscreen(),
        ))
        .with_children(|root| {
            root.spawn((label(f, "PICK A PIPE", 16.0, GOLD), Node { margin: UiRect::bottom(px(6.0)), ..default() }));
            root.spawn((label(f, "", 8.0, DIM_CREAM), Detail::World, Node { margin: UiRect::bottom(px(8.0)), ..default() }));
            // The grid.
            root.spawn(Node {
                display: Display::Grid,
                grid_template_columns: RepeatedGridTrack::px(COLS as u16, 48.0),
                row_gap: px(6.0),
                column_gap: px(6.0),
                ..default()
            })
            .with_children(|grid| {
                for i in 0..LEVEL_COUNT {
                    let unlocked = progress.is_unlocked(i);
                    grid.spawn((
                        Cell(i),
                        Node {
                            height: px(36.0),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceEvenly,
                            border: UiRect::all(px(2.0)),
                            ..default()
                        },
                        BackgroundColor(if unlocked { BROWN } else { LOCKED }),
                        BorderColor::all(INK),
                        Outline::new(px(1.0), px(0.0), Color::NONE),
                    ))
                    .with_children(|cell| {
                        cell.spawn(label(f, format!("{}", i + 1), 16.0, if unlocked { CREAM } else { DIM_CREAM }));
                        if !unlocked {
                            cell.spawn(icon(sprites, SpriteId::IconLock, 8.0, 8.0));
                        } else if let Some(best) = progress.best_nuggets[i] {
                            let total = levels.0.get(i).map_or(0, |l| l.nugget_count());
                            let color = if best as usize >= total { GOLD } else { CREAM };
                            cell.spawn(label(f, format!("{best}/{total}"), 8.0, color));
                        } else {
                            cell.spawn(label(f, "NEW", 8.0, GREEN));
                        }
                    });
                }
            });
            // Details of the highlighted level.
            root.spawn(panel(Node { margin: UiRect::top(px(10.0)), width: px(272.0), ..column(6.0, 6.0) }))
            .with_children(|p| {
                p.spawn((label(f, "", 8.0, GOLD), Detail::Name));
                p.spawn((label(f, "", 8.0, CREAM), Detail::Best));
            });
            root.spawn((
                label(f, "ARROWS:MOVE  ENTER:GO  ESC:BACK", 8.0, DIM_CREAM),
                Node { margin: UiRect::top(px(10.0)), ..default() },
            ));
        });
}

fn input(
    action: Single<&ActionState<Action>>,
    mut cursor: ResMut<Cursor>,
    progress: Res<Progress>,
    mut current: ResMut<CurrentLevel>,
    mut next: ResMut<NextState<AppState>>,
    mut sfx_w: MessageWriter<PlaySfx>,
) {
    let d = nav(&action);
    if d != IVec2::ZERO {
        cursor.0 = step(cursor.0, d);
        sfx(&mut sfx_w, Sfx::MenuMove);
    }
    if action.just_pressed(&Action::Confirm) && progress.is_unlocked(cursor.0) {
        sfx(&mut sfx_w, Sfx::MenuSelect);
        current.0 = cursor.0;
        next.set(AppState::Playing);
    } else if action.just_pressed(&Action::Back) {
        sfx(&mut sfx_w, Sfx::MenuMove);
        next.set(AppState::Title);
    }
}

/// Move around the 5x2 grid, wrapping.
fn step(i: usize, d: IVec2) -> usize {
    let n = LEVEL_COUNT as i32;
    let i = i as i32 + d.x + d.y * COLS as i32;
    i.rem_euclid(n) as usize
}

fn cheat(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mut progress: ResMut<Progress>,
    mut commands: Commands,
    font: Res<UiFont>,
    screen: Query<Entity, With<LevelSelectScreen>>,
    mut sfx_w: MessageWriter<PlaySfx>,
) {
    if !keys.is_some_and(|k| k.just_pressed(KeyCode::KeyU)) || progress.unlocked >= LEVEL_COUNT {
        return;
    }
    progress.unlocked = LEVEL_COUNT;
    sfx(&mut sfx_w, Sfx::MenuSelect);
    spawn_toast(&mut commands, &font, "CHEATER! ALL PIPES UNBLOCKED", 2.5);
    // Rebuild the grid with the new locks.
    for e in &screen {
        commands.entity(e).despawn();
    }
}

fn highlight(
    cursor: Res<Cursor>,
    progress: Res<Progress>,
    levels: Res<Levels>,
    mut cells: Query<(&Cell, &mut BorderColor, &mut Outline)>,
    mut details: Query<(&Detail, &mut Text)>,
) {
    for (cell, mut border, mut outline) in &mut cells {
        let on = cell.0 == cursor.0;
        *border = BorderColor::all(if on { GOLD } else { INK });
        outline.color = if on { CREAM } else { Color::NONE };
    }
    let i = cursor.0;
    let level = levels.0.get(i);
    for (d, mut text) in &mut details {
        let s = match d {
            Detail::World => {
                let w = level.map_or(1, |l| l.world);
                format!("WORLD {w}: {}", world_name(w))
            }
            Detail::Name => format!("{}. {}", i + 1, level.map_or("???", |l| l.name.as_str()).to_uppercase()),
            Detail::Best => {
                if !progress.is_unlocked(i) {
                    format!("LOCKED: BEAT LEVEL {i} FIRST")
                } else if let (Some(n), Some(t)) = (progress.best_nuggets[i], progress.best_time[i]) {
                    let total = level.map_or(0, |l| l.nugget_count());
                    format!("BEST: {n}/{total} NUGGETS  {}", format_time(t))
                } else {
                    "NOT FLUSHED YET".to_string()
                }
            }
        };
        if text.0 != s {
            text.0 = s;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_navigation_wraps() {
        assert_eq!(step(0, IVec2::X), 1);
        assert_eq!(step(0, -IVec2::X), 9);
        assert_eq!(step(2, IVec2::Y), 7);
        assert_eq!(step(7, IVec2::Y), 2);
        assert_eq!(step(7, -IVec2::Y), 2);
        assert_eq!(step(9, IVec2::X), 0);
    }
}
