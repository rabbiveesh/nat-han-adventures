//! The double game's HUD (`game::duo`): everything moves into the **bandstand**, one bar across
//! the middle of the screen covering the slab between Nat's storey and Han's. Nuggets on the
//! left, the band's counters, what the music does to the physics, the time on the right; the
//! level name and the band readout make way.

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::{palette::*, *};
use crate::audio::Sfx;
use crate::game::duo::{self, DeathRule, Duo, ROWS, SLAB};
use crate::game::{GeneratedLevel, LevelRun};
use crate::input::Action;
use crate::state::AppState;

/// The bandstand: the slab's rows, in UI px (216 to the screen's height).
const SEAM_TOP: f32 = VIRTUAL_HEIGHT * SLAB.0 as f32 / ROWS as f32;
const SEAM_HEIGHT: f32 = VIRTUAL_HEIGHT * (SLAB.1 - SLAB.0 + 1) as f32 / ROWS as f32;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Playing), spawn_seam.run_if(resource_exists::<Duo>))
        .add_systems(Update, (layout, say_rule).run_if(in_state(AppState::Playing).and_then(resource_exists::<Duo>)))
        .add_systems(OnEnter(AppState::LevelComplete), results.run_if(resource_exists::<Duo>))
        .add_systems(Update, results_input.run_if(in_state(AppState::LevelComplete).and_then(resource_exists::<Duo>)));
}

#[derive(Component)]
struct Seam;

fn spawn_seam(mut commands: Commands) {
    commands.spawn((
        Name::new("DuoSeam"),
        Seam,
        DespawnOnExit(AppState::Playing),
        GlobalZIndex(-1),
        Node {
            position_type: PositionType::Absolute,
            top: px(SEAM_TOP),
            height: px(SEAM_HEIGHT),
            width: percent(100.0),
            border: UiRect::vertical(px(1.0)),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(DARK_GOLD),
    ));
}

/// Move the story HUD's pieces into the bandstand (they're spawned where the story wants them).
#[allow(clippy::type_complexity)]
fn layout(
    mut named: Query<(&Name, &mut Node, &mut Visibility, Option<&Children>)>,
    mut unnamed: Query<(&mut Node, Option<&Children>), Without<Name>>,
) {
    let mid = SEAM_TOP + SEAM_HEIGHT / 2.0;
    let mut hud_kids = Vec::new();
    for (name, mut node, mut vis, children) in &mut named {
        match name.as_str() {
            "Hud" => {
                let pad = UiRect::axes(px(8.0), px(mid - SEAM_TOP - 4.0));
                if node.top != px(SEAM_TOP) || node.padding != pad {
                    node.top = px(SEAM_TOP);
                    node.padding = pad;
                }
                hud_kids.extend(children.iter().flat_map(|c| c.iter()));
            }
            "BandStrip" => {
                if node.left != px(76.0) {
                    node.top = px(mid - 7.0);
                    node.left = px(76.0);
                    node.width = Val::Auto;
                    node.align_items = AlignItems::Start;
                }
            }
            "GrooveBadge" => {
                if node.right != px(56.0) {
                    node.left = Val::Auto;
                    node.right = px(56.0);
                    node.top = px(mid - 8.0);
                }
            }
            "BandReadoutPanel" => {
                vis.set_if_neq(Visibility::Hidden);
            }
            _ => {}
        }
    }
    // The level name (the bar's middle child) and the splats (under the time) make way.
    let mut gone: Vec<Entity> = hud_kids.get(1).copied().into_iter().collect();
    if let Some(&time) = hud_kids.get(2)
        && let Ok((_, Some(c))) = unnamed.get(time)
    {
        gone.extend(c.get(1).copied());
    }
    for e in gone {
        if let Ok((mut node, _)) = unnamed.get_mut(e)
            && node.display != Display::None
        {
            node.display = Display::None;
        }
    }
}

/// Which death rule is on (Tab switches it): said at the start and on every switch.
/// (The first time once Han's said his line.)
fn say_rule(mut commands: Commands, font: Res<UiFont>, duo: Res<Duo>, run: Res<LevelRun>, mut was: Local<Option<DeathRule>>) {
    if *was != Some(duo.rule) && (was.is_some() || run.time > 3.0) {
        let text = format!("{}  (TAB)", duo.rule.label());
        spawn_toast(&mut commands, &font, text, 2.5);
        *was = Some(duo.rule);
    }
}

/// Both made it: the card (nothing saved, the story's progress untouched).
fn results(mut commands: Commands, font: Res<UiFont>, duo: Res<Duo>, run: Res<LevelRun>, level: Res<GeneratedLevel>) {
    let f = &*font;
    let lines = [
        (format!("NUGGETS {}/{}", run.nuggets, run.nuggets_total), CREAM),
        (format!("TIME {}", format_time(run.time)), CREAM),
        (format!("NAT SPLATS {}", run.deaths), CREAM),
        (format!("HAN TUMBLES {}", duo.han_deaths), CREAM),
        (duo.rule.label().to_string(), DIM_CREAM),
    ];
    commands
        .spawn((Name::new("DuoResults"), DespawnOnExit(AppState::LevelComplete), fullscreen(), BackgroundColor(OVERLAY), GlobalZIndex(10)))
        .with_children(|root| {
            root.spawn(panel(Node { min_width: px(240.0), padding: UiRect::axes(px(16.0), px(12.0)), ..column(16.0, 8.0) })).with_children(
                |p| {
                    p.spawn(label(f, "TOGETHER!", 16.0, GOLD));
                    p.spawn((label(f, level.0.name.to_uppercase(), 8.0, CREAM), Node { margin: UiRect::bottom(px(8.0)), ..default() }));
                    for (text, color) in lines {
                        p.spawn(label(f, text, 8.0, color));
                    }
                    p.spawn((label(f, "ENTER:AGAIN  ESC:TITLE", 8.0, DIM_CREAM), Node { margin: UiRect::top(px(8.0)), ..default() }));
                },
            );
        });
}

fn results_input(
    mut commands: Commands,
    action: Single<&ActionState<Action>>,
    duo: Res<Duo>,
    mut next: ResMut<NextState<AppState>>,
    mut sfx_w: MessageWriter<PlaySfx>,
) {
    if action.just_pressed(&Action::Confirm) {
        sfx(&mut sfx_w, Sfx::MenuSelect);
        duo::start(&mut commands, duo.level, duo.rule);
    } else if action.just_pressed(&Action::Back) {
        sfx(&mut sfx_w, Sfx::MenuMove);
        next.set(AppState::Title);
    }
}
