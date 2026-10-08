//! MML: turns one channel's MML string into a flat, time-ordered list of [`Event`]s with
//! start times and durations in beats (quarter notes). The dialect is documented in
//! [`super`] (and as data in [`super::live::syntax::CHEAT_SHEET`]); on top of the classic
//! commands it has
//! - `{c e g}8`: a chord played as a fast arpeggio (an [`EventKind::Arp`]), up to [`Arp::MAX`]
//!   notes, lowest first as written; `<` / `>` inside the braces shift the octave for the rest of
//!   the chord only;
//! - `;` comments to the end of the line;
//! - checked bar lines ([`parse_checked`], what `.song` files use): a `|` must fall exactly on a
//!   bar line of the meter (counted from the start of the track, repeats expanded), like a
//!   LilyPond bar check. A bar that is too long or too short is caught at the next `|`.
//!   [`parse`] ignores them.
//!
//! Parsing happens in two steps: the text is parsed into a tree (repeats nest), then the tree
//! is walked with the running state (octave, length, volume, duty), which gives repeats the
//! "literal expansion" semantics: `o4 [c >]2 c` plays o4 c, o5 c, o6 c.

use std::fmt;

/// Which kind of channel the MML is for: pulse/triangle use note letters, noise uses drum letters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Melodic,
    Drums,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Drum {
    Kick,
    Snare,
    ClosedHat,
    OpenHat,
    /// A crash cymbal (`x`).
    Crash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    /// MIDI note number (60 = middle C = `o4 c`).
    Note(u8),
    Rest,
    Drum(Drum),
    /// A chord played as a fast chiptune arpeggio (the voice cycles through the notes every
    /// [`super::synth::ARP_STEP`] seconds): `{c e g}` in MML, and what the accompaniment
    /// generator writes.
    Arp(Arp),
}

/// Up to [`Arp::MAX`] MIDI notes, played in order and cycled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Arp {
    notes: [u8; Arp::MAX],
    len: u8,
}

impl Arp {
    pub const MAX: usize = 6;

    /// The first [`Arp::MAX`] of `notes` (at least one is required).
    pub fn new(notes: &[u8]) -> Self {
        assert!(!notes.is_empty(), "an arpeggio needs at least one note");
        let len = notes.len().min(Self::MAX);
        let mut a = [0; Self::MAX];
        a[..len].copy_from_slice(&notes[..len]);
        Arp { notes: a, len: len as u8 }
    }

    pub fn notes(&self) -> &[u8] {
        &self.notes[..self.len as usize]
    }
}

/// One note, rest or drum hit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Event {
    /// Start, in beats (quarter notes) from the start of the track. Unswung.
    pub start: f64,
    /// Duration in beats.
    pub dur: f64,
    pub kind: EventKind,
    /// 0..=15.
    pub volume: u8,
    /// 0..=3 (12.5%, 25%, 50%, 75%).
    pub duty: u8,
    /// Slurred from the previous note (`c4&d4`): no re-attack. (A tie to the *same* pitch is
    /// merged into one longer note instead, so it never shows up here.)
    pub tie: bool,
    /// Instrument (`@i name`): 0 is the channel's built-in, `k` the song's `k`-th
    /// ([`super::live::instrument`]).
    pub inst: u8,
}

/// A parsed channel.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Track {
    pub events: Vec<Event>,
    /// Total length in beats, including trailing rests.
    pub length: f64,
}

/// A parse error at byte offset `pos` of the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmlError {
    pub pos: usize,
    pub line: usize,
    pub col: usize,
    pub msg: String,
    /// The offending line with a caret under the error.
    pub context: String,
}

impl fmt::Display for MmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MML error at line {} col {}: {}\n{}", self.line, self.col, self.msg, self.context)
    }
}

impl std::error::Error for MmlError {}

/// How strictly to read bar lines, and the instruments `@i` may name.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options<'a> {
    /// Beats (quarter notes) per bar; `None` ignores bar lines (the game's dialect).
    pub bar_beats: Option<f64>,
    /// The song's instruments, in order (`@i` names them; instrument `k + 1` is the `k`-th):
    /// (name, is a kit). `default` (0) is always there.
    pub instruments: &'a [(&'a str, bool)],
}

impl Options<'_> {
    /// Bar lines checked every `bar_beats` (`None`: ignored), no song instruments.
    pub fn bars(bar_beats: Option<f64>) -> Options<'static> {
        Options { bar_beats, instruments: &[] }
    }
}

/// One visit of the walker to a source position (repeats visit their body several times).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Visit {
    /// Byte offset of the node (or of the `]` closing a repeat).
    pub pos: usize,
    /// Track time (beats) when the walker got there.
    pub time: f64,
}

/// What sits at a source position, in source order (for tools like the song converter).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    /// A note, rest, drum hit or chord: moves time on.
    Sound,
    /// `o l v @ @i < >`: changes state, takes no time.
    Command,
    /// `&`.
    Tie,
    /// `[`.
    Open,
    /// `]n` (its position is the `]`).
    Close,
    /// `|`.
    Bar,
}

/// A parse with the walker's visits, for tools.
#[derive(Debug, Clone)]
pub struct Parsed {
    pub track: Track,
    /// Every node in source order.
    pub items: Vec<(usize, Item)>,
    /// Every visit, in walk order.
    pub visits: Vec<Visit>,
}

/// Parse one channel; `|` bar lines are ignored. An empty string is an empty track.
pub fn parse(src: &str, channel: Channel) -> Result<Track, MmlError> {
    parse_with(src, channel, Options::bars(None)).map(|p| p.track)
}

/// Parse one channel with checked bar lines (`bar_beats` beats per bar): what `.song` files use.
pub fn parse_checked(src: &str, channel: Channel, bar_beats: f64) -> Result<Track, MmlError> {
    parse_with(src, channel, Options::bars(Some(bar_beats))).map(|p| p.track)
}

/// Parse one channel, keeping the walk's visits.
pub fn parse_with(src: &str, channel: Channel, opts: Options) -> Result<Parsed, MmlError> {
    let mut p = Parser { src, bytes: src.as_bytes(), i: 0, channel, items: Vec::new() };
    let nodes = p.block(false)?;
    let mut w = Walker {
        src,
        opts,
        state: State { octave: 4, length: 1.0, volume: 12, duty: 2, inst: 0 },
        time: 0.0,
        last_bar: 0.0,
        events: Vec::new(),
        pending_tie: None,
        visits: Vec::new(),
        walking_drums: channel == Channel::Drums,
    };
    w.walk(&nodes)?;
    if let Some(pos) = w.pending_tie {
        return Err(error(src, pos, "`&` must be followed by a note"));
    }
    Ok(Parsed { track: Track { events: w.events, length: w.time }, items: p.items, visits: w.visits })
}

fn error(src: &str, pos: usize, msg: impl Into<String>) -> MmlError {
    let pos = pos.min(src.len());
    let line_start = src[..pos].rfind('\n').map_or(0, |i| i + 1);
    let line_end = src[pos..].find('\n').map_or(src.len(), |i| pos + i);
    let line = src[..pos].matches('\n').count() + 1;
    let col = src[line_start..pos].chars().count() + 1;
    let text = &src[line_start..line_end];
    let context = format!("  {text}\n  {}^", " ".repeat(col - 1));
    MmlError { pos, line, col, msg: msg.into(), context }
}

// --- syntax tree ---

#[derive(Debug, Clone)]
struct Node {
    pos: usize,
    kind: NodeKind,
}

#[derive(Debug, Clone)]
enum NodeKind {
    /// Semitone offset from C (may be -1 or 12 with accidentals), length in beats or None.
    Note { semitone: i32, len: Option<f64> },
    /// Semitone offsets from C of the current octave (`<`/`>` inside already applied).
    Chord { semitones: Vec<i32>, len: Option<f64> },
    Rest { len: Option<f64> },
    Drum { drum: Drum, len: Option<f64> },
    Octave(i32),
    OctaveUp,
    OctaveDown,
    Length(f64),
    Volume(u8),
    Duty(u8),
    /// `@i name`.
    Inst(String),
    Tie,
    Bar,
    Repeat { body: Vec<Node>, times: u32, close: usize },
}

struct Parser<'a> {
    src: &'a str,
    bytes: &'a [u8],
    i: usize,
    channel: Channel,
    items: Vec<(usize, Item)>,
}

fn note_semitone(c: u8) -> Option<i32> {
    Some(match c {
        b'c' => 0,
        b'd' => 2,
        b'e' => 4,
        b'f' => 5,
        b'g' => 7,
        b'a' => 9,
        b'b' => 11,
        _ => return None,
    })
}

impl Parser<'_> {
    /// Whitespace and `;` comments.
    fn skip_ws(&mut self) {
        while let Some(&b) = self.bytes.get(self.i) {
            if b.is_ascii_whitespace() {
                self.i += 1;
            } else if b == b';' {
                while self.bytes.get(self.i).is_some_and(|&b| b != b'\n') {
                    self.i += 1;
                }
            } else {
                break;
            }
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip_ws();
        self.bytes.get(self.i).copied()
    }

    fn err(&self, pos: usize, msg: impl Into<String>) -> MmlError {
        error(self.src, pos, msg)
    }

    fn number(&mut self) -> Option<(usize, u32)> {
        self.skip_ws();
        let start = self.i;
        while self.bytes.get(self.i).is_some_and(u8::is_ascii_digit) {
            self.i += 1;
        }
        if self.i == start {
            return None;
        }
        let n = self.src[start..self.i].parse().unwrap_or(u32::MAX);
        Some((start, n))
    }

    fn length(&mut self) -> Result<Option<f64>, MmlError> {
        let Some((pos, n)) = self.number() else {
            if self.peek() == Some(b'.') {
                return Err(self.err(self.i, "a dot needs an explicit length, e.g. `c4.`"));
            }
            return Ok(None);
        };
        if !(1..=96).contains(&n) {
            return Err(self.err(pos, format!("note length must be 1..=96, got {n}")));
        }
        let mut beats = 4.0 / n as f64;
        let mut add = beats;
        while self.peek() == Some(b'.') {
            self.i += 1;
            add /= 2.0;
            beats += add;
        }
        Ok(Some(beats))
    }

    fn ranged(&mut self, cmd_pos: usize, cmd: char, range: std::ops::RangeInclusive<u32>) -> Result<u32, MmlError> {
        let Some((pos, n)) = self.number() else {
            return Err(self.err(cmd_pos, format!("`{cmd}` needs a number")));
        };
        if !range.contains(&n) {
            return Err(self.err(pos, format!("`{cmd}{n}` out of range ({}..={})", range.start(), range.end())));
        }
        Ok(n)
    }

    /// An accidental after a note letter.
    fn accidental(&mut self) -> i32 {
        match self.peek() {
            Some(b'+' | b'#') => {
                self.i += 1;
                1
            }
            Some(b'-') => {
                self.i += 1;
                -1
            }
            _ => 0,
        }
    }

    /// The inside of `{ ... }` (the `{` is consumed).
    fn chord(&mut self, open: usize) -> Result<Vec<i32>, MmlError> {
        let mut semis = Vec::new();
        let mut shift = 0;
        loop {
            let Some(c) = self.peek() else {
                return Err(self.err(open, "missing `}` to close the chord"));
            };
            let pos = self.i;
            self.i += 1;
            match c {
                b'}' => break,
                b'>' => shift += 12,
                b'<' => shift -= 12,
                _ if note_semitone(c).is_some() => {
                    let s = note_semitone(c).unwrap() + self.accidental() + shift;
                    if self.peek().is_some_and(|b| b.is_ascii_digit() || b == b'.') {
                        return Err(self.err(self.i, "the length goes after the `}`: `{c e g}8`"));
                    }
                    semis.push(s);
                }
                _ => {
                    let ch = self.src[pos..].chars().next().unwrap();
                    return Err(self.err(pos, format!("only notes and `<` `>` go inside `{{ }}`, found `{ch}`")));
                }
            }
        }
        if semis.is_empty() {
            return Err(self.err(open, "an empty chord `{}`"));
        }
        if semis.len() > Arp::MAX {
            return Err(self.err(open, format!("a chord holds at most {} notes", Arp::MAX)));
        }
        Ok(semis)
    }

    fn block(&mut self, nested: bool) -> Result<Vec<Node>, MmlError> {
        let mut nodes = Vec::new();
        loop {
            let Some(c) = self.peek() else {
                if nested {
                    return Err(self.err(self.src.len(), "missing `]` to close a repeat"));
                }
                return Ok(nodes);
            };
            let pos = self.i;
            if !c.is_ascii() {
                let ch = self.src[pos..].chars().next().unwrap();
                return Err(self.err(pos, format!("unexpected `{ch}`")));
            }
            self.i += 1;
            let c = c as char;
            let melodic = self.channel == Channel::Melodic;
            let (kind, item) = match c {
                'c' | 'd' | 'e' | 'f' | 'g' | 'a' | 'b' if melodic => {
                    let semitone = note_semitone(c as u8).unwrap() + self.accidental();
                    (NodeKind::Note { semitone, len: self.length()? }, Item::Sound)
                }
                '{' if melodic => {
                    let semitones = self.chord(pos)?;
                    (NodeKind::Chord { semitones, len: self.length()? }, Item::Sound)
                }
                'k' | 's' | 'h' | 'H' | 'x' if !melodic => {
                    let drum = match c {
                        'k' => Drum::Kick,
                        's' => Drum::Snare,
                        'h' => Drum::ClosedHat,
                        'x' => Drum::Crash,
                        _ => Drum::OpenHat,
                    };
                    (NodeKind::Drum { drum, len: self.length()? }, Item::Sound)
                }
                'c' | 'd' | 'e' | 'f' | 'g' | 'a' | 'b' => {
                    return Err(self.err(pos, format!("note `{c}` on the noise channel: use drums `k s h H x` or `r`")));
                }
                '{' => return Err(self.err(pos, "chords `{ }` need a melodic channel")),
                '}' => return Err(self.err(pos, "`}` without a matching `{`")),
                'k' | 's' | 'h' | 'H' | 'x' => return Err(self.err(pos, format!("drum `{c}` on a melodic channel"))),
                'r' => (NodeKind::Rest { len: self.length()? }, Item::Sound),
                'o' => (NodeKind::Octave(self.ranged(pos, 'o', 0..=8)? as i32), Item::Command),
                '>' => (NodeKind::OctaveUp, Item::Command),
                '<' => (NodeKind::OctaveDown, Item::Command),
                'l' => match self.length()? {
                    Some(l) => (NodeKind::Length(l), Item::Command),
                    None => return Err(self.err(pos, "`l` needs a length, e.g. `l8`")),
                },
                'v' => (NodeKind::Volume(self.ranged(pos, 'v', 0..=15)? as u8), Item::Command),
                '@' if self.bytes.get(self.i) == Some(&b'i') => {
                    self.i += 1;
                    self.skip_ws();
                    let from = self.i;
                    while self.bytes.get(self.i).is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_') {
                        self.i += 1;
                    }
                    if self.i == from {
                        return Err(self.err(pos, "`@i` needs an instrument name, e.g. `@i brass`"));
                    }
                    (NodeKind::Inst(self.src[from..self.i].to_string()), Item::Command)
                }
                '@' => (NodeKind::Duty(self.ranged(pos, '@', 0..=3)? as u8), Item::Command),
                '&' => (NodeKind::Tie, Item::Tie),
                '|' => (NodeKind::Bar, Item::Bar),
                '[' => {
                    self.items.push((pos, Item::Open));
                    let body = self.block(true)?;
                    let close = self.i - 1;
                    self.items.push((close, Item::Close));
                    let times = match self.number() {
                        Some((p, 0)) => return Err(self.err(p, "repeat count must be at least 1")),
                        Some((_, n)) if n > 256 => return Err(self.err(pos, "repeat count too large")),
                        Some((_, n)) => n,
                        None => 2,
                    };
                    nodes.push(Node { pos, kind: NodeKind::Repeat { body, times, close } });
                    continue;
                }
                ']' if nested => return Ok(nodes),
                ']' => return Err(self.err(pos, "`]` without a matching `[`")),
                't' => return Err(self.err(pos, "`t` (tempo) isn't supported: tempo is the song's bpm")),
                _ => return Err(self.err(pos, format!("unexpected `{c}`"))),
            };
            self.items.push((pos, item));
            nodes.push(Node { pos, kind });
        }
    }
}

// --- interpretation ---

#[derive(Debug, Clone, Copy)]
struct State {
    octave: i32,
    length: f64,
    volume: u8,
    duty: u8,
    inst: u8,
}

struct Walker<'a> {
    src: &'a str,
    opts: Options<'a>,
    state: State,
    time: f64,
    /// Time of the last checked bar line.
    last_bar: f64,
    events: Vec<Event>,
    pending_tie: Option<usize>,
    visits: Vec<Visit>,
    walking_drums: bool,
}

/// Bar-line tolerance, in beats.
const BAR_EPS: f64 = 1e-6;

impl Walker<'_> {
    fn walk(&mut self, nodes: &[Node]) -> Result<(), MmlError> {
        for node in nodes {
            let pos = node.pos;
            self.visits.push(Visit { pos, time: self.time });
            match node.kind {
                NodeKind::Note { semitone, len } => {
                    let midi = 12 * (self.state.octave + 1) + semitone;
                    if !(0..=127).contains(&midi) {
                        return Err(error(self.src, pos, format!("note out of range (MIDI {midi})")));
                    }
                    self.push(EventKind::Note(midi as u8), len, pos)?;
                }
                NodeKind::Chord { ref semitones, len } => {
                    let mut notes = [0u8; Arp::MAX];
                    for (n, s) in notes.iter_mut().zip(semitones) {
                        let midi = 12 * (self.state.octave + 1) + s;
                        if !(0..=127).contains(&midi) {
                            return Err(error(self.src, pos, format!("chord note out of range (MIDI {midi})")));
                        }
                        *n = midi as u8;
                    }
                    self.push(EventKind::Arp(Arp::new(&notes[..semitones.len()])), len, pos)?;
                }
                NodeKind::Rest { len } => self.push(EventKind::Rest, len, pos)?,
                NodeKind::Drum { drum, len } => self.push(EventKind::Drum(drum), len, pos)?,
                NodeKind::Octave(value) => self.state.octave = value,
                NodeKind::OctaveUp => {
                    if self.state.octave >= 8 {
                        return Err(error(self.src, pos, "`>` above octave 8"));
                    }
                    self.state.octave += 1;
                }
                NodeKind::OctaveDown => {
                    if self.state.octave <= 0 {
                        return Err(error(self.src, pos, "`<` below octave 0"));
                    }
                    self.state.octave -= 1;
                }
                NodeKind::Length(l) => self.state.length = l,
                NodeKind::Volume(v) => self.state.volume = v,
                NodeKind::Duty(d) => self.state.duty = d,
                NodeKind::Inst(ref name) => self.state.inst = self.instrument(name, pos)?,
                NodeKind::Tie => {
                    let tieable = self.events.last().is_some_and(|e| e.start + e.dur >= self.time - 1e-9);
                    if !tieable || self.pending_tie.is_some() {
                        return Err(error(self.src, pos, "`&` must come right after a note"));
                    }
                    self.pending_tie = Some(pos);
                }
                NodeKind::Bar => self.bar_check(pos)?,
                NodeKind::Repeat { ref body, times, close } => {
                    for _ in 0..times {
                        self.walk(body)?;
                        self.visits.push(Visit { pos: close, time: self.time });
                    }
                }
            }
        }
        Ok(())
    }

    /// The number of instrument `name` (`default` is 0), checked against the channel.
    fn instrument(&self, name: &str, pos: usize) -> Result<u8, MmlError> {
        if name == "default" {
            return Ok(0);
        }
        let insts = self.opts.instruments;
        let Some(k) = insts.iter().position(|(n, _)| *n == name) else {
            let known: Vec<&str> = std::iter::once("default").chain(insts.iter().map(|(n, _)| *n)).collect();
            return Err(error(self.src, pos, format!("unknown instrument `{name}` (the song has: {})", known.join(" "))));
        };
        let drums = self.walking_drums;
        match (insts[k].1, drums) {
            (true, false) => Err(error(self.src, pos, format!("`{name}` is a drum kit: kits go on the noise channel"))),
            (false, true) => Err(error(self.src, pos, format!("`{name}` is a tone instrument: the noise channel takes kits"))),
            _ => Ok(k as u8 + 1),
        }
    }

    fn bar_check(&mut self, pos: usize) -> Result<(), MmlError> {
        let Some(bb) = self.opts.bar_beats else { return Ok(()) };
        let t = self.time;
        let on_line = ((t / bb).round() * bb - t).abs() < BAR_EPS;
        if !on_line {
            let bar = (self.last_bar / bb).round() as usize + 1;
            let len = t - self.last_bar;
            let msg = if len < bb {
                format!("bar {bar} is {len} beats long, the meter wants {bb}: {} beats short", bb - len)
            } else {
                format!(
                    "bar line {} beats into bar {}: the meter wants {bb} beats a bar",
                    t - (t / bb).floor() * bb,
                    (t / bb).floor() as usize + 1
                )
            };
            return Err(error(self.src, pos, msg));
        }
        self.last_bar = t;
        Ok(())
    }

    fn push(&mut self, kind: EventKind, len: Option<f64>, pos: usize) -> Result<(), MmlError> {
        let dur = len.unwrap_or(self.state.length);
        let start = self.time;
        self.time += dur;
        let tie = self.pending_tie.take().is_some();
        if tie {
            let prev = self.events.last_mut().expect("checked when the `&` was read");
            match (prev.kind, kind) {
                (a, b) if a == b && prev.volume == self.state.volume && prev.duty == self.state.duty && prev.inst == self.state.inst => {
                    prev.dur += dur;
                    return Ok(());
                }
                (EventKind::Note(_) | EventKind::Arp(_), EventKind::Note(_) | EventKind::Arp(_)) => {}
                (EventKind::Rest, EventKind::Rest) | (EventKind::Drum(_), EventKind::Drum(_)) => {
                    prev.dur += dur;
                    return Ok(());
                }
                _ => return Err(error(self.src, pos, "`&` can only join two notes (or two rests)")),
            }
        }
        self.events.push(Event { start, dur, kind, volume: self.state.volume, duty: self.state.duty, tie, inst: self.state.inst });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chords_are_arpeggios() {
        let t = parse_checked("o4 {c e g}2 {d f > c}4 {c} 4", Channel::Melodic, 4.0).unwrap();
        let kinds: Vec<_> = t.events.iter().map(|e| (e.kind, e.dur)).collect();
        assert_eq!(
            kinds,
            [
                (EventKind::Arp(Arp::new(&[60, 64, 67])), 2.0),
                (EventKind::Arp(Arp::new(&[62, 65, 72])), 1.0),
                (EventKind::Arp(Arp::new(&[60])), 1.0),
            ]
        );
        // The shift inside the braces is local.
        let t = parse_checked("o4 {c > c} c", Channel::Melodic, 4.0).unwrap();
        assert_eq!(t.events[1].kind, EventKind::Note(60));
        assert!(parse_checked("{c4 e}", Channel::Melodic, 4.0).unwrap_err().msg.contains("after the `}`"));
        assert!(parse_checked("{c e", Channel::Melodic, 4.0).unwrap_err().msg.contains("missing `}`"));
        assert!(parse_checked("{}", Channel::Melodic, 4.0).unwrap_err().msg.contains("empty"));
        assert!(parse_checked("{c d e f g a b}", Channel::Melodic, 4.0).unwrap_err().msg.contains("at most"));
        assert!(parse_checked("{k}", Channel::Drums, 4.0).unwrap_err().msg.contains("melodic"));
    }

    #[test]
    fn comments_run_to_the_end_of_the_line() {
        let t = parse_checked("c4 ; d4 e4 |\n d4 ;\n e2 | ; trailing", Channel::Melodic, 4.0).unwrap();
        assert_eq!(t.events.len(), 3);
        assert_eq!(t.length, 4.0);
    }

    #[test]
    fn bar_lines_are_checked() {
        assert!(parse_checked("c4 d4 e4 f4 | g1 |", Channel::Melodic, 4.0).is_ok());
        let e = parse_checked("c4 d4 e4 f4 | g2. |", Channel::Melodic, 4.0).unwrap_err();
        assert!(e.msg.contains("bar 2 is 3 beats long") && e.msg.contains("1 beats short"), "{e}");
        assert_eq!(e.col, 19);
        let e = parse_checked("c4 d4 e4 f4 g4 |", Channel::Melodic, 4.0).unwrap_err();
        assert!(e.msg.contains("1 beats into bar 2"), "{e}");
        // Inside repeats every pass is checked.
        assert!(parse_checked("[c4 d4 e4 f4 |]3", Channel::Melodic, 4.0).is_ok());
        assert!(parse_checked("[c4 d4 e4 |]2", Channel::Melodic, 4.0).is_err());
        // 3/4: three beats a bar; ties may cross bar lines.
        assert!(parse_checked("c2 d4 | e2.& | e4 f2 |", Channel::Melodic, 3.0).is_ok());
        assert!(parse_checked("c1 |", Channel::Melodic, 3.0).is_err());
        // The game's dialect ignores them.
        assert!(parse_with("c4 | d4", Channel::Melodic, Options::bars(None)).is_ok());
    }
}

#[cfg(test)]
mod game_dialect_tests {
    use super::*;

    fn notes(src: &str) -> Vec<(f64, f64, u8)> {
        parse(src, Channel::Melodic)
            .unwrap()
            .events
            .iter()
            .filter_map(|e| match e.kind {
                EventKind::Note(n) => Some((e.start, e.dur, n)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn defaults_and_pitches() {
        let t = parse("c d e- f+ g#8 a16 b2", Channel::Melodic).unwrap();
        let e = &t.events;
        assert_eq!(e[0], Event { start: 0.0, dur: 1.0, kind: EventKind::Note(60), volume: 12, duty: 2, tie: false, inst: 0 });
        let pitches: Vec<_> = e.iter().map(|e| e.kind).collect();
        use EventKind::Note as N;
        assert_eq!(pitches, [N(60), N(62), N(63), N(66), N(68), N(69), N(71)]);
        assert_eq!(e[4].dur, 0.5);
        assert_eq!(e[5].dur, 0.25);
        assert_eq!(t.length, 4.0 + 0.5 + 0.25 + 2.0);
    }

    #[test]
    fn octaves_lengths_dots() {
        assert_eq!(notes("o2 c > c < < c"), [(0.0, 1.0, 36), (1.0, 1.0, 48), (2.0, 1.0, 24)]);
        assert_eq!(notes("l8 c c4. c"), [(0.0, 0.5, 60), (0.5, 1.5, 60), (2.0, 0.5, 60)]);
        assert_eq!(notes("c2.. c-"), [(0.0, 3.5, 60), (3.5, 1.0, 59)]);
        assert_eq!(notes("l12 c c c"), [(0.0, 1.0 / 3.0, 60), (1.0 / 3.0, 1.0 / 3.0, 60), (2.0 / 3.0, 1.0 / 3.0, 60)]);
    }

    #[test]
    fn ties_merge_same_pitch_and_slur_different() {
        assert_eq!(notes("c4&c16 d"), [(0.0, 1.25, 60), (1.25, 1.0, 62)]);
        let t = parse("b4& >c4 r4&r4", Channel::Melodic).unwrap();
        assert_eq!(t.events.len(), 3);
        assert!(!t.events[0].tie);
        assert!(t.events[1].tie);
        assert_eq!(t.events[1].kind, EventKind::Note(72));
        assert_eq!(t.events[2].dur, 2.0);
        assert_eq!(t.length, 4.0);
    }

    #[test]
    fn repeats_expand_and_nest() {
        let t = parse("[c [d e]3 ]2 f", Channel::Melodic).unwrap();
        let p: Vec<u8> = t
            .events
            .iter()
            .map(|e| match e.kind {
                EventKind::Note(n) => n,
                _ => 0,
            })
            .collect();
        assert_eq!(p, [60, 62, 64, 62, 64, 62, 64, 60, 62, 64, 62, 64, 62, 64, 65]);
        assert_eq!(t.length, 15.0);
        // State carries across iterations like a literal expansion; default count is 2.
        assert_eq!(notes("o4 [c >]"), [(0.0, 1.0, 60), (1.0, 1.0, 72)]);
    }

    #[test]
    fn volume_duty_drums() {
        let t = parse("v5 @0 c", Channel::Melodic).unwrap();
        assert_eq!((t.events[0].volume, t.events[0].duty), (5, 0));
        let d = parse("k8 h8 s8 H8 r4 | k", Channel::Drums).unwrap();
        let kinds: Vec<_> = d.events.iter().map(|e| e.kind).collect();
        use EventKind::{Drum as D, Rest};
        use self::Drum::*;
        assert_eq!(kinds, [D(Kick), D(ClosedHat), D(Snare), D(OpenHat), Rest, D(Kick)]);
        assert_eq!(d.length, 4.0);
        assert_eq!(parse("", Channel::Drums).unwrap(), Track::default());
    }

    #[test]
    fn errors_point_at_the_problem() {
        let e = parse("c4 d4 z", Channel::Melodic).unwrap_err();
        assert_eq!((e.line, e.col), (1, 7));
        assert!(e.to_string().contains("unexpected `z`"), "{e}");
        assert!(e.context.ends_with("      ^"), "{:?}", e.context);
        let e = parse("c\n  [d e", Channel::Melodic).unwrap_err();
        assert!(e.msg.contains("missing `]`"), "{e}");
        assert_eq!(e.line, 2);
        assert!(parse("c ]", Channel::Melodic).unwrap_err().msg.contains("without a matching"));
        assert!(parse("v16", Channel::Melodic).unwrap_err().msg.contains("out of range"));
        assert!(parse("c3", Channel::Melodic).is_ok());
        assert!(parse("c0", Channel::Melodic).unwrap_err().msg.contains("1..=96"));
        assert!(parse("k", Channel::Melodic).unwrap_err().msg.contains("drum"));
        assert!(parse("c", Channel::Drums).unwrap_err().msg.contains("noise"));
        assert!(parse("c&", Channel::Melodic).unwrap_err().msg.contains("followed by a note"));
        assert!(parse("&c", Channel::Melodic).unwrap_err().msg.contains("right after a note"));
        assert!(parse("o8 b >c", Channel::Melodic).unwrap_err().msg.contains("above octave 8"));
        assert!(parse("t120 c", Channel::Melodic).unwrap_err().msg.contains("tempo"));
        assert!(parse("l", Channel::Melodic).unwrap_err().msg.contains("needs a length"));
    }
}
