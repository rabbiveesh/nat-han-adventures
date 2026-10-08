//! Level validator: is a level well-formed, beatable, and does it teach what it asks?
//! Pure (no Bevy app, no threads, no environment), so it runs in tests, tools and, later, the
//! game itself (procedural free play), wasm included. `tests/levels.rs` runs it on the
//! campaign and adds the campaign's plan.
//!
//! The heart of it is a conservative reachability search: from each "standable" cell we simulate
//! a fan of jumps (hold lengths, running/standing starts, toot double-jump timings, air control)
//! with the real tuning constants and tile AABB collision, and follow every arc that lands without
//! touching anything deadly. Moving platforms are approximated as one-way tiles along their whole
//! path (as if you can ride them anywhere they go). Flies are treated as static deadly squares
//! covering their whole circle; spray jets are treated as passable (they're timed).
//!
//! The music bends the physics (`Groove`), and levels have gates that need a mode ([`Gate`]):
//! - a **giant wall** (6 tiles) needs Giant Steps, which the player summons by tooting 5 times,
//!   so it must have a flat, hazard-free runway of [`RUNWAY`] tiles before it;
//! - a **long gap** (11 tiles) needs the fired-up band (quartal: faster running), summoned by
//!   grabbing 4 nuggets quickly, so it must have a nugget line right before it (no checkpoint in
//!   between: nuggets since the checkpoint come back after a splat) and a flat run-up;
//! - a **waltz row** needs the waltzing band, summoned by 3 evenly spaced ground jumps, so it
//!   must have a flat, hazard-free runway of [`WALTZ_RUNWAY`] tiles before it. It's a run of at
//!   least [`WALTZ_ROW_MIN`] *adjacent* spray cans (with gaps you could wait between the jets)
//!   under a one-way grating you walk on, with a low ceiling so nobody jumps over the jets
//!   (they're deadly here, whatever the timing). Running through is checked by simulating the
//!   jets as time-varying hazards ([`dash_through`]) with the game's own clocks: impossible at
//!   any phase with the normal shared timing (1.0 s on / 1.5 s off) at the top speed of every
//!   other mode (fired up included), possible in the waltz (on for the big ONE's beat, 2.5 s
//!   off) at a human 90% of top speed. The waltz's jump on ONE (×1.15, weak toot) is checked
//!   like any other mode: it must not climb a giant wall.
//! - a **grease chute** needs sweaty grip: the nervous band (3+ deaths) lets Nat brake and
//!   jump on grease. Without grip grease can't be jumped from (see `game::physics`), so the
//!   validator never takes off from grease in other modes; a chute is a grease floor whose
//!   only way on is a jump from the grease. Deaths are always at hand: every grease run must end
//!   in something deadly ([`grease_runs`]), so a sliding player can splat out (and the splats
//!   are what make the band nervous). The take-off of a chute crossing is any reached grease cell.
//! - a **stain pit** needs splats: a run of at least [`STAIN_PIT_MIN`] floor spikes too wide to
//!   jump in any mode. Each death on a spike leaves a standable stain there; the player can
//!   choose where to die by jumping in. The search places stains where a (human) arc dies, one
//!   death at a time, keeping the [`PIT_BEAM`] most promising stain sets (those that open the
//!   most new places to splat) until normal jumps reach past the pit: at most
//!   [`MAX_PIT_DEATHS`] deaths, at least one (it must be impassable without stains in every mode).
//!
//! Reachability starts with normal physics (with a human margin); from every reached cell that
//! has a runway / nugget line / grease it also tries that mode's jumps ("crossings"), and
//! carries on with normal physics on the far side; when nothing else is new, it tries the
//! stain pits. Every mode crossing must be impossible in the other modes, even with ideal input
//! (full speed, any toot timing, hanging off the very edge), and impossible with normal physics
//! after splats everywhere ([`Report`] "leak" errors: every spike near the gate stained and a
//! raft on every pool it could die in, all at once). So no splat stain, and no raft, ever opens
//! a gate meant for a mode. (Long gaps are bottomless for that reason: a raft over an 11-tile
//! pool is a stepping stone for two normal jumps.)
//!
//! Teaching ([`Lesson`]): for each mechanic ([`Topic`]) the validator finds its first occurrence
//! by path cost from the start (the first jump only a toot makes, the first `=` stood on, the
//! first platform ridden, the first fly or can within 4 tiles, the first take-off of each gate,
//! the first grease) and the `hint@` spots with that topic, heard within
//! [`HINT_RADIUS`](super::HINT_RADIUS) of a reached cell. A hint must come no later than (and
//! at most [`HINT_LEAD`] before) what it teaches; the campaign test asks the first level with each
//! mechanic to have one.
//!
//! Speed: the map is a dense flag grid; per mode the free-flight arc of every strategy is
//! simulated once and its envelope kept ([`Arcs`]): with collisions an arc only ever ends up
//! lower and less far than in free flight, so the gate checks skip every (cell, strategy) that
//! can't land in the region they look for.

use std::collections::{BinaryHeap, HashMap, HashSet};

use super::{Level, MAX_LINE, TILE, ThingKind, Tile, Topic};
use crate::audio::{Harmony, director::NERVOUS_DEATHS, waltz::WALTZ_BPM};
use crate::game::{
    BeatClock, FORGIVE, Groove, SPRAY_CYCLE, SPRAY_WIDTH, WALTZ_ONE_BOOST, WALTZ_ONE_TOOT_SPEED, spray_on,
    tuning::*,
};

pub const NUGGETS: std::ops::RangeInclusive<usize> = 15..=40;
/// Path cost (tiles moved, Manhattan per hop) allowed between respawn points: about 30s of play.
pub const MAX_SEGMENT: u32 = 150;
/// A stain pit or grease chute (where you die on purpose) must have a respawn point at most
/// this far (path cost) before it: every splat is a short walk back.
pub const DEATH_GATE_RESPAWN: u32 = 60;
/// Fly swarms circle ~1 tile around their cell; treat the whole circle (plus a bit) as deadly.
pub const FLY_REACH: f32 = 20.0;
/// Flat, hazard-free tiles to toot 5 times on before a giant wall.
pub const RUNWAY: usize = 8;
/// Flat tiles of run-up behind a long gap's take-off, and how far back (tiles) the nugget line
/// that fires up the band may start.
pub const RUN_UP: usize = 4;
pub const NUGGET_LINE_REACH: i32 = 30;
/// A detour behind a gate must hold at least this many nuggets (if the goal isn't behind it).
pub const DETOUR_NUGGETS: usize = 3;
/// Rows above a runway tile that must be free of hazards (a toot goes ~5 tiles up).
pub const RUNWAY_HEADROOM: i32 = 5;
/// Flat, hazard-free tiles to jump in threes on before a waltz row.
pub const WALTZ_RUNWAY: usize = 6;
/// Adjacent spray cans that make a waltz row.
pub const WALTZ_ROW_MIN: usize = 4;
/// Adjacent floor spikes that make a stain pit.
pub const STAIN_PIT_MIN: usize = 8;
/// Most splats a stain pit may need, and how many stain sets the search keeps per depth.
pub const MAX_PIT_DEATHS: u32 = 4;
pub const PIT_BEAM: usize = 4;
/// Splat spots the pit search tries per stain set (the farthest jumps first).
pub const PIT_TARGETS: usize = 6;
/// A hint may come at most this much path cost before what it teaches.
pub const HINT_LEAD: u32 = 80;
/// Mechanics "appear" when a reached cell is this close (tiles, each axis) to a fly or a can.
const THING_NEAR: i32 = 4;

const HALF_W: f32 = PLAYER_SIZE.0 / 2.0;
const HALF_H: f32 = PLAYER_SIZE.1 / 2.0;
/// The game's own fixed step (60 Hz), integrated in the game's order.
const DT: f32 = 1.0 / 60.0;
const MAX_T: f32 = 3.0;

pub type Cell = (i32, i32);

/// The physics modes the music can put the player in.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Mode {
    Normal,
    GiantSteps,
    FiredUp,
    Waltz,
    /// The nervous band: normal physics plus sweaty grip on grease.
    Nervous,
}

pub const MODES: [Mode; 5] = [Mode::Normal, Mode::GiantSteps, Mode::FiredUp, Mode::Waltz, Mode::Nervous];

impl Mode {
    pub fn groove(self) -> Groove {
        Groove::of(match self {
            Mode::Normal => Harmony::Original,
            Mode::GiantSteps => Harmony::Coltrane,
            Mode::FiredUp => Harmony::Quartal,
            Mode::Waltz => Harmony::Waltz,
            Mode::Nervous => Harmony::MelodicMinor,
        })
    }

    /// Is the spray jet firing at time `t` (s since the music / level clock started)?
    pub fn jets_on(self, t: f32) -> bool {
        let g = self.groove();
        if g.waltz() {
            let beat = 60.0 / WALTZ_BPM as f64;
            g.at(BeatClock::at(t as f64 / beat, beat, 3)).waltz_spray_on()
        } else {
            spray_on(0, t)
        }
    }

    /// How long the jets' pattern takes to repeat.
    pub fn jets_period(self) -> f32 {
        if self == Mode::Waltz { 2.0 * 3.0 * 60.0 / WALTZ_BPM } else { SPRAY_CYCLE }
    }
}

/// Something in a level that has to be crossed a special way.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Gate {
    GiantWall,
    LongGap,
    WaltzRow,
    GreaseChute,
    StainPit,
}

impl Gate {
    pub fn name(self) -> &'static str {
        match self {
            Gate::GiantWall => "giant wall",
            Gate::LongGap => "long gap",
            Gate::WaltzRow => "waltz row",
            Gate::GreaseChute => "grease chute",
            Gate::StainPit => "stain pit",
        }
    }

    /// The mode that crosses it (stain pits: none, normal jumps over your own stains).
    pub fn mode(self) -> Option<Mode> {
        match self {
            Gate::GiantWall => Some(Mode::GiantSteps),
            Gate::LongGap => Some(Mode::FiredUp),
            Gate::WaltzRow => Some(Mode::Waltz),
            Gate::GreaseChute => Some(Mode::Nervous),
            Gate::StainPit => None,
        }
    }

    pub fn topic(self) -> Topic {
        match self {
            Gate::GiantWall => Topic::Giant,
            Gate::LongGap => Topic::Gap,
            Gate::WaltzRow => Topic::Waltz,
            Gate::GreaseChute => Topic::Grip,
            Gate::StainPit => Topic::Stain,
        }
    }

    fn mark(self) -> char {
        match self {
            Gate::GiantWall => 'W',
            Gate::LongGap => 'R',
            Gate::WaltzRow => 'Z',
            Gate::GreaseChute => 'Y',
            Gate::StainPit => 'K',
        }
    }
}

/// The physics of a mode, as the validator simulates them.
#[derive(Clone, Copy, Debug)]
pub struct Env {
    pub gravity: f32,
    pub max_fall: f32,
    pub air_accel: f32,
    /// Running speed the simulated player uses.
    pub vx: f32,
    /// How far (px) the 12px box dares to hang over a ledge before taking off.
    pub overhang: f32,
    /// Ground jump speed multiplier: the waltz's jump on ONE (the waltz's off-beat jumps are
    /// [`Mode::Normal`]'s, checked as that mode).
    pub boost: f32,
    /// Upward speed of the toot: the weak toot after a jump on ONE in the waltz.
    pub toot: f32,
    /// Sweaty grip: grease is just ground.
    pub grip: bool,
}

impl Env {
    /// `ideal`: full top speed and hanging off the very edge; otherwise a human margin (90% of
    /// top speed, 8px of overhang).
    pub fn new(mode: Mode, ideal: bool) -> Env {
        let g = mode.groove();
        let waltz = mode == Mode::Waltz;
        Env {
            gravity: GRAVITY * g.gravity_scale,
            max_fall: MAX_FALL * g.fall_scale(),
            air_accel: AIR_ACCEL * g.speed_scale,
            vx: RUN_SPEED * g.speed_scale * if ideal { 1.0 } else { 0.9 },
            overhang: if ideal { PLAYER_SIZE.0 - 0.5 } else { 8.0 },
            boost: if waltz { WALTZ_ONE_BOOST } else { 1.0 },
            toot: if waltz { WALTZ_ONE_TOOT_SPEED } else { DOUBLE_JUMP_SPEED },
            grip: g.grip(),
        }
    }
}

// Cell flags.
const SOLID: u16 = 1;
const ONEWAY: u16 = 2;
const VIRT: u16 = 4;
const STAIN: u16 = 8;
const GREASE: u16 = 16;
const SPIKE_UP: u16 = 32;
const SPIKE_DOWN: u16 = 64;
const LIQUID: u16 = 128;
const JET: u16 = 256;
const SPRAY: u16 = 512;
const FLY: u16 = 1024;
const RAFT: u16 = 2048;
const DEADLY_TILE: u16 = SPIKE_UP | SPIKE_DOWN | LIQUID;
const FLOOR_ONEWAY: u16 = ONEWAY | VIRT | STAIN | RAFT;

/// A run of adjacent spray cans `c0..=c1` sitting in row `row`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaltzRow {
    pub row: i32,
    pub c0: i32,
    pub c1: i32,
}

impl WaltzRow {
    pub fn cans(&self) -> usize {
        (self.c1 - self.c0 + 1) as usize
    }

    /// The row the player stands in on the grating above the cans.
    pub fn walk_row(&self) -> i32 {
        self.row - 2
    }
}

/// A run of adjacent floor spikes `c0..=c1` in row `row`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pit {
    pub row: i32,
    pub c0: i32,
    pub c1: i32,
}

impl Pit {
    fn contains(&self, (c, r): Cell) -> bool {
        r == self.row && (self.c0..=self.c1).contains(&c)
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Floor {
    None,
    OneWay,
    Solid,
}

/// Collision-relevant view of a level (pixel space here is x right, y DOWN, origin top-left),
/// with whatever stains have been made so far.
#[derive(Clone)]
pub struct Map<'a> {
    pub level: &'a Level,
    w: i32,
    h: i32,
    flags: Vec<u16>,
    flies: Vec<(f32, f32)>,
    pub waltz_rows: Vec<WaltzRow>,
    pub pits: Vec<Pit>,
    has_grease: bool,
}

impl<'a> Map<'a> {
    pub fn new(level: &'a Level) -> Self {
        let (w, h) = (level.width as i32, level.height as i32);
        let mut flags = vec![0u16; (w * h) as usize];
        let at = |c: i32, r: i32| (c >= 0 && r >= 0 && c < w && r < h).then(|| (r * w + c) as usize);
        for r in 0..h {
            for c in 0..w {
                flags[(r * w + c) as usize] = match level.tile(c, r) {
                    Tile::Empty => 0,
                    Tile::Solid => SOLID,
                    Tile::Grease => SOLID | GREASE,
                    Tile::OneWay => ONEWAY,
                    Tile::StainUp | Tile::StainDown => STAIN,
                    Tile::SpikesUp => SPIKE_UP,
                    Tile::SpikesDown => SPIKE_DOWN,
                    Tile::Liquid => LIQUID,
                };
            }
        }
        for p in &level.platforms {
            let steps = ((p.dx.abs() + p.dy.abs()) * 2.0).ceil().max(1.0) as i32;
            for s in 0..=steps {
                let f = s as f32 / steps as f32;
                let c = p.col as i32 + (p.dx * f).round() as i32;
                let r = p.row as i32 - (p.dy * f).round() as i32;
                for k in 0..p.width as i32 {
                    if let Some(i) = at(c + k, r) {
                        flags[i] |= VIRT;
                    }
                }
            }
        }
        let mut flies = Vec::new();
        let mut cans: Vec<Cell> = Vec::new();
        for t in &level.things {
            match t.kind {
                ThingKind::Fly => {
                    let (fx, fy) = (t.col as f32 * TILE + 8.0, t.row as f32 * TILE + 8.0);
                    flies.push((fx, fy));
                    let lo = |v: f32| ((v - FLY_REACH) / TILE).floor() as i32;
                    let hi = |v: f32| ((v + FLY_REACH) / TILE).floor() as i32;
                    for c in lo(fx)..=hi(fx) {
                        for r in lo(fy)..=hi(fy) {
                            if let Some(i) = at(c, r) {
                                flags[i] |= FLY;
                            }
                        }
                    }
                }
                ThingKind::Spray => {
                    for k in 0..=3 {
                        if let Some(i) = at(t.col as i32, t.row as i32 - k) {
                            flags[i] |= SPRAY;
                        }
                    }
                    cans.push((t.col as i32, t.row as i32));
                }
                _ => {}
            }
        }
        cans.sort_by_key(|&(c, r)| (r, c));
        let mut waltz_rows: Vec<WaltzRow> = Vec::new();
        for (c, r) in cans {
            match waltz_rows.last_mut() {
                Some(w) if w.row == r && w.c1 + 1 == c => w.c1 = c,
                _ => waltz_rows.push(WaltzRow { row: r, c0: c, c1: c }),
            }
        }
        waltz_rows.retain(|w| w.cans() >= WALTZ_ROW_MIN);
        for wr in &waltz_rows {
            for c in wr.c0..=wr.c1 {
                for k in 1..=3 {
                    if let Some(i) = at(c, wr.row - k) {
                        flags[i] |= JET;
                    }
                }
            }
        }
        let mut pits = Vec::new();
        for r in 0..h {
            let mut c = 0;
            while c < w {
                if level.tile(c, r) == Tile::SpikesUp {
                    let c0 = c;
                    while c + 1 < w && level.tile(c + 1, r) == Tile::SpikesUp {
                        c += 1;
                    }
                    if (c - c0 + 1) as usize >= STAIN_PIT_MIN {
                        pits.push(Pit { row: r, c0, c1: c });
                    }
                }
                c += 1;
            }
        }
        Map { level, w, h, flags, flies, waltz_rows, pits, has_grease: level.has_grease() }
    }

    #[inline(always)]
    fn f(&self, c: i32, r: i32) -> u16 {
        if c < 0 || c >= self.w {
            SOLID
        } else if r < 0 || r >= self.h {
            0
        } else {
            self.flags[(r * self.w + c) as usize]
        }
    }

    fn set(&mut self, (c, r): Cell, on: u16, off: u16) {
        if c >= 0 && r >= 0 && c < self.w && r < self.h {
            let i = (r * self.w + c) as usize;
            self.flags[i] = (self.flags[i] & !off) | on;
        }
    }

    /// A splat on the spikes at `cell` (a stain) or in the pool at `cell` (a raft on its
    /// surface). Returns the cell the stain makes standable.
    pub fn splat(&mut self, (c, r): Cell) -> Option<Cell> {
        let f = self.f(c, r);
        if f & (SPIKE_UP | SPIKE_DOWN) != 0 {
            self.set((c, r), STAIN, SPIKE_UP | SPIKE_DOWN);
            Some((c, r - 1))
        } else if f & LIQUID != 0 {
            let mut top = r;
            while top > 0 && self.f(c, top - 1) & LIQUID != 0 {
                top -= 1;
            }
            self.set((c, top), RAFT, 0);
            Some((c, top - 1))
        } else {
            None
        }
    }

    pub fn is_solid(&self, c: i32, r: i32) -> bool {
        self.f(c, r) & SOLID != 0
    }

    fn floor(&self, c: i32, r: i32) -> Floor {
        let f = self.f(c, r);
        if f & SOLID != 0 {
            Floor::Solid
        } else if f & FLOOR_ONEWAY != 0 {
            Floor::OneWay
        } else {
            Floor::None
        }
    }

    /// Standing in `cell` on grease.
    pub fn greasy(&self, (c, r): Cell) -> bool {
        self.f(c, r + 1) & GREASE != 0
    }

    fn deadly_tile(&self, c: i32, r: i32) -> bool {
        self.f(c, r) & DEADLY_TILE != 0
    }

    pub fn in_fly_zone(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> bool {
        self.flies.iter().any(|&(fx, fy)| {
            x1 > fx - FLY_REACH && x0 < fx + FLY_REACH && y1 > fy - FLY_REACH && y0 < fy + FLY_REACH
        })
    }

    /// What the player box (center x,y) dies of: `None` if nothing, `Some(Some(cell))` for
    /// spikes or liquid at `cell` (a splat there leaves a stain), `Some(None)` for anything else.
    fn hazard(&self, x: f32, y: f32) -> Option<Option<Cell>> {
        let (x0, x1, y0, y1) = (x - HALF_W, x + HALF_W, y - HALF_H, y + HALF_H);
        if y0 > self.h as f32 * TILE {
            return Some(None);
        }
        // Fast path: nothing dangerous in any cell of the box.
        let (cs, rs) = (cells(x0, x1), cells(y0, y1));
        let mut any = 0;
        for c in cs.clone() {
            for r in rs.clone() {
                any |= self.f(c, r);
            }
        }
        if any & (DEADLY_TILE | JET | FLY) == 0 {
            return None;
        }
        let mut stainable: Option<(f32, Cell)> = None;
        let mut other = false;
        let mut flies = false;
        for c in cs {
            for r in rs.clone() {
                let f = self.f(c, r);
                if f == 0 || f == SOLID {
                    continue;
                }
                let ty = r as f32 * TILE;
                let hit = (f & LIQUID != 0)
                    || (f & SPIKE_UP != 0 && y1 > ty + TILE / 2.0)
                    || (f & SPIKE_DOWN != 0 && y0 < ty + TILE / 2.0);
                if hit {
                    let d = (c as f32 * TILE + 8.0 - x).abs() + (ty + 8.0 - y).abs();
                    if stainable.is_none_or(|(b, _)| d < b) {
                        stainable = Some((d, (c, r)));
                    }
                }
                other |= f & JET != 0;
                flies |= f & FLY != 0;
            }
        }
        if let Some((_, cell)) = stainable {
            return Some(Some(cell));
        }
        if other || (flies && self.in_fly_zone(x0, y0, x1, y1)) {
            return Some(None);
        }
        None
    }

    fn deadly(&self, x: f32, y: f32) -> bool {
        self.hazard(x, y).is_some()
    }

    pub fn standable(&self, (c, r): Cell) -> bool {
        if c < 0 || c >= self.w || r < 0 || r + 1 >= self.h {
            return false;
        }
        let f = self.f(c, r);
        f & SOLID == 0
            && f & DEADLY_TILE == 0
            && self.floor(c, r + 1) != Floor::None
            && !self.deadly(c as f32 * TILE + 8.0, (r + 1) as f32 * TILE - HALF_H)
    }

    /// Standable on real, dry ground (not a moving platform's path, not grease, not a stain),
    /// with nothing deadly in the [`RUNWAY_HEADROOM`] rows above: somewhere to hop up and down
    /// in peace.
    pub fn calm(&self, (c, r): Cell) -> bool {
        let below = self.f(c, r + 1);
        self.standable((c, r))
            && ((below & SOLID != 0 && below & GREASE == 0) || below & ONEWAY != 0)
            && (0..=RUNWAY_HEADROOM).all(|k| {
                let f = self.f(c, r - k);
                f & (DEADLY_TILE | SPRAY) == 0
            })
            && !self.in_fly_zone(
                c as f32 * TILE,
                (r - RUNWAY_HEADROOM) as f32 * TILE,
                (c + 1) as f32 * TILE,
                (r + 1) as f32 * TILE,
            )
    }

    /// The flat, calm stretch of floor `cell` is on (empty if `cell` itself isn't calm).
    pub fn flat_run(&self, (c, r): Cell) -> Vec<Cell> {
        if !self.calm((c, r)) {
            return Vec::new();
        }
        let mut run = vec![(c, r)];
        for d in [-1, 1] {
            let mut k = c + d;
            while self.calm((k, r)) {
                run.push((k, r));
                k += d;
            }
        }
        run
    }
}

/// Tile indices overlapped by the open pixel interval (a, b).
#[inline(always)]
fn cells(a: f32, b: f32) -> std::ops::Range<i32> {
    ((a / TILE).floor() as i32)..(((b - 0.001) / TILE).floor() as i32 + 1)
}

#[derive(Clone, Copy, Debug)]
pub struct Strategy {
    pub dir: f32,
    pub jump: bool,
    /// Release jump after this long (variable jump height).
    pub hold: f32,
    /// Starting speed, as a fraction of the mode's running speed (signed).
    pub vx0: f32,
    /// Toot double jump at this time.
    pub toot: Option<f32>,
    /// Start hanging over the edge of the cell in `dir`.
    pub edge: bool,
    /// Let go of the direction key after this long.
    pub release: f32,
}

/// The jumps a human might try.
pub fn strategies() -> Vec<Strategy> {
    let toots = [None, Some(0.12), Some(0.2), Some(0.27), Some(0.36), Some(0.5)];
    let mut out = Vec::new();
    // Straight up (and maybe toot).
    for hold in [f32::INFINITY, 0.12, 0.05] {
        for toot in toots {
            let release = f32::INFINITY;
            out.push(Strategy { dir: 0.0, jump: true, hold, vx0: 0.0, toot, edge: false, release });
        }
    }
    for dir in [-1.0, 1.0] {
        for toot in toots {
            for release in [f32::INFINITY, 0.15, 0.3] {
                // Jumps: standing or running start, from the middle or the very edge of the cell.
                for hold in [f32::INFINITY, 0.12, 0.05] {
                    for vx0 in [0.0, dir] {
                        for edge in [false, true] {
                            out.push(Strategy { dir, jump: true, hold, vx0, toot, edge, release });
                        }
                    }
                }
                // Running off a ledge (coyote jumps are covered by edge jumps).
                let hold = f32::INFINITY;
                out.push(Strategy { dir, jump: false, hold, vx0: dir, toot, edge: true, release });
            }
        }
    }
    out
}

/// Jumps for proving something is impossible: full holds, every toot timing (one per frame).
pub fn ideal_strategies() -> Vec<Strategy> {
    let toots: Vec<Option<f32>> = std::iter::once(None).chain((3..=80).map(|f| Some(f as f32 / 60.0))).collect();
    let mut out = Vec::new();
    let release = f32::INFINITY;
    let hold = f32::INFINITY;
    for &toot in &toots {
        out.push(Strategy { dir: 0.0, jump: true, hold, vx0: 0.0, toot, edge: false, release });
        for dir in [-1.0, 1.0] {
            for vx0 in [0.0, dir] {
                for edge in [false, true] {
                    out.push(Strategy { dir, jump: true, hold, vx0, toot, edge, release });
                }
            }
            out.push(Strategy { dir, jump: false, hold, vx0: dir, toot, edge: true, release });
        }
    }
    out
}

/// Envelope rows: landings from 12 tiles above to 48 below the take-off.
const DR_MIN: i32 = -12;
const DR_MAX: i32 = 48;

/// A mode's strategies with their free-flight envelopes: for each row offset of a landing,
/// how far (px, relative to the take-off x) the arc can be while it's still at or above that
/// height. Collisions only ever make an arc lower (y down: larger) and nearer than in free
/// flight, so a landing outside the envelope is impossible.
pub struct Arcs {
    pub env: Env,
    pub strategies: Vec<Strategy>,
    envelope: Vec<[Option<(f32, f32)>; (DR_MAX - DR_MIN + 1) as usize]>,
}

impl Arcs {
    pub fn new(env: Env, strategies: Vec<Strategy>) -> Arcs {
        let envelope = strategies.iter().map(|s| free_envelope(&env, s)).collect();
        Arcs { env, strategies, envelope }
    }

    /// Can strategy `k` from `from` (x0 = the take-off box center) possibly land in row `r`,
    /// somewhere in columns `c0..=c1`?
    fn may_land(&self, k: usize, from: Cell, x0: f32, r: i32, c0: i32, c1: i32) -> bool {
        let dr = r - from.1;
        if !(DR_MIN..=DR_MAX).contains(&dr) {
            return true;
        }
        let Some((lo, hi)) = self.envelope[k][(dr - DR_MIN) as usize] else { return false };
        // The box (center in [x0+lo, x0+hi]) must overlap one of the columns.
        let (a, b) = (c0 as f32 * TILE - HALF_W - 1.0, (c1 + 1) as f32 * TILE + HALF_W + 1.0);
        x0 + hi > a && x0 + lo < b
    }
}

fn free_envelope(env: &Env, s: &Strategy) -> [Option<(f32, f32)>; (DR_MAX - DR_MIN + 1) as usize] {
    let mut out = [None; (DR_MAX - DR_MIN + 1) as usize];
    let (mut x, mut y) = (0.0f32, 0.0f32);
    let mut vx = s.vx0 * env.vx;
    let boost = if s.jump { env.boost } else { 1.0 };
    let mut vy = if s.jump { -JUMP_SPEED * boost } else { 0.0 };
    let mut cut = !s.jump;
    let mut tooted = false;
    let (mut lo, mut hi) = (0.0f32, 0.0f32);
    let mut t = 0.0;
    // (y, x extent so far) at every step.
    let mut steps: Vec<(f32, f32, f32)> = vec![(0.0, 0.0, 0.0)];
    while t < MAX_T {
        t += DT;
        let input = if t < s.release { s.dir } else { 0.0 };
        let target = input * env.vx;
        let dv = env.air_accel * DT;
        vx = if (target - vx).abs() <= dv { target } else { vx + dv * (target - vx).signum() };
        if !cut && t >= s.hold {
            cut = true;
            if vy < 0.0 {
                vy *= JUMP_CUT;
            }
        }
        if let Some(tt) = s.toot
            && !tooted
            && t >= tt
        {
            tooted = true;
            vy = -env.toot;
        }
        vy = (vy + env.gravity * DT).min(env.max_fall);
        x += vx * DT;
        y += vy * DT;
        lo = lo.min(x);
        hi = hi.max(x);
        steps.push((y, lo, hi));
    }
    // For each row offset dr (a landing with the box center at y = dr*TILE): the x extent up
    // to the last time the arc was at or above that height. Going backwards in time, the rows
    // not yet assigned are always all those above the highest point seen since.
    let mut k = DR_MAX;
    for &(y, lo, hi) in steps.iter().rev() {
        while k >= DR_MIN && k as f32 * TILE >= y {
            out[(k - DR_MIN) as usize] = Some((lo, hi));
            k -= 1;
        }
    }
    out
}

/// How one simulated arc ends.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Outcome {
    /// Landed safely, standing in this cell.
    Land(Cell),
    /// Splatted; on spikes/liquid at this cell, if so.
    Died(Option<Cell>),
    /// Not a jump you can make from here, or nothing came of it.
    Nothing,
}

/// Where strategy `s` takes off from in cell (c, r) (box center x), if it applies there.
fn take_off(map: &Map, env: &Env, (c, r): Cell, s: &Strategy) -> Option<f32> {
    if s.jump && !env.grip && map.greasy((c, r)) {
        return None; // can't jump off grease
    }
    let mut x = c as f32 * TILE + 8.0;
    if s.edge {
        let n = (c + s.dir as i32, r);
        if map.is_solid(n.0, n.1) || map.standable(n) {
            return None; // not a ledge
        }
        x += s.dir * (8.0 + env.overhang - HALF_W);
    }
    Some(x)
}

/// One arc from cell (c, r); the cells the box touched go into `touched`.
fn simulate(map: &Map, env: &Env, (_, r): Cell, s: &Strategy, x0: f32, touched: &mut Vec<Cell>) -> Outcome {
    touched.clear();
    let mut x = x0;
    let mut y = (r + 1) as f32 * TILE - HALF_H;
    if map.deadly(x, y) {
        return Outcome::Nothing;
    }
    let mut vx = s.vx0 * env.vx;
    let boost = if s.jump { env.boost } else { 1.0 };
    let mut vy = if s.jump { -JUMP_SPEED * boost } else { 0.0 };
    let mut cut = !s.jump;
    let mut tooted = false;
    let mut t = 0.0;
    while t < MAX_T {
        t += DT;
        // Input.
        let input = if t < s.release { s.dir } else { 0.0 };
        let target = input * env.vx;
        let dv = env.air_accel * DT;
        vx = if (target - vx).abs() <= dv { target } else { vx + dv * (target - vx).signum() };
        if !cut && t >= s.hold {
            cut = true;
            if vy < 0.0 {
                vy *= JUMP_CUT;
            }
        }
        if let Some(tt) = s.toot
            && !tooted
            && t >= tt
        {
            tooted = true;
            vy = -env.toot;
        }
        vy = (vy + env.gravity * DT).min(env.max_fall);

        // Horizontal move against solids.
        let nx = x + vx * DT;
        let (y0, y1) = (y - HALF_H, y + HALF_H);
        let mut blocked = false;
        for col in cells(nx - HALF_W, nx + HALF_W) {
            for row in cells(y0, y1) {
                if map.is_solid(col, row) {
                    blocked = true;
                    x = if vx > 0.0 { col as f32 * TILE - HALF_W } else { (col + 1) as f32 * TILE + HALF_W };
                }
            }
        }
        if blocked {
            vx = 0.0;
        } else {
            x = nx;
        }

        // Vertical move.
        let ny = y + vy * DT;
        let mut landed = None;
        if vy > 0.0 {
            let (old_bottom, new_bottom) = (y + HALF_H, ny + HALF_H);
            let first = (old_bottom / TILE).ceil() as i32;
            let last = (new_bottom / TILE).floor() as i32;
            'rows: for row in first.min(last + 1)..=last {
                let top = row as f32 * TILE;
                if top < old_bottom - 0.01 {
                    continue;
                }
                let center = (x / TILE).floor() as i32;
                let mut hit = None;
                for col in cells(x - HALF_W, x + HALF_W) {
                    if map.floor(col, row) != Floor::None && (hit.is_none() || col == center) {
                        hit = Some(col);
                    }
                }
                if let Some(col) = hit {
                    y = top - HALF_H;
                    landed = Some((col, row - 1));
                    break 'rows;
                }
            }
            if landed.is_none() {
                y = ny;
            }
        } else {
            let mut hit = false;
            for col in cells(x - HALF_W, x + HALF_W) {
                for row in cells(ny - HALF_H, ny + HALF_H) {
                    if map.is_solid(col, row) {
                        hit = true;
                        y = (row + 1) as f32 * TILE + HALF_H;
                    }
                }
            }
            if hit {
                vy = 0.0;
            } else {
                y = ny;
            }
        }

        if let Some(death) = map.hazard(x, y) {
            return Outcome::Died(death);
        }
        for col in cells(x - HALF_W, x + HALF_W) {
            for row in cells(y - HALF_H, y + HALF_H) {
                touched.push((col, row));
            }
        }
        if let Some(cell) = landed {
            return if map.standable(cell) { Outcome::Land(cell) } else { Outcome::Nothing };
        }
    }
    Outcome::Nothing
}

/// The reachability graph.
pub struct Graph {
    w: i32,
    h: i32,
    /// Outgoing edges per reachable standable cell.
    edges: Vec<Option<Vec<Cell>>>,
    /// Every cell some safe arc (or standing) touches.
    touched: Vec<bool>,
    /// Which crossing first made each reachable cell reachable (`Some(None)`: normal physics
    /// from the start).
    origin: Vec<Option<Option<usize>>>,
    /// Reached cells, in the order they were reached.
    pub order: Vec<Cell>,
    /// Spike/liquid cells some human arc from a reached cell dies on: the origin of the first
    /// such take-off cell, and up to a few take-off cells.
    deaths: HashMap<Cell, (Option<usize>, Vec<Cell>)>,
}

impl Graph {
    fn new(w: i32, h: i32) -> Graph {
        let n = (w * h) as usize;
        Graph {
            w,
            h,
            edges: vec![None; n],
            touched: vec![false; n],
            origin: vec![None; n],
            order: Vec::new(),
            deaths: HashMap::new(),
        }
    }

    fn idx(&self, (c, r): Cell) -> Option<usize> {
        (c >= 0 && r >= 0 && c < self.w && r < self.h).then(|| (r * self.w + c) as usize)
    }

    pub fn reached(&self, cell: Cell) -> bool {
        self.idx(cell).is_some_and(|i| self.edges[i].is_some())
    }

    pub fn touched(&self, cell: Cell) -> bool {
        self.idx(cell).is_some_and(|i| self.touched[i])
    }

    fn touch(&mut self, cell: Cell) {
        if let Some(i) = self.idx(cell) {
            self.touched[i] = true;
        }
    }

    fn origin(&self, cell: Cell) -> Option<Option<usize>> {
        self.idx(cell).and_then(|i| self.origin[i])
    }

    fn edges(&self, cell: Cell) -> &[Cell] {
        self.idx(cell).and_then(|i| self.edges[i].as_deref()).unwrap_or(&[])
    }

    /// Reached before crossing `i` (from the start, or through an earlier crossing).
    fn before(&self, cell: Cell, i: usize) -> bool {
        self.origin(cell).is_some_and(|o| o.is_none_or(|o| o < i))
    }
}

/// Follow every normal-physics arc from `seeds`, adding to `g`; returns the cells added.
fn explore(map: &Map, arcs: &Arcs, seeds: Vec<Cell>, origin: Option<usize>, g: &mut Graph) -> Vec<Cell> {
    let mut added = Vec::new();
    let mut queue = seeds;
    let mut buf = Vec::new();
    while let Some(cell) = queue.pop() {
        if g.reached(cell) {
            continue;
        }
        g.touch(cell);
        let mut out: Vec<Cell> = Vec::new();
        for d in [-1, 1] {
            let n = (cell.0 + d, cell.1);
            if map.standable(n) {
                out.push(n);
            }
        }
        for s in &arcs.strategies {
            let Some(x0) = take_off(map, &arcs.env, cell, s) else { continue };
            match simulate(map, &arcs.env, cell, s, x0, &mut buf) {
                Outcome::Land(land) => {
                    if !out.contains(&land) {
                        out.push(land);
                    }
                    for &t in &buf {
                        g.touch(t);
                    }
                }
                Outcome::Died(Some(d)) => {
                    let e = g.deaths.entry(d).or_insert((origin, Vec::new()));
                    if e.1.len() < 3 && !e.1.contains(&cell) {
                        e.1.push(cell);
                    }
                }
                _ => {}
            }
        }
        out.retain(|&c| c != cell);
        queue.extend(out.iter().copied().filter(|&c| !g.reached(c)));
        let i = g.idx(cell).expect("standable cells are in bounds");
        g.edges[i] = Some(out);
        g.origin[i] = Some(origin);
        g.order.push(cell);
        added.push(cell);
    }
    added
}

/// Can a player running at `vx` px/s get past `n` adjacent spray jets that fire when
/// `mode.jets_on(t)`? Simulated like the game: 60 Hz steps, the jets updated before the player
/// moves, the hit test after, with the game's forgiving hitbox. The player waits just outside
/// the jets for the best moment (every start phase is tried) and enters at full speed.
pub fn dash_through(n: usize, vx: f32, mode: Mode) -> bool {
    const STEP: f32 = 1.0 / 60.0;
    // The player's center is in danger within this distance of a jet's center.
    let reach = HALF_W - FORGIVE + SPRAY_WIDTH / 2.0;
    let zone = TILE * (n - 1) as f32 + 2.0 * reach;
    let period = mode.jets_period();
    let starts = (period / STEP).round() as usize;
    (0..starts).any(|k| {
        let t0 = k as f32 * STEP;
        let mut x = 0.0;
        let mut i = 0;
        loop {
            i += 1;
            x += vx * STEP;
            if x >= zone {
                return true;
            }
            if mode.jets_on(t0 + i as f32 * STEP) {
                return false;
            }
        }
    })
}

/// Problems with a waltz row's timing: it must stop every other mode cold and let the waltz
/// through.
pub fn waltz_row_timing(w: &WaltzRow) -> Vec<String> {
    let mut errs = Vec::new();
    for mode in [Mode::Normal, Mode::GiantSteps, Mode::FiredUp] {
        let vx = Env::new(mode, true).vx;
        if dash_through(w.cans(), vx, mode) {
            errs.push(format!(
                "waltz row at col {}..={} row {} ({} cans) can be run through with {mode:?} physics and normal spray timing",
                w.c0,
                w.c1,
                w.row,
                w.cans()
            ));
        }
    }
    if !dash_through(w.cans(), Env::new(Mode::Waltz, false).vx, Mode::Waltz) {
        errs.push(format!(
            "waltz row at col {}..={} row {} ({} cans) is too long to dash through even in the waltz",
            w.c0,
            w.c1,
            w.row,
            w.cans()
        ));
    }
    errs
}

/// Is there a ceiling at most 2 tiles above the grating all along the row (so nobody can
/// jump clear of the jets, which reach 2 tiles above it)?
fn low_ceiling(map: &Map, w: &WaltzRow) -> bool {
    let r = w.walk_row();
    (w.c0..=w.c1).all(|c| (1..=2).any(|k| map.is_solid(c, r - k)))
}

/// The waltz dash along row `w` from the approach cell `from`, heading `dir`: the landing cell
/// on the far side, if the corridor is walkable (grating all along) and `from` has a runway to
/// jump in threes on.
fn waltz_dash(map: &Map, w: &WaltzRow, from: Cell, dir: i32) -> Option<Cell> {
    let r = w.walk_row();
    let (start, end) = if dir > 0 { (w.c0 - 1, w.c1 + 1) } else { (w.c1 + 1, w.c0 - 1) };
    if from != (start, r) {
        return None;
    }
    // The runway: calm, flat floor behind the approach, with room to hop.
    let run = map.flat_run(from);
    let behind: Vec<&Cell> = run.iter().filter(|c| (c.0 - from.0) * dir <= 0).collect();
    let roomy = behind.iter().filter(|c| (1..=2).all(|k| !map.is_solid(c.0, c.1 - k))).count();
    if roomy < WALTZ_RUNWAY {
        return None;
    }
    if !low_ceiling(map, w) {
        return None;
    }
    // The corridor: a floor under every step, nothing solid or otherwise deadly in the way.
    let walkable = (w.c0..=w.c1)
        .all(|c| map.floor(c, r + 1) != Floor::None && !map.is_solid(c, r) && !map.deadly_tile(c, r));
    (walkable && map.standable((end, r))).then_some((end, r))
}

/// A gate crossed: from `from` (on its runway / after its nugget line / on grease / at the pit's
/// edge) to `to`.
pub struct Crossing {
    pub gate: Gate,
    pub from: Cell,
    pub to: Cell,
    /// Cells first reachable through it.
    pub new: Vec<Cell>,
    /// Splats it takes (stain pits), and where the stains go.
    pub stains: Vec<Cell>,
    /// The map's flags just before it (stains made by then).
    flags: Vec<u16>,
}

/// Can the player be in `mode` when taking off from `cell` in direction `dir`?
fn ready(map: &Map, g: &Graph, mode: Mode, cell: Cell, dir: i32) -> bool {
    match mode {
        Mode::Normal => true,
        // Room to toot 5 times.
        Mode::GiantSteps => map.flat_run(cell).len() >= RUNWAY,
        // Waltz rows are dashed, not jumped (see `waltz_dash`).
        Mode::Waltz => false,
        // Splat at the end of the grease run (there's always something deadly there, see
        // `grease_runs`) until the band is nervous, then jump off the grease.
        Mode::Nervous => map.greasy(cell),
        Mode::FiredUp => {
            if dir == 0 {
                return false;
            }
            let run = map.flat_run(cell);
            if run.iter().filter(|c| (c.0 - cell.0) * dir <= 0).count() < RUN_UP {
                return false;
            }
            // The 4 nearest reachable nuggets behind the take-off.
            let mut line: Vec<i32> = map
                .level
                .things
                .iter()
                .filter(|t| t.kind == ThingKind::Nugget)
                .map(|t| (t.col as i32, t.row as i32))
                .filter(|&(c, r)| {
                    let back = (cell.0 - c) * dir;
                    (0..=NUGGET_LINE_REACH).contains(&back) && (cell.1 - 4..=cell.1 + 1).contains(&r) && g.touched((c, r))
                })
                .map(|(c, _)| (cell.0 - c) * dir)
                .collect();
            line.sort();
            if line.len() < 4 {
                return false;
            }
            let farthest = line[3];
            // No checkpoint between the line and the take-off (nuggets since the checkpoint come
            // back after a splat, so every try gets the line again).
            !map.level.checkpoints().any(|t| {
                let back = (cell.0 - t.col as i32) * dir;
                (0..=farthest).contains(&back) && (t.row as i32 - cell.1).abs() <= 6
            })
        }
    }
}

/// The simulated physics, per mode: human strategies (reachability) and ideal ones (proofs).
pub struct Physics {
    human: Vec<Arcs>,
    ideal: Vec<Arcs>,
    no_toot: Arcs,
}

impl Physics {
    pub fn new() -> Physics {
        let human = MODES.iter().map(|&m| Arcs::new(Env::new(m, false), strategies())).collect();
        let ideal = MODES.iter().map(|&m| Arcs::new(Env::new(m, true), ideal_strategies())).collect();
        let no_toot =
            Arcs::new(Env::new(Mode::Normal, false), strategies().into_iter().filter(|s| s.toot.is_none()).collect());
        Physics { human, ideal, no_toot }
    }

    /// Physics whose reachability tries only `human` jumps (the proofs keep every ideal one).
    /// Fewer jumps reach less, so whatever a level passes with them it passes with
    /// [`strategies`] too: free play (`crate::freeplay`) validates rooms with a lean set, fast
    /// enough to do while the game runs.
    pub fn with_human(human: Vec<Strategy>) -> Physics {
        let ideal = MODES.iter().map(|&m| Arcs::new(Env::new(m, true), ideal_strategies())).collect();
        let no_toot = Arcs::new(Env::new(Mode::Normal, false), human.iter().copied().filter(|s| s.toot.is_none()).collect());
        let human = MODES.iter().map(|&m| Arcs::new(Env::new(m, false), human.clone())).collect();
        Physics { human, ideal, no_toot }
    }

    pub fn human(&self, m: Mode) -> &Arcs {
        &self.human[MODES.iter().position(|&x| x == m).unwrap()]
    }

    pub fn ideal(&self, m: Mode) -> &Arcs {
        &self.ideal[MODES.iter().position(|&x| x == m).unwrap()]
    }
}

impl Default for Physics {
    fn default() -> Self {
        Self::new()
    }
}

/// Stain pit search: the fewest splats (beam search, at most [`MAX_PIT_DEATHS`]) that let
/// normal jumps past `pit`. Returns the stains, the take-off cell and the first cell past it.
fn cross_pit(map: &Map, phys: &Physics, g: &Graph, pit: &Pit) -> Option<(Vec<Cell>, Cell, Cell)> {
    let arcs = phys.human(Mode::Normal);
    // Death spots in this pit reached so far, with their take-off cells.
    let start: Vec<(Cell, Cell)> = g
        .deaths
        .iter()
        .filter(|(d, _)| pit.contains(**d))
        .flat_map(|(d, (_, froms))| froms.iter().map(move |f| (*d, *f)))
        .collect();
    if start.is_empty() {
        return None;
    }
    let first_from: HashMap<Cell, Cell> = {
        let mut m = HashMap::new();
        let mut sorted = start.clone();
        sorted.sort();
        for (d, f) in sorted {
            m.entry(d).or_insert(f);
        }
        m
    };
    // A state: stains so far, cells reached on them, death spots open from them (and those
    // the latest stain opened: the next splat goes there; splats anywhere else could have come
    // earlier, in another order).
    struct State {
        stains: Vec<Cell>,
        reached: HashSet<Cell>,
        open: Vec<(Cell, Cell)>,
        fresh: Vec<(Cell, Cell)>,
    }
    let mut beam = vec![State { stains: Vec::new(), reached: HashSet::new(), open: start.clone(), fresh: start }];
    let mut buf = Vec::new();
    for _depth in 0..MAX_PIT_DEATHS {
        let mut next: Vec<(usize, State)> = Vec::new();
        let mut seen: HashSet<Vec<Cell>> = HashSet::new();
        for st in &beam {
            // The farthest splats from where they're jumped from first (a stain helps most at
            // the end of a jump), at most PIT_TARGETS of them.
            let mut targets: Vec<(i32, Cell)> =
                st.fresh.iter().map(|&(d, f)| (-((d.0 - f.0).abs() + (d.1 - f.1).abs()), d)).collect();
            targets.sort();
            let mut picked: Vec<Cell> = Vec::new();
            for (_, d) in targets {
                if !picked.contains(&d) && picked.len() < PIT_TARGETS {
                    picked.push(d);
                }
            }
            for d in picked {
                let mut stains = st.stains.clone();
                stains.push(d);
                stains.sort();
                if !seen.insert(stains.clone()) {
                    continue;
                }
                let mut m = map.clone();
                let mut tops = Vec::new();
                for &s in &stains {
                    if let Some(top) = m.splat(s) {
                        tops.push(top);
                    }
                }
                // Reach the new stain top again from where the splat jump started (or from
                // another stain), then explore on the stains.
                let froms: Vec<Cell> = st.open.iter().filter(|o| o.0 == d).map(|o| o.1).collect();
                let mut reached = st.reached.clone();
                let mut queue: Vec<Cell> = Vec::new();
                for &from in &froms {
                    for s in &arcs.strategies {
                        let Some(x0) = take_off(&m, &arcs.env, from, s) else { continue };
                        if let Outcome::Land(l) = simulate(&m, &arcs.env, from, s, x0, &mut buf)
                            && !g.reached(l)
                            && reached.insert(l)
                        {
                            queue.push(l);
                        }
                    }
                }
                let mut open: Vec<(Cell, Cell)> = Vec::new();
                let mut crossed: Option<(Cell, Cell)> = None;
                let mut visited: HashSet<Cell> = HashSet::new();
                while let Some(cell) = queue.pop() {
                    if !visited.insert(cell) {
                        continue;
                    }
                    if !tops.contains(&cell) {
                        crossed = Some((cell, cell));
                        break;
                    }
                    for dd in [-1, 1] {
                        let n = (cell.0 + dd, cell.1);
                        if m.standable(n) && !g.reached(n) && reached.insert(n) {
                            queue.push(n);
                        }
                    }
                    for s in &arcs.strategies {
                        let Some(x0) = take_off(&m, &arcs.env, cell, s) else { continue };
                        match simulate(&m, &arcs.env, cell, s, x0, &mut buf) {
                            Outcome::Land(l) if !g.reached(l) && reached.insert(l) => queue.push(l),
                            Outcome::Died(Some(dc)) if pit.contains(dc) && !stains.contains(&dc) => {
                                open.push((dc, cell))
                            }
                            _ => {}
                        }
                    }
                }
                if let Some((to, _)) = crossed {
                    let from = first_from.get(&stains[0]).copied().unwrap_or(froms[0]);
                    return Some((stains, from, to));
                }
                // Keep the old open spots too (die elsewhere next time).
                for &o in &st.open {
                    if !stains.contains(&o.0) {
                        open.push(o);
                    }
                }
                let fresh: Vec<(Cell, Cell)> =
                    open.iter().copied().filter(|o| !st.open.iter().any(|p| p.0 == o.0)).collect();
                let mut cells: Vec<Cell> = fresh.iter().map(|o| o.0).collect();
                cells.sort();
                cells.dedup();
                next.push((cells.len(), State { stains, reached, open, fresh }));
            }
        }
        next.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.stains.cmp(&b.1.stains)));
        beam = next.into_iter().take(PIT_BEAM).map(|(_, s)| s).collect();
        if beam.is_empty() {
            break;
        }
    }
    None
}

/// Normal-physics reachability from the start, plus gate crossings from wherever a mode can be
/// summoned (and stain pits when nothing else is new), until nothing new is reached. Stains
/// made for pits stay in `map`.
fn reach(map: &mut Map, phys: &Physics, start: Cell) -> (Graph, Vec<Crossing>) {
    let mut g = Graph::new(map.w, map.h);
    explore(map, phys.human(Mode::Normal), vec![start], None, &mut g);
    let mut crossings: Vec<Crossing> = Vec::new();
    let mut tried: Vec<bool> = vec![false; (map.w * map.h) as usize];
    let mut pits_done: Vec<bool> = vec![false; map.pits.len()];
    let mut buf = Vec::new();
    loop {
        let mut todo: Vec<Cell> = g.order.iter().copied().filter(|&c| !tried[g.idx(c).unwrap()]).collect();
        if todo.is_empty() {
            // Nothing new: splat into a stain pit.
            let mut crossed = false;
            for (k, pit) in map.pits.clone().iter().enumerate() {
                if pits_done[k] {
                    continue;
                }
                if let Some((stains, from, to)) = cross_pit(map, phys, &g, pit) {
                    pits_done[k] = true;
                    let flags = map.flags.clone();
                    let mut seeds = Vec::new();
                    for &s in &stains {
                        seeds.extend(map.splat(s));
                    }
                    seeds.push(to);
                    seeds.retain(|&c| map.standable(c) && !g.reached(c));
                    let i = crossings.len();
                    if let Some(e) = g.idx(from).and_then(|ix| g.edges[ix].as_mut()) {
                        e.extend(seeds.iter().copied());
                    }
                    let new = explore(map, phys.human(Mode::Normal), seeds, Some(i), &mut g);
                    crossings.push(Crossing { gate: Gate::StainPit, from, to, new, stains, flags });
                    crossed = true;
                    break;
                }
            }
            if !crossed {
                break;
            }
            continue;
        }
        todo.sort();
        let mut found: Vec<(Gate, Cell, Cell, Vec<Cell>)> = Vec::new();
        for &cell in &todo {
            tried[g.idx(cell).unwrap()] = true;
            for mode in [Mode::GiantSteps, Mode::FiredUp, Mode::Nervous] {
                if mode == Mode::Nervous && !map.has_grease {
                    continue;
                }
                let arcs = phys.human(mode);
                let ok = [-1, 0, 1].map(|d| ready(map, &g, mode, cell, d));
                if !ok.iter().any(|&b| b) {
                    continue;
                }
                let gate = match mode {
                    Mode::GiantSteps => Gate::GiantWall,
                    Mode::FiredUp => Gate::LongGap,
                    _ => Gate::GreaseChute,
                };
                for s in &arcs.strategies {
                    if !ok[(s.dir as i32 + 1) as usize] {
                        continue;
                    }
                    let Some(x0) = take_off(map, &arcs.env, cell, s) else { continue };
                    if let Outcome::Land(land) = simulate(map, &arcs.env, cell, s, x0, &mut buf)
                        && !g.reached(land)
                    {
                        found.push((gate, cell, land, buf.clone()));
                    }
                }
            }
            for w in &map.waltz_rows {
                for dir in [-1, 1] {
                    if let Some(land) = waltz_dash(map, w, cell, dir)
                        && !g.reached(land)
                    {
                        let touched = (w.c0..=w.c1).map(|c| (c, w.walk_row())).collect();
                        found.push((Gate::WaltzRow, cell, land, touched));
                    }
                }
            }
        }
        for (gate, from, to, touched) in found {
            if g.reached(to) {
                continue;
            }
            let i = g.idx(from).expect("reached");
            g.edges[i].as_mut().expect("reached").push(to);
            for t in touched {
                g.touch(t);
            }
            let flags = map.flags.clone();
            let new = explore(map, phys.human(Mode::Normal), vec![to], Some(crossings.len()), &mut g);
            crossings.push(Crossing { gate, from, to, new, stains: Vec::new(), flags });
        }
    }
    (g, crossings)
}

/// Can some ideal arc in `arcs` from one of `near` land in `beyond` (on `map`)? The first
/// such (from, to).
///
/// `extra`: more cells (splat stains) the player may get to on the way: landing on one, or
/// walking onto one from another, carries on from there.
fn leak(
    map: &Map,
    arcs: &Arcs,
    near: &[Cell],
    beyond: &HashSet<Cell>,
    extra: &HashSet<Cell>,
) -> Option<(Cell, Cell)> {
    // Targets, per row: the column range (for pruning with the envelopes).
    let mut rows: HashMap<i32, (i32, i32)> = HashMap::new();
    for &(c, r) in beyond.iter().chain(extra) {
        let e = rows.entry(r).or_insert((c, c));
        e.0 = e.0.min(c);
        e.1 = e.1.max(c);
    }
    let mut buf = Vec::new();
    let mut frontier: Vec<Cell> = near.to_vec();
    let mut visited: HashSet<Cell> = HashSet::new();
    while !frontier.is_empty() {
        let mut next = Vec::new();
        for &from in &frontier {
            for (k, s) in arcs.strategies.iter().enumerate() {
                let Some(x0) = take_off(map, &arcs.env, from, s) else { continue };
                if !rows.iter().any(|(&r, &(c0, c1))| arcs.may_land(k, from, x0, r, c0, c1)) {
                    continue;
                }
                if let Outcome::Land(l) = simulate(map, &arcs.env, from, s, x0, &mut buf) {
                    if beyond.contains(&l) {
                        return Some((from, l));
                    }
                    if extra.contains(&l) && visited.insert(l) {
                        next.push(l);
                    }
                }
            }
            if extra.contains(&from) {
                for d in [-1, 1] {
                    let n = (from.0 + d, from.1);
                    if beyond.contains(&n) {
                        return Some((from, n));
                    }
                    if extra.contains(&n) && visited.insert(n) {
                        next.push(n);
                    }
                }
            }
        }
        frontier = next;
    }
    None
}

/// Problems with crossing `i`: it must be impossible in every other mode (and, for a stain pit,
/// in every mode without its stains), even with ideal input, from anywhere near its take-off
/// reached before it; and a mode gate must not open to normal physics after splats.
fn exclusive(map: &Map, phys: &Physics, g: &Graph, crossings: &[Crossing], i: usize) -> Vec<String> {
    let c = &crossings[i];
    let beyond: HashSet<Cell> = c.new.iter().copied().collect();
    let near: Vec<Cell> = g
        .order
        .iter()
        .copied()
        .filter(|&cell| g.before(cell, i) && (cell.0 - c.from.0).abs() <= 12 && (cell.1 - c.from.1).abs() <= 8)
        .collect();
    let before = Map { flags: c.flags.clone(), ..map.clone() };
    let mut errs = Vec::new();
    let what = format!("{} from col {} row {} (to col {} row {})", c.gate.name(), c.from.0, c.from.1, c.to.0, c.to.1);
    for mode in MODES {
        if Some(mode) == c.gate.mode() || (mode == Mode::Nervous && !map.has_grease) {
            continue;
        }
        if let Some((from, to)) = leak(&before, phys.ideal(mode), &near, &beyond, &HashSet::new()) {
            errs.push(format!(
                "{what} is passable with {mode:?} physics too: col {} row {} -> col {} row {}",
                from.0, from.1, to.0, to.1
            ));
        }
    }
    if c.gate != Gate::StainPit {
        // Splat everywhere near the gate (every death spot reached before it, all at once) and
        // try again with normal physics, from the stains too.
        let mut stained = before.clone();
        let mut tops = HashSet::new();
        for (&d, (o, _)) in &g.deaths {
            if o.is_none_or(|o| o < i) && (d.0 - c.from.0).abs() <= 16 && (d.1 - c.from.1).abs() <= 10 {
                tops.extend(stained.splat(d));
            }
        }
        tops.retain(|&t| stained.standable(t) && !g.reached(t));
        if !tops.is_empty() {
            for mode in [Mode::Normal, Mode::Waltz] {
                if Some(mode) == c.gate.mode() {
                    continue;
                }
                if let Some((f, to)) = leak(&stained, phys.ideal(mode), &near, &beyond, &tops) {
                    errs.push(format!(
                        "{what} is passable with {mode:?} physics after splat stains/rafts: col {} row {} -> col {} row {}",
                        f.0, f.1, to.0, to.1
                    ));
                }
            }
        }
    }
    errs
}

/// The crossings on the way to `cell` (latest first).
fn needs(g: &Graph, crossings: &[Crossing], cell: Cell) -> Vec<usize> {
    let mut chain = Vec::new();
    let mut at = cell;
    while let Some(Some(i)) = g.origin(at) {
        chain.push(i);
        at = crossings[i].from;
    }
    chain
}

/// Shortest path costs (Manhattan tiles per hop) from `from` to every reached cell.
fn distances(g: &Graph, from: Cell) -> HashMap<Cell, u32> {
    let mut dist: HashMap<Cell, u32> = HashMap::new();
    let mut heap = BinaryHeap::new();
    dist.insert(from, 0);
    heap.push(std::cmp::Reverse((0u32, from)));
    while let Some(std::cmp::Reverse((d, c))) = heap.pop() {
        if dist.get(&c).is_some_and(|&b| d > b) {
            continue;
        }
        for &n in g.edges(c) {
            let nd = d + ((n.0 - c.0).abs() + (n.1 - c.1).abs()) as u32;
            if dist.get(&n).is_none_or(|&b| nd < b) {
                dist.insert(n, nd);
                heap.push(std::cmp::Reverse((nd, n)));
            }
        }
    }
    dist
}

/// Horizontal runs of standable grease cells: each must end (at least on one side) in something
/// deadly, so a player sliding on it can always splat out (and make the band nervous).
fn grease_runs(map: &Map) -> Vec<String> {
    let mut errs = Vec::new();
    for r in 0..map.h {
        let mut c = 0;
        while c < map.w {
            let slick = |c: i32| map.greasy((c, r)) && !map.is_solid(c, r) && !map.deadly_tile(c, r);
            if slick(c) {
                let c0 = c;
                while slick(c + 1) {
                    c += 1;
                }
                let ends = [(c0 - 1, r), (c + 1, r)];
                if !ends.iter().any(|&(ec, er)| map.deadly_tile(ec, er)) {
                    errs.push(format!(
                        "grease at col {c0}..={c} row {r} must end in spikes (so a sliding player can always splat out)"
                    ));
                }
            }
            c += 1;
        }
    }
    errs
}

/// A mechanic's first appearance, and the hint that teaches it.
#[derive(Debug, Clone)]
pub struct Lesson {
    pub topic: Topic,
    /// Where the player first meets it (path cost `dist` from the start).
    pub at: Cell,
    pub dist: u32,
    /// The best hint with this topic: where, and the path cost at which it's first heard.
    pub hint: Option<(Cell, u32)>,
}

impl Lesson {
    /// The hint is heard no later than the mechanic and not long before it.
    pub fn taught(&self) -> bool {
        self.hint.is_some_and(|(_, d)| d <= self.dist && self.dist - d <= HINT_LEAD)
    }
}

/// What [`check`] found.
#[derive(Debug, Default)]
pub struct Report {
    pub errs: Vec<String>,
    /// Gates crossed: (gate, leads to the goal, nuggets behind it, splats it takes).
    pub gates: Vec<(Gate, bool, usize, u32)>,
    /// The goal can only be reached through a gate.
    pub gated_goal: bool,
    /// Deaths the design needs on the way to the goal: splats for stain pits, plus the
    /// nervous band's [`NERVOUS_DEATHS`] for grease chutes.
    pub deaths: u32,
    /// Every mechanic the level has, first appearance first.
    pub lessons: Vec<Lesson>,
    /// Respawn-to-respawn path costs.
    pub segments: Vec<u32>,
    /// The map with reachable cells marked (if asked for).
    pub dump: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    /// Render [`Report::dump`].
    pub dump: bool,
}

fn line_errors(level: &Level) -> Vec<String> {
    let mut errs = Vec::new();
    let cps: Vec<Cell> = level.checkpoints().map(|t| (t.col as i32, t.row as i32)).collect();
    let lines = level.checkpoint_lines();
    for (k, l) in lines.iter().enumerate() {
        if l.is_none() {
            errs.push(format!("checkpoint at col {} row {} has no line (say@)", cps[k].0, cps[k].1));
        }
    }
    let used = level.say_at.len() + level.says.len();
    if used > lines.iter().filter(|l| l.is_some()).count() {
        errs.push("more say lines than checkpoints".into());
    }
    for s in &level.say_at {
        if !cps.contains(&(s.col as i32, s.row as i32)) {
            errs.push(format!("say@{},{} is not on a checkpoint", s.col, s.row));
        }
    }
    for h in &level.hints {
        if level.tile(h.col as i32, h.row as i32).is_solid() {
            errs.push(format!("hint@{},{} is inside the ground", h.col, h.row));
        }
    }
    if level.intro.is_empty() {
        errs.push("missing intro".into());
    }
    let all = std::iter::once(&level.intro)
        .chain(&level.says)
        .chain(level.say_at.iter().map(|s| &s.text))
        .chain(level.hints.iter().map(|s| &s.text));
    for line in all {
        if line.chars().count() > MAX_LINE {
            errs.push(format!("line longer than {MAX_LINE} chars: {line:?}"));
        }
    }
    errs
}

/// Validates one level.
pub fn check(level: &Level, opts: &Options) -> Report {
    check_with(level, opts, &Physics::new())
}

/// [`check`] with the physics tables already built (they take a moment).
pub fn check_with(level: &Level, opts: &Options, phys: &Physics) -> Report {
    check_impl(level, opts, phys, true)
}

/// [`check_with`] for a generated room (free play, `crate::freeplay`): the deaths its design
/// needs are worked out and reported in [`Report::deaths`] (the room's expected deaths) instead
/// of checked against its `deaths:` line.
pub fn check_room(level: &Level, phys: &Physics) -> Report {
    check_impl(level, &Options::default(), phys, false)
}

fn check_impl(level: &Level, opts: &Options, phys: &Physics, check_deaths: bool) -> Report {
    let mut errs = line_errors(level);
    let mut map = Map::new(level);
    let cps: Vec<Cell> = level.checkpoints().map(|t| (t.col as i32, t.row as i32)).collect();
    if !(1..=3).contains(&cps.len()) {
        errs.push(format!("{} checkpoints (want 1..=3)", cps.len()));
    }
    let n = level.nugget_count();
    if !NUGGETS.contains(&n) {
        errs.push(format!("{n} nuggets (want {NUGGETS:?})"));
    }
    if level.height < 14 || level.width < 40 {
        errs.push(format!("level is only {}x{}", level.width, level.height));
    }

    let start = (level.start.0 as i32, level.start.1 as i32);
    let goal = (level.goal.0 as i32, level.goal.1 as i32);
    for (what, cell) in [("start P", start), ("goal G", goal)].into_iter().chain(cps.iter().map(|&c| ("checkpoint", c)))
    {
        if !map.standable(cell) {
            errs.push(format!("{what} at col {} row {} is not standing on safe ground", cell.0, cell.1));
        }
        if map.f(cell.0, cell.1) & SPRAY != 0 {
            errs.push(format!("{what} at col {} row {} is in a spray jet", cell.0, cell.1));
        }
    }
    if map.is_solid(goal.0, goal.1 - 1) {
        errs.push(format!("goal flag at col {} row {} has no room (2 tiles tall)", goal.0, goal.1));
    }
    // Safe start: nothing deadly within 3 tiles.
    for dc in -3..=3 {
        for dr in -3..=3 {
            let c = (start.0 + dc, start.1 + dr);
            if map.f(c.0, c.1) & (DEADLY_TILE | SPRAY) != 0 {
                errs.push(format!("hazard at col {} row {} next to the start", c.0, c.1));
            }
        }
    }
    let (sx, sy) = (start.0 as f32 * TILE + 8.0, start.1 as f32 * TILE + 8.0);
    if map.in_fly_zone(sx - 48.0, sy - 48.0, sx + 48.0, sy + 48.0) {
        errs.push("fly swarm next to the start".into());
    }
    for w in &map.waltz_rows {
        errs.extend(waltz_row_timing(w));
        if !low_ceiling(&map, w) {
            errs.push(format!(
                "waltz row at col {}..={} row {} needs a low ceiling (solid at most 2 tiles above the grating) all along",
                w.c0, w.c1, w.row
            ));
        }
    }
    errs.extend(grease_runs(&map));
    for p in &level.platforms {
        let (c0, c1) = (p.col as f32 + p.dx.min(0.0), (p.col + p.width - 1) as f32 + p.dx.max(0.0));
        let (r0, r1) = (p.row as f32 - p.dy.max(0.0), p.row as f32 - p.dy.min(0.0));
        if c0 < 0.0 || c1 > (level.width - 1) as f32 || r0 < 0.0 || r1 > (level.height - 1) as f32 {
            errs.push(format!("platform at col {} row {} travels out of bounds", p.col, p.row));
        }
    }
    if !errs.is_empty() {
        // Structural problems: reachability would just add noise.
        return Report { errs, ..Report::default() };
    }

    let base = map.clone();
    let (g, crossings) = reach(&mut map, phys, start);
    if !g.reached(goal) {
        errs.push(format!("goal at col {} row {} is unreachable from the start", goal.0, goal.1));
    }
    for &c in &cps {
        if !g.reached(c) {
            errs.push(format!("checkpoint at col {} row {} is unreachable", c.0, c.1));
        }
    }
    let lost: Vec<String> = level
        .things
        .iter()
        .filter(|t| t.kind == ThingKind::Nugget && !g.touched((t.col as i32, t.row as i32)))
        .map(|t| format!("(col {} row {})", t.col, t.row))
        .collect();
    if lost.len() * 10 > n {
        errs.push(format!("{} of {n} nuggets unreachable: {}", lost.len(), lost.join(" ")));
    }
    for (k, pit) in map.pits.iter().enumerate() {
        let near = g.deaths.keys().any(|d| pit.contains(*d));
        let crossed = crossings.iter().any(|c| c.gate == Gate::StainPit && c.stains.iter().all(|s| pit.contains(*s)));
        if near && !crossed && !g.reached((pit.c1 + 1, pit.row)) && !g.reached((pit.c0 - 1, pit.row)) {
            errs.push(format!(
                "stain pit {k} at col {}..={} row {} can't be crossed with ≤{MAX_PIT_DEATHS} splats",
                pit.c0, pit.c1, pit.row
            ));
        }
    }

    // Gates: each one only passable its own way, and worth it (on the way to the goal, or
    // guarding a nugget-rich detour).
    let to_goal = needs(&g, &crossings, goal);
    let mut gates = Vec::new();
    let mut gate_lines = String::new();
    let mut deaths = 0;
    for (i, c) in crossings.iter().enumerate() {
        errs.extend(exclusive(&map, phys, &g, &crossings, i));
        let behind: HashSet<Cell> = c.new.iter().copied().collect();
        let nuggets = level
            .things
            .iter()
            .filter(|t| t.kind == ThingKind::Nugget)
            .filter(|t| {
                let (tc, tr) = (t.col as i32, t.row as i32);
                // Picked up from a cell first reached through this gate (or just above one).
                (0..=2).any(|k| behind.contains(&(tc, tr + k)))
            })
            .count();
        let on_way = to_goal.contains(&i);
        let splats = match c.gate {
            Gate::StainPit => c.stains.len() as u32,
            Gate::GreaseChute => NERVOUS_DEATHS,
            _ => 0,
        };
        if on_way {
            deaths += splats;
        }
        if !on_way && nuggets < DETOUR_NUGGETS {
            errs.push(format!(
                "{} from col {} row {} to col {} row {} leads nowhere much ({nuggets} nuggets, not the goal)",
                c.gate.name(),
                c.from.0,
                c.from.1,
                c.to.0,
                c.to.1
            ));
        }
        gate_lines += &format!(
            "  {} at col {} row {} -> col {} row {}: {}, {nuggets} nuggets behind it{}\n",
            c.gate.name(),
            c.from.0,
            c.from.1,
            c.to.0,
            c.to.1,
            if on_way { "on the way to the goal" } else { "detour" },
            if splats > 0 { format!(", {splats} splats") } else { String::new() }
        );
        gates.push((c.gate, on_way, nuggets, splats));
    }
    if check_deaths && level.deaths.unwrap_or(0) != deaths {
        errs.push(format!(
            "`deaths: {}` but the design needs {deaths} (stain pit splats + {NERVOUS_DEATHS} per grease chute)",
            level.deaths.unwrap_or(0)
        ));
    }

    // Respawn spacing: walk P -> checkpoints (in order of distance) -> G.
    let from_start = distances(&g, start);
    let mut segments = Vec::new();
    if errs.is_empty() {
        let mut order: Vec<(u32, Cell)> =
            cps.iter().map(|&c| (from_start.get(&c).copied().unwrap_or(u32::MAX), c)).collect();
        order.sort();
        let mut chain = vec![start];
        chain.extend(order.iter().map(|&(_, c)| c));
        chain.push(goal);
        for w in chain.windows(2) {
            match distances(&g, w[0]).get(&w[1]).copied() {
                Some(d) if d <= MAX_SEGMENT => segments.push(d),
                Some(d) => errs.push(format!(
                    "segment {:?} -> {:?} costs {d} tiles (> {MAX_SEGMENT}): add a checkpoint",
                    w[0], w[1]
                )),
                None => errs.push(format!("can't get from {:?} to {:?} (respawn chain)", w[0], w[1])),
            }
        }
        // Where you die on purpose, the walk back is short.
        let respawns: Vec<HashMap<Cell, u32>> =
            std::iter::once(start).chain(cps.iter().copied()).map(|p| distances(&g, p)).collect();
        for c in &crossings {
            if !matches!(c.gate, Gate::StainPit | Gate::GreaseChute) {
                continue;
            }
            let best = respawns.iter().filter_map(|d| d.get(&c.from)).min();
            if best.is_none_or(|&b| b > DEATH_GATE_RESPAWN) {
                errs.push(format!(
                    "{} at col {} row {}: no respawn point within {DEATH_GATE_RESPAWN} tiles before it (you splat here on purpose)",
                    c.gate.name(),
                    c.from.0,
                    c.from.1
                ));
            }
        }
    }

    let lessons = lessons(level, &base, &map, phys, &g, &crossings, &from_start);
    for l in &lessons {
        if let Some((h, hd)) = l.hint && !l.taught() {
            errs.push(format!(
                "hint@{},{} for {} is heard at path cost {hd}, but the {} is at col {} row {} (cost {}): it must come before, by at most {HINT_LEAD}",
                h.0,
                h.1,
                l.topic.word(),
                l.topic.word(),
                l.at.0,
                l.at.1,
                l.dist
            ));
        }
    }
    for h in &level.hints {
        for t in &h.topics {
            if !lessons.iter().any(|l| l.topic == *t) {
                errs.push(format!("hint@{},{} teaches {} but the level has none", h.col, h.row, t.word()));
            }
        }
    }

    let dump = opts.dump.then(|| {
        format!(
            "=== {:?}: {}x{}, {} checkpoints, {n} nuggets ({} unreachable), segments: {:?}, deaths: {deaths}\n{gate_lines}{}",
            level.name,
            level.width,
            level.height,
            cps.len(),
            lost.len(),
            segments,
            dump(level, &map, &g, &crossings)
        )
    });
    Report { errs, gates, gated_goal: !to_goal.is_empty(), deaths, lessons, segments, dump }
}

/// Every mechanic's first appearance (by path cost from the start), with its best hint.
fn lessons(
    level: &Level,
    base: &Map,
    map: &Map,
    phys: &Physics,
    g: &Graph,
    crossings: &[Crossing],
    dist: &HashMap<Cell, u32>,
) -> Vec<Lesson> {
    let mut first: HashMap<Topic, (u32, Cell)> = HashMap::new();
    let mut see = |t: Topic, cell: Cell| {
        if let Some(&d) = dist.get(&cell) {
            let e = first.entry(t).or_insert((d, cell));
            if (d, cell) < *e {
                *e = (d, cell);
            }
        }
    };
    for &cell in &g.order {
        let below = map.f(cell.0, cell.1 + 1);
        if below & ONEWAY != 0 {
            see(Topic::OneWay, cell);
        }
        if below & VIRT != 0 && below & (SOLID | ONEWAY) == 0 {
            see(Topic::Platform, cell);
        }
        if below & GREASE != 0 {
            see(Topic::Grease, cell);
        }
    }
    for t in &level.things {
        let topic = match t.kind {
            ThingKind::Fly => Topic::Fly,
            ThingKind::Spray => Topic::Spray,
            _ => continue,
        };
        for &cell in &g.order {
            if (cell.0 - t.col as i32).abs() <= THING_NEAR && (cell.1 - t.row as i32).abs() <= THING_NEAR {
                see(topic, cell);
            }
        }
    }
    for c in crossings {
        see(c.gate.topic(), c.from);
    }
    // The toot: the first jump only a toot makes (normal physics, no stains).
    {
        let start = (level.start.0 as i32, level.start.1 as i32);
        let mut a = Graph::new(base.w, base.h);
        explore(base, &phys.no_toot, vec![start], None, &mut a);
        let mut cells: Vec<(u32, Cell)> =
            a.order.iter().filter_map(|&c| dist.get(&c).map(|&d| (d, c))).collect();
        cells.sort();
        let arcs = phys.human(Mode::Normal);
        let mut buf = Vec::new();
        'cells: for (_, cell) in cells {
            for s in arcs.strategies.iter().filter(|s| s.toot.is_some()) {
                let Some(x0) = take_off(base, &arcs.env, cell, s) else { continue };
                if let Outcome::Land(l) = simulate(base, &arcs.env, cell, s, x0, &mut buf)
                    && !a.reached(l)
                {
                    see(Topic::Toot, cell);
                    break 'cells;
                }
            }
        }
    }
    let mut out: Vec<Lesson> = first
        .into_iter()
        .map(|(topic, (d, at))| {
            // Heard at the cheapest reached cell within earshot; the best hint is the latest
            // one heard no later than the lesson (else the earliest one).
            let heard: Vec<(Cell, u32)> = level
                .hints
                .iter()
                .filter(|h| h.topics.contains(&topic))
                .filter_map(|h| {
                    let hc = (h.col as i32, h.row as i32);
                    let r2 = (super::HINT_RADIUS / TILE).powi(2);
                    g.order
                        .iter()
                        .filter(|c| (((c.0 - hc.0).pow(2) + (c.1 - hc.1).pow(2)) as f32) <= r2)
                        .filter_map(|c| dist.get(c).copied())
                        .min()
                        .map(|hd| (hc, hd))
                })
                .collect();
            let hint = heard
                .iter()
                .filter(|h| h.1 <= d)
                .max_by_key(|h| h.1)
                .or_else(|| heard.iter().min_by_key(|h| h.1))
                .copied();
            Lesson { topic, at, dist: d, hint }
        })
        .collect();
    out.sort_by_key(|l| (l.dist, l.topic));
    out
}

#[allow(clippy::needless_range_loop)]
fn dump(level: &Level, map: &Map, g: &Graph, crossings: &[Crossing]) -> String {
    let mut rows: Vec<Vec<char>> = (0..level.height)
        .map(|r| {
            (0..level.width)
                .map(|c| {
                    let f = map.f(c as i32, r as i32);
                    if f & GREASE != 0 {
                        '_'
                    } else if f & SOLID != 0 {
                        '#'
                    } else if f & STAIN != 0 {
                        '%'
                    } else if f & ONEWAY != 0 {
                        '='
                    } else if f & SPIKE_UP != 0 {
                        '^'
                    } else if f & SPIKE_DOWN != 0 {
                        'v'
                    } else if f & LIQUID != 0 {
                        '~'
                    } else {
                        '.'
                    }
                })
                .collect()
        })
        .collect();
    for r in 0..level.height {
        for c in 0..level.width {
            let cell = (c as i32, r as i32);
            if rows[r][c] == '.' {
                if g.reached(cell) {
                    rows[r][c] = '+';
                } else if map.f(cell.0, cell.1) & VIRT != 0 {
                    rows[r][c] = '-';
                } else if g.touched(cell) {
                    rows[r][c] = ',';
                }
            }
        }
    }
    for p in &level.platforms {
        for k in 0..p.width {
            rows[p.row][p.col + k] = '@';
        }
    }
    for x in crossings {
        rows[x.from.1 as usize][x.from.0 as usize] = x.gate.mark();
    }
    for t in &level.things {
        let ok = g.touched((t.col as i32, t.row as i32));
        rows[t.row][t.col] = match (t.kind, ok) {
            (ThingKind::Nugget, true) => 'o',
            (ThingKind::Nugget, false) => 'X',
            (ThingKind::Checkpoint, true) => 'C',
            (ThingKind::Checkpoint, false) => '!',
            (ThingKind::Fly, _) => 'F',
            (ThingKind::Spray, _) => 'S',
        };
    }
    for h in &level.hints {
        if matches!(rows[h.row][h.col], '.' | '+' | ',') {
            rows[h.row][h.col] = '?';
        }
    }
    rows[level.start.1][level.start.0] = 'P';
    rows[level.goal.1][level.goal.0] =
        if g.reached((level.goal.0 as i32, level.goal.1 as i32)) { 'G' } else { '!' };
    let mut s = String::new();
    let ruler: String = (0..level.width).map(|c| if c % 10 == 0 { '|' } else { ' ' }).collect();
    s += &format!("    {ruler}\n");
    for (r, row) in rows.iter().enumerate() {
        s += &format!("{r:>3} {}\n", row.iter().collect::<String>());
    }
    s
}
