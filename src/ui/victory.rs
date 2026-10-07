//! After level 10: the golden throne, and a credits roll.

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::{palette::*, *};
use crate::audio::{Music, songs::song};
use crate::input::Action;
use crate::level::Levels;
use crate::save::Progress;
use crate::state::AppState;

/// Credits scroll speed, virtual px per second.
const SCROLL_SPEED: f32 = 20.0;
/// Width of a "NAME ...... ROLE" line, in characters.
const LINE_CHARS: usize = 32;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Victory), spawn)
        .add_systems(Update, (scroll, input).run_if(in_state(AppState::Victory)));
}

#[derive(Component)]
struct Roll {
    y: f32,
}

/// One credits line.
enum Line {
    Heading(String),
    Text(String),
    Gap,
}

/// "NAME ......... ROLE", padded with dots to `width` characters.
pub fn dotted(name: &str, role: &str, width: usize) -> String {
    let used = name.chars().count() + role.chars().count() + 2;
    let dots = width.saturating_sub(used).max(3);
    format!("{name} {} {role}", ".".repeat(dots))
}

fn credits() -> Vec<Line> {
    use Line::*;
    let role = |n: &str, r: &str| Text(dotted(n, r, LINE_CHARS));
    let mut lines = vec![
        Heading(GAME_TITLE.into()),
        Text(GAME_SUBTITLE.to_uppercase()),
        Gap,
        Heading("STARRING".into()),
        role(&HERO_NAME.to_uppercase(), "AS HIMSELF"),
        role(&format!("{} THE PLUMBER", SIDEKICK_NAME.to_uppercase()), "AS HIMSELF"),
        role("GOLDEN NUGGETS", "THEMSELVES"),
        role("THE FLIES", "THE FLIES"),
        role("TOILET BRUSHES", "PURE EVIL"),
        role("RUBBER DUCK", "HIS OWN STUNTS"),
        Gap,
        Heading("MUSIC".into()),
        Text("8-BIT JAZZ STANDARDS".into()),
        Text("(PUBLIC DOMAIN, PRE-1931)".into()),
    ];
    let mut seen = Vec::new();
    for m in Music::ALL {
        let title = song(m).title;
        if !seen.contains(&title) {
            seen.push(title);
            lines.push(Text(title.to_uppercase()));
        }
    }
    lines.extend([
        Gap,
        Heading("SPECIAL THANKS".into()),
        Text("FIBRE".into()),
        Text("THE PLUMBERS' GUILD".into()),
        Text(format!("YOU, {}'S #1 FAN", SIDEKICK_NAME.to_uppercase())),
        Gap,
        Text("NO PLUMBERS WERE HARMED".into()),
        Text("IN THE MAKING OF THIS GAME".into()),
        Gap,
        Gap,
        Heading("THANKS FOR PLAYING!".into()),
    ]);
    lines
}

fn spawn(mut commands: Commands, font: Res<UiFont>, sprites: Option<Res<Sprites>>, progress: Res<Progress>, levels: Res<Levels>) {
    let f = &*font;
    let got: u32 = progress.best_nuggets.iter().flatten().sum();
    let total: usize = levels.0.iter().map(|l| l.nugget_count()).sum();
    commands
        .spawn((Name::new("Victory"), DespawnOnExit(AppState::Victory), fullscreen(), BackgroundColor(INK), GlobalZIndex(5)))
        .with_children(|root| {
            root.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(8.0),
                padding: UiRect::vertical(px(12.0)),
                ..default()
            })
            .with_children(|h| {
                h.spawn(label(f, "THE GOLDEN THRONE", 16.0, GOLD));
                h.spawn(label(f, "IS YOURS!", 16.0, GOLD));
                h.spawn(Node { column_gap: px(8.0), align_items: AlignItems::Center, ..default() })
                    .with_children(|r| {
                        r.spawn(icon(sprites.as_deref(), SpriteId::PooIdle, 16.0, 16.0));
                        r.spawn(icon(sprites.as_deref(), SpriteId::IconNugget, 8.0, 8.0));
                        r.spawn(label(f, format!("{got}/{total} NUGGETS"), 8.0, CREAM));
                        r.spawn(icon(sprites.as_deref(), SpriteId::GusIdle, 16.0, 16.0));
                    });
            });
            // The rolling credits, clipped to the space below the header.
            root.spawn(Node {
                width: percent(100.0),
                flex_grow: 1.0,
                overflow: Overflow::clip(),
                border: UiRect::top(px(2.0)),
                ..default()
            })
            .insert(BorderColor::all(DARK_GOLD))
            .with_children(|view| {
                view.spawn((
                    Roll { y: 0.0 },
                    Node {
                        position_type: PositionType::Absolute,
                        width: percent(100.0),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: px(8.0),
                        ..default()
                    },
                ))
                .with_children(|c| {
                    for line in credits() {
                        match line {
                            Line::Heading(s) => c.spawn(label(f, s, 8.0, GOLD)),
                            Line::Text(s) => c.spawn((
                                label(f, s, 8.0, CREAM),
                                TextLayout::justify(Justify::Center),
                                Node { max_width: px(272.0), ..default() },
                            )),
                            Line::Gap => c.spawn(Node { height: px(8.0), ..default() }),
                        };
                    }
                });
            });
            root.spawn((
                label(f, "ENTER: BACK TO TITLE", 8.0, DIM_CREAM),
                Node { margin: UiRect::vertical(px(8.0)), ..default() },
            ));
        });
}

/// Scroll up from below the view; loop once everything has gone past.
fn scroll(
    time: Res<Time>,
    mut q: Query<(&mut Roll, &mut Node, &ComputedNode, &ChildOf)>,
    parents: Query<&ComputedNode>,
) {
    for (mut roll, mut node, computed, child_of) in &mut q {
        let Ok(view) = parents.get(child_of.parent()) else { continue };
        let k = computed.inverse_scale_factor();
        let (h, view_h) = (computed.size().y * k, view.size().y * k);
        if view_h <= 0.0 {
            continue;
        }
        roll.y += time.delta_secs() * SCROLL_SPEED;
        let mut top = view_h - roll.y;
        if top < -h {
            roll.y = 0.0;
            top = view_h;
        }
        node.top = px(top.round());
    }
}

fn input(action: Single<&ActionState<Action>>, mut next: ResMut<NextState<AppState>>, mut sfx_w: MessageWriter<PlaySfx>) {
    if action.just_pressed(&Action::Confirm) || action.just_pressed(&Action::Back) {
        sfx(&mut sfx_w, Sfx::MenuSelect);
        next.set(AppState::Title);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dotted_lines_fit() {
        let s = dotted("NAT", "AS HIMSELF", 32);
        assert_eq!(s.chars().count(), 32);
        assert!(s.starts_with("NAT ...") && s.ends_with("... AS HIMSELF"));
        // Too long: still has some dots.
        assert!(dotted("A VERY VERY LONG NAME INDEED", "ROLE", 10).contains("..."));
    }

    #[test]
    fn credits_include_cast_and_music() {
        let texts: Vec<String> = credits()
            .into_iter()
            .filter_map(|l| match l {
                Line::Text(s) | Line::Heading(s) => Some(s),
                Line::Gap => None,
            })
            .collect();
        assert!(texts.iter().any(|s| s.starts_with("HAN THE PLUMBER") && s.ends_with("AS HIMSELF")));
        assert!(texts.iter().any(|s| s == "THANKS FOR PLAYING!"));
        assert!(texts.contains(&song(Music::Title).title.to_uppercase()));
    }
}
