//! The "band director": picks the [`Filters`] from how the player is playing.
//!
//! # Charge, then call
//! Three counters charge from play, each over a rolling window of the last
//! [`MUSIC_CHECK_SECS`] (shorter at the start of a level):
//! - toots (double jumps) ≥ [`GIANT_STEPS_TOOTS`] charge Giant Steps (Coltrane);
//! - nuggets ≥ [`FIRED_UP_NUGGETS`] at ≥ 1 per [`FIRED_UP_SECS_PER_NUGGET`]s charge the
//!   fired-up band (quartal);
//! - a *jump in threes* (see [`ThreeStep`]): three consecutive ground jumps whose takeoffs are
//!   evenly spaced (the two intervals within ±[`WALTZ_EVENNESS`] of each other, each
//!   [`WALTZ_MIN_INTERVAL`]–[`WALTZ_MAX_INTERVAL`]s) charges the waltz. Toots break the count,
//!   and so does a death.
//!
//! A full counter stops draining and holds its charge (through deaths and checkpoints, until a
//! restart). Nothing plays until the player *calls* the band ([`Events::calls`], Down): the
//! call plays the [`Band::ready`] mode, the full counter ticked most recently (a toot, a
//! nugget, a timed hop), so ticking steers which one a call plays. The call switches at once
//! (the music at the next bar line) and starts a *hold*:
//! - the mode holds for [`SUMMON_HOLD_SECS`]; its own move (a toot in Giant Steps, a nugget
//!   fired up, a jump in threes in the waltz) refills the hold to that long from then, and
//!   doesn't count toward its own counter meanwhile;
//! - the other counters keep charging, and calling one of them switches to it;
//! - deaths and checkpoints during the hold don't change the harmony (the laughing band's
//!   tuning can still come and go on top), nor count toward the nervous band;
//! - when the hold runs out the band decides afresh: back to the chart, or nervous.
//!
//! Other decisions happen at a level start (always plain), after a death (decided as the player
//! dies, so the new version renders during the respawn delay and switches in afterwards), on
//! reaching a checkpoint, and every [`MUSIC_CHECK_SECS`] of play (pause excluded) since the
//! last decision. Everything else (title, level select, the jingle, victory) is plain. A
//! decision that comes out the same as what's playing changes nothing.
//!
//! The music also bends the physics (`crate::game::Groove`), and levels have gates that need a
//! mode: giant walls need Giant Steps, long gaps a fired-up band, glide launches a waltzing one.
//! A charge never decays and a call always plays, so a gate's mode is always one call away.
//!
//! Rules ([`choose_filters`], when nothing is held), harmony first:
//! 1. deaths since the level start or the last call, while nothing was held
//!    ([`PlayStats::mood_deaths`]) ≥ [`NERVOUS_DEATHS`] → melodic minor, "THE BAND IS
//!    NERVOUS". That count only grows until the next call, so once nervous the band stays
//!    nervous until a call or a restart;
//! 2. otherwise as written.
//!
//! The laughing band (the tuning medley, [`Filters::just_intonation`]) plays when *slips*
//! since the last checkpoint ≥ [`LAUGHING_SLIPS`] ("THE BAND CAN'T STOP LAUGHING"): a slip
//! ([`Events::slips`]) is a death at something the game rates easy for the player (the
//! adaptive engine's per-skill judgement, `crate::game::adaptive`), never a designed one (a
//! stain pit, a grease chute). It wears off after [`LAUGH_DECAY_SECS`] without another, and
//! goes with any harmony, the waltz included.

use std::collections::VecDeque;

use super::{Filters, Harmony};

/// Seconds of play between periodic checks, and the length of the rolling stats window.
pub const MUSIC_CHECK_SECS: f32 = 20.0;

pub const NERVOUS_DEATHS: u32 = 3;
pub const GIANT_STEPS_TOOTS: u32 = 5;
pub const FIRED_UP_NUGGETS: u32 = 4;
pub const FIRED_UP_SECS_PER_NUGGET: f32 = 5.0;
/// Slips since the checkpoint that set the band laughing.
pub const LAUGHING_SLIPS: u32 = 1;
/// A called mode holds this long (play seconds), and each use of its move refills the hold to
/// this long from then.
pub const SUMMON_HOLD_SECS: f32 = MUSIC_CHECK_SECS;
/// Jump in threes: each interval between takeoffs, seconds.
pub const WALTZ_MIN_INTERVAL: f32 = 0.35;
pub const WALTZ_MAX_INTERVAL: f32 = 1.2;
/// Jump in threes: the longer interval at most this much (relative) longer than the shorter.
pub const WALTZ_EVENNESS: f32 = 0.2;

pub const REASON_NERVOUS: &str = "THE BAND IS NERVOUS";
pub const REASON_GIANT_STEPS: &str = "GIANT STEPS!";
pub const REASON_FIRED_UP: &str = "THE BAND IS FIRED UP";
pub const REASON_WALTZ: &str = "THE BAND WALTZES";
pub const REASON_LAUGHING: &str = "THE BAND CAN'T STOP LAUGHING";

/// The summonable harmonies, in the HUD's order (toots, nuggets, hops).
pub const SUMMON_ORDER: [Harmony; 3] = [Harmony::Coltrane, Harmony::Quartal, Harmony::Waltz];

/// What the director knows about the current level.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PlayStats {
    /// Deaths since the level started.
    pub level_deaths: u32,
    /// Deaths since the level started or the last call, not counting those while a called
    /// mode was held (the nervous rule).
    pub mood_deaths: u32,
    /// Slips since the last checkpoint (or the level start), less those worn off, at most
    /// [`LAUGHING_SLIPS`] (the laughing band).
    pub checkpoint_slips: u32,
    /// In the rolling window:
    pub stretch_toots: u32,
    pub stretch_nuggets: u32,
    pub stretch_deaths: u32,
    /// Jumps in threes completed (overlapping: a 4th even jump completes another).
    pub stretch_waltzes: u32,
    /// Length of the window: [`MUSIC_CHECK_SECS`], or less early in a level.
    pub stretch_secs: f32,
    /// The mode called most recently this level.
    pub last_summon: Option<Harmony>,
}

impl PlayStats {
    /// Does the window hold enough to charge `h`?
    pub fn met(&self, h: Harmony) -> bool {
        match h {
            Harmony::Coltrane => self.stretch_toots >= GIANT_STEPS_TOOTS,
            Harmony::Quartal => {
                self.stretch_nuggets >= FIRED_UP_NUGGETS
                    && self.stretch_secs <= self.stretch_nuggets as f32 * FIRED_UP_SECS_PER_NUGGET
            }
            Harmony::Waltz => self.stretch_waltzes >= 1,
            Harmony::Original | Harmony::MelodicMinor => false,
        }
    }
}

/// Timestamped (play time) toots, nuggets, deaths and waltz steps of the last
/// [`MUSIC_CHECK_SECS`], with their running totals (so filling the stats every frame doesn't
/// re-add the whole window: the formal checks step the band tens of millions of times).
#[derive(Debug, Clone, Default)]
pub struct Window {
    entries: VecDeque<(f32, [u32; 4])>,
    /// The sum of `entries`.
    total: [u32; 4],
}

impl Window {
    /// Note what happened at play time `now` (and forget what's too old).
    pub fn record(&mut self, now: f32, toots: u32, nuggets: u32, deaths: u32, waltzes: u32) {
        let c = [toots, nuggets, deaths, waltzes];
        if c != [0; 4] {
            self.entries.push_back((now, c));
            self.total = std::array::from_fn(|k| self.total[k] + c[k]);
        }
        while let Some(&(t, c)) = self.entries.front()
            && t < now - MUSIC_CHECK_SECS
        {
            self.total = std::array::from_fn(|k| self.total[k] - c[k]);
            self.entries.pop_front();
        }
    }

    /// The entries remembered: (play time, [toots, nuggets, deaths, waltz steps]), oldest
    /// first (for the formal checks in `tests/formal.rs`, which hash the band's state).
    pub fn entries(&self) -> impl Iterator<Item = (f32, [u32; 4])> + '_ {
        self.entries.iter().copied()
    }

    /// Forget a summon's counter (column 0 toots, 1 nuggets, 3 waltz steps).
    pub fn clear(&mut self, column: usize) {
        for e in &mut self.entries {
            e.1[column] = 0;
        }
        self.total[column] = 0;
    }

    /// Fill the window fields of `stats` as of `now` (the level started at `level_start`).
    pub fn fill(&self, stats: &mut PlayStats, now: f32, level_start: f32) {
        // The total, less the oldest entries if they've aged out since the last record.
        let mut c = self.total;
        let mut i = 0;
        while let Some(&(t, e)) = self.entries.get(i)
            && t < now - MUSIC_CHECK_SECS
        {
            c = std::array::from_fn(|k| c[k] - e[k]);
            i += 1;
        }
        stats.stretch_toots = c[0];
        stats.stretch_nuggets = c[1];
        stats.stretch_deaths = c[2];
        stats.stretch_waltzes = c[3];
        stats.stretch_secs = (now - level_start).clamp(0.0, MUSIC_CHECK_SECS);
    }
}

/// Are three takeoffs at `a < b < c` (seconds) a jump in threes?
pub fn even_threes(a: f32, b: f32, c: f32) -> bool {
    let (i, j) = (b - a, c - b);
    let ok = |x: f32| (WALTZ_MIN_INTERVAL..=WALTZ_MAX_INTERVAL).contains(&x);
    ok(i) && ok(j) && i.max(j) <= i.min(j) * (1.0 + WALTZ_EVENNESS) + 1e-4
}

/// Spots the jump in threes in the ground-jump takeoff times.
#[derive(Debug, Clone, Default)]
pub struct ThreeStep {
    /// The last two takeoffs.
    last: [Option<f32>; 2],
}

impl ThreeStep {
    /// A ground jump took off at play time `t`. True when it completes a jump in threes with
    /// the two before it (so waltzing on, every further even jump completes another).
    pub fn jump(&mut self, t: f32) -> bool {
        let done = matches!(self.last, [Some(a), Some(b)] if even_threes(a, b, t));
        self.last = [self.last[1], Some(t)];
        done
    }

    /// The last two takeoffs, older first (for the formal checks).
    pub fn takeoffs(&self) -> [Option<f32>; 2] {
        self.last
    }

    /// Takeoffs toward the next jump in threes at play time `now` (the HUD's pips, 0..=2): the
    /// last one while a next jump could still be on time, and the one before if it was.
    pub fn progress(&self, now: f32) -> u32 {
        let ok = |i: f32| (WALTZ_MIN_INTERVAL..=WALTZ_MAX_INTERVAL).contains(&i);
        match self.last {
            [a, Some(b)] if now - b <= WALTZ_MAX_INTERVAL => 1 + a.is_some_and(|a| ok(b - a)) as u32,
            _ => 0,
        }
    }

    /// Start counting afresh (a death, a new level).
    pub fn reset(&mut self) {
        self.last = [None, None];
    }
}

fn reason_of(h: Harmony) -> &'static str {
    match h {
        Harmony::Coltrane => REASON_GIANT_STEPS,
        Harmony::Quartal => REASON_FIRED_UP,
        Harmony::Waltz => REASON_WALTZ,
        Harmony::MelodicMinor => REASON_NERVOUS,
        Harmony::Original => "",
    }
}

/// The filters for the stats when nothing is held, and why (the reason of the harmony rule if
/// one fired, else the laughing band's reason, else ""). The summonable modes come only from
/// a call ([`Band`]).
pub fn choose_filters(s: &PlayStats) -> (Filters, &'static str) {
    let harmony = if s.mood_deaths >= NERVOUS_DEATHS { Harmony::MelodicMinor } else { Harmony::Original };
    let reason = reason_of(harmony);
    let just_intonation = s.checkpoint_slips >= LAUGHING_SLIPS;
    let reason = if reason.is_empty() && just_intonation { REASON_LAUGHING } else { reason };
    (Filters { harmony, just_intonation }, reason)
}

/// Harmonies the player calls up on purpose (levels have gates that need them).
pub fn summonable(h: Harmony) -> bool {
    SUMMON_ORDER.contains(&h)
}

/// What the player did in one frame.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Events {
    pub toots: u32,
    /// Jumps off the ground (not toots).
    pub ground_jumps: u32,
    pub nuggets: u32,
    pub deaths: u32,
    /// Of `deaths`, those at something the game rates easy for the player (the laughing band).
    pub slips: u32,
    pub checkpoints: u32,
    /// Jumps in threes spotted elsewhere (an editor's "waltz step" button): each counts as one
    /// completed, on top of those [`Band`] spots in `ground_jumps`.
    pub waltz_steps: u32,
    /// Calls to the band (Down): play the [`Band::ready`] mode.
    pub calls: u32,
    /// A call for this mode by name (Side B's set list, `game::sideb`): plays it, charged or
    /// not.
    pub call_mode: Option<Harmony>,
}

/// One counter of the HUD's strip ([`Band::meters`]).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Meter {
    /// Pips lit, of `of`.
    pub lit: u32,
    pub of: u32,
    /// Charged: a call can play it.
    pub full: bool,
    /// Playing (called): the share of its hold left, 0..=1.
    pub active: Option<f32>,
}

/// The HUD's counter strip: toots, nuggets, hops in threes (the callable modes, in
/// [`SUMMON_ORDER`]), then deaths (the nervous band, never callable).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Meters {
    pub summons: [Meter; 3],
    pub deaths: Meter,
    /// The mode a call would play (the CALL arrow).
    pub ready: Option<Harmony>,
}

/// The band's memory of a level: the stats, the rolling window, the jump-in-threes watch, the
/// charged counters and the mode being held. [`Band::step`] takes each frame's events and
/// returns a decision when one is due (see the module docs).
#[derive(Debug, Clone, Default)]
pub struct Band {
    pub stats: PlayStats,
    pub window: Window,
    pub steps: ThreeStep,
    /// The called mode being held, and until when (play time).
    pub hold: Option<(Harmony, f32)>,
    /// Jumps in threes completed this level (the music hears each as a waltz step).
    pub steps_taken: u32,
    /// Per [`SUMMON_ORDER`]: charged, and when its counter was last ticked (the count of
    /// frames with a tick by then, so the order holds within one play-time instant too).
    charged: [Option<u64>; 3],
    /// Frames so far this level with a tick (a toot, a nugget, a jump in threes).
    frames_ticked: u64,
    level_start: f32,
    next_check: f32,
    /// When the laughing band's slip count next eases off by one (see [`LAUGH_DECAY_SECS`]).
    laugh_decay_at: f32,
}

/// The laughing band wears off: every this many seconds without a slip, one slip comes off
/// the since-checkpoint count (so a slip then 10 seconds without another bring equal
/// temperament back before the next checkpoint). Grip is a level-long safety net and doesn't
/// decay.
pub const LAUGH_DECAY_SECS: f32 = 10.0;

/// The rolling-window column of a summon's counter.
fn column(h: Harmony) -> Option<usize> {
    match h {
        Harmony::Coltrane => Some(0),
        Harmony::Quartal => Some(1),
        Harmony::Waltz => Some(3),
        _ => None,
    }
}

/// A summonable harmony's index in [`SUMMON_ORDER`].
fn slot(h: Harmony) -> Option<usize> {
    SUMMON_ORDER.iter().position(|&s| s == h)
}

impl Band {
    /// A level (re)starts at play time `now`: everything resets, the band plays it straight.
    pub fn start(&mut self, now: f32) -> (Filters, &'static str) {
        *self = Band { level_start: now, next_check: now + MUSIC_CHECK_SECS, ..Band::default() };
        (Filters::default(), "")
    }

    /// Play time the level (re)started (for the formal checks).
    pub fn level_start(&self) -> f32 {
        self.level_start
    }

    /// When the laughing count next eases off (for the formal checks).
    pub fn laugh_decay_at(&self) -> f32 {
        self.laugh_decay_at
    }

    /// Play time of the next periodic check.
    pub fn next_check(&self) -> f32 {
        self.next_check
    }

    /// The mode held at `now`, if any.
    pub fn held(&self, now: f32) -> Option<Harmony> {
        self.hold.filter(|&(_, until)| now < until).map(|(h, _)| h)
    }

    /// The charged modes, the one a call plays ([`Band::ready`]) last.
    pub fn charged(&self) -> Vec<Harmony> {
        let mut c: Vec<(u64, usize)> = (0..3).filter_map(|i| self.charged[i].map(|t| (t, i))).collect();
        c.sort();
        c.into_iter().map(|(_, i)| SUMMON_ORDER[i]).collect()
    }

    /// The mode a call plays: the charged one ticked most recently (ties: the later in
    /// [`SUMMON_ORDER`]).
    pub fn ready(&self) -> Option<Harmony> {
        self.charged().last().copied()
    }

    /// The HUD's counters at `now`.
    pub fn meters(&self, now: f32) -> Meters {
        let held = self.hold.filter(|&(_, until)| now < until);
        let lit = [self.stats.stretch_toots, self.stats.stretch_nuggets, self.steps.progress(now)];
        let of = [GIANT_STEPS_TOOTS, FIRED_UP_NUGGETS, 3];
        let summons = std::array::from_fn(|i| {
            let full = self.charged[i].is_some();
            let active = held.filter(|&(h, _)| h == SUMMON_ORDER[i]).map(|(_, until)| ((until - now) / SUMMON_HOLD_SECS).clamp(0.0, 1.0));
            Meter { lit: if full { of[i] } else { lit[i].min(of[i]) }, of: of[i], full, active }
        });
        let deaths = self.stats.mood_deaths.min(NERVOUS_DEATHS);
        let deaths = Meter { lit: deaths, of: NERVOUS_DEATHS, full: deaths >= NERVOUS_DEATHS, active: None };
        Meters { summons, deaths, ready: self.ready() }
    }

    /// One frame's events at play time `now`; returns a decision if one is due.
    pub fn step(&mut self, now: f32, ev: Events) -> Option<(Filters, &'static str)> {
        // Ground jumps: a jump in threes is a waltz step.
        let mut waltzes = ev.waltz_steps;
        for _ in 0..ev.ground_jumps {
            waltzes += self.steps.jump(now) as u32;
        }
        self.steps_taken += waltzes;
        // A death, or a toot, breaks a jump in threes: waltz steps are plain hops, so a
        // jump-toot rhythm on a giant wall's runway charges Giant Steps, not the waltz.
        if ev.deaths > 0 || ev.toots > 0 {
            self.steps.reset();
        }
        // Each mode's ticks: the held mode's refill its hold, a charged one's steer the call,
        // the rest count.
        let held = self.held(now);
        let mut ticks = [ev.toots, ev.nuggets, waltzes];
        if ticks != [0; 3] {
            self.frames_ticked += 1;
        }
        let frame = self.frames_ticked;
        for (i, &h) in SUMMON_ORDER.iter().enumerate() {
            if ticks[i] == 0 {
                continue;
            }
            if held == Some(h) {
                self.hold = Some((h, now + SUMMON_HOLD_SECS));
                ticks[i] = 0;
            } else if self.charged[i].is_some() {
                self.charged[i] = Some(frame);
                ticks[i] = 0;
            }
        }
        self.window.record(now, ticks[0], ticks[1], ev.deaths, ticks[2]);
        self.stats.level_deaths += ev.deaths;
        let slips = ev.slips.min(ev.deaths);
        // (Capped: the band laughs until the last slip wears off, however many there were.)
        self.stats.checkpoint_slips = (self.stats.checkpoint_slips + slips).min(LAUGHING_SLIPS);
        // The laughing band wears off without more slips.
        let mut sobered = false;
        if slips > 0 {
            self.laugh_decay_at = now + LAUGH_DECAY_SECS;
        } else if self.stats.checkpoint_slips > 0 && now >= self.laugh_decay_at {
            sobered = self.stats.checkpoint_slips == LAUGHING_SLIPS;
            self.stats.checkpoint_slips -= 1;
            self.laugh_decay_at = now + LAUGH_DECAY_SECS;
        }
        // Deaths while a mode is held don't count toward a mood.
        if held.is_none() {
            self.stats.mood_deaths += ev.deaths;
        }
        self.window.fill(&mut self.stats, now, self.level_start);
        // Counters that just filled charge (and stop counting).
        if ticks != [0; 3] {
            let mut cleared = false;
            for (i, &h) in SUMMON_ORDER.iter().enumerate() {
                if ticks[i] > 0 && self.stats.met(h) {
                    self.charged[i] = Some(frame);
                    self.window.clear(column(h).expect("summonable"));
                    cleared = true;
                }
            }
            if cleared {
                self.window.fill(&mut self.stats, now, self.level_start);
            }
        }
        // A call plays the ready mode.
        let called = ev.call_mode.filter(|&h| summonable(h)).or(if ev.calls > 0 { self.ready() } else { None });
        if let Some(h) = called {
            self.begin(h, now);
        }
        let expired = self.hold.is_some_and(|(_, until)| now >= until);
        if !(called.is_some() || expired || sobered || ev.deaths > 0 || ev.checkpoints > 0 || now >= self.next_check) {
            return None;
        }
        let decided = self.decide(now);
        self.next_check = now + MUSIC_CHECK_SECS;
        if ev.checkpoints > 0 {
            self.stats.checkpoint_slips = 0;
        }
        Some(decided)
    }

    /// A call of `h` starts: its charge spent, its hold started, the mood forgotten.
    fn begin(&mut self, h: Harmony, now: f32) {
        self.charged[slot(h).expect("summonable")] = None;
        self.hold = Some((h, now + SUMMON_HOLD_SECS));
        self.stats.mood_deaths = 0;
        self.stats.last_summon = Some(h);
    }

    fn decide(&mut self, now: f32) -> (Filters, &'static str) {
        let (mut filters, reason) = choose_filters(&self.stats);
        match self.held(now) {
            Some(h) => {
                filters.harmony = h;
                let r = reason_of(h);
                (filters, r)
            }
            None => {
                self.hold = None;
                (filters, reason)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(level_deaths: u32, checkpoint_slips: u32, toots: u32, nuggets: u32, secs: f32) -> PlayStats {
        PlayStats {
            level_deaths,
            mood_deaths: level_deaths,
            checkpoint_slips,
            stretch_toots: toots,
            stretch_nuggets: nuggets,
            stretch_secs: secs,
            ..PlayStats::default()
        }
    }

    #[test]
    fn rules_table() {
        use Harmony::*;
        // Full counters alone never play a mode: only a call does.
        let cases: &[(PlayStats, Harmony, bool, &str)] = &[
            (st(0, 0, 0, 0, 10.0), Original, false, ""),
            (st(0, 0, 9, 9, 1.0), Original, false, ""),
            (st(3, 0, 9, 9, 1.0), MelodicMinor, false, REASON_NERVOUS),
            (st(2, 1, 0, 0, 30.0), Original, true, REASON_LAUGHING),
            (st(2, 0, 0, 0, 30.0), Original, false, ""),
            (st(3, 1, 0, 0, 30.0), MelodicMinor, true, REASON_NERVOUS),
        ];
        for (s, h, ji, why) in cases {
            let (f, reason) = choose_filters(s);
            assert_eq!((f.harmony, f.just_intonation, reason), (*h, *ji, *why), "{s:?}");
        }
    }

    #[test]
    fn what_charges() {
        use Harmony::*;
        assert!(st(0, 0, 5, 0, 20.0).met(Coltrane));
        assert!(!st(0, 0, 4, 0, 20.0).met(Coltrane));
        // Quick nuggets only.
        assert!(st(0, 0, 0, 4, 12.0).met(Quartal));
        assert!(st(0, 0, 0, 4, 20.0).met(Quartal));
        assert!(!st(0, 0, 0, 4, 20.1).met(Quartal));
        assert!(!st(0, 0, 0, 3, 1.0).met(Quartal));
        assert!(PlayStats { stretch_waltzes: 1, ..st(0, 0, 0, 0, 10.0) }.met(Waltz));
    }

    #[test]
    fn the_window_rolls() {
        let mut w = Window::default();
        let mut s = PlayStats::default();
        for k in 0..5 {
            w.record(1.0 + k as f32, 1, 0, 0, 0);
        }
        w.record(10.0, 0, 2, 1, 1);
        w.fill(&mut s, 10.0, 0.0);
        assert_eq!((s.stretch_toots, s.stretch_nuggets, s.stretch_deaths, s.stretch_waltzes, s.stretch_secs), (5, 2, 1, 1, 10.0));
        // 22s later the early toots have aged out.
        w.record(22.5, 0, 0, 0, 0);
        w.fill(&mut s, 22.5, 0.0);
        assert_eq!((s.stretch_toots, s.stretch_nuggets, s.stretch_waltzes, s.stretch_secs), (3, 2, 1, 20.0));
        // A cleared column stays cleared.
        w.clear(1);
        w.fill(&mut s, 22.5, 0.0);
        assert_eq!((s.stretch_toots, s.stretch_nuggets, s.stretch_deaths), (3, 0, 1));
        // Aged out before the next record too.
        w.fill(&mut s, 40.0, 0.0);
        assert_eq!((s.stretch_toots, s.stretch_nuggets, s.stretch_waltzes), (0, 0, 0));
    }

    #[test]
    fn jump_in_threes() {
        let threes = |times: &[f32]| {
            let mut s = ThreeStep::default();
            times.iter().map(|&t| s.jump(t)).collect::<Vec<bool>>()
        };
        // Evenly spaced: the 3rd jump completes it, and every even one after.
        assert_eq!(threes(&[1.0, 1.6, 2.2, 2.8]), [false, false, true, true]);
        assert_eq!(threes(&[0.0, 0.35, 0.7]), [false, false, true], "as fast as allowed");
        assert_eq!(threes(&[0.0, 1.2, 2.4]), [false, false, true], "as slow as allowed");
        assert_eq!(threes(&[0.0, 0.5, 1.1]), [false, false, true], "0.6 is within 20% of 0.5");
        // Uneven, too fast, too slow: no.
        assert_eq!(threes(&[0.0, 0.5, 1.15]), [false; 3], "0.65 vs 0.5 is 30% off");
        assert_eq!(threes(&[0.0, 0.8, 1.3]), [false; 3]);
        assert_eq!(threes(&[0.0, 0.25, 0.5]), [false; 3], "too fast (bunny hops)");
        assert_eq!(threes(&[0.0, 1.5, 3.0]), [false; 3], "too slow");
        // A stumble breaks it; the next even three counts again.
        assert_eq!(threes(&[0.0, 0.6, 1.6, 2.2, 2.8]), [false, false, false, false, true]);
        let mut s = ThreeStep::default();
        s.jump(0.0);
        s.jump(0.6);
        s.reset();
        assert!(!s.jump(1.2), "a death resets the count");
    }

    #[test]
    fn three_step_progress() {
        let mut s = ThreeStep::default();
        assert_eq!(s.progress(0.0), 0);
        s.jump(1.0);
        assert_eq!((s.progress(1.1), s.progress(2.1), s.progress(2.3)), (1, 1, 0), "a missed beat drops it");
        s.jump(1.6);
        assert_eq!(s.progress(1.7), 2);
        s.jump(3.5);
        assert_eq!(s.progress(3.6), 1, "too late for the one before");
    }

    // --- Charge, then call (Band) ---

    const TOOT: Events = Events { toots: 1, ..NOTHING };
    const HOP: Events = Events { ground_jumps: 1, ..NOTHING };
    const NUGGET: Events = Events { nuggets: 1, ..NOTHING };
    const DEATH: Events = Events { deaths: 1, ..NOTHING };
    const SLIP: Events = Events { deaths: 1, slips: 1, ..NOTHING };
    const CHECKPOINT: Events = Events { checkpoints: 1, ..NOTHING };
    const CALL: Events = Events { calls: 1, ..NOTHING };
    const NOTHING: Events = Events { toots: 0, ground_jumps: 0, nuggets: 0, deaths: 0, slips: 0, checkpoints: 0, waltz_steps: 0, calls: 0, call_mode: None };

    fn harmony(d: Option<(Filters, &'static str)>) -> Option<Harmony> {
        d.map(|d| d.0.harmony)
    }

    /// A band 5 toots into a level (t = 1..=5): Giant Steps charged, not playing.
    fn charged_giant_steps() -> Band {
        let mut b = Band::default();
        b.start(0.0);
        for t in 1..=5 {
            assert_eq!(b.step(t as f32, TOOT), None, "a full counter doesn't play by itself");
        }
        assert_eq!(b.ready(), Some(Harmony::Coltrane));
        b
    }

    /// Giant Steps called at t = 6.
    fn giant_steps() -> Band {
        let mut b = charged_giant_steps();
        let d = b.step(6.0, CALL).expect("a call decides at once");
        assert_eq!(d, (Filters { harmony: Harmony::Coltrane, just_intonation: false }, REASON_GIANT_STEPS));
        b
    }

    #[test]
    fn a_full_counter_holds_its_charge() {
        let mut b = charged_giant_steps();
        let m = b.meters(5.0);
        assert_eq!(m.summons[0], Meter { lit: 5, of: 5, full: true, active: None });
        assert_eq!(m.ready, Some(Harmony::Coltrane));
        // It doesn't drain, through deaths and checkpoints and long waits.
        b.step(10.0, DEATH);
        b.step(12.0, CHECKPOINT);
        for t in 13..80 {
            b.step(t as f32, NOTHING);
        }
        assert_eq!(b.ready(), Some(Harmony::Coltrane));
        assert!(b.meters(80.0).summons[0].full);
        // A restart forgets it.
        b.start(81.0);
        assert_eq!(b.ready(), None);
    }

    #[test]
    fn a_counter_drains_until_full() {
        let mut b = Band::default();
        b.start(0.0);
        for t in 1..=4 {
            b.step(t as f32, TOOT);
        }
        assert_eq!(b.meters(4.0).summons[0].lit, 4);
        b.step(23.5, NOTHING);
        assert_eq!(b.meters(23.5).summons[0].lit, 1, "the first three aged out");
        assert_eq!(b.ready(), None);
        // Slow nuggets never fill.
        for t in [30.0, 37.0, 44.0, 51.0, 58.0] {
            b.step(t, NUGGET);
        }
        assert_eq!(b.ready(), None);
    }

    #[test]
    fn a_call_with_nothing_full_does_nothing() {
        let mut b = Band::default();
        b.start(0.0);
        b.step(1.0, TOOT);
        assert_eq!(b.step(2.0, CALL), None);
        assert_eq!(b.hold, None);
    }

    #[test]
    fn a_call_spends_the_charge() {
        let mut b = giant_steps();
        assert_eq!(b.ready(), None);
        assert_eq!(b.hold, Some((Harmony::Coltrane, 6.0 + SUMMON_HOLD_SECS)));
        let m = b.meters(16.0);
        assert_eq!(m.summons[0].active, Some(0.5));
        assert!(!m.summons[0].full);
        // Toots in Giant Steps refill its timer and don't charge it again.
        for t in 7..=12 {
            assert_eq!(b.step(t as f32, TOOT), None);
        }
        assert_eq!((b.stats.stretch_toots, b.ready()), (0, None));
        assert_eq!(b.hold, Some((Harmony::Coltrane, 12.0 + SUMMON_HOLD_SECS)));
        // Calling again does nothing.
        assert_eq!(b.step(13.0, CALL), None);
    }

    #[test]
    fn the_last_ticked_full_counter_is_ready() {
        use Harmony::*;
        let mut b = charged_giant_steps();
        for t in [6.0, 6.5, 7.0, 7.5] {
            b.step(t, NUGGET);
        }
        assert_eq!(b.charged(), [Coltrane, Quartal]);
        assert_eq!(b.ready(), Some(Quartal), "the nugget counter filled last");
        b.step(8.0, TOOT);
        assert_eq!(b.ready(), Some(Coltrane), "a toot steers it back");
        b.step(8.5, NUGGET);
        assert_eq!(b.ready(), Some(Quartal));
        // A hop that isn't a jump in threes doesn't tick the waltz.
        b.step(9.0, HOP);
        assert_eq!(b.ready(), Some(Quartal));
        assert_eq!(harmony(b.step(9.5, CALL)), Some(Quartal));
        // The other charge stays, and a call switches to it.
        assert_eq!(b.ready(), Some(Coltrane));
        assert_eq!(harmony(b.step(10.0, CALL)), Some(Coltrane));
        assert_eq!(b.ready(), None);
    }

    #[test]
    fn jumps_in_threes_charge_the_waltz() {
        let mut b = Band::default();
        b.start(0.0);
        b.step(1.0, HOP);
        b.step(1.6, HOP);
        assert_eq!(b.meters(1.7).summons[2].lit, 2);
        b.step(2.2, HOP);
        assert_eq!(b.meters(2.3).summons[2], Meter { lit: 3, of: 3, full: true, active: None });
        assert_eq!(harmony(b.step(3.0, CALL)), Some(Harmony::Waltz));
        // Waltz steps keep the waltz going.
        b.step(15.0, HOP);
        b.step(15.6, HOP);
        assert_eq!(b.hold, Some((Harmony::Waltz, 23.0)), "a stumble doesn't refill it");
        b.step(16.2, HOP);
        assert_eq!(b.hold, Some((Harmony::Waltz, 36.2)), "an even jump does");
    }

    #[test]
    fn deaths_and_checkpoints_dont_change_a_held_harmony() {
        let mut b = giant_steps();
        // Three deaths would make the band nervous, a slip laughing.
        assert_eq!(b.step(7.0, DEATH), Some((Filters { harmony: Harmony::Coltrane, just_intonation: false }, REASON_GIANT_STEPS)));
        let d = b.step(8.0, SLIP).unwrap();
        assert_eq!(d.0, Filters { harmony: Harmony::Coltrane, just_intonation: true }, "the laughing band layers on top");
        assert_eq!(harmony(b.step(9.0, DEATH)), Some(Harmony::Coltrane));
        assert_eq!(harmony(b.step(10.0, CHECKPOINT)), Some(Harmony::Coltrane));
        // The hold runs out: the band decides afresh right then. The deaths came during the
        // hold, so the band isn't nervous: back to the chart.
        assert_eq!(b.step(25.75, NOTHING), None);
        assert_eq!(harmony(b.step(26.0, NOTHING)), Some(Harmony::Original));
        assert_eq!(b.hold, None);
    }

    #[test]
    fn no_flip_flopping() {
        use Harmony::*;
        let mut b = Band::default();
        b.start(0.0);
        // (time, events) and the harmony the band plays after each decision.
        let script: &[(f32, Events, Option<Harmony>)] = &[
            (1.0, TOOT, None),
            (1.5, TOOT, None),
            (2.0, TOOT, None),
            (2.5, TOOT, None),
            (3.0, TOOT, None),
            (3.5, CALL, Some(Coltrane)),
            (6.0, DEATH, Some(Coltrane)),
            (9.0, DEATH, Some(Coltrane)),
            (10.0, NUGGET, None),
            (11.0, NUGGET, None),
            (12.0, NUGGET, None),
            (13.0, NUGGET, None),
            (13.5, CALL, Some(Quartal)),
            (15.0, DEATH, Some(Quartal)),
            (20.0, DEATH, Some(Quartal)),
            (30.0, DEATH, Some(Quartal)),
            // The quartal hold ends at 33.5 (the deaths during it don't count toward a mood):
            // back to the chart.
            (33.5, NOTHING, Some(Original)),
            (52.0, DEATH, Some(Original)),
            (54.0, DEATH, Some(Original)),
            // Three deaths with nothing held: nervous, and it stays nervous through deaths,
            // checkpoints and checks alike.
            (56.0, DEATH, Some(MelodicMinor)),
            (60.0, CHECKPOINT, Some(MelodicMinor)),
            (65.0, DEATH, Some(MelodicMinor)),
            (85.0, NOTHING, Some(MelodicMinor)),
            (105.0, NOTHING, Some(MelodicMinor)),
            // Until a call.
            (106.0, HOP, None),
            (106.6, HOP, None),
            (107.2, HOP, None),
            (107.5, CALL, Some(Waltz)),
            (110.0, DEATH, Some(Waltz)),
            (111.0, DEATH, Some(Waltz)),
            (112.0, DEATH, Some(Waltz)),
        ];
        let mut playing = Original;
        let mut changes = Vec::new();
        for &(t, ev, want) in script {
            let got = harmony(b.step(t, ev));
            assert_eq!(got, want, "at {t}s");
            if let Some(h) = got
                && h != playing
            {
                changes.push((t, h));
                playing = h;
            }
        }
        assert_eq!(changes, [(3.5, Coltrane), (13.5, Quartal), (33.5, Original), (56.0, MelodicMinor), (107.5, Waltz)]);
    }

    #[test]
    fn the_death_counter_is_the_nervous_count() {
        let mut b = Band::default();
        b.start(0.0);
        b.step(1.0, DEATH);
        b.step(2.0, DEATH);
        assert_eq!(b.meters(2.0).deaths, Meter { lit: 2, of: 3, full: false, active: None });
        b.step(3.0, DEATH);
        b.step(4.0, DEATH);
        assert_eq!(b.meters(4.0).deaths, Meter { lit: 3, of: 3, full: true, active: None });
    }
}

#[cfg(test)]
mod chute_tests {
    use super::*;

    /// A grease chute: quick successive deaths with nothing called. The nervous band comes at 3
    /// (the physics give grip from 3 level deaths regardless: see `Groove::nervous`); designed
    /// deaths aren't slips, so the band doesn't laugh.
    #[test]
    fn quick_deaths_reach_nervous() {
        let mut b = Band::default();
        b.start(0.0);
        let death = Events { deaths: 1, ..Default::default() };
        let mut last = None;
        for t in [2.0f32, 4.5, 7.0, 9.5] {
            last = b.step(t, death);
        }
        assert_eq!(last.unwrap().0, Filters { harmony: Harmony::MelodicMinor, just_intonation: false });
        assert!(b.stats.level_deaths >= NERVOUS_DEATHS);
    }
}

#[cfg(test)]
mod laugh_decay_tests {
    use super::*;

    /// Plain deaths don't start the laughing band, a slip does; ten seconds without another
    /// wear it off before any checkpoint.
    #[test]
    fn the_laughing_band_wears_off() {
        let mut b = Band::default();
        b.start(0.0);
        let (f, _) = b.step(1.0, Events { deaths: 1, ..Default::default() }).unwrap();
        assert!(!f.just_intonation, "a plain death isn't funny");
        let (f, _) = b.step(2.0, Events { deaths: 1, slips: 1, ..Default::default() }).unwrap();
        assert!(f.just_intonation, "laughing after a slip");
        // Quiet frames: at 2 + 10 s the slip comes off and the band sobers up right away.
        let mut sober_at = None;
        let mut t = 2.25;
        while t < 40.0 {
            if let Some((f, _)) = b.step(t, Events::default())
                && !f.just_intonation
                && sober_at.is_none()
            {
                sober_at = Some(t);
            }
            t += 0.25;
        }
        let at = sober_at.expect("never sobered up");
        assert!((12.0..=12.5).contains(&at), "sobered at {at}");
    }
}
