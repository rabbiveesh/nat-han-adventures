//! The building code on screen (`game::code`): the secret paper, read (a card up top with the
//! code's arrows, for a moment: remember it!), and the keypad's little screen over its door
//! while Nat types (what's typed, then blanks).

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::text::{FontSize, FontSmoothing};
use bevy::window::PrimaryWindow;

use super::{palette::*, *};
use crate::game::code::{AtKeypad, CodeDoor, CodeRead};
use crate::state::AppState;

/// The code's card stays up this long (s).
const CARD_SECS: f32 = 4.0;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (show_code, card_time, keypad_screen).chain().run_if(in_state(AppState::Playing)));
}

#[derive(Component)]
struct CodeCard(f32);

fn show_code(mut commands: Commands, font: Res<UiFont>, mut read: MessageReader<CodeRead>, old: Query<Entity, With<CodeCard>>) {
    for r in read.read() {
        for e in &old {
            commands.entity(e).despawn();
        }
        let arrows: String = r.code.iter().map(|a| a.glyph().to_string()).collect::<Vec<_>>().join(" ");
        commands
            .spawn((
                Name::new("CodeCard"),
                CodeCard(0.0),
                DespawnOnExit(AppState::Playing),
                Node { justify_content: JustifyContent::Start, padding: UiRect::top(px(28.0)), ..fullscreen() },
                GlobalZIndex(25),
            ))
            .with_children(|root| {
                root.spawn(panel(Node { padding: UiRect::axes(px(12.0), px(8.0)), ..column(8.0, 6.0) })).with_children(|p| {
                    p.spawn(label(&font, "THE SECRET CODE", 8.0, GOLD));
                    p.spawn(label(&font, arrows, 16.0, CREAM));
                    p.spawn(label(&font, "SHHH! REMEMBER IT!", 8.0, GOLD));
                });
            });
    }
}

fn card_time(mut commands: Commands, time: Res<Time>, mut cards: Query<(Entity, &mut CodeCard)>) {
    for (e, mut c) in &mut cards {
        c.0 += time.delta_secs();
        if c.0 > CARD_SECS {
            commands.entity(e).despawn();
        }
    }
}

/// The keypad's screen, over the door: one slot an arrow.
#[derive(Component)]
struct KeypadScreen;

fn keypad_screen(
    mut commands: Commands,
    font: Res<UiFont>,
    active: Res<crate::game::ActiveLevel>,
    window: Option<Single<&Window, With<PrimaryWindow>>>,
    nat: Query<&AtKeypad>,
    doors: Query<&Transform, With<CodeDoor>>,
    mut screens: Query<(Entity, &mut Text2d), With<KeypadScreen>>,
) {
    let Some(pad) = nat.iter().next() else {
        for (e, _) in &screens {
            commands.entity(e).despawn();
        }
        return;
    };
    let slots: String = (0..active.level.code.len()).map(|i| pad.typed.get(i).map_or('_', |a| a.glyph()).to_string()).collect::<Vec<_>>().join(" ");
    if let Some((_, mut text)) = screens.iter_mut().next() {
        if text.0 != slots {
            text.0 = slots;
        }
        return;
    }
    let Some(door) = pad.door.and_then(|d| doors.get(d).ok()) else { return };
    let k = window.map_or(1.0, |w| (w.resolution.height() / VIRTUAL_HEIGHT).ceil().max(1.0));
    commands.spawn((
        Name::new("KeypadScreen"),
        KeypadScreen,
        DespawnOnExit(AppState::Playing),
        Text2d::new(slots),
        TextFont { font: font.0.clone().into(), font_size: FontSize::Px(8.0 * k), font_smoothing: FontSmoothing::None, ..default() },
        TextColor(Color::srgb(0.5, 1.0, 0.55)),
        TextBackgroundColor(Color::srgba(0.05, 0.05, 0.08, 0.85)),
        Anchor::BOTTOM_CENTER,
        Transform::from_translation((door.translation.truncate() + Vec2::Y * 36.0).extend(10.0)).with_scale(Vec3::splat(1.0 / k)),
    ));
}
