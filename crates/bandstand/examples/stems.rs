//! A take as stems, for training a stem separator: every part's dry mono audio and its notes,
//! plus the mix and what was chosen.
//!
//! ```text
//! cargo run --release -p bandstand --features soundfont --example stems -- \
//!     /usr/share/sounds/sf2/FluidR3_GM.sf2 out/ [--seed N] [--song stem] [--horns bone4,bari,tuba]
//!     [--low unison|octave|pedal|pads|hits|tacet] [--chorus shout|soli|...] [--passes N]
//!     [--freedom 0..1]
//! ```
//!
//! Anything not given is rolled from the seed (the song, who's on the stand, the freedom), so
//! `--seed 1..1000` makes a thousand different takes. Writes, at 44.1 kHz:
//! `<song>-<seed>-<part>.wav` (mono f32, dry; parts: lead, piano, bass, drums, tenor, bone4,
//! bari, tuba), `<song>-<seed>-<part>.txt` (its notes: `start end note volume gain tie slur`,
//! [`bandstand::live::stems::events_txt`]), `<song>-<seed>-mix.wav` (the sum) and
//! `<song>-<seed>.json` (the choices, and each bar's chorus and low-horn role).

use std::collections::BTreeMap;
use std::path::Path;

use bandstand::Frame;
use bandstand::live::chorus::{Call, Chorus};
use bandstand::live::font::FontVoicing;
use bandstand::live::horns::{Horn, Horns, LowRole};
use bandstand::live::{Engine, EngineConfig, Input, library, stems};

const RATE: u32 = 44_100;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |args: &mut Vec<String>, name: &str| -> Option<String> {
        let i = args.iter().position(|a| a == name)?;
        let v = args.get(i + 1).cloned();
        args.drain(i..(i + 2).min(args.len()));
        v
    };
    let seed: u64 = flag(&mut args, "--seed").map_or(1, |s| s.parse().expect("seed"));
    let song_arg = flag(&mut args, "--song");
    let horns_arg = flag(&mut args, "--horns");
    let low = flag(&mut args, "--low").map(|s| LowRole::parse(&s).unwrap_or_else(|| panic!("no role `{s}`")));
    let chorus = flag(&mut args, "--chorus").map(|s| Chorus::parse(&s).unwrap_or_else(|| panic!("no chorus `{s}`")));
    let passes: u64 = flag(&mut args, "--passes").map_or(2, |s| s.parse().expect("passes"));
    let freedom_arg: Option<f32> = flag(&mut args, "--freedom").map(|s| s.parse().expect("freedom"));
    if args.len() < 2 {
        eprintln!("usage: stems <font.sf2> <out dir> [--seed N] [--song stem] [--horns bone4,bari,tuba] [--low role] [--chorus c] [--passes N] [--freedom f]");
        std::process::exit(2);
    }
    let font = FontVoicing::load(&std::fs::read(&args[0]).expect("read the soundfont")).expect("parse the soundfont");
    let out = Path::new(&args[1]);
    std::fs::create_dir_all(out).expect("out dir");

    // The dice.
    let mut rng = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut roll = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        (rng >> 11) as f64 / (1u64 << 53) as f64
    };
    let song_stem = song_arg.unwrap_or_else(|| library::FILES[(roll() * library::FILES.len() as f64) as usize].0.to_string());
    let horns = match horns_arg {
        Some(s) => Horns::parse(&s).unwrap_or_else(|| panic!("bad --horns `{s}`")),
        None => {
            let mut h = Horns::NONE;
            for (horn, p) in [(Horn::BassTrombone, 0.85), (Horn::Bari, 0.85), (Horn::Tuba, 0.3)] {
                if roll() < p {
                    h = h.with(horn);
                }
            }
            if h.is_empty() { Horns::LOW } else { h }
        }
    };
    let freedom = freedom_arg.unwrap_or_else(|| 0.4 + 0.5 * roll() as f32);
    let lead = [56, 56, 65, 66, 71][(roll() * 5.0) as usize];

    let song = library::load(&song_stem).unwrap_or_else(|e| panic!("{e}"));
    let config = EngineConfig { seed, horns, ..EngineConfig::default() };
    let mut e = Engine::with_config(&song, RATE, config).expect("engine");
    let voicing = FontVoicing::combo(font).with(0, Some(lead));
    e.use_font(voicing.clone()).expect("font");
    e.tap(true);
    e.post(Input::LevelStart);
    e.post(Input::SetFreedom { lead: freedom, comp: freedom, bass: freedom, drums: freedom, dynamics: 0.5 });
    e.post(Input::ForceLowEnd(low));
    if let Some(chorus) = chorus {
        e.post(Input::ForceChorus(Some(Call { chorus, key_up: false })));
    }
    let bars = e.shape().bars as u64 * passes;
    let mut buf = [Frame::ZERO; 1024];
    let mut t = 0u64;
    let mut seen: BTreeMap<u64, String> = BTreeMap::new();
    let mut events = Vec::new();
    while e.beat_clock().position.bar < bars {
        e.fill(&mut buf);
        t += buf.len() as u64;
        events.extend(e.take_tapped());
        for b in e.state().upcoming.iter().filter(|b| b.slot.index < bars) {
            seen.entry(b.slot.index).or_insert_with(|| {
                format!(
                    "{{\"bar\":{},\"pass\":{},\"song_bar\":{},\"chorus\":\"{}\",\"feel\":\"{}\",\"low\":\"{}\"}}",
                    b.slot.index,
                    b.slot.pass,
                    b.slot.song_bar,
                    b.band.chorus.slug(),
                    b.band.feel.slug(),
                    b.low.map_or("", LowRole::slug)
                )
            });
        }
    }
    // Only the bars played; let the last notes ring a second.
    events.retain(|ev| ev.start < t);
    let len = events.iter().map(|ev| ev.end).max().unwrap_or(t).min(t) + RATE as u64;
    let parts = stems::render(&voicing, RATE, &events, len).expect("render");
    let name = format!("{song_stem}-{seed}");
    for s in &parts {
        std::fs::write(out.join(format!("{name}-{}.wav", s.name)), stems::wav_f32(&s.samples, RATE)).expect("write wav");
        std::fs::write(out.join(format!("{name}-{}.txt", s.name)), stems::events_txt(&s.events)).expect("write events");
    }
    let mix = stems::mix(&parts, |_| true);
    let peak = mix.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    std::fs::write(out.join(format!("{name}-mix.wav")), stems::wav_f32(&mix, RATE)).expect("write mix");
    let json = format!(
        "{{\"song\":\"{song_stem}\",\"seed\":{seed},\"bpm\":{},\"sample_rate\":{RATE},\"samples\":{len},\"freedom\":{freedom},\"lead_program\":{lead},\"horns\":\"{}\",\"forced_low\":\"{}\",\"forced_chorus\":\"{}\",\"peak\":{peak},\"parts\":[{}],\"bars\":[\n{}\n]}}\n",
        e.shape().bpm,
        horns.slugs(),
        low.map_or("", LowRole::slug),
        chorus.map_or("", Chorus::slug),
        parts.iter().map(|s| format!("\"{}\"", s.name)).collect::<Vec<_>>().join(","),
        seen.values().cloned().collect::<Vec<_>>().join(",\n")
    );
    std::fs::write(out.join(format!("{name}.json")), json).expect("write json");
    println!("{name}: {:.1} s, parts {}, peak {peak:.2}", len as f64 / RATE as f64, parts.iter().map(|s| s.name).collect::<Vec<_>>().join(" "));
}
