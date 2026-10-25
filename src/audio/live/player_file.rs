//! The `.player` file: how a soloist builds lines ([`Profile`]), as text, one file per
//! player in `music/players/` (`include_str!`'d, like the songs). Sections of `name = value`
//! lines, `;` comments:
//!
//! ```text
//! [player]
//! name = Miles '56
//! lay_back = 1.6            ; on-beat lay-back against the band's average soloist (×)
//! swing = 1.36              ; long 8th over short
//! legato = 0.88             ; sounding length against the time to the next note
//! register = 72 64 82       ; MIDI: centre, soft floor, soft ceiling
//!
//! [line]
//! phrase = 6.1 3.9 10.2     ; beats: median, lower and upper quartile
//! rest = 3.3 2.0 4.9
//! cells = eighths 19.8, quarter 11.2, triplet 2.7, sixteenths 2.0, dotted 5.7, quarter_triplet 1.9, long 5.9
//! intervals = 9.4 28.0 33.5 13.1 8.4 3.8 0.7 1.8 1.3   ; 0 1 2 3 4 5 6 7 8-12 semitones
//! step_back = 0.24          ; and end_chord_tone, approach, from_below, enclosure,
//!                           ; pentatonic, anticipation, falls, strong_chord_tone (shares)
//!
//! [dynamics]
//! arch = -1.46 1.06 0.58 -0.83 -1.31   ; dB: first note, early, middle, late, last
//! per_octave = 4.5          ; and ghost, accent, long, scatter
//!
//! [motifs]
//! motif = 0.15              ; and sequence
//! moves = 0:7.7 -2:0.8      ; up to five transpositions, semitones:weight
//! ```
//!
//! Every key of `[player]`, `[line]` and `[dynamics]` is required but `strong_chord_tone`
//! and `falls` (0); `[motifs]` is optional (none). The field meanings are on [`Profile`].

use std::fmt;

use super::soloist::Profile;

/// A parsed `.player` file.
#[derive(Debug, Clone)]
pub struct PlayerFile {
    pub name: String,
    /// The on-beat lay-back, against the band's average soloist ([`super::pocket`]).
    pub lay_back: f32,
    pub profile: Profile,
}

/// An error in a `.player` file, with its 1-based line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerError {
    pub line: usize,
    pub msg: String,
}

impl fmt::Display for PlayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.msg)
    }
}

impl std::error::Error for PlayerError {}

const SECTIONS: [&str; 4] = ["player", "line", "dynamics", "motifs"];
const CELLS: [&str; 7] = ["eighths", "quarter", "triplet", "sixteenths", "dotted", "quarter_triplet", "long"];
const REQUIRED: [&str; 20] = [
    "name", "lay_back", "swing", "legato", "register", "phrase", "rest", "cells", "intervals", "step_back", "end_chord_tone", "approach", "from_below",
    "enclosure", "pentatonic", "anticipation", "arch", "per_octave", "ghost", "accent",
];

fn floats<const N: usize>(v: &str) -> Option<[f32; N]> {
    let mut out = [0.0; N];
    let mut it = v.split_whitespace();
    for x in &mut out {
        *x = it.next()?.parse().ok().filter(|f: &f32| f.is_finite())?;
    }
    it.next().is_none().then_some(out)
}

impl PlayerFile {
    pub fn parse(text: &str) -> Result<PlayerFile, PlayerError> {
        let err = |line: usize, msg: String| PlayerError { line, msg };
        let mut p = Profile::default();
        let (mut name, mut lay_back) = (None, 1.0);
        let mut seen: Vec<&str> = Vec::new();
        let mut section: Option<&str> = None;
        for (i, raw) in text.lines().enumerate() {
            let n = i + 1;
            let line = raw.split_once(';').map_or(raw, |(a, _)| a).trim();
            if line.is_empty() {
                continue;
            }
            if let Some(s) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                let s = SECTIONS.iter().find(|x| **x == s.trim()).ok_or_else(|| err(n, format!("unknown section `[{s}]` (expected [player] [line] [dynamics] [motifs])")))?;
                section = Some(s);
                continue;
            }
            let Some(sec) = section else { return Err(err(n, "text before the first section (start with `[player]`)".into())) };
            let Some((key, v)) = line.split_once('=') else { return Err(err(n, format!("expected `name = value`, found `{line}`"))) };
            let (key, v) = (key.trim(), v.trim());
            let bad = |what: &str| err(n, format!("`{key} = {v}`: {what}"));
            let one = || v.parse::<f32>().ok().filter(|f| f.is_finite()).ok_or_else(|| bad("expected a number"));
            let share = || one().and_then(|f| if (0.0..=1.0).contains(&f) { Ok(f) } else { Err(bad("a share is 0..1")) });
            let quartiles = || {
                floats::<3>(v)
                    .filter(|[m, a, b]| *a > 0.0 && a <= m && m <= b)
                    .map(|[m, a, b]| (m as f64, a as f64, b as f64))
                    .ok_or_else(|| bad("expected median, lower and upper quartile (0 < q1 <= median <= q3)"))
            };
            let key_sec = match key {
                "name" | "lay_back" | "swing" | "legato" | "register" => "player",
                "phrase" | "rest" | "cells" | "intervals" | "step_back" | "end_chord_tone" | "approach" | "from_below" | "enclosure" | "pentatonic"
                | "anticipation" | "falls" | "strong_chord_tone" => "line",
                "arch" | "per_octave" | "ghost" | "accent" | "long" | "scatter" => "dynamics",
                "motif" | "sequence" | "moves" => "motifs",
                _ => return Err(err(n, format!("unknown key `{key}` in [{sec}]"))),
            };
            if key_sec != sec {
                return Err(err(n, format!("`{key}` belongs in [{key_sec}], not [{sec}]")));
            }
            if seen.contains(&key) {
                return Err(err(n, format!("`{key}` appears twice")));
            }
            seen.push(key);
            match key {
                "name" => name = Some(v.to_string()),
                "lay_back" => lay_back = one()?,
                "swing" => p.swing = one().and_then(|s| if (1.0..=3.0).contains(&s) { Ok(s) } else { Err(bad("swing is 1.0..3.0")) })?,
                "legato" => p.legato = share()?,
                "register" => {
                    let [c, lo, hi] = floats::<3>(v).filter(|[c, lo, hi]| lo <= c && c <= hi).ok_or_else(|| bad("expected centre, floor, ceiling (MIDI)"))?;
                    p.register = (c as i32, lo as i32, hi as i32);
                }
                "phrase" => p.phrase = quartiles()?,
                "rest" => p.rest = quartiles()?,
                "cells" => {
                    for part in v.split(',') {
                        let mut w = part.split_whitespace();
                        let (Some(c), Some(x), None) = (w.next(), w.next(), w.next()) else { return Err(bad("expected `cell weight, ...`")) };
                        let k = CELLS.iter().position(|n| *n == c).ok_or_else(|| bad(&format!("unknown cell `{c}` (expected {})", CELLS.join(", "))))?;
                        p.cells[k] = x.parse().ok().filter(|f: &f32| *f >= 0.0).ok_or_else(|| bad("a weight is a number >= 0"))?;
                    }
                }
                "intervals" => p.intervals = floats::<9>(v).ok_or_else(|| bad("expected 9 weights (0 1 2 3 4 5 6 7 8-12 semitones)"))?,
                "step_back" => p.step_back = share()?,
                "end_chord_tone" => p.end_chord_tone = share()?,
                "approach" => p.approach = share()?,
                "from_below" => p.from_below = share()?,
                "enclosure" => p.enclosure = share()?,
                "pentatonic" => p.pentatonic = share()?,
                "anticipation" => p.anticipation = share()?,
                "falls" => p.falls = share()?,
                "strong_chord_tone" => p.strong_chord_tone = share()?,
                "arch" => p.arch = floats::<5>(v).ok_or_else(|| bad("expected 5 dB values (first, early, middle, late, last)"))?,
                "per_octave" => p.per_octave = one()?,
                "ghost" => p.ghost = share()?,
                "accent" => p.accent = one()?,
                "long" => p.long = one()?,
                "scatter" => p.scatter = one().and_then(|s| if s >= 0.0 { Ok(s) } else { Err(bad("scatter is >= 0")) })?,
                "motif" => p.motif = share()?,
                "sequence" => p.sequence = share()?,
                _ => {
                    // moves
                    let mut k = 0;
                    for m in v.split_whitespace() {
                        let parsed = m.split_once(':').and_then(|(a, b)| Some((a.parse::<i8>().ok()?, b.parse::<f32>().ok().filter(|w| *w >= 0.0)?)));
                        let mv = parsed.ok_or_else(|| bad("a move is `semitones:weight`"))?;
                        if k == p.moves.len() {
                            return Err(bad("at most five moves"));
                        }
                        p.moves[k] = mv;
                        k += 1;
                    }
                }
            }
        }
        if let Some(missing) = REQUIRED.iter().chain(&["long", "scatter"]).find(|k| !seen.contains(k)) {
            return Err(err(text.lines().count().max(1), format!("missing `{missing}`")));
        }
        Ok(PlayerFile { name: name.unwrap_or_default(), lay_back, profile: p })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_name_the_line() {
        let e = PlayerFile::parse("[player]\nname = X\nswing = 7\n").unwrap_err();
        assert_eq!(e.line, 3, "{e}");
        let e = PlayerFile::parse("[line]\nname = X\n").unwrap_err();
        assert!(e.msg.contains("belongs in [player]"), "{e}");
        let e = PlayerFile::parse("[player]\nname = X\n").unwrap_err();
        assert!(e.msg.contains("missing"), "{e}");
    }
}
