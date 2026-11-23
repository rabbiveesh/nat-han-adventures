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
    /// Each channel's level in the balance, 0..=1 (amplitude; MIDI expression): the mix a band
    /// sits in ([`FontVoicing::big_band`]). 1 everywhere in a combo.
    pub trims: [f32; CHANNELS],
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
        FontVoicing { font, programs, gain: 3.0, trims: [1.0; CHANNELS] }
    }

    /// [`Self::combo`] balanced as a big band record is: the horns on top, the rhythm section
    /// the floor under them (the arranging and mixing books: the brass the apex, the saxes
    /// colour a few dB under, the bass and piano felt under the ensemble, the drums driving
    /// but behind the horns). Measured in a shout chorus through GeneralUser GS: trumpets
    /// about 3 dB over the altos, 6 over the low horns and the drums, 7 over bass and piano,
    /// the guitar 13 under. With the stage panned ([`FontVoices`]) and a room round the horns.
    pub fn big_band(font: Arc<SoundFont>) -> Self {
        let mut v = Self::combo(font);
        // The jazz kit (the standard one where a font has none).
        v.programs[3] = Some(32);
        // dB against the combo; the loudest (the drums) sets the overall gain.
        let mut db = [0.0f32; CHANNELS];
        use super::horns::Horn;
        db[0] = 2.3;
        db[1] = -0.7;
        db[2] = -7.0;
        db[3] = 7.0;
        db[4] = 2.3;
        for h in Horn::ALL {
            db[h.ch() as usize] = match h {
                Horn::Guitar => -1.0,
                Horn::BassTrombone => 7.7,
                Horn::Bari | Horn::Tuba => 10.0,
                Horn::Trumpets => 4.5,
                Horn::Altos => 9.0,
                _ => 8.0,
            };
        }
        let top = db.iter().copied().fold(f32::MIN, f32::max);
        for (t, d) in v.trims.iter_mut().zip(db) {
            *t = 10f32.powf((d - top) / 20.0);
        }
        // (Headroom for the shout's peaks.)
        v.gain *= 10f32.powf((top - 4.0) / 20.0);
        v
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

/// A MIDI message at a sample. Ordered by time, then note-offs (and controls) before note-ons
/// (a repeated note re-attacks, a bend resets before the next note), then queue order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Msg {
    at: u64,
    on: bool,
    seq: u64,
    ch: u8,
    key: u8,
    vel: u8,
    ctl: Ctl,
    /// The note it belongs to (later notes win a channel's bend, expression and vibrato).
    owner: u64,
}

/// A control change a note's shape sends ([`Fx`]: slides, falls, swells).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Ctl {
    None,
    /// Pitch bend, 0..16383 (8192 centred), over [`BEND_RANGE`].
    Bend(u16),
    /// Expression, as a gain in dB (tenths) against the channel's trim.
    Expr(i16),
    /// Modulation (the GM vibrato depth), 0..127.
    Mod(u8),
}

/// The pitch-bend range set on every melodic channel, in semitones (falls and doits go far).
const BEND_RANGE: i32 = 12;

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
    sample_rate: f32,
    /// The note being queued ([`Msg::owner`]), and each channel's latest note sounding.
    note: u64,
    owner: [u64; 16],
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
            // (On the drums, the program picks the kit: 0 standard, 32 jazz, 40 brushes.)
            if m != DRUM_CH || *p > 0 {
                synth.process_midi_message(m as i32, 0xC0, *p as i32, 0);
            }
            // The bandstand as a stereo record hears it: the soloist out front in the
            // middle, bass and drums centred, the piano left and the guitar right, the saxes
            // across the left (altos in, tenors, the bari at the end), the brass on the right
            // (trumpets out, the bass trombone and tuba in). Parts apart are parts heard.
            let pan = match ch {
                _ if dry => 64,
                0 => 64,
                1 => 36,
                4 => 46,
                5 => 74,
                6 => 34,
                7 => 70,
                8 => 90,
                9 => 86,
                10 => 52,
                11 => 42,
                _ => 64,
            };
            synth.process_midi_message(m as i32, 0xB0, 0x0A, pan);
            // Expression squared is the gain (the GM curve): the trim's square root.
            let expr = (127.0 * voicing.trims[ch].clamp(0.0, 1.0).sqrt()).round() as i32;
            if expr < 127 {
                synth.process_midi_message(m as i32, 0xB0, 0x0B, expr);
            }
            // A room round the horns (the band's glue), the rhythm section drier.
            let room = if (5..16).contains(&ch) && ch != 8 { 48 } else { 28 };
            synth.process_midi_message(m as i32, 0xB0, 0x5B, if dry { 0 } else { room });
            if m != DRUM_CH {
                // The bend range (RPN 0), for falls and doits.
                synth.process_midi_message(m as i32, 0xB0, 0x65, 0);
                synth.process_midi_message(m as i32, 0xB0, 0x64, 0);
                synth.process_midi_message(m as i32, 0xB0, 0x06, BEND_RANGE);
                synth.process_midi_message(m as i32, 0xB0, 0x26, 0);
            }
        }
        Ok(FontVoices { voicing, synth, msgs: BinaryHeap::new(), seq: 0, queued: Vec::new(), tied: [[0; 128]; 16], last: [None; 16], left: Vec::new(), right: Vec::new(), sample_rate: sample_rate as f32, note: 0, owner: [0; 16] })
    }

    pub(crate) fn routes(&self, ch: u8) -> bool {
        self.voicing.routes(ch)
    }

    fn send(&mut self, at: u64, on: bool, ch: u8, key: u8, vel: u8) {
        self.seq += 1;
        self.msgs.push(Reverse(Msg { at, on, seq: self.seq, ch, key, vel, ctl: Ctl::None, owner: self.note }));
    }

    fn control(&mut self, at: u64, ch: u8, ctl: Ctl) {
        self.seq += 1;
        self.msgs.push(Reverse(Msg { at, on: false, seq: self.seq, ch, key: 0, vel: 0, ctl, owner: self.note }));
    }

    /// A note's shape on its MIDI channel `ch` (melodic only): a slide onto it, a swell
    /// through it, a fall (or a doit, up) off its end; the bend and the level back after.
    fn shape(&mut self, ch: u8, ev: &NoteEvent, end: u64) {
        let fx = ev.fx;
        // (Not the piano or the guitar: they don't bend or swell.)
        if matches!(engine_channel(ch), 1 | 8) {
            return;
        }
        // A held note grows a vibrato after its attack, as a horn player's does.
        let depth = self.vibrato(engine_channel(ch));
        let secs = (end - ev.start) as f64 / self.sample_rate as f64;
        if depth > 0 && secs >= 0.45 {
            let sr = self.sample_rate as f64;
            let from = ev.start + (0.22 * sr) as u64;
            for k in 0..=4u64 {
                let at = from + (k as f64 * 0.08 * sr) as u64;
                if at < end {
                    self.control(at, ch, Ctl::Mod((depth as u64 * k / 4) as u8));
                }
            }
            self.control(end + 1, ch, Ctl::Mod(0));
        }
        if (fx.slide == 0 || fx.slide_frames == 0) && fx.fall == 0 && fx.swell == 0 {
            return;
        }
        let len = (end - ev.start) as f64;
        let sr = self.sample_rate as f64;
        let frames = |f: u8| (f as f64 * sr / 60.0).min(len);
        let bend = |semis: f64| Ctl::Bend((8192.0 + semis / BEND_RANGE as f64 * 8192.0).round().clamp(0.0, 16383.0) as u16);
        const STEPS: u64 = 8;
        if fx.slide != 0 && fx.slide_frames > 0 {
            let l = frames(fx.slide_frames);
            for k in 0..=STEPS {
                let x = k as f64 / STEPS as f64;
                self.control(ev.start + (l * x) as u64, ch, bend(fx.slide as f64 * (1.0 - x)));
            }
        }
        if fx.swell != 0 {
            // Up from `swell` dB under (a crescendo), or (negative) an attack falling away
            // `swell` dB and growing back (fp, crescendo).
            let db = fx.swell.unsigned_abs() as f64;
            for k in 0..=STEPS {
                let x = k as f64 / STEPS as f64;
                let g = if fx.swell > 0 { -db * (1.0 - x) } else if x < 0.25 { -db * x / 0.25 } else { -db * (1.0 - (x - 0.25) / 0.75) };
                self.control(ev.start + (len * x) as u64, ch, Ctl::Expr((g * 10.0).round() as i16));
            }
        }
        if fx.fall != 0 {
            let l = frames(fx.fall_frames.max(1));
            let from = end - l as u64;
            for k in 1..=STEPS {
                let x = k as f64 / STEPS as f64;
                // Slow, then away: the lip lets go.
                self.control(from + (l * x) as u64, ch, bend(fx.fall as f64 * x * x));
                if fx.fall < 0 {
                    self.control(from + (l * x) as u64, ch, Ctl::Expr((-12.0 * x * 10.0) as i16));
                }
            }
        }
        // Back to centre once it's off.
        self.control(end + 1, ch, bend(0.0));
        self.control(end + 1, ch, Ctl::Expr(0));
    }

    /// Queue a committed event on a routed channel.
    pub(crate) fn push(&mut self, ev: NoteEvent) {
        let Some(ch) = midi_channel(ev.ch) else { return };
        self.note += 1;
        let vel = velocity(ev.volume, ev.gain);
        let end = ev.end.max(ev.start + 1);
        match &ev.sound {
            Sound::Note(n) => {
                self.shape(ch, &ev, end);
                let k = (*n).min(127);
                if ev.tie && self.last[ch as usize].is_some_and(|(l, e)| l == k && e >= ev.start) {
                    // Held over: just end later.
                    self.tied[ch as usize][k as usize] = end;
                } else {
                    // No two notes struck quite alike (the drums have their own feel).
                    let h = spread(ev.salt ^ 0xd1, 0);
                    self.send(ev.start, true, ch, k, (vel as i32 + (h * 9.0) as i32 - 4).clamp(1, 127) as u8);
                }
                self.send(end, false, ch, k, 0);
                self.last[ch as usize] = Some((k, end));
            }
            Sound::Arp(a) => {
                self.shape(ch, &ev, end);
                let vel = (vel as f32 * 0.85) as u8;
                // A section is players, not a keyboard: each voice a few ms off and a little
                // louder or softer, the lead voice (the top) over the rest; the guitar strums.
                let section = ev.ch >= super::horns::FIRST_CH;
                let strum = super::horns::Horn::from_ch(ev.ch) == Some(super::horns::Horn::Guitar);
                let ms = self.sample_rate as f64 / 1000.0;
                for (i, &n) in a.notes().iter().enumerate() {
                    let k = n.min(127);
                    let h = spread(ev.salt, i as u64);
                    let (at, v) = if strum {
                        let rank = a.notes().iter().filter(|&&x| x < n).count();
                        (ev.start + (rank as f64 * 5.0 * ms) as u64, vel as i32 + (h * 8.0 - 4.0) as i32)
                    } else if section {
                        let rank = a.notes().iter().filter(|&&x| x > n).count();
                        let lead = ev.fx.balance.offset(rank, a.notes().len());
                        (ev.start + (h * 18.0 * ms) as u64, vel as i32 + lead + ((spread(ev.salt ^ 0x55, i as u64) * 14.0) as i32 - 7))
                    } else {
                        (ev.start, vel as i32)
                    };
                    self.send(at.min(end.saturating_sub(1)), true, ch, k, v.clamp(1, 127) as u8);
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

    /// How deep a held note's vibrato goes on engine channel `ch` (CC 1): the saxes and
    /// clarinet more, the brass lighter, the rest none.
    fn vibrato(&self, ch: u8) -> u8 {
        match self.voicing.programs.get(ch as usize).copied().flatten() {
            Some(64..=68) => 70,
            Some(71) => 50,
            Some(56 | 59) => 35,
            Some(57 | 58 | 60 | 61) => 25,
            _ => 0,
        }
    }

    /// A channel's shape back to plain: no bend, its trim's expression, no vibrato.
    fn neutral(&self, m: u8) -> [(i32, i32, i32); 3] {
        let trim = self.voicing.trims[engine_channel(m) as usize].clamp(0.0, 1.0);
        [(0xE0, 0, 64), (0xB0, 0x0B, (127.0 * trim.sqrt()).round() as i32), (0xB0, 0x01, 0)]
    }

    /// The MIDI bytes for a control message (`None`: a note).
    fn ctl_message(&self, m: Msg) -> Option<(i32, i32, i32)> {
        match m.ctl {
            Ctl::None => None,
            Ctl::Bend(v) => Some((0xE0, (v & 0x7f) as i32, (v >> 7) as i32)),
            Ctl::Mod(v) => Some((0xB0, 0x01, v as i32)),
            Ctl::Expr(tenths) => {
                let ch = engine_channel(m.ch);
                let trim = self.voicing.trims[ch as usize].clamp(0.0, 1.0) * 10f32.powf(tenths as f32 / 200.0);
                Some((0xB0, 0x0B, (127.0 * trim.clamp(0.0, 1.0).sqrt()).round() as i32))
            }
        }
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
                // A new note takes its channel's shape over: the last one's bend, swell and
                // vibrato end where it starts (a fall never drags the next note flat); the
                // old note's late shaping is dropped.
                let c = m.ch as usize;
                if m.ch != DRUM_CH && (m.on || m.ctl != Ctl::None) {
                    if m.owner < self.owner[c] {
                        if m.ctl != Ctl::None {
                            continue;
                        }
                    } else if m.owner > self.owner[c] {
                        self.owner[c] = m.owner;
                        for (status, a, b) in self.neutral(m.ch) {
                            self.synth.process_midi_message(m.ch as i32, status, a, b);
                        }
                    }
                }
                if let Some((status, a, b)) = self.ctl_message(m) {
                    self.synth.process_midi_message(m.ch as i32, status, a, b);
                } else if m.on {
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

/// A deterministic spread in [0, 1) for voice `i` of a note salted `salt`.
fn spread(salt: u64, i: u64) -> f64 {
    let mut x = salt ^ i.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x5eed_5eed;
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

/// The engine channel a MIDI channel plays (the inverse of [`midi_channel`]).
fn engine_channel(m: u8) -> u8 {
    match m {
        DRUM_CH => 3,
        3 => 9,
        c => c,
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

/// MIDI velocity from the chip's 0..=15 volume and the dynamics gain. GM players' level
/// goes about as 40 log10(velocity), so a volume step near the top is about 1 dB, 12 against
/// 15 about 3 dB and 6 about 13 dB: the musicians' dynamics as wide as a player's. (The gain,
/// mostly a part's level, keeps the gentler square root it always had.)
fn velocity(volume: u8, gain: f32) -> u8 {
    let v = (volume.min(15) as f32 / 15.0).powf(0.85) * gain.max(0.0).sqrt();
    (127.0 * v).round().clamp(8.0, 127.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::{Engine, EngineConfig, Input, library};

    #[test]
    fn velocity_follows_volume_and_dynamics() {
        assert_eq!(velocity(0, 1.0), 8);
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

    /// A fall bends the note's end down and the channel comes back level for the next one; a
    /// swell starts it soft. (Same SoundFont as above, skipped without one.)
    #[test]
    fn falls_and_swells_shape_a_note() {
        use crate::live::voice::Fx;
        let path = std::env::var("BANDSTAND_SF2").unwrap_or_else(|_| "/usr/share/sounds/sf2/FluidR3_GM.sf2".into());
        let Ok(bytes) = std::fs::read(&path) else {
            eprintln!("no soundfont at {path}: skipped");
            return;
        };
        let sr = 32_000u64;
        let render = |fx: Fx| {
            let mut v = FontVoices::dry(FontVoicing::combo(FontVoicing::load(&bytes).unwrap()), sr as u32).unwrap();
            let note = |start: u64, fx: Fx| NoteEvent {
                ch: 0,
                bar: 0,
                start,
                end: start + sr / 2,
                sound: Sound::Note(72),
                volume: 12,
                duty: 2,
                tie: false,
                slur_out: false,
                gain: 1.0,
                beat: 0.0,
                phrase_beat: 0.0,
                waltz: false,
                loop_start: 0,
                salt: 0,
                tuning: Default::default(),
                anchor: 0,
                inst: 0,
                fx,
                seq: 0,
            };
            v.push(note(0, fx));
            v.push(note(sr, Fx::NONE));
            let mut out = vec![Frame::ZERO; (sr * 2) as usize];
            v.render(&mut out, 0);
            out.iter().map(|f| f.left).collect::<Vec<f32>>()
        };
        let plain = render(Fx::NONE);
        let fall = render(Fx { fall: -7, fall_frames: 12, ..Fx::NONE });
        let swell = render(Fx { swell: 6, ..Fx::NONE });
        let rms = |x: &[f32]| (x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len() as f64).sqrt();
        let n = sr as usize;
        // The same attack; a different end; the next note just as it was.
        assert!((rms(&plain[..n / 8]) - rms(&fall[..n / 8])).abs() < 1e-6);
        assert!(plain[n / 3..n / 2].iter().zip(&fall[n / 3..n / 2]).any(|(a, b)| (a - b).abs() > 1e-3));
        assert!((rms(&plain[n + n / 8..n + n / 2]) / rms(&fall[n + n / 8..n + n / 2]) - 1.0).abs() < 0.02);
        // The swell starts under the plain note and the next note is level again.
        assert!(rms(&swell[..n / 16]) < rms(&plain[..n / 16]) * 0.8);
        assert!((rms(&plain[n + n / 8..n + n / 2]) / rms(&swell[n + n / 8..n + n / 2]) - 1.0).abs() < 0.02);
    }
}
