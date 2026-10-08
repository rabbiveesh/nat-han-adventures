//! Drive the live engine headlessly with a scripted input timeline and write WAVs, to hear
//! filters land on bar lines and the freedom dial at work.
//!
//! ```sh
//! cargo run --release --example live_render                     # every song, the default script
//! cargo run --release --example live_render -- out/ tiger_rag   # one song, somewhere else
//! ```
//!
//! The script (bars count from 1), with the engine directing itself from the stats like the
//! game's director would ([`EngineConfig::self_directed`]):
//! - bar 4: five toots → Giant Steps (Coltrane changes);
//! - bar 12: two deaths at a checkpoint → the laughing band (the tuning medley); the toots
//!   have aged out of the window, so the harmony goes back to as written;
//! - bar 20: every freedom dial (and the dynamics) to 0.6;
//! - bar 28: a checkpoint, the stats calm down → as written, freedom stays.
//!
//! Files: `live_<song>.wav` (40 bars) and `live_<song>.txt` (what each bar was committed as).

use std::{fmt::Write as _, path::Path, time::Instant};

use bevy_kira_audio::prelude::Frame;
use nat_han_adventures::audio::{
    director::PlayStats,
    live::{Engine, EngineConfig, Input, library, song::SongFile},
};

const RATE: u32 = 32_000;
const BARS: u64 = 40;
const DEFAULT_DIR: &str = "/tmp/claude-1000/-home-veesh-personal-durhay/2c5c2687-dc51-48ab-9741-24d8f862ee8a/scratchpad/live";

/// The inputs posted at the start of each (1-based) bar.
fn script(bar: u64) -> Vec<Input> {
    let stats = |toots, deaths, cp_deaths| {
        Input::SetStats(PlayStats {
            level_deaths: deaths,
            checkpoint_deaths: cp_deaths,
            stretch_toots: toots,
            stretch_nuggets: 0,
            stretch_deaths: deaths,
            stretch_secs: 20.0,
        })
    };
    match bar {
        4 => [vec![Input::Toot; 5], vec![stats(5, 0, 0)]].concat(),
        12 => vec![Input::Death, Input::Death, stats(0, 2, 2)],
        20 => vec![Input::SetFreedom { lead: 0.6, comp: 0.6, bass: 0.6, drums: 0.6, dynamics: 0.6 }],
        28 => vec![Input::Checkpoint, stats(0, 2, 0)],
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
        let shape = e.shape().clone();
        let bars = if shape.looping { BARS } else { shape.bars as u64 + 1 };
        let mut out: Vec<Frame> = Vec::new();
        let mut log = String::new();
        let mut buf = vec![Frame::ZERO; 512];
        let t = Instant::now();
        let mut blocks = 0u64;
        let mut logged = 0u64;
        for bar in 0..bars {
            // Post the bar's inputs a beat and a half before its bar line (before it's committed).
            let pass = bar / shape.bars as u64;
            let start = pass * shape.len + shape.bar_starts[(bar % shape.bars as u64) as usize];
            let post_at = start.saturating_sub((shape.samples_per_beat * 1.5) as u64);
            render_until(&mut e, post_at, &mut out, &mut buf, &mut blocks);
            for i in script(bar + 1) {
                e.post(i);
            }
            // Apply them now (and commit whatever is due).
            e.fill(&mut []);
            let end = start + (shape.bar_starts[1] - shape.bar_starts[0]);
            log_new_bars(&e, &mut logged, &mut log, RATE);
            render_until(&mut e, end, &mut out, &mut buf, &mut blocks);
            log_new_bars(&e, &mut logged, &mut log, RATE);
            if e.finished() {
                break;
            }
        }
        let secs = t.elapsed().as_secs_f64();
        let audio_secs = out.len() as f64 / RATE as f64;
        write_wav(&dir.join(format!("live_{stem}.wav")), &out);
        std::fs::write(dir.join(format!("live_{stem}.txt")), &log).expect("write log");
        println!(
            "{:<24} {:>6.1}s of audio in {:>6.1}ms ({:.0}x real time, {:.1} us per 512-frame block)",
            stem,
            audio_secs,
            secs * 1000.0,
            audio_secs / secs,
            secs * 1e6 / blocks.max(1) as f64
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

fn render_until(e: &mut Engine, to: u64, out: &mut Vec<Frame>, buf: &mut [Frame], blocks: &mut u64) {
    loop {
        let now = e.beat_clock().position.sample;
        if now >= to {
            return;
        }
        let n = ((to - now) as usize).min(buf.len());
        e.fill(&mut buf[..n]);
        out.extend_from_slice(&buf[..n]);
        *blocks += 1;
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
