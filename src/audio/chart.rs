//! Chord charts ([`super::Song::chords`]): parser and a little chord vocabulary.
//!
//! The grammar is documented on [`super::Filters`] ("Chord charts"). Times are in beats
//! (quarter notes) from the start of the song, like [`super::mml`].

use std::fmt;

/// Chord quality, exactly as spelled in a chart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Quality {
    /// `` major triad
    Major,
    /// `6`
    Six,
    /// `maj7`
    Maj7,
    /// `7`
    Dom7,
    /// `9`
    Dom9,
    /// `7b9`
    Dom7b9,
    /// `7#9`
    Dom7s9,
    /// `7#5`
    Dom7s5,
    /// `7sus4`
    Sus4,
    /// `m`
    Minor,
    /// `m6`
    Minor6,
    /// `m7`
    Minor7,
    /// `mMaj7`
    MinMaj7,
    /// `m7b5`
    HalfDim,
    /// `dim7`
    Dim7,
    /// `aug`
    Aug,
}

/// What a chord does, harmonically: the reharmonizers only care about this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Family {
    Major,
    Dominant,
    Minor,
    HalfDim,
    Dim,
    Aug,
}

impl Quality {
    /// Spellings, in the order they're tried (longest first where one is a prefix of another
    /// doesn't matter: the whole remainder must match).
    pub const ALL: [(Quality, &'static str); 16] = [
        (Quality::Major, ""),
        (Quality::Six, "6"),
        (Quality::Maj7, "maj7"),
        (Quality::Dom7, "7"),
        (Quality::Dom9, "9"),
        (Quality::Dom7b9, "7b9"),
        (Quality::Dom7s9, "7#9"),
        (Quality::Dom7s5, "7#5"),
        (Quality::Sus4, "7sus4"),
        (Quality::Minor, "m"),
        (Quality::Minor6, "m6"),
        (Quality::Minor7, "m7"),
        (Quality::MinMaj7, "mMaj7"),
        (Quality::HalfDim, "m7b5"),
        (Quality::Dim7, "dim7"),
        (Quality::Aug, "aug"),
    ];

    pub fn name(self) -> &'static str {
        Self::ALL.iter().find(|(q, _)| *q == self).map(|(_, s)| *s).unwrap_or("")
    }

    /// Chord tones as semitones above the root (may exceed an octave: 9ths).
    pub fn intervals(self) -> &'static [u8] {
        match self {
            Quality::Major => &[0, 4, 7],
            Quality::Six => &[0, 4, 7, 9],
            Quality::Maj7 => &[0, 4, 7, 11],
            Quality::Dom7 => &[0, 4, 7, 10],
            Quality::Dom9 => &[0, 4, 7, 10, 14],
            Quality::Dom7b9 => &[0, 4, 7, 10, 13],
            Quality::Dom7s9 => &[0, 4, 7, 10, 15],
            Quality::Dom7s5 => &[0, 4, 8, 10],
            Quality::Sus4 => &[0, 5, 7, 10],
            Quality::Minor => &[0, 3, 7],
            Quality::Minor6 => &[0, 3, 7, 9],
            Quality::Minor7 => &[0, 3, 7, 10],
            Quality::MinMaj7 => &[0, 3, 7, 11],
            Quality::HalfDim => &[0, 3, 6, 10],
            Quality::Dim7 => &[0, 3, 6, 9],
            Quality::Aug => &[0, 4, 8],
        }
    }

    pub fn family(self) -> Family {
        match self {
            Quality::Major | Quality::Six | Quality::Maj7 => Family::Major,
            Quality::Dom7 | Quality::Dom9 | Quality::Dom7b9 | Quality::Dom7s9 | Quality::Dom7s5 | Quality::Sus4 => {
                Family::Dominant
            }
            Quality::Minor | Quality::Minor6 | Quality::Minor7 | Quality::MinMaj7 => Family::Minor,
            Quality::HalfDim => Family::HalfDim,
            Quality::Dim7 => Family::Dim,
            Quality::Aug => Family::Aug,
        }
    }
}

/// One chord symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Chord {
    /// Root pitch class, 0 = C.
    pub root: u8,
    pub quality: Quality,
    /// Slash bass pitch class (`C7/E`).
    pub bass: Option<u8>,
}

const NAMES: [&str; 12] = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"];

/// Pitch-class name, flats preferred (`Eb`, not `D#`).
pub fn pc_name(pc: u8) -> &'static str {
    NAMES[(pc % 12) as usize]
}

impl Chord {
    pub const fn new(root: u8, quality: Quality) -> Self {
        Chord { root: root % 12, quality, bass: None }
    }

    pub fn family(&self) -> Family {
        self.quality.family()
    }

    /// Pitch classes of the chord tones (root first).
    pub fn pitch_classes(&self) -> impl Iterator<Item = u8> + '_ {
        self.quality.intervals().iter().map(move |i| (self.root + i) % 12)
    }

    /// The note the bass should land on: the slash bass if any, else the root.
    pub fn bass_pc(&self) -> u8 {
        self.bass.unwrap_or(self.root)
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", pc_name(self.root), self.quality.name())?;
        if let Some(b) = self.bass {
            write!(f, "/{}", pc_name(b))?;
        }
        Ok(())
    }
}

/// A chord with its place in time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Slot {
    pub start: f64,
    pub dur: f64,
    pub chord: Chord,
}

impl Slot {
    pub fn end(&self) -> f64 {
        self.start + self.dur
    }
}

/// A parsed chart: chords in time order, contiguous from beat 0 to `bars * 4`.
#[derive(Debug, Clone, PartialEq)]
pub struct Chart {
    pub slots: Vec<Slot>,
    pub bars: usize,
}

impl Chart {
    pub fn beats(&self) -> f64 {
        self.bars as f64 * 4.0
    }

    /// Consecutive identical chords merged into one longer slot.
    pub fn merged(&self) -> Vec<Slot> {
        let mut out: Vec<Slot> = Vec::with_capacity(self.slots.len());
        for s in &self.slots {
            match out.last_mut() {
                Some(last) if last.chord == s.chord && (last.end() - s.start).abs() < 1e-9 => last.dur += s.dur,
                _ => out.push(*s),
            }
        }
        out
    }

    /// The chord sounding at beat `t` (wrapping around the loop).
    pub fn at(&self, t: f64) -> Chord {
        let t = t.rem_euclid(self.beats());
        let i = self.slots.partition_point(|s| s.start <= t + 1e-9);
        self.slots[i.saturating_sub(1)].chord
    }

    /// Compact text form (one bar per `|`, chords split evenly), mainly for tests and logs.
    /// Bars whose chords don't fall on 1-, 2- or 4-beat splits are written with durations.
    pub fn to_text(&self) -> String {
        let mut bars = vec![Vec::new(); self.bars];
        for s in &self.slots {
            // A slot may span several bars: write it in each.
            let mut t = s.start;
            while t < s.end() - 1e-9 {
                let bar = (t / 4.0).floor() as usize;
                let bar_end = (bar as f64 + 1.0) * 4.0;
                let d = s.end().min(bar_end) - t;
                if let Some(b) = bars.get_mut(bar) {
                    b.push((s.chord.to_string(), d));
                }
                t += d;
            }
        }
        bars.iter()
            .map(|b| {
                b.iter()
                    .map(|(c, d)| if b.len() == 1 || (b.len() == 2 && *d == 2.0) || b.len() == 4 {
                        c.clone()
                    } else {
                        format!("{c}:{d}")
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect::<Vec<_>>()
            .join(" | ")
    }
}

/// A chart parse error. `bar` is 1-based.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChartError {
    pub bar: usize,
    pub token: String,
    pub msg: String,
}

impl fmt::Display for ChartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.token.is_empty() {
            write!(f, "chord chart, bar {}: {}", self.bar, self.msg)
        } else {
            write!(f, "chord chart, bar {}, `{}`: {}", self.bar, self.token, self.msg)
        }
    }
}

impl std::error::Error for ChartError {}

/// Parse a note name `C`, `F#`, `Bb` at the start of `s`; returns pitch class and bytes used.
fn note(s: &str) -> Option<(u8, usize)> {
    let mut chars = s.chars();
    let pc: i32 = match chars.next()? {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    match chars.next() {
        Some('#') => Some(((pc + 1).rem_euclid(12) as u8, 2)),
        Some('b') => Some(((pc - 1).rem_euclid(12) as u8, 2)),
        _ => Some((pc as u8, 1)),
    }
}

/// Parse one chord token like `Bb7`, `F#m7b5`, `C7/E`.
pub fn parse_chord(token: &str) -> Result<Chord, String> {
    let (root, used) = note(token).ok_or_else(|| "a chord starts with a root `A`-`G` (then `#`/`b`)".to_string())?;
    let rest = &token[used..];
    let (qual, bass) = match rest.split_once('/') {
        Some((q, b)) => {
            let (pc, n) = note(b).filter(|(_, n)| *n == b.len()).ok_or_else(|| format!("bad slash bass `/{b}`"))?;
            debug_assert_eq!(n, b.len());
            (q, Some(pc))
        }
        None => (rest, None),
    };
    let quality = Quality::ALL.iter().find(|(_, s)| *s == qual).map(|(q, _)| *q).ok_or_else(|| {
        let known: Vec<String> = Quality::ALL.iter().map(|(_, s)| format!("`{s}`")).collect();
        format!("unknown chord quality `{qual}` (expected one of {})", known.join(" "))
    })?;
    Ok(Chord { root, quality, bass })
}

/// Parse a chart. An empty (or all-whitespace) chart is an error: callers check
/// `chords.trim().is_empty()` first to mean "no chart".
pub fn parse(src: &str) -> Result<Chart, ChartError> {
    let trimmed = src.trim().trim_matches('|');
    if trimmed.trim().is_empty() {
        return Err(ChartError { bar: 1, token: String::new(), msg: "the chart is empty".into() });
    }
    let mut slots = Vec::new();
    let mut prev: Option<Chord> = None;
    let mut bars = 0;
    for (b, bar) in trimmed.split('|').enumerate() {
        let tokens: Vec<&str> = bar.split_whitespace().collect();
        let err = |token: &str, msg: String| ChartError { bar: b + 1, token: token.to_string(), msg };
        if !matches!(tokens.len(), 1 | 2 | 4) {
            return Err(err(
                bar.trim(),
                format!("a bar holds 1, 2 or 4 chords, found {} (use `%` to repeat a chord)", tokens.len()),
            ));
        }
        let dur = 4.0 / tokens.len() as f64;
        for (k, tok) in tokens.iter().enumerate() {
            let chord = if *tok == "%" {
                prev.ok_or_else(|| err(tok, "`%` repeats the previous chord, but there isn't one yet".into()))?
            } else {
                parse_chord(tok).map_err(|m| err(tok, m))?
            };
            prev = Some(chord);
            slots.push(Slot { start: b as f64 * 4.0 + k as f64 * dur, dur, chord });
        }
        bars += 1;
    }
    Ok(Chart { slots, bars })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_roots_qualities_and_splits() {
        let c = parse("| D7 | % | Bb7 F#m7b5 | C7/E Ebmaj7 Am6 Gdim7 |").unwrap();
        assert_eq!(c.bars, 4);
        let s: Vec<String> = c.slots.iter().map(|s| format!("{}@{}+{}", s.chord, s.start, s.dur)).collect();
        assert_eq!(
            s,
            [
                "D7@0+4", "D7@4+4", "Bb7@8+2", "Gbm7b5@10+2", "C7/E@12+1", "Ebmaj7@13+1", "Am6@14+1", "Gdim7@15+1"
            ]
        );
        assert_eq!(c.merged().len(), 7);
        assert_eq!(c.merged()[0].dur, 8.0);
        assert_eq!(c.at(9.0).to_string(), "Bb7");
        assert_eq!(c.at(16.0 + 1.0).to_string(), "D7");
        for (q, s) in Quality::ALL {
            assert_eq!(parse_chord(&format!("C{s}")).unwrap().quality, q);
        }
        assert_eq!(c.to_text(), "D7 | D7 | Bb7 Gbm7b5 | C7/E Ebmaj7 Am6 Gdim7");
    }

    #[test]
    fn errors_say_where_and_why() {
        let e = parse("| C | D7 E7 F7 |").unwrap_err();
        assert_eq!(e.bar, 2);
        assert!(e.to_string().contains("1, 2 or 4"), "{e}");
        let e = parse("C | Dx7").unwrap_err();
        assert_eq!((e.bar, e.token.as_str()), (2, "Dx7"));
        assert!(e.msg.contains("unknown chord quality `x7`"), "{e}");
        assert!(parse("% | C").unwrap_err().msg.contains("previous chord"));
        assert!(parse("H7").unwrap_err().msg.contains("root"));
        assert!(parse("C7/X").unwrap_err().msg.contains("slash bass"));
        assert!(parse("C | | D").unwrap_err().msg.contains("found 0"));
        assert!(parse("  ").unwrap_err().msg.contains("empty"));
    }
}
