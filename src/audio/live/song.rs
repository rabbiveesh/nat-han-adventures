//! `.song` files: one song, INI-ish (the grammar is [`super::syntax::CHEAT_SHEET`]).
//!
//! ```text
//! ; comments start with `;`, anywhere
//! [song]
//! title = Shave and a Haircut (Charles Hale, 1899)
//! bpm = 140
//! swing = 0.25
//! key = C
//! loop = no
//! meter = 4/4
//!
//! [chords]
//! | C % F C/E | G7 % C % |
//!
//! [pulse1]
//! v13 @1 o5 c4 o4 g8 g8 a4 g4 | o4 r4 b4 o5 c2 |
//! ...
//! ```
//!
//! Every section is optional except `[song]` with `title` and `bpm`; missing channels are
//! silent. The MML is the game's dialect plus chords, comments and checked bar lines (see
//! [`crate::audio::mml`]). Every non-empty track must be whole bars long, and the chart (if any)
//! exactly as long as the song.

use std::fmt;

use crate::audio::chart::{self, Chart};
use crate::audio::mml::{self, Channel, Track};

use super::syntax::{SECTIONS, SONG_KEYS};

/// A time signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Meter {
    /// Beats per bar.
    pub beats: u8,
    /// The beat's note value (4 = quarter).
    pub unit: u8,
}

impl Default for Meter {
    fn default() -> Self {
        Meter { beats: 4, unit: 4 }
    }
}

impl Meter {
    /// Bar length in quarter notes (the MML's beats).
    pub fn bar_beats(self) -> f64 {
        self.beats as f64 * 4.0 / self.unit as f64
    }
}

impl fmt::Display for Meter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.beats, self.unit)
    }
}

/// The four channels, in file order.
pub const CHANNELS: [(&str, Channel); 4] =
    [("pulse1", Channel::Melodic), ("pulse2", Channel::Melodic), ("triangle", Channel::Melodic), ("noise", Channel::Drums)];

/// A parsed song file.
#[derive(Debug, Clone, PartialEq)]
pub struct SongFile {
    pub title: String,
    pub bpm: f32,
    pub swing: f32,
    /// Tonic pitch class, 0 = C.
    pub key: u8,
    pub looping: bool,
    pub meter: Meter,
    /// The chart as one line (`| ` separated, comments stripped); "" if none.
    pub chords: String,
    pub chart: Option<Chart>,
    /// MML source of each channel (as written, comments included).
    pub sources: [String; 4],
    /// Parsed (unswung) tracks.
    pub tracks: [Track; 4],
}

/// An error in a song file, with its 1-based line (and column, where known).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongError {
    pub line: usize,
    pub col: usize,
    pub msg: String,
    /// The offending line (with a caret), when known.
    pub context: String,
}

impl fmt::Display for SongError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}", self.line)?;
        if self.col > 0 {
            write!(f, " col {}", self.col)?;
        }
        write!(f, ": {}", self.msg)?;
        if !self.context.is_empty() {
            write!(f, "\n{}", self.context)?;
        }
        Ok(())
    }
}

impl std::error::Error for SongError {}

fn err(line: usize, msg: impl Into<String>) -> SongError {
    SongError { line, col: 0, msg: msg.into(), context: String::new() }
}

/// Strip a `;` comment.
fn uncomment(line: &str) -> &str {
    line.split_once(';').map_or(line, |(a, _)| a)
}

/// Parse a pitch-class name (`C`, `Bb`, `F#`) or number 0..=11.
pub fn parse_key(s: &str) -> Option<u8> {
    if let Ok(n) = s.parse::<u8>() {
        return (n < 12).then_some(n);
    }
    chart::parse_chord(s).ok().filter(|c| c.quality == chart::Quality::Major && c.bass.is_none()).map(|c| c.root)
}

fn parse_bool(s: &str) -> Option<bool> {
    match s.to_ascii_lowercase().as_str() {
        "yes" | "true" | "on" | "1" => Some(true),
        "no" | "false" | "off" | "0" => Some(false),
        _ => None,
    }
}

fn parse_meter(s: &str) -> Option<Meter> {
    let (b, u) = s.split_once('/')?;
    let beats: u8 = b.trim().parse().ok()?;
    let unit: u8 = u.trim().parse().ok()?;
    ((1..=16).contains(&beats) && matches!(unit, 2 | 4 | 8 | 16)).then_some(Meter { beats, unit })
}

/// The chart text of a `[chords]` body: bars may spread over lines, with or without leading
/// and trailing `|` on each line.
pub fn join_chart(body: &str) -> String {
    let bars: Vec<&str> = body
        .lines()
        .flat_map(|l| uncomment(l).split('|'))
        .map(str::trim)
        .filter(|b| !b.is_empty())
        .collect();
    if bars.is_empty() { String::new() } else { format!("| {} |", bars.join(" | ")) }
}

impl SongFile {
    /// Bar length in beats.
    pub fn bar_beats(&self) -> f64 {
        self.meter.bar_beats()
    }

    /// Song length in beats (the longest track).
    pub fn beats(&self) -> f64 {
        self.tracks.iter().map(|t| t.length).fold(0.0, f64::max)
    }

    /// Whole bars per loop.
    pub fn bars(&self) -> usize {
        (self.beats() / self.bar_beats()).round() as usize
    }

    pub fn parse(text: &str) -> Result<SongFile, SongError> {
        // Split into sections: (name, header line, body start line, body).
        let mut sections: Vec<(String, usize, usize, String)> = Vec::new();
        for (i, raw) in text.lines().enumerate() {
            let line_no = i + 1;
            let line = uncomment(raw).trim();
            // A header is a bare word in brackets (`[c e]2` or `[c e |]` are MML repeats).
            let word = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')).map(str::trim);
            if let Some(name) = word.filter(|w| w.len() >= 3 && w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')) {
                let name = name.to_string();
                if !SECTIONS.contains(&name.as_str()) {
                    return Err(err(line_no, format!("unknown section `[{name}]` (expected one of {})", SECTIONS.map(|s| format!("[{s}]")).join(" "))));
                }
                if sections.iter().any(|s| s.0 == name) {
                    return Err(err(line_no, format!("`[{name}]` appears twice")));
                }
                sections.push((name, line_no, line_no + 1, String::new()));
                continue;
            }
            match sections.last_mut() {
                Some(s) => {
                    s.3.push_str(raw);
                    s.3.push('\n');
                }
                None if line.is_empty() => {}
                None => return Err(err(line_no, "text before the first section (start with `[song]`)")),
            }
        }
        let get = |name: &str| sections.iter().find(|s| s.0 == name);
        let Some((_, song_line, _, header)) = get("song") else {
            return Err(err(1, "missing `[song]` section"));
        };

        let (mut title, mut bpm, mut swing, mut key, mut looping, mut meter) = (None, None, 0.0f32, 0u8, true, Meter::default());
        for (k, raw) in header.lines().enumerate() {
            let line_no = song_line + 1 + k;
            let line = uncomment(raw).trim();
            if line.is_empty() {
                continue;
            }
            let Some((name, value)) = line.split_once('=') else {
                return Err(err(line_no, format!("expected `name = value`, found `{line}`")));
            };
            let (name, value) = (name.trim(), value.trim());
            let bad = |what: &str| err(line_no, format!("`{name} = {value}`: {what}"));
            match name {
                "title" => title = Some(value.to_string()),
                "bpm" => bpm = Some(value.parse::<f32>().ok().filter(|b| b.is_finite() && *b > 0.0).ok_or_else(|| bad("bpm must be a positive number"))?),
                "swing" => swing = value.parse::<f32>().ok().filter(|s| (0.0..=0.9).contains(s)).ok_or_else(|| bad("swing must be 0..0.9"))?,
                "key" => key = parse_key(value).ok_or_else(|| bad("key is a note name like `F`, `Bb`, `F#` or 0..11"))?,
                "loop" => looping = parse_bool(value).ok_or_else(|| bad("loop is `yes` or `no`"))?,
                "meter" => meter = parse_meter(value).ok_or_else(|| bad("meter is like `4/4` or `3/4`"))?,
                _ => return Err(err(line_no, format!("unknown key `{name}` (expected one of {})", SONG_KEYS.join(", ")))),
            }
        }
        let title = title.ok_or_else(|| err(*song_line, "`[song]` needs a `title`"))?;
        let bpm = bpm.ok_or_else(|| err(*song_line, "`[song]` needs a `bpm`"))?;
        let bar_beats = meter.bar_beats();

        let mut sources: [String; 4] = Default::default();
        let mut tracks: [Track; 4] = Default::default();
        for (ch, (name, channel)) in CHANNELS.iter().enumerate() {
            let Some((_, header_line, body_line, body)) = get(name) else { continue };
            let t = mml::parse_checked(body, *channel, bar_beats).map_err(|e| SongError {
                line: body_line + e.line - 1,
                col: e.col,
                msg: format!("[{name}]: {}", e.msg),
                context: e.context.clone(),
            })?;
            if t.length > 0.0 {
                let bars = t.length / bar_beats;
                if (bars - bars.round()).abs() > 1e-6 {
                    return Err(err(*header_line, format!("[{name}] is {} beats long: not whole bars of {meter}", t.length)));
                }
            }
            sources[ch] = body.trim_end().to_string();
            tracks[ch] = t;
        }
        let beats = tracks.iter().map(|t| t.length).fold(0.0, f64::max);
        if beats <= 0.0 {
            return Err(err(*song_line, "the song has no notes"));
        }

        let (chords, chart) = match get("chords") {
            None => (String::new(), None),
            Some((_, line, _, body)) => {
                let text = join_chart(body);
                if text.is_empty() {
                    (text, None)
                } else {
                    if meter != Meter::default() {
                        return Err(err(*line, "chord charts are 4/4 only (for now)"));
                    }
                    let c = chart::parse(&text).map_err(|e| err(*line, e.to_string()))?;
                    if (c.beats() - beats).abs() > 1e-6 {
                        return Err(err(*line, format!("the chart has {} bars but the song is {} bars", c.bars, beats / bar_beats)));
                    }
                    (text, Some(c))
                }
            }
        };
        Ok(SongFile { title, bpm, swing, key, looping, meter, chords, chart, sources, tracks })
    }

    /// A 4/4 song straight from MML (pulse 1, pulse 2, triangle, noise) and a chart (`""` for
    /// none), for tests and demos: bar lines aren't checked and tracks needn't be whole bars,
    /// but the chart must cover the song exactly. Errors name the song and channel.
    #[allow(clippy::too_many_arguments)]
    pub fn from_mml(title: &str, bpm: f32, swing: f32, looping: bool, key: u8, chords: &str, parts: [&str; 4]) -> Result<SongFile, String> {
        let mut tracks: [Track; 4] = Default::default();
        for (ch, ((name, channel), src)) in CHANNELS.iter().zip(parts).enumerate() {
            tracks[ch] = mml::parse(src, *channel).map_err(|e| format!("song \"{title}\", {name}: {e}"))?;
        }
        let beats = tracks.iter().map(|t| t.length).fold(0.0, f64::max);
        if beats <= 0.0 {
            return Err(format!("song \"{title}\" is empty"));
        }
        if !bpm.is_finite() || bpm <= 0.0 {
            return Err(format!("song \"{title}\": bpm must be positive"));
        }
        let chart = if chords.trim().is_empty() {
            None
        } else {
            let c = chart::parse(chords).map_err(|e| format!("song \"{title}\": {e}"))?;
            if (c.beats() - beats).abs() > 1e-6 {
                return Err(format!("song \"{title}\": the chord chart has {} bars ({} beats) but the song is {beats} beats", c.bars, c.beats()));
            }
            Some(c)
        };
        Ok(SongFile {
            title: title.to_string(),
            bpm,
            swing,
            key,
            looping,
            meter: Meter::default(),
            chords: if chart.is_some() { join_chart(chords) } else { String::new() },
            chart,
            sources: parts.map(str::to_string),
            tracks,
        })
    }

    /// Write the file back out (canonical layout; the MML exactly as stored).
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        s += "[song]\n";
        s += &format!("title = {}\n", self.title);
        s += &format!("bpm = {}\n", self.bpm);
        s += &format!("swing = {}\n", self.swing);
        s += &format!("key = {}\n", chart::pc_name(self.key));
        s += &format!("loop = {}\n", if self.looping { "yes" } else { "no" });
        s += &format!("meter = {}\n", self.meter);
        if !self.chords.is_empty() {
            s += "\n[chords]\n";
            s += &chart_lines(&self.chords, 8);
        }
        for ((name, _), src) in CHANNELS.iter().zip(&self.sources) {
            if !src.trim().is_empty() {
                s += &format!("\n[{name}]\n{}\n", src.trim_end());
            }
        }
        s
    }
}

/// A chart as lines of `per_line` bars, each ending with its bar numbers.
pub fn chart_lines(chart: &str, per_line: usize) -> String {
    let bars: Vec<&str> = chart.split('|').map(str::trim).filter(|b| !b.is_empty()).collect();
    let mut s = String::new();
    for (k, chunk) in bars.chunks(per_line).enumerate() {
        let first = k * per_line + 1;
        let line = format!("| {} |", chunk.join(" | "));
        s += &format!("{line:<60} ; {first}-{}\n", first + chunk.len() - 1);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINI: &str = "; a test\n[song]\ntitle = Mini ; it's tiny\nbpm = 120\nkey = Bb\nloop = no\n\n[chords]\n| C | G7 |\n\n[pulse1]\no4 c1 | ; bar 1\nd1 |\n[noise]\nk1 | s1 |\n";

    #[test]
    fn parses_and_round_trips() {
        let s = SongFile::parse(MINI).unwrap();
        assert_eq!((s.title.as_str(), s.bpm, s.swing, s.key, s.looping, s.meter), ("Mini", 120.0, 0.0, 10, false, Meter::default()));
        assert_eq!(s.chords, "| C | G7 |");
        assert_eq!(s.tracks[0].events.len(), 2);
        assert!(s.tracks[1].events.is_empty());
        assert_eq!(s.bars(), 2);
        let again = SongFile::parse(&s.to_text()).unwrap();
        assert_eq!(again, s);
    }

    #[test]
    fn errors_point_at_the_file_line() {
        let e = SongFile::parse(&MINI.replace("d1 |", "d2 |")).unwrap_err();
        assert_eq!(e.line, 13, "{e}");
        assert!(e.msg.contains("[pulse1]") && e.msg.contains("short"), "{e}");
        let e = SongFile::parse(&MINI.replace("bpm = 120", "bpm = fast")).unwrap_err();
        assert_eq!(e.line, 4);
        assert!(SongFile::parse(&MINI.replace("[noise]", "[drums]")).unwrap_err().msg.contains("unknown section"));
        assert!(SongFile::parse(&MINI.replace("key = Bb", "tempo = 3")).unwrap_err().msg.contains("unknown key"));
        assert!(SongFile::parse(&MINI.replace("| C | G7 |", "| C |")).unwrap_err().msg.contains("1 bars"));
        assert!(SongFile::parse(&MINI.replace("k1 | s1 |", "k1 | s2")).unwrap_err().msg.contains("whole bars"));
        assert!(SongFile::parse("[pulse1]\nc1\n").unwrap_err().msg.contains("[song]"));
    }

    #[test]
    fn meters_and_keys() {
        assert_eq!(parse_meter("3/4").unwrap().bar_beats(), 3.0);
        assert_eq!(parse_meter("6/8").unwrap().bar_beats(), 3.0);
        assert!(parse_meter("4/5").is_none());
        assert_eq!((parse_key("F#"), parse_key("Eb"), parse_key("7"), parse_key("Cm"), parse_key("12")), (Some(6), Some(3), Some(7), None, None));
        let waltz = "[song]\ntitle = w\nbpm = 120\nmeter = 3/4\n[pulse1]\nc2. | d2 e4 |\n";
        assert_eq!(SongFile::parse(waltz).unwrap().bars(), 2);
        assert!(SongFile::parse(&format!("{waltz}[chords]\n| C | G |\n")).unwrap_err().msg.contains("4/4"));
    }
}
