//! Room templates: one hand-shaped room per [`Skill`], with randomized parameters scaled by
//! the band (1 gentlest ..= 10 hardest). A template only draws; the generator validates what
//! it drew and re-rolls on failure, so templates may be bold.
//!
//! # Adding templates
//! [`TEMPLATES`] is the registry. A new kind of room (e.g. Han's buddy rooms: buddy ledges,
//! shield rows, chain chasms, buddy raft puzzles) is a [`BuildFn`] plus an entry naming the
//! [`Skill`] it exercises (add the skill to `crate::adapt::Skill` first) and the story level
//! that must be unlocked before free play serves it. Several templates may share a skill: the
//! generator picks among them by seed. Its gates are checked by `crate::level::validate` like
//! any other room's, so a new gate kind needs only the validator to know it.

use super::canvas::*;
use super::dice::{Dice, t};
use crate::adapt::{Band, Skill};
use crate::level::validate::{WaltzRow, waltz_row_timing};
use crate::level::{PlatformKind, Topic};

/// Room dressing from the assist levers (`crate::adapt::AssistLevers`), built into the room.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Dressing {
    /// Extra nuggets on a long gap's nugget line (the band fires up more easily).
    pub extra_nuggets: u8,
}

/// What a template tells the generator about the room it drew.
#[derive(Debug, Clone, Default)]
pub struct Built {
    /// Han's hint for the room's mechanic (said near the start when the run first meets it or
    /// when assists ask for it), with what it teaches.
    pub hint: Option<(&'static [Topic], &'static str)>,
    /// A safe spot (column, standing row) for an extra checkpoint mid-room.
    pub checkpoint: Option<(usize, usize)>,
}

pub type BuildFn = fn(&mut Canvas, &mut Dice, Band, &Dressing) -> Built;

pub struct Template {
    pub name: &'static str,
    pub skill: Skill,
    /// Story levels `0..=unlock` must be unlocked (`crate::save::Progress::unlocked`) before
    /// free play serves this room: the mechanic has been taught.
    pub unlock: usize,
    pub build: BuildFn,
}

/// Every template.
pub const TEMPLATES: &[Template] = &[
    Template { name: "jump gauntlet", skill: Skill::Precision, unlock: 0, build: gauntlet },
    Template { name: "flies and sprays", skill: Skill::HazardTiming, unlock: 0, build: hazards },
    Template { name: "moving platforms", skill: Skill::MovingPlatforms, unlock: 0, build: platforms },
    Template { name: "giant wall", skill: Skill::GiantSteps, unlock: 0, build: giant_wall },
    Template { name: "long gap", skill: Skill::FiredUp, unlock: 2, build: long_gap },
    Template { name: "waltz row", skill: Skill::Waltz, unlock: 6, build: waltz_row },
    Template { name: "stain pit", skill: Skill::Stains, unlock: 2, build: stain_pit },
    Template { name: "grease chute", skill: Skill::Grease, unlock: 5, build: grease_chute },
];

/// The templates for a skill.
pub fn for_skill(skill: Skill) -> impl Iterator<Item = &'static Template> {
    TEMPLATES.iter().filter(move |t| t.skill == skill)
}

/// Skills free play can serve with `unlocked` story levels playable.
pub fn unlocked_skills(unlocked_levels: usize) -> Vec<Skill> {
    let mut out: Vec<Skill> = Vec::new();
    for t in TEMPLATES {
        if t.unlock < unlocked_levels.max(1) && !out.contains(&t.skill) {
            out.push(t.skill);
        }
    }
    out
}

fn lerp(band: Band, lo: f32, hi: f32) -> f32 {
    lo + (hi - lo) * t(band)
}

fn lerpi(band: Band, lo: f32, hi: f32) -> usize {
    lerp(band, lo, hi).round().max(0.0) as usize
}

/// Nuggets in an arc over columns `c0..c0+n` peaking `h` rows above `stand`.
fn arc(c: &mut Canvas, c0: usize, n: usize, stand: usize, h: usize) {
    for k in 0..n {
        let x = (k as f32 + 0.5) / n as f32 * 2.0 - 1.0;
        let up = ((1.0 - x * x) * h as f32).round() as usize;
        c.nugget(c0 + k, stand.saturating_sub(up.max(1)));
    }
}

// ─── Precision: jump gauntlets ───────────────────────────────────────────────

/// Gaps (2 → 6 tiles: the widest need a toot), steps up and down, pillars and floating shelves;
/// landings shrink from 5 tiles to 1-2 as the band rises.
fn gauntlet(c: &mut Canvas, d: &mut Dice, band: Band, _: &Dressing) -> Built {
    let tb = t(band);
    c.ground(3, FLOOR);
    let mut top = FLOOR;
    let obstacles = 3 + d.scaled(band, 0.0, 3.0, 1) as usize;
    let mut checkpoint = None;
    for i in 0..obstacles {
        if i == obstacles / 2 {
            checkpoint = Some((c.width() + 1, top - 1));
            c.ground(3, top);
        }
        match d.int(0, if band >= 3 { 3 } else { 1 }) {
            // A gap, maybe to a ledge up or down.
            0 | 1 => {
                let g = (d.scaled(band, 2.0, 4.6, 1) as usize).clamp(2, 6);
                let max_up = if g >= 5 { 1 } else { 2 + (tb > 0.5) as i32 };
                let next = (top as i32 - d.int(-2, max_up)).clamp(FLOOR as i32 - 4, FLOOR as i32) as usize;
                let x = c.width();
                c.pit(g);
                arc(c, x, g, top.min(next) - 1, 2);
                let land = (lerpi(band, 5.0, 2.0) + d.int(0, 1) as usize).max(2);
                c.ground(land, next);
                top = next;
            }
            // Pillars: one-to-three-wide columns at varying heights.
            2 => {
                for _ in 0..2 + d.scaled(band, 0.0, 2.0, 0) as usize {
                    let g = d.int(2, 2 + (tb * 2.0).round() as i32) as usize;
                    c.pit(g);
                    top = (top as i32 - d.int(-1, 1)).clamp(FLOOR as i32 - 4, FLOOR as i32) as usize;
                    let w = (lerpi(band, 3.0, 1.0) + d.int(0, 1) as usize).max(1);
                    let x = c.width();
                    c.ground(w, top);
                    c.nugget(x, top - 1);
                }
                c.ground(2, top);
            }
            // Floating shelves (`=`) over a pit.
            _ => {
                let shelves = 2 + d.scaled(band, 0.0, 1.0, 1) as usize;
                let w = lerpi(band, 3.0, 1.0).max(1);
                let mut row = top;
                for _ in 0..shelves {
                    let g = d.int(2, 2 + (tb * 2.0).round() as i32) as usize;
                    c.pit(g);
                    row = (row as i32 - d.int(-1, 2)).clamp(FLOOR as i32 - 4, FLOOR as i32) as usize;
                    let x = c.width();
                    c.pit(w);
                    for k in 0..w {
                        c.set(x + k, row, b'=');
                    }
                    c.nugget(x, row - 1);
                }
                let g = d.int(2, 3) as usize;
                c.pit(g);
                top = (row as i32 + d.int(0, 1)).clamp(FLOOR as i32 - 4, FLOOR as i32) as usize;
                c.ground(3, top);
            }
        }
    }
    // Back down to the floor for the exit.
    if top != FLOOR {
        c.ground(2, top);
    }
    c.ground(3, FLOOR);
    Built { hint: None, checkpoint }
}

// ─── HazardTiming: flies and sprays ──────────────────────────────────────────

/// Spray cans (single, then pairs) and fly swarms, hovering lower and over gaps as the band
/// rises; 2 → 7 hazards.
fn hazards(c: &mut Canvas, d: &mut Dice, band: Band, _: &Dressing) -> Built {
    c.ground(4, FLOOR);
    let n = 2 + d.scaled(band, 0.0, 4.0, 1) as usize;
    let (mut flies, mut sprays) = (false, false);
    let mut checkpoint = None;
    for i in 0..n {
        if i == n / 2 {
            checkpoint = Some((c.width() + 1, STAND));
            c.ground(3, FLOOR);
        }
        let gap = lerpi(band, 4.0, 2.0) + d.int(0, 1) as usize;
        match d.int(0, if band >= 4 { 3 } else { 2 }) {
            0 => {
                let x = c.width();
                c.ground(1, FLOOR);
                c.set(x, STAND, b'S');
                sprays = true;
            }
            1 if band >= 3 => {
                let x = c.width();
                c.ground(2, FLOOR);
                c.set(x, STAND, b'S');
                c.set(x + 1, STAND, b'S');
                sprays = true;
            }
            // A swarm over the path: walk under it, don't jump into it.
            1 | 2 => {
                let x = c.width();
                c.ground(3, FLOOR);
                let h = if d.chance(lerp(band, 0.2, 0.7)) { 2 } else { 3 };
                c.set(x + 1, STAND - h, b'F');
                flies = true;
            }
            // A swarm over a gap: a flat jump under it.
            _ => {
                let g = d.int(2, 3) as usize;
                let x = c.width();
                c.pit(g);
                c.set(x + g / 2, STAND - 4, b'F');
                flies = true;
            }
        }
        c.ground(gap, FLOOR);
    }
    c.ground(3, FLOOR);
    let hint: Option<(&'static [Topic], &'static str)> = match (flies, sprays) {
        (true, true) => Some((&[Topic::Fly, Topic::Spray], "Flies and sprays! Wait for the pssht, then go.")),
        (true, false) => Some((&[Topic::Fly], "Fly swarms! Duck under 'em, Nat. Don't jump in.")),
        (false, true) => Some((&[Topic::Spray], "Spray cans! Wait till the pssht stops, then go.")),
        _ => None,
    };
    Built { hint, checkpoint }
}

// ─── MovingPlatforms ─────────────────────────────────────────────────────────

/// Pits (6 → 13 wide, bottomless or sewage) crossed on rolls and ducks, and plunger lifts up to
/// high ledges; platforms shrink (3 → 1 tiles) and speed up (5 s → 2.6 s round trips).
fn platforms(c: &mut Canvas, d: &mut Dice, band: Band, _: &Dressing) -> Built {
    c.ground(3, FLOOR);
    let sections = 1 + d.scaled(band, 0.0, 2.0, 1) as usize;
    let mut checkpoint = None;
    for i in 0..sections.min(3) {
        if i == 1 {
            checkpoint = Some((c.width() + 1, STAND));
        }
        let period = d.scaled_f(band, 5.0, 2.6, 0.1);
        let pw = lerpi(band, 3.0, 1.0).max(1) + d.int(0, 1) as usize;
        if d.chance(0.65) {
            // Across a pit.
            let w = d.scaled(band, 6.0, 11.0, 2) as usize;
            let x = c.width();
            let sewage = d.chance(0.5);
            if sewage { c.pool(w) } else { c.pit(w) };
            let kind = if sewage { PlatformKind::Duck } else { PlatformKind::Tp };
            if w <= 8 {
                c.platform(x, FLOOR, pw, (w - pw) as f32, 0.0, period, d.u(), kind);
            } else {
                let half = w / 2;
                c.platform(x, FLOOR, pw, (half - pw) as f32, 0.0, period, d.u(), kind);
                c.platform(x + half, FLOOR, pw, (w - half - pw) as f32, 0.0, period, d.u(), kind);
            }
            arc(c, x + 1, w - 2, STAND - 1, 1);
            c.ground(lerpi(band, 4.0, 2.0) + 1, FLOOR);
        } else {
            // A plunger lift up to a high ledge, then drop back down.
            let h = d.int(4, 6) as usize;
            let x = c.width();
            c.pit(pw + 1);
            c.platform(x, FLOOR, pw, 0.0, h as f32, period, d.u(), PlatformKind::Plunger);
            let ledge = c.width();
            c.ground(3 + d.int(0, 2) as usize, FLOOR - h);
            for k in 0..3 {
                c.nugget(ledge + k, FLOOR - h - 1);
            }
            c.ground(2, FLOOR - h / 2);
            c.ground(2, FLOOR);
        }
    }
    c.ground(3, FLOOR);
    Built { hint: Some((&[Topic::Platform], "Ride the rolls, Nat. Hop on, hop off!")), checkpoint }
}

// ─── GiantSteps: a giant wall with a runway ──────────────────────────────────

/// A runway (12 → 8 tiles) and a 6-tile wall (2 → 5 thick); higher bands add jumps on and after
/// the wall.
fn giant_wall(c: &mut Canvas, d: &mut Dice, band: Band, _: &Dressing) -> Built {
    c.ground(2, FLOOR);
    let checkpoint = Some((c.width(), STAND));
    let runway = (lerpi(band, 12.0, 8.0) + d.int(0, 1) as usize).max(9);
    c.ground(runway, FLOOR);
    let top = FLOOR - 6;
    let x = c.width();
    let thick = d.int(2, 2 + (t(band) * 3.0).round() as i32) as usize;
    c.ground(thick, top);
    for k in 0..thick {
        c.nugget(x + k, top - 1);
    }
    if band >= 5 {
        // Hop along the top.
        let g = d.int(2, 3) as usize;
        let x = c.width();
        c.pit(g);
        arc(c, x, g, top - 1, 2);
        c.ground(lerpi(band, 3.0, 2.0), top);
    }
    // Down the far side in steps.
    c.ground(2, top + 3);
    if band >= 7 {
        c.pit(d.int(2, 3) as usize);
    }
    c.ground(4, FLOOR);
    Built { hint: Some((&[Topic::Giant], "Big wall! Toot 5 times: the band plays Giant Steps!")), checkpoint }
}

// ─── FiredUp: a long gap with a nugget line ──────────────────────────────────

/// A nugget line (4+, spaced 2 → 3) on a flat run-up, an 11-tile bottomless gap, a landing
/// (6 → 3 tiles); higher bands add a jump after it.
fn long_gap(c: &mut Canvas, d: &mut Dice, band: Band, dress: &Dressing) -> Built {
    c.ground(2, FLOOR);
    let checkpoint = Some((c.width(), STAND));
    c.ground(2, FLOOR);
    let line = 4 + dress.extra_nuggets as usize + (band <= 3) as usize;
    let spacing = if band >= 7 { 3 } else { 2 };
    let x = c.width();
    let run = line * spacing + 5;
    c.ground(run, FLOOR);
    for k in 0..line {
        c.nugget(x + 1 + k * spacing, STAND);
    }
    let gap = 11;
    let g0 = c.width();
    c.pit(gap);
    for k in 3..gap - 3 {
        c.nugget(g0 + k, STAND - 2);
    }
    c.ground((lerpi(band, 6.0, 3.0) + d.int(0, 1) as usize).max(3), FLOOR);
    if band >= 6 {
        c.pit(d.int(2, 3) as usize);
        c.ground(3, FLOOR);
    }
    c.ground(2, FLOOR);
    Built { hint: Some((&[Topic::Gap], "Big gap! Grab nuggets fast, band fires up. Then RUN!")), checkpoint }
}

// ─── Waltz: a waltz row ──────────────────────────────────────────────────────

/// Spray cans a waltz needs, by length: every row the validator accepts (shortest first). The
/// cans stand on the floor you walk on (deadly while they fire, like their jets; the can is
/// no wider than its jet), so a row's danger zone, and so the lengths, are the same as the
/// campaign's grated tunnels: 20 or 21 cans.
pub fn waltz_cans() -> Vec<usize> {
    (crate::level::validate::WALTZ_ROW_MIN..48)
        .filter(|&n| {
            waltz_row_timing(&WaltzRow { row: STAND as i32, c0: 0, c1: n as i32 - 1, grated: false }).is_empty()
        })
        .collect()
}

/// A runway (9 → 7 tiles) and a row of adjacent cans on the floor, no grating, under a low
/// roof (so nobody hops over the cans in the off-beats and hangs above them while they fire):
/// longer rows (the valid range, shortest to longest) as the band rises.
fn waltz_row(c: &mut Canvas, d: &mut Dice, band: Band, _: &Dressing) -> Built {
    use std::sync::OnceLock;
    static CANS: OnceLock<Vec<usize>> = OnceLock::new();
    let cans = CANS.get_or_init(waltz_cans);
    c.ground(2, FLOOR);
    let checkpoint = Some((c.width(), STAND));
    c.ground(lerpi(band, 9.0, 7.0) + d.int(0, 1) as usize, FLOOR);
    let k = ((cans.len() - 1) as f32 * t(band) * 0.8).round() as usize + d.int(0, 1) as usize;
    let n = cans[k.min(cans.len() - 1)];
    for _ in 0..n {
        c.column(|r| match r {
            r if r <= PIPE_ROOF => b'#',
            STAND => b'S',
            r if r >= FLOOR => b'#',
            _ => b'.',
        });
    }
    // The prize past the row.
    let past = c.width();
    c.ground(4, FLOOR);
    for k in 0..3 {
        c.nugget(past + k, STAND);
    }
    Built { hint: Some((&[Topic::Waltz], "Hop-hop-hop, even beats: the band waltzes! GO!")), checkpoint }
}

// ─── Stains: a stain pit ─────────────────────────────────────────────────────

/// A spike pit too wide to jump in any mode (14 → 19 tiles: 1 → 3+ splats to cross), nuggets
/// over it; higher bands add a jump after it.
fn stain_pit(c: &mut Canvas, d: &mut Dice, band: Band, _: &Dressing) -> Built {
    c.ground(4, FLOOR);
    let w = d.scaled(band, 14.0, 18.0, 1) as usize;
    let x = c.width();
    for _ in 0..w {
        c.column(|r| if r == FLOOR { b'^' } else if r > FLOOR { b'#' } else { b'.' });
    }
    for k in (2..w - 1).step_by(3) {
        c.nugget(x + k, STAND - 3);
    }
    c.ground(4, FLOOR);
    if band >= 6 {
        c.pit(d.int(2, 3) as usize);
        c.ground(3, FLOOR);
    }
    Built { hint: Some((&[Topic::Stain], "Too wide to jump! Splat on spikes: your stain's a step!")), checkpoint: None }
}

// ─── Grease: a chute ─────────────────────────────────────────────────────────

/// A sunken grease run (15 → 20 tiles) ending in spikes (one per splat the band needs to get
/// nervous); the way out is a jump off the grease, which only sweaty grip allows.
fn grease_chute(c: &mut Canvas, d: &mut Dice, band: Band, _: &Dressing) -> Built {
    c.ground(4, FLOOR);
    let w = d.scaled(band, 15.0, 20.0, 1) as usize;
    let spikes = crate::audio::director::NERVOUS_DEATHS as usize;
    let x = c.width();
    for k in 0..w + spikes {
        let spike = k >= w;
        c.column(move |r| match r {
            r if r == FLOOR + 2 => b'_',
            r if r == FLOOR + 1 && spike => b'^',
            r if r > FLOOR + 2 => b'#',
            _ => b'.',
        });
    }
    for k in (2..w - 1).step_by(3) {
        c.nugget(x + k, FLOOR + 1);
    }
    c.ground(5, FLOOR);
    Built {
        hint: Some((&[Topic::Grease, Topic::Grip], "Grease! Can't stop or jump. Splat a few: band gets nervous.")),
        checkpoint: None,
    }
}
