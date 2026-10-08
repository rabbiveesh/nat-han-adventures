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
//! covering their whole circle; spray cans and their jets are treated as passable (they're
//! timed: a can and its jet fire together, 1.0 s on / 1.5 s off, and a lone can or a few in a
//! row are a stroll in the off-beat), except in waltz and shield rows (below).
//!
//! The music bends the physics (`Groove`), and levels have gates that need a mode ([`Gate`]):
//! - a **giant wall** (6 tiles) needs Giant Steps, which the player summons by tooting 5 times,
//!   so it must have a flat, hazard-free runway of [`RUNWAY`] tiles before it;
//! - a **long gap** (11 tiles) needs the fired-up band (quartal: faster running), summoned by
//!   grabbing 4 nuggets quickly, so it must have a nugget line right before it (no checkpoint in
//!   between: nuggets since the checkpoint come back after a splat) and a flat run-up;
//! - a **waltz row** needs the waltzing band, summoned by 3 evenly spaced ground jumps, so it
//!   must have a flat, hazard-free runway of [`WALTZ_RUNWAY`] tiles before it. It's a run of at
//!   least [`WALTZ_ROW_MIN`] *adjacent* spray cans (with gaps you could wait between the jets),
//!   either on the floor you walk on (the cans are deadly while they fire) or under a one-way
//!   grating you walk on (in the jets: the campaign's tunnels), with a low ceiling so nobody
//!   jumps over the jets (they're deadly here, whatever the timing). Running through is checked
//!   by simulating the cans and jets as time-varying hazards ([`dash_through`]) with the game's
//!   own clocks: impossible at
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
//! Han follows Nat right up to these gates, but in their *band zone* his boost is the weak one
//! (`game::WEAK_BOOST_SPEED`), so the same goes for him (`boost_leak`): no full boost from
//! anywhere outside the zones Han may be (standing, or in mid-air as high as his head holds
//! Nat), no weak boost from anywhere in them Nat may stand, and no chain of boosts and toots
//! between them, opens a band or death gate, in any mode, with ideal input.
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

use super::buddy::{Chasm, HAN_CATCH_RISE, HAN_CHASM_CATCH_RISE, HanPhys, chain_cross};
use super::{GateMark, HAN_BERTH, Level, MAX_LINE, TILE, ThingKind, Tile, Topic};
use crate::audio::{Harmony, director::NERVOUS_DEATHS, waltz::WALTZ_BPM};
use crate::game::{
    BOOST_SPEED, BeatClock, CAN_WIDTH, FORGIVE, RAFT_LIFE_FLOOR, Groove, SPRAY_CYCLE, SPRAY_WIDTH, WALTZ_ONE_BOOST, WALTZ_ONE_TOOT_SPEED,
    WEAK_BOOST_SPEED, spray_on,
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
/// Adjacent spray cans that make a shield row (too long for any mode, the waltz included:
/// only behind Han going ahead).
pub const SHIELD_ROW_MIN: usize = 24;
/// Liquid surface tiles (under ceiling spikes) that make a buddy raft pool.
pub const BUDDY_POOL_MIN: usize = 10;
/// Bottomless columns that make a chain-jump chasm (wider than a long gap).
pub const CHASM_MIN: usize = 14;
/// Most of Nat's own rafts the raft-bridge search tries for a buddy raft pool.
pub const MAX_NAT_RAFTS: u32 = 6;
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
    /// Needs the plunger boost off Han's head.
    BuddyLedge,
    /// Needs Han going ahead into the jets.
    ShieldRow,
    /// Needs boosts off Han in mid-air, chained with toots.
    ChainChasm,
    /// Needs Han's big raft.
    BuddyRaft,
}

impl Gate {
    /// Han's gates (he's what crosses them).
    pub fn needs_han(self) -> bool {
        matches!(self, Gate::BuddyLedge | Gate::ShieldRow | Gate::ChainChasm | Gate::BuddyRaft)
    }

    pub fn name(self) -> &'static str {
        match self {
            Gate::GiantWall => "giant wall",
            Gate::LongGap => "long gap",
            Gate::WaltzRow => "waltz row",
            Gate::GreaseChute => "grease chute",
            Gate::StainPit => "stain pit",
            Gate::BuddyLedge => "buddy ledge",
            Gate::ShieldRow => "shield row",
            Gate::ChainChasm => "chain chasm",
            Gate::BuddyRaft => "buddy raft pool",
        }
    }

    /// The mode that crosses it (stain pits: none, normal jumps over your own stains).
    pub fn mode(self) -> Option<Mode> {
        match self {
            Gate::GiantWall => Some(Mode::GiantSteps),
            Gate::LongGap => Some(Mode::FiredUp),
            Gate::WaltzRow => Some(Mode::Waltz),
            Gate::GreaseChute => Some(Mode::Nervous),
            Gate::StainPit | Gate::BuddyLedge | Gate::ShieldRow | Gate::ChainChasm | Gate::BuddyRaft => None,
        }
    }

    pub fn topic(self) -> Topic {
        match self {
            Gate::GiantWall => Topic::Giant,
            Gate::LongGap => Topic::Gap,
            Gate::WaltzRow => Topic::Waltz,
            Gate::GreaseChute => Topic::Grip,
            Gate::StainPit => Topic::Stain,
            Gate::BuddyLedge => Topic::Boost,
            Gate::ShieldRow => Topic::Shield,
            Gate::ChainChasm => Topic::Chain,
            Gate::BuddyRaft => Topic::BuddyRaft,
        }
    }

    fn mark(self) -> char {
        match self {
            Gate::GiantWall => 'W',
            Gate::LongGap => 'R',
            Gate::WaltzRow => 'Z',
            Gate::GreaseChute => 'Y',
            Gate::StainPit => 'K',
            Gate::BuddyLedge => 'B',
            Gate::ShieldRow => 'H',
            Gate::ChainChasm => 'N',
            Gate::BuddyRaft => 'U',
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
    /// [`Mode::Normal`]'s, checked as that mode); for weak boosts ([`Launch::Weak`]), of
    /// [`WEAK_BOOST_SPEED`].
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

    /// Han's physics in `mode`, for his navigation ([`super::nav`]): his run speed and gravity
    /// ([`HanPhys`]), a human margin on ledges, and grip on grease (plumber's boots).
    pub fn han(mode: Mode) -> Env {
        let p = HanPhys::of(&mode.groove());
        Env {
            gravity: p.fall.gravity,
            max_fall: p.fall.max_fall,
            air_accel: p.air_accel,
            vx: p.speed,
            overhang: 8.0,
            boost: 1.0,
            toot: p.toot_speed,
            grip: true,
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

/// A run of adjacent spray cans `c0..=c1` sitting in row `row`: under a one-way grating
/// (`grated`, the campaign's tunnels: you walk on the grating, in the jets) or on the floor
/// you walk on (the cans themselves are deadly while they fire, see `game::CAN_WIDTH`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaltzRow {
    pub row: i32,
    pub c0: i32,
    pub c1: i32,
    pub grated: bool,
}

impl WaltzRow {
    pub fn cans(&self) -> usize {
        (self.c1 - self.c0 + 1) as usize
    }

    /// The row the player walks through the row in: on the grating above the cans, or among
    /// the cans themselves.
    pub fn walk_row(&self) -> i32 {
        if self.grated { self.row - 2 } else { self.row }
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

/// A buddy raft pool: liquid whose surface is row `row`, columns `c0..=c1`, with ceiling
/// spikes low over all of it (no jumping across) and too wide for Nat's own rafts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pool {
    pub row: i32,
    pub c0: i32,
    pub c1: i32,
}

impl Pool {
    fn contains(&self, (c, r): Cell) -> bool {
        r >= self.row && (self.c0..=self.c1).contains(&c)
    }

    /// The shore cell Nat stands on before it, heading `dir`, and the one past it.
    pub fn shores(&self, dir: i32) -> (Cell, Cell) {
        let (a, b) = ((self.c0 - 1, self.row - 1), (self.c1 + 1, self.row - 1));
        if dir > 0 { (a, b) } else { (b, a) }
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
    /// Runs of [`SHIELD_ROW_MIN`]+ cans (same shape as waltz rows).
    pub shield_rows: Vec<WaltzRow>,
    pub pools: Vec<Pool>,
    pub chasms: Vec<Chasm>,
    has_grease: bool,
    /// Generated rooms (free play) carry no `gate:` marks: the band and death gates found so far,
    /// marked as they're found (Han's boost is weak around them like around marked ones).
    derive_marks: bool,
    pub(crate) derived: Vec<GateMark>,
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
                _ => waltz_rows.push(WaltzRow { row: r, c0: c, c1: c, grated: false }),
            }
        }
        for w in &mut waltz_rows {
            w.grated = (w.c0..=w.c1).all(|c| level.tile(c, w.row - 1) == Tile::OneWay);
        }
        let shield_rows: Vec<WaltzRow> = waltz_rows.iter().copied().filter(|w| w.cans() >= SHIELD_ROW_MIN).collect();
        waltz_rows.retain(|w| (WALTZ_ROW_MIN..SHIELD_ROW_MIN).contains(&w.cans()));
        // A row's cans and jets are deadly (the timing is the dash's, see `dash_through`).
        for wr in waltz_rows.iter().chain(&shield_rows) {
            for c in wr.c0..=wr.c1 {
                for k in 0..=3 {
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
        let pools = find_pools(level);
        let chasms = find_chasms(level, &flags);
        Map {
            level,
            w,
            h,
            flags,
            flies,
            waltz_rows,
            pits,
            shield_rows,
            pools,
            chasms,
            has_grease: level.has_grease(),
            derive_marks: false,
            derived: Vec::new(),
        }
    }

    /// Han's view of the level: flies and spray jets don't hurt him (spikes and sewage still
    /// count as deadly: he keeps off them).
    pub fn for_han(level: &'a Level) -> Self {
        let mut m = Map::new(level);
        for f in &mut m.flags {
            *f &= !(FLY | JET | SPRAY);
        }
        m.flies.clear();
        m
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

    /// Is Han's boost full strength in `cell`? Not in the zone of a band or death gate (marked,
    /// or found in a room), where it's the weak one ([`Level::han_allowed`]).
    pub fn han_allowed(&self, cell: Cell) -> bool {
        !self.in_band_zone(cell)
    }

    /// Is `cell` in a band zone (marked, or found in a room)? See [`Level::in_band_zone`].
    pub fn in_band_zone(&self, cell: Cell) -> bool {
        self.level.in_band_zone(cell) || self.derived.iter().any(|g| g.berth().contains(cell))
    }

    /// Does Han keep out of `cell` (marked, or found in a room)? See [`Level::han_keeps_out`].
    pub fn han_keeps_out(&self, cell: Cell) -> bool {
        self.level.han_keeps_out(cell)
            || self.derived.iter().any(|g| match g.topic {
                Topic::Waltz => g.contains(cell),
                Topic::Grip => self.greasy(cell) && g.berth().contains(cell),
                _ => false,
            })
    }

    /// Does a player box centered at (x, y) overlap anything solid?
    pub fn overlaps_solid(&self, x: f32, y: f32) -> bool {
        cells(x - HALF_W, x + HALF_W).any(|c| cells(y - HALF_H, y + HALF_H).any(|r| self.is_solid(c, r)))
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

/// Buddy raft pools: liquid surfaces at least [`BUDDY_POOL_MIN`] wide with ceiling spikes 2-4
/// rows above every column (hanging from solid ground).
fn find_pools(level: &Level) -> Vec<Pool> {
    let (w, h) = (level.width as i32, level.height as i32);
    let surface = |c: i32, r: i32| level.tile(c, r) == Tile::Liquid && level.tile(c, r - 1) != Tile::Liquid;
    let spiked = |c: i32, r: i32| {
        (2..=4).any(|k| level.tile(c, r - k) == Tile::SpikesDown && level.tile(c, r - k - 1).is_solid())
    };
    let mut out = Vec::new();
    for r in 1..h {
        let mut c = 0;
        while c < w {
            if surface(c, r) {
                let c0 = c;
                while c + 1 < w && surface(c + 1, r) {
                    c += 1;
                }
                if (c - c0 + 1) as usize >= BUDDY_POOL_MIN && (c0..=c).all(|k| spiked(k, r)) {
                    out.push(Pool { row: r, c0, c1: c });
                }
            }
            c += 1;
        }
    }
    out
}

/// Chain-jump chasms: at least [`CHASM_MIN`] columns with nothing at all from the walk row
/// down (bottomless), between two floors at the same height.
fn find_chasms(level: &Level, flags: &[u16]) -> Vec<Chasm> {
    let (w, h) = (level.width as i32, level.height as i32);
    let open = |c: i32, r: i32| (r..h).all(|k| flags[(k * w + c) as usize] == 0);
    let floor = |c: i32, r: i32| {
        c >= 0 && c < w && r + 1 < h && {
            let t = level.tile(c, r + 1);
            (t.is_solid() || t.is_one_way()) && !level.tile(c, r).is_solid()
        }
    };
    let mut out = Vec::new();
    for r in 0..h - 1 {
        let mut c = 1;
        while c < w {
            if open(c, r) && floor(c - 1, r) {
                let c0 = c;
                while c + 1 < w && open(c + 1, r) {
                    c += 1;
                }
                if (c - c0 + 1) as usize >= CHASM_MIN && floor(c + 1, r) {
                    out.push(Chasm { row: r, c0, c1: c });
                }
            }
            c += 1;
        }
    }
    out
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

/// Envelope rows: landings from 20 tiles above (more than any arc climbs: a full boost and its
/// toot under Giant Steps are ~16) to 48 below the take-off.
const DR_MIN: i32 = -20;
const DR_MAX: i32 = 48;

/// How the arcs of an [`Arcs`] start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Launch {
    /// Standing in the take-off cell: a ground jump (or running off the edge).
    Ground,
    /// Standing on Han's head, Han standing in the take-off cell: the plunger boost
    /// ([`BOOST_SPEED`], toot refreshed). Only jumping strategies apply.
    Boost,
    /// Standing on Han's head in a band zone, Han standing in the take-off cell: the weak boost
    /// ([`WEAK_BOOST_SPEED`], toot refreshed), or stepping off his head (then a toot). Han may
    /// stand anywhere in the cell, out to hanging off a ledge, and Nat anywhere on his head:
    /// edge strategies start from the very edge of his head (off a ledge if there is one).
    Weak,
}

/// A mode's strategies with their free-flight envelopes: for each row offset of a landing,
/// how far (px, relative to the take-off x) the arc can be while it's still at or above that
/// height. Collisions only ever make an arc lower (y down: larger) and nearer than in free
/// flight, so a landing outside the envelope is impossible.
pub struct Arcs {
    pub env: Env,
    pub strategies: Vec<Strategy>,
    pub launch: Launch,
    envelope: Vec<[Option<(f32, f32)>; (DR_MAX - DR_MIN + 1) as usize]>,
}

/// Nat's box center (y down) standing on Han, relative to Nat standing on Han's floor.
const ON_HAN: f32 = -PLAYER_SIZE.1;

impl Arcs {
    pub fn new(env: Env, strategies: Vec<Strategy>) -> Arcs {
        Arcs::with_launch(env, strategies, Launch::Ground)
    }

    /// Plunger boosts off Han (the jumping strategies of `strategies`).
    pub fn boost(env: Env, strategies: Vec<Strategy>) -> Arcs {
        Arcs::with_launch(env, strategies.into_iter().filter(|s| s.jump).collect(), Launch::Boost)
    }

    /// Weak boosts off Han in a band zone (and steps off his head: the edge strategies that
    /// don't jump). `env`'s toot is the full one (a weak boost refreshes it).
    pub fn weak(env: Env, strategies: Vec<Strategy>) -> Arcs {
        let env = Env { toot: DOUBLE_JUMP_SPEED, ..env };
        Arcs::with_launch(env, strategies.into_iter().filter(|s| s.jump || s.edge).collect(), Launch::Weak)
    }

    fn with_launch(env: Env, strategies: Vec<Strategy>, launch: Launch) -> Arcs {
        let y0 = if launch == Launch::Ground { 0.0 } else { ON_HAN };
        let envelope =
            strategies.iter().map(|s| free_envelope(&env, &Flight::of(&env, s, launch, 0.0, y0))).collect();
        Arcs { env, strategies, launch, envelope }
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

    /// [`Arcs::may_land`] on one of the (sorted) columns `cols` of row `r`.
    fn may_land_on(&self, k: usize, from: Cell, x0: f32, r: i32, cols: &[i32]) -> bool {
        let dr = r - from.1;
        if !(DR_MIN..=DR_MAX).contains(&dr) {
            return true;
        }
        let Some((lo, hi)) = self.envelope[k][(dr - DR_MIN) as usize] else { return false };
        // The columns a box centered in [x0+lo, x0+hi] overlaps.
        let c0 = ((x0 + lo - HALF_W - 1.0) / TILE).floor() as i32;
        let c1 = ((x0 + hi + HALF_W + 1.0) / TILE).floor() as i32;
        let i = cols.partition_point(|&c| c < c0);
        cols.get(i).is_some_and(|&c| c <= c1)
    }

    /// Strategy `s` taking off from `cell` (on Han, for boosts), if it applies there.
    pub(crate) fn flight(&self, map: &Map, cell: Cell, s: &Strategy) -> Option<Flight> {
        let x0 = match self.launch {
            Launch::Weak => weak_take_off(map, &self.env, cell, s),
            _ => take_off(map, &self.env, cell, s)?,
        };
        let floor = (cell.1 + 1) as f32 * TILE;
        let y0 = floor - HALF_H + if self.launch == Launch::Ground { 0.0 } else { ON_HAN };
        Some(Flight::of(&self.env, s, self.launch, x0, y0))
    }
}

/// One simulated flight: where it starts and how it's steered (pixels, y down).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Flight {
    pub x0: f32,
    pub y0: f32,
    pub vx0: f32,
    pub vy0: f32,
    /// Direction key held until `release`.
    pub dir: f32,
    pub release: f32,
    /// Jump released at `hold` (rise cut by [`JUMP_CUT`]), unless `cut` already.
    pub hold: f32,
    pub cut: bool,
    /// Toot times (`INFINITY`: unused), and the toot's upward speed.
    pub toots: [f32; 3],
    pub toot_speed: f32,
}

impl Flight {
    pub(crate) fn of(env: &Env, s: &Strategy, launch: Launch, x0: f32, y0: f32) -> Flight {
        let vy0 = match (s.jump, launch) {
            (false, _) => 0.0,
            (true, Launch::Ground) => -JUMP_SPEED * env.boost,
            (true, Launch::Boost) => -BOOST_SPEED,
            (true, Launch::Weak) => -WEAK_BOOST_SPEED * env.boost,
        };
        Flight {
            x0,
            y0,
            vx0: s.vx0 * env.vx,
            vy0,
            dir: s.dir,
            release: s.release,
            hold: s.hold,
            cut: !s.jump,
            toots: [s.toot.unwrap_or(f32::INFINITY), f32::INFINITY, f32::INFINITY],
            toot_speed: env.toot,
        }
    }
}

/// The in-air controls of one step of `f` at time `t` (after the step): input, jump cut,
/// toots, gravity. Shared by the free-flight envelopes and [`fly`].
#[inline(always)]
fn air_controls(env: &Env, f: &Flight, t: f32, vx: &mut f32, vy: &mut f32, cut: &mut bool, tooted: &mut [bool; 3]) {
    let input = if t < f.release { f.dir } else { 0.0 };
    let target = input * env.vx;
    let dv = env.air_accel * DT;
    *vx = if (target - *vx).abs() <= dv { target } else { *vx + dv * (target - *vx).signum() };
    if !*cut && t >= f.hold {
        *cut = true;
        if *vy < 0.0 {
            *vy *= JUMP_CUT;
        }
    }
    for k in 0..3 {
        if !tooted[k] && t >= f.toots[k] {
            tooted[k] = true;
            *vy = -f.toot_speed;
        }
    }
    *vy = (*vy + env.gravity * DT).min(env.max_fall);
}

fn free_envelope(env: &Env, f: &Flight) -> [Option<(f32, f32)>; (DR_MAX - DR_MIN + 1) as usize] {
    let mut out = [None; (DR_MAX - DR_MIN + 1) as usize];
    let (mut x, mut y) = (f.x0, f.y0);
    let (mut vx, mut vy) = (f.vx0, f.vy0);
    let mut cut = f.cut;
    let mut tooted = [false; 3];
    let (mut lo, mut hi) = (0.0f32, 0.0f32);
    let mut t = 0.0;
    // (y, x extent so far) at every step.
    let mut steps: Vec<(f32, f32, f32)> = vec![(y, 0.0, 0.0)];
    while t < MAX_T {
        t += DT;
        air_controls(env, f, t, &mut vx, &mut vy, &mut cut, &mut tooted);
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
pub(crate) enum Outcome {
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

/// Where Nat takes off from Han's head (box center x) for a weak boost (or a step off his head)
/// with Han standing in `cell`: Han in its middle, or hanging off its ledge in `s.dir` (if it
/// has one, for edge strategies); Nat in the middle of his head, or (edge strategies) on its
/// very edge in `s.dir` (just off it, stepping off).
fn weak_take_off(map: &Map, env: &Env, (c, r): Cell, s: &Strategy) -> f32 {
    let mut x = c as f32 * TILE + 8.0;
    if s.edge {
        let n = (c + s.dir as i32, r);
        if !map.is_solid(n.0, n.1) && !map.standable(n) {
            x += s.dir * (8.0 + env.overhang - HALF_W);
        }
        x += s.dir * (PLAYER_SIZE.0 + if s.jump { -0.5 } else { 0.5 });
    }
    x
}

/// One arc from cell (c, r); the cells the box touched go into `touched`.
fn simulate(map: &Map, env: &Env, (_, r): Cell, s: &Strategy, x0: f32, touched: &mut Vec<Cell>) -> Outcome {
    let y0 = (r + 1) as f32 * TILE - HALF_H;
    fly(map, env, &Flight::of(env, s, Launch::Ground, x0, y0), touched)
}

/// Fly `f` until it lands, dies or times out; the cells the box touched go into `touched`.
pub(crate) fn fly(map: &Map, env: &Env, f: &Flight, touched: &mut Vec<Cell>) -> Outcome {
    fly_timed(map, env, f, touched).0
}

/// [`fly`], and how long (s) the flight took.
pub(crate) fn fly_timed(map: &Map, env: &Env, f: &Flight, touched: &mut Vec<Cell>) -> (Outcome, f32) {
    let o = fly_inner(map, env, f, touched);
    (o.0, o.1)
}

fn fly_inner(map: &Map, env: &Env, f: &Flight, touched: &mut Vec<Cell>) -> (Outcome, f32) {
    touched.clear();
    let (mut x, mut y) = (f.x0, f.y0);
    if map.deadly(x, y) || map.overlaps_solid(x, y) {
        return (Outcome::Nothing, 0.0);
    }
    let (mut vx, mut vy) = (f.vx0, f.vy0);
    let mut cut = f.cut;
    let mut tooted = [false; 3];
    let mut t = 0.0;
    while t < MAX_T {
        t += DT;
        air_controls(env, f, t, &mut vx, &mut vy, &mut cut, &mut tooted);

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
            return (Outcome::Died(death), t);
        }
        for col in cells(x - HALF_W, x + HALF_W) {
            for row in cells(y - HALF_H, y + HALF_H) {
                touched.push((col, row));
            }
        }
        if let Some(cell) = landed {
            return (if map.standable(cell) { Outcome::Land(cell) } else { Outcome::Nothing }, t);
        }
    }
    (Outcome::Nothing, t)
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
    // The player's center is in danger within this distance of a can's center (its jet, or
    // the can itself on the floor you walk on: both fire together).
    let reach = HALF_W - FORGIVE + SPRAY_WIDTH.max(CAN_WIDTH) / 2.0;
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

/// Problems with a shield row's timing: no mode may dash through its jets on their own clock
/// (ideal speed, the waltz's long breath included). Behind Han it's safe: each jet he walks
/// through stays plugged [`crate::game::HAN_PLUG_LINGER`] s after he's past it, longer than
/// Nat right behind him takes to clear it (`tests/han.rs`).
pub fn shield_row_timing(w: &WaltzRow) -> Vec<String> {
    let mut errs = Vec::new();
    for mode in [Mode::Normal, Mode::GiantSteps, Mode::FiredUp, Mode::Waltz] {
        if dash_through(w.cans(), Env::new(mode, true).vx, mode) {
            errs.push(format!(
                "shield row at col {}..={} row {} ({} cans) can be run through with {mode:?} physics: it doesn't need Han",
                w.c0,
                w.c1,
                w.row,
                w.cans()
            ));
        }
    }
    errs
}

/// Is there a ceiling at most 2 tiles above the walk all along the row (so nobody can jump
/// clear of the jets, or hop over the cans in the off-beats and hang above them while they
/// fire)?
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
    /// Plunger boosts off Han standing still: a human's (normal physics), and ideal ones in
    /// the modes that could carry them farthest.
    boost: Arcs,
    boost_ideal: Vec<(Mode, Arcs)>,
    /// Weak boosts off Han in a band zone, ideal, in the modes with their own geometry (the
    /// waltz and the nervous band jump like normal).
    weak_ideal: Vec<(Mode, Arcs)>,
}

impl Physics {
    pub fn new() -> Physics {
        let human = MODES.iter().map(|&m| Arcs::new(Env::new(m, false), strategies())).collect();
        let ideal = MODES.iter().map(|&m| Arcs::new(Env::new(m, true), ideal_strategies())).collect();
        let no_toot =
            Arcs::new(Env::new(Mode::Normal, false), strategies().into_iter().filter(|s| s.toot.is_none()).collect());
        let boost = Arcs::boost(
            Env::new(Mode::Normal, false),
            strategies().into_iter().filter(|s| s.vx0 == 0.0).collect(),
        );
        let boost_ideal = [Mode::Normal, Mode::GiantSteps, Mode::FiredUp]
            .into_iter()
            .map(|m| (m, Arcs::boost(Env::new(m, true), ideal_strategies())))
            .collect();
        let weak_ideal = [Mode::Normal, Mode::GiantSteps, Mode::FiredUp]
            .into_iter()
            .map(|m| (m, Arcs::weak(Env::new(m, true), ideal_strategies())))
            .collect();
        Physics { human, ideal, no_toot, boost, boost_ideal, weak_ideal }
    }

    /// Physics whose reachability tries only `human` jumps (the proofs keep every ideal one).
    /// Fewer jumps reach less, so whatever a level passes with them it passes with
    /// [`strategies`] too: free play (`crate::freeplay`) validates rooms with a lean set, fast
    /// enough to do while the game runs.
    pub fn with_human(human: Vec<Strategy>) -> Physics {
        let ideal = MODES.iter().map(|&m| Arcs::new(Env::new(m, true), ideal_strategies())).collect();
        let no_toot = Arcs::new(Env::new(Mode::Normal, false), human.iter().copied().filter(|s| s.toot.is_none()).collect());
        let human = MODES.iter().map(|&m| Arcs::new(Env::new(m, false), human.clone())).collect();
        let Physics { boost, boost_ideal, weak_ideal, .. } = Physics::new();
        Physics { human, ideal, no_toot, boost, boost_ideal, weak_ideal }
    }

    /// The same physics with the weak boost (Han in a band zone) launching at `speed` (px/s)
    /// instead of [`WEAK_BOOST_SPEED`]: for testing the boost proof.
    pub fn with_weak_boost(mut self, speed: f32) -> Physics {
        for (_, a) in &mut self.weak_ideal {
            *a = Arcs::weak(Env { boost: speed / WEAK_BOOST_SPEED, ..a.env }, a.strategies.clone());
        }
        self
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
    cross_dying(map, phys.human(Mode::Normal), g, &|c| g.reached(c), &|_| true, &|d| pit.contains(d), MAX_PIT_DEATHS)
}

/// The fewest splats in cells `pit` contains (spikes: stains; liquid: rafts) that let `arcs`
/// past it, at most `max_deaths` (beam search). Returns the splats, the take-off cell and the
/// first cell past it.
fn cross_dying(
    map: &Map,
    arcs: &Arcs,
    g: &Graph,
    known: &dyn Fn(Cell) -> bool,
    past: &dyn Fn(Cell) -> bool,
    pit: &dyn Fn(Cell) -> bool,
    max_deaths: u32,
) -> Option<(Vec<Cell>, Cell, Cell)> {
    struct P<'a>(&'a dyn Fn(Cell) -> bool);
    impl P<'_> {
        fn contains(&self, c: Cell) -> bool {
            (self.0)(c)
        }
    }
    let pit = P(pit);
    // Death spots in this pit reached so far, with their take-off cells.
    let start: Vec<(Cell, Cell)> = g
        .deaths
        .iter()
        .filter(|(d, _)| pit.contains(**d))
        .flat_map(|(d, (_, froms))| froms.iter().map(move |f| (*d, *f)))
        .filter(|&(_, f)| known(f))
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
    for _depth in 0..max_deaths {
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
                            && !known(l)
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
                    if !tops.contains(&cell) && past(cell) {
                        crossed = Some((cell, cell));
                        break;
                    }
                    for dd in [-1, 1] {
                        let n = (cell.0 + dd, cell.1);
                        if m.standable(n) && !known(n) && reached.insert(n) {
                            queue.push(n);
                        }
                    }
                    for s in &arcs.strategies {
                        let Some(x0) = take_off(&m, &arcs.env, cell, s) else { continue };
                        match simulate(&m, &arcs.env, cell, s, x0, &mut buf) {
                            Outcome::Land(l) if !known(l) && reached.insert(l) => queue.push(l),
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
    let mut buddy_tried: Vec<bool> = vec![false; (map.w * map.h) as usize];
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
                    derive_mark(map, Gate::StainPit, from, to);
                    crossings.push(Crossing { gate: Gate::StainPit, from, to, new, stains, flags });
                    crossed = true;
                    break;
                }
            }
            if !crossed {
                // Still nothing: Han's gates.
                let found = buddy_found(map, phys, &g, &mut buddy_tried);
                if found.is_empty() {
                    break;
                }
                take(map, phys, &mut g, &mut crossings, found);
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
        take(map, phys, &mut g, &mut crossings, found);
    }
    (g, crossings)
}

/// A crossing found: (gate, from, to, cells touched on the way).
type Found = (Gate, Cell, Cell, Vec<Cell>);

/// Add the crossings `found` (those still leading somewhere new) and explore past them.
fn take(map: &mut Map, phys: &Physics, g: &mut Graph, crossings: &mut Vec<Crossing>, found: Vec<Found>) {
    for (gate, from, to, touched) in found {
        if g.reached(to) {
            continue;
        }
        derive_mark(map, gate, from, to);
        let i = g.idx(from).expect("reached");
        g.edges[i].as_mut().expect("reached").push(to);
        for t in touched {
            g.touch(t);
        }
        let flags = map.flags.clone();
        let new = explore(map, phys.human(Mode::Normal), vec![to], Some(crossings.len()), g);
        crossings.push(Crossing { gate, from, to, new, stains: Vec::new(), flags });
    }
}

/// The mark a crossing from `from` to `to` would need.
fn mark_of(gate: Gate, from: Cell, to: Cell) -> GateMark {
    GateMark {
        topic: gate.topic(),
        c0: from.0.min(to.0) - 1,
        r0: from.1.min(to.1) - 1,
        c1: from.0.max(to.0) + 1,
        r1: from.1.max(to.1),
    }
}

/// In a generated room, a band or death gate is marked as soon as it's found.
fn derive_mark(map: &mut Map, gate: Gate, from: Cell, to: Cell) {
    if map.derive_marks && !gate.needs_han() {
        map.derived.push(mark_of(gate, from, to));
    }
}

/// Can Han be in `cell` with Nat, at full strength? Nat got there (Han navigates with his own
/// physics, and when he can't follow he parachutes in next to Nat, so wherever Nat stands, Han
/// can be), and neither Han nor Nat on his head is in a band zone (there it's the weak boost,
/// no good for Han's gates, and he doesn't go ahead).
fn han_at(map: &Map, g: &Graph, cell: Cell) -> bool {
    g.reached(cell) && map.han_allowed(cell) && map.han_allowed((cell.0, cell.1 - 1)) && !map.han_keeps_out(cell)
}

/// Han's gates from reached cells not tried yet: buddy ledges (a boost off Han standing
/// there), shield rows (Han goes ahead), buddy raft pools (Han's rafts), chain chasms.
fn buddy_found(map: &Map, phys: &Physics, g: &Graph, tried: &mut [bool]) -> Vec<Found> {
    let mut found = Vec::new();
    let mut buf = Vec::new();
    let mut cells: Vec<Cell> = g.order.iter().copied().filter(|&c| !tried[g.idx(c).unwrap()]).collect();
    cells.sort();
    for &cell in &cells {
        tried[g.idx(cell).unwrap()] = true;
        if !han_at(map, g, cell) {
            continue;
        }
        for s in &phys.boost.strategies {
            let Some(f) = phys.boost.flight(map, cell, s) else { continue };
            if let Outcome::Land(l) = fly(map, &phys.boost.env, &f, &mut buf)
                && !g.reached(l)
            {
                found.push((Gate::BuddyLedge, cell, l, buf.clone()));
            }
        }
        for w in &map.shield_rows {
            for dir in [-1, 1] {
                if let Some(l) = shield_walk(map, w, cell, dir)
                    && !g.reached(l)
                {
                    let touched = (w.c0..=w.c1).map(|c| (c, w.walk_row())).collect();
                    found.push((Gate::ShieldRow, cell, l, touched));
                }
            }
        }
        for p in &map.pools {
            for dir in [-1, 1] {
                if p.shores(dir).0 == cell
                    && let Some(l) = buddy_rafts(map, p, dir)
                    && !g.reached(l)
                {
                    let touched = (p.c0..=p.c1).map(|c| (c, p.row - 1)).collect();
                    found.push((Gate::BuddyRaft, cell, l, touched));
                }
            }
        }
        for ch in &map.chasms {
            for dir in [-1, 1] {
                let edge = if dir > 0 { (ch.c0 - 1, ch.row) } else { (ch.c1 + 1, ch.row) };
                if edge == cell
                    && let Some(l) = chain_cross(map.level, ch, dir)
                    && map.standable(l)
                    && !g.reached(l)
                {
                    let touched = (ch.c0..=ch.c1).map(|c| (c, ch.row)).collect();
                    found.push((Gate::ChainChasm, cell, l, touched));
                }
            }
        }
    }
    found
}

/// Han marching through shield row `w` from the approach cell `from` (heading `dir`), Nat right
/// behind him: the cell past the row, if the corridor is walkable and roofed.
fn shield_walk(map: &Map, w: &WaltzRow, from: Cell, dir: i32) -> Option<Cell> {
    let r = w.walk_row();
    let (start, end) = if dir > 0 { (w.c0 - 1, w.c1 + 1) } else { (w.c1 + 1, w.c0 - 1) };
    if from != (start, r) || !low_ceiling(map, w) {
        return None;
    }
    let walkable = (w.c0..=w.c1)
        .all(|c| map.floor(c, r + 1) != Floor::None && !map.is_solid(c, r) && !map.deadly_tile(c, r));
    (walkable && map.standable((end, r))).then_some((end, r))
}

/// Han's raft bridge over pool `p` heading `dir`: he wades in off the end of the shore (or of
/// his last raft), splats, and leaves a 3-tile raft there (see `game::han`); Nat steps on, Han
/// parachutes back and goes again. His rafts tile the pool, so the far shore is a walk if Nat
/// can stand on every one of them (under the ceiling spikes). The time it takes is checked by
/// `tests/han.rs` against the raft's life ([`crate::game::HAN_RAFT_LIFE_FLOOR`]).
fn buddy_rafts(map: &Map, p: &Pool, dir: i32) -> Option<Cell> {
    let mut m = map.clone();
    for c in p.c0..=p.c1 {
        m.set((c, p.row), RAFT, 0);
    }
    let (from, to) = p.shores(dir);
    (from.0.min(to.0)..=from.0.max(to.0)).all(|c| m.standable((c, p.row - 1))).then_some(to)
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
                let Some(f) = arcs.flight(map, from, s) else { continue };
                if !rows.iter().any(|(&r, &(c0, c1))| arcs.may_land(k, from, f.x0, r, c0, c1)) {
                    continue;
                }
                if let Outcome::Land(l) = fly(map, &arcs.env, &f, &mut buf) {
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
        // try again with normal physics, from the stains too. (A buddy raft pool's own rafts
        // are timed instead, below: permanent ones would bridge it by design.)
        let mut stained = before.clone();
        let mut tops = HashSet::new();
        for (&d, (o, _)) in &g.deaths {
            let pool = c.gate == Gate::BuddyRaft && before.f(d.0, d.1) & LIQUID != 0;
            if !pool && o.is_none_or(|o| o < i) && (d.0 - c.from.0).abs() <= 16 && (d.1 - c.from.1).abs() <= 10 {
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
    if !c.gate.needs_han()
        && let Some(e) = boost_leak(&before, phys, g, i, c, &beyond)
    {
        errs.push(format!("{what} {e}"));
    }
    match c.gate {
        Gate::ChainChasm => {
            // A chain, not one boost: no single boost off Han standing anywhere near crosses.
            let cands: Vec<Cell> = near.iter().copied().filter(|&n| before.han_allowed(n)).collect();
            for (mode, arcs) in phys.boost_ideal.iter().filter(|(m, _)| *m != Mode::FiredUp) {
                if let Some((f, to)) = leak(&before, arcs, &cands, &beyond, &HashSet::new()) {
                    errs.push(format!(
                        "{what} is crossable with one plunger boost ({mode:?}) from col {} row {} -> col {} row {}: not a chain",
                        f.0, f.1, to.0, to.1
                    ));
                }
            }
        }
        Gate::BuddyRaft => errs.extend(nat_rafts(&before, phys, g, crossings, i)),
        _ => {}
    }
    errs
}

/// In mid-air Han's head holds Nat up to [`HAN_CATCH_RISE`] above the floor he last stood on:
/// that many rows, plus one (his feet anywhere in a cell); [`HAN_CHASM_CATCH_RISE`] over a
/// chain-jump chasm...
const CATCH_ROWS: i32 = (HAN_CATCH_RISE / TILE) as i32 + 1;
const CHASM_CATCH_ROWS: i32 = (HAN_CHASM_CATCH_RISE / TILE) as i32 + 1;
/// ...as far as this (columns) from it (at that height he runs and toots a long way)...
const CATCH_COLS: i32 = 16;
/// ...and down to this many rows below it (off a ledge, down a pit).
const CATCH_DROP: i32 = 4;
/// Where Han may be in mid-air, holding Nat, after leaving the floor of `cell`: the open cells
/// (flood-filled, so not through walls) up to [`CATCH_ROWS`] above it (over a chasm,
/// [`CHASM_CATCH_ROWS`]), [`CATCH_DROP`] below and [`CATCH_COLS`] to either side.
fn han_air(map: &Map, h: Cell) -> Vec<Cell> {
    let up = |x: i32| if map.level.in_chasm(x) { CHASM_CATCH_ROWS } else { CATCH_ROWS };
    let inside = |(x, y): Cell| (x - h.0).abs() <= CATCH_COLS && (h.1 - up(x)..=h.1 + CATCH_DROP).contains(&y);
    let mut seen: HashSet<Cell> = HashSet::from([h]);
    let mut todo = vec![h];
    while let Some((x, y)) = todo.pop() {
        for n in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
            if inside(n) && !map.is_solid(n.0, n.1) && !map.deadly_tile(n.0, n.1) && seen.insert(n) {
                todo.push(n);
            }
        }
    }
    seen.into_iter().collect()
}

/// How far (columns, rows) around a gate's take-off the boost proof looks for Nat and Han.
pub const BOOST_COLS: i32 = HAN_BERTH + 16;
pub const BOOST_ROWS: i32 = 12;

/// The boost proof for band (or death) gate crossing `c` (number `i`): no plunger boost, and no
/// chain of boosts and toots, opens it, in any mode, with ideal input. Returns what does.
///
/// Han follows Nat everywhere, so (over-approximating where a chain can put them):
/// - **Full boosts**, outside the band zones (Han's cell and Nat's on his head): Han standing,
///   or in mid-air up to [`HAN_CATCH_RISE`] (over a chasm [`HAN_CHASM_CATCH_RISE`]) above a
///   floor he may have stood on, anywhere [`CATCH_COLS`] from it, through open air ([`han_air`]):
///   a cell Nat reached before the gate, one a chain got Nat to, or one nobody reached in a
///   zone this side of the gate whose air Nat gets into on his own. Whatever came before, a
///   chain of full boosts only ever launches from one of these.
/// - **Weak boosts**, in the zones: his head holds Nat there only while he stands, so from the
///   cells Nat stands in: reached before the gate, and (the chains) every unreached cell any
///   of these arcs lands on (or passes through, in a zone), iterated. Each weak boost
///   refreshes the toot, so every one is tried with every toot timing, and stepping off his
///   head too.
/// - From cells first got to that way: Nat's own ideal jumps, and walking on.
///
/// Every arc landing past the gate is a leak. (The gate's own mode is left out for weak boosts
/// and jumps: there the band is playing.)
fn boost_leak(map: &Map, phys: &Physics, g: &Graph, i: usize, c: &Crossing, beyond: &HashSet<Cell>) -> Option<String> {
    let at = c.from;
    let window = |(x, y): Cell| (x - at.0).abs() <= BOOST_COLS && (y - at.1).abs() <= BOOST_ROWS;
    let reached: Vec<Cell> = g.order.iter().copied().filter(|&cell| g.before(cell, i) && window(cell)).collect();
    let mut unreached: HashSet<Cell> = HashSet::new();
    for y in at.1 - BOOST_ROWS..=at.1 + BOOST_ROWS {
        for x in at.0 - BOOST_COLS..=at.0 + BOOST_COLS {
            if map.standable((x, y)) && !g.reached((x, y)) {
                unreached.insert((x, y));
            }
        }
    }
    // This side of the gate (an unreached cell past where it lands isn't a floor Han jumps from
    // to catch a Nat still on the near side).
    let dir = (c.to.0 - at.0).signum();
    let near_side = |(x, _): Cell| dir == 0 || (x - c.to.0) * dir < 0;
    let zone = |(x, y): Cell| map.in_band_zone((x, y)) || map.in_band_zone((x, y - 1));
    let weak_at = |cell: Cell| zone(cell) && map.standable(cell) && !map.han_keeps_out(cell);
    let full_at = |cell: Cell| {
        !zone(cell) && !map.is_solid(cell.0, cell.1) && !map.deadly_tile(cell.0, cell.1) && !map.han_keeps_out(cell)
    };
    // Landing targets (per row, the columns), for the envelopes.
    let mut rows: HashMap<i32, Vec<i32>> = HashMap::new();
    for &(x, y) in beyond.iter().chain(&unreached) {
        rows.entry(y).or_default().push(x);
    }
    let rows: Vec<(i32, Vec<i32>)> = rows
        .into_iter()
        .map(|(r, mut cols)| {
            cols.sort_unstable();
            cols.dedup();
            (r, cols)
        })
        .collect();
    let mut buf = Vec::new();
    // One launch: a landing past the gate, or (into `new`) the unreached cells it gets Nat to.
    let mut fly_from = |arcs: &Arcs, from: Cell, new: &mut Vec<Cell>| -> Option<Cell> {
        for (k, s) in arcs.strategies.iter().enumerate() {
            let Some(f) = arcs.flight(map, from, s) else { continue };
            if !rows.iter().any(|(r, cols)| arcs.may_land_on(k, from, f.x0, *r, cols)) {
                continue;
            }
            if let Outcome::Land(l) = fly(map, &arcs.env, &f, &mut buf) {
                if beyond.contains(&l) {
                    return Some(l);
                }
                if unreached.contains(&l) {
                    new.push(l);
                }
            }
            // Passing through a zone cell: Han may be standing there to catch him.
            new.extend(buf.iter().copied().filter(|&t| unreached.contains(&t) && weak_at(t)));
        }
        None
    };
    let name = |m: Mode| if m == Mode::Normal { "normal, waltz or nervous".to_string() } else { format!("{m:?}") };
    let own = |m: Mode| Some(m) == c.gate.mode();
    // Han's floors to start with, and the air around each where he may catch Nat: where Nat
    // stands before the gate, and the unreached ledges in a zone this side of it whose air Nat
    // gets into on his own (a jump above where he stands).
    let mut nat_air: HashSet<Cell> = HashSet::new();
    for &(x, y) in &reached {
        for k in 0..=5 {
            if map.is_solid(x, y - k) {
                break;
            }
            nat_air.insert((x, y - k));
        }
    }
    let mut first_floors: Vec<(Cell, Vec<Cell>)> = reached.iter().map(|&h| (h, han_air(map, h))).collect();
    for &u in &unreached {
        if near_side(u) && map.in_band_zone(u) {
            let air = han_air(map, u);
            if air.iter().any(|a| nat_air.contains(a)) {
                first_floors.push((u, air));
            }
        }
    }
    for (mode, full) in &phys.boost_ideal {
        let mode = *mode;
        let weak = phys.weak_ideal.iter().find(|(m, _)| *m == mode).map(|(_, a)| a).filter(|_| !own(mode));
        let alike = if mode == Mode::Normal { vec![Mode::Normal, Mode::Waltz, Mode::Nervous] } else { vec![mode] };
        let ground: Vec<&Arcs> = alike
            .into_iter()
            .filter(|&m| !own(m) && (m != Mode::Nervous || map.has_grease))
            .map(|m| phys.ideal(m))
            .collect();
        // Nat's cells: reached before, then whatever the boosts get him to.
        let mut nat: HashSet<Cell> = reached.iter().copied().collect();
        let mut queue: Vec<(Cell, bool)> = reached.iter().map(|&r| (r, false)).collect();
        // Han's floors (for full boosts in mid-air), and the cells full boosts were tried from.
        let mut floors: Vec<(Cell, Vec<Cell>)> = first_floors.clone();
        let mut full_tried: HashSet<Cell> = HashSet::new();
        let mut new = Vec::new();
        loop {
            // Full boosts from every cell Han may be in, around every floor so far.
            for (h, air) in std::mem::take(&mut floors) {
                for a in air {
                    if !full_at(a) || near_side(a) != near_side(h) || !full_tried.insert(a) {
                        continue;
                    }
                    if let Some(l) = fly_from(full, a, &mut new) {
                        return Some(format!(
                            "is passable with a full plunger boost ({}) off Han at col {} row {} (from his floor at col {} row {}) -> col {} row {}: mark it (`gate:`) so his boost is weak there",
                            name(mode), a.0, a.1, h.0, h.1, l.0, l.1
                        ));
                    }
                }
            }
            for l in new.drain(..) {
                if nat.insert(l) {
                    queue.push((l, true));
                }
            }
            let Some((cell, fresh)) = queue.pop() else { break };
            if let Some(weak) = weak
                && weak_at(cell)
                && let Some(l) = fly_from(weak, cell, &mut new)
            {
                let how = if fresh { " (got to by a chain of boosts)" } else { "" };
                return Some(format!(
                    "is passable with a weak plunger boost ({}) off Han at col {} row {}{how} -> col {} row {}: his band-zone boost must not open it",
                    name(mode), cell.0, cell.1, l.0, l.1
                ));
            }
            if fresh {
                // Somewhere new: Nat's own jumps from there, walking on, and a floor for Han.
                floors.push((cell, han_air(map, cell)));
                for arcs in &ground {
                    if let Some(l) = fly_from(arcs, cell, &mut new) {
                        return Some(format!(
                            "is passable with a chain of plunger boosts ({}) off Han to col {} row {}, then a jump -> col {} row {}",
                            name(mode), cell.0, cell.1, l.0, l.1
                        ));
                    }
                }
                for d in [-1, 1] {
                    let n = (cell.0 + d, cell.1);
                    if beyond.contains(&n) {
                        return Some(format!(
                            "is passable with a chain of plunger boosts ({}) off Han to col {} row {}, then a walk",
                            name(mode), cell.0, cell.1
                        ));
                    }
                    if unreached.contains(&n) {
                        new.push(n);
                    }
                }
            }
        }
    }
    None
}

/// A buddy raft pool must defeat Nat's own rafts: the fewest of them that bridge it (ideal
/// play), each needing a respawn and a walk back from the nearest respawn point before it, must
/// take longer than a raft floats ([`RAFT_LIFE_FLOOR`]: every raft has to still be there for
/// the last walk across).
fn nat_rafts(map: &Map, phys: &Physics, g: &Graph, crossings: &[Crossing], i: usize) -> Vec<String> {
    let c = &crossings[i];
    let Some(pool) = map.pools.iter().find(|p| p.shores(1).0 == c.from || p.shores(-1).0 == c.from) else {
        return vec![format!("buddy raft pool from col {} row {}: no pool there", c.from.0, c.from.1)];
    };
    let start = (map.level.start.0 as i32, map.level.start.1 as i32);
    let respawns: Vec<Cell> = std::iter::once(start)
        .chain(map.level.checkpoints().map(|t| (t.col as i32, t.row as i32)))
        .filter(|&r| g.before(r, i))
        .collect();
    let walk = respawns.iter().filter_map(|&r| distances(g, r).get(&c.from).copied()).min();
    let Some(walk) = walk else { return Vec::new() };
    let mut errs = Vec::new();
    for mode in [Mode::Normal, Mode::FiredUp] {
        // Short hops under the ceiling spikes: the human jumps have them (the ideal set is
        // full-height jumps only); the walk back is timed at full speed.
        let arcs = phys.human(mode);
        let trip = crate::game::tuning::RESPAWN_DELAY + walk as f32 * TILE / Env::new(mode, true).vx;
        let contains = |d: Cell| pool.contains(d);
        let known = |cell: Cell| g.before(cell, i);
        let past = |cell: Cell| c.new.contains(&cell);
        if let Some((rafts, _, _)) = cross_dying(map, arcs, g, &known, &past, &contains, MAX_NAT_RAFTS) {
            let k = rafts.len() as f32;
            if k * trip < RAFT_LIFE_FLOOR {
                errs.push(format!(
                    "buddy raft pool at col {}..={} row {}: Nat's own rafts bridge it ({mode:?}): {} rafts x {trip:.1}s round trip < {RAFT_LIFE_FLOOR}s",
                    pool.c0, pool.c1, pool.row, rafts.len()
                ));
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
    /// The level's gate marks; for a generated room ([`check_room`]), the marks it needs (its
    /// gates, found): copy them into the room so the game's Han is feeble around its band gates.
    pub marks: Vec<GateMark>,
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
    map.derive_marks = !check_deaths;
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
                "waltz row at col {}..={} row {} needs a low ceiling (solid at most 2 tiles above the walk) all along",
                w.c0, w.c1, w.row
            ));
        }
    }
    for w in &map.shield_rows {
        errs.extend(shield_row_timing(w));
        if !low_ceiling(&map, w) {
            errs.push(format!(
                "shield row at col {}..={} row {} needs a low ceiling (solid at most 2 tiles above the walk) all along",
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

    // Gate marks: every crossing in a mark of its kind, every mark holding one. (A generated
    // room's marks are derived instead.)
    let mut marks = level.gates.clone();
    if map.derive_marks {
        marks.extend(map.derived.iter().copied());
        marks.extend(crossings.iter().filter(|c| c.gate.needs_han()).map(|c| mark_of(c.gate, c.from, c.to)));
    }
    for c in crossings.iter().filter(|_| !map.derive_marks) {
        let t = c.gate.topic();
        if !level.gates.iter().any(|m| m.topic == t && m.contains(c.from) && m.contains(c.to)) {
            errs.push(format!(
                "{} from col {} row {} to col {} row {} isn't in a `gate: {} <c0>,<r0> <c1>,<r1>` mark",
                c.gate.name(),
                c.from.0,
                c.from.1,
                c.to.0,
                c.to.1,
                t.word()
            ));
        }
    }
    for m in level.gates.iter().filter(|_| !map.derive_marks) {
        if !crossings.iter().any(|c| c.gate.topic() == m.topic && m.contains(c.from)) {
            errs.push(format!(
                "`gate: {} {},{} {},{}` holds no {} crossing",
                m.topic.word(),
                m.c0,
                m.r0,
                m.c1,
                m.r1,
                m.topic.word()
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
    Report { errs, gates, gated_goal: !to_goal.is_empty(), deaths, lessons, segments, dump, marks }
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
