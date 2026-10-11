//! World -1's look (`game::minus1`): the corrupted pause menu (a crash screen: junk text,
//! tiles leaking in as entries, jittering garbage stripes), and the glitch flash when a pick
//! rewrites the level's tiles.

use bevy::prelude::*;

use super::{palette::*, *};
use crate::game::ActiveLevel;
use crate::game::LevelTile;
use crate::game::minus1::{ENTRIES, GlitchMenu, MinusOne, TileRewrite};
use crate::level::Tile;
use crate::state::PlayState;

/// The NES-ish colours garbage comes in.
const JUNK: [Color; 8] = [
    Color::srgb(0.98, 0.22, 0.60),
    Color::srgb(0.22, 0.90, 0.95),
    Color::srgb(0.95, 0.85, 0.20),
    Color::srgb(0.40, 0.25, 0.95),
    Color::srgb(0.10, 0.75, 0.30),
    Color::srgb(0.95, 0.45, 0.10),
    Color::srgb(0.95, 0.95, 0.95),
    Color::srgb(0.05, 0.05, 0.10),
];
const STRIPES: usize = 18;
const FLASH_SECS: f32 = 0.45;
const FLICKER_SECS: f32 = 0.9;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(PlayState::Paused), spawn_menu.run_if(resource_exists::<GlitchMenu>))
        .add_systems(
            Update,
            (
                (highlight, jitter).run_if(in_state(PlayState::Paused).and_then(resource_exists::<GlitchMenu>)),
                (flash_on_rewrite, fade_flash, flicker, escaped_card).run_if(resource_exists::<MinusOne>),
            ),
        );
}

/// A cheap deterministic hash (garbage should look random, and the same every take).
fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^ (x >> 16)
}

fn junk(seed: u32) -> Color {
    JUNK[(hash(seed) % JUNK.len() as u32) as usize]
}

#[derive(Component)]
struct MenuEntry(usize);

/// A garbage stripe; re-placed every few frames.
#[derive(Component)]
struct Stripe(u32);

#[derive(Component)]
struct Flash(f32);

#[derive(Component)]
struct Flicker(f32);

fn tile_icon(tile: Option<Tile>, world: u8) -> Option<SpriteId> {
    Some(match tile? {
        Tile::SpikesUp => SpriteId::SpikesUp,
        Tile::Liquid => SpriteId::LiquidTop(world),
        Tile::OneWay => SpriteId::OneWay(world),
        Tile::Grease => SpriteId::Grease(world),
        Tile::Solid => SpriteId::GroundTop(world),
        _ => return None,
    })
}

fn stripe_node(seed: u32) -> Node {
    let h = hash(seed);
    Node {
        position_type: PositionType::Absolute,
        top: percent((h % 100) as f32),
        left: percent(((h >> 8) % 70) as f32),
        width: percent(10.0 + ((h >> 16) % 60) as f32),
        height: px(2.0 + ((h >> 24) % 4) as f32 * 2.0),
        ..default()
    }
}

fn spawn_stripes(p: &mut ChildSpawnerCommands, seed: u32) {
    for i in 0..STRIPES as u32 {
        let s = seed.wrapping_mul(31).wrapping_add(i);
        p.spawn((stripe_node(s), BackgroundColor(junk(s ^ 0x55).with_alpha(0.85)), Stripe(s)));
    }
}

fn spawn_menu(
    mut commands: Commands,
    font: Res<UiFont>,
    sprites: Option<Res<Sprites>>,
    active: Option<Res<ActiveLevel>>,
    minus: Res<MinusOne>,
) {
    let f = &*font;
    let world = active.map_or(1, |a| a.level.world);
    let seed = 0x1234 + minus.writes * 7;
    commands
        .spawn((
            Name::new("GlitchMenu"),
            DespawnOnExit(PlayState::Paused),
            fullscreen(),
            BackgroundColor(Color::srgba(0.05, 0.02, 0.20, 0.55)),
            GlobalZIndex(10),
        ))
        .with_children(|root| {
            spawn_stripes(root, seed);
            root.spawn((
                Node {
                    border: UiRect::all(px(2.0)),
                    margin: UiRect { left: px(-22.0), top: px(6.0), ..default() },
                    ..column(12.0, 4.0)
                },
                BackgroundColor(Color::srgba(0.30, 0.04, 0.28, 0.94)),
                BorderColor::all(JUNK[1]),
            ))
            .with_children(|p| {
                p.spawn((label(f, "PA@$E#", 16.0, JUNK[1]), Node { margin: UiRect::bottom(px(4.0)), ..default() }));
                p.spawn(label(f, "0F 00 FF 3C", 8.0, JUNK[3]));
                for (i, e) in ENTRIES.iter().enumerate() {
                    p.spawn(Node { column_gap: px(4.0), align_items: AlignItems::Center, height: px(16.0), ..default() })
                        .with_children(|row| {
                            match tile_icon(e.tile, world) {
                                Some(id) => {
                                    row.spawn(icon(sprites.as_deref(), id, 16.0, 16.0));
                                }
                                None => {
                                    row.spawn(Node { width: px(16.0), ..default() });
                                }
                            }
                            row.spawn((label(f, e.text, 8.0, CREAM), MenuEntry(i)));
                        });
                }
                p.spawn(label(f, "?SYNTAX  ERR0R", 8.0, JUNK[0]));
                p.spawn(label(f, "LVL -1", 8.0, DIM_CREAM));
            });
        });
}

fn highlight(menu: Res<GlitchMenu>, mut q: Query<(&MenuEntry, &mut Text, &mut TextColor)>) {
    for (e, mut text, mut color) in &mut q {
        let on = e.0 == menu.cursor;
        let name = ENTRIES[e.0].text;
        let s = if on { format!(">{name}<") } else { format!(" {name} ") };
        if text.0 != s {
            text.0 = s;
        }
        color.0 = if on { GOLD } else { CREAM };
    }
}

/// The stripes jump about every few frames, like a crashed console's garbage.
fn jitter(time: Res<Time<Real>>, mut q: Query<(&Stripe, &mut Node, &mut BackgroundColor)>) {
    let tick = (time.elapsed_secs() * 12.0) as u32;
    for (s, mut node, mut bg) in &mut q {
        if hash(s.0 ^ tick) % 3 == 0 {
            let seed = s.0.wrapping_add(tick.wrapping_mul(977));
            *node = stripe_node(seed);
            bg.0 = junk(seed).with_alpha(0.85);
        }
    }
}

fn flash_on_rewrite(
    mut rewrites: MessageReader<TileRewrite>,
    mut commands: Commands,
    tiles: Query<(Entity, &LevelTile)>,
    minus: Res<MinusOne>,
) {
    for r in rewrites.read() {
        commands
            .spawn((Name::new("GlitchFlash"), fullscreen(), GlobalZIndex(9), Flash(FLASH_SECS)))
            .with_children(|root| spawn_stripes(root, 0x9e37 + minus.writes));
        for (e, t) in &tiles {
            if t.tile == r.to {
                commands.entity(e).insert(Flicker(FLICKER_SECS));
            }
        }
    }
}

fn fade_flash(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Flash)>) {
    for (e, mut f) in &mut q {
        f.0 -= time.delta_secs();
        if f.0 <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}

/// Freshly rewritten tiles cycle through the wrong palettes for a moment.
fn flicker(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &LevelTile, &mut Flicker, &mut Sprite)>) {
    let tick = (time.elapsed_secs() * 20.0) as u32;
    for (e, t, mut f, mut sprite) in &mut q {
        f.0 -= time.delta_secs();
        if f.0 <= 0.0 {
            sprite.color = Color::WHITE;
            commands.entity(e).remove::<Flicker>();
        } else {
            let k = (t.col as u32).wrapping_mul(7) ^ (t.row as u32).wrapping_mul(13) ^ tick;
            sprite.color = if hash(k) % 3 == 0 { Color::WHITE } else { junk(k) };
        }
    }
}

#[derive(Component)]
struct EscapedCard;

/// On the goal: not "FLUSHED IT!", just a question.
fn escaped_card(mut commands: Commands, font: Res<UiFont>, minus: Res<MinusOne>, card: Query<(), With<EscapedCard>>) {
    if minus.escaped.is_some() && card.is_empty() {
        commands.spawn((
            Name::new("EscapedCard"),
            EscapedCard,
            DespawnOnExit(crate::state::AppState::Playing),
            fullscreen(),
            GlobalZIndex(9),
        ))
        .with_children(|root| {
            root.spawn(label(&font, "OUT...?", 16.0, JUNK[1]));
        });
    }
}
