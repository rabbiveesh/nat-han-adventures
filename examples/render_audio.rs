//! Render every song (through every filter) and sound effect to WAV files, for listening
//! outside the game.
//!
//! ```sh
//! cargo run --release --example render_audio -- out/                # everything
//! cargo run --release --example render_audio -- out/ 2              # loop each song twice (hear the seam)
//! cargo run --release --example render_audio -- out/ 1 coltrane+ji  # only these filters
//! cargo run --release --example render_audio -- out/ 1 waltz title,world3  # the jazz waltz, two songs
//! ```
//!
//! Files: `music_<song>[_<harmony>][_ji].wav`, e.g. `music_world3_coltrane.wav`, `music_title_waltz.wav`,
//! `music_title_quartal_ji.wav`, `music_title_ji.wav` (as written, the laughing band's medley tuning), plus
//! `music_demo_*` for the built-in ii-V-I exercise ([`audio::demo`]). Songs without a chord
//! chart only get the original harmony.

use std::{path::Path, time::Instant};

use nat_han_adventures::audio::{self, Filters, Harmony, Music, Sfx, Song, sfx, songs, synth};

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().unwrap_or_else(|| {
        eprintln!("usage: render_audio <out-dir> [loops] [filters, e.g. coltrane+ji or waltz] [songs, e.g. title,world3]");
        std::process::exit(2);
    });
    let loops: usize = args.next().map_or(1, |s| s.parse().expect("loops must be a number"));
    let only: Option<Filters> =
        args.next().map(|s| Filters::parse(&s).expect("filters like quartal, melodic+ji, waltz"));
    let only_songs: Option<Vec<String>> = args.next().map(|s| s.split(',').map(|x| x.trim().to_lowercase()).collect());
    let dir = Path::new(&dir);
    std::fs::create_dir_all(dir).expect("create output dir");

    let mut songs: Vec<(String, Song)> = Music::ALL
        .into_iter()
        .map(|m| {
            let name = match m {
                Music::World(w) => format!("world{w}"),
                other => format!("{other:?}").to_lowercase(),
            };
            (name, songs::song(m))
        })
        .collect();
    songs.push(("demo".into(), audio::demo::demo_song()));

    let mut total_ms = 0.0;
    let mut worst_ms = 0.0f64;
    for (name, song) in &songs {
        if only_songs.as_ref().is_some_and(|o| !o.contains(name)) {
            continue;
        }
        for harmony in Harmony::ALL {
            if harmony != Harmony::Original && song.chords.trim().is_empty() {
                continue;
            }
            for just_intonation in [false, true] {
                let filters = Filters { harmony, just_intonation };
                if only.is_some_and(|o| o != filters) {
                    continue;
                }
                let t = Instant::now();
                let r = match synth::render_song_with(song, filters, 1) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("{name} {filters:?}: {e}");
                        continue;
                    }
                };
                let ms = t.elapsed().as_secs_f64() * 1000.0;
                total_ms += ms;
                worst_ms = worst_ms.max(ms);
                let mut file = format!("music_{name}");
                if harmony != Harmony::Original {
                    file += &format!("_{}", harmony.slug());
                }
                if just_intonation {
                    file += "_ji";
                }
                let n = if r.looping { loops } else { 1 };
                write(&dir.join(format!("{file}.wav")), &r, n);
                println!(
                    "{file:<28} {:>6.1}s {:>4} {:>7.1}ms  \"{}\"",
                    r.duration_secs(),
                    if r.looping { "loop" } else { "once" },
                    ms,
                    song.title
                );
            }
        }
    }
    println!("all music: {total_ms:.1}ms (slowest {worst_ms:.1}ms)");

    let t = Instant::now();
    for s in Sfx::ALL {
        let r = sfx::render(s);
        write(&dir.join(format!("sfx_{s:?}.wav").to_lowercase()), &r, 1);
    }
    for v in 0..sfx::HAN_VARIANTS {
        write(&dir.join(format!("sfx_hanblip{v}.wav")), &sfx::han_blip(v), 1);
    }
    println!("all sfx: {:.1}ms", t.elapsed().as_secs_f64() * 1000.0);
}

fn write(path: &Path, r: &synth::Rendered, repeats: usize) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: r.sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).expect("create wav");
    let q = |x: f32| (x.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
    for _ in 0..repeats {
        for f in &r.frames {
            w.write_sample(q(f.left)).unwrap();
            w.write_sample(q(f.right)).unwrap();
        }
    }
    w.finalize().expect("finish wav");
}
