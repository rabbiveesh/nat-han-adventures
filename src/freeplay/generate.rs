//! Room generation: a [`RoomPlan`] (what the adaptive engine asked for) becomes a validated
//! [`Room`]. Each attempt draws the room from its own seeded dice and runs the level validator
//! on it ([`check_room`]); a failed attempt re-rolls with the next attempt's dice. [`Job`] does
//! one attempt per [`Job::step`], so the game can spread the work over frames.

use std::sync::OnceLock;

use bevy::platform::time::Instant;

use super::canvas::{Canvas, ENTRY, STAND};
use super::dice::{Dice, stream};
use super::templates::{Built, Dressing, TEMPLATES, Template, for_skill};
use crate::adapt::{AssistLevers, RoomRequest, Skill};
use crate::level::validate::{Physics, Report, Strategy, check_room};
use crate::level::{Level, TILE};

/// Rooms wider than this get their template's mid-room checkpoint.
pub const LONG_ROOM: usize = 80;
/// Attempts at the requested room before falling back to a gentle one.
pub const MAX_ATTEMPTS: u32 = 8;

/// Dice streams (the `what` of [`stream`]).
pub const ROOM_STREAM: u64 = 1;
pub const PICK_STREAM: u64 = 2;
pub const WORLD_STREAM: u64 = 3;

/// Han's line at each room's checkpoint.
pub const CHECKPOINT_LINES: &[&str] = &[
    "Fresh roll! Onward, Nat.",
    "Two-ply! This pipe's all yours.",
    "New pipe, new smells. Let's go!",
    "Plumber's tip: always flush forward.",
    "This one's got character. And odor.",
    "Roll call! Nat? Here. Han? Here. Go!",
    "Another pipe! Who builds these? Oh. Me.",
    "Keep rollin', Nat. I'll keep followin'.",
];

/// The disguised placement rooms' lines ("Han checks your plumbing").
pub const CALIBRATION_LINES: &[&str] = &[
    "Lemme check your plumbing, Nat. Hop along!",
    "Good pressure! Couple more test pipes.",
    "Last test pipe, Nat. Then the real stuff!",
];

/// The validator's physics tables for rooms, built once: reachability with [`lean_strategies`]
/// (the proofs that gates are exclusive keep every ideal jump).
pub fn physics() -> &'static Physics {
    static PHYS: OnceLock<Physics> = OnceLock::new();
    PHYS.get_or_init(|| Physics::with_human(lean_strategies()))
}

/// The jumps a room must be beatable with: a lean subset of the validator's human
/// [`strategies`](crate::level::validate::strategies) (a seventh of them, and so about that
/// much faster to check). Every room free play serves is beatable with these alone: full and
/// short hops, running and standing starts, from the middle or the edge of a tile, toots early,
/// at the apex and late, and letting go of the direction mid-air to land short.
pub fn lean_strategies() -> Vec<Strategy> {
    let inf = f32::INFINITY;
    let mut out = Vec::new();
    for toot in [None, Some(0.27), Some(0.36)] {
        out.push(Strategy { dir: 0.0, jump: true, hold: inf, vx0: 0.0, toot, edge: false, release: inf });
    }
    for dir in [-1.0, 1.0] {
        for toot in [None, Some(0.2), Some(0.36), Some(0.5)] {
            for (hold, vx0, edge) in [
                (inf, 0.0, false),
                (inf, dir, false),
                (inf, dir, true),
                (inf, 0.0, true),
                (0.12, dir, true),
                (0.12, 0.0, false),
            ] {
                out.push(Strategy { dir, jump: true, hold, vx0, toot, edge, release: inf });
            }
            // Running off a ledge.
            out.push(Strategy { dir, jump: false, hold: inf, vx0: dir, toot, edge: true, release: inf });
        }
        for toot in [None, Some(0.36)] {
            for release in [0.15, 0.3] {
                out.push(Strategy { dir, jump: true, hold: inf, vx0: dir, toot, edge: true, release });
            }
        }
        for edge in [false, true] {
            out.push(Strategy { dir, jump: true, hold: 0.05, vx0: dir, toot: None, edge, release: inf });
        }
    }
    out
}

/// One room to build.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomPlan {
    /// 0-based position in the run.
    pub index: u32,
    pub request: RoomRequest,
    /// Index into [`TEMPLATES`].
    pub template: usize,
    /// Han hints the mechanic at the room's start.
    pub hint: bool,
    /// Han's line at the room's checkpoint.
    pub line: &'static str,
    pub world: u8,
}

impl RoomPlan {
    /// The plan for `request` as room `index` of the run with `seed`: picks the template (by
    /// seed, among the skill's unlocked with `unlocked_levels` story levels playable) and the
    /// dressing. `seen` says whether the run already had a room of this skill (the first one
    /// gets Han's hint).
    pub fn new(
        seed: u32,
        index: u32,
        request: RoomRequest,
        world: u8,
        seen: bool,
        calibrating: bool,
        unlocked_levels: usize,
    ) -> RoomPlan {
        let mut d = Dice::new(stream(seed, PICK_STREAM, index as u64));
        let choices: Vec<usize> = TEMPLATES
            .iter()
            .enumerate()
            .filter(|(_, t)| t.skill == request.skill && t.unlocked(unlocked_levels))
            .map(|(i, _)| i)
            .collect();
        let template = if choices.is_empty() { 0 } else { d.pick(&choices) };
        let line = if calibrating {
            CALIBRATION_LINES[(index as usize).min(CALIBRATION_LINES.len() - 1)]
        } else {
            d.pick(CHECKPOINT_LINES)
        };
        RoomPlan { index, request, template, hint: !seen || request.assists.han_hint, line, world }
    }

    pub fn template(&self) -> &'static Template {
        &TEMPLATES[self.template]
    }
}

/// A validated room.
#[derive(Debug, Clone)]
pub struct Room {
    pub plan: RoomPlan,
    /// The room on its own (entry pipe, body, exit pipe), as validated.
    pub level: Level,
    /// Deaths the room's design needs (stain pit splats, the nervous band's splats for a chute).
    pub expected_deaths: u32,
    /// A relaxed time to clear it.
    pub par_secs: f32,
    /// Attempts it took, and whether it's the fallback room instead of the one planned.
    pub attempts: u32,
    pub fallback: bool,
    /// Time spent drawing and validating, all attempts (µs).
    pub micros: u64,
}

/// Draw attempt `attempt` of `plan` (no validation).
pub fn draw(seed: u32, plan: &RoomPlan, attempt: u32) -> Level {
    let mut d = Dice::new(stream(seed, ROOM_STREAM, plan.index as u64 * 1024 + attempt as u64));
    let mut c = Canvas::new();
    let dress = Dressing { extra_nuggets: plan.request.assists.extra_nuggets_before_quartal };
    let built: Built = (plan.template().build)(&mut c, &mut d, plan.request.band, &dress);
    if plan.hint
        && let Some((topics, text)) = built.hint
    {
        c.hint(ENTRY + 1, STAND - 1, topics, text);
    }
    // An extra checkpoint when assists ask for one, and in long rooms (respawn points must be
    // at most `MAX_SEGMENT` apart).
    if (plan.request.assists.extra_checkpoint || c.width() > LONG_ROOM)
        && let Some((col, row)) = built.checkpoint
    {
        c.checkpoint(col, row, "Extra roll, on the house!");
    }
    c.finish(plan.world, plan.line)
}

/// Validate a drawn room: the report, with its errors.
pub fn validate(level: &Level) -> Report {
    check_room(level, physics())
}

/// The gentle room used when a plan keeps failing: a band-1 jump gauntlet, no frills.
pub fn fallback_plan(plan: &RoomPlan) -> RoomPlan {
    let template = TEMPLATES.iter().position(|t| t.skill == Skill::Precision).unwrap_or(0);
    let request = RoomRequest { skill: Skill::Precision, band: 1, assists: AssistLevers::NONE };
    RoomPlan { request, template, hint: false, ..plan.clone() }
}

/// Generation of one room, an attempt at a time.
#[derive(Debug, Clone)]
pub struct Job {
    pub seed: u32,
    pub plan: RoomPlan,
    pub attempt: u32,
    pub fallback: bool,
    pub micros: u64,
}

impl Job {
    pub fn new(seed: u32, plan: RoomPlan) -> Job {
        Job { seed, plan, attempt: 0, fallback: false, micros: 0 }
    }

    /// One attempt: the room if it validated.
    pub fn step(&mut self) -> Option<Room> {
        let t0 = Instant::now();
        let level = draw(self.seed, &self.plan, self.attempt);
        let report = validate(&level);
        self.attempt += 1;
        self.micros += t0.elapsed().as_micros() as u64;
        if report.errs.is_empty() {
            let mut level = level;
            level.deaths = Some(report.deaths);
            // The marks the validator found its gates by: the game's Han reads them.
            level.gates = report.marks.clone();
            let par_secs = level.width as f32 * TILE / 70.0 + 4.0 + 3.0 * report.deaths as f32;
            return Some(Room {
                plan: self.plan.clone(),
                level,
                expected_deaths: report.deaths,
                par_secs,
                attempts: self.attempt,
                fallback: self.fallback,
                micros: self.micros,
            });
        }
        if self.attempt >= MAX_ATTEMPTS {
            if self.fallback {
                // Never seen in tests (`tests/freeplay.rs` runs the fallback on many seeds);
                // keep re-rolling rather than hang the run on one bad draw.
                assert!(self.attempt < 8 * MAX_ATTEMPTS, "free play: the fallback room keeps failing: {:?}", report.errs);
                return None;
            }
            // The plan keeps failing: a gentle room instead (validated like any other).
            self.plan = fallback_plan(&self.plan);
            self.fallback = true;
            self.attempt = 0;
        }
        None
    }

    /// Run to completion.
    pub fn run(mut self) -> Room {
        loop {
            if let Some(r) = self.step() {
                return r;
            }
        }
    }
}

/// Generate a room in one go.
pub fn generate(seed: u32, plan: RoomPlan) -> Room {
    Job::new(seed, plan).run()
}

/// Is a template available? (Free play only plans skills that have one.)
pub fn has_template(skill: Skill) -> bool {
    for_skill(skill).next().is_some()
}
