//! Level validator: every level parses, matches the campaign plan, and is beatable.
//!
//! The heart of it is a conservative reachability search: from each "standable" cell we simulate
//! a fan of jumps (hold lengths, running/standing starts, toot double-jump timings, air control)
//! with the real tuning constants and tile AABB collision, and follow every arc that lands without
//! touching anything deadly. Moving platforms are approximated as one-way tiles along their whole
//! path (as if you can ride them anywhere they go). Flies are treated as static deadly squares
//! covering their whole circle; spray jets are treated as passable (they're timed).
//!
//! `LEVEL_DUMP=1 cargo test --test levels -- --nocapture` prints every level with the reachable
//! cells marked (add `LEVEL_ONLY=3` for just level 3):
//! `+` standable & reachable, `,` passed through by some safe arc, `X` unreachable nugget,
//! `!` unreachable checkpoint/goal, `@` moving platform (start), `-` its path.

use std::collections::{BinaryHeap, HashMap, HashSet};

use durhay::game::tuning::*;
use durhay::level::*;

const PLAN: [(&str, u8); LEVEL_COUNT] = [
    ("Bathroom Floor", 1),
    ("The Bowl", 1),
    ("U-Bend", 2),
    ("Pipe Maze", 2),
    ("Main Sewer", 3),
    ("Rat Kingdom", 3),
    ("Septic Tank", 4),
    ("Porta-Potty Festival", 4),
    ("Treatment Plant", 5),
    ("The Golden Throne", 5),
];

const MAX_LINE: usize = 60;
const NUGGETS: std::ops::RangeInclusive<usize> = 15..=40;
/// Path cost (tiles moved, Manhattan per hop) allowed between respawn points: about 30s of play.
const MAX_SEGMENT: u32 = 150;
/// Fly swarms circle ~1 tile around their cell; treat the whole circle (plus a bit) as deadly.
const FLY_REACH: f32 = 20.0;

const HALF_W: f32 = PLAYER_SIZE.0 / 2.0;
const HALF_H: f32 = PLAYER_SIZE.1 / 2.0;
const DT: f32 = 1.0 / 120.0;
/// Human margin: assume the player only gets 90% of top speed out of a jump, and only dares to
/// hang 8px of the 12px box over a ledge before taking off.
const VX: f32 = RUN_SPEED * 0.9;
const OVERHANG: f32 = 8.0;
const MAX_T: f32 = 3.0;

type Cell = (i32, i32);

/// Collision-relevant view of a level (pixel space here is x right, y DOWN, origin top-left).
struct Map<'a> {
    level: &'a Level,
    w: i32,
    h: i32,
    /// Cells occupied by some moving platform at some point of its path.
    virt: HashSet<Cell>,
    flies: Vec<(f32, f32)>,
    sprays: HashSet<Cell>,
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
        for t in &level.things {
            match t.kind {
                ThingKind::Fly => flies.push((t.col as f32 * TILE + 8.0, t.row as f32 * TILE + 8.0)),
                ThingKind::Spray => {
                    for k in 0..=3 {
                        sprays.insert((t.col as i32, t.row as i32 - k));
                    }
                }
                _ => {}
            }
        }
        Map { level, w: level.width as i32, h: level.height as i32, virt, flies, sprays }
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
    vx0: f32,
    /// Toot double jump at this time.
    toot: Option<f32>,
    /// Start hanging over the edge of the cell in `dir`.
    edge: bool,
    /// Let go of the direction key after this long.
    release: f32,
}

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
                    for vx0 in [0.0, dir * VX] {
                        for edge in [false, true] {
                            out.push(Strategy { dir, jump: true, hold, vx0, toot, edge, release });
                        }
                    }
                }
                // Running off a ledge (coyote jumps are covered by edge jumps).
                let hold = f32::INFINITY;
                out.push(Strategy { dir, jump: false, hold, vx0: dir * VX, toot, edge: true, release });
            }
        }
    }
    out
}

/// Result of one simulated arc: where it lands (if safely) and which cells the box touched.
fn simulate(map: &Map, (c, r): Cell, s: &Strategy, touched: &mut Vec<Cell>) -> Option<Cell> {
    touched.clear();
    let mut x = c as f32 * TILE + 8.0;
    if s.edge {
        let next = map.tile(c + s.dir as i32, r);
        if next == Tile::Solid || map.standable((c + s.dir as i32, r)) {
            return None; // not a ledge
        }
        x += s.dir * (8.0 + OVERHANG - HALF_W);
    }
    let mut y = (r + 1) as f32 * TILE - HALF_H;
    if map.deadly(x, y) {
        return None;
    }
    let mut vx = s.vx0;
    let mut vy = if s.jump { -JUMP_SPEED } else { 0.0 };
    let mut cut = !s.jump;
    let mut tooted = false;
    let mut t = 0.0;
    while t < MAX_T {
        t += DT;
        // Input.
        let input = if t < s.release { s.dir } else { 0.0 };
        let target = input * VX;
        let dv = AIR_ACCEL * DT;
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
        vy = (vy + GRAVITY * DT).min(MAX_FALL);

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

struct Graph {
    /// Outgoing edges per reachable standable cell.
    edges: HashMap<Cell, Vec<Cell>>,
    /// Every cell some safe arc (or standing) touches.
    touched: HashSet<Cell>,
}

fn explore(map: &Map, start: Cell) -> Graph {
    let strategies = strategies();
    let mut edges: HashMap<Cell, Vec<Cell>> = HashMap::new();
    let mut touched = HashSet::new();
    let mut queue = vec![start];
    let mut buf = Vec::new();
    while let Some(cell) = queue.pop() {
        if edges.contains_key(&cell) {
            continue;
        }
        touched.insert(cell);
        let mut out: HashSet<Cell> = HashSet::new();
        for d in [-1, 1] {
            let n = (cell.0 + d, cell.1);
            if map.standable(n) {
                out.insert(n);
            }
        }
        for s in &strategies {
            if let Some(land) = simulate(map, cell, s, &mut buf) {
                out.insert(land);
                touched.extend(buf.iter().copied());
            }
        }
        out.remove(&cell);
        let out: Vec<Cell> = out.into_iter().collect();
        queue.extend(out.iter().copied().filter(|c| !edges.contains_key(c)));
        edges.insert(cell, out);
    }
    Graph { edges, touched }
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

fn dump(level: &Level, map: &Map, g: &Graph) -> String {
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

/// Validates one level; returns a list of problems.
fn check(idx: usize, level: &Level, allow_dump: bool) -> Vec<String> {
    let mut errs = Vec::new();
    let (name, world) = PLAN[idx];
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
    for p in &level.platforms {
        let (c0, c1) = (p.col as f32 + p.dx.min(0.0), (p.col + p.width - 1) as f32 + p.dx.max(0.0));
        let (r0, r1) = (p.row as f32 - p.dy.max(0.0), p.row as f32 - p.dy.min(0.0));
        if c0 < 0.0 || c1 > (level.width - 1) as f32 || r0 < 0.0 || r1 > (level.height - 1) as f32 {
            errs.push(format!("platform at col {} row {} travels out of bounds", p.col, p.row));
        }
    }
    if !errs.is_empty() {
        return errs; // structural problems: reachability would just add noise
    }

    let g = explore(&map, start);
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
        println!("{}", dump(level, &map, &g));
    }
    errs
}

#[test]
fn levels_are_valid_and_beatable() {
    let mut failures = Vec::new();
    for (i, src) in LEVEL_SOURCES.iter().enumerate() {
        match Level::parse(src) {
            Err(e) => failures.push(format!("levels/{:02}.txt: parse error: {e}", i + 1)),
            Ok(level) => {
                for e in check(i, &level, true) {
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
    let errs_of = |s: &str| {
        let mut l = Level::parse(s).unwrap();
        // Ignore nugget count for these synthetic levels.
        for c in 3..18 {
            l.things.push(Thing { kind: ThingKind::Nugget, col: c, row: 13 });
        }
        check(0, &l, false)
    };
    assert_eq!(errs_of(ok), Vec::<String>::new());

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
    assert!(
        errs_of(&with_gap(12)).iter().any(|e| e.contains("goal")),
        "a 12-tile gap must make the goal unreachable"
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
    assert!(errs_of(&wall(7)).iter().any(|e| e.contains("goal")), "7 tiles is too tall");
}
