//! The editor's state and its per-frame logic (everything but drawing).

use std::path::{Path, PathBuf};

use nat_han_adventures::audio::live::song::CHANNELS;
use nat_han_adventures::audio::live::feel::Feel;
use nat_han_adventures::audio::live::{Input, SongFile, library};
use nat_han_adventures::audio::mml::{self, Options};
use nat_han_adventures::audio::{AudioOutput, Harmony};

use crate::feed::{AutoNat, Button, Gameplay};
use crate::model::{Part, STEP, SongDoc, excerpt, sections};
use crate::played;
use crate::player::{Context, Dials, Player};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Tracker,
    Piano,
    Text,
    Instruments,
}

impl View {
    pub fn parse(s: &str) -> Option<View> {
        match s {
            "tracker" => Some(View::Tracker),
            "piano" | "piano-roll" => Some(View::Piano),
            "text" => Some(View::Text),
            "instruments" | "inst" => Some(View::Instruments),
            _ => None,
        }
    }
}

/// A song file in `music/`.
#[derive(Debug, Clone)]
pub struct SongEntry {
    pub stem: String,
    pub title: String,
}

/// The tracker's cursor and settings.
#[derive(Debug, Clone)]
pub struct TrackerUi {
    pub row: usize,
    pub ch: usize,
    pub rec: bool,
    pub octave: i32,
    /// New notes' length, and how far the cursor moves after one, in rows.
    pub len: usize,
    pub step: usize,
    pub follow: bool,
    pub show_played: bool,
    /// The cursor row last scrolled to (a moved cursor brings the view along).
    pub scrolled: Option<usize>,
}

impl Default for TrackerUi {
    fn default() -> Self {
        TrackerUi { row: 0, ch: 0, rec: false, octave: 4, len: 2, step: 2, follow: true, show_played: false, scrolled: None }
    }
}

/// A drag in the piano roll.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Drag {
    Draw { step: usize, sound_pitch: i32 },
    Move { index: usize, grab: f64, pitch: i32 },
    Resize { index: usize },
    Velocity { index: usize },
}

#[derive(Debug, Clone)]
pub struct PianoUi {
    pub ch: usize,
    /// First bar shown, and how many.
    pub first_bar: usize,
    pub bars: usize,
    /// New notes' length, in steps.
    pub len: usize,
    pub ghosts: bool,
    pub overlay: bool,
    pub follow: bool,
    pub selected: Option<usize>,
    pub drag: Option<Drag>,
    /// Scroll the pitch axis to the channel's notes on the next frame.
    pub recenter: bool,
}

impl Default for PianoUi {
    fn default() -> Self {
        PianoUi { ch: 0, first_bar: 0, bars: 2, len: 2, ghosts: true, overlay: true, follow: true, selected: None, drag: None, recenter: true }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TextUi {
    pub cheat: bool,
    /// Insert this at the cursor (a cheat-sheet click).
    pub insert: Option<String>,
    /// Char index of the cursor.
    pub cursor: usize,
    pub focus: bool,
}

/// The instruments tab.
#[derive(Debug, Clone)]
pub struct InstUi {
    /// The instrument selected (by name).
    pub selected: Option<String>,
    /// The preview's channel and octave.
    pub ch: usize,
    pub octave: i32,
    /// The definition being typed (it's written to the song when it parses).
    pub draft: Option<(String, String)>,
}

impl Default for InstUi {
    fn default() -> Self {
        InstUi { selected: None, ch: 0, octave: 4, draft: None }
    }
}

/// The "as played" parts and charts, cached per song revision and harmony.
pub struct PlayedCache {
    pub revision: u64,
    pub harmony: Harmony,
    pub parts: Option<[Part; 4]>,
    pub written_chart: Vec<String>,
    pub played_chart: Vec<String>,
}

pub struct Editor {
    pub root: PathBuf,
    pub songs: Vec<SongEntry>,
    pub doc: SongDoc,
    pub view: View,
    pub player: Player,
    pub game: Gameplay,
    pub auto: AutoNat,
    pub dials: Dials,
    pub mute: [bool; 4],
    pub solo: [bool; 4],
    /// Channel levels, 0..=15 (12 = as written).
    pub level: [u8; 4],
    pub tracker: TrackerUi,
    pub piano: PianoUi,
    pub text: TextUi,
    pub inst: InstUi,
    /// Audition: play channel `.0` on instrument `.1` (a try-out, not written to the song).
    pub audition: Option<(usize, String)>,
    /// Loop the bars `selection` (a half-open range).
    pub loop_on: bool,
    pub selection: (usize, usize),
    pub status: Option<(String, bool)>,
    /// When the text last changed, and the revision playing.
    pub last_change: f64,
    pub swapped: u64,
    pub played: Option<PlayedCache>,
    pub now: f64,
    /// The feel each bar of the edited song was last committed in (the bottom strip's lane).
    pub feels: Vec<Feel>,
}

/// The repository root (where `music/` is).
pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The songs in `music/` (stem, title), or the built-in ones if there's no such directory.
pub fn list_songs(root: &Path) -> Vec<SongEntry> {
    let mut out: Vec<SongEntry> = std::fs::read_dir(root.join("music"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            (p.extension()? == "song").then(|| {
                let stem = p.file_stem()?.to_string_lossy().to_string();
                let text = std::fs::read_to_string(&p).ok()?;
                let title = SongFile::parse(&text).map(|s| s.title).unwrap_or_else(|_| "(doesn't parse)".into());
                Some(SongEntry { stem, title })
            })?
        })
        .collect();
    if out.is_empty() {
        out = library::FILES
            .iter()
            .map(|(stem, t)| SongEntry { stem: stem.to_string(), title: SongFile::parse(t).map(|s| s.title).unwrap_or_default() })
            .collect();
    }
    out.sort_by(|a, b| a.stem.cmp(&b.stem));
    out
}

/// Read a song's text (from disk, else the built-in copy).
pub fn read_song(root: &Path, stem: &str) -> Option<String> {
    std::fs::read_to_string(root.join(format!("music/{stem}.song"))).ok().or_else(|| library::text(stem).map(str::to_string))
}

/// Short title ("Sweet Georgia Brown" from "Sweet Georgia Brown (Bernie/..., 1925)").
pub fn short_title(t: &str) -> &str {
    t.split(" (").next().unwrap_or(t).trim()
}

impl Editor {
    pub fn new(root: PathBuf, stem: Option<&str>, output: AudioOutput) -> Editor {
        let songs = list_songs(&root);
        // The title music, unless asked.
        let stem = stem
            .map(str::to_string)
            .or_else(|| songs.iter().find(|s| s.stem == "sweet_georgia_brown").or(songs.first()).map(|s| s.stem.clone()))
            .unwrap_or_else(|| "sweet_georgia_brown".into());
        let text = read_song(&root, &stem).unwrap_or_default();
        Editor {
            doc: SongDoc::new(&stem, &text),
            root,
            songs,
            view: View::Tracker,
            player: Player::new(output),
            game: Gameplay::default(),
            auto: AutoNat::default(),
            dials: Dials::default(),
            mute: [false; 4],
            solo: [false; 4],
            level: [12; 4],
            tracker: TrackerUi::default(),
            piano: PianoUi::default(),
            text: TextUi { cheat: true, ..TextUi::default() },
            inst: InstUi::default(),
            audition: None,
            loop_on: false,
            selection: (0, 2),
            status: None,
            last_change: 0.0,
            swapped: 0,
            played: None,
            now: 0.0,
            feels: Vec::new(),
        }
    }

    pub fn say(&mut self, msg: impl Into<String>) {
        self.status = Some((msg.into(), false));
    }

    pub fn complain(&mut self, msg: impl Into<String>) {
        self.status = Some((msg.into(), true));
    }

    pub fn song(&self) -> Option<&SongFile> {
        self.doc.good.as_ref()
    }

    pub fn bar_beats(&self) -> f64 {
        self.song().map_or(4.0, SongFile::bar_beats)
    }

    pub fn bars(&self) -> usize {
        self.song().map_or(0, SongFile::bars)
    }

    pub fn rows_per_bar(&self) -> usize {
        (self.bar_beats() / STEP).round() as usize
    }

    pub fn open(&mut self, stem: &str) {
        let Some(text) = read_song(&self.root, stem) else {
            self.complain(format!("no music/{stem}.song"));
            return;
        };
        let was_playing = self.player.playing;
        self.player.stop();
        self.doc = SongDoc::new(stem, &text);
        self.tracker.row = 0;
        self.piano.first_bar = 0;
        self.piano.selected = None;
        self.piano.recenter = true;
        self.selection = (0, 2.min(self.bars().max(1)));
        self.loop_on = false;
        self.played = None;
        self.feels.clear();
        self.swapped = self.doc.revision;
        match self.doc.error() {
            Some(e) => self.complain(format!("music/{stem}.song: {e}")),
            None => self.say(format!("opened music/{stem}.song")),
        }
        if was_playing {
            self.play_from(0);
        }
    }

    /// The song the engine plays (the loop selection cut out, if on) and its offset in bars.
    fn engine_song(&self) -> Option<(SongFile, usize)> {
        let s = self.song()?;
        let auditioned;
        let s = match &self.audition {
            Some((ch, name)) => {
                let mut t = s.clone();
                t.sources[*ch] = format!("@i {name} {}", t.sources[*ch]);
                auditioned = SongFile::parse(&t.to_text()).unwrap_or_else(|_| s.clone());
                &auditioned
            }
            None => s,
        };
        if self.loop_on {
            let (a, b) = self.selection;
            let a = a.min(s.bars().saturating_sub(1));
            Some((excerpt(s, a, b.max(a + 1)), a))
        } else {
            Some((s.clone(), 0))
        }
    }

    /// Audition instrument `name` on channel `ch` in the song (again: stop auditioning).
    pub fn audition(&mut self, ch: usize, name: &str) {
        let same = self.audition.as_ref().is_some_and(|(c, n)| *c == ch && n == name);
        self.audition = (!same).then(|| (ch, name.to_string()));
        if self.player.playing {
            if let Some((song, offset)) = self.engine_song() {
                self.player.swap(song, offset);
            }
        } else if !same {
            let bar = self.cursor_bar();
            self.play_from(bar);
        }
    }

    /// Play a few notes (or a groove, for a kit) on instrument `name`, alone.
    pub fn preview(&mut self, name: &str) {
        let Some(song) = self.song() else { return };
        let kit = song.instruments.index(name).is_some_and(|k| song.instruments.is_kit(k));
        let ch = if kit { 3 } else { self.inst.ch.min(2) };
        let o = (self.inst.octave - if ch == 2 { 2 } else { 0 }).clamp(1, 6);
        let body = if kit {
            format!("@i {name} v12 k4 h8 h8 s4 h8 H8 | k8 k8 s8 s16 s16 x2 |")
        } else {
            format!("@i {name} v12 o{o} c4 e4 g4 > c4 | < c8 e8 g8 > c8 c2 | < {{c e g}}1 |")
        };
        let text = format!(
            "[song]\ntitle = preview\nbpm = {}\nloop = no\n[instruments]\n{}\n[{}]\n{body}\n",
            song.bpm,
            song.instruments_src,
            CHANNELS[ch].0
        );
        match SongFile::parse(&text).map_err(|e| e.to_string()).and_then(|s| nat_han_adventures::audio::synth::render_song(&s)) {
            Ok(r) => self.player.play_rendered(r),
            Err(e) => self.complain(format!("preview: {e}")),
        }
    }

    fn context(&self) -> Context {
        Context { filters: self.game.decided.map(|d| d.0).unwrap_or_default(), stats: self.game.band.stats }
    }

    pub fn play_from(&mut self, bar: usize) {
        let Some((song, offset)) = self.engine_song() else {
            self.complain("nothing to play: the song doesn't parse");
            return;
        };
        let beat = (bar.saturating_sub(offset) as f64 * song.bar_beats()).min((song.bars().saturating_sub(1)) as f64 * song.bar_beats());
        let ctx = self.context();
        self.player.play(&song, offset, beat.max(0.0), &self.dials, &ctx);
        self.swapped = self.doc.revision;
        if let Some(e) = self.player.error.clone() {
            self.complain(e);
        }
    }

    pub fn toggle_play(&mut self) {
        if self.player.playing {
            self.player.stop();
        } else {
            let bar = self.cursor_bar();
            self.play_from(bar);
        }
    }

    /// The bar the current view's cursor is in.
    pub fn cursor_bar(&self) -> usize {
        match self.view {
            View::Tracker | View::Instruments => self.tracker.row / self.rows_per_bar().max(1),
            View::Piano => self.piano.first_bar,
            View::Text => self.text_cursor_beat().map_or(0, |b| (b / self.bar_beats()).floor() as usize),
        }
        .min(self.bars().saturating_sub(1))
    }

    /// The song beat at the text cursor (inside a channel's MML: from the parser's walk).
    pub fn text_cursor_beat(&self) -> Option<f64> {
        let text = &self.doc.text;
        let byte = text.char_indices().nth(self.text.cursor).map_or(text.len(), |(i, _)| i);
        let line = text[..byte].matches('\n').count();
        let sec = sections(text).into_iter().find(|s| s.body.0 <= line && line < s.body.1.max(s.body.0 + 1))?;
        let ch = CHANNELS.iter().position(|(n, _)| *n == sec.name)?;
        let start: usize = text.split_inclusive('\n').take(sec.body.0).map(str::len).sum();
        let end: usize = text.split_inclusive('\n').take(sec.body.1).map(str::len).sum();
        let body = &text[start..end.min(text.len())];
        let parsed = mml::parse_with(body, CHANNELS[ch].1, Options::bars(None)).ok()?;
        let at = byte - start;
        // The last visit at or before the cursor (first pass of a repeat).
        parsed.visits.iter().filter(|v| v.pos <= at).map(|v| (v.pos, v.time)).max_by(|a, b| a.0.cmp(&b.0).then(b.1.total_cmp(&a.1))).map(|v| v.1)
    }

    pub fn set_view(&mut self, v: View) {
        self.view = v;
    }

    /// Recompute the mixer from mute / solo / levels and send it.
    pub fn remix(&mut self) {
        let any_solo = self.solo.iter().any(|s| *s);
        self.dials.mix = std::array::from_fn(|ch| {
            let audible = !self.mute[ch] && (!any_solo || self.solo[ch]);
            if audible { self.level[ch] as f32 / 12.0 } else { 0.0 }
        });
        self.player.post(Input::SetMix(self.dials.mix));
    }

    pub fn set_force_harmony(&mut self, h: Option<Harmony>) {
        self.dials.force_harmony = h;
        self.player.post(Input::ForceHarmony(h));
    }

    pub fn set_force_tuning(&mut self, t: Option<nat_han_adventures::audio::tuning::Tuning>) {
        self.dials.force_tuning = t;
        self.player.post(Input::ForceTuning(t));
    }

    pub fn set_force_feel(&mut self, f: Option<nat_han_adventures::audio::live::feel::Feel>) {
        self.dials.force_feel = f;
        self.player.post(Input::ForceFeel(f));
    }

    pub fn set_freedom(&mut self, f: [f32; 5]) {
        self.dials.freedom = f;
        self.player.post(self.dials.freedom_input());
    }

    /// A gameplay button.
    pub fn press(&mut self, b: Button) {
        let mut out = Vec::new();
        self.game.press(b, &mut out);
        for i in out {
            self.player.post(i);
        }
        if let Some(s) = b.sfx() {
            self.player.play_sfx(s);
        }
    }

    /// Edit channel `ch` (any view's grid edit); a refusal shows in the status line.
    pub fn edit(&mut self, r: Result<(), String>) {
        match r {
            Ok(()) => {
                self.last_change = self.now;
                self.status = None;
            }
            Err(e) => self.complain(e),
        }
    }

    /// Typed text.
    pub fn set_text(&mut self, text: String) {
        self.doc.set_text(text);
        self.last_change = self.now;
    }

    pub fn save(&mut self) {
        match self.doc.save(&self.root) {
            Ok(p) => {
                let msg = format!("saved {}", p.strip_prefix(&self.root).unwrap_or(&p).display());
                self.say(msg);
                self.songs = list_songs(&self.root);
            }
            Err(e) => self.complain(e),
        }
    }

    /// The harmony "as played" means: what's sounding, or a forced one.
    pub fn shown_harmony(&self) -> Harmony {
        if self.player.playing {
            self.player.published.state.harmony
        } else {
            self.dials.force_harmony.or(self.game.decided.map(|d| d.0.harmony)).unwrap_or_default()
        }
    }

    pub fn played(&mut self) -> &PlayedCache {
        let h = self.shown_harmony();
        let rev = self.doc.revision;
        let stale = self.played.as_ref().is_none_or(|p| p.revision != rev || p.harmony != h);
        if stale {
            let song = self.doc.good.as_ref();
            let parts = song.and_then(|s| played::parts(s, h));
            let chart = song.and_then(|s| s.chart.as_ref());
            self.played = Some(PlayedCache {
                revision: rev,
                harmony: h,
                parts,
                written_chart: chart.map(played::written).unwrap_or_default(),
                played_chart: chart.map(|c| played::played(c, h)).unwrap_or_default(),
            });
        }
        self.played.as_ref().expect("filled")
    }

    /// Once a frame, before drawing.
    pub fn update(&mut self, now: f64, dt: f64) {
        self.now = now;
        let ctx = self.context();
        self.player.update(now, dt, &self.dials, &ctx);
        if let Some(e) = self.player.error.take() {
            self.complain(e);
        }
        self.note_feels();
        if self.player.playing {
            if self.player.published.state.finished {
                self.player.stop();
            }
            // Play time runs while the music plays.
            let presses = self.auto.tick(self.game.now, dt as f32);
            for b in presses {
                self.press(b);
            }
            let mut out = Vec::new();
            self.game.tick(dt as f32, &mut out);
            for i in out {
                self.player.post(i);
            }
        }
        // Hot swap, debounced.
        if self.doc.revision != self.swapped && now - self.last_change > 0.25 {
            self.swapped = self.doc.revision;
            if self.player.playing
                && let Some((song, offset)) = self.engine_song()
            {
                self.player.swap(song, offset);
            }
        }
    }

    /// Remember the feel of every bar committed (by bar of the edited song).
    fn note_feels(&mut self) {
        let bars = self.bars();
        self.feels.resize(bars, Feel::Swing);
        if !self.player.playing {
            return;
        }
        for b in &self.player.published.state.upcoming {
            let bar = if b.harmony == Harmony::Waltz { b.slot.song_bar / 2 } else { b.slot.song_bar } + self.player.offset;
            if let Some(f) = self.feels.get_mut(bar) {
                *f = b.band.feel;
            }
        }
    }

    /// The loop selection changed (or was switched on or off): play the new one.
    pub fn reloop(&mut self) {
        if self.player.playing
            && let Some((song, offset)) = self.engine_song()
        {
            if self.loop_on {
                let bar = self.selection.0;
                self.play_from(bar);
            } else {
                self.player.swap(song, offset);
            }
        }
    }

    /// The heard position in the edited song (beats), when playing.
    pub fn playhead(&self) -> Option<f64> {
        self.player.playing.then(|| self.player.song_beat(self.now)).filter(|b| b.is_finite())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_text_cursor_knows_its_beat() {
        let mut e = Editor::new(repo_root(), Some("sweet_georgia_brown"), AudioOutput::Headless);
        let text = e.doc.text.clone();
        // The cursor on bar 5's first note of the lead (`o5 r4 g4 a4 b4 |` starts the line).
        let at = text.find("o5 r4 g4 a4 b4").unwrap();
        e.text.cursor = text[..at].chars().count() + 3;
        assert_eq!(e.text_cursor_beat(), Some(16.0));
        e.view = View::Text;
        assert_eq!(e.cursor_bar(), 4);
        // In the noise channel's repeat: its first pass.
        let at = text.find("k8 s8 s8 s8").unwrap();
        e.text.cursor = text[..at].chars().count();
        assert_eq!(e.text_cursor_beat(), Some(28.0));
    }

    #[test]
    fn mute_and_solo_mix() {
        let mut e = Editor::new(repo_root(), Some("tiger_rag"), AudioOutput::Headless);
        e.solo[2] = true;
        e.remix();
        assert_eq!(e.dials.mix, [0.0, 0.0, 1.0, 0.0]);
        e.solo[2] = false;
        e.mute[0] = true;
        e.level[3] = 6;
        e.remix();
        assert_eq!(e.dials.mix, [0.0, 1.0, 1.0, 0.5]);
    }

    /// The instruments tab's try-out plays the song with a channel switched, without writing
    /// it; the preview renders.
    #[test]
    fn auditions_switch_a_channel_without_writing() {
        let mut e = Editor::new(repo_root(), Some("tiger_rag"), AudioOutput::Headless);
        let before = e.doc.text.clone();
        e.audition(0, "brass");
        let (song, _) = e.engine_song().unwrap();
        let k = song.instruments.index("brass").unwrap();
        assert!(song.tracks[0].events.iter().all(|ev| ev.inst == k));
        assert_eq!(e.doc.text, before);
        e.audition(0, "brass");
        assert!(e.audition.is_none());
        e.preview("brass");
        e.preview("brushes");
        assert!(e.status.as_ref().is_none_or(|s| !s.1), "{:?}", e.status);
    }

    /// The loop selection plays bars 5-6 over and over.
    #[test]
    fn the_selection_loops() {
        let mut e = Editor::new(repo_root(), Some("sweet_georgia_brown"), AudioOutput::Headless);
        e.selection = (4, 6);
        e.loop_on = true;
        e.play_from(4);
        let mut seen = Vec::new();
        for k in 1..=60 * 6 {
            e.update(k as f64 / 60.0, 1.0 / 60.0);
            if let Some(b) = e.playhead() {
                seen.push((b / 4.0).floor() as usize);
            }
        }
        assert_eq!(e.player.offset, 4);
        assert!(seen.iter().all(|b| (4..6).contains(b)), "{:?}", &seen[..10]);
        assert!(seen.contains(&4) && seen.contains(&5));
        assert!(e.player.published.clock.position.pass >= 1, "it looped");
    }

    /// Play, edit, and the edit is swapped into the running engine at the next bar line.
    #[test]
    fn edits_are_hot_swapped() {
        let mut e = Editor::new(repo_root(), Some("sweet_georgia_brown"), AudioOutput::Headless);
        e.play_from(2);
        let mut t = 0.0;
        let mut step = |e: &mut Editor, secs: f64| {
            let end = t + secs;
            while t < end {
                t += 1.0 / 60.0;
                e.update(t, 1.0 / 60.0);
            }
        };
        step(&mut e, 0.5);
        assert!(e.player.published.playing);
        let bar = e.player.published.state.position.song_bar;
        assert!((2..=3).contains(&bar), "started at bar 3: {bar}");
        let r = e.doc.edit(0, |p| p.remove(0));
        e.edit(r);
        step(&mut e, 2.0);
        assert_eq!(e.swapped, e.doc.revision);
        let bar2 = e.player.published.state.position.song_bar;
        assert!(bar2 > bar && bar2 <= bar + 3, "carried on from where it was: {bar} -> {bar2}");
    }
}
