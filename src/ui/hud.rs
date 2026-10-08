//! In-level HUD: nuggets, level name, timer, splats; the "LEVEL N" intro card; checkpoint toast;
//! the band toast when the music changes style to match how you play, and a badge naming the
//! physics mode the music is putting you in (Giant Steps, fired up, ...).

use bevy::prelude::*;

use super::{palette::*, *};
use crate::audio::{MusicChanged, NowPlaying};
use crate::events::CheckpointReached;
use crate::game::{Groove, LevelRun};
use crate::level::Levels;
use crate::state::{AppState, CurrentLevel};

/// How long the band toast stays up.
const BAND_SECS: f32 = 3.0;

/// How long the level intro card stays up.
const INTRO_SECS: f32 = 1.6;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Playing), (spawn_hud, spawn_intro)).add_systems(
        Update,
        (update_hud, update_groove_badge, tick_intro, checkpoint_toast, band_toast).run_if(in_state(AppState::Playing)),
    );
}

#[derive(Component)]
enum HudText {
    Nuggets,
    Name,
    Time,
    Splats,
}

#[derive(Component)]
struct IntroCard {
    age: f32,
}

fn level_name(levels: &Levels, i: usize) -> String {
    levels.0.get(i).map_or_else(|| "???".into(), |l| l.name.to_uppercase())
}

fn spawn_hud(
    mut commands: Commands,
    font: Res<UiFont>,
    sprites: Option<Res<Sprites>>,
    levels: Res<Levels>,
    current: Res<CurrentLevel>,
) {
    let f = &*font;
    commands
        .spawn((
            Name::new("Hud"),
            DespawnOnExit(AppState::Playing),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.0),
                padding: UiRect::all(px(8.0)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Start,
                ..default()
            },
        ))
        .with_children(|bar| {
            bar.spawn(Node { column_gap: px(4.0), align_items: AlignItems::Center, min_width: px(64.0), ..default() })
                .with_children(|n| {
                    n.spawn(icon(sprites.as_deref(), SpriteId::IconNugget, 8.0, 8.0));
                    n.spawn((label(f, "0/0", 8.0, GOLD), HudText::Nuggets));
                });
            bar.spawn((label(f, level_name(&levels, current.0), 8.0, CREAM), HudText::Name));
            bar.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::End,
                row_gap: px(4.0),
                min_width: px(64.0),
                ..default()
            })
            .with_children(|c| {
                c.spawn((label(f, "00:00", 8.0, CREAM), HudText::Time));
                c.spawn((label(f, "", 8.0, DIM_CREAM), HudText::Splats));
            });
        });
    spawn_groove_badge(&mut commands, f, sprites.as_deref());
}

/// Small persistent badge, top left under the nugget count: the mode the music has the physics in.
#[derive(Component)]
struct GrooveBadge;

#[derive(Component)]
struct GrooveBadgeText;

fn spawn_groove_badge(commands: &mut Commands, f: &UiFont, sprites: Option<&Sprites>) {
    commands
        .spawn((
            Name::new("GrooveBadge"),
            GrooveBadge,
            DespawnOnExit(AppState::Playing),
            Visibility::Hidden,
            Node {
                position_type: PositionType::Absolute,
                top: px(22.0),
                left: px(8.0),
                padding: UiRect::axes(px(4.0), px(3.0)),
                column_gap: px(4.0),
                align_items: AlignItems::Center,
                border: UiRect::all(px(1.0)),
                ..default()
            },
            BackgroundColor(PANEL),
            BorderColor::all(DARK_GOLD),
        ))
        .with_children(|b| {
            b.spawn(icon(sprites, SpriteId::Note, 6.0, 7.0));
            b.spawn((label(f, "", 8.0, GOLD), GrooveBadgeText));
        });
}

/// The badge's text for a groove ("" for normal physics). Several knobs can be on at once
/// (a harmony + just intonation's bounce).
pub fn groove_badge(g: &Groove) -> String {
    let mut parts = Vec::new();
    if g.giant_steps() {
        parts.push("GIANT STEPS");
    }
    if g.speed_scale > 1.0 {
        parts.push("FIRED UP");
    }
    if g.time_scale < 1.0 {
        parts.push("SLOW-MO");
    }
    if g.bounce {
        parts.push("BOUNCY");
    }
    parts.join(" + ")
}

fn update_groove_badge(
    groove: Option<Res<Groove>>,
    mut badge: Query<&mut Visibility, With<GrooveBadge>>,
    mut text: Query<&mut Text, With<GrooveBadgeText>>,
) {
    let s = groove.map_or_else(String::new, |g| groove_badge(&g));
    let vis = if s.is_empty() { Visibility::Hidden } else { Visibility::Inherited };
    for mut v in &mut badge {
        v.set_if_neq(vis);
    }
    for mut t in &mut text {
        if !s.is_empty() && t.0 != s {
            t.0 = s.clone();
        }
    }
}

fn update_hud(
    run: Res<LevelRun>,
    levels: Res<Levels>,
    current: Res<CurrentLevel>,
    mut q: Query<(&HudText, &mut Text)>,
) {
    for (h, mut text) in &mut q {
        let s = match h {
            HudText::Nuggets => format!("{}/{}", run.nuggets, run.nuggets_total),
            HudText::Name => level_name(&levels, current.0),
            HudText::Time => format_time(run.time),
            HudText::Splats if run.deaths == 0 => String::new(),
            HudText::Splats => format!("SPLATS {}", run.deaths),
        };
        if text.0 != s {
            text.0 = s;
        }
    }
}

fn spawn_intro(mut commands: Commands, font: Res<UiFont>, levels: Res<Levels>, current: Res<CurrentLevel>) {
    let f = &*font;
    let world = levels.0.get(current.0).map_or(1, |l| l.world);
    commands
        .spawn((
            Name::new("IntroCard"),
            IntroCard { age: 0.0 },
            DespawnOnExit(AppState::Playing),
            Node { padding: UiRect::bottom(px(48.0)), ..fullscreen() },
        ))
        .with_children(|root| {
            root.spawn(panel(Node { padding: UiRect::axes(px(16.0), px(8.0)), ..column(8.0, 8.0) }))
                .with_children(|p| {
                    p.spawn(label(f, format!("LEVEL {}", current.0 + 1), 16.0, GOLD));
                    p.spawn(label(f, level_name(&levels, current.0), 8.0, CREAM));
                    p.spawn(label(f, format!("WORLD {world}: {}", world_name(world)), 8.0, DIM_CREAM));
                });
        });
}

fn tick_intro(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut IntroCard)>) {
    for (e, mut card) in &mut q {
        card.age += time.delta_secs();
        if card.age >= INTRO_SECS {
            commands.entity(e).despawn();
        }
    }
}

fn checkpoint_toast(mut commands: Commands, font: Res<UiFont>, mut reader: MessageReader<CheckpointReached>) {
    if reader.read().last().is_some() {
        spawn_toast(&mut commands, &font, "CHECKPOINT!", 1.5);
    }
}

/// The band reacting to your play: why (gold) over what they switched to (cream), top of screen.
fn band_toast(
    mut commands: Commands,
    font: Res<UiFont>,
    mut reader: MessageReader<MusicChanged>,
    old: Query<Entity, With<BandToast>>,
) {
    let Some(change) = reader.read().last() else { return };
    let (why, what) = band_lines(&change.now);
    for e in &old {
        commands.entity(e).despawn();
    }
    commands
        .spawn((
            Name::new("BandToast"),
            BandToast,
            Toast { ttl: BAND_SECS },
            Node {
                position_type: PositionType::Absolute,
                top: px(28.0),
                width: percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            GlobalZIndex(20),
        ))
        .with_children(|t| {
            t.spawn(panel(Node {
                padding: UiRect::axes(px(8.0), px(4.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(4.0),
                ..default()
            }))
            .with_children(|p| {
                p.spawn(label(&font, why, 8.0, GOLD));
                p.spawn(label(&font, what, 8.0, CREAM));
            });
        });
}

#[derive(Component)]
struct BandToast;

/// The toast's two lines. Switching back to the plain arrangement gets its own line.
fn band_lines(now: &NowPlaying) -> (String, String) {
    let label = now.filters.label();
    if label.is_empty() {
        ("THE BAND CALMS DOWN".into(), "BACK TO THE CHART".into())
    } else if now.reason.is_empty() {
        ("THE BAND IS FEELING IT".into(), label)
    } else {
        (now.reason.to_string(), label)
    }
}
