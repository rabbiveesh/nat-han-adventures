//! The "band director": picks the [`Filters`] from how the player is playing.
//!
//! Decisions happen only at a level start (always plain), after a death (decided as the player
//! dies, so the new version renders during the respawn delay and switches in afterwards), on
//! reaching a checkpoint, every [`MUSIC_CHECK_SECS`] of play (pause excluded) since the last
//! decision, and the moment the player *summons* a mode (see [`summoned`]). Everything else
//! (title, level select, the jingle, victory) is plain. A decision that comes out the same as
//! what's playing changes nothing.
//!
//! The music also bends the physics (`crate::game::Groove`), and levels have gates that need a
//! mode: giant walls need Giant Steps, long gaps a fired-up band, waltz rows a waltzing one. So
//! the summonable rules come first (a player with many deaths must still be able to summon
//! them), and switch in at the next bar line rather than at the next check.
//!
//! Summon rules (each over the stretch):
//! - toots (double jumps) ≥ [`GIANT_STEPS_TOOTS`] → Coltrane, "GIANT STEPS!";
//! - nuggets ≥ [`FIRED_UP_NUGGETS`] at ≥ 1 per [`FIRED_UP_SECS_PER_NUGGET`]s → quartal,
//!   "THE BAND IS FIRED UP";
//! - a *jump in threes* (see [`ThreeStep`]): three consecutive ground jumps whose takeoffs are
//!   evenly spaced (the two intervals within ±[`WALTZ_EVENNESS`] of each other, each
//!   [`WALTZ_MIN_INTERVAL`]–[`WALTZ_MAX_INTERVAL`]s) → the waltz, "THE BAND WALTZES". Toots
//!   don't count and don't break the count; a death does.
//!
//! # Summons hold ([`Band`])
//! A summon (its rule newly met) switches at once and starts a *hold*:
//! - the summon's own rolling-window counter is cleared, so the same 5 toots can't re-trigger
//!   it or keep it qualifying;
//! - the mode holds for at least [`SUMMON_HOLD_SECS`]; deaths and checkpoints during the hold
//!   don't change the harmony (the laughing band's tuning can still come and go on top);
//! - using its mechanic keeps it going: a toot in Giant Steps, a nugget fired up, a jump in
//!   threes in the waltz each extend the hold to now + [`SUMMON_HOLD_SECS`];
//! - a *different* summon overrides it at once (the most recent summon wins) and starts its
//!   own hold, its own counter cleared;
//! - when the hold runs out unused, the next decision applies the rules below with whatever
//!   was counted since (fresh counters), which usually means back to the chart.
//!
//! Rules ([`choose_filters`], when no summon is held), harmony first match wins:
//! 1. the summon rule that fired *last* ([`PlayStats::last_summon`]), if met again;
//! 2. otherwise the met summon rules in a fixed order: Coltrane, quartal, waltz (a rule met
//!    here starts a hold like a summon);
//! 3. deaths since the level start or the last summon, while nothing was held
//!    ([`PlayStats::mood_deaths`]) ≥
//!    [`NERVOUS_DEATHS`] → melodic minor, "THE BAND IS NERVOUS". That count only grows until
//!    the next summon, so once nervous the band stays nervous until a summon or a restart;
//! 4. otherwise as written.
//!
//! The laughing band (the tuning medley, [`Filters::just_intonation`]) plays when deaths
//! since the last checkpoint ≥ [`LAUGHING_DEATHS`] ("THE BAND CAN'T STOP LAUGHING"); it goes
//! with any harmony, the waltz included. The "stretch" is a rolling window over the last
//! [`MUSIC_CHECK_SECS`] of play (shorter at the start of a level).

use std::collections::VecDeque;

use super::{Filters, Harmony};

/// Seconds of play between periodic checks, and the length of the rolling stats window.
pub const MUSIC_CHECK_SECS: f32 = 20.0;

pub const NERVOUS_DEATHS: u32 = 3;
pub const GIANT_STEPS_TOOTS: u32 = 5;
pub const FIRED_UP_NUGGETS: u32 = 4;
pub const FIRED_UP_SECS_PER_NUGGET: f32 = 5.0;
pub const LAUGHING_DEATHS: u32 = 2;
/// A summoned mode holds at least this long (play seconds), and each use of its mechanic
/// extends the hold to this long from then.
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

/// The summonable harmonies, in the order rule 2 tries them.
pub const SUMMON_ORDER: [Harmony; 3] = [Harmony::Coltrane, Harmony::Quartal, Harmony::Waltz];

/// What the director knows about the current level.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PlayStats {
    /// Deaths since the level started.
    pub level_deaths: u32,
    /// Deaths since the level started or the last summon, not counting those while a summon
    /// was held (the nervous rule).
    pub mood_deaths: u32,
    /// Deaths since the last checkpoint (or the level start).
    pub checkpoint_deaths: u32,
    /// In the rolling window:
    pub stretch_toots: u32,
    pub stretch_nuggets: u32,
    pub stretch_deaths: u32,
    /// Jumps in threes completed (overlapping: a 4th even jump completes another).
    pub stretch_waltzes: u32,
    /// Length of the window: [`MUSIC_CHECK_SECS`], or less early in a level.
    pub stretch_secs: f32,
    /// The summon rule that fired most recently this level (see [`summoned`]).
    pub last_summon: Option<Harmony>,
}

impl PlayStats {
    /// Is the summon rule of `h` met?
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
/// [`MUSIC_CHECK_SECS`].
#[derive(Debug, Clone, Default)]
pub struct Window(VecDeque<(f32, [u32; 4])>);

impl Window {
    /// Note what happened at play time `now` (and forget what's too old).
    pub fn record(&mut self, now: f32, toots: u32, nuggets: u32, deaths: u32, waltzes: u32) {
        if toots + nuggets + deaths + waltzes > 0 {
            self.0.push_back((now, [toots, nuggets, deaths, waltzes]));
        }
        while self.0.front().is_some_and(|e| e.0 < now - MUSIC_CHECK_SECS) {
            self.0.pop_front();
        }
    }

    /// Forget a summon's counter (column 0 toots, 1 nuggets, 3 waltz steps).
    pub fn clear(&mut self, column: usize) {
        for e in &mut self.0 {
            e.1[column] = 0;
        }
    }

    /// Fill the window fields of `stats` as of `now` (the level started at `level_start`).
    pub fn fill(&self, stats: &mut PlayStats, now: f32, level_start: f32) {
        let recent = self.0.iter().filter(|e| e.0 >= now - MUSIC_CHECK_SECS);
        let c = recent.fold([0; 4], |a, e| std::array::from_fn(|k| a[k] + e.1[k]));
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

/// The filters for the stats, and why (the reason of the harmony rule if one fired, else the
/// laughing band's reason, else "").
pub fn choose_filters(s: &PlayStats) -> (Filters, &'static str) {
    let summoned = s.last_summon.filter(|&h| s.met(h)).or_else(|| SUMMON_ORDER.into_iter().find(|&h| s.met(h)));
    let harmony = match summoned {
        Some(h) => h,
        None if s.mood_deaths >= NERVOUS_DEATHS => Harmony::MelodicMinor,
        None => Harmony::Original,
    };
    let reason = reason_of(harmony);
    let just_intonation = s.checkpoint_deaths >= LAUGHING_DEATHS;
    let reason = if reason.is_empty() && just_intonation { REASON_LAUGHING } else { reason };
    (Filters { harmony, just_intonation }, reason)
}

/// Harmonies the player calls up on purpose (levels have gates that need them).
pub fn summonable(h: Harmony) -> bool {
    SUMMON_ORDER.contains(&h)
}

/// Did what just happened (stats `before` → `after` it) summon a mode, i.e. newly meet its
/// rule? Then it's the latest summon ([`PlayStats::last_summon`]) and the director decides
/// right away instead of at the next check. (Several at once: the first in [`SUMMON_ORDER`].)
pub fn summoned(before: &PlayStats, after: &PlayStats) -> Option<Harmony> {
    SUMMON_ORDER.into_iter().find(|&h| after.met(h) && !before.met(h))
}

/// What the player did in one frame.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Events {
    pub toots: u32,
    /// Jumps off the ground (not toots).
    pub ground_jumps: u32,
    pub nuggets: u32,
    pub deaths: u32,
    pub checkpoints: u32,
    /// Jumps in threes spotted elsewhere (an editor's "waltz step" button): each counts as one
    /// completed, on top of those [`Band`] spots in `ground_jumps`.
    pub waltz_steps: u32,
}

/// The band's memory of a level: the stats, the rolling window, the jump-in-threes watch and
/// the summon being held. [`Band::step`] takes each frame's events and returns a decision
/// when one is due (see the module docs).
#[derive(Debug, Clone, Default)]
pub struct Band {
    pub stats: PlayStats,
    pub window: Window,
    pub steps: ThreeStep,
    /// The summoned mode being held, and until when (play time).
    pub hold: Option<(Harmony, f32)>,
    /// Jumps in threes completed this level (the music hears each as a waltz step).
    pub steps_taken: u32,
    level_start: f32,
    next_check: f32,
}

/// The rolling-window column of a summon's counter.
fn column(h: Harmony) -> Option<usize> {
    match h {
        Harmony::Coltrane => Some(0),
        Harmony::Quartal => Some(1),
        Harmony::Waltz => Some(3),
        _ => None,
    }
}

impl Band {
    /// A level (re)starts at play time `now`: everything resets, the band plays it straight.
    pub fn start(&mut self, now: f32) -> (Filters, &'static str) {
        *self = Band { level_start: now, next_check: now + MUSIC_CHECK_SECS, ..Band::default() };
        (Filters::default(), "")
    }

    /// The summon held at `now`, if any.
    pub fn held(&self, now: f32) -> Option<Harmony> {
        self.hold.filter(|&(_, until)| now < until).map(|(h, _)| h)
    }

    /// One frame's events at play time `now`; returns a decision if one is due.
    pub fn step(&mut self, now: f32, ev: Events) -> Option<(Filters, &'static str)> {
        // Ground jumps: a jump in threes is a waltz step (a death starts the count afresh).
        let mut waltzes = ev.waltz_steps;
        for _ in 0..ev.ground_jumps {
            waltzes += self.steps.jump(now) as u32;
        }
        self.steps_taken += waltzes;
        if ev.deaths > 0 {
            self.steps.reset();
        }
        // Stats as they stood before this frame's events (to see whether they summon a mode).
        let mut before = self.stats;
        self.window.fill(&mut before, now, self.level_start);
        self.window.record(now, ev.toots, ev.nuggets, ev.deaths, waltzes);
        self.stats.level_deaths += ev.deaths;
        self.stats.checkpoint_deaths += ev.deaths;
        // Deaths while a summon is held don't count toward a mood.
        if self.held(now).is_none() {
            self.stats.mood_deaths += ev.deaths;
        }
        self.window.fill(&mut self.stats, now, self.level_start);
        let summon = if ev.toots + ev.nuggets + waltzes > 0 { summoned(&before, &self.stats) } else { None };
        if let Some(h) = summon {
            self.begin(h, now);
        } else if let Some(h) = self.held(now) {
            let used = match h {
                Harmony::Coltrane => ev.toots > 0,
                Harmony::Quartal => ev.nuggets > 0,
                Harmony::Waltz => waltzes > 0,
                _ => false,
            };
            if used {
                self.hold = Some((h, now + SUMMON_HOLD_SECS));
            }
        }
        if !(summon.is_some() || ev.deaths > 0 || ev.checkpoints > 0 || now >= self.next_check) {
            return None;
        }
        let decided = self.decide(now);
        self.next_check = now + MUSIC_CHECK_SECS;
        if ev.checkpoints > 0 {
            self.stats.checkpoint_deaths = 0;
        }
        Some(decided)
    }

    /// A summon of `h` starts: its counter cleared, its hold started, the mood forgotten.
    fn begin(&mut self, h: Harmony, now: f32) {
        if let Some(k) = column(h) {
            self.window.clear(k);
        }
        self.window.fill(&mut self.stats, now, self.level_start);
        self.hold = Some((h, now + SUMMON_HOLD_SECS));
        self.stats.mood_deaths = 0;
        self.stats.last_summon = Some(h);
    }

    fn decide(&mut self, now: f32) -> (Filters, &'static str) {
        let just_intonation = self.stats.checkpoint_deaths >= LAUGHING_DEATHS;
        let harmony = match self.held(now) {
            Some(h) => h,
            None => {
                self.hold = None;
                let h = choose_filters(&self.stats).0.harmony;
                if summonable(h) {
                    self.begin(h, now);
                }
                h
            }
        };
        let reason = reason_of(harmony);
        let reason = if reason.is_empty() && just_intonation { REASON_LAUGHING } else { reason };
        (Filters { harmony, just_intonation }, reason)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(level_deaths: u32, checkpoint_deaths: u32, toots: u32, nuggets: u32, secs: f32) -> PlayStats {
        PlayStats {
            level_deaths,
            mood_deaths: level_deaths,
            checkpoint_deaths,
            stretch_toots: toots,
            stretch_nuggets: nuggets,
            stretch_secs: secs,
            ..PlayStats::default()
        }
    }

    fn waltzing(s: PlayStats) -> PlayStats {
        PlayStats { stretch_waltzes: 1, ..s }
    }

    #[test]
    fn rules_table() {
        use Harmony::*;
        let cases: &[(PlayStats, Harmony, bool, &str)] = &[
            (st(0, 0, 0, 0, 10.0), Original, false, ""),
            (st(3, 0, 9, 9, 1.0), Coltrane, false, REASON_GIANT_STEPS),
            (st(5, 0, 0, 4, 10.0), Quartal, false, REASON_FIRED_UP),
            (st(3, 0, 4, 3, 10.0), MelodicMinor, false, REASON_NERVOUS),
            (st(2, 0, 5, 9, 1.0), Coltrane, false, REASON_GIANT_STEPS),
            (st(0, 0, 4, 9, 1.0), Quartal, false, REASON_FIRED_UP),
            (st(0, 0, 0, 4, 20.0), Quartal, false, REASON_FIRED_UP),
            (st(0, 0, 0, 5, 24.0), Quartal, false, REASON_FIRED_UP),
            (st(0, 0, 0, 4, 20.1), Original, false, ""),
            (st(0, 0, 0, 3, 1.0), Original, false, ""),
            (st(2, 2, 0, 0, 30.0), Original, true, REASON_LAUGHING),
            (st(1, 1, 0, 0, 30.0), Original, false, ""),
            (st(3, 2, 0, 0, 30.0), MelodicMinor, true, REASON_NERVOUS),
            (st(9, 2, 5, 0, 30.0), Coltrane, true, REASON_GIANT_STEPS),
            (st(2, 2, 6, 0, 30.0), Coltrane, true, REASON_GIANT_STEPS),
            // The waltz: a summon (beats a nervous band), after Coltrane and quartal in the
            // fixed order, and it goes with the laughing band.
            (waltzing(st(0, 0, 0, 0, 10.0)), Waltz, false, REASON_WALTZ),
            (waltzing(st(5, 0, 0, 0, 10.0)), Waltz, false, REASON_WALTZ),
            (waltzing(st(2, 2, 0, 0, 10.0)), Waltz, true, REASON_WALTZ),
            (waltzing(st(0, 0, 5, 0, 10.0)), Coltrane, false, REASON_GIANT_STEPS),
            (waltzing(st(0, 0, 0, 4, 10.0)), Quartal, false, REASON_FIRED_UP),
        ];
        for (s, h, ji, why) in cases {
            // (A level is never more than a window long here.)
            let (f, reason) = choose_filters(s);
            assert_eq!((f.harmony, f.just_intonation, reason), (*h, *ji, *why), "{s:?}");
        }
    }

    #[test]
    fn the_latest_summon_wins_while_it_lasts() {
        use Harmony::*;
        let both = waltzing(st(0, 0, 5, 0, 10.0));
        // Waltzed last: the waltz, though Giant Steps' rule is met too.
        assert_eq!(choose_filters(&PlayStats { last_summon: Some(Waltz), ..both }).0.harmony, Waltz);
        assert_eq!(choose_filters(&PlayStats { last_summon: Some(Coltrane), ..both }).0.harmony, Coltrane);
        // The latest summon lapsed: back to the fixed order among the rest.
        let lapsed = PlayStats { last_summon: Some(Quartal), ..both };
        assert_eq!(choose_filters(&lapsed).0.harmony, Coltrane);
        let only_waltz = PlayStats { last_summon: Some(Coltrane), ..waltzing(st(4, 0, 2, 0, 10.0)) };
        assert_eq!(choose_filters(&only_waltz), (Filters { harmony: Waltz, just_intonation: false }, REASON_WALTZ));
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
        assert_eq!(choose_filters(&s).0.harmony, Harmony::Coltrane);
        // 22s later the early toots have aged out.
        w.record(22.5, 0, 0, 0, 0);
        w.fill(&mut s, 22.5, 0.0);
        assert_eq!((s.stretch_toots, s.stretch_nuggets, s.stretch_waltzes, s.stretch_secs), (3, 2, 1, 20.0));
        w.fill(&mut s, 40.0, 0.0);
        assert_eq!((s.stretch_toots, s.stretch_nuggets, s.stretch_waltzes), (0, 0, 0));
    }

    #[test]
    fn summoning() {
        use Harmony::*;
        // The 5th toot summons Giant Steps, even for a nervous band; the 6th doesn't again.
        assert_eq!(summoned(&st(4, 0, 4, 0, 20.0), &st(4, 0, 5, 0, 20.0)), Some(Coltrane));
        assert_eq!(summoned(&st(0, 0, 3, 0, 20.0), &st(0, 0, 4, 0, 20.0)), None);
        assert_eq!(summoned(&st(0, 0, 5, 0, 20.0), &st(0, 0, 6, 0, 20.0)), None);
        // The 4th quick nugget fires the band up; slow nuggets don't.
        assert_eq!(summoned(&st(0, 0, 0, 3, 12.0), &st(0, 0, 0, 4, 12.0)), Some(Quartal));
        assert_eq!(summoned(&st(0, 0, 0, 3, 20.0), &st(0, 0, 0, 3, 20.0)), None);
        assert_eq!(summoned(&st(0, 0, 0, 3, 20.5), &st(0, 0, 0, 4, 20.5)), None);
        // Toots while fired up summon Giant Steps over it.
        assert_eq!(summoned(&st(0, 0, 4, 6, 10.0), &st(0, 0, 5, 6, 10.0)), Some(Coltrane));
        // A jump in threes summons the waltz, even over Giant Steps; waltzing on doesn't again.
        assert_eq!(summoned(&st(0, 0, 7, 0, 10.0), &waltzing(st(0, 0, 7, 0, 10.0))), Some(Waltz));
        let more = PlayStats { stretch_waltzes: 2, ..st(0, 0, 0, 0, 10.0) };
        assert_eq!(summoned(&waltzing(st(0, 0, 0, 0, 10.0)), &more), None);
        // Deaths alone never summon (they decide anyway).
        assert_eq!(summoned(&st(2, 0, 0, 0, 10.0), &st(3, 0, 0, 0, 10.0)), None);
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

    // --- The band's holds (Band) ---

    const TOOT: Events = Events { toots: 1, ground_jumps: 0, nuggets: 0, deaths: 0, checkpoints: 0, waltz_steps: 0 };
    const HOP: Events = Events { toots: 0, ground_jumps: 1, nuggets: 0, deaths: 0, checkpoints: 0, waltz_steps: 0 };
    const NUGGET: Events = Events { toots: 0, ground_jumps: 0, nuggets: 1, deaths: 0, checkpoints: 0, waltz_steps: 0 };
    const DEATH: Events = Events { toots: 0, ground_jumps: 0, nuggets: 0, deaths: 1, checkpoints: 0, waltz_steps: 0 };
    const CHECKPOINT: Events = Events { toots: 0, ground_jumps: 0, nuggets: 0, deaths: 0, checkpoints: 1, waltz_steps: 0 };
    const NOTHING: Events = Events { toots: 0, ground_jumps: 0, nuggets: 0, deaths: 0, checkpoints: 0, waltz_steps: 0 };

    /// A band 5 toots into a level (t = 1..=5): Giant Steps just summoned.
    fn giant_steps() -> Band {
        let mut b = Band::default();
        b.start(0.0);
        for t in 1..=4 {
            assert_eq!(b.step(t as f32, TOOT), None);
        }
        let d = b.step(5.0, TOOT).expect("the 5th toot decides at once");
        assert_eq!(d.0.harmony, Harmony::Coltrane);
        b
    }

    fn harmony(d: Option<(Filters, &'static str)>) -> Option<Harmony> {
        d.map(|d| d.0.harmony)
    }

    #[test]
    fn a_summon_clears_its_counter() {
        let mut b = giant_steps();
        assert_eq!(b.stats.stretch_toots, 0, "the 5 toots are spent");
        assert_eq!(b.hold, Some((Harmony::Coltrane, 5.0 + SUMMON_HOLD_SECS)));
        // 4 more toots don't summon again (they only keep it alive).
        for t in 6..=9 {
            assert_eq!(b.step(t as f32, TOOT), None);
        }
        assert_eq!(b.stats.stretch_toots, 4);
        // The same goes for the nuggets and the waltz.
        let mut b = Band::default();
        b.start(0.0);
        for t in 1..=4 {
            b.step(t as f32, NUGGET);
        }
        assert_eq!((b.held(4.5), b.stats.stretch_nuggets), (Some(Harmony::Quartal), 0));
        let mut b = Band::default();
        b.start(0.0);
        for t in [1.0, 1.6, 2.2] {
            b.step(t, HOP);
        }
        assert_eq!((b.held(2.5), b.stats.stretch_waltzes), (Some(Harmony::Waltz), 0));
    }

    #[test]
    fn deaths_and_checkpoints_dont_change_a_held_harmony() {
        let mut b = giant_steps();
        // Three deaths would make the band nervous, two at a checkpoint laughing.
        assert_eq!(b.step(6.0, DEATH), Some((Filters { harmony: Harmony::Coltrane, just_intonation: false }, REASON_GIANT_STEPS)));
        let d = b.step(7.0, DEATH).unwrap();
        assert_eq!(d.0, Filters { harmony: Harmony::Coltrane, just_intonation: true }, "the laughing band layers on top");
        assert_eq!(harmony(b.step(8.0, DEATH)), Some(Harmony::Coltrane));
        assert_eq!(harmony(b.step(9.0, CHECKPOINT)), Some(Harmony::Coltrane));
        // The hold runs out unused: the next check decides afresh. The deaths came during the
        // hold of a summon, so the band isn't nervous: back to the chart.
        assert_eq!(b.step(28.0, NOTHING), None);
        assert_eq!(harmony(b.step(29.5, NOTHING)), Some(Harmony::Original));
    }

    #[test]
    fn using_the_mode_keeps_it_going() {
        let mut b = giant_steps();
        assert_eq!(b.step(20.0, TOOT), None);
        assert_eq!(b.hold, Some((Harmony::Coltrane, 40.0)), "a toot extends the hold");
        // The periodic check at 25s (20s after the summon) finds it still held.
        assert_eq!(harmony(b.step(25.0, NOTHING)), Some(Harmony::Coltrane));
        assert_eq!(harmony(b.step(39.0, DEATH)), Some(Harmony::Coltrane));
        // Unused since 20s: the check after 40s goes back to the chart.
        assert_eq!(harmony(b.step(45.0, NOTHING)), None);
        assert_eq!(harmony(b.step(59.0, NOTHING)), Some(Harmony::Original));
        assert_eq!(b.hold, None);
        // Waltz steps keep the waltz going, the same way.
        let mut b = Band::default();
        b.start(0.0);
        for t in [1.0, 1.6, 2.2] {
            b.step(t, HOP);
        }
        b.step(15.0, HOP);
        b.step(15.6, HOP);
        assert_eq!(b.hold, Some((Harmony::Waltz, 22.2)), "a stumble doesn't extend it");
        b.step(16.2, HOP);
        assert_eq!(b.hold, Some((Harmony::Waltz, 36.2)), "an even jump does");
    }

    #[test]
    fn a_new_summon_overrides_a_held_one() {
        let mut b = giant_steps();
        assert_eq!(b.step(6.0, HOP), None);
        assert_eq!(b.step(6.6, HOP), None);
        let d = b.step(7.2, HOP).expect("the waltz is summoned at once");
        assert_eq!(d, (Filters { harmony: Harmony::Waltz, just_intonation: false }, REASON_WALTZ));
        assert_eq!(b.hold, Some((Harmony::Waltz, 7.2 + SUMMON_HOLD_SECS)));
        assert_eq!(b.stats.stretch_waltzes, 0);
        // And back: 5 fresh toots summon Giant Steps over the waltz.
        for t in 8..=11 {
            assert_eq!(b.step(t as f32, TOOT), None);
        }
        assert_eq!(harmony(b.step(12.0, TOOT)), Some(Harmony::Coltrane));
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
            (3.0, TOOT, Some(Coltrane)),
            (6.0, DEATH, Some(Coltrane)),
            (9.0, DEATH, Some(Coltrane)),
            (10.0, NUGGET, None),
            (11.0, NUGGET, None),
            (12.0, NUGGET, None),
            (13.0, NUGGET, Some(Quartal)),
            (15.0, DEATH, Some(Quartal)),
            (20.0, DEATH, Some(Quartal)),
            (30.0, DEATH, Some(Quartal)),
            // The quartal hold ended at 33 (the deaths during it don't count toward a mood); the
            // check at 50 decides afresh: back to the chart.
            (50.0, NOTHING, Some(Original)),
            (52.0, DEATH, Some(Original)),
            (54.0, DEATH, Some(Original)),
            // Three deaths with nothing held: nervous, and it stays nervous through deaths,
            // checkpoints and checks alike.
            (56.0, DEATH, Some(MelodicMinor)),
            (60.0, CHECKPOINT, Some(MelodicMinor)),
            (65.0, DEATH, Some(MelodicMinor)),
            (85.0, NOTHING, Some(MelodicMinor)),
            (105.0, NOTHING, Some(MelodicMinor)),
            // Until a summon.
            (106.0, HOP, None),
            (106.6, HOP, None),
            (107.2, HOP, Some(Waltz)),
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
        assert_eq!(changes, [(3.0, Coltrane), (13.0, Quartal), (50.0, Original), (56.0, MelodicMinor), (107.2, Waltz)]);
    }
}

#[cfg(test)]
mod chute_tests {
    use super::*;

    /// A grease chute: quick successive deaths with no summon. The laughing band comes at 2 and
    /// the nervous band at 3 (the physics give grip from 3 level deaths regardless: see
    /// `Groove::nervous`).
    #[test]
    fn quick_deaths_reach_nervous() {
        let mut b = Band::default();
        b.start(0.0);
        let death = Events { deaths: 1, ..Default::default() };
        let mut last = None;
        for t in [2.0f32, 4.5, 7.0, 9.5] {
            last = b.step(t, death);
        }
        assert_eq!(last.unwrap().0.harmony, Harmony::MelodicMinor);
        assert!(b.stats.level_deaths >= NERVOUS_DEATHS);
    }
}
