//! Instruments, FamiTracker style: a song's `[instruments]` section names *tone* instruments
//! (macros stepped once per 60 Hz NES frame: volume, duty, pitch, vibrato) and drum *kits*
//! (per-drum synthesis parameters), and lists each channel's *palette* (the instruments its
//! musician may switch to). MML picks one with `@i <name>` (it can change mid-line).
//!
//! ```text
//! [instruments]
//! lead  : vol 15 14 12 10 9 8 | duty 2 | vib delay=8 depth=12 speed=5.5
//! pluck : vol 15 9 6 4 2 0 | duty 1 1 2
//! brass : vol 8 12 15 | 14 / 10 6 3 0 | duty 0 1 2 | pitch +3 +1 0
//! flute : tri | vol 6 10 12 | vib delay=6 depth=20 speed=5
//! kit   : kick pitch=-24 decay=6 | snare noise=short decay=8 | hat decay=2
//! pulse1 = default brass pluck   ; the lead's palette (its first is the base)
//! noise = default kit
//! ```
//!
//! # Tone instruments
//! A line `name : macros`. Each macro starts with its keyword; numbers after it are its
//! sequence, one step per frame (1/60 s). Inside a sequence `|` marks where it loops from and
//! `/` where the *release* starts (both FamiTracker's); a `|` before the next keyword (or at
//! the end) only separates macros. A sequence without a loop holds its last step.
//! - `vol 15 12 9 ...` 0..=15, scales the note's MML volume (`v`): 15 is as written.
//! - `duty 0 1 2 ...` pulse duty 0..=3 per frame (overrides `@n`).
//! - `pitch +3 +1 0` (or `arp 0 4 7 |`): offset from the note in semitones per frame
//!   (fractions allowed: `+0.5` is 50 cents). Not cumulative: each step is the offset itself.
//! - `vib delay=<frames> depth=<cents> speed=<Hz> [ramp=<frames>]`: delayed vibrato (single
//!   notes only, as the NES's did), fading in over `ramp`.
//! - `fade <secs>`: the built-in pulses' gentle decay to 65% (time constant in seconds).
//! - `tri`: the triangle's 4-bit waveform on any channel (the triangle channel is always tri).
//!
//! The release part (after `/`) plays when the note ends, unless the next note cuts it (it
//! starts on time; the tail ducks out in ~8 ms) or slurs on from it.
//!
//! # Kits
//! A line `name : drum params | drum params ...` with drums `kick snare hat ohat crash` and
//! `key=value` params (decays in frames, 1/60 s):
//! - kick: `pitch=<semitones>` (the sweep's start, below 0 = from above), `hz=` the bottom,
//!   `sweep=` the pitch drop's time constant, `decay=`, `click=` 0..1.
//! - snare: `noise=short|long` (the NES's metallic short mode), `period=0..15` (NES noise
//!   period: lower is brighter), `decay=` the rattle, `tone=<Hz>` and `body=` its decay.
//! - hat, ohat, crash: `decay=`, `period=`, `noise=`.
//!
//! Lengths follow the decays (a longer decay rings longer). Unset params keep the built-in
//! kit's.
//!
//! # Defaults
//! `default` names each channel's built-in instrument: what every song played before
//! instruments existed (pulse 1 with its delayed vibrato and gentle decay, pulse 2 without the
//! vibrato, the triangle, the noise kit). A channel starts on it; a song without an
//! `[instruments]` section plays exactly as before (`tests/live.rs` checks bit for bit).
//!
//! # For the band ([`super::musician`]) and feels
//! [`Instruments::palette`] is what a musician may switch to; switching is just a different
//! [`super::voice::NoteEvent::inst`] on the events it commits. A *feel* ([`super::feel`]) plays
//! its own palette: `bossa.pulse1 = clarinet flute` (a feel's name, a dot, a channel) lists the
//! instruments the lead plays a bossa on (the first its base, the others alternates); a feel
//! and channel a song doesn't list plays the shared built-in set ([`super::feel::equip`]).

use std::fmt;

use super::feel::Feel;

/// Most steps in one sequence.
pub const SEQ_MAX: usize = 64;
/// No loop / release point.
const NO: u8 = u8::MAX;

/// A macro: one value per 60 Hz frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Seq {
    steps: [i16; SEQ_MAX],
    len: u8,
    /// Index the held part loops back to ([`NO`]: hold the last step).
    loop_at: u8,
    /// Index where the release part starts ([`NO`]: none).
    release: u8,
}

impl Seq {
    /// A sequence of `steps` (at most [`SEQ_MAX`], at least one), with optional loop and
    /// release points (indices into `steps`).
    pub fn new(steps: &[i16], loop_at: Option<usize>, release: Option<usize>) -> Seq {
        assert!(!steps.is_empty() && steps.len() <= SEQ_MAX);
        let mut s = [0; SEQ_MAX];
        s[..steps.len()].copy_from_slice(steps);
        Seq {
            steps: s,
            len: steps.len() as u8,
            loop_at: loop_at.map_or(NO, |l| l as u8),
            release: release.map_or(NO, |r| r as u8),
        }
    }

    pub fn steps(&self) -> &[i16] {
        &self.steps[..self.len as usize]
    }

    pub fn loop_at(&self) -> Option<usize> {
        (self.loop_at != NO).then_some(self.loop_at as usize)
    }

    pub fn release(&self) -> Option<usize> {
        (self.release != NO).then_some(self.release as usize)
    }

    /// End of the held part (the release point, or the end).
    fn held_end(&self) -> usize {
        self.release().unwrap_or(self.len as usize)
    }

    /// The value at frame `f` of a held note: loops between the loop point and the release
    /// point (or the end); without a loop it holds the held part's last step.
    #[inline]
    pub fn held(&self, f: u32) -> i16 {
        let end = self.held_end().max(1);
        let f = f as usize;
        let i = if f < end {
            f
        } else {
            match self.loop_at() {
                Some(l) if l < end => l + (f - l) % (end - l),
                _ => end - 1,
            }
        };
        self.steps[i]
    }

    /// Frames in the release part (0 without one).
    pub fn release_frames(&self) -> u32 {
        self.release().map_or(0, |r| self.len as u32 - r as u32)
    }

    /// The value at frame `f` after the note's end (in the release part); holds the last step.
    #[inline]
    pub fn released(&self, f: u32) -> i16 {
        let r = self.release().unwrap_or(self.len as usize - 1);
        self.steps[(r + f as usize).min(self.len as usize - 1)]
    }

    /// Back to text (`15 12 | 9 / 4 0`).
    fn write(&self, out: &mut String, scale: f64) {
        for (i, &v) in self.steps().iter().enumerate() {
            if self.loop_at() == Some(i) {
                out.push_str(" |");
            }
            if self.release() == Some(i) {
                out.push_str(" /");
            }
            let x = v as f64 / scale;
            if scale != 1.0 && x > 0.0 {
                out.push_str(&format!(" +{x}"));
            } else {
                out.push_str(&format!(" {x}"));
            }
        }
    }
}

/// A delayed vibrato.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vibrato {
    /// Seconds before it starts.
    pub delay: f32,
    /// Seconds it takes to reach full depth.
    pub ramp: f32,
    /// Depth as a frequency ratio (0.006 ≈ 10 cents).
    pub depth: f32,
    /// Hz.
    pub rate: f32,
}

impl Vibrato {
    /// Pulse 1's built-in vibrato.
    pub const LEGACY: Vibrato = Vibrato { delay: 0.18, ramp: 0.25, depth: 0.006, rate: 5.5 };

    /// From the text's units: frames, cents, Hz.
    pub fn from_units(delay_frames: f32, depth_cents: f32, speed_hz: f32, ramp_frames: f32) -> Vibrato {
        Vibrato { delay: delay_frames / 60.0, ramp: ramp_frames / 60.0, depth: (2f32.powf(depth_cents / 1200.0) - 1.0), rate: speed_hz }
    }
}

/// The waveform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wave {
    Pulse,
    Triangle,
}

/// A tone instrument (pulses, triangle).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tone {
    /// 0..=15 per frame, scaling the note's volume.
    pub vol: Option<Seq>,
    /// 0..=3 per frame.
    pub duty: Option<Seq>,
    /// Cents per frame.
    pub pitch: Option<Seq>,
    pub vib: Option<Vibrato>,
    /// The gentle decay (to 65%), time constant in seconds.
    pub fade: Option<f32>,
    /// `None`: the channel's own.
    pub wave: Option<Wave>,
}

impl Tone {
    /// Nothing: a plain pulse (or triangle) at the note's volume and duty.
    pub const PLAIN: Tone = Tone { vol: None, duty: None, pitch: None, vib: None, fade: None, wave: None };
    /// Pulse 1's built-in instrument.
    pub const PULSE1: Tone = Tone { vib: Some(Vibrato::LEGACY), fade: Some(0.8), ..Tone::PLAIN };
    /// Pulse 2's.
    pub const PULSE2: Tone = Tone { fade: Some(0.8), ..Tone::PLAIN };
    /// The triangle's.
    pub const TRIANGLE: Tone = Tone::PLAIN;

    /// Channel `ch`'s built-in instrument.
    pub fn builtin(ch: usize) -> Tone {
        match ch {
            0 => Tone::PULSE1,
            1 => Tone::PULSE2,
            _ => Tone::TRIANGLE,
        }
    }

    /// Does anything change per frame?
    pub fn has_macros(&self) -> bool {
        self.vol.is_some() || self.duty.is_some() || self.pitch.is_some()
    }

    /// Frames of release after the note's end.
    pub fn release_frames(&self) -> u32 {
        [self.vol, self.duty, self.pitch].iter().flatten().map(Seq::release_frames).max().unwrap_or(0)
    }
}

/// The NES noise periods (NTSC), in CPU cycles.
pub const NOISE_PERIODS: [u16; 16] = [4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068];
const CPU_HZ: f32 = 1_789_773.0;

/// The kick: a pitch-dropping triangle with a click.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Kick {
    pub base_hz: f32,
    pub sweep_hz: f32,
    pub pitch_tau: f32,
    pub amp_tau: f32,
    pub click: f32,
    pub len: f32,
}

/// The snare: noise plus a tone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Snare {
    pub noise_hz: f32,
    pub short: bool,
    pub noise_tau: f32,
    pub tone_hz: f32,
    pub tone_tau: f32,
    pub len: f32,
}

/// Hats and the crash: high-passed noise.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metal {
    pub clock_hz: f32,
    pub short: bool,
    pub tau: f32,
    pub len: f32,
}

/// A drum kit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Kit {
    pub kick: Kick,
    pub snare: Snare,
    pub hat: Metal,
    pub ohat: Metal,
    pub crash: Metal,
}

/// Longest any hit rings, in seconds (the voices' buffers).
pub const MAX_HIT_SECS: f32 = 1.2;

impl Kit {
    /// The built-in kit (what the noise channel always played).
    pub const DEFAULT: Kit = Kit {
        kick: Kick { base_hz: 48.0, sweep_hz: 130.0, pitch_tau: 0.025, amp_tau: 0.07, click: 0.3, len: 0.16 },
        snare: Snare { noise_hz: 18_000.0, short: false, noise_tau: 0.055, tone_hz: 185.0, tone_tau: 0.03, len: 0.2 },
        hat: Metal { clock_hz: 220_000.0, short: false, tau: 0.012, len: 0.05 },
        ohat: Metal { clock_hz: 220_000.0, short: false, tau: 0.07, len: 0.3 },
        crash: Metal { clock_hz: 220_000.0, short: false, tau: 0.3, len: 1.1 },
    };
}

/// One named instrument.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Def {
    Tone(Tone),
    Kit(Kit),
}

/// A song's instruments: the definitions (instrument `k` of a [`super::voice::NoteEvent`] is
/// `defs[k - 1]`; 0 is the channel's built-in) and each channel's palette.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Instruments {
    pub names: Vec<String>,
    pub defs: Vec<Def>,
    /// Per channel: instrument numbers (0 = `default`) the musician may switch between; the
    /// first is the base. Empty: only what the written part uses.
    pub palettes: [Vec<u8>; 4],
    /// Per feel ([`Feel::OTHERS`]) and channel: the instruments the band plays it on
    /// (`bossa.pulse1 = flute clarinet`); empty: the built-in ([`super::feel::equip`]).
    pub feel_palettes: [[Vec<u8>; 4]; 4],
    /// The players' extra feel sounds ([`super::feel::Extra`]), once equipped.
    pub feel_extras: [u8; 5],
}

/// The instrument name that means "the channel's built-in".
pub const DEFAULT_NAME: &str = "default";
/// Most instruments in a song (numbers are a `u8`; 0 is the built-in).
pub const MAX_INSTRUMENTS: usize = 64;

/// Keywords of a tone instrument.
pub const TONE_KEYS: [&str; 7] = ["vol", "duty", "pitch", "arp", "vib", "fade", "tri"];
/// Drums of a kit.
pub const KIT_DRUMS: [&str; 5] = ["kick", "snare", "hat", "ohat", "crash"];
/// Every `key=` of a kit or a vibrato.
pub const PARAMS: [&str; 13] =
    ["delay=", "depth=", "speed=", "ramp=", "pitch=", "hz=", "sweep=", "decay=", "click=", "noise=", "period=", "tone=", "body="];

impl Instruments {
    /// The number of a name (`default` is 0).
    pub fn index(&self, name: &str) -> Option<u8> {
        if name == DEFAULT_NAME {
            return Some(0);
        }
        self.names.iter().position(|n| n == name).map(|i| i as u8 + 1)
    }

    /// The name of a number.
    pub fn name(&self, inst: u8) -> &str {
        if inst == 0 { DEFAULT_NAME } else { self.names.get(inst as usize - 1).map_or("?", String::as_str) }
    }

    /// Is instrument `inst` a kit?
    pub fn is_kit(&self, inst: u8) -> bool {
        inst > 0 && matches!(self.defs.get(inst as usize - 1), Some(Def::Kit(_)))
    }

    /// The tone instrument channel `ch` plays as `inst` (the built-in if `inst` is 0 or not a
    /// tone).
    pub fn tone(&self, ch: usize, inst: u8) -> Tone {
        match self.defs.get((inst as usize).wrapping_sub(1)) {
            Some(Def::Tone(t)) if inst > 0 => *t,
            _ => Tone::builtin(ch),
        }
    }

    /// The kit `inst` (the built-in if 0 or not a kit).
    pub fn kit(&self, inst: u8) -> Kit {
        match self.defs.get((inst as usize).wrapping_sub(1)) {
            Some(Def::Kit(k)) if inst > 0 => *k,
            _ => Kit::DEFAULT,
        }
    }

    /// Channel `ch`'s palette.
    pub fn palette(&self, ch: usize) -> &[u8] {
        &self.palettes[ch]
    }

    /// Channel `ch`'s palette in `feel` (empty for the tune's own, or before
    /// [`super::feel::equip`] for a feel the song doesn't list).
    pub fn feel_palette(&self, feel: Feel, ch: usize) -> &[u8] {
        feel.other_index().map_or(&[], |f| &self.feel_palettes[f][ch])
    }

    /// Names and kinds for the MML parser: `(name, is a kit)`.
    pub fn for_mml(&self) -> Vec<(&str, bool)> {
        self.names.iter().zip(&self.defs).map(|(n, d)| (n.as_str(), matches!(d, Def::Kit(_)))).collect()
    }

    /// Parse an `[instruments]` body. Errors carry the 1-based line within the body.
    pub fn parse(body: &str) -> Result<Instruments, (usize, String)> {
        let mut out = Instruments::default();
        let mut palettes: Vec<(usize, Option<usize>, usize, Vec<String>)> = Vec::new();
        for (k, raw) in body.lines().enumerate() {
            let line_no = k + 1;
            let line = raw.split_once(';').map_or(raw, |(a, _)| a).trim();
            if line.is_empty() {
                continue;
            }
            let colon = line.find(':');
            let eq = line.find('=');
            match (colon, eq) {
                (Some(c), e) if e.is_none_or(|e| c < e) => {
                    let (name, rest) = (line[..c].trim(), &line[c + 1..]);
                    check_name(name).map_err(|m| (line_no, m))?;
                    if out.index(name).is_some() {
                        return Err((line_no, format!("instrument `{name}` is defined twice")));
                    }
                    if out.names.len() >= MAX_INSTRUMENTS {
                        return Err((line_no, format!("at most {MAX_INSTRUMENTS} instruments")));
                    }
                    let def = parse_def(rest).map_err(|m| (line_no, format!("`{name}`: {m}")))?;
                    out.names.push(name.to_string());
                    out.defs.push(def);
                }
                (_, Some(e)) => {
                    let lhs = line[..e].trim();
                    // `bossa.pulse1 = ...`: a feel's palette.
                    let (feel, ch) = match lhs.split_once('.') {
                        Some((f, c)) => {
                            let feel = Feel::parse(f.trim()).and_then(Feel::other_index).ok_or_else(|| {
                                (line_no, format!("`{lhs} = ...`: unknown feel `{}` (bossa, samba, rock, funk)", f.trim()))
                            })?;
                            (Some(feel), c.trim())
                        }
                        None => (None, lhs),
                    };
                    let Some(i) = super::song::CHANNELS.iter().position(|(n, _)| *n == ch) else {
                        return Err((line_no, format!("`{lhs} = ...`: a palette is for a channel (pulse1, pulse2, triangle, noise)")));
                    };
                    if palettes.iter().any(|p| p.1 == feel && p.2 == i) {
                        return Err((line_no, format!("`{lhs}`'s palette is given twice")));
                    }
                    let names = line[e + 1..].split([' ', '\t', ',']).filter(|w| !w.is_empty()).map(str::to_string).collect();
                    palettes.push((line_no, feel, i, names));
                }
                _ => return Err((line_no, format!("expected `name : macros` or `channel = names`, found `{line}`"))),
            }
        }
        for (line_no, feel, ch, names) in palettes {
            for n in names {
                let Some(i) = out.index(&n) else {
                    return Err((line_no, format!("unknown instrument `{n}` in the palette")));
                };
                let kit = out.is_kit(i);
                if kit != (ch == 3) && i != 0 {
                    let what = if kit { "a kit is for the noise channel" } else { "the noise channel takes kits" };
                    return Err((line_no, format!("`{n}`: {what}")));
                }
                let p = match feel {
                    Some(f) => &mut out.feel_palettes[f][ch],
                    None => &mut out.palettes[ch],
                };
                if !p.contains(&i) {
                    p.push(i);
                }
            }
        }
        Ok(out)
    }

    /// One instrument as text (canonical), e.g. for the editor after a change.
    pub fn def_text(def: &Def) -> String {
        let mut s = String::new();
        match def {
            Def::Tone(t) => {
                let mut parts: Vec<String> = Vec::new();
                if t.wave == Some(Wave::Triangle) {
                    parts.push("tri".into());
                }
                for (key, seq, scale) in [("vol", t.vol, 1.0), ("duty", t.duty, 1.0), ("pitch", t.pitch, 100.0)] {
                    if let Some(q) = seq {
                        let mut p = key.to_string();
                        q.write(&mut p, scale);
                        parts.push(p);
                    }
                }
                if let Some(v) = t.vib {
                    let cents = 1200.0 * (1.0 + v.depth as f64).log2();
                    parts.push(format!(
                        "vib delay={} depth={} speed={} ramp={}",
                        round2(v.delay as f64 * 60.0),
                        round2(cents),
                        round2(v.rate as f64),
                        round2(v.ramp as f64 * 60.0)
                    ));
                }
                if let Some(f) = t.fade {
                    parts.push(format!("fade {}", round2(f as f64)));
                }
                s += &parts.join(" | ");
            }
            Def::Kit(k) => {
                let d = Kit::DEFAULT;
                let fr = |x: f32| round2(x as f64 * 60.0);
                let mut parts = Vec::new();
                let mut p = "kick".to_string();
                if k.kick.sweep_hz != d.kick.sweep_hz || k.kick.base_hz != d.kick.base_hz {
                    let semis = 12.0 * ((k.kick.base_hz + k.kick.sweep_hz) as f64 / k.kick.base_hz as f64).log2();
                    p += &format!(" pitch=-{}", round2(semis));
                }
                if k.kick.base_hz != d.kick.base_hz {
                    p += &format!(" hz={}", round2(k.kick.base_hz as f64));
                }
                if k.kick.pitch_tau != d.kick.pitch_tau {
                    p += &format!(" sweep={}", fr(k.kick.pitch_tau));
                }
                if k.kick.amp_tau != d.kick.amp_tau {
                    p += &format!(" decay={}", fr(k.kick.amp_tau));
                }
                if k.kick.click != d.kick.click {
                    p += &format!(" click={}", round2(k.kick.click as f64));
                }
                parts.push(p);
                let mut p = "snare".to_string();
                if k.snare.short {
                    p += " noise=short";
                }
                if k.snare.noise_hz != d.snare.noise_hz {
                    p += &format!(" period={}", period_of(k.snare.noise_hz));
                }
                if k.snare.noise_tau != d.snare.noise_tau {
                    p += &format!(" decay={}", fr(k.snare.noise_tau));
                }
                if k.snare.tone_hz != d.snare.tone_hz {
                    p += &format!(" tone={}", round2(k.snare.tone_hz as f64));
                }
                if k.snare.tone_tau != d.snare.tone_tau {
                    p += &format!(" body={}", fr(k.snare.tone_tau));
                }
                parts.push(p);
                for (name, m, dm) in [("hat", k.hat, d.hat), ("ohat", k.ohat, d.ohat), ("crash", k.crash, d.crash)] {
                    let mut p = name.to_string();
                    if m.short {
                        p += " noise=short";
                    }
                    if m.clock_hz != dm.clock_hz {
                        p += &format!(" period={}", period_of(m.clock_hz));
                    }
                    if m.tau != dm.tau {
                        p += &format!(" decay={}", fr(m.tau));
                    }
                    parts.push(p);
                }
                parts.retain(|p| p.contains('='));
                s += &parts.join(" | ");
                if s.is_empty() {
                    s += "kick";
                }
            }
        }
        s
    }
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

fn period_of(clock: f32) -> usize {
    NOISE_PERIODS.iter().position(|&p| (CPU_HZ / p as f32 - clock).abs() < 1.0).unwrap_or(1)
}

fn check_name(name: &str) -> Result<(), String> {
    let ok = name.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !ok {
        return Err(format!("`{name}` isn't a name (letters, digits and `_`, starting with a letter)"));
    }
    if name == DEFAULT_NAME {
        return Err("`default` is the built-in instrument's name".into());
    }
    if super::song::CHANNELS.iter().any(|(n, _)| *n == name) {
        return Err(format!("`{name}` is a channel's name (palettes are `{name} = ...`)"));
    }
    Ok(())
}

/// Words of a definition, with `|` kept as its own word.
fn words(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for w in s.split([' ', '\t', ',']).filter(|w| !w.is_empty()) {
        // `15|12` or `|12`: split the bars and slashes off.
        let mut rest = w;
        while !rest.is_empty() {
            match rest.find(['|', '/']) {
                Some(0) => {
                    out.push(&rest[..1]);
                    rest = &rest[1..];
                }
                Some(i) => {
                    out.push(&rest[..i]);
                    rest = &rest[i..];
                }
                None => {
                    out.push(rest);
                    rest = "";
                }
            }
        }
    }
    out
}

fn parse_def(rest: &str) -> Result<Def, String> {
    let w = words(rest);
    let first = w.iter().find(|x| **x != "|").copied().unwrap_or("");
    if KIT_DRUMS.contains(&first) {
        parse_kit(&w).map(Def::Kit)
    } else {
        parse_tone(&w).map(Def::Tone)
    }
}

fn number(w: &str) -> Option<f64> {
    w.strip_prefix('+').unwrap_or(w).parse::<f64>().ok().filter(|x| x.is_finite())
}

fn parse_tone(w: &[&str]) -> Result<Tone, String> {
    let mut t = Tone::PLAIN;
    let mut i = 0;
    if w.is_empty() {
        return Err("an empty instrument (try `vol 15 12 9`)".into());
    }
    while i < w.len() {
        let key = w[i];
        i += 1;
        match key {
            "|" => continue,
            "tri" => t.wave = Some(Wave::Triangle),
            "vol" | "duty" | "pitch" | "arp" => {
                let (mut steps, mut loop_at, mut release) = (Vec::new(), None, None);
                while i < w.len() {
                    match w[i] {
                        "|" => {
                            // A loop point only if a number follows.
                            if w.get(i + 1).and_then(|x| number(x)).is_none() && w.get(i + 1) != Some(&"/") {
                                break;
                            }
                            if loop_at.is_some() {
                                return Err(format!("`{key}` has two loop points `|`"));
                            }
                            loop_at = Some(steps.len());
                        }
                        "/" => {
                            if release.is_some() {
                                return Err(format!("`{key}` has two release points `/`"));
                            }
                            release = Some(steps.len());
                        }
                        x => match number(x) {
                            Some(v) => {
                                let (lo, hi, scale) = match key {
                                    "vol" => (0.0, 15.0, 1.0),
                                    "duty" => (0.0, 3.0, 1.0),
                                    _ => (-48.0, 48.0, 100.0),
                                };
                                if !(lo..=hi).contains(&v) {
                                    return Err(format!("`{key}` steps are {lo}..={hi}, found {x}"));
                                }
                                if scale == 1.0 && v.fract() != 0.0 {
                                    return Err(format!("`{key}` steps are whole numbers, found {x}"));
                                }
                                steps.push((v * scale).round() as i16);
                            }
                            None => break,
                        },
                    }
                    i += 1;
                }
                if steps.is_empty() {
                    return Err(format!("`{key}` needs steps, e.g. `{key} {}`", if key == "vol" { "15 12 9" } else { "0 1 2" }));
                }
                if steps.len() > SEQ_MAX {
                    return Err(format!("`{key}` has {} steps (at most {SEQ_MAX})", steps.len()));
                }
                if let Some(r) = release {
                    if r >= steps.len() {
                        return Err(format!("`{key}`: the release `/` needs steps after it"));
                    }
                    if loop_at.is_some_and(|l| l >= r) {
                        return Err(format!("`{key}`: the loop `|` must come before the release `/`"));
                    }
                }
                if loop_at.is_some_and(|l| l >= steps.len()) {
                    return Err(format!("`{key}`: nothing to loop after `|`"));
                }
                let seq = Some(Seq::new(&steps, loop_at, release));
                match key {
                    "vol" => t.vol = seq,
                    "duty" => t.duty = seq,
                    _ => t.pitch = seq,
                }
            }
            "vib" => {
                let (mut delay, mut depth, mut speed, mut ramp) = (11.0f32, 10.0f32, 5.5f32, 15.0f32);
                while let Some((k, v)) = w.get(i).and_then(|x| x.split_once('=')) {
                    let x = number(v).ok_or_else(|| format!("`vib {k}=`: `{v}` isn't a number"))? as f32;
                    match k {
                        "delay" if (0.0..=600.0).contains(&x) => delay = x,
                        "depth" if (0.0..=1200.0).contains(&x) => depth = x,
                        "speed" if (0.0..=30.0).contains(&x) => speed = x,
                        "ramp" if (0.0..=600.0).contains(&x) => ramp = x,
                        "delay" | "depth" | "speed" | "ramp" => return Err(format!("`vib {k}={v}` out of range")),
                        _ => return Err(format!("`vib` takes delay= depth= speed= ramp=, not `{k}=`")),
                    }
                    i += 1;
                }
                t.vib = Some(Vibrato::from_units(delay, depth, speed, ramp.max(0.01)));
            }
            "fade" => {
                let x = w.get(i).and_then(|x| number(x)).ok_or("`fade` needs seconds, e.g. `fade 0.8`")?;
                if !(0.0..=60.0).contains(&x) {
                    return Err("`fade` is 0..60 seconds".into());
                }
                i += 1;
                t.fade = (x > 0.0).then_some(x as f32);
            }
            k if KIT_DRUMS.contains(&k) => return Err(format!("`{k}` belongs in a kit (a kit is drums only)")),
            k => return Err(format!("unknown `{k}` (expected one of {})", TONE_KEYS.join(" "))),
        }
    }
    Ok(t)
}

fn parse_kit(w: &[&str]) -> Result<Kit, String> {
    let mut kit = Kit::DEFAULT;
    let d = Kit::DEFAULT;
    let mut i = 0;
    let frames = |x: f64| (x / 60.0) as f32;
    while i < w.len() {
        let drum = w[i];
        i += 1;
        if drum == "|" {
            continue;
        }
        if !KIT_DRUMS.contains(&drum) {
            return Err(format!("unknown drum `{drum}` (a kit has {})", KIT_DRUMS.join(" ")));
        }
        while let Some((k, v)) = w.get(i).and_then(|x| x.split_once('=')) {
            i += 1;
            let bad = || format!("`{drum} {k}={v}`");
            if k == "noise" {
                let short = match v {
                    "short" => true,
                    "long" => false,
                    _ => return Err(format!("{}: noise is `short` or `long`", bad())),
                };
                match drum {
                    "snare" => kit.snare.short = short,
                    "hat" => kit.hat.short = short,
                    "ohat" => kit.ohat.short = short,
                    "crash" => kit.crash.short = short,
                    _ => return Err(format!("{}: the kick has no noise", bad())),
                }
                continue;
            }
            let x = number(v).ok_or_else(|| format!("{}: not a number", bad()))?;
            let positive = |lo: f64, hi: f64| if (lo..=hi).contains(&x) { Ok(()) } else { Err(format!("{}: out of range ({lo}..={hi})", bad())) };
            match (drum, k) {
                ("kick", "pitch") => {
                    positive(-60.0, 0.0)?;
                    kit.kick.sweep_hz = kit.kick.base_hz * (2f32.powf((-x) as f32 / 12.0) - 1.0);
                }
                ("kick", "hz") => {
                    positive(20.0, 400.0)?;
                    let semis = (1.0 + kit.kick.sweep_hz / kit.kick.base_hz).log2();
                    kit.kick.base_hz = x as f32;
                    kit.kick.sweep_hz = x as f32 * (2f32.powf(semis) - 1.0);
                }
                ("kick", "sweep") => {
                    positive(0.1, 60.0)?;
                    kit.kick.pitch_tau = frames(x);
                }
                ("kick", "decay") => {
                    positive(0.5, 60.0)?;
                    kit.kick.amp_tau = frames(x);
                    kit.kick.len = (d.kick.len * kit.kick.amp_tau / d.kick.amp_tau).min(MAX_HIT_SECS);
                }
                ("kick", "click") => {
                    positive(0.0, 1.0)?;
                    kit.kick.click = x as f32;
                }
                ("snare", "decay") => {
                    positive(0.5, 60.0)?;
                    kit.snare.noise_tau = frames(x);
                    kit.snare.len = (d.snare.len * kit.snare.noise_tau / d.snare.noise_tau).min(MAX_HIT_SECS);
                }
                ("snare", "tone") => {
                    positive(40.0, 2000.0)?;
                    kit.snare.tone_hz = x as f32;
                }
                ("snare", "body") => {
                    positive(0.1, 60.0)?;
                    kit.snare.tone_tau = frames(x);
                }
                ("snare", "period") => {
                    positive(0.0, 15.0)?;
                    kit.snare.noise_hz = CPU_HZ / NOISE_PERIODS[x as usize] as f32;
                }
                ("hat" | "ohat" | "crash", "decay" | "period") => {
                    let (m, dm) = match drum {
                        "hat" => (&mut kit.hat, d.hat),
                        "ohat" => (&mut kit.ohat, d.ohat),
                        _ => (&mut kit.crash, d.crash),
                    };
                    if k == "decay" {
                        positive(0.1, 60.0)?;
                        m.tau = frames(x);
                        m.len = (dm.len * m.tau / dm.tau).min(MAX_HIT_SECS);
                    } else {
                        positive(0.0, 15.0)?;
                        m.clock_hz = CPU_HZ / NOISE_PERIODS[x as usize] as f32;
                    }
                }
                _ => return Err(format!("{}: `{drum}` has no `{k}=`", bad())),
            }
        }
        if w.get(i).is_some_and(|x| *x != "|" && !KIT_DRUMS.contains(x)) {
            return Err(format!("`{drum}`: expected `key=value`, found `{}`", w[i]));
        }
    }
    Ok(kit)
}

impl fmt::Display for Def {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&Instruments::def_text(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(s: &str) -> Tone {
        match parse_def(s).unwrap() {
            Def::Tone(t) => t,
            d => panic!("{d:?}"),
        }
    }

    #[test]
    fn sequences_loop_and_release() {
        let t = tone("vol 15 12 | 9 8 / 4 0 | duty 2");
        let v = t.vol.unwrap();
        assert_eq!(v.steps(), [15, 12, 9, 8, 4, 0]);
        assert_eq!((v.loop_at(), v.release()), (Some(2), Some(4)));
        let held: Vec<i16> = (0..8).map(|f| v.held(f)).collect();
        assert_eq!(held, [15, 12, 9, 8, 9, 8, 9, 8]);
        assert_eq!((v.released(0), v.released(1), v.released(5)), (4, 0, 0));
        assert_eq!(v.release_frames(), 2);
        assert_eq!(t.duty.unwrap().steps(), [2]);
        // No loop: hold the last.
        let p = tone("pitch +3 +1 0").pitch.unwrap();
        assert_eq!((0..5).map(|f| p.held(f)).collect::<Vec<_>>(), [300, 100, 0, 0, 0]);
        // An arpeggio loops.
        let a = tone("arp | 0 4 7").pitch.unwrap();
        assert_eq!((0..5).map(|f| a.held(f)).collect::<Vec<_>>(), [0, 400, 700, 0, 400]);
        assert_eq!(tone("tri vol 8").wave, Some(Wave::Triangle));
    }

    #[test]
    fn the_examples_parse() {
        let body = "lead  : vol 15 14 12 10 9 8 | duty 2 | vib delay=8 depth=3 speed=5\n\
                    pluck : vol 15 9 6 4 2 0 | duty 1 1 2\n\
                    brass : vol 8 12 15 15 14 | duty 0 1 2 | pitch +3 +1 0\n\
                    kit   : kick pitch=-24 decay=6 | snare noise=short decay=8 | hat decay=2\n\
                    pulse1 = default lead brass pluck ; the palette\n\
                    noise = kit default\n";
        let i = Instruments::parse(body).unwrap();
        assert_eq!(i.names, ["lead", "pluck", "brass", "kit"]);
        assert_eq!(i.palettes[0], [0, 1, 3, 2]);
        assert_eq!(i.palettes[3], [4, 0]);
        let Def::Tone(lead) = i.defs[0] else { panic!() };
        assert_eq!(lead.vol.unwrap().steps(), [15, 14, 12, 10, 9, 8]);
        assert_eq!(lead.vol.unwrap().loop_at(), None);
        let v = lead.vib.unwrap();
        assert!((v.delay - 8.0 / 60.0).abs() < 1e-6 && v.rate == 5.0);
        let Def::Kit(k) = i.defs[3] else { panic!() };
        assert!(k.snare.short && (k.kick.amp_tau - 0.1).abs() < 1e-6);
        assert!((k.kick.base_hz + k.kick.sweep_hz - 192.0).abs() < 0.01);
        assert_eq!(k.ohat, Kit::DEFAULT.ohat);
        assert_eq!(i.index("default"), Some(0));
        assert!(i.is_kit(4) && !i.is_kit(1) && !i.is_kit(0));
    }

    #[test]
    fn errors_say_what_and_where() {
        let e = |body: &str| Instruments::parse(body).unwrap_err();
        assert_eq!(e("a : vol 1\nb : vol 16\n").0, 2);
        assert!(e("a : vol 16").1.contains("0..=15"));
        assert!(e("a : duty 4").1.contains("0..=3"));
        assert!(e("a : wobble 3").1.contains("unknown `wobble`"));
        assert!(e("a : vol").1.contains("needs steps"));
        assert!(e("a : vol 1\na : vol 2").1.contains("twice"));
        assert!(e("default : vol 1").1.contains("built-in"));
        assert!(e("pulse1 : vol 1").1.contains("channel"));
        assert!(e("9x : vol 1").1.contains("isn't a name"));
        assert!(e("a : vol 1 / ").1.contains("release"));
        assert!(e("a : vol 1 | 2 | 3").1.contains("two loop"));
        assert!(e("a : vib delay=x").1.contains("number"));
        assert!(e("a : vib wobble=3").1.contains("not `wobble=`"));
        assert!(e("k : kick pitch=5").1.contains("out of range"));
        assert!(e("k : kick noise=short").1.contains("no noise"));
        assert!(e("k : snare noise=loud").1.contains("short"));
        assert!(e("k : tom decay=3").1.contains("unknown"));
        assert!(e("k : kick decay=3 | vol 3").1.contains("unknown drum"));
        assert!(e("a : vol 3 kick").1.contains("kit"));
        assert!(e("a : vol 3\npulse1 = a b").1.contains("unknown instrument `b`"));
        assert!(e("k : kick decay=3\npulse1 = k").1.contains("noise channel"));
        assert!(e("a : vol 3\nnoise = a").1.contains("takes kits"));
        assert!(e("drums = a").1.contains("channel"));
        assert!(e("just words").1.contains("expected"));
        assert!(e(&format!("a : vol {}", "1 ".repeat(65))).1.contains("at most"));
    }

    #[test]
    fn text_round_trips() {
        let body = "a : tri | vol 15 12 | 9 / 4 0 | duty 0 1 | pitch +0.5 -1 0 | vib delay=8 depth=12 speed=6 ramp=10 | fade 0.8\n\
                    k : kick pitch=-24 decay=6 click=0.5 | snare noise=short period=6 decay=8 tone=200 body=3 | hat decay=2 period=0 | ohat noise=short | crash decay=30\n";
        let i = Instruments::parse(body).unwrap();
        for (name, def) in i.names.iter().zip(&i.defs) {
            let again = Instruments::parse(&format!("{name} : {def}")).unwrap();
            let d = again.defs[0];
            match (def, d) {
                (Def::Tone(a), Def::Tone(b)) => {
                    assert_eq!((a.vol, a.duty, a.pitch, a.wave, a.fade), (b.vol, b.duty, b.pitch, b.wave, b.fade), "{def}");
                    let (va, vb) = (a.vib.unwrap(), b.vib.unwrap());
                    assert!((va.depth - vb.depth).abs() < 1e-5 && (va.delay - vb.delay).abs() < 1e-5, "{def}");
                }
                (Def::Kit(a), Def::Kit(b)) => {
                    assert_eq!((a.snare.short, a.ohat.short, a.hat.clock_hz), (b.snare.short, b.ohat.short, b.hat.clock_hz), "{def}");
                    assert!((a.kick.sweep_hz - b.kick.sweep_hz).abs() < 0.05 && (a.crash.tau - b.crash.tau).abs() < 1e-5, "{def}");
                }
                _ => panic!("{def}"),
            }
        }
    }
}
