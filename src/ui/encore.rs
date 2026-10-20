//! The encore's words on screen (`game::encore`): ENCORE! and how to call the band in the lap,
//! FLUSH! when Nat goes in (NAT ALONE: SLAM!), then TO BE CONTINUED... (NAT ALONE, with World 4
//! still shut: how to open it).

use bevy::prelude::*;

use super::{palette::*, *};
use crate::game::encore::{Encore, EncorePhase, Gag};
use crate::save::Progress;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (spawn.run_if(resource_added::<Encore>), despawn.run_if(resource_removed::<Encore>), show).chain());
}

#[derive(Component)]
struct EncoreText;

/// How far up from the bottom the words sit (virtual px): over the stage's floor, clear of the
/// band's toasts, the throne and Han's bubble.
const BOTTOM: f32 = 22.0;

/// The top line (ENCORE!) and the middle one (FLUSH!, TO BE CONTINUED...).
#[derive(Component)]
struct Top;
#[derive(Component)]
struct Hint;
#[derive(Component)]
struct Mid;
/// Under the middle line: NAT ALONE's way into World 4, while it's shut.
#[derive(Component)]
struct Sub;

fn spawn(mut commands: Commands, font: Res<UiFont>) {
    let f = &*font;
    commands
        .spawn((Name::new("Encore"), EncoreText, Node { justify_content: JustifyContent::FlexEnd, ..fullscreen() }, GlobalZIndex(4)))
        .with_children(|root| {
            root.spawn((label(f, "ENCORE!", 16.0, GOLD), Top));
            root.spawn((label(f, "DOWN: CALL THE BAND", 8.0, CREAM), Hint, Node { margin: UiRect::new(px(0.0), px(0.0), px(6.0), px(BOTTOM)), ..default() }));
            root.spawn(Node { justify_content: JustifyContent::FlexEnd, ..fullscreen() }).with_children(|m| {
                m.spawn((label(f, "", 24.0, CREAM), Mid, Node { margin: UiRect::bottom(px(8.0)), ..default() }));
                m.spawn((label(f, "", 8.0, GOLD), Sub, Node { margin: UiRect::bottom(px(BOTTOM + 4.0)), ..default() }));
            });
        });
}

fn despawn(mut commands: Commands, q: Query<Entity, With<EncoreText>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

#[allow(clippy::type_complexity)]
fn show(
    time: Res<Time>,
    encore: Option<Res<Encore>>,
    mut top: Query<(&mut Text, &mut Visibility, &mut TextColor), (With<Top>, Without<Hint>, Without<Mid>)>,
    mut hint: Query<&mut Visibility, (With<Hint>, Without<Top>, Without<Mid>)>,
    mut mid: Query<(&mut Text, &mut TextFont, &mut TextColor), (With<Mid>, Without<Top>, Without<Hint>, Without<Sub>)>,
    mut sub: Query<&mut Text, (With<Sub>, Without<Top>, Without<Hint>, Without<Mid>)>,
    progress: Option<Res<Progress>>,
) {
    let Some(e) = encore else { return };
    let lap = matches!(e.phase, EncorePhase::Lap | EncorePhase::Ending);
    if let Ok((mut text, mut vis, mut color)) = top.single_mut() {
        let want = match e.calls {
            0 | 1 => "ENCORE!".to_string(),
            n => format!("ENCORE! x{n}"),
        };
        if lap && text.0 != want {
            text.0 = want;
        }
        // It throbs gold to cream, faster with every call.
        let k = (time.elapsed_secs() * (4.0 + e.calls.min(12) as f32)).sin() * 0.5 + 0.5;
        color.0 = GOLD.mix(&CREAM, k * 0.6);
        vis.set_if_neq(if lap { Visibility::Inherited } else { Visibility::Hidden });
    }
    if let Ok(mut vis) = hint.single_mut() {
        // (Until it's sunk in.)
        let show = e.phase == EncorePhase::Lap && e.calls < 2;
        vis.set_if_neq(if show { Visibility::Inherited } else { Visibility::Hidden });
    }
    if let Ok((mut text, mut font, mut color)) = mid.single_mut() {
        let (want, size, c) = match e.phase {
            EncorePhase::Flushing => ("FLUSH!", 24.0, Color::srgb(0.6, 0.85, 1.0)),
            EncorePhase::Slam if e.t >= crate::game::encore::SHUT_SECS => ("SLAM!", 24.0, GOLD),
            EncorePhase::Continued => ("TO BE CONTINUED...", 16.0, CREAM),
            _ => ("", 24.0, CREAM),
        };
        if text.0 != want {
            text.0 = want.into();
            font.font_size = bevy::text::FontSize::Px(size);
            color.0 = c;
        }
    }
    if let Ok(mut text) = sub.single_mut() {
        let shut = progress.is_some_and(|p| !p.world_4_unlocked());
        let want = if e.gag == Gag::Grate && e.phase == EncorePhase::Continued && shut { "FIND ALL TEN GRATES" } else { "" };
        if text.0 != want {
            text.0 = want.into();
        }
    }
}
