//! The level editor's state and everything it does that isn't drawing: the document and its
//! undo, files, the validator (on demand, on its own thread), the playtest (the real game in a
//! second process, reloading the level file whenever the editor writes it).

use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, OnceLock};

use nat_han_adventures::game::sideb::SideBLevel;
use nat_han_adventures::level::doc::{LevelDoc, key_of, value_of};
use nat_han_adventures::level::validate::{self, Options, Physics};
use nat_han_adventures::level::{Level, Topic, Tweak};

/// What a left click does on the canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    /// Paint the brush's tile (shift-drag: a rectangle of it).
    Tiles,
    /// Paint the tweak brush's letter on the tweak layer.
    Tweaks,
    /// Drag out a gate mark of the gate brush's kind.
    Gates,
    /// Click a cell: a hint spot there (a `say@` line on a checkpoint).
    Spots,
    /// Click a cell to inspect it; drag to select a rectangle (copy, cut, paste, fill).
    Select,
}

/// A file the editor can open.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Relative to the repo root.
    pub path: PathBuf,
    pub label: String,
}

/// A validator run's findings, for the doc text it ran on.
pub struct Validation {
    pub text: String,
    pub errs: Vec<String>,
    /// The story's house rules broken (Side A only).
    pub rules: Vec<String>,
    pub summary: Vec<String>,
    /// The reach map (`validate` module's dump): one row of chars per grid row.
    pub reach: Vec<Vec<char>>,
    pub secs: f32,
}

/// The doc, parsed (or why not).
pub struct Parsed {
    pub level: Result<Level, String>,
    pub side_b: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditKind {
    Stroke(u64),
    Field(&'static str),
    Other,
}

/// A stroke on the canvas: where it started, the last cell painted, right button.
#[derive(Debug, Clone, Copy, Default)]
pub struct Drag {
    pub from: Option<(usize, usize)>,
    pub last: Option<(usize, usize)>,
    pub erase: bool,
}

pub struct Editor {
    pub root: PathBuf,
    pub path: Option<PathBuf>,
    pub doc: LevelDoc,
    pub saved: String,
    pub parsed: Parsed,
    undo: Vec<LevelDoc>,
    redo: Vec<LevelDoc>,
    last_edit: Option<EditKind>,
    pub stroke: u64,

    pub tool: Tool,
    pub brush: char,
    pub tweak_brush: char,
    pub gate_brush: Topic,
    /// Inspected cell, and the selected rectangle (inclusive corners).
    pub selected: Option<(usize, usize)>,
    pub selection: Option<((usize, usize), (usize, usize))>,
    pub clipboard: Option<(Vec<Vec<char>>, Vec<Vec<char>>)>,
    pub drag: Drag,
    /// Scroll the canvas to this cell next frame.
    pub center_on: Option<(usize, usize)>,

    // View.
    pub zoom: f32,
    pub pan: bevy_egui::egui::Vec2,
    pub show_grid: bool,
    pub show_tweaks: bool,
    pub show_marks: bool,
    pub show_reach: bool,

    pub validation: Option<Validation>,
    validating: Option<Receiver<Validation>>,
    pub validate_started: Option<std::time::Instant>,

    pub child: Option<Child>,
    pub play_log: Vec<String>,
    play_rx: Option<Receiver<String>>,
    pub live: bool,
    pushed: String,

    pub status: String,
    pub save_as: String,
    pub files: Vec<Entry>,
}

/// The validator's physics tables, built once (they take a moment).
fn physics() -> &'static Arc<Physics> {
    static P: OnceLock<Arc<Physics>> = OnceLock::new();
    P.get_or_init(|| Arc::new(Physics::new()))
}

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every level file: the campaign, Side B, and `levels/custom/`.
pub fn list_files(root: &Path) -> Vec<Entry> {
    let mut out = Vec::new();
    for (dir, tag) in [("levels", "A"), ("levels/side-b", "B"), ("levels/custom", "custom")] {
        let mut v: Vec<Entry> = std::fs::read_dir(root.join(dir))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                (p.extension()? == "txt").then(|| {
                    let rel = p.strip_prefix(root).unwrap_or(&p).to_path_buf();
                    let name = std::fs::read_to_string(&p)
                        .ok()
                        .and_then(|t| t.lines().find(|l| key_of(l) == Some("name")).map(|l| value_of(l).to_string()))
                        .unwrap_or_default();
                    let stem = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
                    Entry { label: format!("{tag} {stem}  {name}"), path: rel }
                })
            })
            .collect();
        v.sort_by(|a, b| a.path.cmp(&b.path));
        out.extend(v);
    }
    out
}

/// A file name for a level name: `levels/custom/<slug>.txt`.
pub fn slug(name: &str) -> String {
    let s: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let s = s.split('-').filter(|w| !w.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() { "untitled".into() } else { s }
}

pub fn parse(doc: &LevelDoc) -> Parsed {
    let side_b = doc.is_side_b();
    let level = if side_b { SideBLevel::parse(&doc.text()).map(|b| b.level) } else { doc.level() };
    Parsed { level, side_b }
}

impl Editor {
    pub fn new(root: PathBuf, open: Option<&Path>) -> Editor {
        let doc = LevelDoc::blank("Untitled", 60, 14, false);
        let mut ed = Editor {
            files: list_files(&root),
            root,
            path: None,
            saved: doc.text(),
            parsed: parse(&doc),
            doc,
            undo: Vec::new(),
            redo: Vec::new(),
            last_edit: None,
            stroke: 0,
            tool: Tool::Tiles,
            brush: '#',
            tweak_brush: 'a',
            gate_brush: Topic::Giant,
            selected: None,
            selection: None,
            clipboard: None,
            drag: Drag::default(),
            center_on: None,
            zoom: 2.0,
            pan: bevy_egui::egui::Vec2::ZERO,
            show_grid: true,
            show_tweaks: true,
            show_marks: true,
            show_reach: true,
            validation: None,
            validating: None,
            validate_started: None,
            child: None,
            play_log: Vec::new(),
            play_rx: None,
            live: false,
            pushed: String::new(),
            status: String::new(),
            save_as: String::new(),
        };
        if let Some(p) = open {
            ed.open(p);
        }
        ed
    }

    /// Open a level file (relative to the repo root, or absolute).
    pub fn open(&mut self, path: &Path) {
        let full = if path.is_absolute() { path.to_path_buf() } else { self.root.join(path) };
        match std::fs::read_to_string(&full).map_err(|e| e.to_string()).and_then(|t| LevelDoc::parse(&t).map(|d| (t, d))) {
            Ok((text, doc)) => {
                self.saved = text;
                self.set_doc(doc);
                self.undo.clear();
                self.redo.clear();
                self.path = Some(full.strip_prefix(&self.root).map(Path::to_path_buf).unwrap_or(full));
                self.validation = None;
                self.selected = None;
                self.selection = None;
                self.pan = bevy_egui::egui::Vec2::ZERO;
                self.status = format!("opened {}", self.path.as_ref().unwrap().display());
            }
            Err(e) => self.status = format!("can't open {}: {e}", full.display()),
        }
    }

    pub fn new_level(&mut self, side_b: bool) {
        self.set_doc(LevelDoc::blank(if side_b { "Untitled Side B" } else { "Untitled" }, 60, 14, side_b));
        self.saved = String::new();
        self.path = None;
        self.undo.clear();
        self.redo.clear();
        self.validation = None;
        self.status = "new level: Save As names it".into();
    }

    fn set_doc(&mut self, doc: LevelDoc) {
        self.doc = doc;
        self.parsed = parse(&self.doc);
    }

    pub fn dirty(&self) -> bool {
        self.doc.text() != self.saved
    }

    /// Change the doc (undoable). Edits of one kind in a row (a stroke, typing in one field)
    /// are one undo step.
    fn edit_kind(&mut self, kind: EditKind, f: impl FnOnce(&mut LevelDoc)) {
        let before = self.doc.clone();
        f(&mut self.doc);
        if self.doc == before {
            return;
        }
        if self.last_edit != Some(kind) || kind == EditKind::Other {
            self.undo.push(before);
            if self.undo.len() > 500 {
                self.undo.remove(0);
            }
        }
        self.last_edit = Some(kind);
        self.redo.clear();
        self.parsed = parse(&self.doc);
    }

    /// A canvas stroke's edit (one undo step per stroke).
    pub fn stroke_edit(&mut self, f: impl FnOnce(&mut LevelDoc)) {
        self.edit_kind(EditKind::Stroke(self.stroke), f);
    }

    /// A header field's edit (typing into one field is one undo step).
    pub fn field_edit(&mut self, field: &'static str, f: impl FnOnce(&mut LevelDoc)) {
        self.edit_kind(EditKind::Field(field), f);
    }

    pub fn edit(&mut self, f: impl FnOnce(&mut LevelDoc)) {
        self.edit_kind(EditKind::Other, f);
    }

    pub fn undo(&mut self) {
        if let Some(d) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.doc, d));
            self.parsed = parse(&self.doc);
            self.last_edit = None;
        }
    }

    pub fn redo(&mut self) {
        if let Some(d) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.doc, d));
            self.parsed = parse(&self.doc);
            self.last_edit = None;
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Write the doc to `path` (relative to the root). Refuses a level that doesn't parse.
    pub fn save_to(&mut self, path: PathBuf) {
        if let Err(e) = &self.parsed.level {
            self.status = format!("not saved: {e}");
            return;
        }
        let full = self.root.join(&path);
        if let Some(dir) = full.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let text = self.doc.text();
        match std::fs::write(&full, &text) {
            Ok(()) => {
                self.saved = text;
                self.status = format!("saved {}", path.display());
                self.path = Some(path);
                self.files = list_files(&self.root);
                if self.child.is_some() {
                    self.push_playtest(None);
                }
            }
            Err(e) => self.status = format!("can't save {}: {e}", full.display()),
        }
    }

    pub fn save(&mut self) {
        match self.path.clone() {
            Some(p) => self.save_to(p),
            None => {
                let name = self.doc.get_field("name").unwrap_or("untitled").to_string();
                self.save_to(PathBuf::from(format!("levels/custom/{}.txt", slug(&name))));
            }
        }
    }

    // ─── Validation ──────────────────────────────────────────────────────────

    pub fn validating(&self) -> bool {
        self.validating.is_some()
    }

    /// Run the validator on the doc as it is now (on its own thread).
    pub fn validate(&mut self) {
        let Ok(level) = self.parsed.level.clone() else {
            self.status = "fix the parse error first".into();
            return;
        };
        let text = self.doc.text();
        let side_b = self.parsed.side_b;
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let t0 = std::time::Instant::now();
            let report = validate::check_with(&level, &Options { dump: true, rules_apart: true }, physics());
            let mut summary = Vec::new();
            if side_b {
                summary.push("Side B: the story's house rules (nuggets, checkpoints, lines) don't apply".into());
            }
            if !level.tweaks.is_empty() {
                summary.push("the validator doesn't model tweaks: playtest them".into());
            }
            for (gate, to_goal, nuggets, splats) in &report.gates {
                summary.push(format!(
                    "{}{}{}{}",
                    gate.name(),
                    if *to_goal { " (on the way to the goal)" } else { "" },
                    if *nuggets > 0 { format!(", {nuggets} nuggets behind it") } else { String::new() },
                    if *splats > 0 { format!(", {splats} splats") } else { String::new() },
                ));
            }
            summary.push(format!("deaths the design needs: {}", report.deaths));
            if !report.segments.is_empty() {
                summary.push(format!("respawn-to-respawn path costs: {:?}", report.segments));
            }
            for l in &report.lessons {
                summary.push(format!(
                    "{} first at {},{}: {}",
                    l.topic.word(),
                    l.at.0,
                    l.at.1,
                    if l.taught() { "taught" } else { "no hint before it" }
                ));
            }
            let reach = report
                .dump
                .as_deref()
                .unwrap_or("")
                .lines()
                .filter_map(|l| {
                    let (n, row) = l.get(..3).zip(l.get(4..))?;
                    n.trim().parse::<usize>().ok().map(|_| row.chars().collect())
                })
                .collect();
            let rules = if side_b { Vec::new() } else { report.rules };
            let _ = tx.send(Validation { text, errs: report.errs, rules, summary, reach, secs: t0.elapsed().as_secs_f32() });
        });
        self.validating = Some(rx);
        self.validate_started = Some(std::time::Instant::now());
        self.status = "validating...".into();
    }

    /// Is the last validation for the doc as it is now?
    pub fn validation_fresh(&self) -> bool {
        self.validation.as_ref().is_some_and(|v| v.text == self.doc.text())
    }

    // ─── Playtest ────────────────────────────────────────────────────────────

    pub fn playtest_file(&self) -> PathBuf {
        self.root.join("target/level-editor/playtest.txt")
    }

    pub fn playing(&self) -> bool {
        self.child.is_some()
    }

    /// The doc as the game should get it: Nat's start moved to `from`, if given (Side A).
    fn playtest_text(&self, from: Option<(usize, usize)>) -> String {
        let mut doc = self.doc.clone();
        if let Some((c, r)) = from {
            for row in 0..doc.height() {
                for col in 0..doc.width() {
                    if doc.get(col, row) == 'P' {
                        doc.set(col, row, '.');
                    }
                }
            }
            doc.set(c, r, 'P');
        }
        doc.text()
    }

    /// Hand the doc to the running game (it reloads the file and starts over).
    pub fn push_playtest(&mut self, from: Option<(usize, usize)>) {
        let text = self.playtest_text(from);
        let file = self.playtest_file();
        let _ = std::fs::create_dir_all(file.parent().unwrap());
        if let Err(e) = std::fs::write(&file, &text) {
            self.status = format!("can't write {}: {e}", file.display());
        }
        self.pushed = self.doc.text();
    }

    /// Play the level in the real game (a second window), from the start or from `from`. If
    /// the game's already running, it reloads.
    pub fn play(&mut self, from: Option<(usize, usize)>) {
        if let Err(e) = &self.parsed.level {
            self.status = format!("can't play: {e}");
            return;
        }
        self.push_playtest(from);
        if self.child.is_some() {
            self.status = "sent to the game".into();
            return;
        }
        let exe = std::env::current_exe().expect("the editor's own path");
        let child = Command::new(exe)
            .arg("--play")
            .env("NATHAN_LEVEL", self.playtest_file())
            .stdout(Stdio::piped())
            .spawn();
        match child {
            Ok(mut c) => {
                let out = c.stdout.take().unwrap();
                let (tx, rx) = channel();
                std::thread::spawn(move || {
                    for line in std::io::BufReader::new(out).lines().map_while(Result::ok) {
                        if tx.send(line).is_err() {
                            break;
                        }
                    }
                });
                self.play_rx = Some(rx);
                self.child = Some(c);
                self.status = "playing (Esc pauses; quit to the title closes it)".into();
            }
            Err(e) => self.status = format!("can't start the game: {e}"),
        }
    }

    pub fn stop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        self.play_rx = None;
    }

    /// Once a frame: collect the validator's verdict and the game's output.
    pub fn poll(&mut self) {
        if let Some(rx) = &self.validating
            && let Ok(v) = rx.try_recv()
        {
            self.status = if v.errs.is_empty() && v.rules.is_empty() {
                format!("valid ({:.1}s)", v.secs)
            } else {
                format!("{} problems ({:.1}s)", v.errs.len() + v.rules.len(), v.secs)
            };
            self.validation = Some(v);
            self.validating = None;
            self.show_reach = true;
        }
        if let Some(rx) = &self.play_rx {
            while let Ok(line) = rx.try_recv() {
                if let Some(l) = line.strip_prefix("playtest: ") {
                    self.play_log.push(l.to_string());
                    if self.play_log.len() > 50 {
                        self.play_log.remove(0);
                    }
                }
            }
        }
        if let Some(c) = &mut self.child
            && let Ok(Some(_)) = c.try_wait()
        {
            self.child = None;
            self.play_rx = None;
            self.status = "game closed".into();
        }
        if self.live && self.child.is_some() && self.parsed.level.is_ok() && self.doc.text() != self.pushed {
            self.push_playtest(None);
        }
    }

    // ─── Header helpers ──────────────────────────────────────────────────────

    /// The `tweak` lines: (letter, its tweak, header line index).
    pub fn tweaks(&self) -> Vec<(char, Tweak, usize)> {
        self.doc
            .lines_where(|k| k.starts_with("tweak "))
            .into_iter()
            .filter_map(|i| {
                let line = &self.doc.header[i];
                let ch = key_of(line)?.strip_prefix("tweak ")?.trim().chars().next()?;
                Some((ch, Tweak::parse(value_of(line)).unwrap_or_default(), i))
            })
            .collect()
    }

    /// Add a tweak (the next free letter) and make it the brush.
    pub fn add_tweak(&mut self, t: Tweak) -> char {
        let used: Vec<char> = self.tweaks().iter().map(|t| t.0).collect();
        let ch = ('a'..='z').find(|c| !used.contains(c)).unwrap_or('z');
        self.edit(|d| d.push_line(format!("tweak {ch}: {}", t.to_value())));
        self.tweak_brush = ch;
        self.tool = Tool::Tweaks;
        ch
    }

    /// The set list (`setlist:`), in order: (word, count).
    pub fn setlist(&self) -> Vec<(String, u32)> {
        self.doc
            .get_field("setlist")
            .unwrap_or("")
            .split_whitespace()
            .filter_map(|it| {
                let (w, n) = it.split_once('=')?;
                Some((w.to_string(), n.parse().ok()?))
            })
            .collect()
    }

    pub fn set_setlist(&mut self, list: &[(String, u32)]) {
        let v = list.iter().map(|(w, n)| format!("{w}={n}")).collect::<Vec<_>>().join(" ");
        self.field_edit("setlist", |d| d.set_field("setlist", Some(&v)));
    }

    /// Make the level Side B (a set list) or Side A (none).
    pub fn set_side_b(&mut self, on: bool) {
        if on == self.parsed.side_b {
            return;
        }
        self.edit(|d| {
            if on {
                d.set_field("setlist", Some("giant=1 gap=1"));
            } else {
                d.set_field("setlist", None);
            }
        });
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        self.stop();
    }
}
