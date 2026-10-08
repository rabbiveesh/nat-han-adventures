//! The formal model of the band director (shared by `tests/formal.rs`, tier 1, and
//! `tests/deep.rs`, tier 2): the real [`Band`] driven by abstract play, its reachable states
//! explored breadth-first, and the properties checked from every one of them.
//!
//! - **Play** is a sequence of [`Act`]s on a 0.25 s grid ([`TICK`]): frames of events (a toot,
//!   a nugget, a death, a checkpoint, a ground jump, or several at once in tier 2) at the
//!   current time, restarts, and waits (each waited tick is one empty frame, so periodic checks
//!   and hold expiry happen exactly as in the game, a tick late at most).
//! - **The model** ([`Sim`]) is what `audio::plugin::direct` does with the director: the band
//!   steps, its decisions become the music playing, and the physics follow
//!   (`Groove { nervous: level_deaths ≥ NERVOUS_DEATHS, ..Groove::new(playing) }`). The bar-line
//!   latency of a switch is not modelled here (tier 2's gate recipes allow for it).
//! - **States** are deduplicated by an abstraction of the band ([`Key`]): times relative to now,
//!   the rolling window's entries within the 20 s horizon, counters saturated at the thresholds
//!   that read them. Two states with the same key behave the same from then on (the key keeps
//!   everything the director reads), so the search is exhaustive up to its depth.
//! - **A failure** is the shortest trace (breadth-first) to a state from which a property
//!   fails, plus the property's own witness, printed as e.g.
//!   `t=0 nugget ×4 | t=0 death | t=1 wait`. [`replay`] parses that format back, so a
//!   counterexample becomes a regression test: `regression("t=0 nugget ×4")` in
//!   `tests/formal.rs` replays the state part and checks every property from each state on
//!   the way.
#![allow(dead_code)]

use std::collections::HashSet;
use std::fmt::{self, Write as _};
use std::time::{Duration, Instant};

use nat_han_adventures::audio::director::{
    self, Band, Events, GIANT_STEPS_TOOTS, MUSIC_CHECK_SECS, NERVOUS_DEATHS, SUMMON_HOLD_SECS, WALTZ_MAX_INTERVAL,
    WALTZ_MIN_INTERVAL,
};
use nat_han_adventures::audio::{Filters, Harmony};
use nat_han_adventures::game::Groove;
use nat_han_adventures::game::tuning::RESPAWN_DELAY;

/// The time grid (s).
pub const TICK: f32 = 0.25;
/// The director's horizon in ticks (the rolling window, a hold, the periodic check).
pub const HORIZON: i32 = (MUSIC_CHECK_SECS / TICK) as i32;
pub const HOLD_TICKS: i32 = (SUMMON_HOLD_SECS / TICK) as i32;

/// Seconds to ticks (times in the band are all on the grid).
pub fn ticks(secs: f32) -> i32 {
    (secs / TICK).round() as i32
}

// Event bits of a frame.
pub const TOOT: u8 = 1;
pub const NUGGET: u8 = 2;
pub const DEATH: u8 = 4;
pub const CHECKPOINT: u8 = 8;
pub const JUMP: u8 = 16;
const BITS: [(u8, &str); 5] =
    [(TOOT, "toot"), (NUGGET, "nugget"), (DEATH, "death"), (CHECKPOINT, "checkpoint"), (JUMP, "jump")];

/// One step of play.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Act {
    /// A frame at the current time with these events ([`TOOT`] | [`NUGGET`] | ...): one each.
    Frame(u8),
    /// The level restarts (the band starts afresh).
    Restart,
    /// This many ticks pass, each an empty frame.
    Wait(u16),
}

impl Act {
    pub const TOOT: Act = Act::Frame(TOOT);
    pub const NUGGET: Act = Act::Frame(NUGGET);
    pub const DEATH: Act = Act::Frame(DEATH);
    pub const CHECKPOINT: Act = Act::Frame(CHECKPOINT);
    pub const JUMP: Act = Act::Frame(JUMP);

    fn events(bits: u8) -> Events {
        let n = |b: u8| (bits & b != 0) as u32;
        Events {
            toots: n(TOOT),
            nuggets: n(NUGGET),
            deaths: n(DEATH),
            checkpoints: n(CHECKPOINT),
            ground_jumps: n(JUMP),
            waltz_steps: 0,
        }
    }

    fn name(self) -> String {
        match self {
            Act::Frame(bits) => {
                BITS.iter().filter(|(b, _)| bits & b != 0).map(|(_, n)| *n).collect::<Vec<_>>().join("+")
            }
            Act::Restart => "restart".into(),
            Act::Wait(n) => format!("wait {}s", n as f32 * TICK),
        }
    }
}

/// A deliberately broken model, to show the checks catch real bugs (mutation testing).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mutation {
    None,
    /// The grease-chute soft-lock (before 826ce7b): sweaty grip only from the nervous band
    /// (melodic minor), no death-count layer. Deaths during a held summon don't count toward
    /// the mood, so a summon kept alive (ghost nuggets re-grabbed after every splat) keeps grip
    /// away forever.
    GripOnlyFromMelodicMinor,
    /// A restart that only resets the music, not the band's memory.
    RestartKeepsBand,
    /// Summons wait for the next periodic check instead of switching at once.
    SummonWaitsForCheck,
}

/// The director as the game drives it (see the module docs).
#[derive(Clone, Debug)]
pub struct Sim {
    pub band: Band,
    pub tick: i32,
    /// The music decided last (what's playing, once the bar line comes).
    pub playing: Filters,
    pub mutation: Mutation,
}

impl Sim {
    /// A level starts at t = 0.
    pub fn new(mutation: Mutation) -> Sim {
        let mut s = Sim { band: Band::default(), tick: 0, playing: Filters::default(), mutation };
        s.restart();
        s
    }

    pub fn now(&self) -> f32 {
        self.tick as f32 * TICK
    }

    fn restart(&mut self) {
        let now = self.now();
        if self.mutation != Mutation::RestartKeepsBand {
            self.band.start(now);
        }
        self.playing = Filters::default();
        // The plugin steps the band in the same frame (nothing happened yet).
        self.frame(Events::default());
    }

    /// One frame at the current time; returns the decision, if one was made.
    fn frame(&mut self, ev: Events) -> Option<Filters> {
        let now = self.now();
        let periodic = now >= self.band.next_check();
        let d = self.band.step(now, ev).map(|d| d.0);
        if let Some(f) = d {
            let summon_only = ev.deaths == 0 && ev.checkpoints == 0 && !periodic;
            if !(self.mutation == Mutation::SummonWaitsForCheck && summon_only) {
                self.playing = f;
            }
        }
        d
    }

    /// Play `act`; calls `seen` after every frame (for properties that look at each one).
    pub fn apply_with(&mut self, act: Act, mut seen: impl FnMut(&Sim, Option<Filters>)) {
        match act {
            Act::Frame(bits) => {
                let d = self.frame(Act::events(bits));
                seen(self, d);
            }
            Act::Restart => {
                self.restart();
                seen(self, None);
            }
            Act::Wait(n) => {
                for _ in 0..n {
                    self.tick += 1;
                    let d = self.frame(Events::default());
                    seen(self, d);
                }
            }
        }
    }

    pub fn apply(&mut self, act: Act) {
        self.apply_with(act, |_, _| {});
    }

    pub fn harmony(&self) -> Harmony {
        self.playing.harmony
    }

    /// The physics the music gives (the audio plugin's `set_groove` + the grip layer).
    pub fn groove(&self) -> Groove {
        groove_of(self.playing, self.band.stats.level_deaths, self.mutation)
    }

    pub fn grip(&self) -> bool {
        self.groove().grip()
    }

    pub fn held(&self) -> Option<Harmony> {
        self.band.held(self.now())
    }

    /// The abstract state (see [`Key`]).
    pub fn key(&self) -> Key {
        let now = self.now();
        let rel = |t: f32| ticks(t - now);
        let s = &self.band.stats;
        // The window within the horizon, merged per time, empty entries dropped.
        let mut window: Vec<(i32, [u32; 4])> = Vec::new();
        for (t, c) in self.band.window.entries() {
            let age = -rel(t);
            if age > HORIZON || c == [0; 4] {
                continue;
            }
            match window.iter_mut().find(|e| e.0 == age) {
                Some(e) => e.1 = std::array::from_fn(|k| e.1[k] + c[k]),
                None => window.push((age, c)),
            }
        }
        window.sort();
        // The jump-in-threes watch: a takeoff only matters while a next jump could still make
        // an even three with it.
        let [a, b] = self.band.steps.takeoffs();
        let ok = |i: f32| (WALTZ_MIN_INTERVAL..=WALTZ_MAX_INTERVAL).contains(&i);
        let steps = match b {
            Some(b) if now - b <= WALTZ_MAX_INTERVAL => {
                let a = a.filter(|&a| ok(b - a));
                [a.map(|a| -rel(a)), Some(-rel(b))]
            }
            _ => [None, None],
        };
        Key {
            counters: [
                s.level_deaths.min(NERVOUS_DEATHS) as u8,
                s.mood_deaths.min(NERVOUS_DEATHS) as u8,
                s.checkpoint_deaths.min(director::LAUGHING_DEATHS) as u8,
            ],
            last_summon: s.last_summon,
            hold: self.band.hold.filter(|&(_, until)| until > now).map(|(h, until)| (h, rel(until))),
            next_check: rel(self.band.next_check()).max(0),
            since_start: (-rel(self.band.level_start())).min(HORIZON),
            window,
            steps,
            playing: self.playing,
        }
    }
}

/// The physics for the music `playing` after `level_deaths` deaths (what the audio plugin
/// writes: `set_groove` keeps the grip layer, `direct` sets it from the death count).
pub fn groove_of(playing: Filters, level_deaths: u32, mutation: Mutation) -> Groove {
    let nervous = mutation != Mutation::GripOnlyFromMelodicMinor && level_deaths >= NERVOUS_DEATHS;
    Groove { nervous, ..Groove::new(playing) }
}

/// The abstract state of a [`Sim`]: everything the director reads, relative to now.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    /// Level deaths (saturated at 3: the grip layer), mood deaths (3: nervous), checkpoint
    /// deaths (2: the laughing band).
    counters: [u8; 3],
    last_summon: Option<Harmony>,
    /// The held summon and ticks left.
    hold: Option<(Harmony, i32)>,
    /// Ticks to the next periodic check (0: due).
    next_check: i32,
    /// Ticks since the level started (saturated: the stretch is at most a window long).
    since_start: i32,
    /// (age in ticks, [toots, nuggets, deaths, waltz steps]).
    window: Vec<(i32, [u32; 4])>,
    /// Ages of the takeoffs that still matter.
    steps: [Option<i32>; 2],
    playing: Filters,
}

/// A trace: acts from the start of a level.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Trace(pub Vec<Act>);

impl fmt::Display for Trace {
    /// `t=0 toot ×5 | t=2 death | t=2.5 nugget ×4 | t=22.5 wait`: each frame at its time (the
    /// same frame repeated at the same time: ×n), restarts too, and a final wait as the time
    /// play ends.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut parts: Vec<(i32, Act, u32)> = Vec::new();
        let mut tick = 0;
        for &a in &self.0 {
            match a {
                Act::Wait(n) => tick += n as i32,
                a => match parts.last_mut() {
                    Some((t, b, n)) if *t == tick && *b == a => *n += 1,
                    _ => parts.push((tick, a, 1)),
                },
            }
        }
        let trailing = parts.last().map_or(tick > 0, |p| tick > p.0);
        let mut out = String::new();
        for (t, a, n) in &parts {
            if !out.is_empty() {
                out += " | ";
            }
            let _ = write!(out, "t={} {}", *t as f32 * TICK, a.name());
            if *n > 1 {
                let _ = write!(out, " ×{n}");
            }
        }
        if trailing {
            if !out.is_empty() {
                out += " | ";
            }
            let _ = write!(out, "t={} wait", tick as f32 * TICK);
        }
        if out.is_empty() {
            out = "(level start)".into();
        }
        f.write_str(&out)
    }
}

impl Trace {
    /// Parse the [`Display`](fmt::Display) format back.
    pub fn parse(s: &str) -> Result<Trace, String> {
        let mut acts = Vec::new();
        let mut tick = 0;
        let s = s.trim();
        if s.is_empty() || s == "(level start)" {
            return Ok(Trace(acts));
        }
        for part in s.split('|') {
            let part = part.trim();
            let rest = part.strip_prefix("t=").ok_or_else(|| format!("{part:?}: want t=<secs> <event>"))?;
            let (t, what) = rest.split_once(' ').unwrap_or((rest, "wait"));
            let t: f32 = t.parse().map_err(|_| format!("{part:?}: bad time"))?;
            let at = ticks(t);
            if (at as f32 * TICK - t).abs() > 1e-3 {
                return Err(format!("{part:?}: time not on the {TICK}s grid"));
            }
            if at < tick {
                return Err(format!("{part:?}: time goes backwards"));
            }
            if at > tick {
                acts.push(Act::Wait((at - tick) as u16));
                tick = at;
            }
            let (what, n) = match what.split_once('×') {
                Some((w, n)) => (w.trim(), n.trim().parse::<u32>().map_err(|_| format!("{part:?}: bad count"))?),
                None => (what.trim(), 1),
            };
            let act = match what {
                "wait" => continue,
                "restart" => Act::Restart,
                w => {
                    let mut bits = 0;
                    for name in w.split('+') {
                        let b = BITS
                            .iter()
                            .find(|(_, n)| *n == name)
                            .ok_or_else(|| format!("{part:?}: unknown event {name:?}"))?;
                        bits |= b.0;
                    }
                    Act::Frame(bits)
                }
            };
            acts.extend(std::iter::repeat_n(act, n as usize));
        }
        Ok(Trace(acts))
    }
}

/// Play `trace` from a level start.
pub fn replay(trace: &Trace, mutation: Mutation) -> Sim {
    let mut s = Sim::new(mutation);
    for &a in &trace.0 {
        s.apply(a);
    }
    s
}

// --- Properties ---

/// A property that failed from some state: which, the witness (acts from that state), why.
#[derive(Debug, Clone)]
pub struct Violation {
    pub prop: &'static str,
    pub witness: Vec<Act>,
    pub detail: String,
}

/// The properties checked from every reachable state.
pub const PROPERTIES: [(&str, &str); 6] = [
    (
        "a",
        "deaths always reach grip: 3+ level deaths give grip, whatever else happens (chute loops with ghost nuggets)",
    ),
    (
        "b",
        "each summon input reaches its harmony at once (≤ 1 decision): 5 toots, 4 quick nuggets, 3 even ground jumps",
    ),
    ("c", "no flip-flop: with no input the harmony changes at most once, then stays"),
    ("d", "holds expire without keep-alive input (no input, or only deaths and checkpoints)"),
    ("e", "restart returns to the initial state"),
    ("f", "the laughing band never blocks grip"),
];

/// Respawn and walk back to a chute (ticks): the respawn delay, rounded up.
const RESPAWN_TICKS: u16 = ((RESPAWN_DELAY / TICK) as u16) + 1;

/// The summon input sequences (target, acts): each with a few spacings.
pub fn summons() -> Vec<(Harmony, Vec<Act>)> {
    let mut v = Vec::new();
    let spaced = |act: Act, n: u32, gap: u16| -> Vec<Act> {
        let mut out = Vec::new();
        for k in 0..n {
            if k > 0 && gap > 0 {
                out.push(Act::Wait(gap));
            }
            out.push(act);
        }
        out
    };
    for gap in [0, 1, 4] {
        v.push((Harmony::Coltrane, spaced(Act::TOOT, GIANT_STEPS_TOOTS, gap)));
        v.push((Harmony::Quartal, spaced(Act::NUGGET, director::FIRED_UP_NUGGETS, gap)));
    }
    // Even ground jumps: 0.5, 0.75, 1 s apart (on the 0.25 s grid inside 0.35..=1.2 s).
    for gap in 2..=4 {
        v.push((Harmony::Waltz, spaced(Act::JUMP, 3, gap)));
    }
    v
}

/// Chute loops: splat, respawn, maybe touch the checkpoint again, grab `k` (ghost) nuggets on
/// the way back, slide in again.
pub fn chute_loops() -> Vec<(String, Vec<Act>)> {
    let mut v = Vec::new();
    for k in [0u32, 1, 4] {
        for cp in [false, true] {
            let mut lap = vec![Act::DEATH, Act::Wait(RESPAWN_TICKS)];
            if cp {
                lap.push(Act::CHECKPOINT);
            }
            for _ in 0..k {
                lap.push(Act::NUGGET);
                lap.push(Act::Wait(1));
            }
            lap.push(Act::Wait(4));
            v.push((format!("{k} ghost nugget(s) per lap{}", if cp { ", checkpoint" } else { "" }), lap));
        }
    }
    v
}

/// Every property from `s` (`init`: the key of a fresh level).
pub fn check_state(s: &Sim, init: &Key) -> Result<(), Violation> {
    prop_a(s)?;
    prop_b(s)?;
    let q = quiet(s, QUIET);
    prop_c(s, &q)?;
    prop_d(s, &q)?;
    prop_e(s, init)?;
    prop_f(s)?;
    Ok(())
}

fn prop_a(s: &Sim) -> Result<(), Violation> {
    if s.band.stats.level_deaths >= NERVOUS_DEATHS && !s.grip() {
        return Err(Violation {
            prop: "a",
            witness: Vec::new(),
            detail: format!("{} level deaths but no grip (playing {:?})", s.band.stats.level_deaths, s.harmony()),
        });
    }
    for (name, lap) in chute_loops() {
        let mut t = s.clone();
        let mut witness = Vec::new();
        let mut deaths = 0;
        for _ in 0..NERVOUS_DEATHS {
            for &a in &lap {
                t.apply(a);
                witness.push(a);
                deaths += (a == Act::DEATH) as u32;
                if deaths == NERVOUS_DEATHS && !t.grip() {
                    return Err(Violation {
                        prop: "a",
                        witness,
                        detail: format!(
                            "chute loop ({name}): {NERVOUS_DEATHS} more deaths and still no grip (playing {:?}, held {:?})",
                            t.harmony(),
                            t.held()
                        ),
                    });
                }
            }
        }
    }
    Ok(())
}

fn prop_b(s: &Sim) -> Result<(), Violation> {
    for (target, seq) in summons() {
        let mut t = s.clone();
        let mut decided: Vec<Harmony> = Vec::new();
        for &a in &seq {
            t.apply_with(a, |_, d| decided.extend(d.map(|f| f.harmony)));
        }
        let after_target = decided.iter().position(|&h| h == target).map(|i| &decided[i..]);
        let ok = t.harmony() == target && after_target.is_some_and(|r| r.iter().all(|&h| h == target));
        if !ok {
            return Err(Violation {
                prop: "b",
                witness: seq,
                detail: format!("summoning {target:?}: decisions {decided:?}, playing {:?}", t.harmony()),
            });
        }
    }
    Ok(())
}

/// With no input for `n` ticks: the harmony changes (tick, harmony), and the tick the hold
/// ended.
type Quiet = (Vec<(i32, Harmony)>, Option<i32>);

fn quiet(s: &Sim, n: i32) -> Quiet {
    let mut t = s.clone();
    let mut changes = Vec::new();
    let mut unheld = t.held().is_none().then_some(0);
    let mut last = t.harmony();
    for k in 1..=n {
        t.apply(Act::Wait(1));
        if t.harmony() != last {
            last = t.harmony();
            changes.push((k, last));
        }
        if unheld.is_none() && t.held().is_none() {
            unheld = Some(k);
        }
    }
    (changes, unheld)
}

/// How long the no-input runs of (c) and (d) look: a hold ends within a horizon, the check
/// after it within another, and a third shows whether the harmony stays.
const QUIET: i32 = 3 * HORIZON + 4;

fn prop_c(s: &Sim, (changes, _): &Quiet) -> Result<(), Violation> {
    let late = changes.iter().any(|&(k, _)| k > QUIET - HORIZON);
    if changes.len() > 1 || late {
        let upto = changes.get(1).or(changes.last()).map_or(QUIET, |c| c.0);
        return Err(Violation {
            prop: "c",
            witness: vec![Act::Wait(upto as u16)],
            detail: format!(
                "with no input the harmony goes {:?} → {}",
                s.harmony(),
                changes.iter().map(|(k, h)| format!("{h:?} (t+{}s)", *k as f32 * TICK)).collect::<Vec<_>>().join(" → ")
            ),
        });
    }
    Ok(())
}

fn prop_d(s: &Sim, (_, unheld): &Quiet) -> Result<(), Violation> {
    let now = s.now();
    if let Some((h, until)) = s.band.hold
        && until > now + SUMMON_HOLD_SECS + 1e-3
    {
        return Err(Violation {
            prop: "d",
            witness: Vec::new(),
            detail: format!("{h:?} held until t={until}, more than {SUMMON_HOLD_SECS}s ahead"),
        });
    }
    if unheld.is_none_or(|k| k > HOLD_TICKS + 1) {
        return Err(Violation {
            prop: "d",
            witness: vec![Act::Wait(HOLD_TICKS as u16 + 1)],
            detail: format!("still holding {:?} after {SUMMON_HOLD_SECS}s with no input", s.held()),
        });
    }
    // Only deaths and checkpoints (no keep-alive).
    let mut t = s.clone();
    let mut witness = Vec::new();
    while t.tick <= s.tick + HOLD_TICKS {
        for a in [Act::DEATH, Act::Wait(4), Act::CHECKPOINT, Act::Wait(4)] {
            t.apply(a);
            witness.push(a);
        }
    }
    if let Some(h) = t.held() {
        return Err(Violation {
            prop: "d",
            witness,
            detail: format!("still holding {h:?} after {SUMMON_HOLD_SECS}s of only deaths and checkpoints"),
        });
    }
    Ok(())
}

fn prop_e(s: &Sim, init: &Key) -> Result<(), Violation> {
    let mut t = s.clone();
    t.apply(Act::Restart);
    if t.key() != *init || t.grip() || t.playing != Filters::default() {
        return Err(Violation {
            prop: "e",
            witness: vec![Act::Restart],
            detail: format!("after a restart: {:?}\n  a fresh level: {:?}", t.key(), init),
        });
    }
    Ok(())
}

fn prop_f(s: &Sim) -> Result<(), Violation> {
    let laughing = Filters { just_intonation: !s.playing.just_intonation, ..s.playing };
    let other = groove_of(laughing, s.band.stats.level_deaths, s.mutation);
    if other.grip() != s.grip() {
        return Err(Violation {
            prop: "f",
            witness: Vec::new(),
            detail: format!(
                "grip {} with the laughing band {}, {} without",
                s.grip(),
                s.playing.just_intonation,
                other.grip()
            ),
        });
    }
    Ok(())
}

// --- Exploration ---

/// What to explore.
#[derive(Clone, Debug)]
pub struct Config {
    pub depth: usize,
    pub acts: Vec<Act>,
    pub mutation: Mutation,
}

impl Config {
    /// Tier 1: single events per frame, waits for the jump spacings, the respawn and the
    /// horizon.
    pub fn tier1(depth: usize) -> Config {
        let mut acts: Vec<Act> = BITS.iter().map(|&(b, _)| Act::Frame(b)).collect();
        acts.push(Act::Restart);
        acts.extend([1, 2, 3, 4, 10, 40, 80].map(Act::Wait));
        Config { depth, acts, mutation: Mutation::None }
    }

    /// Tier 2: also two events in one frame, and finer waits.
    pub fn deep(depth: usize) -> Config {
        let mut c = Config::tier1(depth);
        for (i, &(a, _)) in BITS.iter().enumerate() {
            for &(b, _) in &BITS[i + 1..] {
                c.acts.push(Act::Frame(a | b));
            }
        }
        c.acts.extend([20, 60].map(Act::Wait));
        c
    }

    pub fn with(mut self, m: Mutation) -> Config {
        self.mutation = m;
        self
    }
}

/// A failure: the trace to the state, the property's witness from it, and why.
#[derive(Debug, Clone)]
pub struct Counterexample {
    pub state: Trace,
    pub violation: Violation,
}

impl fmt::Display for Counterexample {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (_, what) = PROPERTIES.iter().find(|p| p.0 == self.violation.prop).copied().unwrap_or(("?", "?"));
        let mut all = self.state.clone();
        all.0.extend(&self.violation.witness);
        writeln!(f, "property ({}) fails: {what}", self.violation.prop)?;
        writeln!(f, "  state:   {}", self.state)?;
        writeln!(f, "  full:    {all}")?;
        write!(f, "  because: {}", self.violation.detail)
    }
}

/// What an exploration covered.
#[derive(Debug, Clone, Default)]
pub struct Coverage {
    /// Distinct abstract states reached (each checked).
    pub states: usize,
    /// Acts deep the search went (the last layer can be empty: everything seen).
    pub depth: usize,
    /// States per depth.
    pub layers: Vec<usize>,
    pub elapsed: Duration,
    /// Harmonies (with the laughing band) and grip seen playing.
    pub music: HashSet<(Filters, bool)>,
}

struct Node {
    sim: Sim,
    parent: usize,
    act: Option<Act>,
}

/// The reachable states (one representative each) and how to get there.
pub struct Exploration {
    nodes: Vec<Node>,
    pub coverage: Coverage,
}

impl Exploration {
    pub fn trace(&self, mut i: usize) -> Trace {
        let mut acts = Vec::new();
        while let Some(a) = self.nodes[i].act {
            acts.push(a);
            i = self.nodes[i].parent;
        }
        acts.reverse();
        Trace(acts)
    }

    pub fn states(&self) -> impl Iterator<Item = (usize, &Sim)> {
        self.nodes.iter().enumerate().map(|(i, n)| (i, &n.sim))
    }
}

/// Explore breadth-first to `cfg.depth` acts, checking every property from every new state.
/// Stops at the first failure (the shortest trace to a failing state).
pub fn explore(cfg: &Config) -> Result<Exploration, Box<Counterexample>> {
    let (ex, mut found) = explore_with(cfg, 1, true, &|s, init| check_state(s, init));
    match found.pop() {
        Some(c) => Err(Box::new(c)),
        None => Ok(ex),
    }
}

/// Breadth-first to `cfg.depth` acts, `check` from every new state (on `threads` threads, a
/// layer at a time, so the order and the counterexamples don't depend on them). `first`: stop
/// at the first failure; else carry on and keep the first (shortest) failure per property.
pub fn explore_with(
    cfg: &Config,
    threads: usize,
    first: bool,
    check: &(dyn Fn(&Sim, &Key) -> Result<(), Violation> + Sync),
) -> (Exploration, Vec<Counterexample>) {
    let t0 = Instant::now();
    let root = Sim::new(cfg.mutation);
    let init = root.key();
    let mut seen: HashSet<Key> = HashSet::new();
    seen.insert(init.clone());
    let mut ex = Exploration { nodes: vec![Node { sim: root, parent: 0, act: None }], coverage: Coverage::default() };
    let mut found: Vec<Counterexample> = Vec::new();
    let mut layer = vec![0usize];
    ex.coverage.layers.push(1);
    let mut depth = 0;
    loop {
        // Check the layer.
        let nodes = &ex.nodes;
        let chunk = layer.len().div_ceil(threads.max(1)).max(1);
        let run = |part: &[usize]| {
            let mut out = Vec::new();
            for &i in part {
                if let Err(v) = check(&nodes[i].sim, &init) {
                    out.push((i, v));
                    if first {
                        break;
                    }
                }
            }
            out
        };
        // One thread: right here (so its CPU time is this thread's).
        let fails: Vec<(usize, Violation)> = if threads <= 1 {
            run(&layer)
        } else {
            std::thread::scope(|scope| {
                let hs: Vec<_> = layer.chunks(chunk).map(|part| scope.spawn(|| run(part))).collect();
                hs.into_iter().flat_map(|h| h.join().expect("check panicked")).collect()
            })
        };
        for (i, v) in fails {
            if !found.iter().any(|c| c.violation.prop == v.prop) {
                found.push(Counterexample { state: ex.trace(i), violation: v });
            }
            if first {
                break;
            }
        }
        if (first && !found.is_empty()) || depth == cfg.depth {
            break;
        }
        // The next layer.
        depth += 1;
        let mut next = Vec::new();
        for &i in &layer {
            for &a in &cfg.acts {
                let mut s = ex.nodes[i].sim.clone();
                s.apply(a);
                if !seen.insert(s.key()) {
                    continue;
                }
                next.push(ex.nodes.len());
                ex.nodes.push(Node { sim: s, parent: i, act: Some(a) });
            }
        }
        ex.coverage.layers.push(next.len());
        if next.is_empty() {
            break;
        }
        layer = next;
    }
    ex.coverage.states = ex.nodes.len();
    ex.coverage.depth = depth;
    ex.coverage.music = ex.nodes.iter().map(|n| (n.sim.playing, n.sim.grip())).collect();
    ex.coverage.elapsed = t0.elapsed();
    (ex, found)
}

/// Replay `state` and check every property from each state along it (a regression test).
pub fn check_trace(state: &str) -> Result<(), String> {
    let trace = Trace::parse(state)?;
    let init = Sim::new(Mutation::None).key();
    let mut s = Sim::new(Mutation::None);
    let mut done = Vec::new();
    let check = |s: &Sim, done: &[Act]| {
        check_state(s, &init).map_err(|v| Counterexample { state: Trace(done.to_vec()), violation: v }.to_string())
    };
    check(&s, &done)?;
    for &a in &trace.0 {
        s.apply(a);
        done.push(a);
        check(&s, &done)?;
    }
    Ok(())
}

/// The abstraction is sound: concrete states with the same [`Key`] (all reached within `depth`
/// acts, no deduplication) behave the same under `probes` pseudo-random plays of `len` acts:
/// the same decisions, music, grip and holds every frame. Returns the pairs compared.
pub fn check_abstraction(depth: usize, probes: usize, len: usize) -> Result<usize, String> {
    let acts = Config::tier1(depth).acts;
    let mut layer = vec![(Trace::default(), Sim::new(Mutation::None))];
    let mut reps: std::collections::HashMap<Key, (Trace, Sim)> = std::collections::HashMap::new();
    let mut pairs = 0;
    let mut seed = 0x9e3779b97f4a7c15u64;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let plays: Vec<Vec<Act>> =
        (0..probes).map(|_| (0..len).map(|_| acts[(rnd() % acts.len() as u64) as usize]).collect()).collect();
    let observe = |s: &Sim, play: &[Act]| {
        let mut t = s.clone();
        let mut seen = Vec::new();
        for &a in play {
            t.apply_with(a, |x, d| seen.push((d, x.playing, x.grip(), x.held())));
        }
        seen
    };
    for d in 0..=depth {
        let mut next = Vec::new();
        for (trace, s) in layer {
            match reps.get(&s.key()) {
                Some((rt, r)) => {
                    pairs += 1;
                    for play in &plays {
                        if observe(r, play) != observe(&s, play) {
                            return Err(format!(
                                "`{rt}` and `{trace}` share a key but differ under `{}`",
                                Trace(play.clone())
                            ));
                        }
                    }
                }
                None => {
                    reps.insert(s.key(), (trace.clone(), s.clone()));
                }
            }
            if d < depth {
                for &a in &acts {
                    let mut t = s.clone();
                    t.apply(a);
                    let mut tr = trace.clone();
                    tr.0.push(a);
                    next.push((tr, t));
                }
            }
        }
        layer = next;
    }
    Ok(pairs)
}

/// CPU time this thread has used (Linux: `/proc/thread-self/schedstat`), so a time budget
/// holds however busy the machine is. `None` elsewhere.
pub fn thread_cpu() -> Option<Duration> {
    let s = std::fs::read_to_string("/proc/thread-self/schedstat").ok()?;
    let ns: u64 = s.split_whitespace().next()?.parse().ok()?;
    Some(Duration::from_nanos(ns))
}

/// Time `f`: (its result, CPU time if known else wall time, wall time).
pub fn timed<T>(f: impl FnOnce() -> T) -> (T, Duration, Duration) {
    let (c0, w0) = (thread_cpu(), Instant::now());
    let out = f();
    let wall = w0.elapsed();
    let cpu = match (c0, thread_cpu()) {
        (Some(a), Some(b)) => b.saturating_sub(a),
        _ => wall,
    };
    (out, cpu, wall)
}
