//! Side B rooms in free play: Han goes on alone while Nat waits in the pipe, and the player
//! calls the band from a set list, as on Side B (`game::sideb`). They're drawn from Side B's
//! pieces (a giant wall, a long gap, a drip pipe, the spike alley, the deck fork), only those of
//! the Side B levels the player has cleared, and the nuggets that pay for the calls come with
//! them.
//!
//! The validator can't prove these (it plays Nat, not Han alone with a band), so every room is
//! built *with* its solution: each piece comes with the calls that get Han over it (a bar
//! earlier than the Side B levels' routes, `tests/side_b.rs`: a call lands on the next bar
//! line, and the music's playing on for Nat when the room starts), pieces are spaced so one call's
//! hold is over before the next piece needs another, and a nugget is put on Han's way ahead of
//! every call that isn't paid for yet. [`draw`] returns that solution as a plan, and
//! `tests/side_b.rs` plays it on many seeds.

use super::canvas::{Canvas, ENTRY, H};
use super::dice::{Dice, t};
use super::templates::{Built, Dressing};
use crate::adapt::Band;
use crate::audio::Harmony;
use crate::game::sideb::SetCall;

/// Rows in a Side B room: one more than a room, so the floor and the headroom above it are
/// Side B's own (air rows 0..=11, the floor at 12).
pub const ALONE_H: usize = H + 1;
const FLOOR: usize = ALONE_H - 5;
const STAND: usize = FLOOR - 1;

/// The calls, as indices into [`setlist`].
pub const GIANT: usize = 0;
pub const GAP: usize = 1;
pub const MELT: usize = 2;
/// In a plan, `PICK + k` picks call `k` without calling it (as `tests/side_b.rs` writes it).
pub const PICK: usize = 10;

/// The set list for a room, with `cleared` Side B levels behind the player: Giant Steps and
/// fired up, and melting once Spike Alley (B2, its drip pipe) is cleared. Always empty.
pub fn setlist(cleared: usize) -> Vec<(SetCall, u32)> {
    let mut out = vec![(SetCall::Mode(Harmony::Coltrane), 0), (SetCall::Mode(Harmony::Quartal), 0)];
    if cleared >= 2 {
        out.push((SetCall::Melt, 0));
    }
    out
}

/// One piece of a room, and the way over it that the room's nuggets pay for.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Piece {
    /// A 6-tall wall: Giant Steps.
    Wall,
    /// A 9-wide gap: fired up.
    Gap,
    /// A drip pipe down to Han's head: melting.
    Pipe,
    /// The spike alley, over its roof (Giant Steps off a step) or through it (no music).
    Alley { over: bool },
    /// Down the Drain's fork: a deck to Giant Steps up onto, or the tunnel under it.
    Deck { over: bool },
    /// The full fork: leap the deck's gap (Giant Steps, fired up), drop through it (Giant
    /// Steps, melting), or under both drips (melting twice).
    Fork { route: Route },
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Route {
    Leap,
    Drop,
    Under,
}

/// Where a call is made, relative to the piece's anchor column, and which.
type Call = (i32, usize);

impl Piece {
    /// Columns it takes from its anchor.
    fn width(self) -> i32 {
        match self {
            Piece::Wall => 6,
            Piece::Gap => 9,
            Piece::Pipe => 10,
            Piece::Alley { .. } => 36,
            Piece::Deck { .. } => 55,
            Piece::Fork { .. } => 75,
        }
    }

    /// Its calls (relative to the anchor).
    fn calls(self) -> Vec<Call> {
        match self {
            // (A little earlier than the Side B levels' routes: a bar line can come late, and a
            // call holds long enough either way.)
            Piece::Wall => vec![(-22, GIANT)],
            Piece::Gap => vec![(-20, GAP)],
            Piece::Pipe => vec![(-26, MELT)],
            Piece::Alley { over: true } => vec![(-21, GIANT)],
            Piece::Deck { over: true } => vec![(-22, GIANT)],
            Piece::Alley { over: false } | Piece::Deck { over: false } => vec![],
            Piece::Fork { route: Route::Leap } => vec![(-20, GIANT), (26, GAP)],
            Piece::Fork { route: Route::Drop } => vec![(-20, GIANT), (31, MELT)],
            Piece::Fork { route: Route::Under } => vec![(-20, MELT), (32, MELT)],
        }
    }

    /// Where the music must be quiet from (no hold left over from the last call): its first
    /// call, or, going through with no music, where that call would be.
    fn quiet_from(self) -> i32 {
        match self {
            Piece::Alley { .. } => Piece::Alley { over: true }.calls()[0].0,
            Piece::Deck { .. } => Piece::Deck { over: true }.calls()[0].0,
            _ => self.calls()[0].0,
        }
    }

    /// The nuggets Han surely grabs on his way through (column, row), relative to the anchor:
    /// the ones he runs past on the floor. (Up on a deck Giant Steps may carry him over them,
    /// and in the alley he's hopping pits: those are a bonus, not a call the plan counts on.)
    fn nuggets_on_route(self) -> Vec<(i32, usize)> {
        match self {
            Piece::Deck { over: false } => vec![(30, STAND)],
            Piece::Fork { route: Route::Under } => vec![(33, STAND), (40, STAND)],
            _ => vec![],
        }
    }

    /// The row Han's on at `dx` from the anchor (for a nugget that pays for a call there).
    fn row_at(self, dx: i32) -> usize {
        match self {
            Piece::Fork { route: Route::Leap | Route::Drop } if (5..=43).contains(&dx) => 5,
            _ => STAND,
        }
    }

    /// Draw it at `o` on the grid (`grid[row][col]`, absolute columns).
    fn draw(self, g: &mut Grid, o: usize) {
        match self {
            Piece::Wall => g.fill(o..o + 6, FLOOR - 6..FLOOR, b'#'),
            Piece::Gap => g.fill(o..o + 9, FLOOR..ALONE_H, b'.'),
            Piece::Pipe => {
                g.fill(o..o + 10, 0..STAND, b'#');
                g.fill(o..o + 10, STAND..FLOOR, b'v');
            }
            Piece::Alley { .. } => {
                // A step to Giant Steps off, then the alley: a roof with spikes under it and
                // three spike pits in its floor to hop.
                g.set(o, STAND, b'#');
                let (c0, c1) = (o + 5, o + 35);
                g.fill(c0..c1 + 1, 6..7, b'#');
                g.fill(c0..c1 + 1, 7..8, b'v');
                for p in [5, 13, 22] {
                    g.fill(c0 + p..c0 + p + 3, STAND..FLOOR, b'^');
                }
                for (c, r) in [(15, 5), (25, 5), (22, STAND)] {
                    g.set(o + c, r, b'o');
                }
            }
            Piece::Deck { .. } => {
                g.set(o, STAND, b'#');
                g.fill(o + 5..o + 55, 6..10, b'#');
                for (c, r) in [(20, 5), (30, 5), (40, 5), (30, STAND)] {
                    g.set(o + c, r, b'o');
                }
            }
            Piece::Fork { .. } => {
                g.set(o, STAND, b'#');
                for c in o + 5..o + 75 {
                    if !(o + 43..=o + 50).contains(&c) {
                        g.fill(c..c + 1, 6..10, b'#');
                    }
                }
                g.set(o + 41, 5, b'#');
                for d in [o + 5, o + 53] {
                    g.fill(d..d + 10, 10..11, b'#');
                    g.fill(d..d + 10, STAND..FLOOR, b'v');
                }
                for (c, r) in [(20, 5), (25, 5), (33, STAND), (40, STAND)] {
                    g.set(o + c, r, b'o');
                }
            }
        }
    }
}

/// The room's body, before it goes on the canvas.
struct Grid(Vec<Vec<u8>>);

impl Grid {
    fn new(width: usize) -> Grid {
        Grid((0..ALONE_H).map(|r| vec![if r >= FLOOR { b'#' } else { b'.' }; width]).collect())
    }

    fn set(&mut self, col: usize, row: usize, ch: u8) {
        if let Some(c) = self.0.get_mut(row).and_then(|r| r.get_mut(col)) {
            *c = ch;
        }
    }

    fn get(&self, col: usize, row: usize) -> u8 {
        self.0.get(row).and_then(|r| r.get(col)).copied().unwrap_or(b'#')
    }

    fn fill(&mut self, cols: std::ops::Range<usize>, rows: std::ops::Range<usize>, ch: u8) {
        for r in rows {
            for c in cols.clone() {
                self.set(c, r, ch);
            }
        }
    }
}

/// A drawn Side B room: what the canvas got, and the plan that clears it (`col/k` pairs as
/// `tests/side_b.rs` plays them: `k` calls set-list call `k`, `PICK + k` only picks it).
pub struct Drawn {
    pub built: Built,
    pub solution: Vec<(f32, usize)>,
}

/// The pieces a room may use with `cleared` Side B levels behind the player.
fn pieces(d: &mut Dice, cleared: usize) -> Piece {
    let mut kinds = vec![Piece::Wall, Piece::Gap, Piece::Deck { over: d.chance(0.5) }];
    if cleared >= 2 {
        kinds.push(Piece::Pipe);
        kinds.push(Piece::Alley { over: d.chance(0.5) });
    }
    if cleared >= 3 {
        let route = d.pick(&[Route::Leap, Route::Drop, Route::Under]);
        kinds.push(Piece::Fork { route });
        kinds.push(Piece::Fork { route });
    }
    d.pick(&kinds)
}

/// Draw a Side B room on `c` (a canvas [`ALONE_H`] tall) for `band`, with `cleared` Side B
/// levels behind the player; returns it with its solution.
pub fn draw(c: &mut Canvas, d: &mut Dice, band: Band, cleared: usize) -> Drawn {
    debug_assert_eq!(c.height(), ALONE_H);
    let n = (2.0 + 2.0 * t(band)).round() as usize + d.int(0, 1) as usize;
    let lerp = |lo: f32, hi: f32| lo + (hi - lo) * t(band);
    // Columns between one call and the next piece's (a call holds ~34 columns of Han's run),
    // and between one piece's end and the next one's start: tighter as the band climbs.
    let call_gap = lerp(48.0, 32.0).round() as i32;
    let room_gap = lerp(16.0, 8.0).round() as i32;
    let slack = lerp(10.0, 3.0).round() as i32;
    let mut placed: Vec<(Piece, i32)> = Vec::new();
    // Han runs up from the entry pipe (and the band may still be on its intro): the first
    // call no earlier than this.
    let (mut last_call, mut end) = (ENTRY as i32 + 18 - call_gap, ENTRY as i32);
    let mut last_deck = false;
    while placed.len() < n {
        let mut p = pieces(d, cleared);
        // Two fork decks in a row would land Han's drop on the next deck's step: let the
        // music breathe with a plain piece between them.
        if last_deck && matches!(p, Piece::Deck { .. } | Piece::Fork { .. }) {
            p = if d.chance(0.5) { Piece::Wall } else { Piece::Gap };
        }
        let calls = p.calls();
        // Its first call a little past the last piece (Han's back on the floor by the nugget
        // that pays for it, 6 columns before), and far enough from the last call.
        let o = (end + room_gap.max(20) - p.quiet_from()).max(last_call + call_gap - p.quiet_from()) + d.int(0, slack);
        last_call = calls.last().map_or(o + p.quiet_from(), |c| o + c.0);
        end = o + p.width();
        last_deck = matches!(p, Piece::Deck { .. } | Piece::Fork { .. });
        placed.push((p, o));
    }
    let width = (end + 14) as usize;
    let mut g = Grid::new(width);
    for &(p, o) in &placed {
        p.draw(&mut g, o as usize);
    }

    // The nuggets on Han's way (each one more of the picked call), and the calls, in order;
    // a call nothing pays for yet gets a nugget a few columns before it.
    let mut supply: Vec<f32> = Vec::new();
    let mut calls: Vec<(f32, usize)> = Vec::new();
    let mut paid: Vec<(f32, usize)> = Vec::new(); // (nugget column, call it pays for)
    let mut events: Vec<(i32, bool, usize, Piece, i32)> = Vec::new(); // (col, is_call, k, piece, o)
    for &(p, o) in &placed {
        for (dx, _) in p.nuggets_on_route() {
            events.push((o + dx, false, 0, p, o));
        }
        for (dx, k) in p.calls() {
            events.push((o + dx, true, k, p, o));
        }
    }
    events.sort_by_key(|e| (e.0, e.1));
    for (col, is_call, k, p, o) in events {
        if !is_call {
            supply.push(col as f32);
            continue;
        }
        // The oldest nugget at least three columns back (time to pick before Han's there), that
        // a pick can come before: not right after a call or a nugget for another call (Han
        // grabs a nugget a column before he's at it).
        let have = supply.iter().position(|&n| {
            let clash = |c: f32, kind: usize| c < n && c > n - 2.0 && kind != k;
            n <= col as f32 - 3.0 && !calls.iter().any(|&(c, kind)| clash(c, kind)) && !paid.iter().any(|&(c, kind)| clash(c, kind))
        });
        let n = match have {
            Some(i) => supply.remove(i),
            None => {
                let dx = col - o - 6;
                let (mut nc, row) = (col - 6, p.row_at(dx));
                while g.get(nc as usize, row) != b'.' || g.get(nc as usize, row + 1) == b'.' && row == STAND {
                    nc -= 1;
                }
                g.set(nc as usize, row, b'o');
                nc as f32
            }
        };
        paid.push((n, k));
        calls.push((col as f32, k));
    }

    // The plan: each call, and a pick before each nugget that pays for a call not under the
    // cursor (the cursor moves with every pick and call).
    let mut ev: Vec<(f32, u8, usize)> = calls.iter().map(|&(c, k)| (c, 1, k)).collect();
    for &(n, k) in &paid {
        let before = calls.iter().filter(|c| c.0 < n).map(|c| c.0).fold(f32::NEG_INFINITY, f32::max);
        let prior_nug = paid.iter().filter(|p| p.0 < n).map(|p| p.0).fold(f32::NEG_INFINITY, f32::max);
        let at = (n - 2.5).max(before + 0.3).max(prior_nug + 0.5);
        ev.push((at, 0, k));
    }
    ev.sort_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)));
    let mut cur = GIANT;
    let mut solution = Vec::new();
    for (c, call, k) in ev {
        if call == 1 {
            solution.push((c, k));
            cur = k;
        } else if k != cur {
            solution.push((c, PICK + k));
            cur = k;
        }
    }

    for col in c.width()..width {
        c.column(|r| g.get(col, r));
    }
    // The rewinds it takes to learn the room: one a piece (a fork, two: its way in and its
    // way on), as a grease chute expects its splats.
    let rewinds = placed.iter().map(|(p, _)| if matches!(p, Piece::Fork { .. }) { 2 } else { 1 }).sum();
    c.expect_deaths(rewinds);
    Drawn { built: Built { hint: None, checkpoint: None }, solution }
}

/// The template's build function (`templates::TEMPLATES`): the room, without its solution.
pub fn build(c: &mut Canvas, d: &mut Dice, band: Band, dress: &Dressing) -> Built {
    draw(c, d, band, dress.side_b as usize).built
}
