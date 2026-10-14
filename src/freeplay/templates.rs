//! Room templates: one hand-shaped room per [`Skill`], with randomized parameters scaled by
//! the band (1 gentlest ..= 10 hardest). A template only draws; the generator validates what
//! it drew and re-rolls on failure, so templates may be bold.
//!
//! # Adding templates
//! [`TEMPLATES`] is the registry. A new kind of room is a [`BuildFn`] plus an entry naming the
//! [`Skill`] it exercises (add the skill to `crate::adapt::Skill` first) and the story level
//! that must be unlocked before free play serves it. Several templates may share a skill (Han's
//! four gates are all [`Skill::Buddy`]): the generator picks among the unlocked ones by seed. Its gates are checked by `crate::level::validate` like
//! any other room's, so a new gate kind needs only the validator to know it.

use super::alone::{self, ALONE_H};
use super::canvas::*;
use super::dice::{Dice, t};
use crate::adapt::{Band, Skill};
use crate::level::validate::{BUDDY_POOL_MIN, CHASM_MIN};
use crate::level::{PlatformKind, Topic};

/// Room dressing from the assist levers (`crate::adapt::AssistLevers`), built into the room.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Dressing {
    /// Extra nuggets on a long gap's nugget line (the band fires up more easily).
    pub extra_nuggets: u8,
    /// Side B levels the player has cleared (which of Side B's pieces a Side B room may use).
    pub side_b: u8,
}

/// What the player has unlocked, which decides the rooms free play may serve: story levels
/// playable (`crate::save::Progress::unlocked`) and Side B levels cleared.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Unlocks {
    pub levels: usize,
    pub side_b: usize,
}

impl From<usize> for Unlocks {
    /// Story levels only.
    fn from(levels: usize) -> Unlocks {
        Unlocks { levels, side_b: 0 }
    }
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
    /// Side B levels that must be cleared first; a template that asks for any is a Side B room
    /// (`super::alone`): Han goes on alone, the player calls the band, and it isn't validated.
    pub side_b: usize,
    /// Rows in its canvas: [`H`], or more for a tall room ([`CLIMB_H`] for a climb,
    /// [`CHASM_H`] for a fired-up chasm's headroom).
    pub height: usize,
    pub build: BuildFn,
}

/// Every template.
pub const TEMPLATES: &[Template] = &[
    Template { name: "jump gauntlet", skill: Skill::Precision, unlock: 0, side_b: 0, height: H, build: gauntlet },
    Template { name: "shelf climb", skill: Skill::Precision, unlock: 1, side_b: 0, height: CLIMB_H, build: shelf_climb },
    Template { name: "flies and sprays", skill: Skill::HazardTiming, unlock: 0, side_b: 0, height: H, build: hazards },
    Template { name: "moving platforms", skill: Skill::MovingPlatforms, unlock: 0, side_b: 0, height: H, build: platforms },
    Template { name: "giant wall", skill: Skill::GiantSteps, unlock: 0, side_b: 0, height: H, build: giant_wall },
    Template { name: "long gap", skill: Skill::FiredUp, unlock: 2, side_b: 0, height: H, build: long_gap },
    Template { name: "glide launch", skill: Skill::Waltz, unlock: 6, side_b: 0, height: H, build: glide_launch },
    Template { name: "stain pit", skill: Skill::Stains, unlock: 2, side_b: 0, height: H, build: stain_pit },
    Template { name: "grease chute", skill: Skill::Grease, unlock: 5, side_b: 0, height: H, build: grease_chute },
    Template { name: "mega wall", skill: Skill::Buddy, unlock: 2, side_b: 0, height: MEGA_H, build: mega_wall },
    Template { name: "buddy raft pool", skill: Skill::Buddy, unlock: 4, side_b: 0, height: H, build: buddy_raft_pool },
    Template { name: "fired-up chasm", skill: Skill::Buddy, unlock: 7, side_b: 0, height: CHASM_H, build: fired_up_chasm },
    Template { name: "cling shaft", skill: Skill::Buddy, unlock: 8, side_b: 0, height: MEGA_H, build: cling_shaft },
    Template { name: "trapeze", skill: Skill::Buddy, unlock: 9, side_b: 0, height: MEGA_H, build: trapeze },
    // The Side B room: once B1 is cleared (its pieces grow with each Side B level cleared).
    // Never picked by skill: `super::run` mixes it in now and then.
    Template { name: "han alone", skill: Skill::Buddy, unlock: 0, side_b: 1, height: ALONE_H, build: alone::build },
];

/// The templates for a skill.
pub fn for_skill(skill: Skill) -> impl Iterator<Item = &'static Template> {
    TEMPLATES.iter().filter(move |t| t.skill == skill)
}

impl Template {
    /// Free play may serve it with `unlocked_levels` story levels playable.
    pub fn unlocked(&self, unlocks: impl Into<Unlocks>) -> bool {
        let u = unlocks.into();
        self.unlock < u.levels.max(1) && self.side_b <= u.side_b
    }

    /// A Side B room (`super::alone`).
    pub fn alone(&self) -> bool {
        self.side_b > 0
    }
}

/// Skills free play can serve with `unlocked` story levels playable.
pub fn unlocked_skills(unlocks: impl Into<Unlocks>) -> Vec<Skill> {
    let u = unlocks.into();
    let mut out: Vec<Skill> = Vec::new();
    for t in TEMPLATES {
        if t.unlocked(u) && !t.alone() && !out.contains(&t.skill) {
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

/// The highest and lowest ground surfaces a room's terrain goes to (rows from the top): up to
/// 7 tiles above the floor, 2 below it.
const HIGH: i32 = FLOOR as i32 - 7;
const LOW: i32 = FLOOR as i32 + 2;

/// About `n` columns of rolling ground, from the floor and back to it: humps and dips 2-5
/// columns wide, up to 2 (band 1) .. 4 (band 10) tiles high and 2 deep, a hop (≤ 2 tiles) from
/// one to the next, nuggets on the high ones. The connective tissue between a room's
/// features, so no room is all flat.
fn hills(c: &mut Canvas, d: &mut Dice, band: Band, n: usize) {
    let rise = 2 + d.scaled(band, 0.0, 2.0, 0);
    let end = c.width() + n;
    let mut top = FLOOR as i32;
    while c.width() + 5 < end {
        let w = d.int(2, 5) as usize;
        let next = (top + d.pick(&[-2, -1, 1, 2])).clamp(FLOOR as i32 - rise, LOW);
        let x = c.width();
        c.ground(w, next as usize);
        if next < FLOOR as i32 - 1 {
            arc(c, x, w, next as usize - 1, 1);
        }
        top = next;
    }
    while top != FLOOR as i32 {
        top += (FLOOR as i32 - top).clamp(-2, 2);
        c.ground(2, top as usize);
    }
    while c.width() < end {
        c.ground(1, FLOOR);
    }
}

/// How long a room's rolling lead-in is (columns).
fn lead_in(d: &mut Dice) -> usize {
    d.int(6, 12) as usize
}

// ─── Precision: jump gauntlets ───────────────────────────────────────────────

/// Gaps (2 → 6 tiles: the widest need a toot), steps up and down, pillars, floating shelves and
/// climbs (stairs up to 7 tiles above the floor, and a drop back down); landings shrink from 5
/// tiles to 1-2 as the band rises.
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
        let kind = if band < 3 && d.chance(0.3) { 4 } else { d.int(0, if band >= 3 { 4 } else { 1 }) };
        match kind {
            // A gap, maybe to a ledge up or down.
            0 | 1 => {
                let g = (d.scaled(band, 2.0, 4.6, 1) as usize).clamp(2, 6);
                let max_up = if g >= 5 { 1 } else { 2 + (tb > 0.5) as i32 };
                let next = (top as i32 - d.int(-2, max_up)).clamp(HIGH, LOW) as usize;
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
                    top = (top as i32 - d.int(-1, 1)).clamp(HIGH, FLOOR as i32) as usize;
                    let w = (lerpi(band, 3.0, 1.0) + d.int(0, 1) as usize).max(1);
                    let x = c.width();
                    c.ground(w, top);
                    c.nugget(x, top - 1);
                }
                c.ground(2, top);
            }
            // A climb: stairs up two tiles at a time, then a drop back down (over a pit, from
            // band 5).
            4 => {
                let steps = d.int(2, 4);
                for _ in 0..steps {
                    let next = (top as i32 - 2).max(HIGH) as usize;
                    if next == top {
                        break;
                    }
                    let w = (lerpi(band, 3.0, 2.0) + d.int(0, 1) as usize).max(2);
                    let x = c.width();
                    c.ground(w, next);
                    c.nugget(x + w / 2, next - 1);
                    top = next;
                }
                if band >= 5 {
                    let x = c.width();
                    let g = d.int(2, 3) as usize;
                    c.pit(g);
                    arc(c, x, g, top - 1, 2);
                }
                top = (top as i32 + d.int(3, 6)).min(FLOOR as i32) as usize;
                c.ground(3, top);
            }
            // Floating shelves (`=`) over a pit.
            _ => {
                let shelves = 2 + d.scaled(band, 0.0, 1.0, 1) as usize;
                let w = lerpi(band, 3.0, 1.0).max(1);
                let mut row = top;
                for _ in 0..shelves {
                    let g = d.int(2, 2 + (tb * 2.0).round() as i32) as usize;
                    c.pit(g);
                    row = (row as i32 - d.int(-1, 2)).clamp(HIGH, FLOOR as i32) as usize;
                    let x = c.width();
                    c.pit(w);
                    for k in 0..w {
                        c.set(x + k, row, b'=');
                    }
                    c.nugget(x, row - 1);
                }
                let g = d.int(2, 3) as usize;
                c.pit(g);
                top = (row as i32 + d.int(0, 1)).clamp(HIGH, FLOOR as i32) as usize;
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

// ─── Precision, tall: a shelf climb ──────────────────────────────────────────

/// How far a climb rises: the tower top is this many rows above the floor.
pub const CLIMB_RISE: usize = 20;

/// Up a shaft of one-way shelves, zigzagging [`CLIMB_RISE`] tiles up beside a tower (the toilet
/// climb of level 2), across its top and a drop down the far side. Rungs 3 tiles apart (a
/// jump) and from band 6 sometimes 4 (a toot); shelves 4 → 2 wide with a wider gap between the
/// sides; from band 5 sewage floods the shaft floor. (No flies: a fly in the shaft sits on
/// every jump's path.)
fn shelf_climb(c: &mut Canvas, d: &mut Dice, band: Band, _: &Dressing) -> Built {
    let floor = c.floor();
    let summit = floor - CLIMB_RISE;
    c.ground(3, floor);
    let w = lerpi(band, 4.0, 2.0);
    let shaft = 2 * w + lerpi(band, 1.0, 3.0) + d.int(0, 1) as usize;
    let x0 = c.width();
    let landing = w.min(2);
    for k in 0..shaft {
        if band >= 5 && k >= landing {
            c.pool(1);
        } else {
            c.ground(1, floor);
        }
    }
    // Rungs bottom up, alternating sides; the last is flush with the tower.
    let mut rungs = Vec::new();
    let mut r = floor;
    let mut right = false;
    loop {
        let step = if band >= 6 && d.chance(0.4) { 4 } else { 3 };
        r -= step;
        let last = r <= summit + step;
        let sx = if last || right { shaft - w - if last { 0 } else { d.int(0, 1) as usize } } else { d.int(0, 1) as usize };
        rungs.push((x0 + sx, r.max(summit + 1)));
        right = !right;
        if last {
            break;
        }
    }
    for &(x, r) in &rungs {
        for k in 0..w {
            c.set(x + k, r, b'=');
        }
        c.nugget(x + w / 2, r - 1);
    }
    let checkpoint = Some((c.width() + 1, summit - 1));
    let x = c.width();
    let top = (lerpi(band, 5.0, 3.0) + d.int(0, 1) as usize).max(3);
    c.ground(top, summit);
    for k in 0..top {
        c.nugget(x + k, summit - 1);
    }
    // Over the edge: nuggets down the drop, and the floor.
    let x = c.width();
    c.ground(5, floor);
    for k in (summit + 2..floor - 1).step_by(4) {
        c.nugget(x + 1, k);
    }
    Built { hint: Some((&[Topic::OneWay], "Hop hop hop!")), checkpoint }
}

// ─── HazardTiming: flies and sprays ──────────────────────────────────────────

/// Spray cans (single, then pairs) and fly swarms, hovering lower and over gaps as the band
/// rises; 2 → 7 hazards.
fn hazards(c: &mut Canvas, d: &mut Dice, band: Band, _: &Dressing) -> Built {
    c.ground(4, FLOOR);
    let n = lead_in(d);
    hills(c, d, band, n);
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
        (true, true) => Some((&[Topic::Fly, Topic::Spray], "Flies! Sprays! Wait...")),
        (true, false) => Some((&[Topic::Fly], "Little hops!")),
        (false, true) => Some((&[Topic::Spray], "Pssht! Wait...")),
        _ => None,
    };
    Built { hint, checkpoint }
}

// ─── MovingPlatforms ─────────────────────────────────────────────────────────

/// Pits (6 → 13 wide, bottomless or sewage) crossed on rolls and ducks, and plunger lifts up to
/// high ledges; platforms shrink (3 → 1 tiles) and speed up (5 s → 2.6 s round trips).
fn platforms(c: &mut Canvas, d: &mut Dice, band: Band, _: &Dressing) -> Built {
    c.ground(3, FLOOR);
    let n = lead_in(d);
    hills(c, d, band, n);
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
    Built { hint: Some((&[Topic::Platform], "Ride the rolls!")), checkpoint }
}

// ─── GiantSteps: a giant wall with a runway ──────────────────────────────────

/// A runway (12 → 8 tiles) and a 6-tile wall (2 → 5 thick); higher bands add jumps on and after
/// the wall.
fn giant_wall(c: &mut Canvas, d: &mut Dice, band: Band, _: &Dressing) -> Built {
    c.ground(2, FLOOR);
    let n = lead_in(d);
    hills(c, d, band, n);
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
    Built { hint: Some((&[Topic::Giant], "Big wall!")), checkpoint }
}

// ─── FiredUp: a long gap with a nugget line ──────────────────────────────────

/// A nugget line (4+, spaced 2 → 3) on a flat run-up, an 11-tile bottomless gap, a landing
/// (6 → 3 tiles); higher bands add a jump after it.
fn long_gap(c: &mut Canvas, d: &mut Dice, band: Band, dress: &Dressing) -> Built {
    c.ground(2, FLOOR);
    let n = lead_in(d);
    hills(c, d, band, n);
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
    Built { hint: Some((&[Topic::Gap], "Big gap!")), checkpoint }
}

// ─── Waltz: a glide launch ───────────────────────────────────────────────────

/// A runway (9 → 7 tiles) and a pit with a toilet-roll platform sliding out over it, too wide
/// to cross from the platform at its everyday speed: in the waltz the platform glides on ONE,
/// and a jump off it then is a long launch. Wider gaps as the band rises.
fn glide_launch(c: &mut Canvas, d: &mut Dice, band: Band, _: &Dressing) -> Built {
    c.ground(2, FLOOR);
    let n = lead_in(d);
    hills(c, d, band, n);
    let checkpoint = Some((c.width(), STAND));
    c.ground(lerpi(band, 9.0, 7.0) + d.int(0, 1) as usize, FLOOR);
    let (pw, dx) = (2, 8);
    let gap = lerpi(band, GLIDE_GAP.0 as f32, GLIDE_GAP.1 as f32);
    let x = c.width();
    c.pit(pw + dx + gap);
    c.platform(x, FLOOR, pw, dx as f32, 0.0, GLIDE_PERIOD, d.u(), PlatformKind::Tp);
    // The prize past it.
    let past = c.width();
    c.ground(4, FLOOR);
    for k in 0..3 {
        c.nugget(past + k, STAND);
    }
    Built { hint: Some((&[Topic::Waltz], "Waltz time!")), checkpoint }
}

/// A glide launch's platform period (s): 8 tiles in 4.5 s, 89 px/s at its fastest (the most
/// any other mode keeps from it), ×4.5 gliding in the waltz (a jump keeps up to
/// `game::LAUNCH_MAX`)...
const GLIDE_PERIOD: f32 = 4.5;
/// ...and the gap past its path (tiles), band 1 → 10: the validator passes 17-19 (narrower
/// leaks to fired up and Giant Steps); `tests/gameplay.rs` clears 18 with the real physics at
/// every phase of the platform.
const GLIDE_GAP: (usize, usize) = (17, 18);

// ─── Stains: a stain pit ─────────────────────────────────────────────────────

/// A spike pit too wide to jump in any mode (14 → 19 tiles: 1 → 3+ splats to cross), nuggets
/// over it; higher bands add a jump after it.
fn stain_pit(c: &mut Canvas, d: &mut Dice, band: Band, _: &Dressing) -> Built {
    c.ground(4, FLOOR);
    let n = lead_in(d);
    hills(c, d, band, n);
    let w = d.scaled(band, 14.0, 18.0, 1) as usize;
    let x = c.width();
    for _ in 0..w {
        c.column(|r| {
            if r == FLOOR {
                b'^'
            } else if r > FLOOR {
                b'#'
            } else {
                b'.'
            }
        });
    }
    for k in (2..w - 1).step_by(3) {
        c.nugget(x + k, STAND - 3);
    }
    c.ground(4, FLOOR);
    if band >= 6 {
        c.pit(d.int(2, 3) as usize);
        c.ground(3, FLOOR);
    }
    Built { hint: Some((&[Topic::Stain], "Splat time!")), checkpoint: None }
}

// ─── Grease: a chute ─────────────────────────────────────────────────────────

/// A sunken grease run (15 → 20 tiles) ending in spikes (one per splat the band needs to get
/// nervous); the way out is a jump off the grease, which only sweaty grip allows.
fn grease_chute(c: &mut Canvas, d: &mut Dice, band: Band, _: &Dressing) -> Built {
    c.ground(4, FLOOR);
    let n = lead_in(d);
    hills(c, d, band, n);
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
        hint: Some((&[Topic::Grease, Topic::Grip], "Grease! Careful!")),
        checkpoint: None,
    }
}

// ─── Buddy: Han's gates ──────────────────────────────────────────────────────
//
// Each needs Han, so each keeps its gate at least `HAN_BERTH` columns from the room's ends: in
// the course, a neighbouring room's band gate must not put it in a band zone (where Han's
// boost is the weak one and his head holds Nat only while he stands).

/// Columns of floor from the entry pipe before a Han gate's approach.
const HAN_LEAD: usize = 6;
/// Columns of floor after a Han gate before the room closes.
const HAN_TAIL: usize = 8;

/// Floor from the entry pipe to the approach of a Han gate; returns the room's checkpoint.
fn han_lead(c: &mut Canvas) -> Option<(usize, usize)> {
    let floor = c.floor();
    c.ground(2, floor);
    let checkpoint = Some((c.width(), floor - 1));
    c.ground(HAN_LEAD - 2, floor);
    checkpoint
}

/// A mega wall room's rows: headroom over the wall.
pub const MEGA_H: usize = H + 4;

/// A mega wall: an 11-tile mesa (out of Giant Steps' reach and the plain boost's, in Giant
/// Steps' mega-boost off Han) after a flat approach long enough to toot up Giant Steps (10 → 9
/// tiles; a hop on the way from band 4); the top narrows (6 → 2 tiles) and from band 6 has a
/// hop along it.
fn mega_wall(c: &mut Canvas, d: &mut Dice, band: Band, _: &Dressing) -> Built {
    let floor = c.floor();
    let checkpoint = han_lead(c);
    if band >= 4 {
        c.pit(d.int(2, 3) as usize);
    }
    c.ground(lerpi(band, 10.0, 9.0) + d.int(0, 1) as usize, floor);
    let top = floor - 11;
    let x = c.width();
    let thick = (lerpi(band, 6.0, 2.0) + d.int(0, 1) as usize).max(2);
    c.ground(thick, top);
    for k in 0..thick {
        c.nugget(x + k, top - 1);
    }
    if band >= 6 {
        let g = d.int(2, 3) as usize;
        let x = c.width();
        c.pit(g);
        arc(c, x, g, top - 1, 1);
        c.ground(lerpi(band, 3.0, 2.0), top);
    }
    // Down the far side in steps.
    c.ground(2, top + 4);
    c.ground(2, top + 8);
    c.ground(HAN_TAIL, floor);
    Built { hint: Some((&[Topic::Mega], "Hop on!")), checkpoint }
}

/// A cling shaft: a low corridor (under a wall up to the ceiling) into a 3-wide shaft whose far
/// wall is an 11-tile mesa (out of the plain boost's reach). Nervous, Han climbs a wall of it and kicks Nat up. A sunken spike
/// strip on the way (one tile per splat the band needs to get nervous), easy to hop; from band 5
/// a hop before it too.
fn cling_shaft(c: &mut Canvas, d: &mut Dice, band: Band, _: &Dressing) -> Built {
    let floor = c.floor();
    let checkpoint = han_lead(c);
    if band >= 5 {
        c.pit(d.int(2, 3) as usize);
        c.ground(3, floor);
    }
    let spikes = crate::audio::director::NERVOUS_DEATHS as usize;
    for _ in 0..spikes {
        c.column(move |r| match r {
            r if r == floor => b'^',
            r if r > floor => b'#',
            _ => b'.',
        });
    }
    c.ground(lerpi(band, 5.0, 3.0) + d.int(0, 1) as usize, floor);
    let top = floor - 11;
    // The corridor under the near wall: two rows of headroom, solid to the top (no roof to
    // mega-boost onto from outside).
    for _ in 0..4 {
        c.column(move |r| if r < floor - 2 || r >= floor { b'#' } else { b'.' });
    }
    c.ground(3, floor);
    let x = c.width();
    let thick = (lerpi(band, 6.0, 3.0) + d.int(0, 1) as usize).max(3);
    c.ground(thick, top);
    for k in 1..thick - 1 {
        c.nugget(x + k, top - 1);
    }
    // Down the far side in steps.
    c.ground(2, top + 4);
    c.ground(2, top + 8);
    c.ground(HAN_TAIL, floor);
    Built { hint: Some((&[Topic::Shaft], "Wall time!")), checkpoint }
}

/// A trapeze: a runway to waltz on, then a lip under a ceiling that ends over the gap (Han's
/// plunger sticks in its edge, `TRAPEZE_OUT` past the lip, 5 tiles of handle), a bottomless gap
/// (15 → 16 tiles) and the far side 3 rows up behind 3 spikes: out of every mode's reach, a
/// launch off Han's swing on ONE clears it.
fn trapeze(c: &mut Canvas, d: &mut Dice, band: Band, _: &Dressing) -> Built {
    let floor = c.floor();
    let checkpoint = han_lead(c);
    c.ground(lerpi(band, 9.0, 7.0) + d.int(0, 1) as usize, floor);
    // The ceiling: over the end of the runway and out over the gap to the anchor.
    let cb = floor - 8;
    for _ in 0..4 {
        c.column(move |r| if r <= cb || r >= floor { b'#' } else { b'.' });
    }
    let out = crate::level::buddy::TRAPEZE_OUT as usize;
    let gap = lerpi(band, 15.0, 16.0);
    for k in 0..gap {
        c.column(move |r| if k < out && r <= cb { b'#' } else { b'.' });
    }
    let high = floor - 3;
    for _ in 0..3 {
        c.column(move |r| match r {
            r if r == high - 1 => b'^',
            r if r >= high => b'#',
            _ => b'.',
        });
    }
    let x = c.width();
    c.ground(4, high);
    for k in 1..3 {
        c.nugget(x + k, high - 1);
    }
    c.ground(2, high + 2);
    c.ground(HAN_TAIL, floor);
    Built { hint: Some((&[Topic::Trapeze], "Swing time!")), checkpoint }
}

/// Where a buddy raft pool starts (column in the room, from the entry checkpoint near 0).
const RAFT_POOL_AT: usize = 40;

/// A buddy raft pool: sewage (16 → 19 wide) under ceiling spikes, too far from the checkpoint
/// for Nat's own rafts; Han's rafts are the way. A hop on the way there from band 3, and one
/// after it from band 6.
fn buddy_raft_pool(c: &mut Canvas, d: &mut Dice, band: Band, _: &Dressing) -> Built {
    han_lead(c);
    // A long way from the checkpoint: Nat's rafts sink before the walk back for the next.
    if band >= 3 {
        c.ground(10, FLOOR);
        c.pit(d.int(2, 3) as usize);
    }
    let pool_at = RAFT_POOL_AT + d.int(0, 3) as usize;
    c.ground(pool_at.saturating_sub(c.width()).max(6), FLOOR);
    let w = (d.scaled(band, 16.0, 18.0, 1) as usize).max(BUDDY_POOL_MIN);
    let x = c.width();
    for _ in 0..w {
        c.column(|r| match r {
            r if r < FLOOR - 3 => b'#',
            r if r == FLOOR - 3 => b'v',
            r if (FLOOR..=FLOOR + 2).contains(&r) => b'~',
            r if r > FLOOR + 2 => b'#',
            _ => b'.',
        });
    }
    for k in (1..w - 1).step_by(3) {
        c.nugget(x + k, STAND);
    }
    c.ground(4, FLOOR);
    if band >= 6 {
        c.pit(d.int(2, 3) as usize);
        c.ground(3, FLOOR);
    }
    c.ground(HAN_TAIL, FLOOR);
    // No mid-room checkpoint: one near the pool would let Nat's own rafts bridge it.
    Built { hint: Some((&[Topic::BuddyRaft], "Raft time!")), checkpoint: None }
}

/// A fired-up chasm room's rows: a throw stays well under the room's top (a ceiling).
pub const CHASM_H: usize = H + 4;

/// A fired-up chasm: a nugget line (4, spaced 2) on a flat run-up, then a bottomless gap
/// (15 → 18 wide) between floors at one height: past the fired-up band's own reach, in the
/// running throw's off Han at the edge.
fn fired_up_chasm(c: &mut Canvas, d: &mut Dice, band: Band, _: &Dressing) -> Built {
    let floor = c.floor();
    let checkpoint = han_lead(c);
    let x = c.width();
    c.ground(4 * 2 + 5, floor);
    for k in 0..4 {
        c.nugget(x + 1 + k * 2, floor - 1);
    }
    let w = d.scaled(band, 15.0, 18.0, 1).clamp(CHASM_MIN as i32 + 1, 18) as usize;
    c.pit(w);
    let x = c.width();
    c.ground(HAN_TAIL, floor);
    for k in (1..HAN_TAIL - 1).step_by(2) {
        c.nugget(x + k, floor - 1);
    }
    Built { hint: Some((&[Topic::Chain], "Hop on!")), checkpoint }
}
