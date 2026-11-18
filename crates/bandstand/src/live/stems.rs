//! Stems: a take rendered one part at a time, each through its own SoundFont synth, dry and
//! mono, so the parts sum to the mix exactly (the `soundfont` feature). For training a stem
//! separator: every part's audio, and every part's notes as text ([`events_txt`]), so a part can
//! be voiced again later with other samples, or left out of the mix ("the band minus the
//! bass").
//!
//! Turn on [`Engine::tap`](super::Engine::tap), play the take with [`Engine::fill`](super::Engine::fill)
//! (its own sound is thrown away or kept as a reference), collect
//! [`Engine::take_tapped`](super::Engine::take_tapped), then [`render`].

use super::font::{CHANNELS, FontVoices, FontVoicing, drum_key};
use super::horns::Horn;
use super::voice::{NoteEvent, Sound};
use crate::Frame;

/// One part of a take.
#[derive(Debug, Clone)]
pub struct Stem {
    /// `lead`, `piano`, `bass`, `drums`, `tenor`, or a horn's slug (`bone4`, `bari`, `tuba`).
    pub name: &'static str,
    pub ch: u8,
    /// Mono samples, dry.
    pub samples: Vec<f32>,
    /// Its notes, in start order.
    pub events: Vec<NoteEvent>,
}

/// The name of the part on channel `ch`.
pub fn part_name(ch: u8) -> &'static str {
    match ch {
        0 => "lead",
        1 => "piano",
        2 => "bass",
        3 => "drums",
        4 => "tenor",
        _ => Horn::from_ch(ch).map_or("other", Horn::slug),
    }
}

/// Render each channel of `events` that `voicing` routes, `len` samples long, as its own stem
/// (in channel order; channels with no notes left out).
pub fn render(voicing: &FontVoicing, sample_rate: u32, events: &[NoteEvent], len: u64) -> Result<Vec<Stem>, String> {
    let mut stems = Vec::new();
    for ch in 0..CHANNELS as u8 {
        let Some(program) = voicing.programs[ch as usize] else { continue };
        let mut evs: Vec<NoteEvent> = events.iter().filter(|e| e.ch == ch).copied().collect();
        if evs.is_empty() {
            continue;
        }
        evs.sort_by_key(|e| (e.start, e.seq));
        let mut programs = [None; CHANNELS];
        programs[ch as usize] = Some(program);
        let mut voices = FontVoices::dry(FontVoicing { programs, ..voicing.clone() }, sample_rate)?;
        for e in &evs {
            voices.push(*e);
        }
        let mut samples = Vec::with_capacity(len as usize);
        let mut buf = vec![Frame::ZERO; 1024];
        let mut t = 0u64;
        while t < len {
            let n = (len - t).min(buf.len() as u64) as usize;
            let out = &mut buf[..n];
            out.fill(Frame::ZERO);
            voices.render(out, t);
            samples.extend(out.iter().map(|f| 0.5 * (f.left + f.right)));
            t += n as u64;
        }
        stems.push(Stem { name: part_name(ch), ch, samples, events: evs });
    }
    Ok(stems)
}

/// The sum of `stems` (all of them, or those `keep` says).
pub fn mix(stems: &[Stem], keep: impl Fn(&Stem) -> bool) -> Vec<f32> {
    let len = stems.iter().map(|s| s.samples.len()).max().unwrap_or(0);
    let mut out = vec![0.0f32; len];
    for s in stems.iter().filter(|s| keep(s)) {
        for (o, x) in out.iter_mut().zip(&s.samples) {
            *o += x;
        }
    }
    out
}

/// A part's notes as text, one line per note, space-separated, sorted by (start, note) with
/// that pair deduplicated: `start end note volume gain tie slur` (start and end in samples, end
/// one past the last; note MIDI; volume 0-15; gain the event's dynamics and mix gain; tie and
/// slur 0/1). Chords (the piano's) and drums (the GM kit's key) leave out tie and slur.
pub fn events_txt(events: &[NoteEvent]) -> String {
    let mut rows: Vec<(u64, u8, String)> = Vec::new();
    for e in events {
        let end = e.end.max(e.start + 1);
        match &e.sound {
            Sound::Note(n) => rows.push((e.start, *n, format!("{} {end} {n} {} {} {} {}", e.start, e.volume, e.gain, e.tie as u8, e.slur_out as u8))),
            Sound::Arp(a) => {
                for &n in a.notes() {
                    rows.push((e.start, n, format!("{} {end} {n} {} {}", e.start, e.volume, e.gain)));
                }
            }
            Sound::Drum(d) => {
                let k = drum_key(*d).0;
                rows.push((e.start, k, format!("{} {end} {k} {} {}", e.start, e.volume, e.gain)));
            }
        }
    }
    rows.sort_by_key(|r| (r.0, r.1));
    rows.dedup_by_key(|r| (r.0, r.1));
    let mut s = String::new();
    for (_, _, line) in rows {
        s.push_str(&line);
        s.push('\n');
    }
    s
}

/// A mono 32-bit float WAV (sums stay exact; nothing clips).
pub fn wav_f32(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let data = samples.len() as u32 * 4;
    let mut b = Vec::with_capacity(44 + data as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&3u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&sample_rate.to_le_bytes());
    b.extend_from_slice(&(sample_rate * 4).to_le_bytes());
    b.extend_from_slice(&4u16.to_le_bytes());
    b.extend_from_slice(&32u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    for x in samples {
        b.extend_from_slice(&x.to_le_bytes());
    }
    b
}
