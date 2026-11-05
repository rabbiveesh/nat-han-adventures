//! The live engine's notes as a MIDI file, to hear the band outside the 8-bit synth (through
//! a General MIDI soundfont, say). A blowing chorus (or more) of a `.song` by one soloist:
//!
//! ```text
//! cargo run --release --example export_midi -- <song.song> <soloist> <out.mid> [passes] [seed] [lead program]
//! fluidsynth -ni -F out.wav -r 44100 /usr/share/sounds/sf2/FluidR3_GM.sf2 out.mid
//! ```
//!
//! Tracks: the lead (channel 1, the soloist's horn, or `lead program`), the comp (2, piano:
//! each chiptune arpeggio as a chord), the bass (3, upright), the drums (10: the kick
//! feathered, the snare, the hats as a ride cymbal, the crash), the soli's tenor (5).
//! Timing is the engine's own, pocket and drift included (one MIDI beat = half a second).

use std::collections::BTreeMap;

use kira::Frame;
use nat_han_adventures::audio::{
    live::{
        Engine, EngineConfig, Input,
        chorus::{Call, Chorus},
        soloist::Soloist,
        song::SongFile,
        voice::{NoteEvent, Sound},
    },
    mml::Drum,
};

const RATE: u32 = 32_000;
const PPQ: u32 = 480;
/// MIDI ticks a second at the file's 120 bpm.
const TICKS: f64 = 2.0 * PPQ as f64;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("usage: export_midi <song.song> <soloist> <out.mid> [passes] [seed] [lead program]");
        std::process::exit(2);
    }
    let text = std::fs::read_to_string(&args[0]).expect("read the song");
    let file = SongFile::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", args[0]));
    let soloist = Soloist::parse(&args[1]).unwrap_or_else(|| panic!("no soloist `{}`", args[1]));
    let passes: u64 = args.get(3).map_or(1, |p| p.parse().expect("passes"));
    let seed: u64 = args.get(4).map_or(7, |s| s.parse().expect("seed"));
    let horn = args.get(5).map(|p| p.parse::<u8>().expect("program")).unwrap_or(match soloist {
        Soloist::Miles => 59, // muted trumpet
        Soloist::Pops => 56,  // trumpet
        Soloist::Bird => 65,  // alto sax
        Soloist::Band => 56,
        _ => 66, // tenor sax
    });

    let config = EngineConfig { seed, soloist: Some(soloist), ..EngineConfig::default() };
    let mut e = Engine::with_config(&file, RATE, config).expect("engine");
    let bars = e.shape().bars as u64 * passes;
    let call = Call { chorus: Chorus::Blowing, key_up: false };
    for i in [Input::LevelStart, Input::ForceChorus(Some(call)), Input::SetFreedom { lead: 0.5, comp: 0.5, bass: 0.5, drums: 0.5, dynamics: 0.5 }] {
        e.post(i);
    }
    let mut events: BTreeMap<u64, NoteEvent> = BTreeMap::new();
    let mut buf = [Frame::ZERO; 256];
    let mut end = 0u64;
    loop {
        let c = e.beat_clock();
        if c.position.bar >= bars {
            break;
        }
        e.fill(&mut buf);
        end += buf.len() as u64;
        for ev in e.pending_events() {
            if ev.bar < bars {
                events.insert(ev.seq, *ev);
            }
        }
    }

    // Each channel's notes as (tick, on/off, key, velocity).
    let tick = |s: u64| (s as f64 / RATE as f64 * TICKS).round() as u32;
    let mut tracks: Vec<(u8, u8, &str, Vec<(u32, bool, u8, u8)>)> =
        vec![(0, horn, "lead", vec![]), (1, 0, "comp", vec![]), (2, 32, "bass", vec![]), (9, 0, "drums", vec![]), (4, 66, "soli tenor", vec![])];
    for ev in events.values() {
        let k = match ev.ch {
            0 => 0,
            1 => 1,
            2 => 2,
            3 => 3,
            _ => 4,
        };
        let vel = |scale: f32| ((ev.volume as f32 / 15.0 * ev.gain * scale).clamp(0.0, 1.0) * 100.0 + 20.0).round().min(127.0) as u8;
        let (a, b) = (tick(ev.start), tick(ev.end.max(ev.start + 1)).max(tick(ev.start) + 1));
        let mut add = |key: u8, v: u8, len: Option<u32>| {
            tracks[k].3.push((a, true, key, v));
            tracks[k].3.push((len.map_or(b, |l| a + l), false, key, 0));
        };
        match ev.sound {
            Sound::Note(n) => add(n, vel(1.0), None),
            Sound::Arp(arp) => {
                for &n in arp.notes() {
                    add(n, vel(0.8), None);
                }
            }
            Sound::Drum(d) => {
                let short = Some(PPQ / 4);
                match d {
                    Drum::Kick => add(36, vel(0.45), short),
                    Drum::Snare => add(38, vel(0.6), short),
                    Drum::ClosedHat => add(51, vel(0.8), short),
                    Drum::OpenHat => add(51, vel(1.0), short),
                    Drum::Crash => add(49, vel(0.9), short),
                }
                if d == Drum::Kick {
                    // The ride keeps time over the feathered kick.
                    add(51, vel(0.75), short);
                }
            }
        }
    }
    let _ = end;

    // The file: a tempo track, then one track a channel.
    let mut smf = Vec::new();
    smf.extend_from_slice(b"MThd");
    smf.extend_from_slice(&6u32.to_be_bytes());
    smf.extend_from_slice(&1u16.to_be_bytes());
    smf.extend_from_slice(&(tracks.len() as u16 + 1).to_be_bytes());
    smf.extend_from_slice(&(PPQ as u16).to_be_bytes());
    let mut tempo = vec![0x00, 0xFF, 0x51, 0x03, 0x07, 0xA1, 0x20];
    tempo.extend_from_slice(&[0x00, 0xFF, 0x2F, 0x00]);
    chunk(&mut smf, &tempo);
    for (ch, program, name, mut notes) in tracks {
        notes.sort_by_key(|&(t, on, ..)| (t, on));
        let mut d = Vec::new();
        d.push(0);
        d.extend_from_slice(&[0xFF, 0x03, name.len() as u8]);
        d.extend_from_slice(name.as_bytes());
        d.extend_from_slice(&[0x00, 0xC0 | ch, program]);
        let mut last = 0;
        for (t, on, key, v) in notes {
            vlq(&mut d, t - last);
            last = t;
            d.extend_from_slice(&[if on { 0x90 } else { 0x80 } | ch, key, v]);
        }
        d.extend_from_slice(&[0x00, 0xFF, 0x2F, 0x00]);
        chunk(&mut smf, &d);
    }
    std::fs::write(&args[2], smf).expect("write the MIDI file");
    println!("{}: {bars} bars, {} events", args[2], events.len());
}

fn chunk(out: &mut Vec<u8>, data: &[u8]) {
    out.extend_from_slice(b"MTrk");
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(data);
}

fn vlq(out: &mut Vec<u8>, mut x: u32) {
    let mut b = [(x & 0x7F) as u8; 5];
    let mut n = 1;
    x >>= 7;
    while x > 0 {
        b[n] = (x & 0x7F) as u8 | 0x80;
        n += 1;
        x >>= 7;
    }
    out.extend(b[..n].iter().rev());
}
