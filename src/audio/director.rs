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
//! mode: giant walls need Giant Steps, long gaps a fired-up band. So the two summonable rules
//! come first (a player with many deaths must still be able to summon them), and switch in at
//! the next bar line rather than at the next check.
//!
//! Rules ([`choose_filters`]), harmony first match wins:
//! 1. toots (double jumps) in the stretch ≥ [`GIANT_STEPS_TOOTS`] → Coltrane, "GIANT STEPS!";
//! 2. nuggets in the stretch ≥ [`FIRED_UP_NUGGETS`] at ≥ 1 per [`FIRED_UP_SECS_PER_NUGGET`]s →
//!    quartal, "THE BAND IS FIRED UP";
//! 3. level deaths ≥ [`NERVOUS_DEATHS`] → melodic minor, "THE BAND IS NERVOUS";
//! 4. otherwise as written.
//!
//! A mode lasts until a later decision no longer finds its rule met; toots and nuggets made
//! while it plays count, so keeping it up keeps it going.
//!
//! The laughing band (the tuning medley, [`Filters::just_intonation`]) plays when deaths
//! since the last checkpoint ≥ [`LAUGHING_DEATHS`] ("THE BAND CAN'T STOP LAUGHING"). The "stretch" is a rolling window over the last
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

pub const REASON_NERVOUS: &str = "THE BAND IS NERVOUS";
pub const REASON_GIANT_STEPS: &str = "GIANT STEPS!";
pub const REASON_FIRED_UP: &str = "THE BAND IS FIRED UP";
pub const REASON_LAUGHING: &str = "THE BAND CAN'T STOP LAUGHING";

/// What the director knows about the current level.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PlayStats {
    /// Deaths since the level started.
    pub level_deaths: u32,
    /// Deaths since the last checkpoint (or the level start).
    pub checkpoint_deaths: u32,
    /// In the rolling window:
    pub stretch_toots: u32,
    pub stretch_nuggets: u32,
    pub stretch_deaths: u32,
    /// Length of the window: [`MUSIC_CHECK_SECS`], or less early in a level.
    pub stretch_secs: f32,
}

/// Timestamped (play time) toots, nuggets and deaths of the last [`MUSIC_CHECK_SECS`].
#[derive(Debug, Clone, Default)]
pub struct Window(VecDeque<(f32, u32, u32, u32)>);

impl Window {
    /// Note what happened at play time `now` (and forget what's too old).
    pub fn record(&mut self, now: f32, toots: u32, nuggets: u32, deaths: u32) {
        if toots + nuggets + deaths > 0 {
            self.0.push_back((now, toots, nuggets, deaths));
        }
        while self.0.front().is_some_and(|e| e.0 < now - MUSIC_CHECK_SECS) {
            self.0.pop_front();
        }
    }

    /// Fill the window fields of `stats` as of `now` (the level started at `level_start`).
    pub fn fill(&self, stats: &mut PlayStats, now: f32, level_start: f32) {
        let recent = self.0.iter().filter(|e| e.0 >= now - MUSIC_CHECK_SECS);
        let (t, n, d) = recent.fold((0, 0, 0), |(t, n, d), e| (t + e.1, n + e.2, d + e.3));
        stats.stretch_toots = t;
        stats.stretch_nuggets = n;
        stats.stretch_deaths = d;
        stats.stretch_secs = (now - level_start).clamp(0.0, MUSIC_CHECK_SECS);
    }
}

/// The filters for the stats, and why (the reason of the harmony rule if one fired, else the
/// laughing band's reason, else "").
pub fn choose_filters(s: &PlayStats) -> (Filters, &'static str) {
    let (harmony, reason) = if s.stretch_toots >= GIANT_STEPS_TOOTS {
        (Harmony::Coltrane, REASON_GIANT_STEPS)
    } else if s.stretch_nuggets >= FIRED_UP_NUGGETS
        && s.stretch_secs <= s.stretch_nuggets as f32 * FIRED_UP_SECS_PER_NUGGET
    {
        (Harmony::Quartal, REASON_FIRED_UP)
    } else if s.level_deaths >= NERVOUS_DEATHS {
        (Harmony::MelodicMinor, REASON_NERVOUS)
    } else {
        (Harmony::Original, "")
    };
    let just_intonation = s.checkpoint_deaths >= LAUGHING_DEATHS;
    let reason = if reason.is_empty() && just_intonation { REASON_LAUGHING } else { reason };
    (Filters { harmony, just_intonation }, reason)
}

/// Harmonies the player calls up on purpose (levels have gates that need them).
pub fn summonable(h: Harmony) -> bool {
    matches!(h, Harmony::Coltrane | Harmony::Quartal)
}

/// Did what just happened (stats `before` → `after` it) summon a mode? Then the director
/// decides right away instead of at the next check.
pub fn summoned(before: &PlayStats, after: &PlayStats) -> bool {
    let h = choose_filters(after).0.harmony;
    summonable(h) && choose_filters(before).0.harmony != h
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(level_deaths: u32, checkpoint_deaths: u32, toots: u32, nuggets: u32, secs: f32) -> PlayStats {
        PlayStats {
            level_deaths,
            checkpoint_deaths,
            stretch_toots: toots,
            stretch_nuggets: nuggets,
            stretch_deaths: 0,
            stretch_secs: secs,
        }
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
        ];
        for (s, h, ji, why) in cases {
            // (A level is never more than a window long here.)
            let (f, reason) = choose_filters(s);
            assert_eq!((f.harmony, f.just_intonation, reason), (*h, *ji, *why), "{s:?}");
        }
    }

    #[test]
    fn the_window_rolls() {
        let mut w = Window::default();
        let mut s = PlayStats::default();
        for k in 0..5 {
            w.record(1.0 + k as f32, 1, 0, 0);
        }
        w.record(10.0, 0, 2, 1);
        w.fill(&mut s, 10.0, 0.0);
        assert_eq!((s.stretch_toots, s.stretch_nuggets, s.stretch_deaths, s.stretch_secs), (5, 2, 1, 10.0));
        assert_eq!(choose_filters(&s).0.harmony, Harmony::Coltrane);
        // 22s later the early toots have aged out.
        w.record(22.5, 0, 0, 0);
        w.fill(&mut s, 22.5, 0.0);
        assert_eq!((s.stretch_toots, s.stretch_nuggets, s.stretch_secs), (3, 2, 20.0));
        w.fill(&mut s, 40.0, 0.0);
        assert_eq!((s.stretch_toots, s.stretch_nuggets), (0, 0));
    }

    #[test]
    fn summoning() {
        // The 5th toot summons Giant Steps, even for a nervous band; the 6th doesn't again.
        assert!(summoned(&st(4, 0, 4, 0, 20.0), &st(4, 0, 5, 0, 20.0)));
        assert!(!summoned(&st(0, 0, 3, 0, 20.0), &st(0, 0, 4, 0, 20.0)));
        assert!(!summoned(&st(0, 0, 5, 0, 20.0), &st(0, 0, 6, 0, 20.0)));
        // The 4th quick nugget fires the band up; slow nuggets don't.
        assert!(summoned(&st(0, 0, 0, 3, 12.0), &st(0, 0, 0, 4, 12.0)));
        assert!(!summoned(&st(0, 0, 0, 3, 20.0), &st(0, 0, 0, 3, 20.0)));
        assert!(!summoned(&st(0, 0, 0, 3, 20.5), &st(0, 0, 0, 4, 20.5)));
        // Toots while fired up summon Giant Steps over it.
        assert!(summoned(&st(0, 0, 4, 6, 10.0), &st(0, 0, 5, 6, 10.0)));
        // Deaths alone never summon (they decide anyway).
        assert!(!summoned(&st(2, 0, 0, 0, 10.0), &st(3, 0, 0, 0, 10.0)));
    }
}
