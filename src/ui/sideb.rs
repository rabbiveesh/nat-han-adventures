//! Side B's set list strip (in place of the band's counter strip): one cell per call, the
//! horn for Giant Steps, the nugget for fired up, with its key (1/2/3: the calls are free); the
//! last one called in a gold frame, its timer draining under it. Under the strip, the band's
//! balance (where Han's heading: he follows the sound) and the bar's beats, the one a hit
//! will land on in gold, flashing as it lands. On a phone the strip sits along the bottom, a
//! cell over each third of the screen: a tap on one calls it (index.html). And the banner:
//! OOPS! when Han's done for, CLEAR! at the goal.

use bevy::prelude::*;

use super::{palette::*, *};
use crate::audio::{BandMeters, Harmony};
use crate::audio::director::SUMMON_ORDER;
use crate::game::sideb::{HOLD_SECS, MELT_SECS, Phase, SetCall, SideB};
use crate::state::AppState;
use crate::touch::TouchMode;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Playing), spawn.run_if(resource_exists::<SideB>))
        .add_systems(
            Update,
            (respawn, update).chain().run_if(in_state(AppState::Playing).and_then(resource_exists::<SideB>)),
        )
        .add_systems(Update, clear_away.run_if(not(resource_exists::<SideB>)))
        .add_systems(Update, (mark_page, lift_readout));
}

/// On a phone the set list takes the bottom row: the band's readout sits over it.
fn lift_readout(side: Option<Res<SideB>>, touch: Option<Res<TouchMode>>, mut panel: Query<&mut Node, With<super::hud::BandReadoutPanel>>) {
    let up = side.is_some() && touch.is_some_and(|t| t.0);
    for mut node in &mut panel {
        let bottom = px(if up { 44.0 } else { 6.0 });
        if node.bottom != bottom {
            node.bottom = bottom;
        }
    }
}

/// Tell the page when Side B is on (`<body class="sideb">`): on a phone its touch controls
/// become the three call zones.
fn mark_page(side: Option<Res<SideB>>, mut on: Local<bool>) {
    if side.is_some() == *on {
        return;
    }
    *on = side.is_some();
    #[cfg(target_arch = "wasm32")]
    if let Some(body) = web_sys::window().and_then(|w| w.document()).and_then(|d| d.body()) {
        let _ = body.class_list().toggle_with_force("sideb", *on);
    }
}

/// The set list strip, for Side B level `.0`.
#[derive(Component)]
struct SetList(usize);

/// Back to Nat after a free play Side B room: the set list goes.
#[allow(clippy::type_complexity)]
fn clear_away(mut commands: Commands, strip: Query<Entity, Or<(With<SetList>, With<Banner>)>>) {
    for e in &strip {
        commands.entity(e).despawn();
    }
}

/// On to the next level (or into a free play Side B room): its set list.
fn respawn(
    mut commands: Commands,
    font: Res<UiFont>,
    sprites: Option<Res<Sprites>>,
    side: Res<SideB>,
    touch: Option<Res<TouchMode>>,
    strip: Query<(Entity, &SetList)>,
    banner: Query<Entity, With<Banner>>,
) {
    if !strip.is_empty() && strip.iter().all(|(_, s)| s.0 == side.level) {
        return;
    }
    for e in strip.iter().map(|(e, _)| e).chain(&banner) {
        commands.entity(e).despawn();
    }
    spawn(commands, font, sprites, side, touch);
}

#[derive(Component)]
struct CallCell(usize);

#[derive(Component)]
struct CallCount(#[allow(dead_code)] usize);

#[derive(Component)]
struct CallTimer(usize);


#[derive(Component)]
struct Banner;

/// The balance meter's knob.
#[derive(Component)]
struct PanKnob;

/// The record's state: playing, held, scratched back.
#[derive(Component)]
struct RecordLabel;

/// The bar's beat `.0`.
#[derive(Component)]
struct BeatDot(u8);

/// The record's states, on the strip.
const RECORD_PLAY: &str = "> PLAY";
const RECORD_HOLD: &str = "|| HOLD";
const RECORD_BACK: &str = "<< BACK";

/// Width of the balance meter (px).
const PAN_WIDTH: f32 = 56.0;

fn icon_of(c: SetCall) -> SpriteId {
    match c {
        SetCall::Mode(Harmony::Quartal) => SpriteId::IconNugget,
        SetCall::Mode(Harmony::Waltz) => SpriteId::IconNote,
        SetCall::Mode(_) => SpriteId::IconHorn,
        // A drop: melting.
        SetCall::Melt => SpriteId::IconSweat,
        SetCall::Nervous => SpriteId::IconJitters,
    }
}

fn spawn(mut commands: Commands, font: Res<UiFont>, sprites: Option<Res<Sprites>>, side: Res<SideB>, touch: Option<Res<TouchMode>>) {
    let sprites = sprites.as_deref();
    // (On a phone: along the bottom, a big cell over each third of the screen, its tap zone.)
    let phone = touch.is_some_and(|t| t.0);
    let (place, size) = if phone {
        (Node { bottom: px(3.0), ..default() }, 16.0)
    } else {
        (Node { top: px(strip::STRIP_TOP), ..default() }, 8.0)
    };
    commands
        .spawn((
            Name::new("SetList"),
            SetList(side.level),
            DespawnOnExit(AppState::Playing),
            Node {
                position_type: PositionType::Absolute,
                top: place.top,
                bottom: place.bottom,
                width: percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(2.0),
                ..default()
            },
        ))
        .with_children(|root| {
            if !phone {
                root.spawn(label(&font, "SET LIST", 8.0, DIM_CREAM));
            }
            root.spawn((
                Node {
                    width: if phone { percent(98.0) } else { Val::Auto },
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
                            flex_grow: if phone { 1.0 } else { 0.0 },
                            flex_basis: if phone { px(0.0) } else { Val::Auto },
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
                            line.spawn(icon(sprites, icon_of(*h), size, size));
                            let key = if phone { String::new() } else { format!("{}", k + 1) };
                            line.spawn((CallCount(k), label(&font, &key, size, CREAM)));
                        });
                        cell.spawn((Node { width: px(26.0 * size / 8.0), height: px(2.0), ..default() }, BackgroundColor(DARK_BROWN)))
                            .with_child((CallTimer(k), Node { width: percent(0.0), height: percent(100.0), ..default() }, BackgroundColor(GOLD)));
                    });
                }
            });
            // The balance (L, the meter, R) and the beats.
            root.spawn(Node { column_gap: px(6.0), align_items: AlignItems::Center, ..default() }).with_children(|row| {
                if side.on_record() {
                    row.spawn((RecordLabel, label(&font, RECORD_PLAY, 8.0, CREAM), Node { width: px(PAN_WIDTH + 24.0), ..default() }));
                } else {
                    row.spawn(label(&font, "L", 8.0, DIM_CREAM));
                    row.spawn((Node { width: px(PAN_WIDTH), height: px(4.0), ..default() }, BackgroundColor(DARK_BROWN))).with_child((
                        PanKnob,
                        Node { position_type: PositionType::Absolute, left: px(PAN_WIDTH - 4.0), top: px(-2.0), width: px(4.0), height: px(8.0), ..default() },
                        BackgroundColor(GOLD),
                    ));
                    row.spawn(label(&font, "R", 8.0, DIM_CREAM));
                }
                row.spawn(Node { width: px(6.0), ..default() });
                for b in 0..4 {
                    row.spawn((BeatDot(b), Node { width: px(5.0), height: px(5.0), ..default() }, BackgroundColor(DARK_BROWN)));
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
    mut timers: Query<(&CallTimer, &mut Node), (Without<PanKnob>, Without<BeatDot>)>,
    mut banner: Query<(&mut Visibility, &Children), With<Banner>>,
    mut texts: Query<&mut Text, (Without<CallCount>, Without<RecordLabel>)>,
    mut record: Query<(&mut Text, &mut TextColor), With<RecordLabel>>,
    mut knob: Query<&mut Node, (With<PanKnob>, Without<CallTimer>, Without<BeatDot>)>,
    mut dots: Query<(&BeatDot, &mut Node, &mut BackgroundColor), (Without<CallCell>, Without<PanKnob>, Without<CallTimer>)>,
    groove: Option<Res<crate::game::Groove>>,
    (time, mut landed, mut flash): (Res<Time>, MessageReader<crate::events::HitLanded>, Local<f32>),
) {
    let mut playing = [false; 3];
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
            SetCall::Nervous => {
                let now = run.as_ref().map_or(0.0, |r| r.time);
                director.as_ref().and_then(|d| d.nervous_until).map_or(0.0, |u| ((u - now) / HOLD_SECS).clamp(0.0, 1.0))
            }
        };
        let w = percent(share * 100.0);
        if node.width != w {
            node.width = w;
        }
        if let Some(p) = playing.get_mut(c.0) {
            *p = share > 0.0;
        }
    }
    for (c, mut border, mut bg) in &mut cells {
        let on = playing.get(c.0).copied().unwrap_or(false) && c.0 == side.selected;
        border.set_if_neq(BorderColor::all(if on { GOLD } else { Color::NONE }));
        bg.set_if_neq(BackgroundColor(if on { Color::srgb(0.35, 0.24, 0.08) } else { Color::NONE }));
    }
    if let Ok((mut text, mut color)) = record.single_mut() {
        let (t, c) = match side.pan_to {
            p if p > 0.5 => (RECORD_PLAY, CREAM),
            p if p < -0.25 => (RECORD_BACK, GOLD),
            _ => (RECORD_HOLD, GOLD),
        };
        if text.0 != t {
            text.0 = t.to_string();
        }
        color.set_if_neq(TextColor(c));
    }
    if let Ok(mut node) = knob.single_mut() {
        let x = px((side.pan + 1.0) / 2.0 * (PAN_WIDTH - 4.0));
        if node.left != x {
            node.left = x;
        }
    }
    if landed.read().count() > 0 {
        *flash = 0.2;
    }
    *flash = (*flash - time.delta_secs()).max(0.0);
    if let Some(g) = groove {
        let c = g.clock;
        let bpb = c.beats_per_bar.max(1);
        // (The hit lands on the beat after the one it was played on.)
        let target = side.hit_on.map(|(_, b)| (b + 1) % bpb);
        for (d, mut node, mut bg) in &mut dots {
            let shown = d.0 < bpb;
            let display = if shown { Display::Flex } else { Display::None };
            if node.display != display {
                node.display = display;
            }
            let color = if *flash > 0.0 && d.0 == c.beat {
                CREAM
            } else if Some(d.0) == target {
                GOLD
            } else if d.0 == c.beat {
                DIM_CREAM
            } else {
                DARK_BROWN
            };
            bg.set_if_neq(BackgroundColor(color));
        }
    }
    let Ok((mut v, children)) = banner.single_mut() else { return };
    let text = match side.phase {
        Phase::Rewind { .. } => Some("OOPS!"),
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
