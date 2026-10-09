//! Persistent progress: which levels are unlocked and the best nugget haul / time per level,
//! plus the adaptive engine's state (the story assist dial and free play's
//! [`PlayerProfile`]: see [`adaptive_to_text`]). Stored in `localStorage` on the web and a small
//! text file on native.
//!
//! [`plugin`] (part of `gameplay`) only creates the in-memory [`Progress`]: it never touches disk,
//! so headless tests stay hermetic. [`persistence_plugin`] (added by the UI when there's a real
//! window) loads it at startup and writes it back whenever it changes.

use bevy::prelude::*;

use crate::adapt::{Calibration, Outcome, PlayerProfile, Probe, Skill, StoryAssist, WindowEntry};
use crate::game::{AdaptiveProfile, StoryAssistState};
use crate::level::LEVEL_COUNT;

pub fn plugin(app: &mut App) {
    app.init_resource::<Progress>();
}

/// Load progress at startup and save it on every change. Only for the real game.
pub fn persistence_plugin(app: &mut App) {
    app.add_systems(PreStartup, load_progress).add_systems(Last, save_progress);
}

#[derive(Resource, Debug, Clone, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct Progress {
    /// Levels `0..unlocked` are playable. Always >= 1.
    pub unlocked: usize,
    /// Best nugget count per level, `None` if never completed.
    pub best_nuggets: [Option<u32>; LEVEL_COUNT],
    /// Best completion time per level, seconds.
    pub best_time: [Option<f32>; LEVEL_COUNT],
    /// The seed of the last free-play run (offered as "LAST" between visits).
    pub last_seed: Option<u32>,
    /// How late the player's speakers or headphones sound beyond what the platform reports
    /// (Bluetooth on a phone often hides 150+ ms): the beat clock steps back by it. Set in the
    /// pause menu's AUDIO DELAY (tap along, or nudge). 0..=[`MAX_AUDIO_DELAY_MS`].
    pub audio_delay_ms: u32,
}

/// The largest audio delay the player can set.
pub const MAX_AUDIO_DELAY_MS: u32 = 500;

impl Default for Progress {
    fn default() -> Self {
        Self { unlocked: 1, best_nuggets: [None; LEVEL_COUNT], best_time: [None; LEVEL_COUNT], last_seed: None, audio_delay_ms: 0 }
    }
}

/// What changed when a level was completed (for "NEW BEST!" on the results card).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RecordOutcome {
    pub first_clear: bool,
    pub new_best_nuggets: bool,
    pub new_best_time: bool,
    /// The next level just became playable.
    pub unlocked_next: bool,
}

const HEADER: &str = "nat-han-adventures-progress 1";

impl Progress {
    /// Record a completed run of `level`: unlock the next one and keep the bests.
    pub fn record(&mut self, level: usize, nuggets: u32, time: f32) -> RecordOutcome {
        let mut out = RecordOutcome::default();
        if level >= LEVEL_COUNT {
            return out;
        }
        out.first_clear = self.best_nuggets[level].is_none();
        if self.best_nuggets[level].is_none_or(|b| nuggets > b) {
            self.best_nuggets[level] = Some(nuggets);
            out.new_best_nuggets = !out.first_clear;
        }
        if self.best_time[level].is_none_or(|b| time < b) {
            self.best_time[level] = Some(time);
            out.new_best_time = !out.first_clear;
        }
        let next = (level + 2).min(LEVEL_COUNT);
        if next > self.unlocked {
            self.unlocked = next;
            out.unlocked_next = true;
        }
        out
    }

    pub fn is_unlocked(&self, level: usize) -> bool {
        level < self.unlocked
    }

    /// Tiny line-based text format:
    /// ```text
    /// nat-han-adventures-progress 1
    /// unlocked 3
    /// level 1 12 63.250
    /// audio-delay 180
    /// ```
    /// (`level <index> <best nuggets> <best time secs>`, only for completed levels;
    /// `audio-delay <ms>` only when set.)
    pub fn to_text(&self) -> String {
        let mut s = format!("{HEADER}\nunlocked {}\n", self.unlocked);
        for i in 0..LEVEL_COUNT {
            if let (Some(n), Some(t)) = (self.best_nuggets[i], self.best_time[i]) {
                s.push_str(&format!("level {i} {n} {t:.3}\n"));
            }
        }
        if let Some(seed) = self.last_seed {
            s.push_str(&format!("freeplay-seed {seed}\n"));
        }
        if self.audio_delay_ms > 0 {
            s.push_str(&format!("audio-delay {}\n", self.audio_delay_ms));
        }
        s
    }

    /// Parse [`Progress::to_text`] output. Lenient: unknown or broken lines are skipped;
    /// returns `None` only if the header is missing (not our file).
    pub fn from_text(src: &str) -> Option<Progress> {
        let mut lines = src.lines().map(str::trim).filter(|l| !l.is_empty());
        if lines.next()? != HEADER {
            return None;
        }
        let mut p = Progress::default();
        for line in lines {
            let parts: Vec<&str> = line.split_whitespace().collect();
            match parts.as_slice() {
                ["unlocked", n] => {
                    if let Ok(n) = n.parse::<usize>() {
                        p.unlocked = n.clamp(1, LEVEL_COUNT);
                    }
                }
                ["level", i, n, t] => {
                    if let (Ok(i), Ok(n), Ok(t)) = (i.parse::<usize>(), n.parse(), t.parse::<f32>())
                        && i < LEVEL_COUNT
                        && t.is_finite()
                    {
                        p.best_nuggets[i] = Some(n);
                        p.best_time[i] = Some(t.max(0.0));
                    }
                }
                ["freeplay-seed", seed] => p.last_seed = seed.parse().ok(),
                ["audio-delay", ms] => p.audio_delay_ms = ms.parse::<u32>().map_or(0, |ms| ms.min(MAX_AUDIO_DELAY_MS)),
                _ => {}
            }
        }
        Some(p)
    }
}

fn load_progress(mut commands: Commands) {
    let Some(text) = storage::read() else { return };
    match Progress::from_text(&text) {
        Some(p) => {
            info!("loaded progress: {} level(s) unlocked", p.unlocked);
            commands.insert_resource(p);
            let (dial, profile) = adaptive_from_text(&text);
            commands.insert_resource(StoryAssistState(StoryAssist::new(dial)));
            commands.insert_resource(AdaptiveProfile(profile));
        }
        None => warn!("ignoring unreadable save data"),
    }
}

/// The whole save file: progress, then the adaptive state.
pub fn save_text(progress: &Progress, story: &StoryAssist, profile: &PlayerProfile) -> String {
    progress.to_text() + &adaptive_to_text(story.assists, profile)
}

fn save_progress(
    progress: Res<Progress>,
    story: Res<StoryAssistState>,
    profile: Res<AdaptiveProfile>,
    mut last: Local<Option<String>>,
) {
    if !progress.is_changed() && !story.is_changed() && !profile.is_changed() {
        return;
    }
    let text = save_text(&progress, &story.0, &profile.0);
    if last.as_deref() == Some(text.as_str()) {
        return;
    }
    // The first change is the load itself: remember it without rewriting the same data.
    let first = last.is_none();
    *last = Some(text.clone());
    if !first || storage::read().as_deref() != Some(text.as_str()) {
        if let Err(e) = storage::write(&text) {
            warn!("couldn't save progress: {e}");
        }
    }
}

// ─── The adaptive state ──────────────────────────────────────────────────────

/// The adaptive lines of the save, appended after [`Progress::to_text`]'s (same header, so
/// older builds just skip them, and saves without them load with a fresh dial and profile):
/// ```text
/// story-assists 0.24
/// profile-assists 0.07
/// profile-rooms 12
/// profile-streak 3
/// profile-calibrated 1
/// profile-probe 3 1 0.8
/// profile-skill precision 4 0.5 2 -
/// profile-window precision C 4 4 2 0
/// ```
/// (`profile-probe <band> <clean 0/1> <time ratio or ->`;
/// `profile-skill <name> <center> <spread> <epoch> <fell from or ->`;
/// `profile-window <skill> <C clean / S struggle / K careless> <band> <center> <epoch> <assists>`,
/// oldest first.) Floats are written exactly (shortest round-trip form).
pub fn adaptive_to_text(story_dial: f32, p: &PlayerProfile) -> String {
    let mut s = format!(
        "story-assists {story_dial}\nprofile-assists {}\nprofile-rooms {}\nprofile-streak {}\nprofile-calibrated {}\n",
        p.assists,
        p.rooms_played,
        p.streak,
        u8::from(p.calibration.done)
    );
    let opt = |v: Option<String>| v.unwrap_or_else(|| "-".into());
    for probe in &p.calibration.probes {
        let ratio = opt(probe.time_ratio.map(|r| r.to_string()));
        s.push_str(&format!("profile-probe {} {} {ratio}\n", probe.band, u8::from(probe.clean)));
    }
    for skill in Skill::ALL {
        let st = p.skill(skill);
        let name = skill.name();
        let fell = opt(st.fell_from.map(|b| b.to_string()));
        s.push_str(&format!("profile-skill {name} {} {} {} {fell}\n", st.center, st.spread, st.epoch));
        for e in &st.window.entries {
            let o = match e.outcome {
                Outcome::Clean => 'C',
                Outcome::Struggle => 'S',
                Outcome::Careless => 'K',
            };
            s.push_str(&format!("profile-window {name} {o} {} {} {} {}\n", e.band, e.center, e.epoch, e.assists));
        }
    }
    s
}

/// Parse [`adaptive_to_text`]'s lines out of a save (any other lines are skipped): the story
/// dial (0 if absent) and the profile ([`PlayerProfile::new`] where absent). Lenient like
/// [`Progress::from_text`]: broken lines are skipped, values clamped.
pub fn adaptive_from_text(src: &str) -> (f32, PlayerProfile) {
    use crate::adapt::skill::{MAX_BAND, MIN_BAND};
    let unit = |v: &str| v.parse::<f32>().ok().filter(|v| v.is_finite()).map(|v| v.clamp(0.0, 1.0));
    let band = |v: &str| v.parse::<u8>().ok().filter(|b| (MIN_BAND..=MAX_BAND).contains(b));
    let skill_of = |n: &str| Skill::ALL.into_iter().find(|s| s.name() == n);
    let mut dial = 0.0;
    let mut p = PlayerProfile::new();
    let mut probes = Vec::new();
    let mut calibrated = false;
    for line in src.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        match parts.as_slice() {
            ["story-assists", v] => dial = unit(v).unwrap_or(dial),
            ["profile-assists", v] => p.assists = unit(v).unwrap_or(p.assists),
            ["profile-rooms", v] => p.rooms_played = v.parse().unwrap_or(p.rooms_played),
            ["profile-streak", v] => p.streak = v.parse().unwrap_or(p.streak),
            ["profile-calibrated", v] => calibrated = *v == "1",
            ["profile-probe", b, clean, ratio] => {
                if let Some(b) = band(b) {
                    let time_ratio = ratio.parse::<f32>().ok().filter(|r| r.is_finite() && *r >= 0.0);
                    probes.push(Probe { band: b, clean: *clean == "1", time_ratio });
                }
            }
            ["profile-skill", name, center, spread, epoch, fell] => {
                if let (Some(skill), Some(center), Some(spread), Ok(epoch)) =
                    (skill_of(name), band(center), unit(spread), epoch.parse::<u32>())
                {
                    let st = &mut p.skills[skill.index()];
                    st.center = center;
                    st.spread = spread;
                    st.epoch = epoch;
                    st.fell_from = band(fell);
                }
            }
            ["profile-window", name, o, b, center, epoch, assists] => {
                let outcome = match *o {
                    "C" => Some(Outcome::Clean),
                    "S" => Some(Outcome::Struggle),
                    "K" => Some(Outcome::Careless),
                    _ => None,
                };
                if let (Some(skill), Some(outcome), Some(b), Some(center), Ok(epoch), Some(assists)) =
                    (skill_of(name), outcome, band(b), band(center), epoch.parse::<u32>(), unit(assists))
                {
                    let st = &mut p.skills[skill.index()];
                    st.window = st.window.push(WindowEntry { outcome, band: b, center, epoch, assists });
                }
            }
            _ => {}
        }
    }
    p.calibration = Calibration { probes, done: calibrated };
    (dial, p)
}

#[cfg(target_arch = "wasm32")]
pub(crate) mod storage {
    const KEY: &str = "nat-han-adventures.progress";

    fn local_storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok().flatten()
    }

    pub fn read() -> Option<String> {
        local_storage()?.get_item(KEY).ok().flatten()
    }

    pub fn write(text: &str) -> Result<(), String> {
        let s = local_storage().ok_or("no localStorage")?;
        s.set_item(KEY, text).map_err(|e| format!("{e:?}"))
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod storage {
    use std::path::PathBuf;

    /// `$NATHAN_SAVE` if set (e.g. a scratch file for agent/headless runs), else
    /// `$XDG_DATA_HOME/nat-han-adventures/progress.txt` (or `~/.local/share/...`, `%APPDATA%\...`),
    /// falling back to next to the executable.
    pub fn path() -> PathBuf {
        let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
        if let Some(p) = env("NATHAN_SAVE") {
            return p;
        }
        let dir = env("XDG_DATA_HOME")
            .or_else(|| env("APPDATA"))
            .or_else(|| env("HOME").map(|h| h.join(".local/share")))
            .map(|d| d.join("Nat Han Adventures"))
            .or_else(|| std::env::current_exe().ok()?.parent().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("."));
        dir.join("progress.txt")
    }

    pub fn read() -> Option<String> {
        std::fs::read_to_string(path()).ok()
    }

    pub fn write(text: &str) -> Result<(), String> {
        let p = path();
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&p, text).map_err(|e| format!("{}: {e}", p.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_round_trip() {
        let mut p = Progress::default();
        p.record(0, 12, 63.25);
        p.record(1, 0, 5.5);
        p.record(2, 30, 120.125);
        let back = Progress::from_text(&p.to_text()).unwrap();
        assert_eq!(back, p);
        assert_eq!(back.unlocked, 4);
        assert_eq!(Progress::from_text(&Progress::default().to_text()), Some(Progress::default()));
        let seeded = Progress { last_seed: Some(424242), ..Progress::default() };
        assert_eq!(Progress::from_text(&seeded.to_text()), Some(seeded));
        let delayed = Progress { audio_delay_ms: 180, ..Progress::default() };
        assert_eq!(Progress::from_text(&delayed.to_text()), Some(delayed));
    }

    #[test]
    fn parse_is_lenient() {
        assert_eq!(Progress::from_text(""), None);
        assert_eq!(Progress::from_text("hello\nunlocked 3"), None);
        let p = Progress::from_text(
            "nat-han-adventures-progress 1\nunlocked 99\nlevel 2 7 10.0\nlevel 42 1 1.0\nlevel x y z\ngarbage\nlevel 3 5 NaN\n",
        )
        .unwrap();
        assert_eq!(p.unlocked, LEVEL_COUNT);
        assert_eq!(p.best_nuggets[2], Some(7));
        assert_eq!(p.best_time[2], Some(10.0));
        assert_eq!(p.best_nuggets[3], None);
        let p = Progress::from_text("nat-han-adventures-progress 1\nunlocked 0\n").unwrap();
        assert_eq!(p.unlocked, 1);
        let p = Progress::from_text("nat-han-adventures-progress 1\naudio-delay 9000\n").unwrap();
        assert_eq!(p.audio_delay_ms, MAX_AUDIO_DELAY_MS);
    }

    #[test]
    fn record_keeps_bests_and_unlocks() {
        let mut p = Progress::default();
        let o = p.record(0, 5, 60.0);
        assert!(o.first_clear && o.unlocked_next && !o.new_best_nuggets && !o.new_best_time);
        assert_eq!(p.unlocked, 2);

        // Worse run: nothing improves.
        let o = p.record(0, 3, 70.0);
        assert_eq!(o, RecordOutcome::default());
        assert_eq!((p.best_nuggets[0], p.best_time[0]), (Some(5), Some(60.0)));

        // More nuggets but slower: only nuggets improve. Bests are tracked independently.
        let o = p.record(0, 9, 80.0);
        assert!(o.new_best_nuggets && !o.new_best_time);
        let o = p.record(0, 1, 30.0);
        assert!(!o.new_best_nuggets && o.new_best_time);
        assert_eq!((p.best_nuggets[0], p.best_time[0]), (Some(9), Some(30.0)));

        // Replaying an early level never re-locks later ones.
        p.unlocked = 6;
        assert!(!p.record(1, 1, 1.0).unlocked_next);
        assert_eq!(p.unlocked, 6);

        // The last level can't unlock past the end.
        p.unlocked = LEVEL_COUNT;
        p.record(LEVEL_COUNT - 1, 1, 1.0);
        assert_eq!(p.unlocked, LEVEL_COUNT);
        // Out of range is ignored.
        assert_eq!(p.record(LEVEL_COUNT, 1, 1.0), RecordOutcome::default());
    }

    fn played_profile() -> PlayerProfile {
        use crate::adapt::{AdaptEvent, RoomResult, reduce};
        let mut p = PlayerProfile::new();
        for i in 0..14u64 {
            p = reduce(p, AdaptEvent::RoomStarted {
                room_id: i,
                skills: vec![(Skill::Precision, 3), (Skill::Grease, 2)],
                expected_deaths: 1,
                par_secs: Some(30.0),
            });
            let deaths = [0, 0, 3, 1, 0, 5][i as usize % 6];
            p = reduce(p, AdaptEvent::RoomFinished(RoomResult { deaths, time_secs: 21.5 + i as f32, ..default() }));
        }
        p
    }

    #[test]
    fn adaptive_state_round_trips() {
        let mut progress = Progress::default();
        progress.record(0, 3, 40.0);
        let p = played_profile();
        assert!(!p.skill(Skill::Precision).window.entries.is_empty());
        let story = StoryAssist::new(0.37);
        let text = save_text(&progress, &story, &p);
        assert_eq!(Progress::from_text(&text), Some(progress));
        let (dial, back) = adaptive_from_text(&text);
        assert_eq!(dial, 0.37);
        assert_eq!(back.assists, p.assists);
        assert_eq!(back.calibration, p.calibration);
        assert_eq!((back.rooms_played, back.streak), (p.rooms_played, p.streak));
        assert_eq!(back.skills, p.skills);
        // Saving what was loaded writes the same file.
        assert_eq!(save_text(&Progress::from_text(&text).unwrap(), &StoryAssist::new(dial), &back), text);
    }

    #[test]
    fn old_saves_load_with_a_fresh_adaptive_state() {
        let old = "nat-han-adventures-progress 1\nunlocked 3\nlevel 0 12 63.250\nlevel 1 4 20.000\n";
        let p = Progress::from_text(old).unwrap();
        assert_eq!(p.unlocked, 3);
        assert_eq!(p.best_nuggets[1], Some(4));
        let (dial, profile) = adaptive_from_text(old);
        assert_eq!(dial, 0.0);
        assert_eq!(profile, PlayerProfile::new());
        // Broken adaptive lines are skipped, values clamped.
        let (dial, profile) = adaptive_from_text(
            "story-assists 7\nprofile-assists NaN\nprofile-skill nope 3 0.5 0 -\nprofile-skill grease 99 0.5 0 -\nprofile-window grease X 1 1 0 0\nprofile-skill waltz 4 2 1 5\n",
        );
        assert_eq!(dial, 1.0);
        assert_eq!(profile.assists, 0.0);
        assert_eq!(profile.center(Skill::Grease), 1);
        assert!(profile.skill(Skill::Grease).window.entries.is_empty());
        let w = profile.skill(Skill::Waltz);
        assert_eq!((w.center, w.spread, w.epoch, w.fell_from), (4, 1.0, 1, Some(5)));
    }
}
