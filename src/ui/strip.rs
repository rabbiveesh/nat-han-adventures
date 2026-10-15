//! The band's counter strip, top centre under the level name: the horn (toots → Giant Steps),
//! the nugget (quick nuggets → fired up), the note (hops in threes → the waltz), then past a
//! divider the sweat drop (deaths → nervous, never callable). Pips fill as you play; a full
//! counter holds its charge; the full one a call (Down) would play gets a gold frame and the
//! CALL arrow under it; the called mode shows a draining timer instead of pips. Each counter
//! appears once it has been taught (`ui::lesson`, saved in [`Progress::taught`]).

use bevy::prelude::*;

use super::{palette::*, *};
use crate::audio::BandMeters;
use crate::audio::director::{Meter, SUMMON_ORDER};
use crate::save::{BandCounter, Progress};
use crate::state::AppState;

pub fn plugin(app: &mut App) {
    use crate::game::sideb::SideB;
    // (Side B has its set list instead: `ui::sideb`.)
    app.add_systems(OnEnter(AppState::Playing), spawn_strip.run_if(not(resource_exists::<SideB>)))
        .add_systems(Update, update_strip.run_if(in_state(AppState::Playing).and_then(not(resource_exists::<SideB>))))
        .add_systems(Update, hide_strip.run_if(resource_exists::<SideB>));
}

/// Where the strip's panel sits (virtual px from the top).
pub const STRIP_TOP: f32 = 19.0;
const PIP: f32 = 3.0;
const PIP_GAP: f32 = 1.0;
/// Unlit pips, and pips of counters that aren't the one in play.
const DIM_PIP: Color = Color::srgb(0.37, 0.28, 0.24);
/// The ready (or playing) counter's frame fill.
const READY_FILL: Color = Color::srgb(0.35, 0.24, 0.08);
const SWEAT_BLUE: Color = Color::srgb(0.62, 0.85, 1.0);

/// The pips a counter has.
fn pips_of(c: BandCounter) -> u32 {
    use crate::audio::director::{FIRED_UP_NUGGETS, GIANT_STEPS_TOOTS, NERVOUS_DEATHS};
    match c {
        BandCounter::Horn => GIANT_STEPS_TOOTS,
        BandCounter::Nugget => FIRED_UP_NUGGETS,
        BandCounter::Note => 3,
        BandCounter::Sweat => NERVOUS_DEATHS,
    }
}

fn icon_of(c: BandCounter) -> SpriteId {
    match c {
        BandCounter::Horn => SpriteId::IconHorn,
        BandCounter::Nugget => SpriteId::IconNugget,
        BandCounter::Note => SpriteId::IconNote,
        BandCounter::Sweat => SpriteId::IconSweat,
    }
}

/// A counter's width: the icon, a gap, the pips, a 2 px pad each side (inside its 1 px frame).
fn cell_width(c: BandCounter) -> f32 {
    8.0 + 2.0 + pips_of(c) as f32 * (PIP + PIP_GAP) - PIP_GAP + 6.0
}

/// The meter of `c` in `m`.
fn meter(m: &crate::audio::director::Meters, c: BandCounter) -> Meter {
    match c {
        BandCounter::Sweat => m.deaths,
        _ => m.summons[c.index()],
    }
}

#[derive(Component)]
pub struct Strip;

/// One counter's framed cell, and its spacer in the CALL row under the strip.
#[derive(Component)]
struct Cell(BandCounter);

#[derive(Component)]
struct CallSlot(BandCounter);

/// The arrow and "CALL" under the ready counter.
#[derive(Component)]
struct CallMark(BandCounter);

#[derive(Component)]
struct Pip(BandCounter, u32);

#[derive(Component)]
struct Pips(BandCounter);

/// The called mode's timer, in place of its pips.
#[derive(Component)]
struct ModeTimer(BandCounter);

#[derive(Component)]
struct TimerFill(BandCounter);

/// The divider before the sweat drop.
#[derive(Component)]
struct Divider;

fn spawn_strip(mut commands: Commands, font: Res<UiFont>, sprites: Option<Res<Sprites>>) {
    let sprites = sprites.as_deref();
    commands
        .spawn((
            Name::new("BandStrip"),
            Strip,
            DespawnOnExit(AppState::Playing),
            Visibility::Hidden,
            // Above a lesson card's dimming (it points at the strip).
            GlobalZIndex(31),
            Node {
                position_type: PositionType::Absolute,
                top: px(STRIP_TOP),
                width: percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|root| {
            // The panel with the counters.
            root.spawn((
                Node {
                    padding: UiRect::axes(px(3.0), px(1.0)),
                    column_gap: px(3.0),
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(1.0)),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderColor::all(DARK_GOLD),
            ))
            .with_children(|row| {
                for c in BandCounter::ALL {
                    if c == BandCounter::Sweat {
                        row.spawn((
                            Divider,
                            Node { width: px(1.0), height: px(8.0), display: Display::None, ..default() },
                            BackgroundColor(DARK_GOLD),
                        ));
                    }
                    let n = pips_of(c);
                    row.spawn((
                        Cell(c),
                        Node {
                            width: px(cell_width(c)),
                            height: px(12.0),
                            padding: UiRect::horizontal(px(2.0)),
                            column_gap: px(2.0),
                            align_items: AlignItems::Center,
                            border: UiRect::all(px(1.0)),
                            display: Display::None,
                            ..default()
                        },
                        BackgroundColor(Color::NONE),
                        BorderColor::all(Color::NONE),
                    ))
                    .with_children(|cell| {
                        cell.spawn(icon(sprites, icon_of(c), 8.0, 8.0));
                        cell.spawn((Pips(c), Node { column_gap: px(PIP_GAP), ..default() })).with_children(|p| {
                            for k in 0..n {
                                p.spawn((Pip(c, k), Node { width: px(PIP), height: px(PIP), ..default() }, BackgroundColor(DIM_PIP)));
                            }
                        });
                        let w = n as f32 * (PIP + PIP_GAP) - PIP_GAP;
                        cell.spawn((
                            ModeTimer(c),
                            Node { width: px(w), height: px(PIP), display: Display::None, ..default() },
                            BackgroundColor(DIM_PIP),
                        ))
                        .with_child((TimerFill(c), Node { width: percent(100.0), height: percent(100.0), ..default() }, BackgroundColor(CREAM)));
                    });
                }
            });
            // The CALL row: one slot per counter, as wide as its cell.
            root.spawn(Node { padding: UiRect::horizontal(px(4.0)), column_gap: px(3.0), ..default() }).with_children(|row| {
                for c in BandCounter::ALL {
                    if c == BandCounter::Sweat {
                        row.spawn((Divider, Node { width: px(1.0), display: Display::None, ..default() }));
                    }
                    row.spawn((
                        CallSlot(c),
                        Node {
                            width: px(cell_width(c)),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            display: Display::None,
                            ..default()
                        },
                    ))
                    .with_children(|slot| {
                        slot.spawn((CallMark(c), Visibility::Hidden, Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: px(1.0), ..default() }))
                            .with_children(|m| {
                                // A small arrow pointing up at the counter, then the word.
                                for w in [1.0, 3.0] {
                                    m.spawn((Node { width: px(w), height: px(1.0), ..default() }, BackgroundColor(GOLD)));
                                }
                                m.spawn((label(&font, "CALL", 8.0, GOLD), Node { margin: UiRect::top(px(1.0)), ..default() }));
                            });
                    });
                }
            });
        });
}

/// What the strip shows, from the band's meters and what's been taught.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StripView {
    /// Per [`BandCounter`]: shown, the meter.
    pub shown: [bool; 4],
    pub meters: [Meter; 4],
    /// The counter a call plays (shown counters only).
    pub ready: Option<BandCounter>,
}

pub fn strip_view(meters: &crate::audio::director::Meters, progress: &Progress) -> StripView {
    let shown = progress.taught;
    let m = BandCounter::ALL.map(|c| meter(meters, c));
    let ready = meters
        .ready
        .and_then(|h| SUMMON_ORDER.iter().position(|&s| s == h))
        .map(|i| BandCounter::ALL[i])
        .filter(|c| shown[c.index()]);
    StripView { shown, meters: m, ready }
}

#[allow(clippy::type_complexity)]
fn update_strip(
    meters: Option<Res<BandMeters>>,
    progress: Res<Progress>,
    mut strip: Query<&mut Visibility, (With<Strip>, Without<CallMark>)>,
    mut nodes: Query<(&mut Node, Option<&Cell>, Option<&CallSlot>, Option<&Pips>, Option<&ModeTimer>, Has<Divider>), Without<TimerFill>>,
    mut cells: Query<(&Cell, &mut BackgroundColor, &mut BorderColor), (Without<Pip>, Without<TimerFill>)>,
    mut pips: Query<(&Pip, &mut BackgroundColor), (Without<Cell>, Without<TimerFill>)>,
    mut fills: Query<(&TimerFill, &mut Node), (Without<Cell>, Without<CallSlot>, Without<Pips>, Without<ModeTimer>)>,
    mut marks: Query<(&CallMark, &mut Visibility), Without<Strip>>,
) {
    let m = meters.map(|m| m.0).unwrap_or_default();
    let v = strip_view(&m, &progress);
    let any = v.shown.iter().any(|&s| s);
    for mut vis in &mut strip {
        vis.set_if_neq(if any { Visibility::Inherited } else { Visibility::Hidden });
    }
    let display = |on: bool| if on { Display::Flex } else { Display::None };
    for (mut node, cell, slot, pips_node, timer, divider) in &mut nodes {
        let want = match (cell, slot, pips_node, timer) {
            (Some(Cell(c)), ..) | (_, Some(CallSlot(c)), ..) => v.shown[c.index()],
            (_, _, Some(Pips(c)), _) => v.meters[c.index()].active.is_none(),
            (_, _, _, Some(ModeTimer(c))) => v.meters[c.index()].active.is_some(),
            _ if divider => v.shown[BandCounter::Sweat.index()] && v.shown[..3].iter().any(|&s| s),
            _ => continue,
        };
        if node.display != display(want) {
            node.display = display(want);
        }
    }
    for (Cell(c), mut bg, mut border) in &mut cells {
        let meter = v.meters[c.index()];
        let framed = v.ready == Some(*c) || meter.active.is_some();
        let (fill, rim) = if framed { (READY_FILL, GOLD) } else { (Color::NONE, Color::NONE) };
        bg.set_if_neq(BackgroundColor(fill));
        border.set_if_neq(BorderColor::all(rim));
    }
    for (Pip(c, k), mut bg) in &mut pips {
        let meter = v.meters[c.index()];
        let lit = *k < meter.lit;
        let on = match c {
            BandCounter::Sweat => SWEAT_BLUE,
            _ if v.ready == Some(*c) => CREAM,
            _ => GOLD,
        };
        bg.set_if_neq(BackgroundColor(if lit { on } else { DIM_PIP }));
    }
    for (TimerFill(c), mut node) in &mut fills {
        let w = percent(100.0 * v.meters[c.index()].active.unwrap_or(0.0));
        if node.width != w {
            node.width = w;
        }
    }
    for (CallMark(c), mut vis) in &mut marks {
        vis.set_if_neq(if v.ready == Some(*c) { Visibility::Inherited } else { Visibility::Hidden });
    }
}

/// A free play Side B room: the set list's up there instead.
fn hide_strip(mut strip: Query<&mut Visibility, With<Strip>>) {
    for mut v in &mut strip {
        v.set_if_neq(Visibility::Hidden);
    }
}
