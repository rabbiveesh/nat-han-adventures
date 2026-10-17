//! Proving a Side B level (native): Han runs it in the real gameplay, headless, the band
//! (headless audio) answering a plan of set-list calls; and a search for a plan that clears it.
//! The level editor's Validate uses it (a level's `solution:` line is replayed, else searched
//! for), as do the Side B tests' plans.
//!
//! A plan is a list of steps: once Han's past a column, pick a call on the set list and call it
//! (`32.6/gap`), or only pick it (`30/pick-melt`: the call Han's next nuggets add to). Calls go
//! through the keys a player presses (Left/Right pick, Down calls), so a plan plays the same as
//! it would by hand.

use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;

use bevy::{ecs::system::RunSystemOnce, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use leafwing_input_manager::user_input::Buttonlike;

use super::sideb::{self, Fate, Phase, SetCall, SideB, SideBLevel};
use super::{Han, Pos};
use crate::audio::AudioOutput;
use crate::level::TILE;

const DT: f64 = 1.0 / 60.0;
/// The longest a plan's step waits for Han to reach its column, and the longest he gets after
/// the last step (s, game time).
const STEP_WAIT: f32 = 30.0;
const END_WAIT: f32 = 40.0;

/// What a step does with set-list call `k` (its index in the set list).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Act {
    Call(usize),
    Pick(usize),
}

/// Once Han's past column `col`, `act`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Step {
    pub col: f32,
    pub act: Act,
}

pub type Plan = Vec<Step>;

/// How a play of a plan ended.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub cleared: bool,
    /// How Han met his end (none: he stopped, or time ran out).
    pub fate: Option<Fate>,
    /// The column he got to.
    pub col: f32,
    /// The set list's calls left at the end.
    pub left: Vec<u32>,
}

/// The word of a set-list call (`setlist:` header).
pub fn word(call: SetCall) -> &'static str {
    ["giant", "gap", "melt"].into_iter().find(|w| sideb::call_of(w) == Some(call)).unwrap_or("?")
}

/// A plan as text: `1/giant 32.6/gap 30/pick-melt`.
pub fn plan_text(b: &SideBLevel, plan: &[Step]) -> String {
    let name = |k: usize| b.setlist.get(k).map_or("?", |c| word(c.0));
    plan.iter()
        .map(|s| match s.act {
            Act::Call(k) => format!("{}/{}", s.col, name(k)),
            Act::Pick(k) => format!("{}/pick-{}", s.col, name(k)),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// [`plan_text`] back.
pub fn parse_plan(b: &SideBLevel, text: &str) -> Result<Plan, String> {
    let index = |w: &str| {
        let call = sideb::call_of(w).ok_or_else(|| format!("no such call `{w}`"))?;
        b.setlist.iter().position(|c| c.0 == call).ok_or_else(|| format!("`{w}` isn't on the set list"))
    };
    text.split_whitespace()
        .map(|item| {
            let (col, what) = item.split_once('/').ok_or_else(|| format!("expected col/call, got `{item}`"))?;
            let col: f32 = col.parse().map_err(|_| format!("bad column `{col}`"))?;
            let act = match what.strip_prefix("pick-") {
                Some(w) => Act::Pick(index(w)?),
                None => Act::Call(index(what)?),
            };
            Ok(Step { col, act })
        })
        .collect()
}

pub use super::sideb::split_solution;

/// A headless game playing Side B level `b` (the real gameplay and band).
fn app(b: &SideBLevel) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin, bevy::input::InputPlugin, crate::gameplay, crate::audio::plugin))
        .insert_resource(AudioOutput::Headless)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(DT)));
    app.update();
    let b = b.clone();
    app.world_mut().run_system_once(move |mut c: Commands| sideb::start_level(&mut c, 0, b.clone())).expect("starts");
    app.update();
    app.update();
    app
}

fn side(app: &App) -> &SideB {
    app.world().resource::<SideB>()
}

fn col(app: &mut App) -> f32 {
    let mut q = app.world_mut().query_filtered::<&Pos, With<Han>>();
    q.single(app.world()).map_or(0.0, |p| p.0.x / TILE)
}

fn until(app: &mut App, secs: f32, mut done: impl FnMut(&mut App) -> bool) -> bool {
    for _ in 0..(secs as f64 / DT).round() as usize {
        app.update();
        if done(app) {
            return true;
        }
    }
    false
}

fn tap(app: &mut App, k: KeyCode) {
    k.press(app.world_mut());
    app.update();
    k.release(app.world_mut());
    app.update();
}

fn pick(app: &mut App, k: usize) {
    let n = side(app).setlist.len();
    for _ in 0..n {
        let s = side(app).selected;
        if s == k {
            break;
        }
        tap(app, if s < k { KeyCode::ArrowRight } else { KeyCode::ArrowLeft });
    }
}

/// Play `b` by `plan`.
pub fn play(b: &SideBLevel, plan: &[Step]) -> Run {
    let mut app = app(b);
    let playing = |a: &mut App| side(a).phase == Phase::Playing;
    for s in plan {
        until(&mut app, STEP_WAIT, |a| col(a) >= s.col || !playing(a));
        if !playing(&mut app) {
            break;
        }
        match s.act {
            Act::Pick(k) => pick(&mut app, k),
            Act::Call(k) => {
                pick(&mut app, k);
                tap(&mut app, KeyCode::ArrowDown);
            }
        }
    }
    until(&mut app, END_WAIT, |a| !playing(a));
    let s = side(&app);
    let (cleared, fate, left) = (matches!(s.phase, Phase::Cleared { .. }), s.fate, s.left.clone());
    Run { cleared, fate, col: col(&mut app), left }
}

/// The search's knobs.
#[derive(Debug, Clone, Copy)]
pub struct SolveOpts {
    /// Plans kept per round (the ones that get Han furthest, distinct outcomes first).
    pub beam: usize,
    /// Columns between tried steps.
    pub every: f32,
    /// How far back from where Han ended a new step may go (columns).
    pub back: f32,
    /// Give up after this many plays.
    pub budget: usize,
    /// Plays at once.
    pub threads: usize,
}

impl Default for SolveOpts {
    fn default() -> Self {
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).max(1);
        SolveOpts { beam: 4, every: 3.0, back: 40.0, budget: 4000, threads }
    }
}

/// How the search is going.
#[derive(Debug, Clone)]
pub struct Progress {
    pub plays: usize,
    /// The plan that gets Han furthest so far, and where it gets him.
    pub best: Plan,
    pub best_col: f32,
}

/// What the search found.
#[derive(Debug, Clone)]
pub enum Solved {
    Cleared(Plan),
    /// No plan cleared it: the one that got Han furthest, and how that ended.
    Stuck { plan: Plan, run: Run, plays: usize },
}

/// Search for a plan that clears `b`: a beam of the plans that get Han furthest, each grown by
/// one step (a call, or a pick of the call his nuggets add to) at columns back from where it
/// ended. `progress` hears after each round; `cancel` stops it (as stuck).
pub fn solve(b: &SideBLevel, opts: SolveOpts, progress: &(dyn Fn(&Progress) + Sync), cancel: &AtomicBool) -> Solved {
    let plays = AtomicUsize::new(0);
    let run = |plan: &Plan| {
        plays.fetch_add(1, Ordering::Relaxed);
        play(b, plan)
    };
    let first = run(&Vec::new());
    if first.cleared {
        return Solved::Cleared(Vec::new());
    }
    let n = b.setlist.len();
    let mut beam: Vec<(Plan, Run)> = vec![(Vec::new(), first)];
    let mut best = beam[0].clone();
    loop {
        // Every child of every plan in the beam.
        let mut tries: Vec<Plan> = Vec::new();
        for (plan, r) in &beam {
            let from = plan.last().map_or(1.0, |s| s.col + 1.0).max(r.col - opts.back);
            let mut c = from;
            while c < r.col.max(from + opts.every) {
                for k in 0..n {
                    for act in [Act::Call(k), Act::Pick(k)] {
                        let mut p = plan.clone();
                        p.push(Step { col: (c * 10.0).round() / 10.0, act });
                        tries.push(p);
                    }
                }
                c += opts.every;
            }
        }
        let results: Mutex<Vec<(Plan, Run)>> = Mutex::new(Vec::new());
        let next = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..opts.threads.min(tries.len()).max(1) {
                scope.spawn(|| {
                    loop {
                        if cancel.load(Ordering::Relaxed) || plays.load(Ordering::Relaxed) >= opts.budget {
                            return;
                        }
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(p) = tries.get(i) else { return };
                        let r = run(p);
                        let done = r.cleared;
                        results.lock().unwrap().push((p.clone(), r));
                        if done {
                            // Let the others finish their plays; no new ones.
                            next.store(usize::MAX / 2, Ordering::Relaxed);
                        }
                    }
                });
            }
        });
        let mut results = results.into_inner().unwrap();
        if let Some((p, _)) = results.iter().filter(|(_, r)| r.cleared).min_by_key(|(p, _)| p.len()) {
            return Solved::Cleared(p.clone());
        }
        // Children that got further, or as far with a different set list left (a pick whose
        // nuggets went elsewhere).
        let parent = |p: &Plan| beam.iter().find(|(q, _)| q[..] == p[..p.len() - 1]).map(|(_, r)| r.clone());
        results.retain(|(p, r)| {
            parent(p).is_some_and(|pr| r.col > pr.col + 0.5 || (r.col > pr.col - 0.5 && r.left != pr.left))
        });
        results.sort_by(|a, b| b.1.col.total_cmp(&a.1.col).then(a.0.len().cmp(&b.0.len())));
        let mut kept: Vec<(Plan, Run)> = Vec::new();
        for c in &results {
            if kept.len() < opts.beam && kept.iter().all(|k| (k.1.col - c.1.col).abs() > 0.3 || k.1.left != c.1.left) {
                kept.push(c.clone());
            }
        }
        if let Some(k) = kept.first()
            && k.1.col > best.1.col
        {
            best = k.clone();
        }
        let used = plays.load(Ordering::Relaxed);
        progress(&Progress { plays: used, best: best.0.clone(), best_col: best.1.col });
        if kept.is_empty() || cancel.load(Ordering::Relaxed) || used >= opts.budget {
            return Solved::Stuck { plan: best.0, run: best.1, plays: used };
        }
        beam = kept;
    }
}
