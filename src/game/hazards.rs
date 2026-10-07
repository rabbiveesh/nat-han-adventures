//! Things that splat you: spikes, liquid, flies, spray jets, the bottomless pit. And respawning.

use std::f32::consts::TAU;

use bevy::prelude::*;

use super::gus::{self, DEATH_LINES, GusTrail};
use super::physics::{Body, Dead, Finished, PlayerControl, cells, tile_at};
use super::{
    ActiveLevel, Fly, GameSet, Gus, LevelRun, Player, Pos, PrevPos, SimClock, Spray, tuning,
};
use crate::events::{GusSays, PlayerDied, PlayerRespawned};
use crate::level::{TILE, Tile};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(FixedUpdate, (move_flies, update_sprays).in_set(GameSet::World))
        .add_systems(
            FixedUpdate,
            (check_hazards, respawn).chain().in_set(GameSet::Interact),
        );
}

/// Hitboxes are this much more forgiving than the art, on each side.
pub const FORGIVE: f32 = 2.0;
/// Fly orbit radius and period.
pub const FLY_RADIUS: f32 = TILE;
pub const FLY_PERIOD: f32 = 2.0;
pub const FLY_HITBOX: f32 = 6.0;
/// Spray jet cycle: on for `SPRAY_ON`, then off, `SPRAY_CYCLE` in total.
pub const SPRAY_ON: f32 = 1.0;
pub const SPRAY_CYCLE: f32 = 2.5;
/// Jet height (tiles above the can) and width (px).
pub const SPRAY_HEIGHT: f32 = 3.0 * TILE;
pub const SPRAY_WIDTH: f32 = 8.0;

/// Is the jet of the spray in column `col` firing at sim time `t`?
/// All cans share one clock (levels design spray rows as a rhythm to run through together);
/// `col` is kept so per-can phases can come back without touching callers.
pub fn spray_on(_col: usize, t: f32) -> bool {
    // Off first, so nothing is firing the moment a level (re)starts.
    t.rem_euclid(SPRAY_CYCLE) >= SPRAY_CYCLE - SPRAY_ON
}

fn move_flies(clock: Res<SimClock>, mut flies: Query<(&Fly, &mut Pos)>) {
    for (fly, mut pos) in &mut flies {
        let a = TAU * (clock.time / FLY_PERIOD + fly.phase);
        pos.0 = fly.center + FLY_RADIUS * Vec2::new(a.cos(), a.sin());
    }
}

fn update_sprays(clock: Res<SimClock>, mut sprays: Query<&mut Spray>) {
    for mut spray in &mut sprays {
        let on = spray_on(spray.col, clock.time);
        if spray.on != on {
            spray.on = on;
        }
    }
}

fn rects_overlap(a_min: Vec2, a_max: Vec2, b_min: Vec2, b_max: Vec2) -> bool {
    a_min.x < b_max.x && a_max.x > b_min.x && a_min.y < b_max.y && a_max.y > b_min.y
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn check_hazards(
    mut commands: Commands,
    active: Res<ActiveLevel>,
    mut player: Query<
        (Entity, &Pos, &mut Body),
        (With<Player>, Without<Dead>, Without<Finished>),
    >,
    flies: Query<&Pos, (With<Fly>, Without<Player>)>,
    sprays: Query<(&Spray, &Transform)>,
    mut run: ResMut<LevelRun>,
    mut died: MessageWriter<PlayerDied>,
    mut says: MessageWriter<GusSays>,
) {
    let Ok((entity, pos, mut body)) = player.single_mut() else { return };
    let level = &active.level;
    let min = pos.0 - body.half + FORGIVE;
    let max = pos.0 + body.half - FORGIVE;

    let mut dead = max.y < 0.0; // Fell into the pit.

    // Deadly tiles.
    let (xs, ys) = cells(min, max);
    'tiles: for j in ys {
        for i in xs.clone() {
            let (x0, y0) = (i as f32 * TILE, j as f32 * TILE);
            let (lo, hi) = match tile_at(level, i, j) {
                Tile::SpikesUp => (y0, y0 + TILE / 2.0),
                Tile::SpikesDown => (y0 + TILE / 2.0, y0 + TILE),
                Tile::Liquid => (y0, y0 + TILE),
                _ => continue,
            };
            if rects_overlap(min, max, Vec2::new(x0, lo), Vec2::new(x0 + TILE, hi)) {
                dead = true;
                break 'tiles;
            }
        }
    }

    // Flies.
    let fh = Vec2::splat(FLY_HITBOX / 2.0);
    dead |= flies.iter().any(|f| rects_overlap(min, max, f.0 - fh, f.0 + fh));

    // Spray jets.
    dead |= sprays.iter().any(|(spray, tf)| {
        let base = tf.translation.truncate() + Vec2::new(0.0, TILE);
        spray.on
            && rects_overlap(
                min,
                max,
                base - Vec2::new(SPRAY_WIDTH / 2.0, 0.0),
                base + Vec2::new(SPRAY_WIDTH / 2.0, SPRAY_HEIGHT),
            )
    });

    if !dead {
        return;
    }
    body.vel = Vec2::ZERO;
    body.riding = None;
    commands.entity(entity).insert(Dead { remaining: tuning::RESPAWN_DELAY });
    run.deaths += 1;
    died.write(PlayerDied { pos: pos.0 });
    // Han pipes up on the 1st, 4th, 7th... death.
    if run.deaths % 3 == 1 {
        let line = DEATH_LINES[(run.deaths as usize / 3) % DEATH_LINES.len()];
        says.write(GusSays { text: line.to_string() });
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn respawn(
    mut commands: Commands,
    time: Res<Time>,
    active: Res<ActiveLevel>,
    run: Res<LevelRun>,
    mut player: Query<
        (Entity, &mut Dead, &mut Pos, &mut PrevPos, &mut Body, &mut PlayerControl),
        (With<Player>, Without<Gus>),
    >,
    mut gus_q: Query<(&mut Pos, &mut PrevPos, &mut GusTrail), (With<Gus>, Without<Player>)>,
    mut respawned: MessageWriter<PlayerRespawned>,
) {
    let Ok((entity, mut dead, mut pos, mut prev, mut body, mut ctl)) = player.single_mut() else {
        return;
    };
    dead.remaining -= time.delta_secs();
    if dead.remaining > 0.0 {
        return;
    }
    let level = &active.level;
    let (col, row) = run
        .checkpoint
        .and_then(|i| level.checkpoints().nth(i))
        .map_or(level.start, |c| (c.col, c.row));
    let at = super::lifecycle::stand_pos(level, col, row);
    pos.0 = at;
    prev.0 = at;
    *body = Body::player();
    *ctl = PlayerControl::default();
    commands.entity(entity).remove::<Dead>();
    respawned.write(PlayerRespawned { pos: at });

    for (mut gpos, mut gprev, mut trail) in &mut gus_q {
        gpos.0 = gus::behind(level, at, ctl.facing);
        gprev.0 = gpos.0;
        trail.0.clear();
    }
}
