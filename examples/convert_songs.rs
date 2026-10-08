//! Convert the soundtrack in `src/audio/songs.rs` into `music/*.song` files (for the live
//! engine), checking that every channel parses to exactly the same events.
//!
//! ```sh
//! cargo run --example convert_songs            # writes music/*.song
//! cargo run --example convert_songs -- out/    # somewhere else
//! ```

use std::path::Path;

use nat_han_adventures::audio::{
    Music,
    live::{convert, library, song::SongFile},
    mml, songs,
};

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "music".into());
    let dir = Path::new(&dir);
    std::fs::create_dir_all(dir).expect("create output dir");
    for m in Music::ALL {
        let song = songs::song(m);
        let text = convert::to_song_file(&song).unwrap_or_else(|e| panic!("{m:?}: {e}"));
        let parsed = SongFile::parse(&text).unwrap_or_else(|e| panic!("{m:?}: the converted file doesn't parse: {e}\n{text}"));
        let srcs = [song.pulse1, song.pulse2, song.triangle, song.noise];
        for (ch, src) in srcs.iter().enumerate() {
            let channel = if ch == 3 { mml::Channel::Drums } else { mml::Channel::Melodic };
            let old = mml::parse(src, channel).expect("songs.rs parses");
            assert_eq!(parsed.tracks[ch], old, "{m:?} channel {ch}: events differ");
        }
        let path = dir.join(format!("{}.song", library::stem(m)));
        std::fs::write(&path, &text).expect("write song file");
        println!("{:<40} {} bars, {} lines", path.display(), parsed.bars(), text.lines().count());
    }
}
