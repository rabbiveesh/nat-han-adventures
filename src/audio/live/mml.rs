//! The `.song` MML dialect: the game's dialect ([`crate::audio::mml`]) plus
//! - `{c e g}8`: a chord played as a fast arpeggio (an [`EventKind::Arp`]), up to [`Arp::MAX`]
//!   notes, lowest first as written; `<` / `>` inside the braces shift the octave for the rest of
//!   the chord only;
//! - `;` comments to the end of the line;
//! - checked bar lines: a `|` must fall exactly on a bar line of the meter (counted from the
//!   start of the track, repeats expanded), like a LilyPond bar check. A bar that is too long or
//!   too short is caught at the next `|`.
//!
//! It produces the same [`Track`] / [`Event`] types, so everything downstream is shared.
//!
//! Consolidation note: this is a copy of `audio::mml`'s parser (private there) with the
//! extensions above; when the live engine replaces the old one, `audio::mml` can become this.

use crate::audio::mml::{Arp, Channel, Drum, Event, EventKind, MmlError, Track};

/// How strictly to read bar lines.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    /// Beats (quarter notes) per bar; `None` ignores bar lines (the game's dialect).
    pub bar_beats: Option<f64>,
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
    /// `o l v @ < >`: changes state, takes no time.
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

/// Parse one channel with checked bar lines (`bar_beats` beats per bar).
pub fn parse(src: &str, channel: Channel, bar_beats: f64) -> Result<Track, MmlError> {
    parse_with(src, channel, Options { bar_beats: Some(bar_beats) }).map(|p| p.track)
}

/// Parse one channel, keeping the walk's visits.
pub fn parse_with(src: &str, channel: Channel, opts: Options) -> Result<Parsed, MmlError> {
    let mut p = Parser { src, bytes: src.as_bytes(), i: 0, channel, items: Vec::new() };
    let nodes = p.block(false)?;
    let mut w = Walker {
        src,
        opts,
        state: State { octave: 4, length: 1.0, volume: 12, duty: 2 },
        time: 0.0,
        last_bar: 0.0,
        events: Vec::new(),
        pending_tie: None,
        visits: Vec::new(),
    };
    w.walk(&nodes)?;
    if let Some(pos) = w.pending_tie {
        return Err(error(src, pos, "`&` must be followed by a note"));
    }
    Ok(Parsed { track: Track { events: w.events, length: w.time }, items: p.items, visits: w.visits })
}

pub(crate) fn error(src: &str, pos: usize, msg: impl Into<String>) -> MmlError {
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
                'k' | 's' | 'h' | 'H' if !melodic => {
                    let drum = match c {
                        'k' => Drum::Kick,
                        's' => Drum::Snare,
                        'h' => Drum::ClosedHat,
                        _ => Drum::OpenHat,
                    };
                    (NodeKind::Drum { drum, len: self.length()? }, Item::Sound)
                }
                'c' | 'd' | 'e' | 'f' | 'g' | 'a' | 'b' => {
                    return Err(self.err(pos, format!("note `{c}` on the noise channel: use drums `k s h H` or `r`")));
                }
                '{' => return Err(self.err(pos, "chords `{ }` need a melodic channel")),
                '}' => return Err(self.err(pos, "`}` without a matching `{`")),
                'k' | 's' | 'h' | 'H' => return Err(self.err(pos, format!("drum `{c}` on a melodic channel"))),
                'r' => (NodeKind::Rest { len: self.length()? }, Item::Sound),
                'o' => (NodeKind::Octave(self.ranged(pos, 'o', 0..=8)? as i32), Item::Command),
                '>' => (NodeKind::OctaveUp, Item::Command),
                '<' => (NodeKind::OctaveDown, Item::Command),
                'l' => match self.length()? {
                    Some(l) => (NodeKind::Length(l), Item::Command),
                    None => return Err(self.err(pos, "`l` needs a length, e.g. `l8`")),
                },
                'v' => (NodeKind::Volume(self.ranged(pos, 'v', 0..=15)? as u8), Item::Command),
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
}

struct Walker<'a> {
    src: &'a str,
    opts: Options,
    state: State,
    time: f64,
    /// Time of the last checked bar line.
    last_bar: f64,
    events: Vec<Event>,
    pending_tie: Option<usize>,
    visits: Vec<Visit>,
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
                (a, b) if a == b && prev.volume == self.state.volume && prev.duty == self.state.duty => {
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
        self.events.push(Event { start, dur, kind, volume: self.state.volume, duty: self.state.duty, tie });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::mml;

    #[test]
    fn same_as_the_game_dialect_without_the_extensions() {
        let src = "v9 @1 l8 o4 [c d e- f+ g#8 a16 b4. r4 | > c2 c4&c16 c8.. <]3 b4& >c4 r4&r4";
        assert_eq!(parse_with(src, Channel::Melodic, Options { bar_beats: None }).unwrap().track, mml::parse(src, Channel::Melodic).unwrap());
        let d = "v9 [k8 h8 s8 h8 k4 s8 H8 | ]2";
        assert_eq!(parse(d, Channel::Drums, 4.0).unwrap(), mml::parse(d, Channel::Drums).unwrap());
    }

    #[test]
    fn chords_are_arpeggios() {
        let t = parse("o4 {c e g}2 {d f > c}4 {c} 4", Channel::Melodic, 4.0).unwrap();
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
        let t = parse("o4 {c > c} c", Channel::Melodic, 4.0).unwrap();
        assert_eq!(t.events[1].kind, EventKind::Note(60));
        assert!(parse("{c4 e}", Channel::Melodic, 4.0).unwrap_err().msg.contains("after the `}`"));
        assert!(parse("{c e", Channel::Melodic, 4.0).unwrap_err().msg.contains("missing `}`"));
        assert!(parse("{}", Channel::Melodic, 4.0).unwrap_err().msg.contains("empty"));
        assert!(parse("{c d e f g a b}", Channel::Melodic, 4.0).unwrap_err().msg.contains("at most"));
        assert!(parse("{k}", Channel::Drums, 4.0).unwrap_err().msg.contains("melodic"));
    }

    #[test]
    fn comments_run_to_the_end_of_the_line() {
        let t = parse("c4 ; d4 e4 |\n d4 ;\n e2 | ; trailing", Channel::Melodic, 4.0).unwrap();
        assert_eq!(t.events.len(), 3);
        assert_eq!(t.length, 4.0);
    }

    #[test]
    fn bar_lines_are_checked() {
        assert!(parse("c4 d4 e4 f4 | g1 |", Channel::Melodic, 4.0).is_ok());
        let e = parse("c4 d4 e4 f4 | g2. |", Channel::Melodic, 4.0).unwrap_err();
        assert!(e.msg.contains("bar 2 is 3 beats long") && e.msg.contains("1 beats short"), "{e}");
        assert_eq!(e.col, 19);
        let e = parse("c4 d4 e4 f4 g4 |", Channel::Melodic, 4.0).unwrap_err();
        assert!(e.msg.contains("1 beats into bar 2"), "{e}");
        // Inside repeats every pass is checked.
        assert!(parse("[c4 d4 e4 f4 |]3", Channel::Melodic, 4.0).is_ok());
        assert!(parse("[c4 d4 e4 |]2", Channel::Melodic, 4.0).is_err());
        // 3/4: three beats a bar; ties may cross bar lines.
        assert!(parse("c2 d4 | e2.& | e4 f2 |", Channel::Melodic, 3.0).is_ok());
        assert!(parse("c1 |", Channel::Melodic, 3.0).is_err());
        // The game's dialect ignores them.
        assert!(parse_with("c4 | d4", Channel::Melodic, Options { bar_beats: None }).is_ok());
    }
}
