//! Turn the game's [`Song`]s (Rust data in `songs.rs`) into `.song` files that parse to exactly
//! the same events: the MML is kept as written, with a checked `|` added at every bar line it
//! can go on (and any `|` the old parser ignored that isn't on a bar line dropped), broken into
//! lines of about four bars, each with its bar numbers. `examples/convert_songs.rs` writes
//! `music/*.song`; `tests/live.rs` checks they stay in sync with `songs.rs` until the swap.

use std::collections::HashMap;

use crate::audio::Song;
use crate::audio::chart;
use crate::audio::mml::Channel;

use super::mml::{self as live_mml, Item, Options};
use super::song::{CHANNELS, chart_lines, join_chart};

/// Bars per line in the converted MML (lines only break at top-level bar lines).
const BARS_PER_LINE: usize = 4;

/// The `.song` text of a song.
pub fn to_song_file(song: &Song) -> Result<String, String> {
    let bar = 4.0;
    let mut s = format!("; {}\n; Converted from src/audio/songs.rs. Each line ends with its bar numbers.\n\n", song.title);
    s += "[song]\n";
    s += &format!("title = {}\n", song.title);
    s += &format!("bpm = {}\n", song.bpm);
    s += &format!("swing = {}\n", song.swing);
    s += &format!("key = {}\n", chart::pc_name(song.key));
    s += &format!("loop = {}\n", if song.looping { "yes" } else { "no" });
    s += "meter = 4/4\n";
    let chords = join_chart(song.chords);
    if !chords.is_empty() {
        s += "\n[chords]\n";
        s += &chart_lines(&chords, 8);
    }
    let srcs = [song.pulse1, song.pulse2, song.triangle, song.noise];
    for ((name, channel), src) in CHANNELS.iter().zip(srcs) {
        if src.trim().is_empty() {
            continue;
        }
        let body = with_bar_lines(src, *channel, bar).map_err(|e| format!("{}: {name}: {e}", song.title))?;
        s += &format!("\n[{name}]\n{body}");
    }
    Ok(s)
}

/// `src` with checked bar lines added, laid out a few bars a line.
pub fn with_bar_lines(src: &str, channel: Channel, bar: f64) -> Result<String, String> {
    let p = live_mml::parse_with(src, channel, Options { bar_beats: None }).map_err(|e| e.to_string())?;
    let mut times: HashMap<usize, Vec<f64>> = HashMap::new();
    for v in &p.visits {
        times.entry(v.pos).or_default().push(v.time);
    }
    let on_bar = |pos: usize| {
        times.get(&pos).is_some_and(|ts| ts.iter().all(|&t| t > 1e-9 && ((t / bar).round() * bar - t).abs() < 1e-9))
    };
    let first_time = |pos: usize| times.get(&pos).and_then(|t| t.first().copied()).unwrap_or(0.0);

    // Edits: insert a bar line before a byte offset, or drop the `|` at one. Top-level bar
    // lines carry their bar number (for line breaks).
    enum Edit {
        Insert { at: usize, bar_no: Option<usize>, close: bool },
        Keep { at: usize, bar_no: Option<usize> },
        Drop { at: usize },
    }
    let mut edits = Vec::new();
    let mut depth = 0usize;
    // A sound has played since the last bar line (else a new one would be redundant).
    let mut need = false;
    // Start of the run of commands right before the current item (a bar line goes before them).
    let mut run_start: Option<usize> = None;
    let bar_no = |depth: usize, t: f64| (depth == 0).then(|| (t / bar).round() as usize);
    for &(pos, item) in &p.items {
        match item {
            Item::Command => {
                run_start.get_or_insert(pos);
            }
            Item::Tie => run_start = None,
            Item::Sound | Item::Open => {
                let at = run_start.take().unwrap_or(pos);
                if need && on_bar(pos) {
                    edits.push(Edit::Insert { at, bar_no: bar_no(depth, first_time(pos)), close: false });
                }
                if item == Item::Open {
                    depth += 1;
                    need = false;
                } else {
                    need = true;
                }
            }
            Item::Close => {
                run_start = None;
                depth -= 1;
                if need && on_bar(pos) {
                    edits.push(Edit::Insert { at: pos, bar_no: None, close: true });
                    need = false;
                } else {
                    need = true;
                }
            }
            Item::Bar => {
                run_start = None;
                if on_bar(pos) {
                    edits.push(Edit::Keep { at: pos, bar_no: bar_no(depth, first_time(pos)) });
                    need = false;
                } else {
                    edits.push(Edit::Drop { at: pos });
                }
            }
        }
    }
    edits.sort_by_key(|e| match e {
        Edit::Insert { at, .. } | Edit::Keep { at, .. } | Edit::Drop { at } => *at,
    });

    // Rebuild: tokens separated by single spaces, a newline after every BARS_PER_LINE-th
    // top-level bar line, each line ending with its bar numbers.
    let mut out = String::new();
    let mut line = String::new();
    let mut line_first_bar = 1usize;
    let flush = |out: &mut String, line: &mut String, first: usize, last: usize| {
        let text = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if !text.is_empty() {
            let label = if last > first { format!("{first}-{last}") } else { format!("{first}") };
            out.push_str(&format!("{text:<72} ; {label}\n"));
        }
        line.clear();
    };
    let mut cursor = 0;
    for e in &edits {
        let (at, bar_no, skip, mark) = match *e {
            // Snug against a closing `]`: `[c d e f |]2`.
            Edit::Insert { at, bar_no, close } => (at, bar_no, 0, if close { " |" } else { " | " }),
            Edit::Keep { at, bar_no } => (at, bar_no, 1, " | "),
            Edit::Drop { at } => {
                line.push_str(&src[cursor..at]);
                cursor = at + 1;
                continue;
            }
        };
        line.push_str(&src[cursor..at]);
        line.push_str(mark);
        cursor = at + skip;
        if let Some(n) = bar_no
            && n + 1 - line_first_bar >= BARS_PER_LINE
        {
            flush(&mut out, &mut line, line_first_bar, n);
            line_first_bar = n + 1;
        }
    }
    line.push_str(&src[cursor..]);
    let total = (p.track.length / bar).round() as usize;
    // Close the last bar too.
    let len = p.track.length;
    if need && len > 0.0 && ((len / bar).round() * bar - len).abs() < 1e-9 {
        line.push_str(" |");
    }
    flush(&mut out, &mut line, line_first_bar, total.max(line_first_bar));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::mml;

    fn check(src: &str, ch: Channel) -> String {
        let out = with_bar_lines(src, ch, 4.0).unwrap();
        let strict = live_mml::parse(&out, ch, 4.0).unwrap_or_else(|e| panic!("{out}\n{e}"));
        assert_eq!(strict, mml::parse(src, ch).unwrap(), "{out}");
        out
    }

    #[test]
    fn bar_lines_go_wherever_they_fit() {
        let out = check("v12 @1 o5 c4 d4 e4 f4 o4 g1 [c4 d4 e4 f4]2 c2&c2 b2. c4& c1", Channel::Melodic);
        assert_eq!(
            out.lines().map(|l| l.split(';').next().unwrap().trim()).collect::<Vec<_>>(),
            ["v12 @1 o5 c4 d4 e4 f4 | o4 g1 | [c4 d4 e4 f4 |]2 c2&c2 |", "b2. c4& | c1 |"]
        );
        assert!(out.lines().next().unwrap().ends_with("; 1-5"), "{out}");
        // A stray `|` the old parser ignored is dropped; good ones are kept.
        let out = check("c4 | d4 e4 f4 | g1", Channel::Melodic);
        assert!(out.starts_with("c4 d4 e4 f4 | g1 |"), "{out}");
        check("[k8 h8 s8 h8 k8 h8 s8 h8 ]6 [k4 s4 [h8 h8]2]3 r4", Channel::Drums);
    }
}
