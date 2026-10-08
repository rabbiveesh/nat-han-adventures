//! Listening renders for the band's ornaments: every song at a few freedom settings, with a
//! scripted game (a toot, a checkpoint, a death, a summon and back), to WAVs.
//!
//! ```sh
//! cargo run --release --example ornaments                       # every song, 0 / 0.35 / 0.7
//! cargo run --release --example ornaments -- out/ tiger_rag     # one song, somewhere else
//! cargo run --release --example ornaments -- out/ all before    # a file-name prefix
//! ```

use std::path::Path;

use kira::Frame;
use nat_han_adventures::audio::{
    Filters, Harmony,
    live::{Engine, EngineConfig, Input, library, song::SongFile},
};

const RATE: u32 = 32_000;
const BARS: u64 = 48;
const DEFAULT_DIR: &str = "/tmp/claude-1000/-home-veesh-personal-durhay/2c5c2687-dc51-48ab-9741-24d8f862ee8a/scratchpad/ornaments";
const FREEDOMS: [f32; 3] = [0.0, 0.35, 0.7];

/// The inputs posted just before each (1-based) bar.
fn script(bar: u64, f: f32) -> Vec<Input> {
    match bar {
        1 => vec![Input::LevelStart, Input::SetFreedom { lead: f, comp: f, bass: f, drums: f, dynamics: f * 0.7 }],
        8 => vec![Input::Toot],
        12 => vec![Input::Checkpoint],
        20 => vec![Input::Death],
        28 => vec![Input::SetFilters(Filters { harmony: Harmony::Coltrane, just_intonation: false })],
        40 => vec![Input::SetFilters(Filters::default())],
        _ => vec![],
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().unwrap_or_else(|| DEFAULT_DIR.into());
    let only = args.next().filter(|s| s != "all");
    let prefix = args.next().unwrap_or_else(|| "after".into());
    let dir = Path::new(&dir);
    std::fs::create_dir_all(dir).expect("create output dir");
    for (stem, text) in library::FILES {
        if only.as_deref().is_some_and(|o| o != stem) {
            continue;
        }
        let file = SongFile::parse(text).expect("song parses");
        for f in FREEDOMS {
            let mut e = Engine::with_config(&file, RATE, EngineConfig { seed: 7, ..EngineConfig::default() }).expect("engine");
            let looping = e.shape().looping;
            let mut out: Vec<Frame> = Vec::new();
            let mut buf = [Frame::ZERO; 256];
            for i in script(1, f) {
                e.post(i);
            }
            let mut next = 1u64;
            loop {
                let c = e.beat_clock();
                if c.position.bar >= BARS || (!looping && e.finished()) {
                    break;
                }
                if c.position.bar + 1 == next && c.position.beat >= c.beats_per_bar - 1.5 {
                    for i in script(next + 1, f) {
                        e.post(i);
                    }
                    next += 1;
                }
                e.fill(&mut buf);
                out.extend_from_slice(&buf);
            }
            let name = format!("{prefix}_{stem}_f{:03}.wav", (f * 100.0).round() as u32);
            write_wav(&dir.join(&name), &out);
            println!("{name}: {:.1}s", out.len() as f64 / RATE as f64);
        }
    }
}

fn write_wav(path: &Path, frames: &[Frame]) {
    let spec = hound::WavSpec { channels: 2, sample_rate: RATE, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(path, spec).expect("create wav");
    let q = |x: f32| (x.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
    for f in frames {
        w.write_sample(q(f.left)).unwrap();
        w.write_sample(q(f.right)).unwrap();
    }
    w.finalize().expect("finish wav");
}
