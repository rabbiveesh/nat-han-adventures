//! MML parser: turns one channel's MML string (dialect documented in [`super`]) into a flat,
//! time-ordered list of [`Event`]s with start times and durations in beats (quarter notes).
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    /// MIDI note number (60 = middle C = `o4 c`).
    Note(u8),
    Rest,
    Drum(Drum),
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

/// Parse one channel. An empty string is an empty track.
pub fn parse(src: &str, channel: Channel) -> Result<Track, MmlError> {
    let mut p = Parser { src, bytes: src.as_bytes(), i: 0, channel };
    let nodes = p.block(false)?;
    let mut w = Walker {
        src,
        state: State { octave: 4, length: 1.0, volume: 12, duty: 2 },
        time: 0.0,
        events: Vec::new(),
        pending_tie: None,
    };
    w.walk(&nodes)?;
    if let Some(pos) = w.pending_tie {
        return Err(error(src, pos, "`&` must be followed by a note"));
    }
    Ok(Track { events: w.events, length: w.time })
}

// --- syntax tree ---

#[derive(Debug, Clone)]
enum Node {
    /// Note: semitone offset from C (may be -1 or 12 with accidentals), length in beats or None.
    Note { pos: usize, semitone: i32, len: Option<f64> },
    Rest { pos: usize, len: Option<f64> },
    Drum { pos: usize, drum: Drum, len: Option<f64> },
    Octave(i32),
    OctaveUp { pos: usize },
    OctaveDown { pos: usize },
    Length(f64),
    Volume(u8),
    Duty(u8),
    Tie { pos: usize },
    Repeat { body: Vec<Node>, times: u32 },
}

struct Parser<'a> {
    src: &'a str,
    bytes: &'a [u8],
    i: usize,
    channel: Channel,
}

impl Parser<'_> {
    fn skip_ws(&mut self) {
        while let Some(&b) = self.bytes.get(self.i) {
            if b.is_ascii_whitespace() || b == b'|' {
                self.i += 1;
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

    /// Optional unsigned integer.
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

    /// Optional note length (`4`, `8.`, `2..`): returns beats.
    fn length(&mut self) -> Result<Option<f64>, MmlError> {
        let Some((pos, n)) = self.number() else {
            // A bare dot (`c.`) would dot the default length: not supported, be explicit.
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
            return Err(self.err(
                pos,
                format!("`{cmd}{n}` out of range ({}..={})", range.start(), range.end()),
            ));
        }
        Ok(n)
    }

    /// Parse until end of input (top level) or a `]` (inside a repeat, which consumes it).
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
            let node = match c {
                'c' | 'd' | 'e' | 'f' | 'g' | 'a' | 'b' if self.channel == Channel::Melodic => {
                    let mut semitone = match c {
                        'c' => 0,
                        'd' => 2,
                        'e' => 4,
                        'f' => 5,
                        'g' => 7,
                        'a' => 9,
                        _ => 11,
                    };
                    match self.peek() {
                        Some(b'+' | b'#') => {
                            self.i += 1;
                            semitone += 1;
                        }
                        Some(b'-') => {
                            self.i += 1;
                            semitone -= 1;
                        }
                        _ => {}
                    }
                    Node::Note { pos, semitone, len: self.length()? }
                }
                'k' | 's' | 'h' | 'H' if self.channel == Channel::Drums => {
                    let drum = match c {
                        'k' => Drum::Kick,
                        's' => Drum::Snare,
                        'h' => Drum::ClosedHat,
                        _ => Drum::OpenHat,
                    };
                    Node::Drum { pos, drum, len: self.length()? }
                }
                'c' | 'd' | 'e' | 'f' | 'g' | 'a' | 'b' => {
                    return Err(self.err(pos, format!("note `{c}` on the noise channel: use drums `k s h H` or `r`")));
                }
                'k' | 's' | 'h' | 'H' => {
                    return Err(self.err(pos, format!("drum `{c}` on a melodic channel")));
                }
                'r' => Node::Rest { pos, len: self.length()? },
                'o' => Node::Octave(self.ranged(pos, 'o', 0..=8)? as i32),
                '>' => Node::OctaveUp { pos },
                '<' => Node::OctaveDown { pos },
                'l' => match self.length()? {
                    Some(l) => Node::Length(l),
                    None => return Err(self.err(pos, "`l` needs a length, e.g. `l8`")),
                },
                'v' => Node::Volume(self.ranged(pos, 'v', 0..=15)? as u8),
                '@' => Node::Duty(self.ranged(pos, '@', 0..=3)? as u8),
                '&' => Node::Tie { pos },
                '[' => {
                    let body = self.block(true)?;
                    let times = match self.number() {
                        Some((p, 0)) => return Err(self.err(p, "repeat count must be at least 1")),
                        Some((_, n)) if n > 256 => return Err(self.err(pos, "repeat count too large")),
                        Some((_, n)) => n,
                        None => 2,
                    };
                    Node::Repeat { body, times }
                }
                ']' if nested => return Ok(nodes),
                ']' => return Err(self.err(pos, "`]` without a matching `[`")),
                't' => return Err(self.err(pos, "`t` (tempo) isn't supported: tempo is Song::bpm")),
                _ => return Err(self.err(pos, format!("unexpected `{c}`"))),
            };
            nodes.push(node);
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
    state: State,
    time: f64,
    events: Vec<Event>,
    /// Position of a `&` waiting for its following note.
    pending_tie: Option<usize>,
}

impl Walker<'_> {
    fn walk(&mut self, nodes: &[Node]) -> Result<(), MmlError> {
        for node in nodes {
            match *node {
                Node::Note { pos, semitone, len } => {
                    let midi = 12 * (self.state.octave + 1) + semitone;
                    if !(0..=127).contains(&midi) {
                        return Err(error(self.src, pos, format!("note out of range (MIDI {midi})")));
                    }
                    self.push(EventKind::Note(midi as u8), len, pos)?;
                }
                Node::Rest { pos, len } => self.push(EventKind::Rest, len, pos)?,
                Node::Drum { pos, drum, len } => self.push(EventKind::Drum(drum), len, pos)?,
                Node::Octave(value) => self.state.octave = value,
                Node::OctaveUp { pos } => {
                    if self.state.octave >= 8 {
                        return Err(error(self.src, pos, "`>` above octave 8"));
                    }
                    self.state.octave += 1;
                }
                Node::OctaveDown { pos } => {
                    if self.state.octave <= 0 {
                        return Err(error(self.src, pos, "`<` below octave 0"));
                    }
                    self.state.octave -= 1;
                }
                Node::Length(l) => self.state.length = l,
                Node::Volume(v) => self.state.volume = v,
                Node::Duty(d) => self.state.duty = d,
                Node::Tie { pos } => {
                    let tieable = self.events.last().is_some_and(|e| e.start + e.dur >= self.time - 1e-9);
                    if !tieable || self.pending_tie.is_some() {
                        return Err(error(self.src, pos, "`&` must come right after a note"));
                    }
                    self.pending_tie = Some(pos);
                }
                Node::Repeat { ref body, times } => {
                    for _ in 0..times {
                        self.walk(body)?;
                    }
                }
            }
        }
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
                // Same sound continues: one longer event.
                (a, b) if a == b && prev.volume == self.state.volume && prev.duty == self.state.duty => {
                    prev.dur += dur;
                    return Ok(());
                }
                (EventKind::Note(_), EventKind::Note(_)) => {}
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
        assert_eq!(e[0], Event { start: 0.0, dur: 1.0, kind: EventKind::Note(60), volume: 12, duty: 2, tie: false });
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
        let e = parse("c4 d4 x", Channel::Melodic).unwrap_err();
        assert_eq!((e.line, e.col), (1, 7));
        assert!(e.to_string().contains("unexpected `x`"), "{e}");
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
