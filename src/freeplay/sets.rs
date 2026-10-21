//! Sets: free play planned the way the band plans a chorus. A set is one continuous place
//! (about 45 to 60 s) grown from one *motif* (two things that act on each other, on one kind
//! of terrain), in four sections decided ahead of time, like the band's arcs of four:
//! - **head**: the motif stated plainly, at the set's full difficulty (free play never teaches:
//!   the story already has);
//! - **variation**: the same motif with one thing changed ([`Change`]);
//! - **bridge**: a fork where the call is the choice (`super::templates::fork`): each mode
//!   plays the motif its own way, no call takes the long way;
//! - **shout**: the motif at its densest, the best nuggets.
//!
//! Each section is laid beat by beat: a jump, a toot or a hop about every second of the main
//! line, never more than a few tiles of plain running ([`density`] measures it). The
//! difficulty is the set's band, once for all four sections. A checkpoint sits on the turns
//! into the bridge and the shout.
//!
//! The one motif so far is **rooftop over the canal**: rooftops of wandering heights over a
//! sewage canal, hops and toot gaps between them, chimneys to hop and awnings with nuggets
//! over the gaps.

use super::canvas::Canvas;
use super::dice::Dice;
use super::templates::{Built, CANAL_RISE, Dressing, fork, lerpi};
use crate::adapt::Band;
use crate::level::Topic;

/// What the variation changes about the head's motif.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// The roofs crumble: landings are crumbling slabs over the canal (the floor won't wait).
    Crumble,
    /// Flies hover over the gaps: every hop is a flat one, under them.
    Flies,
}

/// A section's part in the set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Head,
    Variation(Change),
    Bridge,
    Shout,
}

/// The set's chart: its sections in order, with their lengths in columns (the bridge's is
/// the fork's own). A pure function of the dice and the band, decided before anything is drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct Chart {
    pub sections: [(Role, usize); 4],
}

impl Chart {
    pub fn new(d: &mut Dice, band: Band) -> Chart {
        let change = if band >= 3 && d.chance(0.5) { Change::Flies } else { Change::Crumble };
        Chart {
            sections: [
                (Role::Head, 50 + d.int(0, 8) as usize),
                (Role::Variation(change), 44 + d.int(0, 8) as usize),
                (Role::Bridge, 0),
                (Role::Shout, 44 + d.int(0, 8) as usize),
            ],
        }
    }
}

/// The set's canvas rows (the bridge's rooftop, canal and tunnel stacked).
pub const SET_H: usize = super::templates::CROSS_H;

/// Rooftop over the canal, as a free play room: the whole set on one canvas.
pub fn rooftop(c: &mut Canvas, d: &mut Dice, band: Band, _: &Dressing) -> Built {
    let chart = Chart::new(d, band);
    let f = c.floor();
    let k = f - CANAL_RISE;
    let mut s = Lay { top: f, nuggets: 0, budget: 0 };
    for &(role, len) in &chart.sections {
        match role {
            // Up from the floor to the canal's level over the head.
            Role::Head => s.phrase(c, d, band, role, len, (f, k), (k, f)),
            // Wandering about the canal's level, back on it for the fork.
            Role::Variation(_) => s.phrase(c, d, band, role, len, (k, k), (k - 2, k + 3)),
            Role::Bridge => {
                s.settle(c, k);
                let x = c.width();
                c.ground(3, k);
                c.checkpoint(x + 1, k - 1, "Pick a road!");
                c.hint(x + 2, k - 1, &[Topic::Giant, Topic::Gap], "Pick a road!");
                fork(c, d, band, true);
                s.top = f;
            }
            Role::Shout => {
                let x = c.width();
                c.ground(2, f);
                c.checkpoint(x + 1, f - 1, "Shout chorus!");
                s.phrase(c, d, band, role, len, (f, f), (f - 4, f));
                s.settle(c, f);
                c.ground(2, f);
            }
        }
    }
    Built { hint: None, checkpoint: None }
}

/// Laying a section's beats: the current roof's surface row, nuggets placed so far.
struct Lay {
    top: usize,
    nuggets: usize,
    budget: usize,
}

/// Nuggets a section lays at most: the shout's are the best ones (the bridge's own come on
/// top: the validator's cap is 40).
fn budget(role: Role) -> usize {
    if role == Role::Shout { 7 } else { 4 }
}

impl Lay {
    fn nugget(&mut self, c: &mut Canvas, col: usize, row: usize) {
        if self.nuggets < self.budget && c.nugget(col, row) {
            self.nuggets += 1;
        }
    }

    /// Step the roofs to row `to` (hops of up to 2 over short gaps).
    fn settle(&mut self, c: &mut Canvas, to: usize) {
        while self.top != to {
            let next = if self.top > to { self.top - (self.top - to).min(2) } else { self.top + (to - self.top).min(3) };
            c.pool(2);
            c.ground(3, next);
            self.top = next;
        }
    }

    /// `len` columns of beats for `role`, the roofs heading from row `from` to row `to` and
    /// staying within rows `lo..=hi`.
    #[allow(clippy::too_many_arguments)]
    fn phrase(
        &mut self,
        c: &mut Canvas,
        d: &mut Dice,
        band: Band,
        role: Role,
        len: usize,
        (from, to): (usize, usize),
        (lo, hi): (usize, usize),
    ) {
        self.nuggets = 0;
        self.budget = budget(role);
        // A section opens on a roof (never on a gap right out of the pipe or the checkpoint).
        c.ground(2, self.top);
        let x0 = c.width();
        let end = x0 + len;
        let shout = role == Role::Shout;
        let crumble = role == Role::Variation(Change::Crumble) || (shout && d.chance(0.5));
        let flies = role == Role::Variation(Change::Flies);
        // Roofs: 4 → 2 wide (2 in the shout).
        let widest = if shout { 2 } else { lerpi(band, 4.0, 3.0) };
        let mut last = usize::MAX;
        while c.width() + 4 < end {
            let p = (c.width() - x0) as f32 / len as f32;
            // (The shout climbs and comes back down: a swell to the exit.)
            let swell = if shout { (4.0 * (p * std::f32::consts::PI).sin()).round() as usize } else { 0 };
            let aim = (from as f32 + (to as f32 - from as f32) * p).round() as usize - swell;
            // Wander about the aim, up to 2 tiles either side.
            let (lo, hi) = (lo.max(aim.saturating_sub(2)), hi.min(aim + 2));
            let mut beat = d.int(0, 3);
            if beat == last as i32 && d.chance(0.6) {
                beat = (beat + 1) % 4;
            }
            last = beat as usize;
            let w = d.int(2, widest as i32) as usize;
            match beat {
                // A hop: a short gap, a step up or down.
                0 => {
                    let g = d.int(2, 3 + (band >= 6) as i32) as usize;
                    let up = if g >= 4 { 1 } else { 2 };
                    let want = if self.top > aim { -(up as i32) } else { d.int(-1, 2) };
                    let next = self.step(want, up, lo, hi);
                    let x = c.width();
                    c.pool(g);
                    if flies && g <= 3 {
                        c.set(x + g / 2, self.top.min(next) - 5, b'F');
                    } else if g >= 3 {
                        self.nugget(c, x + g / 2, self.top.min(next) - 2);
                    }
                    self.land(c, w, next, crumble);
                }
                // A toot gap: too wide for a jump, a nugget where the toot goes.
                1 => {
                    let g = d.int(5, 6) as usize;
                    let want = if self.top > aim { 1 } else { d.int(-1, 0) };
                    let next = self.step(want, 1, lo, hi);
                    let x = c.width();
                    c.pool(g);
                    self.nugget(c, x + g / 2, self.top.min(next) - 4);
                    self.land(c, w.max(2), next, crumble);
                }
                // A chimney on the roof to hop.
                2 if !crumble => {
                    let x = c.width();
                    c.ground(1, self.top);
                    c.ground(1, self.top - 2);
                    c.ground(2, self.top);
                    self.nugget(c, x + 1, self.top - 4);
                }
                // An awning over a gap: one-way, nuggets on it (the higher line).
                _ => {
                    let g = 4;
                    let x = c.width();
                    c.pool(g);
                    let row = self.top - 3;
                    for j in 1..3 {
                        c.set(x + j, row, b'=');
                    }
                    self.nugget(c, x + 1, row - 1);
                    self.nugget(c, x + 2, row - 1);
                    self.land(c, w, self.top, crumble);
                }
            }
        }
    }

    /// The next roof's row: `want` tiles up (negative: down) from this one, at most `up` up and
    /// 3 down, kept within rows `lo..=hi` where that's in reach.
    fn step(&self, want: i32, up: usize, lo: usize, hi: usize) -> usize {
        let (reach_lo, reach_hi) = (self.top.saturating_sub(up), self.top + 3);
        let next = (self.top as i32 - want).clamp(reach_lo as i32, reach_hi as i32) as usize;
        next.clamp(lo.max(reach_lo).min(reach_hi), hi.min(reach_hi).max(reach_lo))
    }

    /// A landing roof `w` wide at row `top`: a building, or a crumbling slab over the canal.
    fn land(&mut self, c: &mut Canvas, w: usize, top: usize, crumble: bool) {
        if crumble {
            let x = c.width();
            c.pool(w);
            for j in 0..w {
                c.set(x + j, top, b'-');
            }
        } else {
            c.ground(w, top);
        }
        self.top = top;
    }
}

/// How dense a level's main line is: the longest stretch of plain floor (columns of the same
/// surface with nothing on or over them) and the share of the body's columns that are plain.
pub fn density(level: &crate::level::Level) -> (usize, f32) {
    use crate::level::Tile;
    let surface = |col: usize| (1..level.height).find(|&r| level.tile(col as i32, r as i32) != Tile::Empty);
    let thing_at = |col: usize| level.things.iter().any(|t| t.col == col);
    let (mut longest, mut run, mut plain) = (0, 0, 0);
    let mut prev = None;
    let body = super::canvas::ENTRY..level.width.saturating_sub(super::canvas::EXIT);
    for col in body.clone() {
        let s = surface(col);
        let flat = s.is_some() && s == prev && level.tile(col as i32, s.unwrap() as i32) == Tile::Solid && !thing_at(col);
        if flat {
            run += 1;
            plain += 1;
        } else {
            run = 0;
        }
        longest = longest.max(run);
        prev = s;
    }
    (longest, plain as f32 / body.len().max(1) as f32)
}
