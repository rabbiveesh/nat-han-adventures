//! Level validator: every level parses, matches the campaign plan, and is beatable.
//!
//! The heart of it is a conservative reachability search: from each "standable" cell we simulate
//! a fan of jumps (hold lengths, running/standing starts, toot double-jump timings, air control)
//! with the real tuning constants and tile AABB collision, and follow every arc that lands without
//! touching anything deadly. Moving platforms are approximated as one-way tiles along their whole
//! path (as if you can ride them anywhere they go). Flies are treated as static deadly squares
//! covering their whole circle; spray jets are treated as passable (they're timed).
//!
//! The music bends the physics (`Groove`), and levels have gates that need a mode:
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
//!   off) at a human 90% of top speed.
//!
//! Reachability starts with normal physics (with a human margin); from every reached cell that
//! has a runway / nugget line it also tries that mode's jumps ("crossings"), and carries on with
//! normal physics on the far side. Every crossing must be impossible in the other modes, even
//! with ideal input (full speed, any toot timing, hanging off the very edge).
//!
//! `LEVEL_DUMP=1 cargo test --test levels -- --nocapture` prints every level with the reachable
//! cells marked (add `LEVEL_ONLY=3` for just level 3):
//! `+` standable & reachable, `,` passed through by some safe arc, `X` unreachable nugget,
//! `!` unreachable checkpoint/goal, `@` moving platform (start), `-` its path,
//! `W` take-off of a Giant Steps crossing, `R` take-off of a fired-up crossing, `Z` the start of
//! a waltz row's dash.

use std::collections::{BinaryHeap, HashMap, HashSet};

use nat_han_adventures::audio::{Harmony, waltz::WALTZ_BPM};
use nat_han_adventures::game::{BeatClock, FORGIVE, Groove, SPRAY_WIDTH, WALTZ_ONE_BOOST, spray_on, tuning::*};
use nat_han_adventures::level::*;

/// Name, world and the gates the level must use.
const PLAN: [(&str, u8, &[Mode]); LEVEL_COUNT] = [
    ("Bathroom Floor", 1, &[Mode::GiantSteps]),
    ("The Bowl", 1, &[]),
    ("U-Bend", 2, &[Mode::FiredUp]),
    ("Pipe Maze", 2, &[Mode::GiantSteps]),
    ("Main Sewer", 3, &[Mode::FiredUp]),
    ("Rat Kingdom", 3, &[Mode::GiantSteps]),
    ("Septic Tank", 4, &[Mode::Waltz]),
    ("Porta-Potty Festival", 4, &[Mode::GiantSteps, Mode::Waltz]),
    ("Treatment Plant", 5, &[Mode::FiredUp, Mode::Waltz]),
    ("The Golden Throne", 5, &[Mode::GiantSteps]),
];
/// Levels whose goal can only be reached through their gate (the tutorials of each mechanic).
const GATED_GOAL: [usize; 2] = [0, 2];

const MAX_LINE: usize = 60;
const NUGGETS: std::ops::RangeInclusive<usize> = 15..=40;
/// Path cost (tiles moved, Manhattan per hop) allowed between respawn points: about 30s of play.
const MAX_SEGMENT: u32 = 150;
/// Fly swarms circle ~1 tile around their cell; treat the whole circle (plus a bit) as deadly.
const FLY_REACH: f32 = 20.0;
/// Flat, hazard-free tiles to toot 5 times on before a giant wall.
const RUNWAY: usize = 8;
/// Flat tiles of run-up behind a long gap's take-off, and how far back (tiles) the nugget line
/// that fires up the band may start.
const RUN_UP: usize = 4;
const NUGGET_LINE_REACH: i32 = 30;
/// A detour behind a gate must hold at least this many nuggets (if the goal isn't behind it).
const DETOUR_NUGGETS: usize = 3;
/// Rows above a runway tile that must be free of hazards (a toot goes ~5 tiles up).
const RUNWAY_HEADROOM: i32 = 5;
/// Flat, hazard-free tiles to jump in threes on before a waltz row.
const WALTZ_RUNWAY: usize = 6;
/// Adjacent spray cans that make a waltz row.
const WALTZ_ROW_MIN: usize = 4;

const HALF_W: f32 = PLAYER_SIZE.0 / 2.0;
const HALF_H: f32 = PLAYER_SIZE.1 / 2.0;
const DT: f32 = 1.0 / 120.0;
const MAX_T: f32 = 3.0;

type Cell = (i32, i32);

/// The physics modes the music can put the player in.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
enum Mode {
    Normal,
    GiantSteps,
    FiredUp,
    Waltz,
}

const MODES: [Mode; 4] = [Mode::Normal, Mode::GiantSteps, Mode::FiredUp, Mode::Waltz];

impl Mode {
    fn groove(self) -> Groove {
        Groove::of(match self {
            Mode::Normal => Harmony::Original,
            Mode::GiantSteps => Harmony::Coltrane,
            Mode::FiredUp => Harmony::Quartal,
            Mode::Waltz => Harmony::Waltz,
        })
    }

    fn gate(self) -> &'static str {
        match self {
            Mode::Normal => "jump",
            Mode::GiantSteps => "giant wall",
            Mode::FiredUp => "long gap",
            Mode::Waltz => "waltz row",
        }
    }

    /// Is the spray jet firing at time `t` (s since the music / level clock started)?
    fn jets_on(self, t: f32) -> bool {
        let g = self.groove();
        if g.waltz() {
            let beat = 60.0 / WALTZ_BPM as f64;
            g.at(BeatClock::at(t as f64 / beat, beat, 3)).waltz_spray_on()
        } else {
            spray_on(0, t)
        }
    }

    /// How long the jets' pattern takes to repeat.
    fn jets_period(self) -> f32 {
        if self == Mode::Waltz { 2.0 * 3.0 * 60.0 / WALTZ_BPM } else { nat_han_adventures::game::SPRAY_CYCLE }
    }
}

/// The physics of a mode, as the validator simulates them.
#[derive(Clone, Copy, Debug)]
struct Env {
    gravity: f32,
    max_fall: f32,
    air_accel: f32,
    /// Running speed the simulated player uses.
    vx: f32,
    /// How far (px) the 12px box dares to hang over a ledge before taking off.
    overhang: f32,
    /// Ground jump speed multiplier of a jump without a toot (the waltz's jump on ONE, which
    /// spends the toot; the waltz's off-beat jumps are [`Mode::Normal`]'s).
    boost: f32,
}

impl Env {
    /// `ideal`: full top speed and hanging off the very edge; otherwise a human margin (90% of
    /// top speed, 8px of overhang).
    fn new(mode: Mode, ideal: bool) -> Env {
        let g = mode.groove();
        Env {
            gravity: GRAVITY * g.gravity_scale,
            max_fall: MAX_FALL * g.fall_scale(),
            air_accel: AIR_ACCEL * g.speed_scale,
            vx: RUN_SPEED * g.speed_scale * if ideal { 1.0 } else { 0.9 },
            overhang: if ideal { PLAYER_SIZE.0 - 0.5 } else { 8.0 },
            boost: if mode == Mode::Waltz { WALTZ_ONE_BOOST } else { 1.0 },
        }
    }
}

/// Collision-relevant view of a level (pixel space here is x right, y DOWN, origin top-left).
struct Map<'a> {
    level: &'a Level,
    w: i32,
    h: i32,
    /// Cells occupied by some moving platform at some point of its path.
    virt: HashSet<Cell>,
    flies: Vec<(f32, f32)>,
    sprays: HashSet<Cell>,
    /// Runs of adjacent spray cans (waltz rows), and their jet cells: deadly to arcs (passing
    /// them is [`dash_through`]'s business).
    waltz_rows: Vec<WaltzRow>,
    jets: HashSet<Cell>,
}

/// A run of adjacent spray cans `c0..=c1` sitting in row `row`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct WaltzRow {
    row: i32,
    c0: i32,
    c1: i32,
}

impl WaltzRow {
    fn cans(&self) -> usize {
        (self.c1 - self.c0 + 1) as usize
    }

    /// The row the player stands in on the grating above the cans.
    fn walk_row(&self) -> i32 {
        self.row - 2
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Floor {
    None,
    OneWay,
    Solid,
}

impl<'a> Map<'a> {
    fn new(level: &'a Level) -> Self {
        let mut virt = HashSet::new();
        for p in &level.platforms {
            let steps = ((p.dx.abs() + p.dy.abs()) * 2.0).ceil().max(1.0) as i32;
            for s in 0..=steps {
                let f = s as f32 / steps as f32;
                let c = p.col as i32 + (p.dx * f).round() as i32;
                let r = p.row as i32 - (p.dy * f).round() as i32;
                for k in 0..p.width as i32 {
                    virt.insert((c + k, r));
                }
            }
        }
        let mut flies = Vec::new();
        let mut sprays = HashSet::new();
        let mut cans: Vec<Cell> = Vec::new();
        for t in &level.things {
            match t.kind {
                ThingKind::Fly => flies.push((t.col as f32 * TILE + 8.0, t.row as f32 * TILE + 8.0)),
                ThingKind::Spray => {
                    for k in 0..=3 {
                        sprays.insert((t.col as i32, t.row as i32 - k));
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
        let jets = waltz_rows
            .iter()
            .flat_map(|w| (w.c0..=w.c1).flat_map(move |c| (1..=3).map(move |k| (c, w.row - k))))
            .collect();
        Map { level, w: level.width as i32, h: level.height as i32, virt, flies, sprays, waltz_rows, jets }
    }

    fn tile(&self, c: i32, r: i32) -> Tile {
        self.level.tile(c, r)
    }

    fn floor(&self, c: i32, r: i32) -> Floor {
        match self.tile(c, r) {
            Tile::Solid => Floor::Solid,
            Tile::OneWay => Floor::OneWay,
            _ if self.virt.contains(&(c, r)) => Floor::OneWay,
            _ => Floor::None,
        }
    }

    fn in_fly_zone(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> bool {
        self.flies.iter().any(|&(fx, fy)| {
            x1 > fx - FLY_REACH && x0 < fx + FLY_REACH && y1 > fy - FLY_REACH && y0 < fy + FLY_REACH
        })
    }

    /// Does the player box (center x,y) touch anything deadly (or fall out the bottom)?
    fn deadly(&self, x: f32, y: f32) -> bool {
        let (x0, x1, y0, y1) = (x - HALF_W, x + HALF_W, y - HALF_H, y + HALF_H);
        if y0 > self.h as f32 * TILE {
            return true;
        }
        for c in cells(x0, x1) {
            for r in cells(y0, y1) {
                let (tx, ty) = (c as f32 * TILE, r as f32 * TILE);
                let hit = match self.tile(c, r) {
                    Tile::Liquid => true,
                    Tile::SpikesUp => y1 > ty + TILE / 2.0,
                    Tile::SpikesDown => y0 < ty + TILE / 2.0,
                    _ => false,
                };
                let hit = hit || self.jets.contains(&(c, r));
                if hit && x1 > tx && x0 < tx + TILE {
                    return true;
                }
            }
        }
        self.in_fly_zone(x0, y0, x1, y1)
    }

    fn standable(&self, (c, r): Cell) -> bool {
        if c < 0 || c >= self.w || r < 0 || r + 1 >= self.h {
            return false;
        }
        let t = self.tile(c, r);
        t != Tile::Solid
            && !t.is_deadly()
            && self.floor(c, r + 1) != Floor::None
            && !self.deadly(c as f32 * TILE + 8.0, (r + 1) as f32 * TILE - HALF_H)
    }

    /// Standable on real ground (not a moving platform's path), with nothing deadly in the
    /// [`RUNWAY_HEADROOM`] rows above: somewhere to hop up and down in peace.
    fn calm(&self, (c, r): Cell) -> bool {
        self.standable((c, r))
            && matches!(self.tile(c, r + 1), Tile::Solid | Tile::OneWay)
            && (0..=RUNWAY_HEADROOM).all(|k| {
                let cell = (c, r - k);
                !self.tile(cell.0, cell.1).is_deadly() && !self.sprays.contains(&cell)
            })
            && !self.in_fly_zone(
                c as f32 * TILE,
                (r - RUNWAY_HEADROOM) as f32 * TILE,
                (c + 1) as f32 * TILE,
                (r + 1) as f32 * TILE,
            )
    }

    /// The flat, calm stretch of floor `cell` is on (empty if `cell` itself isn't calm).
    fn flat_run(&self, (c, r): Cell) -> Vec<Cell> {
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
fn cells(a: f32, b: f32) -> std::ops::RangeInclusive<i32> {
    ((a / TILE).floor() as i32)..=(((b - 0.001) / TILE).floor() as i32)
}

#[derive(Clone, Copy)]
struct Strategy {
    dir: f32,
    jump: bool,
    /// Release jump after this long (variable jump height).
    hold: f32,
    /// Starting speed, as a fraction of the mode's running speed (signed).
    vx0: f32,
    /// Toot double jump at this time.
    toot: Option<f32>,
    /// Start hanging over the edge of the cell in `dir`.
    edge: bool,
    /// Let go of the direction key after this long.
    release: f32,
}

/// The jumps a human might try.
fn strategies() -> Vec<Strategy> {
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
fn ideal_strategies() -> Vec<Strategy> {
    let toots: Vec<Option<f32>> =
        std::iter::once(None).chain((3..=80).map(|f| Some(f as f32 / 60.0))).collect();
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

/// Result of one simulated arc: where it lands (if safely) and which cells the box touched.
fn simulate(map: &Map, env: &Env, (c, r): Cell, s: &Strategy, touched: &mut Vec<Cell>) -> Option<Cell> {
    touched.clear();
    let mut x = c as f32 * TILE + 8.0;
    if s.edge {
        let next = map.tile(c + s.dir as i32, r);
        if next == Tile::Solid || map.standable((c + s.dir as i32, r)) {
            return None; // not a ledge
        }
        x += s.dir * (8.0 + env.overhang - HALF_W);
    }
    let mut y = (r + 1) as f32 * TILE - HALF_H;
    if map.deadly(x, y) {
        return None;
    }
    let mut vx = s.vx0 * env.vx;
    let boost = if s.toot.is_none() { env.boost } else { 1.0 };
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
            vy = -DOUBLE_JUMP_SPEED;
        }
        vy = (vy + env.gravity * DT).min(env.max_fall);

        // Horizontal move against solids.
        let nx = x + vx * DT;
        let (y0, y1) = (y - HALF_H, y + HALF_H);
        let mut blocked = false;
        for col in cells(nx - HALF_W, nx + HALF_W) {
            for row in cells(y0, y1) {
                if map.tile(col, row) == Tile::Solid {
                    blocked = true;
                    x = if vx > 0.0 {
                        col as f32 * TILE - HALF_W
                    } else {
                        (col + 1) as f32 * TILE + HALF_W
                    };
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
                let cols: Vec<i32> = cells(x - HALF_W, x + HALF_W).collect();
                let center = (x / TILE).floor() as i32;
                let mut hit = None;
                for &col in &cols {
                    if map.floor(col, row) != Floor::None
                        && (hit.is_none() || col == center)
                    {
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
                    if map.tile(col, row) == Tile::Solid {
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

        if map.deadly(x, y) {
            return None;
        }
        for col in cells(x - HALF_W, x + HALF_W) {
            for row in cells(y - HALF_H, y + HALF_H) {
                touched.push((col, row));
            }
        }
        if let Some(cell) = landed {
            return map.standable(cell).then_some(cell);
        }
    }
    None
}

#[derive(Default)]
struct Graph {
    /// Outgoing edges per reachable standable cell.
    edges: HashMap<Cell, Vec<Cell>>,
    /// Every cell some safe arc (or standing) touches.
    touched: HashSet<Cell>,
    /// Which crossing first made each reachable cell reachable (`None`: normal physics from
    /// the start).
    origin: HashMap<Cell, Option<usize>>,
}

/// Follow every normal-physics arc from `seeds`, adding to `g`; returns the cells added.
fn explore(map: &Map, seeds: Vec<Cell>, origin: Option<usize>, g: &mut Graph) -> Vec<Cell> {
    let env = Env::new(Mode::Normal, false);
    let strategies = strategies();
    let mut added = Vec::new();
    let mut queue = seeds;
    let mut buf = Vec::new();
    while let Some(cell) = queue.pop() {
        if g.edges.contains_key(&cell) {
            continue;
        }
        g.touched.insert(cell);
        let mut out: HashSet<Cell> = HashSet::new();
        for d in [-1, 1] {
            let n = (cell.0 + d, cell.1);
            if map.standable(n) {
                out.insert(n);
            }
        }
        for s in &strategies {
            if let Some(land) = simulate(map, &env, cell, s, &mut buf) {
                out.insert(land);
                g.touched.extend(buf.iter().copied());
            }
        }
        out.remove(&cell);
        let out: Vec<Cell> = out.into_iter().collect();
        queue.extend(out.iter().copied().filter(|c| !g.edges.contains_key(c)));
        g.edges.insert(cell, out);
        g.origin.insert(cell, origin);
        added.push(cell);
    }
    added
}

/// Can a player running at `vx` px/s get past `n` adjacent spray jets that fire when
/// `mode.jets_on(t)`? Simulated like the game: 60 Hz steps, the jets updated before the player
/// moves, the hit test after, with the game's forgiving hitbox. The player waits just outside
/// the jets for the best moment (every start phase is tried) and enters at full speed.
fn dash_through(n: usize, vx: f32, mode: Mode) -> bool {
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
fn waltz_row_timing(w: &WaltzRow) -> Vec<String> {
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
    (w.c0..=w.c1).all(|c| (1..=2).any(|k| map.tile(c, r - k) == Tile::Solid))
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
    let roomy = behind.iter().filter(|c| (1..=2).all(|k| map.tile(c.0, c.1 - k) != Tile::Solid)).count();
    if roomy < WALTZ_RUNWAY {
        return None;
    }
    if !low_ceiling(map, w) {
        return None;
    }
    // The corridor: a floor under every step, nothing solid or otherwise deadly in the way.
    let walkable = (w.c0..=w.c1).all(|c| {
        map.floor(c, r + 1) != Floor::None && map.tile(c, r) != Tile::Solid && !map.tile(c, r).is_deadly()
    });
    (walkable && map.standable((end, r))).then_some((end, r))
}

/// A jump that needs a mode: from `from` (on its runway / after its nugget line) to `to`.
struct Crossing {
    mode: Mode,
    from: Cell,
    to: Cell,
    /// Cells first reachable through it.
    new: Vec<Cell>,
}

/// Can the player be in `mode` when taking off from `cell` in direction `dir`?
fn ready(map: &Map, g: &Graph, mode: Mode, cell: Cell, dir: i32) -> bool {
    match mode {
        Mode::Normal => true,
        // Room to toot 5 times.
        Mode::GiantSteps => map.flat_run(cell).len() >= RUNWAY,
        // Waltz rows are dashed, not jumped (see `waltz_dash`).
        Mode::Waltz => false,
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
                    (0..=NUGGET_LINE_REACH).contains(&back)
                        && (cell.1 - 4..=cell.1 + 1).contains(&r)
                        && g.touched.contains(&(c, r))
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

/// Normal-physics reachability from the start, plus mode crossings from wherever a mode can be
/// summoned, until nothing new is reached.
fn reach(map: &Map, start: Cell) -> (Graph, Vec<Crossing>) {
    let mut g = Graph::default();
    explore(map, vec![start], None, &mut g);
    let strategies = strategies();
    let mut crossings: Vec<Crossing> = Vec::new();
    let mut tried: HashSet<Cell> = HashSet::new();
    let mut buf = Vec::new();
    loop {
        let mut todo: Vec<Cell> = g.edges.keys().copied().filter(|c| !tried.contains(c)).collect();
        if todo.is_empty() {
            break;
        }
        todo.sort();
        let mut found: Vec<(Mode, Cell, Cell, Vec<Cell>)> = Vec::new();
        for &cell in &todo {
            tried.insert(cell);
            for mode in [Mode::GiantSteps, Mode::FiredUp] {
                let env = Env::new(mode, false);
                let ok: HashMap<i32, bool> = [-1, 0, 1].iter().map(|&d| (d, ready(map, &g, mode, cell, d))).collect();
                if !ok.values().any(|&b| b) {
                    continue;
                }
                for s in &strategies {
                    if !ok[&(s.dir as i32)] {
                        continue;
                    }
                    if let Some(land) = simulate(map, &env, cell, s, &mut buf)
                        && !g.edges.contains_key(&land)
                    {
                        found.push((mode, cell, land, buf.clone()));
                    }
                }
            }
        }
        for &cell in &todo {
            for w in &map.waltz_rows {
                for dir in [-1, 1] {
                    if let Some(land) = waltz_dash(map, w, cell, dir)
                        && !g.edges.contains_key(&land)
                    {
                        let touched = (w.c0..=w.c1).map(|c| (c, w.walk_row())).collect();
                        found.push((Mode::Waltz, cell, land, touched));
                    }
                }
            }
        }
        for (mode, from, to, touched) in found {
            if g.edges.contains_key(&to) {
                continue;
            }
            g.edges.get_mut(&from).expect("reached").push(to);
            g.touched.extend(touched);
            let new = explore(map, vec![to], Some(crossings.len()), &mut g);
            crossings.push(Crossing { mode, from, to, new });
        }
    }
    (g, crossings)
}

/// Problems with crossing `i`: it must be impossible in every other mode, even with ideal
/// input, from anywhere near its take-off reached before it.
fn exclusive(map: &Map, g: &Graph, crossings: &[Crossing], i: usize) -> Vec<String> {
    let c = &crossings[i];
    let beyond: HashSet<Cell> = c.new.iter().copied().collect();
    let near: Vec<Cell> = g
        .origin
        .iter()
        .filter(|(cell, o)| {
            o.is_none_or(|o| o < i) && (cell.0 - c.from.0).abs() <= 12 && (cell.1 - c.from.1).abs() <= 8
        })
        .map(|(cell, _)| *cell)
        .collect();
    let strategies = ideal_strategies();
    let mut buf = Vec::new();
    let mut errs = Vec::new();
    for mode in MODES.into_iter().filter(|&m| m != c.mode) {
        let env = Env::new(mode, true);
        let leak = near.iter().find_map(|&from| {
            strategies
                .iter()
                .find_map(|s| simulate(map, &env, from, s, &mut buf).filter(|l| beyond.contains(l)))
                .map(|l| (from, l))
        });
        if let Some((from, to)) = leak {
            errs.push(format!(
                "{} from col {} row {} (to col {} row {}) is passable with {mode:?} physics too: col {} row {} -> col {} row {}",
                c.mode.gate(),
                c.from.0,
                c.from.1,
                c.to.0,
                c.to.1,
                from.0,
                from.1,
                to.0,
                to.1
            ));
        }
    }
    errs
}

/// The crossings on the way to `cell` (latest first).
fn needs(g: &Graph, crossings: &[Crossing], cell: Cell) -> Vec<usize> {
    let mut chain = Vec::new();
    let mut at = cell;
    while let Some(Some(i)) = g.origin.get(&at) {
        chain.push(*i);
        at = crossings[*i].from;
    }
    chain
}

/// Shortest path cost (Manhattan tiles per hop) from `from` to `to`.
fn distance(g: &Graph, from: Cell, to: Cell) -> Option<u32> {
    let mut dist: HashMap<Cell, u32> = HashMap::new();
    let mut heap = BinaryHeap::new();
    dist.insert(from, 0);
    heap.push(std::cmp::Reverse((0u32, from)));
    while let Some(std::cmp::Reverse((d, c))) = heap.pop() {
        if c == to {
            return Some(d);
        }
        if dist.get(&c).is_some_and(|&b| d > b) {
            continue;
        }
        for &n in g.edges.get(&c).map(Vec::as_slice).unwrap_or(&[]) {
            let nd = d + ((n.0 - c.0).abs() + (n.1 - c.1).abs()) as u32;
            if dist.get(&n).is_none_or(|&b| nd < b) {
                dist.insert(n, nd);
                heap.push(std::cmp::Reverse((nd, n)));
            }
        }
    }
    None
}

fn dump(level: &Level, map: &Map, g: &Graph, crossings: &[Crossing]) -> String {
    let src_rows: Vec<Vec<char>> = (0..level.height)
        .map(|r| {
            (0..level.width)
                .map(|c| {
                    let i = r * level.width + c;
                    match level.tiles[i] {
                        Tile::Empty => '.',
                        Tile::Solid => '#',
                        Tile::OneWay => '=',
                        Tile::SpikesUp => '^',
                        Tile::SpikesDown => 'v',
                        Tile::Liquid => '~',
                    }
                })
                .collect()
        })
        .collect();
    let mut rows = src_rows;
    for &(c, r) in &g.touched {
        if c >= 0 && r >= 0 && (c as usize) < level.width && (r as usize) < level.height {
            let ch = &mut rows[r as usize][c as usize];
            if *ch == '.' {
                *ch = ',';
            }
        }
    }
    for &(c, r) in g.edges.keys() {
        rows[r as usize][c as usize] = '+';
    }
    for &(c, r) in &map.virt {
        if r >= 0 && c >= 0 && (r as usize) < level.height && (c as usize) < level.width {
            let ch = &mut rows[r as usize][c as usize];
            if *ch == '.' || *ch == ',' {
                *ch = '-';
            }
        }
    }
    for p in &level.platforms {
        for k in 0..p.width {
            rows[p.row][p.col + k] = '@';
        }
    }
    for x in crossings {
        rows[x.from.1 as usize][x.from.0 as usize] = match x.mode {
            Mode::GiantSteps => 'W',
            Mode::Waltz => 'Z',
            _ => 'R',
        };
    }
    for t in &level.things {
        let ok = g.touched.contains(&(t.col as i32, t.row as i32));
        rows[t.row][t.col] = match (t.kind, ok) {
            (ThingKind::Nugget, true) => 'o',
            (ThingKind::Nugget, false) => 'X',
            (ThingKind::Checkpoint, true) => 'C',
            (ThingKind::Checkpoint, false) => '!',
            (ThingKind::Fly, _) => 'F',
            (ThingKind::Spray, _) => 'S',
        };
    }
    rows[level.start.1][level.start.0] = 'P';
    rows[level.goal.1][level.goal.0] =
        if g.edges.contains_key(&(level.goal.0 as i32, level.goal.1 as i32)) { 'G' } else { '!' };
    let mut s = String::new();
    let ruler: String = (0..level.width).map(|c| if c % 10 == 0 { '|' } else { ' ' }).collect();
    s += &format!("    {ruler}\n");
    for (r, row) in rows.iter().enumerate() {
        s += &format!("{r:>3} {}\n", row.iter().collect::<String>());
    }
    s
}

/// What [`check`] found.
struct Report {
    errs: Vec<String>,
    /// Gates crossed: (mode, leads to the goal, nuggets behind it).
    gates: Vec<(Mode, bool, usize)>,
    /// The goal can only be reached through a gate.
    gated_goal: bool,
}

/// Validates one level.
fn check(idx: usize, level: &Level, allow_dump: bool) -> Report {
    let mut errs = Vec::new();
    let (name, world, _) = PLAN[idx];
    if level.name != name {
        errs.push(format!("name is {:?}, expected {name:?}", level.name));
    }
    if level.world != world {
        errs.push(format!("world is {}, expected {world}", level.world));
    }
    let map = Map::new(level);
    let cps: Vec<Cell> = level.checkpoints().map(|t| (t.col as i32, t.row as i32)).collect();
    if !(1..=3).contains(&cps.len()) {
        errs.push(format!("{} checkpoints (want 1..=3)", cps.len()));
    }
    if level.says.len() != cps.len() {
        errs.push(format!("{} `say:` lines for {} checkpoints", level.says.len(), cps.len()));
    }
    if level.intro.is_empty() {
        errs.push("missing intro".into());
    }
    for line in std::iter::once(&level.intro).chain(&level.says) {
        if line.chars().count() > MAX_LINE {
            errs.push(format!("line longer than {MAX_LINE} chars: {line:?}"));
        }
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
    for (what, cell) in
        [("start P", start), ("goal G", goal)].into_iter().chain(cps.iter().map(|&c| ("checkpoint", c)))
    {
        if !map.standable(cell) {
            errs.push(format!("{what} at col {} row {} is not standing on safe ground", cell.0, cell.1));
        }
        if map.sprays.contains(&cell) {
            errs.push(format!("{what} at col {} row {} is in a spray jet", cell.0, cell.1));
        }
    }
    if map.tile(goal.0, goal.1 - 1) == Tile::Solid {
        errs.push(format!("goal flag at col {} row {} has no room (2 tiles tall)", goal.0, goal.1));
    }
    // Safe start: nothing deadly within 3 tiles.
    for dc in -3..=3 {
        for dr in -3..=3 {
            let c = (start.0 + dc, start.1 + dr);
            if map.tile(c.0, c.1).is_deadly() || map.sprays.contains(&c) {
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
    for p in &level.platforms {
        let (c0, c1) = (p.col as f32 + p.dx.min(0.0), (p.col + p.width - 1) as f32 + p.dx.max(0.0));
        let (r0, r1) = (p.row as f32 - p.dy.max(0.0), p.row as f32 - p.dy.min(0.0));
        if c0 < 0.0 || c1 > (level.width - 1) as f32 || r0 < 0.0 || r1 > (level.height - 1) as f32 {
            errs.push(format!("platform at col {} row {} travels out of bounds", p.col, p.row));
        }
    }
    if !errs.is_empty() {
        // Structural problems: reachability would just add noise.
        return Report { errs, gates: Vec::new(), gated_goal: false };
    }

    let (g, crossings) = reach(&map, start);
    let dump_env = std::env::var("LEVEL_DUMP").ok();
    let only = std::env::var("LEVEL_ONLY").ok().and_then(|v| v.parse::<usize>().ok());
    let want_dump = allow_dump && dump_env.is_some() && only.is_none_or(|o| o == idx + 1);
    if !g.edges.contains_key(&goal) {
        errs.push(format!("goal at col {} row {} is unreachable from the start", goal.0, goal.1));
    }
    for &c in &cps {
        if !g.edges.contains_key(&c) {
            errs.push(format!("checkpoint at col {} row {} is unreachable", c.0, c.1));
        }
    }
    let lost: Vec<String> = level
        .things
        .iter()
        .filter(|t| t.kind == ThingKind::Nugget && !g.touched.contains(&(t.col as i32, t.row as i32)))
        .map(|t| format!("(col {} row {})", t.col, t.row))
        .collect();
    if lost.len() * 10 > n {
        errs.push(format!("{} of {n} nuggets unreachable: {}", lost.len(), lost.join(" ")));
    }

    // Gates: each one only passable in its own mode, and worth it (on the way to the goal, or
    // guarding a nugget-rich detour).
    let to_goal = needs(&g, &crossings, goal);
    let mut gates = Vec::new();
    let mut gate_lines = String::new();
    for (i, c) in crossings.iter().enumerate() {
        errs.extend(exclusive(&map, &g, &crossings, i));
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
        if !on_way && nuggets < DETOUR_NUGGETS {
            errs.push(format!(
                "{} from col {} row {} to col {} row {} leads nowhere much ({nuggets} nuggets, not the goal)",
                c.mode.gate(),
                c.from.0,
                c.from.1,
                c.to.0,
                c.to.1
            ));
        }
        gate_lines += &format!(
            "  {} at col {} row {} -> col {} row {}: {}, {nuggets} nuggets behind it\n",
            c.mode.gate(),
            c.from.0,
            c.from.1,
            c.to.0,
            c.to.1,
            if on_way { "on the way to the goal" } else { "detour" }
        );
        gates.push((c.mode, on_way, nuggets));
    }

    // Respawn spacing: walk P -> checkpoints (in order of distance) -> G.
    let mut summary = String::new();
    if errs.is_empty() {
        let mut order: Vec<(u32, Cell)> =
            cps.iter().map(|&c| (distance(&g, start, c).unwrap_or(u32::MAX), c)).collect();
        order.sort();
        let mut chain = vec![start];
        chain.extend(order.iter().map(|&(_, c)| c));
        chain.push(goal);
        for w in chain.windows(2) {
            match distance(&g, w[0], w[1]) {
                Some(d) if d <= MAX_SEGMENT => summary += &format!(" {d}"),
                Some(d) => errs.push(format!(
                    "segment {:?} -> {:?} costs {d} tiles (> {MAX_SEGMENT}): add a checkpoint",
                    w[0], w[1]
                )),
                None => errs.push(format!("can't get from {:?} to {:?} (respawn chain)", w[0], w[1])),
            }
        }
    }

    if want_dump || (allow_dump && !errs.is_empty() && dump_env.is_some()) {
        println!(
            "=== levels/{:02}.txt {:?}: {}x{}, {} checkpoints, {n} nuggets ({} unreachable), segments:{summary}",
            idx + 1,
            level.name,
            level.width,
            level.height,
            cps.len(),
            lost.len()
        );
        print!("{gate_lines}");
        println!("{}", dump(level, &map, &g, &crossings));
    }
    Report { errs, gates, gated_goal: !to_goal.is_empty() }
}

#[test]
fn levels_are_valid_and_beatable() {
    let mut failures = Vec::new();
    for (i, src) in LEVEL_SOURCES.iter().enumerate() {
        match Level::parse(src) {
            Err(e) => failures.push(format!("levels/{:02}.txt: parse error: {e}", i + 1)),
            Ok(level) => {
                let report = check(i, &level, true);
                let mut errs = report.errs;
                if errs.is_empty() {
                    // The campaign's gates: exactly the planned kinds.
                    let mut got: Vec<Mode> = report.gates.iter().map(|g| g.0).collect();
                    got.sort();
                    got.dedup();
                    if got != PLAN[i].2 {
                        errs.push(format!("gates {got:?}, planned {:?}", PLAN[i].2));
                    }
                    if GATED_GOAL.contains(&i) && !report.gated_goal {
                        errs.push("the goal must be behind the level's gate (it teaches it)".into());
                    }
                }
                for e in errs {
                    failures.push(format!("levels/{:02}.txt ({}): {e}", i + 1, level.name));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "level problems (LEVEL_DUMP=N cargo test --test levels -- --nocapture to see a map):\n{}",
        failures.join("\n")
    );
}

/// The validator itself must reject obviously broken levels.
#[test]
fn validator_catches_broken_levels() {
    let raw = "name: Bathroom Floor\nworld: 1\nintro: hi\nsay: hey\n---\n\
              ..........................................\n\
              .P.................C.....................G\n\
              ##########################################\n";
    let pad = |s: &str| {
        // Pad to the minimum height with sky rows.
        let (h, g) = s.split_once("---\n").unwrap();
        format!("{h}---\n{}{g}", format!("{}\n", ".".repeat(42)).repeat(12))
    };
    let ok = &pad(raw);
    let check_with = |s: &str, nuggets: &[usize]| {
        let mut l = Level::parse(s).unwrap();
        for c in 3..18 {
            l.things.push(Thing { kind: ThingKind::Nugget, col: c, row: 13 });
        }
        for &c in nuggets {
            l.things.push(Thing { kind: ThingKind::Nugget, col: c, row: 13 });
        }
        check(0, &l, false)
    };
    let errs_of = |s: &str| check_with(s, &[]).errs;
    let modes_of = |s: &str| check_with(s, &[]).gates.iter().map(|g| g.0).collect::<Vec<_>>();
    assert_eq!(errs_of(ok), Vec::<String>::new());
    assert!(modes_of(ok).is_empty());

    let with_gap = |n: usize| {
        let mut lines: Vec<String> = ok.lines().map(String::from).collect();
        let last = lines.len() - 1;
        let mut floor: Vec<char> = lines[last].chars().collect();
        for c in floor.iter_mut().skip(25).take(n) {
            *c = '.';
        }
        lines[last] = floor.into_iter().collect();
        lines.join("\n") + "\n"
    };
    assert!(errs_of(&with_gap(4)).is_empty(), "{:?}", errs_of(&with_gap(4)));
    assert!(errs_of(&with_gap(7)).is_empty(), "double jump clears 7: {:?}", errs_of(&with_gap(7)));
    assert!(modes_of(&with_gap(7)).is_empty());
    // A long gap: only with the band fired up, so it needs a nugget line after the checkpoint.
    let long = with_gap(11);
    assert!(errs_of(&long).iter().any(|e| e.contains("goal")), "11 tiles is too far without a nugget line");
    let fired = check_with(&long, &[20, 21, 22, 23]);
    assert!(fired.errs.is_empty(), "a nugget line fires up the band for 11 tiles: {:?}", fired.errs);
    assert_eq!(fired.gates.iter().map(|g| g.0).collect::<Vec<_>>(), [Mode::FiredUp]);
    assert!(fired.gated_goal);
    assert!(
        check_with(&with_gap(14), &[20, 21, 22, 23]).errs.iter().any(|e| e.contains("goal")),
        "a 14-tile gap must make the goal unreachable"
    );

    // A wall the player can't climb.
    let wall = |n: usize| {
        let mut lines: Vec<String> = ok.lines().map(String::from).collect();
        let floor_idx = lines.len() - 1;
        for k in 0..n {
            let idx = floor_idx - 1 - k;
            let mut row: Vec<char> = lines[idx].chars().collect();
            for c in row.iter_mut().skip(30) {
                if *c == '.' || *c == 'G' {
                    *c = '#';
                }
            }
            lines[idx] = row.into_iter().collect();
        }
        // Put the goal on top of the wall.
        let top = floor_idx - 1 - n;
        let mut row: Vec<char> = lines[top].chars().collect();
        *row.last_mut().unwrap() = 'G';
        lines[top] = row.into_iter().collect();
        lines.join("\n") + "\n"
    };
    assert!(errs_of(&wall(3)).is_empty(), "single jump climbs 3: {:?}", errs_of(&wall(3)));
    assert!(errs_of(&wall(5)).is_empty(), "double jump climbs 5: {:?}", errs_of(&wall(5)));
    assert!(modes_of(&wall(5)).is_empty(), "5 tiles is a normal wall");
    // A giant wall: only in Giant Steps, summoned on the runway before it.
    let giant = check_with(&wall(6), &[]);
    assert!(giant.errs.is_empty(), "giant steps climbs 6: {:?}", giant.errs);
    assert_eq!(giant.gates.iter().map(|g| g.0).collect::<Vec<_>>(), [Mode::GiantSteps]);
    assert!(giant.gated_goal);
    assert!(errs_of(&wall(9)).iter().any(|e| e.contains("goal")), "9 tiles is too tall");
    // No runway (ceiling spikes 5 tiles up all the way to the wall: tooting would splat):
    // no Giant Steps, no way up.
    let mut spiked: Vec<String> = wall(6).lines().map(String::from).collect();
    let row = spiked.len() - 7;
    let mut chars: Vec<char> = spiked[row].chars().collect();
    for ch in chars.iter_mut().take(30) {
        *ch = 'v';
    }
    spiked[row] = chars.into_iter().collect();
    let spiked = spiked.join("\n") + "\n";
    assert!(errs_of(&spiked).iter().any(|e| e.contains("goal")), "a giant wall needs a runway");
}

/// The waltz row gate: the numbers, and the validator telling good rows from bad ones.
#[test]
fn validator_knows_waltz_rows() {
    // The numbers (see `dash_through`): a row of n adjacent cans is a danger zone of 16n px.
    // Normal timing leaves 1.5 s to cross it; the waltz 2.5 s (on for the big ONE's beat).
    let row = |n: usize| WaltzRow { row: 11, c0: 20, c1: 20 + n as i32 - 1 };
    for n in [4, 10, 13] {
        assert!(dash_through(n, RUN_SPEED, Mode::Normal), "{n} cans: time enough at normal timing");
    }
    assert!(!dash_through(15, RUN_SPEED, Mode::Normal), "15 cans: 240px take 1.6s > 1.5s");
    assert!(dash_through(18, RUN_SPEED * 1.35, Mode::FiredUp), "fired up outruns 18 (288px in 1.42s)");
    assert!(!dash_through(20, RUN_SPEED * 1.35, Mode::FiredUp), "but not 20 (320px in 1.58s)");
    assert!(dash_through(21, RUN_SPEED * 0.9, Mode::Waltz), "the waltz's 2.5s carries a human 21 cans");
    assert!(!dash_through(22, RUN_SPEED * 0.9, Mode::Waltz));
    assert!(!dash_through(20, RUN_SPEED * 0.65, Mode::GiantSteps));
    assert!(waltz_row_timing(&row(20)).is_empty(), "{:?}", waltz_row_timing(&row(20)));
    assert!(waltz_row_timing(&row(12)).iter().any(|e| e.contains("Normal")));
    assert!(waltz_row_timing(&row(18)).iter().any(|e| e.contains("FiredUp")));
    assert!(waltz_row_timing(&row(25)).iter().any(|e| e.contains("too long")));

    // A tunnel: 20 cans under a grating, a low ceiling (a wall to the sky above it), a runway.
    let level = |cans: usize, ceiling: bool, runway_spikes: bool| {
        let (c0, c1) = (20, 20 + cans - 1);
        let w = 60;
        let mut rows: Vec<Vec<char>> = vec![vec!['.'; w]; 14];
        for r in 0..=7 {
            for c in c0..=c1 {
                rows[r][c] = if ceiling || r < 6 { '#' } else { '.' };
            }
        }
        for c in 0..w {
            rows[10][c] = if (c0..=c1).contains(&c) { '=' } else { '#' };
            rows[11][c] = if (c0..=c1).contains(&c) { 'S' } else { '#' };
            rows[12][c] = '#';
            rows[13][c] = '#';
        }
        rows[9][2] = 'P';
        rows[9][8] = 'C';
        rows[9][w - 2] = 'G';
        if runway_spikes {
            // A spike strip right before the row: no room to jump in threes.
            for c in 15..20 {
                rows[10][c] = '^';
            }
            rows[10][14] = '#';
        }
        let mut s = String::from("name: Bathroom Floor\nworld: 1\nintro: hi\nsay: hey\n---\n");
        for r in &rows {
            s.extend(r.iter());
            s.push('\n');
        }
        let mut l = Level::parse(&s).unwrap();
        for c in 3..18 {
            l.things.push(Thing { kind: ThingKind::Nugget, col: c, row: 9 });
        }
        check(0, &l, false)
    };
    let good = level(20, true, false);
    assert!(good.errs.is_empty(), "{:?}", good.errs);
    assert_eq!(good.gates.iter().map(|g| g.0).collect::<Vec<_>>(), [Mode::Waltz]);
    assert!(good.gated_goal, "the only way to the goal is the dash");
    let short = level(12, true, false);
    assert!(short.errs.iter().any(|e| e.contains("run through with Normal")), "{:?}", short.errs);
    let open = level(20, false, false);
    assert!(open.errs.iter().any(|e| e.contains("low ceiling")), "{:?}", open.errs);
    let cramped = level(20, true, true);
    assert!(cramped.errs.iter().any(|e| e.contains("goal")), "no runway, no waltz: {:?}", cramped.errs);
}
