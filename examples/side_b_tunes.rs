//! Render Side B's tunes as the game plays them: the level intro, then two choruses with the
//! band at Side B's starting freedom ([`side_b_mood`]), so the choruses, feels and solos come out.
//!
//! ```sh
//! cargo run --release --example side_b_tunes -- out/
//! ```
//!
//! Files: `sideb<n>_<stem>.wav`.

use std::path::Path;

use kira::Frame;
use nat_han_adventures::audio::Music;
use nat_han_adventures::audio::live::{Engine, EngineConfig, Input, library};
use nat_han_adventures::game::adaptive::{band_freedom, side_b_mood};

const RATE: u32 = 48_000;

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "side-b-tunes".into());
    let dir = Path::new(&dir);
    std::fs::create_dir_all(dir).expect("create output dir");
    for t in 1..=Music::SIDE_B_TUNES {
        let music = Music::SideB(t);
        let (_, file) = library::song(music).expect("song parses");
        let config = EngineConfig { intro: true, ..EngineConfig::default() };
        let mut e = Engine::with_config(file, RATE, config).expect("engine");
        let f = band_freedom(side_b_mood(0, 0));
        e.post(Input::LevelStart);
        e.post(Input::SetFreedom { lead: f.lead, comp: f.comp, bass: f.bass, drums: f.drums, dynamics: f.dynamics });
        let bars = 2 * e.shape().bars as u64 + 4;
        let mut out: Vec<Frame> = Vec::new();
        let mut buf = [Frame::ZERO; 512];
        while e.beat_clock().position.bar < bars {
            e.fill(&mut buf);
            out.extend_from_slice(&buf);
        }
        let path = dir.join(format!("sideb{t}_{}.wav", library::stem(music)));
        write_wav(&path, &out);
        println!("{}: {:.0}s", path.display(), out.len() as f64 / RATE as f64);
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
