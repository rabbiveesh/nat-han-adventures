//! The soundtrack as `.song` files (`music/`), compiled in with `include_str!` so the web
//! build loads nothing at runtime.

pub use super::song::SongFile;
use super::song::SongError;
use crate::audio::Music;

/// `(file stem, text)` of every song.
pub const FILES: [(&str, &str); 8] = [
    ("sweet_georgia_brown", include_str!("../../../music/sweet_georgia_brown.song")),
    ("the_entertainer", include_str!("../../../music/the_entertainer.song")),
    ("tiger_rag", include_str!("../../../music/tiger_rag.song")),
    ("muskrat_ramble", include_str!("../../../music/muskrat_ramble.song")),
    ("st_louis_blues", include_str!("../../../music/st_louis_blues.song")),
    ("i_got_rhythm", include_str!("../../../music/i_got_rhythm.song")),
    ("shave_and_a_haircut", include_str!("../../../music/shave_and_a_haircut.song")),
    ("when_the_saints", include_str!("../../../music/when_the_saints.song")),
];

/// The file for a piece of the game's music.
pub fn stem(music: Music) -> &'static str {
    match music {
        Music::Title => "sweet_georgia_brown",
        Music::World(0 | 1) => "the_entertainer",
        Music::World(2) => "tiger_rag",
        Music::World(3) => "muskrat_ramble",
        Music::World(4) => "st_louis_blues",
        Music::World(_) => "i_got_rhythm",
        Music::LevelClear => "shave_and_a_haircut",
        Music::Victory => "when_the_saints",
    }
}

/// The text of a song file by stem.
pub fn text(stem: &str) -> Option<&'static str> {
    FILES.iter().find(|(s, _)| *s == stem).map(|(_, t)| *t)
}

/// Parse a song file by stem (errors name the file).
pub fn load(stem: &str) -> Result<SongFile, String> {
    let t = text(stem).ok_or_else(|| format!("no song file `{stem}`"))?;
    SongFile::parse(t).map_err(|e: SongError| format!("music/{stem}.song: {e}"))
}

/// Every song file, parsed once.
fn parsed() -> &'static [Result<SongFile, String>] {
    static SONGS: std::sync::OnceLock<Vec<Result<SongFile, String>>> = std::sync::OnceLock::new();
    SONGS.get_or_init(|| FILES.iter().map(|(stem, _)| load(stem)).collect())
}

/// The song for a piece of the game's music, parsed once: its title and file.
pub fn song(music: Music) -> Result<(&'static str, &'static SongFile), String> {
    let stem = stem(music);
    let i = FILES.iter().position(|(s, _)| *s == stem).expect("every stem has a file");
    parsed()[i].as_ref().map(|f| (f.title.as_str(), f)).map_err(Clone::clone)
}

/// The title of a piece of the game's music ("" if its file doesn't parse).
pub fn title(music: Music) -> &'static str {
    song(music).map_or("", |(t, _)| t)
}
