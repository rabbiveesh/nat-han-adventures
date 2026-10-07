//! Render every song and sound effect to WAV files, for listening outside the game.
//!
//! ```sh
//! cargo run --release --example render_audio -- out/       # everything
//! cargo run --release --example render_audio -- out/ 2     # loop each song twice (hear the seam)
//! ```

use std::{path::Path, time::Instant};

use durhay::audio::{Music, Sfx, sfx, songs, synth};

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().unwrap_or_else(|| {
        eprintln!("usage: render_audio <out-dir> [loops]");
        std::process::exit(2);
    });
    let loops: usize = args.next().map_or(1, |s| s.parse().expect("loops must be a number"));
    let dir = Path::new(&dir);
    std::fs::create_dir_all(dir).expect("create output dir");

    let mut total_ms = 0.0;
    let mut total_bytes = 0usize;
    for m in Music::ALL {
        let song = songs::song(m);
        let t = Instant::now();
        let r = match synth::render_song(&song) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{m:?}: {e}");
                continue;
            }
        };
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        total_ms += ms;
        total_bytes += r.frames.len() * 8;
        let name = match m {
            Music::World(w) => format!("music_world{w}"),
            other => format!("music_{other:?}").to_lowercase(),
        };
        let n = if r.looping { loops } else { 1 };
        write(&dir.join(format!("{name}.wav")), &r, n);
        println!(
            "{name:<20} {:>6.1}s {:>4} {:>7.1}ms  \"{}\"",
            r.duration_secs(),
            if r.looping { "loop" } else { "once" },
            ms,
            song.title
        );
    }
    println!("all music: {total_ms:.1}ms, {:.1} MB of frames", total_bytes as f64 / 1e6);

    let t = Instant::now();
    for s in Sfx::ALL {
        let r = sfx::render(s);
        write(&dir.join(format!("sfx_{s:?}.wav").to_lowercase()), &r, 1);
    }
    for v in 0..sfx::GUS_VARIANTS {
        write(&dir.join(format!("sfx_gusblip{v}.wav")), &sfx::gus_blip(v), 1);
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
