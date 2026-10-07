//! Han's speech bubble: a world-space bubble above the [`Gus`] entity showing the latest
//! [`GusSays`] line with a typewriter reveal. A newer line replaces the old one.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::text::{FontSmoothing, FontSize, LineHeight};
use bevy::transform::TransformSystems;
use bevy::window::PrimaryWindow;

use super::{UiFont, VIRTUAL_HEIGHT, palette::INK};
use crate::events::GusSays;
use crate::game::Gus;
use crate::state::AppState;

/// Characters per line.
const WRAP: usize = 20;
/// Glyph size and line pitch, world px.
const GLYPH: f32 = 8.0;
const LINE: f32 = 10.0;
const PAD: f32 = 4.0;
const TAIL: f32 = 4.0;
/// Typewriter speed, characters per second.
const REVEAL_CPS: f32 = 40.0;
/// How long the bubble stays once fully revealed.
const STAY: f32 = 3.5;
/// Tip of the tail, above Han's origin.
const OFFSET: Vec2 = Vec2::new(0.0, 11.0);
const Z: f32 = 10.0;
/// A line said while there's no Han (e.g. the same frame his level spawns) waits this long for him.
const WAIT_FOR_HAN: f32 = 0.5;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (receive, spawn_pending, typewriter).chain())
        .add_systems(PostUpdate, follow.before(TransformSystems::Propagate));
}

#[derive(Resource)]
struct Pending {
    text: String,
    wait: f32,
}

#[derive(Component)]
pub struct SpeechBubble {
    lines: Vec<String>,
    age: f32,
    shown: usize,
}

#[derive(Component)]
struct BubbleText;

/// Greedy word wrap to `width` characters; words longer than a line are split.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        let mut word: Vec<char> = word.chars().collect();
        while !word.is_empty() {
            let cur_len = cur.chars().count();
            let sep = usize::from(cur_len > 0);
            if cur_len + sep + word.len() <= width {
                if sep == 1 {
                    cur.push(' ');
                }
                cur.extend(word.drain(..));
            } else if cur_len > 0 {
                lines.push(std::mem::take(&mut cur));
            } else {
                cur.extend(word.drain(..width));
                lines.push(std::mem::take(&mut cur));
            }
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// The first `n` characters of the wrapped lines (newlines don't count).
fn revealed(lines: &[String], n: usize) -> String {
    let mut out = String::new();
    let mut left = n;
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let take = l.chars().count().min(left);
        out.extend(l.chars().take(take));
        left -= take;
        if left == 0 {
            break;
        }
    }
    out
}

fn receive(mut commands: Commands, mut reader: MessageReader<GusSays>) {
    if let Some(msg) = reader.read().last() {
        commands.insert_resource(Pending { text: msg.text.clone(), wait: WAIT_FOR_HAN });
    }
}

fn spawn_pending(
    mut commands: Commands,
    pending: Option<ResMut<Pending>>,
    time: Res<Time>,
    han: Query<(), With<Gus>>,
    old: Query<Entity, With<SpeechBubble>>,
    font: Option<Res<UiFont>>,
    window: Option<Single<&Window, With<PrimaryWindow>>>,
) {
    let Some(mut pending) = pending else { return };
    if han.is_empty() {
        pending.wait -= time.delta_secs();
        if pending.wait <= 0.0 {
            commands.remove_resource::<Pending>();
        }
        return;
    }
    commands.remove_resource::<Pending>();
    for e in &old {
        commands.entity(e).despawn();
    }
    let lines = wrap(&pending.text, WRAP);
    if lines.is_empty() {
        return;
    }
    let cols = lines.iter().map(|l| l.chars().count()).max().unwrap_or(1) as f32;
    let size = Vec2::new(cols * GLYPH + 2.0 * PAD, lines.len() as f32 * LINE - (LINE - GLYPH) + 2.0 * PAD);
    // Rasterize the glyphs at about screen resolution, then scale down to world size: crisp pixels.
    let k = window.map_or(1.0, |w| (w.resolution.height() / VIRTUAL_HEIGHT).ceil().max(1.0));
    let box_center = Vec2::new(0.0, TAIL + size.y / 2.0);
    let rect = |color: Color, size: Vec2, pos: Vec2, z: f32| {
        (Sprite::from_color(color, size), Transform::from_translation(pos.extend(z)))
    };

    commands
        .spawn((
            Name::new("SpeechBubble"),
            SpeechBubble { lines, age: 0.0, shown: 0 },
            DespawnOnExit(AppState::Playing),
            Transform::from_xyz(0.0, 0.0, Z),
            Visibility::Hidden,
        ))
        .with_children(|b| {
            b.spawn(rect(INK, size + 2.0, box_center, 0.0));
            b.spawn(rect(Color::WHITE, size, box_center, 0.1));
            // The tail: a little pixel triangle pointing down at Han, outlined.
            for i in 0..TAIL as usize {
                let w = 2.0 * i as f32 + 1.0;
                let y = i as f32 + 0.5;
                b.spawn(rect(INK, Vec2::new(w + 2.0, 1.0), Vec2::new(0.0, y), 0.0));
                b.spawn(rect(Color::WHITE, Vec2::new(w, 1.0), Vec2::new(0.0, y), 0.1));
            }
            b.spawn(rect(INK, Vec2::ONE, Vec2::new(0.0, -0.5), 0.0));
            // Open the box's bottom border where the tail joins.
            b.spawn(rect(Color::WHITE, Vec2::new(2.0 * TAIL - 1.0, 1.0), Vec2::new(0.0, TAIL - 0.5), 0.15));
            b.spawn((
                BubbleText,
                Text2d::new(""),
                TextFont {
                    font: font.map(|f| f.0.clone()).unwrap_or_default().into(),
                    font_size: FontSize::Px(GLYPH * k),
                    font_smoothing: FontSmoothing::None,
                    ..default()
                },
                LineHeight::Px(LINE * k),
                TextColor(INK),
                Anchor::TOP_LEFT,
                Transform::from_xyz(-size.x / 2.0 + PAD, TAIL + size.y - PAD, 0.2)
                    .with_scale(Vec3::splat(1.0 / k)),
            ));
        });
}

fn typewriter(
    mut commands: Commands,
    time: Res<Time>,
    mut bubbles: Query<(Entity, &mut SpeechBubble, &Children)>,
    mut texts: Query<&mut Text2d, With<BubbleText>>,
) {
    for (e, mut b, children) in &mut bubbles {
        b.age += time.delta_secs();
        let total: usize = b.lines.iter().map(|l| l.chars().count()).sum();
        let reveal_time = total as f32 / REVEAL_CPS;
        if b.age > reveal_time + STAY {
            commands.entity(e).despawn();
            continue;
        }
        let n = ((b.age * REVEAL_CPS) as usize).min(total);
        if n != b.shown {
            b.shown = n;
            let s = revealed(&b.lines, n);
            for c in children.iter() {
                if let Ok(mut t) = texts.get_mut(c) {
                    t.0 = s.clone();
                }
            }
        }
    }
}

/// Stick to Han (pixel-snapped); vanish with him.
fn follow(
    mut commands: Commands,
    han: Query<(&Transform, &GlobalTransform, Has<ChildOf>), (With<Gus>, Without<SpeechBubble>)>,
    mut bubbles: Query<(Entity, &mut Transform, &mut Visibility), With<SpeechBubble>>,
) {
    let pos = han.iter().next().map(|(t, g, child)| if child { g.translation() } else { t.translation });
    for (e, mut t, mut vis) in &mut bubbles {
        let Some(p) = pos else {
            commands.entity(e).despawn();
            continue;
        };
        let target = (p.truncate() + OFFSET).round();
        t.translation = target.extend(Z);
        vis.set_if_neq(Visibility::Inherited);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_words() {
        assert_eq!(wrap("", 20), Vec::<String>::new());
        assert_eq!(wrap("hello", 20), vec!["hello"]);
        assert_eq!(
            wrap("Watch your step, kid. Those brushes bite!", 20),
            vec!["Watch your step,", "kid. Those brushes", "bite!"]
        );
        for l in wrap("a b c d e f g h i j k l m n o p q r s t u v w x y z", 20) {
            assert!(l.chars().count() <= 20);
        }
        assert_eq!(wrap("aaaaaaaaaaaaaaaaaaaaaaaaa b", 20), vec!["aaaaaaaaaaaaaaaaaaaa", "aaaaa b"]);
    }

    #[test]
    fn reveals_across_lines() {
        let lines = wrap("abc defg", 4);
        assert_eq!(lines, vec!["abc", "defg"]);
        assert_eq!(revealed(&lines, 0), "");
        assert_eq!(revealed(&lines, 2), "ab");
        assert_eq!(revealed(&lines, 3), "abc");
        assert_eq!(revealed(&lines, 5), "abc\nde");
        assert_eq!(revealed(&lines, 99), "abc\ndefg");
    }
}
