//! Things that splat you: spikes, liquid, flies, spray jets, the bottomless pit. And respawning.
//!
//! A splat leaves a stain (see `crate::level`, "Splat stains"): on spikes the spike tile under
//! Nat becomes a [`Tile::StainUp`]/[`Tile::StainDown`] in [`ActiveLevel`] (so physics, Han and
//! the validator all see a one-way block) plus a [`Stain`] entity to draw it; in liquid a
//! [`Raft`] (a one-tile, one-way [`MovingPlatform`] that stays put) floats at the surface for
//! [`RaftLife::secs`] seconds (never less than [`RAFT_LIFE_FLOOR`]), sinking for the last
//! [`RAFT_SINK`].

use bevy::prelude::*;

use super::han::{self, DEATH_LINES, HanTrail};
use super::physics::{Body, Dead, Finished, PlayerControl, cells, tile_at};
use super::{
    ActiveLevel, Fly, GameSet, Groove, Han, LevelEntity, LevelRun, MovingPlatform, Nugget, Player, Pos, PrevPos,
    Raft, SimClock, Spray, Stain, groove::fly_angle, tuning,
};
use crate::events::{HanSays, PlayerDied, PlayerRespawned};
use crate::level::{PlatformKind, TILE, Tile};

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<RaftLife>().add_systems(
        FixedUpdate,
        (move_flies, update_sprays, float_rafts.after(super::platforms::move_platforms)).in_set(GameSet::World),
    )
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
/// A stain raft floats at least this long (s) after the splat that made it: the floor. The
/// actual lifetime is this × [`RaftLife::scale`] (an assist hook, e.g. up to ~2× from the
/// adaptive dial later; never below 1×). Level validation never counts on a raft lasting
/// longer than the floor (`level::validate` doesn't use rafts to cross anything at all; it
/// only checks, treating them as permanent, that they can't open a gate).
pub const RAFT_LIFE_FLOOR: f32 = 12.0;
/// ...sinking [`RAFT_SINK_DEPTH`] px below the surface over the last this many seconds.
pub const RAFT_SINK: f32 = 1.5;
pub const RAFT_SINK_DEPTH: f32 = 12.0;
/// Jet height (tiles above the can) and width (px).
pub const SPRAY_HEIGHT: f32 = 3.0 * TILE;
pub const SPRAY_WIDTH: f32 = 8.0;

/// Is the jet of the spray in column `col` firing at sim time `t`?
/// All cans share one clock (levels design spray rows as a rhythm to run through together);
/// `col` is kept so per-can phases can come back without touching callers. While the band
/// waltzes the cans follow the music instead ([`Groove::waltz_spray_on`]).
pub fn spray_on(_col: usize, t: f32) -> bool {
    // Off first, so nothing is firing the moment a level (re)starts.
    t.rem_euclid(SPRAY_CYCLE) >= SPRAY_CYCLE - SPRAY_ON
}

fn move_flies(clock: Res<SimClock>, mut flies: Query<(&Fly, &mut Pos)>) {
    for (fly, mut pos) in &mut flies {
        let a = fly_angle(clock.fly_turns, fly.phase);
        pos.0 = fly.center + FLY_RADIUS * Vec2::new(a.cos(), a.sin());
    }
}

fn update_sprays(clock: Res<SimClock>, groove: Res<Groove>, mut sprays: Query<&mut Spray>) {
    for mut spray in &mut sprays {
        let on = if groove.waltz() { groove.waltz_spray_on() } else { spray_on(spray.col, clock.time) };
        if spray.on != on {
            spray.on = on;
        }
    }
}

fn rects_overlap(a_min: Vec2, a_max: Vec2, b_min: Vec2, b_max: Vec2) -> bool {
    a_min.x < b_max.x && a_max.x > b_min.x && a_min.y < b_max.y && a_max.y > b_min.y
}

/// How far (px) the raft at age `age` has sunk below the surface.
pub fn raft_sink(age: f32, life: f32) -> f32 {
    let k = ((age - (life - RAFT_SINK)) / RAFT_SINK).clamp(0.0, 1.0);
    RAFT_SINK_DEPTH * k * k
}

/// How long stain rafts last: [`RAFT_LIFE_FLOOR`] × `scale` (default 1; clamped to ≥ 1).
/// The assist hook for the adaptive engine.
#[derive(Resource, Debug, Clone, Copy, Reflect)]
#[reflect(Resource)]
pub struct RaftLife {
    pub scale: f32,
}

impl Default for RaftLife {
    fn default() -> Self {
        Self { scale: 1.0 }
    }
}

impl RaftLife {
    pub fn secs(&self) -> f32 {
        RAFT_LIFE_FLOOR * self.scale.max(1.0)
    }
}

type RaftQuery<'w, 's> = Query<'w, 's, (Entity, &'static mut Raft, &'static MovingPlatform, &'static mut Pos), (Without<Player>, Without<Fly>)>;

/// Rafts age, sink at the end of their life, and go.
fn float_rafts(mut commands: Commands, time: Res<Time>, life: Res<RaftLife>, mut rafts: RaftQuery) {
    let life = life.secs();
    for (e, mut raft, platform, mut pos) in &mut rafts {
        raft.age += time.delta_secs();
        if raft.age >= life {
            commands.entity(e).despawn();
            continue;
        }
        pos.0 = platform.base - Vec2::new(0.0, raft_sink(raft.age, life));
    }
}

/// Leave a splat stain for a death touching the deadly tile at (`i`, `j`) (world tile indices).
fn leave_stain(commands: &mut Commands, active: &mut ActiveLevel, rafts: &mut RaftQuery, i: i32, j: i32) {
    let level = &mut active.level;
    let (col, row) = (i, level.height as i32 - 1 - j);
    if col < 0 || row < 0 || col >= level.width as i32 || row >= level.height as i32 {
        return;
    }
    let tile = level.tile(col, row);
    if let Some(stain) = tile.stained() {
        level.tiles[row as usize * level.width + col as usize] = stain;
        let center = level.tile_center(col as usize, row as usize);
        commands.spawn((
            Name::new("Stain"),
            LevelEntity,
            Stain { col: col as usize, row: row as usize, tile: stain },
            Transform::from_translation(center.extend(0.5)),
        ));
    } else if tile == Tile::Liquid {
        // Float up to the surface.
        let mut top = row;
        while top > 0 && level.tile(col, top - 1) == Tile::Liquid {
            top -= 1;
        }
        let center = level.tile_center(col as usize, top as usize);
        // A fresh splat on an existing raft's spot renews that raft.
        for (_, mut raft, platform, _) in rafts.iter_mut() {
            if (platform.base - center).length() < 1.0 {
                raft.age = 0.0;
                return;
            }
        }
        commands.spawn((
            Name::new("StainRaft"),
            LevelEntity,
            Raft::default(),
            MovingPlatform {
                base: center,
                travel: Vec2::ZERO,
                period: 1.0,
                phase: 0.0,
                width: 1,
                kind: PlatformKind::Raft,
            },
            Pos(center),
            PrevPos(center),
            Transform::from_translation(center.extend(1.0)),
        ));
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn check_hazards(
    mut commands: Commands,
    mut active: ResMut<ActiveLevel>,
    mut rafts: RaftQuery,
    mut player: Query<
        (Entity, &Pos, &mut Body),
        (With<Player>, Without<Dead>, Without<Finished>),
    >,
    flies: Query<&Pos, (With<Fly>, Without<Player>)>,
    sprays: Query<(&Spray, &Transform)>,
    mut run: ResMut<LevelRun>,
    mut died: MessageWriter<PlayerDied>,
    mut says: MessageWriter<HanSays>,
) {
    let Ok((entity, pos, mut body)) = player.single_mut() else { return };
    let level = &active.level;
    let min = pos.0 - body.half + FORGIVE;
    let max = pos.0 + body.half - FORGIVE;

    let mut dead = max.y < 0.0; // Fell into the pit.

    // Deadly tiles. The one nearest Nat's middle takes the stain.
    let (xs, ys) = cells(min, max);
    let mut splat_on: Option<(f32, i32, i32)> = None;
    for j in ys {
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
                let d = (Vec2::new(x0, y0) + TILE / 2.0).distance(pos.0);
                if splat_on.is_none_or(|(best, _, _)| d < best) {
                    splat_on = Some((d, i, j));
                }
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
    if let Some((_, i, j)) = splat_on {
        leave_stain(&mut commands, &mut active, &mut rafts, i, j);
    }
    commands.entity(entity).insert(Dead { remaining: tuning::RESPAWN_DELAY });
    run.deaths += 1;
    died.write(PlayerDied { pos: pos.0 });
    // Han pipes up on the 1st, 4th, 7th... death.
    if run.deaths % 3 == 1 {
        let line = DEATH_LINES[(run.deaths as usize / 3) % DEATH_LINES.len()];
        says.write(HanSays { text: line.to_string() });
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn respawn(
    mut commands: Commands,
    time: Res<Time>,
    active: Res<ActiveLevel>,
    run: Res<LevelRun>,
    mut at_risk: ResMut<super::pickups::NuggetsAtRisk>,
    mut player: Query<
        (Entity, &mut Dead, &mut Pos, &mut PrevPos, &mut Body, &mut PlayerControl),
        (With<Player>, Without<Han>),
    >,
    mut han_q: Query<(&mut Pos, &mut PrevPos, &mut HanTrail), (With<Han>, Without<Player>)>,
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
    // Nuggets grabbed since the checkpoint come back as ghosts: they fire up the band again,
    // but each nugget only ever counts once toward the total.
    for c in at_risk.0.drain(..) {
        commands.spawn((
            Name::new("Nugget"),
            LevelEntity,
            super::GhostNugget,
            Nugget,
            Transform::from_translation(c.extend(2.0)),
        ));
    }

    for (mut gpos, mut gprev, mut trail) in &mut han_q {
        gpos.0 = han::behind(level, at, ctl.facing);
        gprev.0 = gpos.0;
        trail.0.clear();
    }
}
