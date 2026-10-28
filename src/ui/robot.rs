//! The robot pit's wrong game (`game::robot`), on screen the way Robot Buddy Adventure draws
//! it: "WRONG GAME!" as Nat lands, then Sparky's dialogue box (navy, a green border, his green
//! name tab, white typewriter text) with his battery beside his name; and the answer pads'
//! numbers.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::text::{FontSmoothing, FontSize};
use bevy::window::PrimaryWindow;

use super::{UiFont, VIRTUAL_HEIGHT, font, label, palette::*};
use crate::game::robot::{AnswerPad, BATTERY, RobotRoom, RoomPhase};

/// Sparky's colour in his game (his eyes, his name tab, his border).
const SPARKY_GREEN: Color = Color::srgb(0.0, 0.902, 0.463);
const BOX: Color = Color::srgba(0.078, 0.078, 0.157, 0.9);
const TAB_TEXT: Color = Color::srgb(0.102, 0.102, 0.18);
const DRAINED: Color = Color::srgb(0.25, 0.25, 0.32);
/// Typewriter speed, characters per second.
const REVEAL_CPS: f32 = 40.0;

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            spawn_screen.run_if(resource_added::<RobotRoom>),
            update_screen.run_if(resource_exists::<RobotRoom>),
            despawn_screen.run_if(resource_removed::<RobotRoom>),
            pad_numbers,
        )
            .chain()
            .run_if(resource_exists::<UiFont>),
    );
}

#[derive(Component)]
struct RobotScreen;

#[derive(Component)]
struct Splash;

#[derive(Component)]
struct Dialogue;

#[derive(Component)]
struct DialogueText {
    line: &'static str,
    age: f32,
}

#[derive(Component)]
struct BatteryCell(u8);

fn spawn_screen(mut commands: Commands, f: Res<UiFont>) {
    let f = &*f;
    commands
        .spawn((Name::new("RobotScreen"), RobotScreen, DespawnOnExit(crate::state::AppState::Playing), Node { ..super::fullscreen() }, GlobalZIndex(20)))
        .with_children(|root| {
            root.spawn((
                Splash,
                Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: px(8.0), ..default() },
                Visibility::Hidden,
            ))
            .with_children(|s| {
                s.spawn(label(f, "WRONG GAME!", 24.0, GOLD));
                s.spawn(label(f, "ROBOT BUDDY ADVENTURE", 8.0, CREAM));
            });
            // The dialogue box, pinned to the bottom: name tab across its top edge.
            root.spawn((
                Dialogue,
                Visibility::Hidden,
                Node {
                    position_type: PositionType::Absolute,
                    bottom: px(6.0),
                    width: percent(92.0),
                    max_width: px(360.0),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
            ))
            .with_children(|d| {
                d.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(6.0), margin: UiRect::left(px(8.0)), ..default() })
                    .with_children(|tab| {
                        tab.spawn((Node { padding: UiRect::axes(px(4.0), px(3.0)), ..default() }, BackgroundColor(SPARKY_GREEN)))
                            .with_child((Text::new("Sparky"), font(f, 8.0), TextColor(TAB_TEXT)));
                        // His battery.
                        tab.spawn((
                            Node { flex_direction: FlexDirection::Row, column_gap: px(1.0), padding: UiRect::all(px(1.0)), border: UiRect::all(px(1.0)), ..default() },
                            BorderColor::all(CREAM),
                            BackgroundColor(BOX),
                        ))
                        .with_children(|b| {
                            for i in 0..BATTERY {
                                b.spawn((BatteryCell(i), Node { width: px(4.0), height: px(5.0), ..default() }, BackgroundColor(SPARKY_GREEN)));
                            }
                        });
                    });
                d.spawn((
                    Node { padding: UiRect::axes(px(8.0), px(6.0)), min_height: px(36.0), border: UiRect::all(px(2.0)), ..default() },
                    BackgroundColor(BOX),
                    BorderColor::all(SPARKY_GREEN),
                ))
                .with_child((DialogueText { line: "", age: 0.0 }, Text::new(""), font(f, 8.0), TextColor(Color::WHITE), TextLayout::linebreak(LineBreak::WordBoundary)));
            });
        });
}

fn update_screen(
    time: Res<Time>,
    room: Res<RobotRoom>,
    mut splash: Query<&mut Visibility, (With<Splash>, Without<Dialogue>)>,
    mut dialogue: Query<&mut Visibility, (With<Dialogue>, Without<Splash>)>,
    mut text: Query<(&mut DialogueText, &mut Text)>,
    mut cells: Query<(&BatteryCell, &mut BackgroundColor)>,
) {
    let vis = |on: bool| if on { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut splash {
        v.set_if_neq(vis(room.phase == RoomPhase::Arrive));
    }
    let line = room.line();
    for mut v in &mut dialogue {
        v.set_if_neq(vis(line.is_some()));
    }
    for (mut d, mut t) in &mut text {
        let line = line.unwrap_or("");
        if d.line != line {
            d.line = line;
            d.age = 0.0;
        }
        d.age += time.delta_secs();
        let n = (d.age * REVEAL_CPS) as usize;
        let shown: String = line.chars().take(n).collect();
        if t.0 != shown {
            t.0 = shown;
        }
    }
    for (c, mut bg) in &mut cells {
        let want = if c.0 < room.battery { SPARKY_GREEN } else { DRAINED };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

fn despawn_screen(mut commands: Commands, q: Query<Entity, With<RobotScreen>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

/// Each answer pad's number, in world space (rasterized at screen resolution, scaled down:
/// crisp pixels).
fn pad_numbers(
    mut commands: Commands,
    pads: Query<(Entity, &AnswerPad), Added<AnswerPad>>,
    f: Res<UiFont>,
    window: Option<Single<&Window, With<PrimaryWindow>>>,
) {
    let k = window.map_or(1.0, |w| (w.resolution.height() / VIRTUAL_HEIGHT).ceil().max(1.0));
    for (e, pad) in &pads {
        commands.entity(e).with_child((
            Text2d::new(pad.value.to_string()),
            TextFont { font: f.0.clone().into(), font_size: FontSize::Px(8.0 * k), font_smoothing: FontSmoothing::None, ..default() },
            TextColor(Color::srgb(0.1, 0.1, 0.18)),
            Anchor::CENTER,
            Transform::from_xyz(0.5, 0.0, 0.1).with_scale(Vec3::splat(1.0 / k)),
        ));
    }
}
