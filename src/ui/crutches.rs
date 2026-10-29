//! The crutch pit's rescue (`game::crutches`) on screen: CRACK! as Nat lands, then the hospital
//! card (St. Plunger's: the diagnosis, the prescription, NO TOOTING!).

use bevy::prelude::*;

use super::{UiFont, column, icon, label, panel, palette::*};
use crate::art::{SpriteId, Sprites};
use crate::game::crutches::{CrutchRescue, RescuePhase};

/// CRACK! shows this long after the landing (s).
const CRACK_SECS: f32 = 0.9;
const RED: Color = Color::srgb(0.95, 0.25, 0.25);

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            spawn_screen.run_if(resource_added::<CrutchRescue>),
            update_screen.run_if(resource_exists::<CrutchRescue>),
            despawn_screen.run_if(resource_removed::<CrutchRescue>),
        )
            .chain()
            .run_if(resource_exists::<UiFont>),
    );
}

#[derive(Component)]
struct RescueScreen;

#[derive(Component)]
struct Crack;

#[derive(Component)]
struct Hospital;

fn spawn_screen(mut commands: Commands, f: Res<UiFont>, sprites: Option<Res<Sprites>>) {
    let f = &*f;
    commands
        .spawn((Name::new("RescueScreen"), RescueScreen, DespawnOnExit(crate::state::AppState::Playing), super::fullscreen(), GlobalZIndex(20)))
        .with_children(|root| {
            root.spawn((Crack, Node { position_type: PositionType::Absolute, top: px(40.0), ..default() }, Visibility::Hidden))
                .with_child(label(f, "CRACK!", 24.0, CREAM));
            root.spawn((Hospital, Visibility::Hidden, Node { position_type: PositionType::Absolute, ..super::fullscreen() }, BackgroundColor(OVERLAY)))
                .with_children(|h| {
                    h.spawn(panel(Node { padding: UiRect::axes(px(14.0), px(10.0)), ..column(8.0, 6.0) })).with_children(|p| {
                        p.spawn(label(f, "ST. PLUNGER'S HOSPITAL", 8.0, GOLD));
                        p.spawn(icon(sprites.as_deref(), SpriteId::NatOnCrutches, 48.0, 48.0));
                        p.spawn(label(f, "DIAGNOSIS: BOTH LEGS BROKEN", 8.0, CREAM));
                        p.spawn(label(f, "PRESCRIPTION: CRUTCHES", 8.0, CREAM));
                        p.spawn(label(f, "NO TOOTING!", 8.0, RED));
                    });
                });
        });
}

fn update_screen(
    rescue: Res<CrutchRescue>,
    mut crack: Query<&mut Visibility, (With<Crack>, Without<Hospital>)>,
    mut hospital: Query<&mut Visibility, (With<Hospital>, Without<Crack>)>,
) {
    let vis = |on: bool| if on { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut crack {
        v.set_if_neq(vis(rescue.phase == RescuePhase::Ouch && rescue.t < CRACK_SECS));
    }
    for mut v in &mut hospital {
        v.set_if_neq(vis(rescue.phase == RescuePhase::Hospital));
    }
}

fn despawn_screen(mut commands: Commands, q: Query<Entity, With<RescueScreen>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}
