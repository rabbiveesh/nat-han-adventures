//! Render a few songs (and a scale demo) in every alternative [`Tuning`], to pick the "laughing
//! band" tuning by ear.
//!
//! ```sh
//! cargo run --release --example tunings -- out/
//! ```
//!
//! Files: `<piece>_<tuning>.wav` for piece in `title` (Sweet Georgia Brown), `world1` (The
//! Entertainer), `world3` (Muskrat Ramble), `scale` (C major up and down, then a ii–V–I, five
//! times: one phrase each, so `scale_medley` plays it in every medley tuning), and tuning in
//! `equal ji alpha bp tet7 harmonic drunk medley` (see [`audio::tuning`]; `medley` is what the
//! laughing band plays). Songs play their original harmony, one loop each.

use std::path::Path;

use nat_han_adventures::audio::{Filters, Music, Song, songs, synth, tuning::Tuning};

fn scale_demo() -> Song {
    Song {
        title: "Scale + ii-V-I (tuning demo)",
        bpm: 120.0,
        swing: 0.0,
        looping: false,
        // One 4-bar phrase (= one medley phrase, `tuning::MEDLEY_PHRASE_BARS`), five times.
        pulse1: "v12 @2 [o4 c8 d8 e8 f8 g8 a8 b8 > c8 | o5 c8 < b8 a8 g8 f8 e8 d8 c8 | o5 c2 o4 b2 | o4 b1 |]5",
        pulse2: "v9 @1 [r1 | r1 | o4 f2 f2 | o4 e1 |]5",
        triangle: "[r1 | r1 | o2 d2 g2 | o2 c1 |]5",
        noise: "",
        key: 0,
        chords: "",
    }
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: tunings <out-dir>");
        std::process::exit(2);
    });
    let dir = Path::new(&dir);
    std::fs::create_dir_all(dir).expect("create output dir");
    let pieces = [
        ("title", songs::song(Music::Title)),
        ("world1", songs::song(Music::World(1))),
        ("world3", songs::song(Music::World(3))),
        ("scale", scale_demo()),
    ];
    for (name, song) in &pieces {
        for t in Tuning::ALL {
            let r = synth::render_song_tuned(song, Filters::default(), t).expect("render");
            let file = format!("{name}_{}.wav", t.slug());
            write(&dir.join(&file), &r);
            println!("{file:<22} {:>6.1}s  \"{}\"", r.duration_secs(), song.title);
        }
    }
}

fn write(path: &Path, r: &synth::Rendered) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: r.sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).expect("create wav");
    let q = |x: f32| (x.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
    for f in &r.frames {
        w.write_sample(q(f.left)).unwrap();
        w.write_sample(q(f.right)).unwrap();
    }
    w.finalize().expect("finish wav");
}
