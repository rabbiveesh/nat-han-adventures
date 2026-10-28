//! Level select: a 5x2 grid of levels; details of the highlighted one below. The story's
//! levels, Side B's or NAT ALONE's (the [`Shelf`]: nuggets for the story and NAT ALONE, fewest
//! rewinds for Side B).
//! Hidden cheat: U unlocks everything.

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::{palette::*, *};
use crate::input::Action;
use crate::level::{LEVEL_COUNT, Levels};
use crate::game::sideb::{self, SIDE_B_COUNT, SideBLevel};
use crate::game::solo::{self, SOLO_COUNT};
use crate::game::toybox::{self, TOY_COUNT};
use crate::save::Progress;
use crate::state::{AppState, CurrentLevel, Shelf};

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

/// How many levels the shelf has.
fn count(shelf: Shelf) -> usize {
    match shelf {
        Shelf::Story => LEVEL_COUNT,
        Shelf::SideB => SIDE_B_COUNT,
        Shelf::Solo => SOLO_COUNT,
        Shelf::Toybox => TOY_COUNT,
    }
}

fn unlocked(shelf: Shelf, progress: &Progress, i: usize) -> bool {
    match shelf {
        Shelf::Story => progress.is_unlocked(i),
        Shelf::SideB => i < progress.side_b_unlocked,
        Shelf::Solo => i < progress.solo_unlocked,
        Shelf::Toybox => true,
    }
}

fn init_cursor(mut commands: Commands, current: Res<CurrentLevel>, progress: Res<Progress>, shelf: Res<Shelf>) {
    // Start on the last level played, or the newest unlocked one.
    let i = match *shelf {
        Shelf::Story if current.0 > 0 => current.0,
        Shelf::Story => progress.unlocked - 1,
        Shelf::SideB => progress.side_b_unlocked - 1,
        Shelf::Solo => progress.solo_unlocked - 1,
        Shelf::Toybox => 0,
    };
    commands.insert_resource(Cursor(i.min(count(*shelf) - 1)));
}

fn spawn(
    mut commands: Commands,
    font: Res<UiFont>,
    sprites: Option<Res<Sprites>>,
    progress: Res<Progress>,
    levels: Res<Levels>,
    shelf: Res<Shelf>,
) {
    let shelf = *shelf;
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
            let title = match shelf {
                Shelf::Story => "PICK A PIPE",
                Shelf::SideB => "SIDE B",
                Shelf::Solo => "NAT ALONE",
                Shelf::Toybox => "SHHH! A SECRET",
            };
            root.spawn((label(f, title, 16.0, GOLD), Node { margin: UiRect::bottom(px(6.0)), ..default() }));
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
                for i in 0..count(shelf) {
                    let unlocked = unlocked(shelf, &progress, i);
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
                        } else if shelf == Shelf::SideB {
                            match progress.side_b_rewinds[i] {
                                Some(r) => cell.spawn(label(f, format!("R{r}"), 8.0, if r == 0 { GOLD } else { CREAM })),
                                None => cell.spawn(label(f, "NEW", 8.0, GREEN)),
                            };
                        } else if shelf == Shelf::Toybox {
                            cell.spawn(label(f, "TOY", 8.0, GREEN));
                        } else if shelf == Shelf::Solo {
                            match progress.solo_nuggets[i] {
                                Some(best) => {
                                    let total = solo::load(i).nugget_count();
                                    let color = if best as usize >= total { GOLD } else { CREAM };
                                    cell.spawn(label(f, format!("{best}/{total}"), 8.0, color))
                                }
                                None => cell.spawn(label(f, "NEW", 8.0, GREEN)),
                            };
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
    shelf: Res<Shelf>,
    mut commands: Commands,
) {
    let d = nav(&action);
    if d != IVec2::ZERO {
        cursor.0 = step(cursor.0, d, count(*shelf));
        sfx(&mut sfx_w, Sfx::MenuMove);
    }
    if action.just_pressed(&Action::Confirm) && unlocked(*shelf, &progress, cursor.0) {
        sfx(&mut sfx_w, Sfx::MenuSelect);
        match *shelf {
            Shelf::Story => {
                current.0 = cursor.0;
                next.set(AppState::Playing);
            }
            Shelf::SideB => sideb::start(&mut commands, cursor.0),
            Shelf::Solo => solo::start(&mut commands, cursor.0),
            Shelf::Toybox => toybox::start(&mut commands, cursor.0),
        }
    } else if action.just_pressed(&Action::Back) {
        sfx(&mut sfx_w, Sfx::MenuMove);
        next.set(AppState::Title);
    }
}

/// Move around the 5-wide grid of `n` levels, wrapping.
fn step(i: usize, d: IVec2, n: usize) -> usize {
    let n = n as i32;
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
    shelf: Res<Shelf>,
) {
    if !keys.is_some_and(|k| k.just_pressed(KeyCode::KeyU)) {
        return;
    }
    match *shelf {
        Shelf::Story if progress.unlocked < LEVEL_COUNT => progress.unlocked = LEVEL_COUNT,
        Shelf::SideB if progress.side_b_unlocked < SIDE_B_COUNT => progress.side_b_unlocked = SIDE_B_COUNT,
        Shelf::Solo if progress.solo_unlocked < SOLO_COUNT => progress.solo_unlocked = SOLO_COUNT,
        _ => return,
    }
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
    shelf: Res<Shelf>,
) {
    for (cell, mut border, mut outline) in &mut cells {
        let on = cell.0 == cursor.0;
        *border = BorderColor::all(if on { GOLD } else { INK });
        outline.color = if on { CREAM } else { Color::NONE };
    }
    let i = cursor.0;
    let level = levels.0.get(i);
    for (d, mut text) in &mut details {
        let s = if *shelf == Shelf::SideB {
            side_b_detail(d, &progress, i)
        } else if *shelf == Shelf::Solo {
            solo_detail(d, &progress, i)
        } else if *shelf == Shelf::Toybox {
            toy_detail(d, i)
        } else {
            match d {
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
            }
        };
        if text.0 != s {
            text.0 = s;
        }
    }
}

fn side_b_detail(d: &Detail, progress: &Progress, i: usize) -> String {
    match d {
        Detail::World => "HAN ALONE: YOU'RE THE BAND".to_string(),
        Detail::Name => format!("B{}. {}", i + 1, SideBLevel::name(i).to_uppercase()),
        Detail::Best => match progress.side_b_rewinds[i] {
            _ if i >= progress.side_b_unlocked => format!("LOCKED: BEAT B{i} FIRST"),
            Some(0) => "FLUSHED WITH NO REWINDS!".to_string(),
            Some(1) => "BEST: FLUSHED IN 1 REWIND".to_string(),
            Some(r) => format!("BEST: FLUSHED IN {r} REWINDS"),
            None => "NOT FLUSHED YET".to_string(),
        },
    }
}

fn solo_detail(d: &Detail, progress: &Progress, i: usize) -> String {
    match d {
        Detail::World => "NAT ALONE: THE GADGET WORLD".to_string(),
        Detail::Name => format!("{}. {}", i + 1, solo::name(i).to_uppercase()),
        Detail::Best => match (progress.solo_nuggets[i], progress.solo_time[i]) {
            _ if i >= progress.solo_unlocked => format!("LOCKED: BEAT LEVEL {i} FIRST"),
            (Some(n), Some(t)) => format!("BEST: {n}/{} NUGGETS  {}", solo::load(i).nugget_count(), format_time(t)),
            _ => "NOT FLUSHED YET".to_string(),
        },
    }
}

fn toy_detail(d: &Detail, i: usize) -> String {
    match d {
        Detail::World => "THE SECRET PLAYGROUND".to_string(),
        Detail::Name => format!("{}. {}", i + 1, toybox::name(i).to_uppercase()),
        Detail::Best => "THE SILLIEST IDEAS EVER".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_navigation_wraps() {
        assert_eq!(step(0, IVec2::X, LEVEL_COUNT), 1);
        assert_eq!(step(0, -IVec2::X, LEVEL_COUNT), 9);
        assert_eq!(step(2, IVec2::Y, LEVEL_COUNT), 7);
        assert_eq!(step(7, IVec2::Y, LEVEL_COUNT), 2);
        assert_eq!(step(7, -IVec2::Y, LEVEL_COUNT), 2);
        assert_eq!(step(9, IVec2::X, LEVEL_COUNT), 0);
    }
}
