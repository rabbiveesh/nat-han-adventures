//! The room canvas: a column-by-column builder for one room *segment* in the level format
//! (`crate::level`), so a template's output is an ordinary level the validator checks as is.
//!
//! Every segment has the same frame (rows from the top, [`H`] tall; a tall room, like a climb,
//! is [`CLIMB_H`] tall with the same frame pushed down, so its floor is `CLIMB_H - 5`):
//!
//! ```text
//!  rows 0..=8   ######  ......(room body: open sky)......  #####
//!  rows 9..=10  .P.C..  ...                             ...  ...G.
//!  row  11      ######  ######...(floor, pits)...######   #####
//!  rows 12..=15 ######  ######...                ######   #####
//!               entry   body                              exit
//! ```
//!
//! The entry and exit are *pipes*: two tiles of headroom under a solid roof up to the top of the
//! grid. Rooms are stitched exit-to-entry, so the only way between two rooms is a pipe nobody
//! can jump in: everything a room's validation proved about reaching (and not reaching) things
//! from its own entry holds in the stitched course too.

use crate::level::{Level, PlatformKind, Topic};

/// Rows in a room.
pub const H: usize = 16;
/// Rows in a tall room (a climb): the course is this tall, rooms sit on its bottom.
pub const CLIMB_H: usize = 36;
/// The row of the floor's surface tiles, and the row Nat stands in on it.
pub const FLOOR: usize = 11;
pub const STAND: usize = FLOOR - 1;
/// The last solid row of a pipe's roof (2 tiles of headroom under it).
pub const PIPE_ROOF: usize = STAND - 2;
/// Pipe lengths.
pub const ENTRY: usize = 6;
pub const EXIT: usize = 5;
/// Columns of the start (`P`, first room only), the room's checkpoint and the goal (last room).
pub const START_COL: usize = 1;
pub const CHECKPOINT_COL: usize = 3;
pub const GOAL_FROM_END: usize = 2;
/// Narrowest room, and the fewest nuggets in one (the validator's lower bounds for a level).
pub const MIN_WIDTH: usize = 40;
pub const MIN_NUGGETS: usize = 15;

pub struct Canvas {
    /// `grid[row][col]`, level-format chars.
    grid: Vec<Vec<u8>>,
    /// The floor row (see [`FLOOR`]): `height - (H - FLOOR)`.
    floor: usize,
    platforms: Vec<String>,
    hints: Vec<String>,
    says: Vec<String>,
}

impl Default for Canvas {
    fn default() -> Self {
        Self::new()
    }
}

impl Canvas {
    /// A canvas holding just the entry pipe (with `P` and the room's checkpoint).
    pub fn new() -> Canvas {
        Canvas::with_height(H)
    }

    /// A canvas `height` rows tall (at least [`H`]): the usual frame on its bottom rows.
    pub fn with_height(height: usize) -> Canvas {
        let height = height.max(H);
        let floor = height - (H - FLOOR);
        let mut c = Canvas { grid: vec![Vec::new(); height], floor, platforms: Vec::new(), hints: Vec::new(), says: Vec::new() };
        c.pipe(ENTRY);
        c.set(START_COL, c.stand(), b'P');
        c.set(CHECKPOINT_COL, c.stand(), b'C');
        c
    }

    pub fn height(&self) -> usize {
        self.grid.len()
    }

    /// This canvas's [`FLOOR`], [`STAND`] and [`PIPE_ROOF`] rows.
    pub fn floor(&self) -> usize {
        self.floor
    }

    pub fn stand(&self) -> usize {
        self.floor - 1
    }

    pub fn pipe_roof(&self) -> usize {
        self.floor - 3
    }

    /// Columns so far (the next column pushed is at this index).
    pub fn width(&self) -> usize {
        self.grid[0].len()
    }

    pub fn get(&self, col: usize, row: usize) -> u8 {
        self.grid.get(row).and_then(|r| r.get(col)).copied().unwrap_or(b'.')
    }

    pub fn set(&mut self, col: usize, row: usize, ch: u8) {
        if row < self.height() && col < self.width() {
            self.grid[row][col] = ch;
        }
    }

    /// Push one column with `f(row)` per row.
    fn push(&mut self, f: impl Fn(usize) -> u8) {
        for (r, row) in self.grid.iter_mut().enumerate() {
            row.push(f(r));
        }
    }

    /// `n` columns of pipe: roof, two tiles of headroom, floor.
    pub fn pipe(&mut self, n: usize) {
        let (roof, floor) = (self.pipe_roof(), self.floor);
        for _ in 0..n {
            self.push(|r| if r <= roof || r >= floor { b'#' } else { b'.' });
        }
    }

    /// `n` columns of ground whose surface is row `top` (solid from there down).
    pub fn ground(&mut self, n: usize, top: usize) {
        for _ in 0..n {
            self.push(|r| if r >= top { b'#' } else { b'.' });
        }
    }

    /// `n` columns of bottomless pit.
    pub fn pit(&mut self, n: usize) {
        for _ in 0..n {
            self.push(|_| b'.');
        }
    }

    /// `n` columns of sewage from the floor row down.
    pub fn pool(&mut self, n: usize) {
        let floor = self.floor;
        for _ in 0..n {
            self.push(|r| if r >= floor { b'~' } else { b'.' });
        }
    }

    /// Push columns built by `f(row)` (anything the helpers above don't cover).
    pub fn column(&mut self, f: impl Fn(usize) -> u8) {
        self.push(f);
    }

    /// The surface row of column `col` (the first ground, grating or hazard tile from the top
    /// below open air), if any.
    pub fn surface(&self, col: usize) -> Option<usize> {
        (1..self.height()).find(|&r| matches!(self.get(col, r), b'#' | b'=' | b'_' | b'^' | b'~') && self.get(col, r - 1) != b'#')
    }

    /// A nugget at (col, row) if the cell is free.
    pub fn nugget(&mut self, col: usize, row: usize) -> bool {
        if self.get(col, row) == b'.' {
            self.set(col, row, b'o');
            true
        } else {
            false
        }
    }

    pub fn nuggets(&self) -> usize {
        self.grid.iter().flatten().filter(|&&c| c == b'o').count()
    }

    /// A moving platform with its top tiles at (col.., row), `width` wide. Returns false when the
    /// room already has 9 (one digit each).
    #[allow(clippy::too_many_arguments)]
    pub fn platform(&mut self, col: usize, row: usize, width: usize, dx: f32, dy: f32, period: f32, phase: f32, kind: PlatformKind) -> bool {
        let n = self.platforms.len() + 1;
        if n > 9 {
            return false;
        }
        let kind = match kind {
            PlatformKind::Duck => "duck",
            PlatformKind::Plunger => "plunger",
            _ => "tp",
        };
        self.platforms.push(format!("{n}: dx={dx} dy={dy} period={period:.2} phase={phase:.2} kind={kind}"));
        for k in 0..width {
            self.set(col + k, row, b'0' + n as u8);
        }
        true
    }

    /// A hint spot (Han's line the first time Nat comes near).
    pub fn hint(&mut self, col: usize, row: usize, topics: &[Topic], text: &str) {
        let words: Vec<&str> = topics.iter().map(|t| t.word()).collect();
        self.hints.push(format!("hint@{col},{row} {}: {text}", words.join(" ")));
    }

    /// A checkpoint at (col, row) with Han's line.
    pub fn checkpoint(&mut self, col: usize, row: usize, line: &str) {
        self.set(col, row, b'C');
        self.says.push(format!("say@{col},{row}: {line}"));
    }

    /// Top up the nuggets to `min` on plain flat ground in the body: in the standing row every
    /// other column, then the columns between, then floating two tiles up (a hop), keeping clear
    /// of hazards and things.
    pub fn fill_nuggets(&mut self, min: usize) {
        let body = ENTRY..self.width().saturating_sub(1);
        for (parity, up) in [(0, 1), (1, 1), (0, 3), (1, 3)] {
            for col in body.clone().filter(|c| c % 2 == parity) {
                if self.nuggets() >= min {
                    return;
                }
                let Some(top) = self.surface(col) else { continue };
                if self.get(col, top) != b'#' || top < 4 {
                    continue;
                }
                let near_hazard = (col.saturating_sub(2)..=col + 2).any(|c| {
                    (0..self.height()).any(|r| matches!(self.get(c, r), b'S' | b'F' | b'^' | b'v' | b'~' | b'_'))
                });
                let busy = (top - up..top).any(|r| self.get(col, r) != b'.') || self.get(col, top - 1) != b'.';
                if !near_hazard && !busy {
                    self.nugget(col, top - up);
                }
            }
        }
    }

    /// Close the room: a stretch of floor, the exit pipe with the goal, and the level text.
    pub fn finish(mut self, world: u8, checkpoint_line: &str) -> Level {
        let (floor, stand) = (self.floor, self.stand());
        while self.width() + EXIT < MIN_WIDTH {
            self.ground(1, floor);
        }
        self.fill_nuggets(MIN_NUGGETS);
        // Still short (a crowded room): a bit more floor with nuggets on it.
        while self.nuggets() < MIN_NUGGETS {
            let x = self.width();
            self.ground(2, floor);
            self.nugget(x, stand);
        }
        self.says.push(format!("say@{CHECKPOINT_COL},{stand}: {checkpoint_line}"));
        self.pipe(EXIT);
        let goal = self.width() - GOAL_FROM_END;
        self.set(goal, stand, b'G');
        let mut src = format!("name: Free Play\nworld: {world}\nintro: Free play!\n");
        for l in self.platforms.iter().chain(&self.hints).chain(&self.says) {
            src.push_str(l);
            src.push('\n');
        }
        src.push_str("---\n");
        for row in &self.grid {
            src.push_str(std::str::from_utf8(row).expect("ascii"));
            src.push('\n');
        }
        Level::parse(&src).unwrap_or_else(|e| panic!("free play canvas made a bad level ({e}):\n{src}"))
    }
}
