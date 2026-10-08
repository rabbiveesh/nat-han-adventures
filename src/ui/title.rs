//! Title screen: big title, the stars bobbing, blinking "PRESS ENTER".

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::{palette::*, *};
use crate::input::Action;
use crate::state::AppState;

pub fn plugin(app: &mut App) {
    // Spawned from Update (not OnEnter): the initial state's OnEnter runs before PreStartup,
    // i.e. before the sprites exist.
    app.add_systems(
        Update,
        (
            spawn.run_if(in_state(AppState::Title).and_then(not(any_with_component::<TitleScreen>))),
            (bob, input).run_if(in_state(AppState::Title)),
        ),
    );
}

#[derive(Component)]
struct TitleScreen;

/// Bobs up and down; `phase` offsets the cycle so the two stars bounce out of step.
#[derive(Component)]
struct Bob {
    phase: f32,
}

fn spawn(mut commands: Commands, font: Res<UiFont>, sprites: Option<Res<Sprites>>) {
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
                margin: UiRect::vertical(px(16.0)),
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
                Node { margin: UiRect::top(px(24.0)), ..default() },
                Blink(1.0),
            ));
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
    mut next: ResMut<NextState<AppState>>,
    mut sfx_w: MessageWriter<PlaySfx>,
) {
    if action.just_pressed(&Action::Confirm) {
        sfx(&mut sfx_w, Sfx::MenuSelect);
        next.set(AppState::LevelSelect);
    }
}
