//! The new tiles that move or break: crushers (`X`, `<`, `>`), crumbling platforms (`-`) and
//! bricks (`%`). (Trampolines are all physics: `step_body` springs whoever lands on one.)
//!
//! - **Crushers** run their cycle ([`Crusher::offset`]) on the sim clock. Their blocks are solid
//!   to Nat and Han (`step_body`'s `solids`) and carry whoever stands on top; anyone the block
//!   ends up overlapping got squashed: Nat dies ([`DeathCause::Crusher`]), Han goes flat and
//!   parachutes back (on Side B it's his end: `game::sideb`).
//! - **Crumbling platforms** crack once someone stands on them, fall [`CRUMBLE_DELAY`] s later
//!   (the tile goes empty in [`ActiveLevel`]) and come back [`CRUMBLE_BACK`] s after that.
//! - **Bricks** are smashed by whoever rises into them (`Contact::bricks`): [`smash`] empties
//!   the tile and says so ([`BrickSmashed`]) for the art and the sound.
//! - **Beat blocks** swap on the music's bar line ([`beat_blocks`]): red ones are there on even
//!   bars, blue ones on odd ones. One coming back where someone stands waits until they've
//!   moved.
//!
//! (Conveyor belts, fans and bumpers are all physics: `step_body`.)

use bevy::prelude::*;

use super::physics::{Body, Carrier, Dead, Finished};
use super::{ActiveLevel, GameSet, Groove, Han, LevelTile, Player, Pos, SimClock};
use crate::level::{CRUMBLE_BACK, CRUMBLE_DELAY, CrushDir, CrushTime, Crusher, CrusherDef, Level, TILE, Tile};

pub(super) fn plugin(app: &mut App) {
    app.add_message::<BrickSmashed>()
        .add_message::<Sprung>()
        .add_message::<CrusherThud>()
        .add_systems(FixedUpdate, (move_crushers, beat_blocks).in_set(GameSet::World))
        .add_systems(FixedUpdate, crumble.in_set(GameSet::Interact))
        .add_systems(FixedUpdate, sounds.in_set(GameSet::Sync));
}

/// The gadgets' sounds: a trampoline's boing, a brick's and a crusher's thud.
fn sounds(
    mut smashed: MessageReader<BrickSmashed>,
    mut sprung: MessageReader<Sprung>,
    mut thuds: MessageReader<CrusherThud>,
    mut sfx: MessageWriter<crate::events::PlaySfx>,
) {
    use crate::audio::Sfx;
    let n = sprung.read().count();
    if n > 0 {
        sfx.write(crate::events::PlaySfx(Sfx::Boing));
    }
    if smashed.read().count() + thuds.read().count() > 0 {
        sfx.write(crate::events::PlaySfx(Sfx::Land));
    }
}

/// A crusher's block. `Pos` is its center; it sits at `home` at rest.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct CrusherBlock {
    pub def: CrusherDef,
    pub home: Vec2,
    pub half: Vec2,
    /// Slamming right now.
    pub slamming: bool,
}

impl CrusherBlock {
    pub fn new(level: &Level, def: CrusherDef) -> CrusherBlock {
        let a = level.tile_center(def.col, def.row);
        let b = level.tile_center(def.col + def.w - 1, def.row + def.h - 1);
        CrusherBlock { def, home: (a + b) / 2.0, half: Vec2::new(def.w as f32, def.h as f32) * TILE / 2.0, slamming: false }
    }

    /// Where its center is at `t`.
    pub fn at(&self, t: CrushTime) -> Vec2 {
        let (d, _) = Crusher::offset(&self.def, t);
        let dir = match self.def.dir {
            CrushDir::Down => Vec2::NEG_Y,
            CrushDir::Left => Vec2::NEG_X,
            CrushDir::Right => Vec2::X,
        };
        self.home + dir * d
    }

    /// Its box (min, max) centered at `pos`.
    pub fn bounds(&self, pos: Vec2) -> (Vec2, Vec2) {
        (pos - self.half, pos + self.half)
    }

    /// Its top as a carrier (one-way, carries riders; jumping off keeps nothing).
    pub fn carrier(&self, id: Entity, pos: Vec2, prev: Vec2) -> Carrier {
        Carrier {
            id,
            x: pos.x,
            half_w: self.half.x,
            top: pos.y + self.half.y,
            prev_top: prev.y + self.half.y,
            carry: pos - prev,
            launches: false,
        }
    }
}

/// Is a body box (center `pos`, half extents `half`) squashed by a block at `block` (min, max)?
/// Overlapping it by more than [`super::FORGIVE`] each way.
pub fn squashed(pos: Vec2, half: Vec2, (min, max): (Vec2, Vec2)) -> bool {
    let f = super::FORGIVE;
    pos.x + half.x - f > min.x && pos.x - half.x + f < max.x && pos.y + half.y - f > min.y && pos.y - half.y + f < max.y
}

/// When it is for the crushers: the sim clock, and the music's bars.
pub fn crush_time(clock: &SimClock, groove: &Groove) -> CrushTime {
    let c = &groove.clock;
    let bars = c.bar as f32 + (c.beat as f32 + c.phase) / c.beats_per_bar.max(1) as f32;
    CrushTime { sim: clock.time, bars, bar_secs: c.bar_secs() }
}

/// Should a body (center `pos`, half extents `half`) on foot, heading `dir` at `speed` px/s,
/// wait before stepping into this crusher's sweep at `t`? Yes unless it's resting
/// at home long enough to get past it. (Already in the sweep: no, get out.) Han's reflex.
pub fn wait_for(block: &CrusherBlock, pos: Vec2, half: Vec2, dir: f32, speed: f32, t: CrushTime) -> bool {
    let (a0, a1) = block.bounds(block.home);
    let d = block.def.travel as f32 * TILE;
    let far = block.home
        + d * match block.def.dir {
            CrushDir::Down => Vec2::NEG_Y,
            CrushDir::Left => Vec2::NEG_X,
            CrushDir::Right => Vec2::X,
        };
    let (b0, b1) = block.bounds(far);
    let (min, max) = (a0.min(b0), a1.max(b1));
    let overlaps = |p: Vec2| p.x + half.x > min.x && p.x - half.x < max.x && p.y + half.y > min.y && p.y - half.y < max.y;
    if dir == 0.0 || overlaps(pos) || !overlaps(pos + Vec2::new(dir * 6.0, 0.0)) {
        return false;
    }
    let dist = if dir > 0.0 { max.x - (pos.x - half.x) } else { (pos.x + half.x) - min.x } + 2.0;
    let need = dist / speed.max(1.0) + 0.15;
    Crusher::rest_left(&block.def, t).is_none_or(|left| left < need)
}

/// A brick smashed (grid cell; `pos` its center, world px).
#[derive(Message, Debug, Clone, Copy)]
pub struct BrickSmashed {
    pub col: usize,
    pub row: usize,
    pub pos: Vec2,
}

/// Sprang off a trampoline (Nat or Han).
#[derive(Message, Debug, Clone, Copy)]
pub struct Sprung {
    pub pos: Vec2,
    pub han: bool,
}

/// Empty the brick tiles a body smashed through.
pub fn smash(active: &mut ActiveLevel, cells: impl Iterator<Item = (i32, i32)>, out: &mut MessageWriter<BrickSmashed>) {
    for (c, r) in cells {
        let level = &mut active.level;
        if c < 0 || r < 0 || c >= level.width as i32 || r >= level.height as i32 {
            continue;
        }
        let i = r as usize * level.width + c as usize;
        if level.tiles[i] == Tile::Brick {
            level.tiles[i] = Tile::Empty;
            out.write(BrickSmashed { col: c as usize, row: r as usize, pos: level.tile_center(c as usize, r as usize) });
        }
    }
}

/// A crusher hit its stop: where its face is (world px), and which way it slammed.
#[derive(Message, Debug, Clone, Copy)]
pub struct CrusherThud {
    pub face: Vec2,
    pub dir: CrushDir,
    /// Its face's half width (px).
    pub half: f32,
}

/// Crushers follow their cycle.
pub(super) fn move_crushers(
    clock: Res<SimClock>,
    groove: Res<Groove>,
    mut q: Query<(&mut CrusherBlock, &mut Pos)>,
    mut thuds: MessageWriter<CrusherThud>,
) {
    let t = crush_time(&clock, &groove);
    for (mut block, mut pos) in &mut q {
        let at = block.at(t);
        let (_, slamming) = Crusher::offset(&block.def, t);
        if block.slamming != slamming {
            if block.slamming {
                let (face, half) = match block.def.dir {
                    CrushDir::Down => (at - Vec2::new(0.0, block.half.y), block.half.x),
                    CrushDir::Left => (at - Vec2::new(block.half.x, 0.0), block.half.y),
                    CrushDir::Right => (at + Vec2::new(block.half.x, 0.0), block.half.y),
                };
                thuds.write(CrusherThud { face, dir: block.def.dir, half });
            }
            block.slamming = slamming;
        }
        pos.0 = at;
    }
}

/// The crushers' blocks now (min, max): `step_body`'s `solids`.
pub fn solids<'a>(q: impl Iterator<Item = (&'a CrusherBlock, &'a Pos)>) -> Vec<(Vec2, Vec2)> {
    q.map(|(b, p)| b.bounds(p.0)).collect()
}

/// A crumbling platform's state (on its tile's entity).
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Reflect)]
#[reflect(Component)]
pub enum Crumble {
    #[default]
    Whole,
    /// Someone stood on it this long ago (s).
    Cracking(f32),
    /// Fell this long ago (s).
    Fallen(f32),
}

/// Crumbling platforms crack under feet, fall, and come back.
#[allow(clippy::type_complexity)]
fn crumble(
    time: Res<Time>,
    mut active: ResMut<ActiveLevel>,
    mut tiles: Query<(&LevelTile, &mut Crumble)>,
    bodies: Query<(&Pos, &Body), (Or<(With<Player>, With<Han>)>, Without<Dead>, Without<Finished>)>,
) {
    let dt = time.delta_secs();
    let level = &active.level;
    // Cells stood on, and cells someone's in.
    let mut under: Vec<(i32, i32)> = Vec::new();
    let mut inside: Vec<(Vec2, Vec2)> = Vec::new();
    for (pos, body) in &bodies {
        inside.push((pos.0 - body.half, pos.0 + body.half));
        if !body.on_ground {
            continue;
        }
        let feet = pos.0.y - body.half.y - 1.0;
        for x in [pos.0.x - body.half.x + 0.5, pos.0.x + body.half.x - 0.5] {
            under.push(level.cell_at(Vec2::new(x, feet)));
        }
    }
    let mut changes: Vec<(usize, Tile)> = Vec::new();
    for (t, mut state) in &mut tiles {
        let cell = (t.col as i32, t.row as i32);
        let next = match *state {
            Crumble::Whole if under.contains(&cell) => Crumble::Cracking(0.0),
            Crumble::Whole => continue,
            Crumble::Cracking(s) if s + dt >= CRUMBLE_DELAY => {
                changes.push((t.row * level.width + t.col, Tile::Empty));
                Crumble::Fallen(0.0)
            }
            Crumble::Cracking(s) => Crumble::Cracking(s + dt),
            Crumble::Fallen(s) if s + dt >= CRUMBLE_BACK => {
                let c = level.tile_center(t.col, t.row);
                let (min, max) = (c - TILE / 2.0, c + TILE / 2.0);
                if inside.iter().any(|&(a, b)| a.x < max.x && b.x > min.x && a.y < max.y && b.y > min.y) {
                    Crumble::Fallen(s + dt)
                } else {
                    changes.push((t.row * level.width + t.col, Tile::Crumble));
                    Crumble::Whole
                }
            }
            Crumble::Fallen(s) => Crumble::Fallen(s + dt),
        };
        *state = next;
    }
    if !changes.is_empty() {
        for (i, tile) in changes {
            active.level.tiles[i] = tile;
        }
    }
}

/// Beat blocks follow the music's bars: red on even bars, blue on odd ones. One coming back
/// where a body is waits until it's clear (nobody gets stuck in a block).
#[allow(clippy::type_complexity)]
pub(super) fn beat_blocks(
    groove: Res<Groove>,
    mut active: ResMut<ActiveLevel>,
    bodies: Query<(&Pos, &Body), (Or<(With<Player>, With<Han>)>, Without<Dead>)>,
    mut cells: Local<Vec<usize>>,
) {
    let level = &active.level;
    // (A scan is a few thousand compares: cheaper than keeping a list in step with level loads.)
    cells.clear();
    cells.extend(level.tiles.iter().enumerate().filter(|(_, t)| t.is_beat()).map(|(i, _)| i));
    if cells.is_empty() {
        return;
    }
    let odd = groove.clock.bar % 2 == 1;
    let inside: Vec<(Vec2, Vec2)> = bodies.iter().map(|(p, b)| (p.0 - b.half, p.0 + b.half)).collect();
    let mut changes: Vec<(usize, Tile)> = Vec::new();
    for &i in cells.iter() {
        let now = level.tiles[i];
        let next = now.on_bar(odd);
        if next == now {
            continue;
        }
        if next.is_solid() {
            let c = level.tile_center(i % level.width, i / level.width);
            let (min, max) = (c - TILE / 2.0, c + TILE / 2.0);
            if inside.iter().any(|&(a, b)| a.x < max.x && b.x > min.x && a.y < max.y && b.y > min.y) {
                continue;
            }
        }
        changes.push((i, next));
    }
    if !changes.is_empty() {
        for (i, t) in changes {
            active.level.tiles[i] = t;
        }
    }
}
