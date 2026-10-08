//! The course: one growing level that validated rooms are stitched into, exit pipe to entry
//! pipe (see [`super::canvas`]).
//!
//! The grid is allocated at its full width up front ([`Course::new`]) and filled in as rooms
//! come, so tile coordinates never move, the camera and physics need nothing new, and adding a
//! room is a copy into the grid. Past the last room the grid is empty sky, closed off by a
//! *cap*: two solid tiles that plug the end of the last exit pipe until the next room replaces
//! them. The goal flag waits out in the empty sky at the far end until the last room of the
//! run is stitched (fixed-length runs; an endless run that fills the grid ends there too).

use super::canvas::{CHECKPOINT_COL, ENTRY, H, PIPE_ROOF, STAND};
use crate::level::{Level, Thing, ThingKind, Tile};

/// Columns per room the grid allows for.
pub const COLS_PER_ROOM: usize = 160;
/// Rooms an endless run's grid holds (it ends with a goal flag if they're ever all played).
pub const ENDLESS_ROOMS: usize = 99;

/// Where a stitched room sits in the course.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed {
    /// Its first column (the start of its entry pipe).
    pub col0: usize,
    pub width: usize,
    /// Index (among the course's checkpoints) of its entry checkpoint.
    pub checkpoint: usize,
}

impl Placed {
    pub fn cols(&self) -> std::ops::Range<usize> {
        self.col0..self.col0 + self.width
    }

    /// The column of its entry checkpoint: crossing it starts the room.
    pub fn start_col(&self) -> usize {
        self.col0 + CHECKPOINT_COL
    }
}

/// The course's bookkeeping; the grid itself is a [`Level`] kept by the caller (in the game,
/// the loaded `ActiveLevel`, with its splat stains), passed to every change.
#[derive(Debug, Clone, Default)]
pub struct Course {
    pub rooms: Vec<Placed>,
    /// Columns in use (the next room goes here).
    pub frontier: usize,
    /// The plug at the end of the last room, if any (its column).
    pub cap: Option<usize>,
}

impl Course {
    /// An empty course for up to `rooms` rooms, and its grid.
    pub fn new(world: u8, rooms: usize) -> (Course, Level) {
        let width = rooms.max(1) * COLS_PER_ROOM + 4;
        let level = Level {
            name: "Free Play".into(),
            world,
            intro: String::new(),
            says: Vec::new(),
            say_at: Vec::new(),
            hints: Vec::new(),
            deaths: None,
            width,
            height: H,
            tiles: vec![Tile::Empty; width * H],
            start: (1, STAND),
            goal: (width - 2, STAND),
            things: Vec::new(),
            platforms: Vec::new(),
        };
        (Course { rooms: Vec::new(), frontier: 0, cap: None }, level)
    }

    /// Would a room `width` wide still fit in `level`?
    pub fn fits(&self, level: &Level, width: usize) -> bool {
        self.frontier + width + 2 < level.width
    }

    /// Stitch `room` in after the last one; `last` puts the goal flag in its exit pipe.
    /// Returns where it went, and the cap cells it replaced (to redraw).
    pub fn add(&mut self, level: &mut Level, room: &Level, last: bool) -> (Placed, Vec<(usize, usize)>) {
        assert_eq!(room.height, H, "rooms are {H} tall");
        let col0 = self.frontier;
        let w = level.width;
        let mut uncapped = Vec::new();
        if let Some(cap) = self.cap.take() {
            uncapped.extend((PIPE_ROOF + 1..=STAND).map(|r| (cap, r)));
        }
        for r in 0..H {
            for c in 0..room.width {
                level.tiles[r * w + col0 + c] = room.tile(c as i32, r as i32);
            }
        }
        let checkpoint = level.checkpoints().count();
        // Left to right, so checkpoint numbers grow along the course (the game keeps the
        // highest one touched).
        let mut things: Vec<Thing> = room.things.iter().map(|t| Thing { col: t.col + col0, ..*t }).collect();
        things.sort_by_key(|t| (t.col, t.row));
        level.things.extend(things);
        for p in &room.platforms {
            level.platforms.push(crate::level::MovingPlatformDef { col: p.col + col0, ..p.clone() });
        }
        for s in &room.hints {
            level.hints.push(crate::level::Spot { col: s.col + col0, ..s.clone() });
        }
        for s in &room.say_at {
            level.say_at.push(crate::level::Spot { col: s.col + col0, ..s.clone() });
        }
        if self.rooms.is_empty() {
            level.start = (room.start.0 + col0, room.start.1);
        }
        self.frontier = col0 + room.width;
        if last {
            level.goal = (room.goal.0 + col0, room.goal.1);
        } else {
            let cap = self.frontier;
            for r in PIPE_ROOF + 1..=STAND {
                level.tiles[r * w + cap] = Tile::Solid;
            }
            self.cap = Some(cap);
        }
        let placed = Placed { col0, width: room.width, checkpoint };
        self.rooms.push(placed);
        (placed, uncapped)
    }

    /// Close the entry pipe of room `k` behind Nat (the rooms before it get unloaded). Returns
    /// the cells made solid.
    pub fn seal(&self, level: &mut Level, k: usize) -> Vec<(usize, usize)> {
        let Some(p) = self.rooms.get(k) else { return Vec::new() };
        let col = p.col0;
        let w = level.width;
        let cells: Vec<(usize, usize)> = (PIPE_ROOF + 1..=STAND).map(|r| (col, r)).collect();
        for &(c, r) in &cells {
            level.tiles[r * w + c] = Tile::Solid;
        }
        cells
    }

    /// Which room column `col` is in.
    pub fn room_at(&self, col: usize) -> Option<usize> {
        self.rooms.iter().position(|p| p.cols().contains(&col))
    }

    /// Nuggets in room `k`.
    pub fn nuggets_in(&self, level: &Level, k: usize) -> usize {
        let cols = self.rooms[k].cols();
        level.things.iter().filter(|t| t.kind == ThingKind::Nugget && cols.contains(&t.col)).count()
    }
}

/// The body columns start here in a room (for tests and tools).
pub const BODY_START: usize = ENTRY;
