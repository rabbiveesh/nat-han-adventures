//! Persistent progress: which levels are unlocked and the best nugget haul / time per level.
//! Stored in `localStorage` on the web and a small text file on native.
//!
//! [`plugin`] (part of `gameplay`) only creates the in-memory [`Progress`]: it never touches disk,
//! so headless tests stay hermetic. [`persistence_plugin`] (added by the UI when there's a real
//! window) loads it at startup and writes it back whenever it changes.

use bevy::prelude::*;

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
}

impl Default for Progress {
    fn default() -> Self {
        Self { unlocked: 1, best_nuggets: [None; LEVEL_COUNT], best_time: [None; LEVEL_COUNT] }
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
    /// ```
    /// (`level <index> <best nuggets> <best time secs>`, only for completed levels.)
    pub fn to_text(&self) -> String {
        let mut s = format!("{HEADER}\nunlocked {}\n", self.unlocked);
        for i in 0..LEVEL_COUNT {
            if let (Some(n), Some(t)) = (self.best_nuggets[i], self.best_time[i]) {
                s.push_str(&format!("level {i} {n} {t:.3}\n"));
            }
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
                _ => {}
            }
        }
        Some(p)
    }
}

fn load_progress(mut commands: Commands) {
    match storage::read().as_deref().map(Progress::from_text) {
        Some(Some(p)) => {
            info!("loaded progress: {} level(s) unlocked", p.unlocked);
            commands.insert_resource(p);
        }
        Some(None) => warn!("ignoring unreadable save data"),
        None => {}
    }
}

fn save_progress(progress: Res<Progress>, mut last: Local<Option<String>>) {
    if !progress.is_changed() {
        return;
    }
    let text = progress.to_text();
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
}
