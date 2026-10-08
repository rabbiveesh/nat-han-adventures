//! The song model the three views share.
//!
//! The `.song` text is the one source of truth ([`SongDoc::text`]); the real parser
//! ([`SongFile::parse`]) turns it into tracks, and the tracker and the piano roll show those as
//! [`Part`]s: one channel's sounding notes (rests are the gaps). An edit in either grid is an
//! operation on a [`Part`] ([`Part::insert`], [`Part::remove`], [`Part::move_note`],
//! [`Part::resize`], ...), written back as MML by the formatter ([`Part::to_mml`]: one bar per
//! `|`, four bars a line, notes across a bar line tied) and spliced into that channel's section
//! of the text; then the text is parsed again. Typing in the text view re-parses on its own.
//! So every view edits the same text and sees the same parse.
//!
//! A grid edit rewrites the edited channel in the formatter's canonical form: its comments and
//! `[ ]` repeats are written out. The other channels, the header and the chart stay as typed.

use nat_han_adventures::audio::chart::{self, Chart};
use nat_han_adventures::audio::live::song::{CHANNELS, SongError};
use nat_han_adventures::audio::live::SongFile;
use nat_han_adventures::audio::mml::{Arp, Channel, Drum, EventKind, Track};

/// A tracker row / piano-roll snap: a 16th, in beats.
pub const STEP: f64 = 0.25;
/// Time tolerance, in beats.
pub const EPS: f64 = 1e-6;

/// What a note plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sound {
    Note(u8),
    Arp(Arp),
    Drum(Drum),
}

impl Sound {
    /// A pitch to sort and draw by (the lowest note of a chord; drums by kind).
    pub fn pitch(&self) -> u8 {
        match self {
            Sound::Note(n) => *n,
            Sound::Arp(a) => *a.notes().iter().min().unwrap_or(&60),
            Sound::Drum(d) => DRUMS.iter().position(|x| x == d).unwrap_or(0) as u8,
        }
    }

    /// Transposed by `semis` (drums: moved to another drum row).
    pub fn shifted(&self, semis: i32) -> Sound {
        let sh = |n: u8| (n as i32 + semis).clamp(LOWEST as i32, HIGHEST as i32) as u8;
        match self {
            Sound::Note(n) => Sound::Note(sh(*n)),
            Sound::Arp(a) => {
                let lo = a.notes().iter().copied().min().unwrap_or(60) as i32;
                let hi = a.notes().iter().copied().max().unwrap_or(60) as i32;
                let s = semis.clamp(LOWEST as i32 - lo, HIGHEST as i32 - hi);
                let notes: Vec<u8> = a.notes().iter().map(|&n| (n as i32 + s) as u8).collect();
                Sound::Arp(Arp::new(&notes))
            }
            Sound::Drum(d) => {
                let i = DRUMS.iter().position(|x| x == d).unwrap_or(0) as i32;
                Sound::Drum(DRUMS[(i + semis).clamp(0, DRUMS.len() as i32 - 1) as usize])
            }
        }
    }
}

/// The drums, in row order (bottom to top in the piano roll).
pub const DRUMS: [Drum; 5] = [Drum::Kick, Drum::Snare, Drum::ClosedHat, Drum::OpenHat, Drum::Crash];
/// The lowest and highest notes MML can write (`o0 c` .. `o8 b`).
pub const LOWEST: u8 = 12;
pub const HIGHEST: u8 = 119;

/// One note of a [`Part`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Note {
    /// Beats from the start of the song (unswung, as written).
    pub start: f64,
    pub dur: f64,
    pub sound: Sound,
    /// 0..=15.
    pub volume: u8,
    /// 0..=3 (pulse channels).
    pub duty: u8,
    /// Slurred from the note before (`c4&d4`): only right after another note.
    pub tie: bool,
}

impl Note {
    pub fn end(&self) -> f64 {
        self.start + self.dur
    }
}

/// One channel as notes, in time order, never overlapping.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    /// 0 pulse 1, 1 pulse 2, 2 triangle, 3 noise.
    pub ch: usize,
    pub notes: Vec<Note>,
    /// The song's length in beats (every edited channel is written this long).
    pub length: f64,
    pub bar_beats: f64,
}

/// Parser defaults (`o4`, `v12`, `@2`).
const DEFAULT_OCTAVE: i32 = 4;
const DEFAULT_VOLUME: u8 = 12;
const DEFAULT_DUTY: u8 = 2;

impl Part {
    pub fn channel(&self) -> Channel {
        CHANNELS[self.ch].1
    }

    pub fn drums(&self) -> bool {
        self.channel() == Channel::Drums
    }

    /// Channel `ch` of a parsed song.
    pub fn of(song: &SongFile, ch: usize) -> Part {
        Self::from_track(ch, &song.tracks[ch], song.beats(), song.bar_beats())
    }

    pub fn from_track(ch: usize, track: &Track, length: f64, bar_beats: f64) -> Part {
        let pulse = ch < 2;
        let notes = track
            .events
            .iter()
            .filter_map(|e| {
                let sound = match e.kind {
                    EventKind::Note(n) => Sound::Note(n),
                    EventKind::Arp(a) => Sound::Arp(a),
                    EventKind::Drum(d) => Sound::Drum(d),
                    EventKind::Rest => return None,
                };
                // Duty only means something on the pulse channels.
                let duty = if pulse { e.duty } else { DEFAULT_DUTY };
                Some(Note { start: e.start, dur: e.dur, sound, volume: e.volume, duty, tie: e.tie && ch < 3 })
            })
            .collect();
        let mut p = Part { ch, notes, length, bar_beats };
        p.normalize();
        p
    }

    pub fn bars(&self) -> usize {
        (self.length / self.bar_beats - EPS).ceil().max(0.0) as usize
    }

    /// The note sounding at beat `t`.
    pub fn at(&self, t: f64) -> Option<usize> {
        self.notes.iter().position(|n| n.start <= t + EPS && t + EPS < n.end())
    }

    /// The note starting in `[a, b)`.
    pub fn starting_in(&self, a: f64, b: f64) -> impl Iterator<Item = usize> + '_ {
        self.notes.iter().enumerate().filter(move |(_, n)| n.start >= a - EPS && n.start < b - EPS).map(|(i, _)| i)
    }

    /// Insert a note: it cuts the note sounding where it starts and replaces the notes that
    /// start under it (mono channels). Clipped to the song. Returns its index.
    pub fn insert(&mut self, mut n: Note) -> Option<usize> {
        n.start = n.start.max(0.0);
        let end = n.end().min(self.length);
        if end <= n.start + EPS {
            return None;
        }
        n.dur = end - n.start;
        n.tie = false;
        let (s, e) = (n.start, n.end());
        self.notes.retain(|m| !(m.start >= s - EPS && m.start < e - EPS));
        for m in &mut self.notes {
            if m.start < s - EPS && m.end() > s + EPS {
                m.dur = s - m.start;
            }
        }
        let i = self.notes.partition_point(|m| m.start < s);
        self.notes.insert(i, n);
        self.normalize();
        self.notes.iter().position(|m| (m.start - s).abs() < EPS)
    }

    pub fn remove(&mut self, i: usize) {
        if i < self.notes.len() {
            self.notes.remove(i);
            self.normalize();
        }
    }

    /// Move note `i` to `start` playing `sound` (its length kept). Returns its new index.
    pub fn move_note(&mut self, i: usize, start: f64, sound: Sound) -> Option<usize> {
        let mut n = *self.notes.get(i)?;
        self.notes.remove(i);
        n.start = start.clamp(0.0, (self.length - n.dur).max(0.0));
        n.sound = sound;
        self.insert(n)
    }

    /// Change note `i`'s length (it replaces the notes it now covers).
    pub fn resize(&mut self, i: usize, dur: f64) -> Option<usize> {
        let mut n = *self.notes.get(i)?;
        if dur <= EPS {
            self.remove(i);
            return None;
        }
        self.notes.remove(i);
        n.dur = dur;
        let tie = n.tie;
        let k = self.insert(n)?;
        // Resizing keeps a slur.
        self.notes[k].tie = tie;
        self.normalize();
        Some(k)
    }

    pub fn set_volume(&mut self, i: usize, v: u8) {
        if let Some(n) = self.notes.get_mut(i) {
            n.volume = v.min(15);
        }
    }

    /// End the note sounding at `t` there (a tracker's note-off).
    pub fn cut_at(&mut self, t: f64) {
        if let Some(i) = self.at(t)
            && self.notes[i].start < t - EPS
        {
            self.notes[i].dur = t - self.notes[i].start;
            self.normalize();
        }
    }

    /// Sorted, clipped to the song, slurs only between touching notes.
    fn normalize(&mut self) {
        self.notes.sort_by(|a, b| a.start.total_cmp(&b.start));
        let len = self.length;
        self.notes.retain(|n| n.start < len - EPS && n.dur > EPS);
        for n in &mut self.notes {
            if n.end() > len {
                n.dur = len - n.start;
            }
        }
        let drums = self.drums();
        for k in 0..self.notes.len() {
            let touching = k > 0 && (self.notes[k - 1].end() - self.notes[k].start).abs() < EPS;
            if drums || !touching {
                self.notes[k].tie = false;
            }
        }
    }

    /// The channel as MML: one bar per `|`, four bars a line (each ending with its bar numbers),
    /// explicit lengths, the octave at the start of every bar, notes across a bar line tied.
    /// Always whole bars, `length` long.
    pub fn to_mml(&self) -> String {
        let bb = self.bar_beats;
        let bars = self.bars();
        let pulse = self.ch < 2;
        let mut out: Vec<Vec<String>> = vec![Vec::new(); bars];
        let (mut octave, mut volume, mut duty) = (DEFAULT_OCTAVE, DEFAULT_VOLUME, DEFAULT_DUTY);
        // Where the last sounding token is (to put a `&` after it).
        let mut last: Option<(usize, usize)> = None;
        if let Some(first) = self.notes.first() {
            out_first(&mut out, format!("v{}", first.volume));
            volume = first.volume;
            if pulse {
                out_first(&mut out, format!("@{}", first.duty));
                duty = first.duty;
            }
        }
        let notes = &self.notes;
        let mut k = 0; // first note that may reach into this bar
        for b in 0..bars {
            let (t0, t1) = (b as f64 * bb, (b + 1) as f64 * bb);
            let mut t = t0;
            let mut first_in_bar = true;
            while k < notes.len() && notes[k].end() <= t0 + EPS {
                k += 1;
            }
            let mut i = k;
            while let Some(n) = notes.get(i).filter(|n| n.start < t1 - EPS) {
                // Its head is in this bar (else it's carried over the bar line, tied).
                if n.start >= t0 - EPS {
                    if n.start > t + EPS {
                        rests(&mut out[b], n.start - t);
                        t = n.start;
                    }
                    if n.tie
                        && let Some((lb, li)) = last
                    {
                        out[lb][li].push('&');
                    }
                    if n.volume != volume {
                        out[b].push(format!("v{}", n.volume));
                        volume = n.volume;
                    }
                    if pulse && n.duty != duty {
                        out[b].push(format!("@{}", n.duty));
                        duty = n.duty;
                    }
                }
                let seg_end = n.end().min(t1);
                let (name, oct) = sound_text(&n.sound, octave);
                if !self.drums() && (oct != octave || first_in_bar) {
                    out[b].push(format!("o{oct}"));
                    octave = oct;
                }
                let mut tok = lengths(seg_end - t).iter().map(|l| format!("{name}{l}")).collect::<Vec<_>>().join("&");
                if seg_end < n.end() - EPS {
                    // On over the bar line.
                    tok.push('&');
                }
                out[b].push(tok);
                last = Some((b, out[b].len() - 1));
                first_in_bar = false;
                t = seg_end;
                i += 1;
            }
            if t < t1 - EPS {
                rests(&mut out[b], t1 - t);
            }
            out[b].push("|".into());
        }
        let mut s = String::new();
        for (l, chunk) in out.chunks(4).enumerate() {
            let line: Vec<String> = chunk.iter().map(|b| b.join(" ")).collect();
            let line = line.join(" ");
            let (a, z) = (l * 4 + 1, l * 4 + chunk.len());
            let label = if a == z { format!("; {a}") } else { format!("; {a}-{z}") };
            s += &format!("{line:<72} {label}\n");
        }
        s.trim_end().to_string()
    }
}

fn out_first(out: &mut [Vec<String>], tok: String) {
    if let Some(b) = out.first_mut() {
        b.push(tok);
    }
}

/// Note names by pitch class: sharps on F# C# G#, flats on Eb Bb (`c+ e- f+ g+ b-`).
const NAMES: [&str; 12] = ["c", "c+", "d", "e-", "e", "f", "f+", "g", "g+", "a", "b-", "b"];

fn octave_of(n: u8) -> i32 {
    n as i32 / 12 - 1
}

/// The token for a sound (without its length) and the octave to be in for it.
fn sound_text(s: &Sound, octave: i32) -> (String, i32) {
    match s {
        Sound::Note(n) => (NAMES[(*n % 12) as usize].to_string(), octave_of(*n)),
        Sound::Arp(a) => {
            let notes = a.notes();
            let base = octave_of(notes[0]);
            let mut cur = base;
            let mut t = String::from("{");
            for (i, &n) in notes.iter().enumerate() {
                let o = octave_of(n);
                if i > 0 {
                    t.push(' ');
                }
                while cur < o {
                    t += "> ";
                    cur += 1;
                }
                while cur > o {
                    t += "< ";
                    cur -= 1;
                }
                t += NAMES[(n % 12) as usize];
            }
            t.push('}');
            (t, base)
        }
        Sound::Drum(d) => (drum_letter(*d).to_string(), octave),
    }
}

pub fn drum_letter(d: Drum) -> &'static str {
    match d {
        Drum::Kick => "k",
        Drum::Snare => "s",
        Drum::ClosedHat => "h",
        Drum::OpenHat => "H",
        Drum::Crash => "x",
    }
}

fn rests(out: &mut Vec<String>, d: f64) {
    for l in lengths(d) {
        out.push(format!("r{l}"));
    }
}

/// Every MML length token with its beats, longest first: whole..64th with up to two dots, the
/// triplet family with up to one, then everything else in 1..=96.
fn length_tokens() -> &'static [(f64, String)] {
    static T: std::sync::OnceLock<Vec<(f64, String)>> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let mut nice = Vec::new();
        let dotted = |n: u32, dots: u32| {
            let mut b = 4.0 / n as f64;
            let mut add = b;
            for _ in 0..dots {
                add /= 2.0;
                b += add;
            }
            (b, format!("{n}{}", ".".repeat(dots as usize)))
        };
        for n in [1, 2, 4, 8, 16, 32, 64] {
            for d in 0..=2 {
                nice.push(dotted(n, d));
            }
        }
        for n in [3, 6, 12, 24, 48, 96] {
            for d in 0..=1 {
                nice.push(dotted(n, d));
            }
        }
        nice.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut rest: Vec<_> = (1..=96).map(|n| dotted(n, 0)).filter(|(b, _)| !nice.iter().any(|x| (x.0 - b).abs() < EPS)).collect();
        rest.sort_by(|a, b| b.0.total_cmp(&a.0));
        nice.extend(rest);
        nice
    })
}

/// `d` beats as length tokens to tie: one token if any is exact, else the longest that fit.
pub fn lengths(d: f64) -> Vec<String> {
    let all = length_tokens();
    if let Some((_, t)) = all.iter().find(|(b, _)| (b - d).abs() < EPS) {
        return vec![t.clone()];
    }
    let mut out = Vec::new();
    let mut rem = d;
    while rem > EPS && out.len() < 64 {
        if let Some((_, t)) = all.iter().find(|(b, _)| (b - rem).abs() < EPS) {
            out.push(t.clone());
            break;
        }
        match all.iter().find(|(b, _)| *b <= rem + EPS) {
            Some((b, t)) => {
                out.push(t.clone());
                rem -= b;
            }
            None => break,
        }
    }
    out
}

/// A section's place in the text: the header's line, and the body's lines `[from, to)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub name: String,
    pub header: usize,
    pub body: (usize, usize),
}

/// The sections of a song file (0-based lines), found the way [`SongFile::parse`] does.
pub fn sections(text: &str) -> Vec<Section> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<Section> = Vec::new();
    for (i, raw) in lines.iter().enumerate() {
        let line = raw.split_once(';').map_or(*raw, |(a, _)| a).trim();
        let word = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')).map(str::trim);
        if let Some(name) = word.filter(|w| w.len() >= 3 && w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')) {
            if let Some(prev) = out.last_mut() {
                prev.body.1 = i;
            }
            out.push(Section { name: name.to_string(), header: i, body: (i + 1, lines.len()) });
        }
    }
    out
}

/// Replace the body of section `name` with `body` (appending the section if it's missing).
pub fn splice_section(text: &str, name: &str, body: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let secs = sections(text);
    let Some(sec) = secs.iter().find(|s| s.name == name) else {
        let mut t = text.trim_end().to_string();
        t += &format!("\n\n[{name}]\n{}\n", body.trim_end());
        return t;
    };
    let last = sec.body.1 == lines.len();
    let mut out: Vec<String> = lines[..sec.body.0].iter().map(|s| s.to_string()).collect();
    out.extend(body.trim_end().lines().map(str::to_string));
    if !last {
        out.push(String::new());
    }
    out.extend(lines[sec.body.1..].iter().map(|s| s.to_string()));
    let mut t = out.join("\n");
    t.push('\n');
    t
}

/// A song being edited.
#[derive(Debug, Clone)]
pub struct SongDoc {
    /// File stem (`music/<stem>.song`).
    pub stem: String,
    pub text: String,
    /// The text as last loaded or saved.
    pub saved: String,
    /// The parse of `text`.
    pub parsed: Result<SongFile, SongError>,
    /// The last text that parsed, parsed: what the grids show and the engine plays.
    pub good: Option<SongFile>,
    /// Bumped on every change of `good`.
    pub revision: u64,
}

impl SongDoc {
    pub fn new(stem: &str, text: &str) -> SongDoc {
        let parsed = SongFile::parse(text);
        SongDoc { stem: stem.to_string(), text: text.to_string(), saved: text.to_string(), good: parsed.clone().ok(), parsed, revision: 0 }
    }

    pub fn dirty(&self) -> bool {
        self.text != self.saved
    }

    pub fn path(&self) -> String {
        format!("music/{}.song", self.stem)
    }

    /// New text (typed): parsed at once; the grids follow if it parses.
    pub fn set_text(&mut self, text: String) {
        if text == self.text {
            return;
        }
        self.text = text;
        self.reparse();
    }

    fn reparse(&mut self) {
        self.parsed = SongFile::parse(&self.text);
        if let Ok(s) = &self.parsed
            && self.good.as_ref() != Some(s)
        {
            self.good = Some(s.clone());
            self.revision += 1;
        }
    }

    pub fn error(&self) -> Option<&SongError> {
        self.parsed.as_ref().err()
    }

    /// Channel `ch` of the last good parse.
    pub fn part(&self, ch: usize) -> Option<Part> {
        self.good.as_ref().map(|s| Part::of(s, ch))
    }

    /// Edit channel `ch` as notes; the channel is rewritten by the formatter and the text
    /// re-parsed. Refused (text unchanged) if the text doesn't parse now, or the result
    /// wouldn't.
    pub fn edit(&mut self, ch: usize, f: impl FnOnce(&mut Part)) -> Result<(), String> {
        if let Err(e) = &self.parsed {
            return Err(format!("fix the text first: {e}"));
        }
        let mut part = self.part(ch).ok_or("nothing parsed yet")?;
        let before = part.clone();
        f(&mut part);
        if part == before {
            return Ok(());
        }
        self.write_part(&part)
    }

    /// Write `part` into its section.
    pub fn write_part(&mut self, part: &Part) -> Result<(), String> {
        let text = splice_section(&self.text, CHANNELS[part.ch].0, &part.to_mml());
        if let Err(e) = SongFile::parse(&text) {
            return Err(format!("that edit doesn't parse: {e}"));
        }
        self.text = text;
        self.reparse();
        Ok(())
    }

    /// Rewrite channel `ch` in the formatter's form (no change to what it plays).
    pub fn canonicalize(&mut self, ch: usize) -> Result<(), String> {
        let p = self.part(ch).ok_or("nothing parsed")?;
        if p.notes.is_empty() && self.good.as_ref().is_some_and(|s| s.sources[ch].trim().is_empty()) {
            return Ok(());
        }
        self.write_part(&p)
    }

    /// Check with the real parser, then write `music/<stem>.song` under `root`.
    pub fn save(&mut self, root: &std::path::Path) -> Result<std::path::PathBuf, String> {
        SongFile::parse(&self.text).map_err(|e| format!("not saved: {e}"))?;
        let path = root.join(self.path());
        std::fs::write(&path, &self.text).map_err(|e| format!("{}: {e}", path.display()))?;
        self.saved = self.text.clone();
        Ok(path)
    }
}

/// Bars `from..to` of a song as a song of their own (the transport's loop selection: the
/// engine loops it sample-exactly). Notes cut at the edges.
pub fn excerpt(song: &SongFile, from: usize, to: usize) -> SongFile {
    let bb = song.bar_beats();
    let to = to.clamp(from + 1, song.bars().max(1));
    let (a, z) = (from as f64 * bb, to as f64 * bb);
    let mut s = song.clone();
    for (ch, t) in s.tracks.iter_mut().enumerate() {
        if song.tracks[ch].length <= 0.0 {
            continue;
        }
        let mut events = Vec::new();
        for e in &song.tracks[ch].events {
            if e.start + e.dur <= a + EPS || e.start >= z - EPS {
                continue;
            }
            let mut e = *e;
            if e.start < a {
                e.dur -= a - e.start;
                e.start = a;
                e.tie = false;
            }
            e.dur = e.dur.min(z - e.start);
            e.start -= a;
            events.push(e);
        }
        if let Some(e) = events.first_mut() {
            e.tie = false;
        }
        *t = Track { events, length: z - a };
    }
    s.looping = true;
    s.chart = song.chart.as_ref().map(|c| {
        let slots = c
            .slots
            .iter()
            .filter(|sl| sl.end() > a + EPS && sl.start < z - EPS)
            .map(|sl| {
                let start = sl.start.max(a);
                chart::Slot { start: start - a, dur: sl.end().min(z) - start, chord: sl.chord }
            })
            .collect();
        Chart { slots, bars: to - from, meter: c.meter }
    });
    s.chords = s.chart.as_ref().map_or(String::new(), Chart::to_text);
    s
}


#[cfg(test)]
mod tests {
    use super::*;
    use nat_han_adventures::audio::live::library;

    fn tiny() -> SongDoc {
        let text = "; tiny\n[song]\ntitle = Tiny\nbpm = 120\nkey = C\nloop = yes\n\n[chords]\n| C | G7 |\n\n[pulse1]\nv12 @2 o4 c4 d4 e4 f4 | g1 |\n\n[noise]\nk4 s4 k4 s4 | k4 s4 k8 k8 s4 |\n";
        SongDoc::new("tiny", text)
    }

    fn pitches(p: &Part) -> Vec<(f64, f64, u8)> {
        p.notes.iter().map(|n| (n.start, n.dur, n.sound.pitch())).collect()
    }

    /// Every song in `music/`: the formatter writes each channel so it plays exactly the same
    /// notes, and its output is a fixed point.
    #[test]
    fn every_song_round_trips_through_the_formatter() {
        for (stem, text) in library::FILES {
            let mut doc = SongDoc::new(stem, text);
            let before: Vec<Part> = (0..4).map(|ch| doc.part(ch).unwrap()).collect();
            for ch in 0..4 {
                doc.canonicalize(ch).unwrap_or_else(|e| panic!("{stem} ch{ch}: {e}"));
            }
            let canon = doc.text.clone();
            for (ch, b) in before.iter().enumerate() {
                assert_eq!(doc.part(ch).unwrap().notes, b.notes, "{stem} channel {ch}");
            }
            for ch in 0..4 {
                doc.canonicalize(ch).unwrap();
            }
            assert_eq!(doc.text, canon, "{stem}: the formatter's output is a fixed point");
            let song = doc.good.as_ref().unwrap();
            let orig = SongFile::parse(text).unwrap();
            assert_eq!((song.bars(), song.chart.clone()), (orig.bars(), orig.chart.clone()), "{stem}");
        }
    }

    /// The same edit made in the tracker, the piano roll and the text gives the same file.
    #[test]
    fn every_view_writes_the_same_text() {
        use crate::views::{piano, tracker};
        let e4 = 64;
        // Tracker: the cursor on row 4 (beat 1) of pulse 1, typing E with a 4-row length.
        let mut a = tiny();
        tracker::enter(&mut a, 0, 4, Sound::Note(e4), 4, 12).unwrap();
        // Piano roll: draw E4 at step 4, 4 steps long.
        let mut b = tiny();
        piano::draw(&mut b, 0, 4, Sound::Note(e4), 4, 12).unwrap();
        // Text: change the `d4` to `e4`, then let the formatter tidy the channel.
        let mut c = tiny();
        c.set_text(c.text.replace("c4 d4 e4", "c4 e4 e4"));
        assert!(c.error().is_none());
        c.canonicalize(0).unwrap();
        assert_eq!(a.text, b.text);
        assert_eq!(a.text, c.text);
        assert!(a.text.contains("o4 c4 e4 e4 f4 | o4 g1 |"), "{}", a.text);
        // The other sections are untouched.
        assert!(a.text.starts_with("; tiny\n[song]") && a.text.contains("[noise]\nk4 s4 k4 s4 | k4 s4 k8 k8 s4 |"));

        // Moving and resizing too: the piano roll's drag vs the text.
        let mut m = tiny();
        piano::drag(&mut m, 0, 2, 6, 3).unwrap(); // e4 (index 2, step 8) to step 6, up to g
        let mut t = tiny();
        t.set_text(t.text.replace("c4 d4 e4 f4", "c4 d8 g4 r8 f4"));
        t.canonicalize(0).unwrap();
        assert_eq!(m.text, t.text);
        let mut r = tiny();
        piano::resize(&mut r, 0, 0, 2).unwrap(); // c4 -> c8
        let mut t = tiny();
        t.set_text(t.text.replace("c4 d4", "c8 r8 d4"));
        t.canonicalize(0).unwrap();
        assert_eq!(r.text, t.text);
        // Deleting.
        let mut d = tiny();
        tracker::delete(&mut d, 0, 4).unwrap();
        let mut t = tiny();
        t.set_text(t.text.replace("c4 d4", "c4 r4"));
        t.canonicalize(0).unwrap();
        assert_eq!(d.text, t.text);
    }

    /// Grid edits keep every bar line where the meter wants it, whatever they do.
    #[test]
    fn edits_keep_bar_lines_valid() {
        let mut doc = SongDoc::new("sgb", library::text("sweet_georgia_brown").unwrap());
        let mut seed = 7u64;
        let mut rnd = |n: u64| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) % n
        };
        for _ in 0..60 {
            let ch = rnd(4) as usize;
            let part = doc.part(ch).unwrap();
            let steps = (part.length / STEP) as u64;
            let sound = if part.drums() { Sound::Drum(DRUMS[rnd(4) as usize]) } else { Sound::Note(40 + rnd(40) as u8) };
            let r = match rnd(4) {
                0 => doc.edit(ch, |p| {
                    p.insert(Note { start: rnd(steps) as f64 * STEP, dur: (1 + rnd(24)) as f64 * STEP, sound, volume: 10, duty: 1, tie: false });
                }),
                1 if !part.notes.is_empty() => {
                    let i = rnd(part.notes.len() as u64) as usize;
                    doc.edit(ch, |p| p.remove(i))
                }
                2 if !part.notes.is_empty() => {
                    let i = rnd(part.notes.len() as u64) as usize;
                    doc.edit(ch, |p| {
                        p.move_note(i, rnd(steps) as f64 * STEP, sound);
                    })
                }
                _ if !part.notes.is_empty() => {
                    let i = rnd(part.notes.len() as u64) as usize;
                    doc.edit(ch, |p| {
                        p.resize(i, (1 + rnd(40)) as f64 * STEP);
                    })
                }
                _ => Ok(()),
            };
            r.unwrap();
            let song = SongFile::parse(&doc.text).expect("still parses");
            assert_eq!(song.bars(), 32);
            assert_eq!(song.tracks[ch].length, 128.0);
            let p = doc.part(ch).unwrap();
            for w in p.notes.windows(2) {
                assert!(w[0].end() <= w[1].start + EPS, "no overlaps");
            }
        }
    }

    #[test]
    fn notes_across_bar_lines_are_tied() {
        let mut doc = tiny();
        doc.edit(0, |p| {
            p.insert(Note { start: 3.0, dur: 2.0, sound: Sound::Note(67), volume: 12, duty: 2, tie: false });
        })
        .unwrap();
        assert!(doc.text.contains("o4 c4 d4 e4 g4& | o4 g4 r2."), "{}", doc.text);
        let p = doc.part(0).unwrap();
        assert_eq!(pitches(&p), [(0.0, 1.0, 60), (1.0, 1.0, 62), (2.0, 1.0, 64), (3.0, 2.0, 67)]);
        // Slurs, chords, octaves, triplets, volumes and duties survive.
        let mut doc = tiny();
        doc.set_text(doc.text.replace("c4 d4 e4 f4 | g1 |", "c4& >d4 <{c e g}4 l12 c c c | v9 @1 c2 g2 |"));
        assert!(doc.error().is_none(), "{:?}", doc.error());
        let before = doc.part(0).unwrap();
        doc.canonicalize(0).unwrap();
        assert_eq!(doc.part(0).unwrap(), before, "{}", doc.text);
        assert!(doc.text.contains("o4 c4& o5 d4 o4 {c e g}4 c12 c12 c12 | v9 @1 o4 c2 g2 |"), "{}", doc.text);
    }

    #[test]
    fn bad_edits_and_bad_text_are_refused() {
        let mut doc = tiny();
        doc.set_text(doc.text.replace("g1 |", "g2 |"));
        let e = doc.error().expect("short bar").clone();
        assert_eq!(e.line, 12);
        // Grids keep the last good parse, and refuse to edit until the text is fixed.
        assert_eq!(doc.part(0).unwrap().notes.len(), 5);
        assert!(doc.edit(0, |p| p.remove(0)).is_err());
        let dir = std::env::temp_dir().join(format!("editor-save-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("music")).unwrap();
        assert!(doc.save(&dir).unwrap_err().contains("not saved"));
        doc.set_text(doc.text.replace("g2 |", "g1 |"));
        let path = doc.save(&dir).unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), doc.text);
        assert!(!doc.dirty());
    }

    #[test]
    fn length_tokens() {
        assert_eq!(lengths(4.0), ["1"]);
        assert_eq!(lengths(3.0), ["2."]);
        assert_eq!(lengths(1.0 / 3.0), ["12"]);
        assert_eq!(lengths(0.8), ["5"]);
        assert_eq!(lengths(2.5), ["2", "8"]);
        assert_eq!(lengths(4.0 - 0.25), ["2..", "16"]);
    }

    #[test]
    fn excerpts_loop_a_selection() {
        let song = library::load("sweet_georgia_brown").unwrap();
        let x = excerpt(&song, 4, 8);
        assert_eq!(x.bars(), 4);
        assert_eq!(x.chart.as_ref().unwrap().bars, 4);
        assert!(nat_han_adventures::audio::live::Engine::new(&x, 32_000).is_ok());
    }
}
