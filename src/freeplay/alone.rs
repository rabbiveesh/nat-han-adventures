//! Side B rooms in free play: Han goes on alone while Nat waits in the pipe, and the player
//! calls the band, hits and works the record, as on Side B (`game::sideb`). They're built from
//! Side B's building blocks ([`super::blocks`]): only those of the Side B levels the player has
//! cleared, at most three calls' worth (the room's set list).
//!
//! The validator can't prove these (it plays Nat, not Han alone with a band), so every room is
//! built *with* its solution: each block comes with the plan that gets Han through it, and the
//! builder spaces them so one call's obstacle is behind Han before the next call goes in.
//! [`draw`] returns that plan, and `tests/side_b.rs` plays it on many seeds.

use std::collections::BTreeSet;

use super::blocks::{Beat, Builder, Call, Deck, Low};
use super::canvas::{Canvas, ENTRY};
use super::dice::{Dice, t};
use super::templates::{Built, Dressing};
use crate::adapt::Band;
use crate::audio::Harmony;
use crate::game::sideb::SetCall;
use crate::level::{Level, Tile, Topic};

/// Rows in a Side B room: room above its floor for climbs and towers (the floor, as in every
/// room, 5 rows from the bottom).
pub const ALONE_H: usize = 34;

/// The set list a room's level wants: the calls its gates and drips ask for, made up to three
/// (Giant Steps, fired up, melting first), in the set list's order.
pub fn setlist_for(level: &Level) -> Vec<(SetCall, u32)> {
    calls_for(level).into_iter().map(|c| (set_call(c), 0)).collect()
}

/// [`setlist_for`]'s calls.
pub fn calls_for(level: &Level) -> Vec<Call> {
    let mut calls = BTreeSet::new();
    for g in &level.gates {
        if let Some(c) = call_for(g.topic.word()) {
            calls.insert(c);
        }
    }
    if level.tiles.iter().any(|&t| t == Tile::SpikesDown) {
        calls.insert(Call::Melt);
    }
    fill(calls)
}

fn call_for(gate: &str) -> Option<Call> {
    match gate {
        w if w == Topic::Giant.word() => Some(Call::Giant),
        w if w == Topic::Gap.word() => Some(Call::Gap),
        w if w == Topic::Trapeze.word() => Some(Call::Waltz),
        w if w == Topic::Shaft.word() => Some(Call::Nervous),
        _ => None,
    }
}

/// Up to three calls, in the set list's order.
fn fill(mut calls: BTreeSet<Call>) -> Vec<Call> {
    for c in [Call::Giant, Call::Gap, Call::Melt, Call::Waltz, Call::Nervous] {
        if calls.len() >= 3 {
            break;
        }
        calls.insert(c);
    }
    calls.into_iter().collect()
}

pub fn set_call(c: Call) -> SetCall {
    match c {
        Call::Giant => SetCall::Mode(Harmony::Coltrane),
        Call::Gap => SetCall::Mode(Harmony::Quartal),
        Call::Waltz => SetCall::Mode(Harmony::Waltz),
        Call::Nervous => SetCall::Nervous,
        Call::Melt => SetCall::Melt,
    }
}

/// The blocks a room may use.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Piece {
    Pit,
    Wall,
    Drips,
    ClimbPit,
    ShelfFork,
    SpringStep,
    SpringboardDrips,
    SpringboardSewage,
    Doors,
    Drum,
    DeckWalls,
    ScratchBack,
    FerryPit,
    DeckPit,
    Shaft,
    FaceOverDrips,
    ShaftPit,
    Tower,
    Trapeze,
    DeckTrapeze,
    DripsHeld,
}

impl Piece {
    /// The Side B levels to clear first (where the player met it), and every call it draws
    /// (its gates: the calls of all its ways).
    fn needs(self) -> (usize, &'static [Call]) {
        use Call::*;
        match self {
            Piece::Pit => (1, &[Gap]),
            Piece::Wall => (1, &[Giant]),
            Piece::Drips => (1, &[Melt]),
            Piece::ClimbPit => (1, &[Giant, Gap]),
            Piece::ShelfFork => (1, &[Giant, Melt]),
            Piece::SpringStep => (1, &[]),
            Piece::SpringboardDrips => (2, &[Melt]),
            Piece::SpringboardSewage => (2, &[]),
            Piece::Doors => (3, &[]),
            Piece::Drum => (3, &[]),
            Piece::DeckWalls => (3, &[Giant, Melt]),
            Piece::ScratchBack => (3, &[]),
            Piece::Trapeze => (4, &[Waltz]),
            Piece::FerryPit => (5, &[Gap]),
            Piece::DeckPit => (5, &[Gap, Melt]),
            Piece::Shaft => (6, &[Nervous]),
            Piece::FaceOverDrips => (6, &[Nervous, Melt]),
            Piece::ShaftPit => (6, &[Nervous, Gap]),
            Piece::Tower => (6, &[Nervous]),
            Piece::DeckTrapeze => (7, &[Waltz]),
            Piece::DripsHeld => (7, &[Melt]),
        }
    }

    const ALL: [Piece; 21] = [
        Piece::Pit,
        Piece::Wall,
        Piece::Drips,
        Piece::ClimbPit,
        Piece::ShelfFork,
        Piece::SpringStep,
        Piece::SpringboardDrips,
        Piece::SpringboardSewage,
        Piece::Doors,
        Piece::Drum,
        Piece::DeckWalls,
        Piece::ScratchBack,
        Piece::Trapeze,
        Piece::FerryPit,
        Piece::DeckPit,
        Piece::Shaft,
        Piece::FaceOverDrips,
        Piece::ShaftPit,
        Piece::Tower,
        Piece::DeckTrapeze,
        Piece::DripsHeld,
    ];

    /// How far it takes the floor down (positive) or up (negative), at most.
    fn dy(self) -> (i32, i32) {
        match self {
            Piece::ClimbPit => (-6, 0),
            Piece::SpringStep => (-4, 0),
            Piece::DeckWalls | Piece::DeckPit | Piece::DeckTrapeze => (0, 6),
            Piece::ScratchBack => (0, 7),
            Piece::Shaft | Piece::ShaftPit => (-9, 0),
            Piece::Tower => (-9, 0),
            Piece::Trapeze => (0, 2),
            _ => (0, 0),
        }
    }

    fn draw(self, b: &mut Builder, d: &mut Dice) {
        let beat = if d.chance(0.5) { Beat::Red } else { Beat::Blue };
        match self {
            Piece::Pit => b.pit(9, d.chance(0.5)),
            Piece::Wall => b.wall(6, 3),
            Piece::Drips => b.drips(d.int(10, 14), 3),
            Piece::ClimbPit => b.climb_then_pit(d.int(4, 6)),
            Piece::ShelfFork => {
                let n = d.int(28, 40);
                b.shelf_fork(n, &[n / 2 - 1], &[4, n / 2 + 4, n - 6]);
            }
            Piece::SpringStep => b.spring_step(d.int(6, 10)),
            Piece::SpringboardDrips => b.springboard(beat, d.int(18, 24), Low::Drips),
            Piece::SpringboardSewage => b.springboard(beat, d.int(16, 22), Low::Sewage),
            Piece::Doors => {
                b.door(beat, 6);
                b.run(6, &[3]);
                b.bridge(beat.other(), 9);
            }
            Piece::Drum => b.drum(beat, d.int(2, 4), 9, 6),
            Piece::DeckWalls => b.deck_fork(Deck::Walls(d.int(1, 2)), true),
            Piece::ScratchBack => b.scratch_back(7),
            Piece::Trapeze => b.trapeze(d.int(0, 2), 17),
            Piece::FerryPit => b.ferry_then_pit(8),
            Piece::DeckPit => b.deck_fork(Deck::Pit, true),
            Piece::Shaft => b.shaft(9, d.int(6, 10)),
            Piece::FaceOverDrips => b.face_over_drips(14, 9),
            Piece::ShaftPit => b.shaft_then_pit(9, 4),
            Piece::Tower => b.tower(9),
            Piece::DeckTrapeze => b.deck_fork(Deck::Trapeze, false),
            Piece::DripsHeld => b.drips_held(d.int(10, 12)),
        }
    }
}

/// A drawn Side B room: what the canvas got, and the plan that clears it (`parse_plan`'s
/// text, columns the room's own).
pub struct Drawn {
    pub built: Built,
    pub solution: String,
}

/// The room's floor (the canvas frame's) and the highest a block may take it.
fn floor() -> i32 {
    Canvas::with_height(ALONE_H).floor() as i32
}
const HIGHEST: i32 = 13;

/// Draw a Side B room on `c` (a canvas [`ALONE_H`] tall) for `band`, with `cleared` Side B
/// levels behind the player; returns it with its solution.
pub fn draw(c: &mut Canvas, d: &mut Dice, band: Band, cleared: usize) -> Drawn {
    debug_assert_eq!(c.height(), ALONE_H);
    let f = floor();
    let picks: Vec<u32> = (0..64).map(|_| d.int(0, 1 << 20) as u32).collect();
    let mut b = Builder::new(ALONE_H, f, ENTRY as i32, picks);
    b.auto_space = true;
    b.checkpoints = false;
    let n = (3.0 + 3.0 * t(band)).round() as usize + d.int(0, 1) as usize;
    let room = (10.0 - 6.0 * t(band)).round() as i32;
    b.run(room.max(6) + 6, &[4]);
    let open: Vec<Piece> = Piece::ALL.into_iter().filter(|p| p.needs().0 <= cleared.max(1)).collect();
    let mut placed = 0;
    let mut tries = 0;
    while placed < n && tries < 60 {
        tries += 1;
        let p = d.pick(&open);
        let mut next = b.clone();
        // Room to go down: up a springboard first. Room to go up: down some steps first.
        let (up, down) = p.dy();
        while next.top + down > f + 2 {
            next.spring_step(4);
            next.run(4, &[]);
        }
        while next.top + up < HIGHEST {
            next.step(3, 4);
        }
        p.draw(&mut next, d);
        if next.min_top < HIGHEST - 3 || next.max_top > f + 2 || drawn_calls(&next).len() > 3 {
            continue;
        }
        next.run(room, &[room / 2]);
        b = next;
        placed += 1;
    }
    // Back to the room's floor for the exit.
    while b.top < f {
        let dy = (f - b.top).min(3);
        b.step(dy, 4);
    }
    while b.top > f {
        let dy = (b.top - f).min(2);
        b.step(-dy, 5);
    }
    b.run(6, &[3]);

    let setlist = fill(drawn_calls(&b));
    let solution = b.plan(&setlist);
    for l in b.header() {
        c.header(l);
    }
    let rows = b.rows(b.x);
    for col in ENTRY..b.x as usize {
        c.column(|r| rows[r].as_bytes()[col]);
    }
    // The rewinds it takes to learn the room: one a call or hit, about.
    c.expect_deaths((placed as u32 + 1).min(12));
    Drawn { built: Built { hint: None, checkpoint: None }, solution }
}

/// The calls a builder's gates and drips ask for (as [`setlist_for`] reads them off the level).
fn drawn_calls(b: &Builder) -> BTreeSet<Call> {
    let mut calls = BTreeSet::new();
    for l in b.header() {
        if let Some(rest) = l.strip_prefix("gate: ")
            && let Some(c) = call_for(rest.split_whitespace().next().unwrap_or(""))
        {
            calls.insert(c);
        }
    }
    if b.rows(b.x).iter().any(|r| r.contains('v')) {
        calls.insert(Call::Melt);
    }
    calls
}

/// The template's build function (`templates::TEMPLATES`): the room, without its solution.
pub fn build(c: &mut Canvas, d: &mut Dice, band: Band, dress: &Dressing) -> Built {
    draw(c, d, band, dress.side_b as usize).built
}
