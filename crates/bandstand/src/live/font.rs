//! SoundFont voices (the `soundfont` feature): a channel can play through a General MIDI
//! SoundFont instead of the chip, picked per channel ([`FontVoicing`], given to
//! [`Engine::use_font`](super::Engine::use_font)). The same committed [`NoteEvent`]s drive it,
//! so the timing (pocket, drift, swing) is the engine's own; the player is `rustysynth` (pure
//! Rust, builds for wasm), live like the chip. The caller loads the `.sf2` (no asset files in
//! the crate). Nat Han itself stays 8-bit.
//!
//! - Notes play as written; a tie to the same pitch holds the note (no re-attack). An
//!   arpeggio (the comp's chord) plays as a chord, which is what a piano would do.
//! - Drums go to the GM kit (channel 10): kick, snare, the hats as a ride (as a jazz drummer
//!   plays them), crash.
//! - Velocity comes from the event's volume and dynamics; the engine's channel gains (the
//!   mix's ducking, mutes) become channel volume.
//! - Tunings (the laughing band's) don't reach it: a SoundFont plays equal temperament.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::fmt;
use std::sync::Arc;

use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};

use super::voice::{NoteEvent, Sound};
use crate::Frame;
use crate::mml::Drum;

/// The engine's channels (0 pulse 1: the lead, 1 pulse 2: the comp, 2 triangle: the bass,
/// 3 noise: the drums, 4 pulse 3: the soli's tenor, 5 on: the horns, [`super::horns`]), one
/// MIDI channel each (the drums on 10).
pub const CHANNELS: usize = 16;
/// The GM drum channel.
const DRUM_CH: u8 = 9;

/// Which channels play through a SoundFont, with which GM program (0-based: 0 piano, 32
/// acoustic bass, 56 trumpet, 65 alto sax, 66 tenor sax ...). `None` keeps the chip. For the
/// noise channel any `Some` means the GM kit.
#[derive(Clone)]
pub struct FontVoicing {
    pub font: Arc<SoundFont>,
    pub programs: [Option<u8>; CHANNELS],
    /// Overall level of the SoundFont voices against the chip's.
    pub gain: f32,
}

impl fmt::Debug for FontVoicing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FontVoicing").field("programs", &self.programs).field("gain", &self.gain).finish_non_exhaustive()
    }
}

impl FontVoicing {
    /// A jazz combo: trumpet lead, piano comp, upright bass, the GM kit, tenor in the soli; and
    /// the horns, should the engine have any ([`super::horns::Horn::program`]).
    pub fn combo(font: Arc<SoundFont>) -> Self {
        let mut programs = [None; CHANNELS];
        for (ch, p) in [56, 0, 32, 0, 66].into_iter().enumerate() {
            programs[ch] = Some(p);
        }
        for h in super::horns::Horn::ALL {
            programs[h.ch() as usize] = Some(h.program());
        }
        FontVoicing { font, programs, gain: 2.0 }
    }

    /// Channel `ch` on GM program `program` (`None`: the chip).
    pub fn with(mut self, ch: u8, program: Option<u8>) -> Self {
        if let Some(p) = self.programs.get_mut(ch as usize) {
            *p = program;
        }
        self
    }

    /// Parse an `.sf2` file's bytes.
    pub fn load(sf2: &[u8]) -> Result<Arc<SoundFont>, String> {
        SoundFont::new(&mut &sf2[..]).map(Arc::new).map_err(|e| format!("soundfont: {e:?}"))
    }

    pub fn routes(&self, ch: u8) -> bool {
        self.programs.get(ch as usize).is_some_and(Option::is_some)
    }
}

/// A MIDI message at a sample. Ordered by time, then note-offs before note-ons (a repeated
/// note re-attacks), then queue order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Msg {
    at: u64,
    on: bool,
    seq: u64,
    ch: u8,
    key: u8,
    vel: u8,
}

/// The SoundFont side of the voice bank.
pub(crate) struct FontVoices {
    voicing: FontVoicing,
    synth: Synthesizer,
    msgs: BinaryHeap<Reverse<Msg>>,
    seq: u64,
    /// Events queued and not started yet (for [`super::VoiceBank::pending`]).
    queued: Vec<NoteEvent>,
    /// How long a tie holds each key: a note-off before then is skipped.
    tied: [[u64; 128]; 16],
    /// The last note each channel played, and its end (a tie to it holds it).
    last: [Option<(u8, u64)>; 16],
    left: Vec<f32>,
    right: Vec<f32>,
}

impl FontVoices {
    pub(crate) fn new(voicing: FontVoicing, sample_rate: u32) -> Result<Self, String> {
        Self::build(voicing, sample_rate, false)
    }

    /// Dry and centred: no reverb or chorus, every channel in the middle (a stem: [`super::stems`]).
    pub(crate) fn dry(voicing: FontVoicing, sample_rate: u32) -> Result<Self, String> {
        Self::build(voicing, sample_rate, true)
    }

    fn build(voicing: FontVoicing, sample_rate: u32, dry: bool) -> Result<Self, String> {
        let mut settings = SynthesizerSettings::new(sample_rate as i32);
        settings.enable_reverb_and_chorus = !dry;
        let mut synth = Synthesizer::new(&voicing.font, &settings).map_err(|e| format!("soundfont synth: {e:?}"))?;
        for (ch, p) in voicing.programs.iter().enumerate() {
            let (Some(p), Some(m)) = (p, midi_channel(ch as u8)) else { continue };
            if m != DRUM_CH {
                synth.process_midi_message(m as i32, 0xC0, *p as i32, 0);
            }
            // Lead a little left, comp right, bass and drums centred; a touch of room.
            let pan = match ch {
                _ if dry => 64,
                0 => 52,
                1 => 76,
                4 => 44,
                5 => 58,
                6 => 70,
                _ => 64,
            };
            synth.process_midi_message(m as i32, 0xB0, 0x0A, pan);
            synth.process_midi_message(m as i32, 0xB0, 0x5B, if dry { 0 } else { 30 });
        }
        Ok(FontVoices { voicing, synth, msgs: BinaryHeap::new(), seq: 0, queued: Vec::new(), tied: [[0; 128]; 16], last: [None; 16], left: Vec::new(), right: Vec::new() })
    }

    pub(crate) fn routes(&self, ch: u8) -> bool {
        self.voicing.routes(ch)
    }

    fn send(&mut self, at: u64, on: bool, ch: u8, key: u8, vel: u8) {
        self.seq += 1;
        self.msgs.push(Reverse(Msg { at, on, seq: self.seq, ch, key, vel }));
    }

    /// Queue a committed event on a routed channel.
    pub(crate) fn push(&mut self, ev: NoteEvent) {
        let Some(ch) = midi_channel(ev.ch) else { return };
        let vel = velocity(ev.volume, ev.gain);
        let end = ev.end.max(ev.start + 1);
        match &ev.sound {
            Sound::Note(n) => {
                let k = (*n).min(127);
                if ev.tie && self.last[ch as usize].is_some_and(|(l, e)| l == k && e >= ev.start) {
                    // Held over: just end later.
                    self.tied[ch as usize][k as usize] = end;
                } else {
                    self.send(ev.start, true, ch, k, vel);
                }
                self.send(end, false, ch, k, 0);
                self.last[ch as usize] = Some((k, end));
            }
            Sound::Arp(a) => {
                let vel = (vel as f32 * 0.85) as u8;
                for &n in a.notes() {
                    let k = n.min(127);
                    self.send(ev.start, true, ch, k, vel.max(1));
                    self.send(end, false, ch, k, 0);
                }
                self.last[ch as usize] = None;
            }
            Sound::Drum(d) => {
                let (k, scale) = drum_key(*d);
                self.send(ev.start, true, DRUM_CH, k, ((vel as f32 * scale) as u8).max(1));
                self.send(end, false, DRUM_CH, k, 0);
            }
        }
        self.queued.push(ev);
    }

    /// Channel `ch`'s volume (0 mutes, 1 as written), from the next block.
    pub(crate) fn set_channel_gain(&mut self, ch: u8, gain: f32) {
        let Some(m) = midi_channel(ch) else { return };
        if !self.routes(ch) {
            return;
        }
        let v = (100.0 * gain.clamp(0.0, 1.27)).round() as i32;
        self.synth.process_midi_message(m as i32, 0xB0, 0x07, v);
    }

    pub(crate) fn pending(&self) -> impl Iterator<Item = &NoteEvent> {
        self.queued.iter()
    }

    pub(crate) fn idle(&self) -> bool {
        self.msgs.is_empty()
    }

    /// Mix samples `t0..t0 + out.len()` into `out`.
    pub(crate) fn render(&mut self, out: &mut [Frame], t0: u64) {
        let n = out.len();
        self.queued.retain(|e| e.start >= t0 + n as u64);
        if self.left.len() < n {
            self.left.resize(n, 0.0);
            self.right.resize(n, 0.0);
        }
        let mut pos = 0;
        while pos < n {
            let now = t0 + pos as u64;
            while let Some(Reverse(m)) = self.msgs.peek().copied() {
                if m.at > now {
                    break;
                }
                self.msgs.pop();
                if m.on {
                    self.synth.note_on(m.ch as i32, m.key as i32, m.vel as i32);
                } else if self.tied[m.ch as usize][m.key as usize] <= m.at {
                    // Not held over by a tie.
                    self.synth.note_off(m.ch as i32, m.key as i32);
                }
            }
            let next = self.msgs.peek().map_or(n, |Reverse(m)| (m.at.saturating_sub(t0) as usize).clamp(pos + 1, n));
            let (l, r) = (&mut self.left[pos..next], &mut self.right[pos..next]);
            self.synth.render(l, r);
            let g = self.voicing.gain;
            for (f, (l, r)) in out[pos..next].iter_mut().zip(l.iter().zip(r.iter())) {
                f.left += l * g;
                f.right += r * g;
            }
            pos = next;
        }
    }
}

/// The MIDI channel an engine channel plays on.
fn midi_channel(ch: u8) -> Option<u8> {
    match ch {
        3 => Some(DRUM_CH),
        // Channel 10 is the kit's: engine channel 9 takes the drums' MIDI channel.
        9 => Some(3),
        0..16 => Some(ch),
        _ => None,
    }
}

/// The GM kit's key for a drum (the hats as a ride, as a jazz drummer plays them), and how
/// hard it's struck against the event's velocity.
pub(crate) fn drum_key(d: Drum) -> (u8, f32) {
    match d {
        Drum::Kick => (36, 0.6),
        Drum::Snare => (38, 0.7),
        Drum::ClosedHat => (51, 0.75),
        Drum::OpenHat => (51, 0.95),
        Drum::Crash => (49, 0.9),
    }
}

/// MIDI velocity from the chip's 0..=15 volume and the dynamics gain.
fn velocity(volume: u8, gain: f32) -> u8 {
    let v = (volume.min(15) as f32 / 15.0) * gain.max(0.0);
    (20.0 + 107.0 * v.sqrt()).round().clamp(1.0, 127.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::{Engine, EngineConfig, Input, library};

    #[test]
    fn velocity_follows_volume_and_dynamics() {
        assert_eq!(velocity(0, 1.0), 20);
        assert_eq!(velocity(15, 1.0), 127);
        assert!(velocity(8, 1.0) < velocity(12, 1.0));
        assert!(velocity(12, 0.5) < velocity(12, 1.0));
        assert_eq!(velocity(15, 9.0), 127);
    }

    /// A take through a real SoundFont (`BANDSTAND_SF2`, or Debian's FluidR3 if installed;
    /// skipped without one): it sounds, stays in range, and the routed events still show as
    /// pending.
    #[test]
    fn a_take_plays_through_a_soundfont() {
        let path = std::env::var("BANDSTAND_SF2").unwrap_or_else(|_| "/usr/share/sounds/sf2/FluidR3_GM.sf2".into());
        let Ok(bytes) = std::fs::read(&path) else {
            eprintln!("no soundfont at {path}: skipped");
            return;
        };
        let font = FontVoicing::load(&bytes).unwrap();
        let mut e = Engine::with_config(&library::load("tiger_rag").unwrap(), 32_000, EngineConfig::default()).unwrap();
        e.use_font(FontVoicing::combo(font)).unwrap();
        e.post(Input::LevelStart);
        let mut out = vec![Frame::ZERO; 32_000 * 4];
        e.fill(&mut out);
        assert!(e.pending_events().count() > 0);
        let rms = (out.iter().map(|f| (f.left as f64).powi(2)).sum::<f64>() / out.len() as f64).sqrt();
        assert!(rms > 0.02, "silent: {rms}");
        assert!(out.iter().all(|f| f.left.is_finite() && f.left.abs() <= 1.0));
    }
}
