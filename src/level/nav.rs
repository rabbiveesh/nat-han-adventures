//! Han's navigation graph: the validator's reachability graph (standable cells and simulated
//! jump arcs) with Han's own physics ([`Env::han`]: his run speed, his 3 toots), on his view of
//! the level ([`Map::for_han`]: flies and spray jets don't hurt him, spikes and sewage he steers
//! clear of), minus the cells he keeps away from ([`Level::han_allowed`]: the band's gates).
//!
//! Built lazily: a cell's out-edges are simulated the first time a route search reaches it
//! (and kept for the level visit), so a level load costs nothing and a route costs a few
//! hundred arcs the first time through a stretch. [`Nav::build_all`] does the whole level at
//! once (for timing). A route is A* over seconds of travel.

use std::collections::{BinaryHeap, HashMap, HashSet};

use super::validate::{Cell, Env, Flight, Map, Mode, Outcome, fly_timed};
use super::{Level, TILE};
use crate::game::tuning::{JUMP_SPEED, PLAYER_SIZE};
use crate::level::buddy::HAN_RUN_SPEED;

/// One of Han's moves from a standing start (he stops, then goes: easy to replay exactly).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HanMove {
    pub dir: f32,
    /// Start hanging over the edge of the cell in `dir`.
    pub edge: bool,
    /// Jump (else walk off the edge).
    pub jump: bool,
    /// Release jump after this long.
    pub hold: f32,
    /// Let go of the direction after this long.
    pub release: f32,
    /// Toot times (`INFINITY`: unused).
    pub toots: [f32; 3],
}

impl HanMove {
    pub fn toot_count(&self) -> usize {
        self.toots.iter().filter(|t| t.is_finite()).count()
    }
}

const INF: f32 = f32::INFINITY;

/// Han's moves: few, since three toots make up for a sloppy take-off.
pub fn han_moves() -> Vec<HanMove> {
    let toots = [[INF; 3], [0.3, INF, INF], [0.3, 0.62, 0.94]];
    let mut out = Vec::new();
    for hold in [INF, 0.1] {
        for t in toots {
            out.push(HanMove { dir: 0.0, edge: false, jump: true, hold, release: INF, toots: t });
        }
    }
    for dir in [-1.0, 1.0] {
        for release in [INF, 0.2] {
            for t in toots {
                for hold in [INF, 0.1] {
                    for edge in [false, true] {
                        out.push(HanMove { dir, edge, jump: true, hold, release, toots: t });
                    }
                }
                let walk_off = [[INF; 3], [0.15, INF, INF], [0.15, 0.47, INF]];
                let toots = walk_off[toots.iter().position(|x| *x == t).unwrap_or(0)];
                out.push(HanMove { dir, edge: true, jump: false, hold: INF, release, toots });
            }
        }
    }
    out
}

/// How an edge is travelled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Step {
    /// Walk to the neighbouring cell.
    Walk,
    /// Han's move number `n` (of [`han_moves`]).
    Move(u16),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edge {
    pub to: Cell,
    pub step: Step,
    /// Seconds it takes.
    pub secs: f32,
}

/// Where move `m` takes off in `cell` (box center x, y down), if it applies there.
pub fn take_off(map: &Map, env: &Env, (c, r): Cell, m: &HanMove) -> Option<f32> {
    let mut x = c as f32 * TILE + 8.0;
    if m.edge {
        let n = (c + m.dir as i32, r);
        if map.is_solid(n.0, n.1) || map.standable(n) {
            return None;
        }
        x += m.dir * (8.0 + env.overhang - PLAYER_SIZE.0 / 2.0);
    }
    Some(x)
}

/// The flight of move `m` from `cell` (validator coordinates).
pub(crate) fn flight(map: &Map, env: &Env, cell: Cell, m: &HanMove) -> Option<Flight> {
    let x0 = take_off(map, env, cell, m)?;
    Some(Flight {
        x0,
        y0: (cell.1 + 1) as f32 * TILE - PLAYER_SIZE.1 / 2.0,
        vx0: 0.0,
        vy0: if m.jump { -JUMP_SPEED } else { 0.0 },
        dir: m.dir,
        release: m.release,
        hold: m.hold,
        cut: !m.jump,
        toots: m.toots,
        toot_speed: env.toot,
    })
}

/// Seconds per tile walked.
pub const WALK_SECS: f32 = TILE / HAN_RUN_SPEED;
/// Extra cost of a jump (stop, go) and of each toot: Han prefers walking, and fewer toots.
const JUMP_COST: f32 = 0.2;
const TOOT_COST: f32 = 0.08;

/// What a route search found.
#[derive(Debug, Clone, PartialEq)]
pub enum Route {
    /// The edges to take, in order (empty: already there).
    Found(Vec<Edge>),
    /// No way there (with Han's physics, keeping clear of the band's gates).
    NoRoute,
    /// Ran out of the simulation budget: ask again next time (the cache keeps the work).
    Budget,
}

/// Han's graph for one level visit and one mode.
pub struct Nav {
    pub mode: Mode,
    pub env: Env,
    moves: Vec<HanMove>,
    cache: HashMap<Cell, Vec<Edge>>,
    /// Cells whose edges have been simulated (for stats).
    pub expanded: usize,
    buf: Vec<Cell>,
}

impl Nav {
    pub fn new(mode: Mode) -> Nav {
        Nav { mode, env: Env::han(mode), moves: han_moves(), cache: HashMap::new(), expanded: 0, buf: Vec::new() }
    }

    pub fn moves(&self) -> &[HanMove] {
        &self.moves
    }

    /// Forget every edge (the level changed under him: a new stain).
    pub fn invalidate(&mut self) {
        self.cache.clear();
    }

    /// Can Han stand in `cell`?
    pub fn node(map: &Map, level: &Level, cell: Cell) -> bool {
        map.standable(cell) && level.han_allowed(cell)
    }

    pub fn cached(&self, cell: Cell) -> bool {
        self.cache.contains_key(&cell)
    }

    /// `cell`'s out-edges (simulated the first time).
    pub fn edges(&mut self, map: &Map, cell: Cell) -> &[Edge] {
        if !self.cache.contains_key(&cell) {
            let edges = self.simulate_edges(map, cell);
            self.cache.insert(cell, edges);
        }
        &self.cache[&cell]
    }

    fn simulate_edges(&mut self, map: &Map, cell: Cell) -> Vec<Edge> {
        self.expanded += 1;
        let level = map.level;
        let mut out: Vec<Edge> = Vec::new();
        if !Nav::node(map, level, cell) {
            return out;
        }
        for d in [-1, 1] {
            let n = (cell.0 + d, cell.1);
            if Nav::node(map, level, n) {
                out.push(Edge { to: n, step: Step::Walk, secs: WALK_SECS });
            }
        }
        for (k, m) in self.moves.iter().enumerate() {
            let Some(f) = flight(map, &self.env, cell, m) else { continue };
            let (outcome, t) = fly_timed(map, &self.env, &f, &mut self.buf);
            if let Outcome::Land(to) = outcome
                && to != cell
                && level.han_allowed(to)
            {
                let secs = t + JUMP_COST + TOOT_COST * m.toot_count() as f32;
                match out.iter_mut().find(|e| e.to == to) {
                    Some(e) if e.secs > secs => *e = Edge { to, step: Step::Move(k as u16), secs },
                    Some(_) => {}
                    None => out.push(Edge { to, step: Step::Move(k as u16), secs }),
                }
            }
        }
        out
    }

    /// Simulate every node's edges (what a full build at load would cost). Returns the nodes.
    pub fn build_all(&mut self, map: &Map) -> usize {
        let mut n = 0;
        for r in 0..map.level.height as i32 {
            for c in 0..map.level.width as i32 {
                if Nav::node(map, map.level, (c, r)) {
                    self.edges(map, (c, r));
                    n += 1;
                }
            }
        }
        n
    }

    /// A* from `from` to `to`, simulating at most `budget` new cells.
    pub fn route(&mut self, map: &Map, from: Cell, to: Cell, budget: usize) -> Route {
        if from == to {
            return Route::Found(Vec::new());
        }
        if !Nav::node(map, map.level, to) || !Nav::node(map, map.level, from) {
            return Route::NoRoute;
        }
        let h = |c: Cell| ((c.0 - to.0).abs() as f32) * WALK_SECS;
        #[derive(PartialEq)]
        struct Open(f32, Cell);
        impl Eq for Open {}
        impl PartialOrd for Open {
            fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(o))
            }
        }
        impl Ord for Open {
            fn cmp(&self, o: &Self) -> std::cmp::Ordering {
                o.0.total_cmp(&self.0).then(o.1.cmp(&self.1))
            }
        }
        let mut best: HashMap<Cell, (f32, Option<(Cell, Edge)>)> = HashMap::new();
        let mut heap = BinaryHeap::new();
        let mut closed: HashSet<Cell> = HashSet::new();
        best.insert(from, (0.0, None));
        heap.push(Open(h(from), from));
        let mut spent = 0;
        while let Some(Open(_, c)) = heap.pop() {
            if c == to {
                let mut path = Vec::new();
                let mut at = to;
                while let Some((_, Some((prev, e)))) = best.get(&at) {
                    path.push(*e);
                    at = *prev;
                }
                path.reverse();
                return Route::Found(path);
            }
            if !closed.insert(c) {
                continue;
            }
            if !self.cached(c) {
                if spent >= budget {
                    return Route::Budget;
                }
                spent += 1;
            }
            let g = best[&c].0;
            let edges: Vec<Edge> = self.edges(map, c).to_vec();
            for e in edges {
                let ng = g + e.secs;
                if best.get(&e.to).is_none_or(|b| ng < b.0) {
                    best.insert(e.to, (ng, Some((c, e))));
                    heap.push(Open(ng + h(e.to), e.to));
                }
            }
        }
        Route::NoRoute
    }
}
