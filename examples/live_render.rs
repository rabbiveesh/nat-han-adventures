//! Drive the live engine headlessly with a scripted input timeline and write WAVs, to hear
//! filters land on bar lines (the waltz included) and the freedom dial at work.
//!
//! ```sh
//! cargo run --release --example live_render                     # every song, the default script
//! cargo run --release --example live_render -- out/ tiger_rag   # one song, somewhere else
//! ```
//!
//! The script (bars count from 1, as played: a waltz bar is a bar), with the engine directing
//! itself from the gameplay inputs like the game's director would
//! ([`EngineConfig::self_directed`]):
//! - bar 4: five toots → Giant Steps (Coltrane changes), held at least 20 s;
//! - bar 12: two deaths → the laughing band (the tuning medley) joins in;
//! - bar 20: every freedom dial (and the dynamics) to 0.6;
//! - bar 28: a checkpoint (a decision): the summon has lapsed → as written (the laughing band
//!   stops at the next decision, its deaths counted from the checkpoint);
//! - bar 32: a jump in threes → the jazz waltz, from the next bar line (3/4, quarter = 120);
//! - then nothing: 20 s later a check takes the band back to 4/4, on a bar line both share.
//!
//! Files: `live_<song>.wav` and `live_<song>.txt` (what each bar was committed as).

use std::{fmt::Write as _, path::Path, time::Instant};

use kira::Frame;
use nat_han_adventures::audio::live::{Engine, EngineConfig, Input, library, song::SongFile};

const RATE: u32 = 32_000;
const BARS: u64 = 64;
const DEFAULT_DIR: &str = "/tmp/claude-1000/-home-veesh-personal-durhay/2c5c2687-dc51-48ab-9741-24d8f862ee8a/scratchpad/live";

/// The inputs posted just before each (1-based) bar.
fn script(bar: u64) -> Vec<Input> {
    match bar {
        1 => vec![Input::LevelStart],
        4 => vec![Input::Toot; 5],
        12 => vec![Input::Death, Input::Death],
        20 => vec![Input::SetFreedom { lead: 0.6, comp: 0.6, bass: 0.6, drums: 0.6, dynamics: 0.6 }],
        28 => vec![Input::Checkpoint],
        32 => vec![Input::WaltzStep],
        _ => vec![],
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().unwrap_or_else(|| DEFAULT_DIR.into());
    let only = args.next();
    let dir = Path::new(&dir);
    std::fs::create_dir_all(dir).expect("create output dir");
    for (stem, text) in library::FILES {
        if only.as_deref().is_some_and(|o| o != stem) {
            continue;
        }
        let file = SongFile::parse(text).expect("song parses");
        let config = EngineConfig { self_directed: true, ..EngineConfig::default() };
        let mut e = Engine::with_config(&file, RATE, config).expect("engine");
        let looping = e.shape().looping;
        let mut out: Vec<Frame> = Vec::new();
        let mut log = String::new();
        let mut buf = [Frame::ZERO; 64];
        let t = Instant::now();
        let mut blocks = 0u64;
        let mut logged = 0u64;
        for i in script(1) {
            e.post(i);
        }
        // The next bar (0-based) whose inputs are due, posted a beat and a half before its bar
        // line (before it's committed).
        let mut next = 1u64;
        loop {
            let c = e.beat_clock();
            if c.position.bar >= BARS || (!looping && e.finished()) {
                break;
            }
            if c.position.bar + 1 == next && c.position.beat >= c.beats_per_bar - 1.5 {
                for i in script(next + 1) {
                    e.post(i);
                }
                next += 1;
            }
            e.fill(&mut buf);
            out.extend_from_slice(&buf);
            blocks += 1;
            log_new_bars(&e, &mut logged, &mut log, RATE);
        }
        let secs = t.elapsed().as_secs_f64();
        let audio_secs = out.len() as f64 / RATE as f64;
        write_wav(&dir.join(format!("live_{stem}.wav")), &out);
        std::fs::write(dir.join(format!("live_{stem}.txt")), &log).expect("write log");
        println!(
            "{:<24} {:>6.1}s of audio in {:>6.1}ms ({:.0}x real time, {:.1} us per 512 frames)",
            stem,
            audio_secs,
            secs * 1000.0,
            audio_secs / secs,
            secs * 1e6 / blocks.max(1) as f64 * 8.0
        );
    }
    println!("wrote {}", dir.display());
}

/// Log the bars committed since the last call.
fn log_new_bars(e: &Engine, logged: &mut u64, log: &mut String, rate: u32) {
    let s = e.state();
    let from = *logged;
    for b in s.upcoming.iter().filter(|b| b.slot.index >= from) {
        let plan = s.musicians[3].plan.map_or(String::new(), |p| format!("drums plan: bars {}-{} to {:?}", p.start + 1, p.last_bar() + 1, p.target));
        let _ = writeln!(
            log,
            "bar {:>3} (song bar {:>2}, {:>6.2}s): {:?} / {:?}, intensity {:.2}, {} events; {plan}",
            b.slot.index + 1,
            b.slot.song_bar + 1,
            b.slot.start as f64 / rate as f64,
            b.harmony,
            b.tuning,
            b.intensity,
            b.events,
        );
        *logged = b.slot.index + 1;
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
