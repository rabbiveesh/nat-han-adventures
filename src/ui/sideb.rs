//! Side B's set list strip (in place of the band's counter strip): one cell per call, the
//! horn for Giant Steps, the nugget for fired up, with the calls left; the picked one in a gold
//! frame with "DOWN" under it; the playing mode's timer drains under its cell. And the banner:
//! REWIND! when Han's done for, CLEAR! at the goal.

use bevy::prelude::*;

use super::{palette::*, *};
use crate::audio::{BandMeters, Harmony};
use crate::audio::director::SUMMON_ORDER;
use crate::game::sideb::{MELT_SECS, Phase, SetCall, SideB};
use crate::state::AppState;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Playing), spawn.run_if(resource_exists::<SideB>))
        .add_systems(Update, update.run_if(in_state(AppState::Playing).and_then(resource_exists::<SideB>)));
}

#[derive(Component)]
struct CallCell(usize);

#[derive(Component)]
struct CallCount(usize);

#[derive(Component)]
struct CallTimer(usize);

#[derive(Component)]
struct CallPick(usize);

#[derive(Component)]
struct Banner;

fn icon_of(c: SetCall) -> SpriteId {
    match c {
        SetCall::Mode(Harmony::Quartal) => SpriteId::IconNugget,
        SetCall::Mode(Harmony::Waltz) => SpriteId::IconNote,
        SetCall::Mode(_) => SpriteId::IconHorn,
        // A drop: melting.
        SetCall::Melt => SpriteId::IconSweat,
    }
}

fn spawn(mut commands: Commands, font: Res<UiFont>, sprites: Option<Res<Sprites>>, side: Res<SideB>) {
    let sprites = sprites.as_deref();
    commands
        .spawn((
            Name::new("SetList"),
            DespawnOnExit(AppState::Playing),
            Node {
                position_type: PositionType::Absolute,
                top: px(strip::STRIP_TOP),
                width: percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(2.0),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn(label(&font, "SET LIST", 8.0, DIM_CREAM));
            root.spawn((
                Node {
                    padding: UiRect::axes(px(3.0), px(2.0)),
                    column_gap: px(4.0),
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(1.0)),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderColor::all(DARK_GOLD),
            ))
            .with_children(|row| {
                for (k, (h, _)) in side.setlist.iter().enumerate() {
                    row.spawn((
                        CallCell(k),
                        Node {
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            padding: UiRect::axes(px(3.0), px(2.0)),
                            row_gap: px(2.0),
                            border: UiRect::all(px(1.0)),
                            ..default()
                        },
                        BorderColor::all(Color::NONE),
                    ))
                    .with_children(|cell| {
                        cell.spawn(Node { column_gap: px(3.0), align_items: AlignItems::Center, ..default() }).with_children(|line| {
                            line.spawn(icon(sprites, icon_of(*h), 8.0, 8.0));
                            line.spawn((CallCount(k), label(&font, "", 8.0, CREAM)));
                        });
                        cell.spawn((Node { width: px(26.0), height: px(2.0), ..default() }, BackgroundColor(DARK_BROWN)))
                            .with_child((CallTimer(k), Node { width: percent(0.0), height: percent(100.0), ..default() }, BackgroundColor(GOLD)));
                    });
                }
            });
            root.spawn(Node { column_gap: px(4.0), ..default() }).with_children(|row| {
                for k in 0..side.setlist.len() {
                    row.spawn((CallPick(k), label(&font, "DOWN", 8.0, GOLD), Visibility::Hidden));
                }
            });
        });
    commands.spawn((
        Name::new("SideBBanner"),
        DespawnOnExit(AppState::Playing),
        fullscreen(),
        GlobalZIndex(40),
        Visibility::Hidden,
        Banner,
    ))
    .with_child(label(&font, "", 24.0, GOLD));
}

#[allow(clippy::type_complexity)]
fn update(
    side: Res<SideB>,
    meters: Option<Res<BandMeters>>,
    director: Option<Res<crate::audio::plugin::Director>>,
    run: Option<Res<crate::game::LevelRun>>,
    mut cells: Query<(&CallCell, &mut BorderColor, &mut BackgroundColor)>,
    mut counts: Query<(&CallCount, &mut Text, &mut TextColor)>,
    mut timers: Query<(&CallTimer, &mut Node)>,
    mut picks: Query<(&CallPick, &mut Visibility), Without<Banner>>,
    mut banner: Query<(&mut Visibility, &Children), With<Banner>>,
    mut texts: Query<&mut Text, Without<CallCount>>,
) {
    for (c, mut border, mut bg) in &mut cells {
        let picked = c.0 == side.selected;
        border.set_if_neq(BorderColor::all(if picked { GOLD } else { Color::NONE }));
        bg.set_if_neq(BackgroundColor(if picked { Color::srgb(0.35, 0.24, 0.08) } else { Color::NONE }));
    }
    for (c, mut text, mut color) in &mut counts {
        let left = side.left[c.0];
        let s = format!("x{left}");
        if text.0 != s {
            text.0 = s;
        }
        color.set_if_neq(TextColor(if left > 0 { CREAM } else { DIM_CREAM }));
    }
    for (c, mut node) in &mut timers {
        let share = match side.setlist[c.0].0 {
            SetCall::Mode(h) => meters
                .as_ref()
                .and_then(|m| SUMMON_ORDER.iter().position(|&s| s == h).and_then(|i| m.0.summons[i].active))
                .unwrap_or(0.0),
            SetCall::Melt => {
                let now = run.as_ref().map_or(0.0, |r| r.time);
                director.as_ref().and_then(|d| d.melt_until).map_or(0.0, |u| ((u - now) / MELT_SECS).clamp(0.0, 1.0))
            }
        };
        let w = percent(share * 100.0);
        if node.width != w {
            node.width = w;
        }
    }
    for (p, mut v) in &mut picks {
        let show = p.0 == side.selected && side.left[p.0] > 0 && side.phase == Phase::Playing;
        v.set_if_neq(if show { Visibility::Inherited } else { Visibility::Hidden });
    }
    let Ok((mut v, children)) = banner.single_mut() else { return };
    let text = match side.phase {
        Phase::Rewind { .. } => Some("REWIND!"),
        Phase::Cleared { .. } => Some("CLEAR!"),
        Phase::Playing => None,
    };
    v.set_if_neq(if text.is_some() { Visibility::Inherited } else { Visibility::Hidden });
    if let Some(t) = text
        && let Some(mut tx) = children.first().and_then(|&c| texts.get_mut(c).ok())
        && tx.0 != t
    {
        tx.0 = t.to_string();
    }
}
