//! The same take twice, through the chip and through a General MIDI SoundFont, as WAVs:
//!
//! ```text
//! cargo run --release -p bandstand --features soundfont --example font_render -- \
//!     /usr/share/sounds/sf2/FluidR3_GM.sf2 sweet_georgia_brown out/ [passes] [soloist] [seed]
//! ```
//!
//! Writes `<song>-chip.wav` and `<song>-font.wav` (32 kHz stereo): a blowing chorus (or more)
//! by one soloist, the same seed, so only the voices differ. The font take plays the combo
//! ([`FontVoicing::combo`]) with the soloist's horn on the lead.

use std::path::Path;

use bandstand::Frame;
use bandstand::live::chorus::{Call, Chorus};
use bandstand::live::font::FontVoicing;
use bandstand::live::soloist::Soloist;
use bandstand::live::{Engine, EngineConfig, Input, library};

const RATE: u32 = 32_000;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("usage: font_render <font.sf2> <song> <out dir> [passes] [soloist] [seed]");
        std::process::exit(2);
    }
    let font = FontVoicing::load(&std::fs::read(&args[0]).expect("read the soundfont")).expect("parse the soundfont");
    let song = library::load(&args[1]).unwrap_or_else(|e| panic!("{e}"));
    let out = Path::new(&args[2]);
    std::fs::create_dir_all(out).expect("out dir");
    let passes: u64 = args.get(3).map_or(1, |p| p.parse().expect("passes"));
    let soloist = args.get(4).map_or(Soloist::Sonny, |s| Soloist::parse(s).unwrap_or_else(|| panic!("no soloist `{s}`")));
    let seed: u64 = args.get(5).map_or(7, |s| s.parse().expect("seed"));
    let horn = match soloist {
        Soloist::Miles => 59, // muted trumpet
        Soloist::Pops | Soloist::Band => 56,
        Soloist::Bird => 65,
        _ => 66,
    };

    for (name, voicing) in [("chip", None), ("font", Some(FontVoicing::combo(font.clone()).with(0, Some(horn))))] {
        let config = EngineConfig { seed, soloist: Some(soloist), ..EngineConfig::default() };
        let mut e = Engine::with_config(&song, RATE, config).expect("engine");
        if let Some(v) = voicing {
            e.use_font(v).expect("font");
        }
        let call = Call { chorus: Chorus::Blowing, key_up: false };
        for i in [Input::LevelStart, Input::ForceChorus(Some(call)), Input::SetFreedom { lead: 0.5, comp: 0.5, bass: 0.5, drums: 0.5, dynamics: 0.5 }] {
            e.post(i);
        }
        let bars = e.shape().bars as u64 * passes;
        let mut frames = Vec::new();
        let mut buf = [Frame::ZERO; 256];
        let t = std::time::Instant::now();
        while e.beat_clock().position.bar < bars {
            e.fill(&mut buf);
            frames.extend_from_slice(&buf);
        }
        let secs = frames.len() as f64 / RATE as f64;
        let path = out.join(format!("{}-{name}.wav", args[1]));
        write_wav(&path, &frames);
        println!("{}: {secs:.1} s of music in {:.2} s", path.display(), t.elapsed().as_secs_f64());
    }
}

/// 16-bit stereo PCM.
fn write_wav(path: &Path, frames: &[Frame]) {
    let data = frames.len() as u32 * 4;
    let mut b = Vec::with_capacity(44 + data as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&RATE.to_le_bytes());
    b.extend_from_slice(&(RATE * 4).to_le_bytes());
    b.extend_from_slice(&4u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    for f in frames {
        for s in [f.left, f.right] {
            b.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
        }
    }
    std::fs::write(path, b).expect("write the wav");
}
