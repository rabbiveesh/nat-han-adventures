//! Pause menu (the game enters [`PlayState::Paused`] on Back).

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::{palette::*, *};
use crate::game::RestartLevel;
use crate::input::Action;
use crate::state::{AppState, PlayState};

const OPTIONS: [&str; 3] = ["RESUME", "RESTART", "LEVEL SELECT"];

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(PlayState::Paused), spawn)
        .add_systems(Update, (input, highlight).chain().run_if(in_state(PlayState::Paused)));
}

#[derive(Resource, Default)]
struct PauseCursor(usize);

#[derive(Component)]
struct PauseOption(usize);

fn spawn(mut commands: Commands, font: Res<UiFont>) {
    commands.insert_resource(PauseCursor(0));
    let f = &*font;
    commands
        .spawn((
            Name::new("PauseMenu"),
            DespawnOnExit(PlayState::Paused),
            fullscreen(),
            BackgroundColor(OVERLAY),
            GlobalZIndex(10),
        ))
        .with_children(|root| {
            root.spawn(panel(Node { padding: UiRect::axes(px(24.0), px(16.0)), ..column(16.0, 8.0) }))
                .with_children(|p| {
                    p.spawn((label(f, "PAUSED", 16.0, GOLD), Node { margin: UiRect::bottom(px(8.0)), ..default() }));
                    for (i, o) in OPTIONS.iter().enumerate() {
                        p.spawn((label(f, *o, 8.0, CREAM), PauseOption(i)));
                    }
                    p.spawn((
                        label(f, "(HOLD IT IN...)", 8.0, DIM_CREAM),
                        Node { margin: UiRect::top(px(8.0)), ..default() },
                    ));
                });
        });
}

fn input(
    action: Single<&ActionState<Action>>,
    mut cursor: ResMut<PauseCursor>,
    mut play: ResMut<NextState<PlayState>>,
    mut app_state: ResMut<NextState<AppState>>,
    mut restart: MessageWriter<RestartLevel>,
    mut sfx_w: MessageWriter<PlaySfx>,
) {
    let d = nav(&action).y;
    if d != 0 {
        cursor.0 = (cursor.0 as i32 + d).rem_euclid(OPTIONS.len() as i32) as usize;
        sfx(&mut sfx_w, Sfx::MenuMove);
    }
    if action.just_pressed(&Action::Back) {
        sfx(&mut sfx_w, Sfx::MenuSelect);
        play.set(PlayState::Running);
    } else if action.just_pressed(&Action::Confirm) {
        sfx(&mut sfx_w, Sfx::MenuSelect);
        match cursor.0 {
            0 => play.set(PlayState::Running),
            1 => {
                restart.write(RestartLevel);
                play.set(PlayState::Running);
            }
            _ => app_state.set(AppState::LevelSelect),
        }
    }
}

fn highlight(cursor: Res<PauseCursor>, mut q: Query<(&PauseOption, &mut Text, &mut TextColor)>) {
    for (o, mut text, mut color) in &mut q {
        let on = o.0 == cursor.0;
        let s = if on { format!("> {} <", OPTIONS[o.0]) } else { OPTIONS[o.0].to_string() };
        if text.0 != s {
            text.0 = s;
        }
        color.0 = if on { GOLD } else { CREAM };
    }
}
