//! Title screen: big title, the stars bobbing, blinking "PRESS ENTER", and the choice of
//! STORY (level select), FREE PLAY, or SIDE B and NAT ALONE once the story's last level is
//! cleared.

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::{palette::*, *};
use crate::input::Action;
use crate::save::Progress;
use crate::state::{AppState, Shelf};

pub fn plugin(app: &mut App) {
    // Spawned from Update (not OnEnter): the initial state's OnEnter runs before PreStartup,
    // i.e. before the sprites exist.
    app.add_systems(
        Update,
        (
            spawn.run_if(in_state(AppState::Title).and_then(not(any_with_component::<TitleScreen>))),
            (bob, input, highlight).chain().run_if(in_state(AppState::Title)),
        ),
    );
}

#[derive(Component)]
struct TitleScreen;

/// The menu: story mode, free play, Side B (once the story's cleared) and NAT ALONE (once
/// Side B's cleared).
const OPTIONS: [&str; 4] = ["STORY", "FREE PLAY", "SIDE B", "NAT ALONE"];

/// How many of `OPTIONS` the title shows.
fn shown(progress: Option<&Progress>) -> usize {
    match progress {
        Some(p) if p.solo_world_unlocked() => 4,
        Some(p) if p.side_b_unlocked() => 3,
        _ => 2,
    }
}

#[derive(Resource, Default)]
struct TitleCursor(usize, Vec<i32>);

/// The hidden way into Side B (`game::sideb`) before it's unlocked: Up Up Down Down on the title.
const SIDE_B_COMBO: [i32; 4] = [-1, -1, 1, 1];
/// ...and into NAT ALONE (`game::solo`): Down Down Up Up.
const SOLO_COMBO: [i32; 4] = [1, 1, -1, -1];

#[derive(Component)]
struct TitleOption(usize);

/// Bobs up and down; `phase` offsets the cycle so the two stars bounce out of step.
#[derive(Component)]
struct Bob {
    phase: f32,
}

fn spawn(mut commands: Commands, font: Res<UiFont>, sprites: Option<Res<Sprites>>, progress: Option<Res<Progress>>) {
    let sprites = sprites.as_deref();
    let f = &*font;
    commands
        .spawn((Name::new("TitleScreen"), TitleScreen, DespawnOnExit(AppState::Title), fullscreen()))
        .with_children(|root| {
            root.spawn(Node { flex_grow: 1.0, ..default() });
            root.spawn(label(f, GAME_TITLE, 24.0, GOLD));
            root.spawn((label(f, GAME_SUBTITLE, 16.0, GOLD), Node { margin: UiRect::top(px(8.0)), ..default() }));
            // The stars, bobbing on a little stage.
            root.spawn(Node {
                margin: UiRect::vertical(px(12.0)),
                column_gap: px(32.0),
                align_items: AlignItems::End,
                ..default()
            })
            .with_children(|row| {
                for (id, name, phase) in
                    [(SpriteId::PooIdle, HERO_NAME, 0.0), (SpriteId::HanIdle, SIDEKICK_NAME, 0.5)]
                {
                    row.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: px(8.0),
                        ..default()
                    })
                    .with_children(|c| {
                        c.spawn((icon(sprites, id, 32.0, 32.0), Bob { phase }));
                        c.spawn(label(f, name.to_uppercase(), 8.0, CREAM));
                    });
                }
            });
            root.spawn(label(f, GAME_TAGLINE, 8.0, DIM_CREAM));
            root.spawn((
                label(f, "PRESS ENTER", 8.0, GOLD),
                Node { margin: UiRect::top(px(12.0)), ..default() },
                Blink(1.0),
            ));
            for (i, o) in OPTIONS.iter().enumerate().take(shown(progress.as_deref())) {
                root.spawn((label(f, *o, 8.0, CREAM), TitleOption(i), Node { margin: UiRect::top(px(4.0)), ..default() }));
            }
            root.spawn(Node { flex_grow: 1.0, ..default() });
            root.spawn((
                label(f, "a game by their #2 fan", 8.0, DIM_CREAM),
                Node { margin: UiRect::bottom(px(8.0)), ..default() },
            ));
        });
}

fn bob(time: Res<Time>, mut q: Query<(&Bob, &mut Node)>) {
    let t = time.elapsed_secs();
    for (b, mut node) in &mut q {
        // Chunky 2px steps: 8-bit things don't move smoothly.
        let y = ((t * 3.0 + b.phase * std::f32::consts::TAU).sin() * 2.0).round() * 2.0;
        node.top = px(-y);
    }
}

fn input(
    action: Single<&ActionState<Action>>,
    cursor: Option<ResMut<TitleCursor>>,
    mut commands: Commands,
    mut next: ResMut<NextState<AppState>>,
    mut sfx_w: MessageWriter<PlaySfx>,
    progress: Option<Res<Progress>>,
) {
    let options = shown(progress.as_deref());
    let Some(mut cursor) = cursor else {
        commands.init_resource::<TitleCursor>();
        return;
    };
    let d = nav(&action).y;
    if d != 0 {
        cursor.0 = (cursor.0 as i32 + d).rem_euclid(options as i32) as usize;
        sfx(&mut sfx_w, Sfx::MenuMove);
        cursor.1.push(d);
        let n = cursor.1.len();
        if n > SIDE_B_COMBO.len() {
            cursor.1.drain(..n - SIDE_B_COMBO.len());
        }
        let shelf = match cursor.1.as_slice() {
            c if c == SIDE_B_COMBO => Some(Shelf::SideB),
            c if c == SOLO_COMBO => Some(Shelf::Solo),
            _ => None,
        };
        if let Some(shelf) = shelf {
            cursor.1.clear();
            sfx(&mut sfx_w, Sfx::MenuSelect);
            commands.insert_resource(shelf);
            next.set(AppState::LevelSelect);
            return;
        }
    }
    if nav(&action).x != 0 {
        cursor.1.clear();
    }
    if action.just_pressed(&Action::Confirm) {
        sfx(&mut sfx_w, Sfx::MenuSelect);
        match cursor.0 {
            1 => next.set(AppState::FreePlaySetup),
            i => {
                commands.insert_resource(match i {
                    0 => Shelf::Story,
                    2 => Shelf::SideB,
                    _ => Shelf::Solo,
                });
                next.set(AppState::LevelSelect);
            }
        }
    }
}

fn highlight(cursor: Option<Res<TitleCursor>>, mut q: Query<(&TitleOption, &mut Text, &mut TextColor)>) {
    let at = cursor.map_or(0, |c| c.0);
    for (o, mut text, mut color) in &mut q {
        let on = o.0 == at;
        let s = if on { format!("> {} <", OPTIONS[o.0]) } else { OPTIONS[o.0].to_string() };
        if text.0 != s {
            text.0 = s;
        }
        color.0 = if on { GOLD } else { DIM_CREAM };
    }
}
