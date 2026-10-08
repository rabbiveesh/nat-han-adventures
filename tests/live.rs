//! The live music engine (`audio::live`): song files, the engine against the offline renderer,
//! scheduling, inputs, clocks and performance.

use nat_han_adventures::audio::{
    Music, chart,
    live::{convert, library, mml as live_mml, song::SongFile, syntax},
    mml::{self, Channel},
    songs,
};

// --- song files ---------------------------------------------------------------------------

/// Until the swap, songs.rs stays the old engine's source: the files must not drift from it.
#[test]
fn every_song_file_parses_to_the_same_events_as_songs_rs() {
    for m in Music::ALL {
        let old = songs::song(m);
        let stem = library::stem(m);
        let new = library::load(stem).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(new.title, old.title, "{stem}");
        assert_eq!((new.bpm, new.swing, new.key, new.looping), (old.bpm, old.swing, old.key, old.looping), "{stem}");
        let srcs = [old.pulse1, old.pulse2, old.triangle, old.noise];
        for (ch, src) in srcs.iter().enumerate() {
            let channel = if ch == 3 { Channel::Drums } else { Channel::Melodic };
            assert_eq!(new.tracks[ch], mml::parse(src, channel).unwrap(), "{stem} channel {ch}");
        }
        let old_chart = (!old.chords.trim().is_empty()).then(|| chart::parse(old.chords).unwrap());
        assert_eq!(new.chart, old_chart, "{stem}: chart");
        // And the committed file is exactly what the converter makes today.
        assert_eq!(library::text(stem).unwrap(), convert::to_song_file(&old).unwrap(), "{stem}: run `cargo run --example convert_songs`");
    }
}

#[test]
fn song_files_round_trip() {
    for (stem, text) in library::FILES {
        let a = SongFile::parse(text).unwrap();
        let b = SongFile::parse(&a.to_text()).unwrap_or_else(|e| panic!("{stem}: {e}"));
        assert_eq!(a, b, "{stem}");
    }
}

#[test]
fn song_file_bar_lines_are_checked() {
    let text = library::text("shave_and_a_haircut").unwrap();
    // Every bar of every channel is checked: shorten any one note and the file is rejected.
    let broken = text.replacen("o5 c4 o4 g8 g8", "o5 c4 o4 g8 g16", 1);
    assert_ne!(broken, text);
    let e = SongFile::parse(&broken).unwrap_err();
    assert!(e.msg.contains("[pulse1]") && e.msg.contains("short"), "{e}");
    assert_eq!(broken.lines().nth(e.line - 1).unwrap().trim_start().get(..6), Some("v13 @1"), "{e}");
    // A bar line in the middle of a bar.
    let e = SongFile::parse(&text.replacen("g8 g8 a4", "g8 | g8 a4", 1)).unwrap_err();
    assert!(e.msg.contains("bar 1 is 1.5 beats long"), "{e}");
    // The converted files have (nearly) every bar line in place.
    for (stem, text) in library::FILES {
        let s = SongFile::parse(text).unwrap();
        let bars = s.bars();
        let marks = s.sources.iter().map(|src| src.matches('|').count()).sum::<usize>();
        assert!(marks >= bars, "{stem}: only {marks} bar lines for {bars} bars x 4 channels");
    }
}

/// Every character the MML parser treats as a token (not "unexpected"), every chord quality,
/// every section and every `[song]` key appears in the cheat sheet.
#[test]
fn the_cheat_sheet_covers_every_token() {
    let syntaxes: Vec<&str> = syntax::CHEAT_SHEET.iter().flat_map(|(_, rows)| rows.iter().map(|(s, _)| *s)).collect();
    let listed = |tok: &str| syntaxes.iter().any(|s| s.contains(tok));
    for c in (0x21u8..0x7f).map(char::from) {
        for channel in [Channel::Melodic, Channel::Drums] {
            // A token on its own, or with a number (commands), or after a note (accidentals, dots).
            let n = if channel == Channel::Melodic { "c4" } else { "k4" };
            let probes = [format!("{c}"), format!("{c}4"), format!("{n}{c}"), format!("{c}{n}")];
            let known = probes.iter().any(|p| match live_mml::parse(p, channel, 4.0) {
                Ok(_) => true,
                Err(e) => !e.msg.starts_with("unexpected"),
            });
            // Digits only ever follow something; they're covered by the lengths and commands.
            if known && !c.is_ascii_digit() {
                assert!(listed(&c.to_string()), "`{c}` ({channel:?}) is a token but not in the cheat sheet");
            }
        }
    }
    for (_, q) in chart::Quality::ALL {
        if !q.is_empty() {
            assert!(syntaxes.iter().any(|s| s.split_whitespace().any(|w| w == q)), "chord quality `{q}`");
        }
    }
    for s in syntax::SECTIONS {
        assert!(listed(&format!("[{s}]")), "section [{s}]");
    }
    for k in syntax::SONG_KEYS {
        assert!(listed(&format!("{k} = ")), "key {k}");
    }
    assert!(syntax::cheat_sheet_text().lines().count() > 30);
}
