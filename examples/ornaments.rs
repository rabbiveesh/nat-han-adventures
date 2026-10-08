//! Listening renders for the band's ornaments and the instruments: every song at a few
//! freedom settings with a scripted game (a toot, a checkpoint, a death, a summon and back),
//! to WAVs, each with a log of what every musician played bar by bar (with timestamps, to
//! find the moments); and an instrument showcase.
//!
//! ```sh
//! cargo run --release --example ornaments                        # every song, 0 / 0.35 / 0.7
//! cargo run --release --example ornaments -- out/ tiger_rag      # one song, somewhere else
//! cargo run --release --example ornaments -- out/ all before     # a file-name prefix
//! ```
//!
//! Files: `<prefix>_<song>_f035.wav` + `.txt`, and `instruments_<song>.wav` (each song's
//! palette in turn: the base, then each alternate, a phrase each) + `.txt`.

use std::{fmt::Write as _, path::Path};

use kira::Frame;
use nat_han_adventures::audio::{
    Filters, Harmony,
    live::{
        Engine, EngineConfig, Input,
        band::{Fill, Flourish, HitKind, Trade},
        library,
        musician::Role,
        song::SongFile,
    },
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

/// What the script does at a bar, for the log.
fn script_note(bar: u64) -> &'static str {
    match bar {
        8 => "  <- toot",
        12 => "  <- checkpoint",
        20 => "  <- death",
        28 => "  <- summon: Coltrane",
        40 => "  <- back to as written",
        _ => "",
    }
}

/// The flourish tour (self-directed, like the game's director): toots summon Giant Steps,
/// a checkpoint, a death, a nugget streak, then the band let loose.
fn tour(bar: u64) -> Vec<Input> {
    match bar {
        1 => vec![Input::LevelStart, Input::SetFreedom { lead: 0.35, comp: 0.35, bass: 0.35, drums: 0.35, dynamics: 0.4 }],
        4 => vec![Input::Toot; 5],
        10 => vec![Input::Checkpoint],
        14 => vec![Input::Death],
        18 => vec![Input::Nugget; 6],
        24 => vec![Input::SetFreedom { lead: 0.7, comp: 0.7, bass: 0.7, drums: 0.7, dynamics: 0.6 }],
        _ => vec![],
    }
}

fn tour_note(bar: u64) -> &'static str {
    match bar {
        4 => "  <- 5 toots (summon)",
        10 => "  <- checkpoint",
        14 => "  <- death",
        18 => "  <- nugget streak",
        24 => "  <- freedom up to 0.7",
        _ => "",
    }
}

/// `listen`: the quick listening set (three songs at 0.35 and 0.7, a flourish tour for two,
/// the instrument showcase).
fn listen(dir: &Path) {
    for stem in ["sweet_georgia_brown", "the_entertainer", "muskrat_ramble"] {
        let file = SongFile::parse(library::text(stem).unwrap()).expect("song parses");
        for f in [0.35f32, 0.7] {
            let script = |bar: u64| if bar == 1 { vec![Input::LevelStart, Input::SetFreedom { lead: f, comp: f, bass: f, drums: f, dynamics: f * 0.7 }] } else { vec![] };
            let (out, log) = run(&file, script, 32, |_| "", false);
            let name = format!("{stem}_f{:03}", (f * 100.0).round() as u32);
            write_wav(&dir.join(format!("{name}.wav")), &out);
            std::fs::write(dir.join(format!("{name}.txt")), log).expect("write log");
            println!("{name}.wav");
        }
        if stem != "the_entertainer" {
            let (out, log) = run(&file, tour, 34, tour_note, true);
            write_wav(&dir.join(format!("tour_{stem}.wav")), &out);
            std::fs::write(dir.join(format!("tour_{stem}.txt")), log).expect("write log");
            println!("tour_{stem}.wav");
        }
    }
    instrument_tour(dir);
}

/// Every starter instrument, a phrase each (the song's first 4 bars, that channel soloed on it).
fn instrument_tour(dir: &Path) {
    let mut all: Vec<Frame> = Vec::new();
    let mut log = String::new();
    let mut seen = std::collections::HashSet::new();
    for (stem, text) in library::FILES {
        let file = SongFile::parse(text).expect("song parses");
        let insts = &file.instruments;
        for ch in 0..4 {
            for &i in insts.palette(ch).iter().filter(|&&i| i != 0) {
                let name = insts.name(i).to_string();
                if !seen.insert((name.clone(), ch)) {
                    continue;
                }
                let mut song = file.clone();
                song.sources[ch] = format!("@i {name} {}", song.sources[ch]);
                let Ok(song) = SongFile::parse(&song.to_text()) else { continue };
                let t = all.len() as f64 / RATE as f64;
                let _ = writeln!(log, "{:02}:{:02}  {name:<9} on {} ({stem}): first the channel alone, then the band", (t / 60.0) as u32, t as u32 % 60, nat_han_adventures::audio::live::song::CHANNELS[ch].0);
                let solo = |bar: u64| match bar {
                    1 => vec![Input::SetMix(std::array::from_fn(|c| if c == ch { 1.0 } else { 0.0 }))],
                    3 => vec![Input::SetMix([1.0; 4])],
                    _ => vec![],
                };
                let (out, _) = run(&song, solo, 4, |_| "", false);
                all.extend(out);
                all.extend(std::iter::repeat_n(Frame::ZERO, RATE as usize / 3));
            }
        }
    }
    write_wav(&dir.join("instruments_showcase.wav"), &all);
    std::fs::write(dir.join("instruments_showcase.txt"), log).expect("write log");
    println!("instruments_showcase.wav: {:.1}s", all.len() as f64 / RATE as f64);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().unwrap_or_else(|| DEFAULT_DIR.into());
    let only = args.next().filter(|s| s != "all");
    let prefix = args.next().unwrap_or_else(|| "after".into());
    let dir = Path::new(&dir);
    std::fs::create_dir_all(dir).expect("create output dir");
    if only.as_deref() == Some("listen") {
        listen(dir);
        return;
    }
    for (stem, text) in library::FILES {
        if only.as_deref().is_some_and(|o| o != stem) {
            continue;
        }
        let file = SongFile::parse(text).expect("song parses");
        for f in FREEDOMS {
            let (out, log) = run(&file, |bar| script(bar, f), BARS, script_note, false);
            let name = format!("{prefix}_{stem}_f{:03}", (f * 100.0).round() as u32);
            write_wav(&dir.join(format!("{name}.wav")), &out);
            std::fs::write(dir.join(format!("{name}.txt")), log).expect("write log");
            println!("{name}.wav: {:.1}s", out.len() as f64 / RATE as f64);
        }
        if prefix != "before" {
            showcase(&file, stem, dir);
        }
    }
}

/// Run an engine through a script, logging every committed bar.
fn run(file: &SongFile, script: impl Fn(u64) -> Vec<Input>, bars: u64, note: fn(u64) -> &'static str, self_directed: bool) -> (Vec<Frame>, String) {
    let mut e = Engine::with_config(file, RATE, EngineConfig { seed: 7, self_directed, ..EngineConfig::default() }).expect("engine");
    let looping = e.shape().looping;
    let mut out: Vec<Frame> = Vec::new();
    let mut buf = [Frame::ZERO; 256];
    let mut log = String::new();
    let mut logged = 0u64;
    for i in script(1) {
        e.post(i);
    }
    let mut next = 1u64;
    loop {
        let c = e.beat_clock();
        if c.position.bar >= bars || (!looping && e.finished()) {
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
        let s = e.state();
        let from = logged;
        for b in s.upcoming.iter().filter(|b| b.slot.index >= from) {
            let t = b.slot.start as f64 / RATE as f64;
            let _ = write!(log, "bar {:>3}  {:02}:{:05.2}  {:?}", b.slot.index + 1, (t / 60.0) as u32, t % 60.0, b.harmony);
            let _ = write!(log, "{}", note(b.slot.index + 1));
            let _ = writeln!(log);
            let p = b.band;
            let mut band = Vec::new();
            if p.hits != 0 {
                let beats: Vec<String> = p.hit_beats().map(|x| format!("{}", x + 1.0)).collect();
                band.push(format!("hits on {} ({})", beats.join(" "), if p.hit_kind == HitKind::Anticipation { "anticipation" } else { "ending" }));
            }
            for s in p.subs.iter().flatten() {
                band.push(format!("reharm {:?} {} on beats {}-{}", s.kind, s.chord, s.from + 1.0, s.to + 1.0));
            }
            if p.trade != Trade::None {
                band.push(format!("trading: {:?}'s four", p.trade));
            }
            if p.fill != Fill::None {
                band.push(format!("fill: {:?}", p.fill));
            }
            if p.crash {
                band.push("crash".into());
            }
            if p.flourish != Flourish::None {
                band.push(format!("flourish: {:?}", p.flourish));
            }
            if !band.is_empty() {
                let _ = writeln!(log, "    band:  {}", band.join("; "));
            }
            for r in Role::ALL {
                let o = b.orns[r as usize];
                if !o.is_empty() {
                    let _ = writeln!(log, "    {:<6} {}", format!("{r:?}").to_lowercase() + ":", o.names());
                }
            }
            logged = b.slot.index + 1;
        }
    }
    (out, log)
}

/// Each song's palette in turn, a phrase (4 bars) on each instrument: the musicians at a
/// mid freedom, every channel switched together by rewriting the song's `@i`.
fn showcase(file: &SongFile, stem: &str, dir: &Path) {
    let insts = &file.instruments;
    let longest = (0..4).map(|ch| insts.palette(ch).len()).max().unwrap_or(0);
    if longest <= 1 {
        return;
    }
    let mut all = Vec::new();
    let mut log = String::new();
    for k in 0..longest {
        let mut song = file.clone();
        let mut names = Vec::new();
        for ch in 0..4 {
            let p = insts.palette(ch);
            if p.is_empty() || song.sources[ch].trim().is_empty() {
                continue;
            }
            let i = p[k % p.len()];
            let name = insts.name(i).to_string();
            song.sources[ch] = format!("@i {name} {}", song.sources[ch]);
            names.push(format!("{}={name}", nat_han_adventures::audio::live::song::CHANNELS[ch].0));
        }
        let song = match SongFile::parse(&song.to_text()) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{stem}: {e}");
                return;
            }
        };
        let t = all.len() as f64 / RATE as f64;
        let _ = writeln!(log, "{:>2}:{:05.2}  {}", (t / 60.0) as u32, t % 60.0, names.join("  "));
        let (out, _) = run(&song, |bar| if bar == 1 { vec![Input::SetFreedom { lead: 0.0, comp: 0.0, bass: 0.0, drums: 0.0, dynamics: 0.0 }] } else { vec![] }, 8, |_| "", false);
        all.extend(out);
    }
    write_wav(&dir.join(format!("instruments_{stem}.wav")), &all);
    std::fs::write(dir.join(format!("instruments_{stem}.txt")), log).expect("write log");
    println!("instruments_{stem}.wav: {:.1}s", all.len() as f64 / RATE as f64);
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
