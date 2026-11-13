//! The horns: a big band's sections, each player on a channel of their own ([`FIRST_CH`] on),
//! written over the band as each bar is committed. Music only: no physics, no director. Off
//! unless [`super::EngineConfig::horns`] puts someone on the stand (Nat Han stays a combo).
//!
//! So far the low end: the bass trombone, the bari and the tuba, the horns that share the
//! bass's register (down to low D), which is where a stem separator most often mistakes a horn
//! for the bass. Each 8-bar section of a pass gives them a [`LowRole`], rolled from the seed and
//! weighted by the chorus (a shout doubles the line, stop-time hits ONE, a blowing chorus
//! leaves room), or forced ([`super::Input::ForceLowEnd`]):
//!
//! | Role | What they play |
//! |---|---|
//! | Unison | the bass line, note for note (an octave up where it goes under a horn's range) |
//! | Octave | the bass line, one horn at pitch, another an octave up |
//! | Pedal | one note (the tonic, or the dominant into the turnaround) held across the changes |
//! | Pads | root-position long notes, a chord change each |
//! | Hits | a "bwaah" on ONE, short or held, and the band's hits |
//! | Tacet | nothing: the bass alone |
//! | Alone | the bass line, and the bass lays out: the horns are the only low end |
//!
//! The rules are the arranger's usual ones: root position in the low horns (the bari a fifth or
//! an octave over the root at most), every note in the horn's range, nothing during a break.

use super::band::{BandPlan, HitKind};
use super::chorus::Chorus;
use super::musician::Ctx;
use super::voice::{NoteEvent, Sound};

/// The first horn's channel (0-4 are the combo's: lead, comp, bass, drums, the soli's tenor).
pub const FIRST_CH: u8 = 5;

/// A horn player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Horn {
    /// The fourth trombone, a bass trombone.
    BassTrombone,
    Bari,
    /// For older two-beat charts.
    Tuba,
}

impl Horn {
    pub const ALL: [Horn; 3] = [Horn::BassTrombone, Horn::Bari, Horn::Tuba];

    pub fn ch(self) -> u8 {
        FIRST_CH + self as u8
    }

    pub fn from_ch(ch: u8) -> Option<Horn> {
        Horn::ALL.get(ch.checked_sub(FIRST_CH)? as usize).copied()
    }

    /// Sounding range, MIDI (the low notes are a bass trombone's pedal register and a low-A
    /// bari's bottom).
    pub fn range(self) -> (i32, i32) {
        match self {
            Horn::BassTrombone => (28, 65),
            Horn::Bari => (36, 69),
            Horn::Tuba => (26, 58),
        }
    }

    /// Where its root-position notes sit, lowest MIDI note of a twelve-note window.
    fn home(self) -> i32 {
        match self {
            Horn::BassTrombone => 33,
            Horn::Bari => 41,
            Horn::Tuba => 26,
        }
    }

    /// Its General MIDI program (0-based: GM has no bass trombone, so the trombone).
    pub fn program(self) -> u8 {
        match self {
            Horn::BassTrombone => 57,
            Horn::Bari => 67,
            Horn::Tuba => 58,
        }
    }

    /// File names and labels: `bone4`, `bari`, `tuba`.
    pub fn slug(self) -> &'static str {
        match self {
            Horn::BassTrombone => "bone4",
            Horn::Bari => "bari",
            Horn::Tuba => "tuba",
        }
    }

    pub fn parse(s: &str) -> Option<Horn> {
        Horn::ALL.into_iter().find(|h| h.slug() == s)
    }

    /// `n` moved by octaves into the horn's range.
    fn fit(self, n: i32) -> i32 {
        let (lo, hi) = self.range();
        let mut n = n;
        while n < lo {
            n += 12;
        }
        while n > hi {
            n -= 12;
        }
        n
    }
}

/// Who's on the stand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Horns(u8);

impl Horns {
    pub const NONE: Horns = Horns(0);
    /// The bass trombone and the bari.
    pub const LOW: Horns = Horns(0b011);
    /// The two and a tuba.
    pub const ALL: Horns = Horns(0b111);

    pub fn with(self, h: Horn) -> Horns {
        Horns(self.0 | 1 << h as u8)
    }

    pub fn has(self, h: Horn) -> bool {
        self.0 & 1 << h as u8 != 0
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn iter(self) -> impl Iterator<Item = Horn> {
        Horn::ALL.into_iter().filter(move |&h| self.has(h))
    }

    /// `bone4,bari` (empty: none).
    pub fn parse(s: &str) -> Option<Horns> {
        s.split(',').filter(|w| !w.is_empty()).try_fold(Horns::NONE, |hs, w| Horn::parse(w.trim()).map(|h| hs.with(h)))
    }

    pub fn slugs(self) -> String {
        self.iter().map(Horn::slug).collect::<Vec<_>>().join(",")
    }
}

/// What the low horns do in a section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LowRole {
    Tacet,
    Unison,
    Octave,
    Pedal,
    Pads,
    Hits,
    /// The horns take the bass line; the bass lays out.
    Alone,
}

impl LowRole {
    pub const ALL: [LowRole; 7] = [LowRole::Tacet, LowRole::Unison, LowRole::Octave, LowRole::Pedal, LowRole::Pads, LowRole::Hits, LowRole::Alone];

    pub fn slug(self) -> &'static str {
        match self {
            LowRole::Tacet => "tacet",
            LowRole::Unison => "unison",
            LowRole::Octave => "octave",
            LowRole::Pedal => "pedal",
            LowRole::Pads => "pads",
            LowRole::Hits => "hits",
            LowRole::Alone => "alone",
        }
    }

    pub fn parse(s: &str) -> Option<LowRole> {
        LowRole::ALL.into_iter().find(|r| r.slug() == s)
    }

    /// Weights (in [`LowRole::ALL`] order) for a section of `chorus`.
    fn weights(chorus: Chorus) -> [u32; 7] {
        match chorus {
            Chorus::Shout => [0, 30, 30, 0, 10, 20, 10],
            Chorus::Soli => [5, 35, 25, 0, 15, 10, 10],
            Chorus::StopTime => [10, 0, 0, 15, 5, 70, 0],
            Chorus::Breaks => [10, 25, 20, 0, 20, 20, 5],
            Chorus::Head | Chorus::TwoFeel => [10, 15, 10, 15, 20, 20, 10],
            _ => [30, 10, 5, 10, 25, 20, 0],
        }
    }
}

/// Bars in a section (a role holds this long).
const SECTION: usize = 8;

/// The role for the section `ctx`'s bar is in.
pub fn role(ctx: &Ctx, force: Option<LowRole>) -> LowRole {
    if let Some(r) = force {
        return r;
    }
    let band = ctx.band;
    if band.intro.is_some() {
        let x = unit(ctx.seed, ctx.bar.pass, 0, 1);
        return if x < 0.4 {
            LowRole::Pedal
        } else if x < 0.65 {
            LowRole::Hits
        } else {
            LowRole::Alone
        };
    }
    if band.ending.is_some() {
        return LowRole::Hits;
    }
    let section = (ctx.bar.song_bar / SECTION) as u64;
    let w = LowRole::weights(band.chorus);
    let total: u32 = w.iter().sum();
    let mut x = (unit(ctx.seed, ctx.bar.pass, section, 2) * total as f64) as u32;
    for (r, &k) in LowRole::ALL.iter().zip(&w) {
        if x < k {
            return *r;
        }
        x -= k;
    }
    LowRole::Tacet
}

/// Write the horns' notes for `ctx`'s bar into `out`, from the band's events in it (`band`: the
/// bass line they double; [`LowRole::Alone`] takes the bass out of it). Returns the low end's
/// role.
pub fn write(ctx: &Ctx, horns: Horns, force: Option<LowRole>, band: &mut Vec<NoteEvent>, out: &mut Vec<NoteEvent>) -> Option<LowRole> {
    if horns.is_empty() {
        return None;
    }
    let role = role(ctx, force);
    let plan: &BandPlan = ctx.band;
    // A break, the tacet bars: the band lays out, the horns with it.
    if plan.tacet || plan.solo_break || role == LowRole::Tacet {
        return Some(role);
    }
    let bass: Vec<&NoteEvent> = band.iter().filter(|e| e.ch == 2 && matches!(e.sound, Sound::Note(_)) && e.volume >= 6).collect();
    let mut t = ctx.template(2, bass.first().map(|e| **e));
    t.inst = 0;
    t.anchor = 0;
    let bb = ctx.bb();
    // The two that split the octave (the first at pitch, the second an octave up).
    let up = unit(ctx.seed, ctx.bar.pass, (ctx.bar.song_bar / SECTION) as u64, 3) < 0.5;
    for (k, h) in horns.iter().enumerate() {
        let ch = h.ch();
        let at = |e: NoteEvent| NoteEvent { ch, inst: 0, anchor: 0, ..e };
        match role {
            LowRole::Unison | LowRole::Octave | LowRole::Alone => {
                let octave = role == LowRole::Octave && h != Horn::Tuba && ((k % 2 == 1) ^ up);
                for e in &bass {
                    let Sound::Note(n) = e.sound else { continue };
                    let n = n as i32 + if octave { 12 } else { 0 };
                    push(out, NoteEvent { sound: Sound::Note(h.fit(n) as u8), volume: e.volume.max(11), ..at(**e) });
                }
            }
            LowRole::Pedal => {
                // The dominant into the section's last two bars and the intro, else the tonic.
                let last = ctx.bar.song_bar % SECTION >= SECTION - 2 || plan.intro.is_some();
                let pc = (ctx.shape.key as i32 + if last { 7 } else { 0 }) % 12;
                let n = h.fit(h.home() + (pc - h.home()).rem_euclid(12));
                // Held across bar lines, struck again every second bar.
                let tie = ctx.bar.song_bar % 2 == 1;
                push(out, NoteEvent { tie, volume: 10, ..at(ctx.make(&t, 0.0, bb, Sound::Note(n as u8))) });
            }
            LowRole::Pads => {
                for (b, d, root, fifth) in changes(ctx) {
                    let n = if h == Horn::Bari && fifth { root + 7 } else { root };
                    let n = h.fit(h.home() + (n - h.home()).rem_euclid(12));
                    push(out, NoteEvent { volume: 9, ..at(ctx.make(&t, b, (d - 0.15).max(0.4), Sound::Note(n as u8))) });
                }
            }
            LowRole::Hits => {
                let mut hits: Vec<(f64, f64)> = Vec::new();
                let short = plan.hit_kind == HitKind::Stop || unit(ctx.seed, ctx.bar.pass, ctx.bar.song_bar as u64, 4) < 0.5;
                // The bwaah on ONE (every bar in stop-time, else every other bar or on a change).
                let one = plan.hit_kind == HitKind::Stop || ctx.bar.song_bar % 2 == 0 || unit(ctx.seed, ctx.bar.index, 0, 5) < 0.3;
                if one && !plan.anticipated {
                    hits.push((0.0, if short { 0.5 } else { (bb - 1.0).max(1.0) }));
                }
                for k in 0..16 {
                    if plan.hits & 1 << k != 0 {
                        let b = k as f64 / 4.0;
                        if b < bb && hits.iter().all(|&(x, d)| b >= x + d.min(0.5)) {
                            hits.push((b, 0.5));
                        }
                    }
                }
                for (b, d) in hits {
                    let Some(h0) = ctx.harm_at(b) else { continue };
                    let root = h0.chord.bass.unwrap_or(h0.chord.root) as i32;
                    let n = h.fit(h.home() + (root - h.home()).rem_euclid(12));
                    push(out, NoteEvent { volume: 15, ..at(ctx.make(&t, b, d, Sound::Note(n as u8))) });
                }
            }
            LowRole::Tacet => {}
        }
    }
    if role == LowRole::Alone {
        band.retain(|e| e.ch != 2);
    }
    Some(role)
}

/// The bar's chord changes: (beat, beats, root pitch class, the bari takes the fifth).
fn changes(ctx: &Ctx) -> Vec<(f64, f64, i32, bool)> {
    let bb = ctx.bb();
    let mut out: Vec<(f64, f64, i32, bool)> = Vec::new();
    let mut b = 0.0;
    while b < bb - 1e-9 {
        if let Some(h) = ctx.harm_at(b) {
            let root = h.chord.bass.unwrap_or(h.chord.root) as i32;
            match out.last_mut() {
                Some(last) if last.2 == root => last.1 += 1.0,
                _ => out.push((b, 1.0, root, unit(ctx.seed, ctx.bar.index, (b * 4.0) as u64, 6) < 0.5)),
            }
        }
        b += 1.0;
    }
    out
}

fn push(out: &mut Vec<NoteEvent>, e: NoteEvent) {
    out.push(e);
}

/// A deterministic coin in [0, 1).
fn unit(seed: u64, a: u64, b: u64, what: u64) -> f64 {
    let mut x = seed ^ 0x686f_726e_73 ^ a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ what.wrapping_mul(0x1656_67B1_9E37_79F9);
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    x ^= x >> 33;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Frame;
    use crate::live::chorus::Call;
    use crate::live::{Engine, EngineConfig, Input, library};

    fn take(horns: Horns, inputs: &[Input], song: &str) -> Vec<NoteEvent> {
        let mut e = Engine::with_config(&library::load(song).unwrap(), 32_000, EngineConfig { seed: 5, horns, ..EngineConfig::default() }).unwrap();
        e.tap(true);
        e.post(Input::LevelStart);
        e.post(Input::SetFreedom { lead: 0.7, comp: 0.7, bass: 0.7, drums: 0.7, dynamics: 0.5 });
        for i in inputs {
            e.post(*i);
        }
        let bars = e.shape().bars as u64;
        let mut buf = [Frame::ZERO; 1024];
        let mut out = Vec::new();
        while e.beat_clock().position.bar < bars {
            e.fill(&mut buf);
            out.extend(e.take_tapped());
        }
        out
    }

    /// No horns, no change: the band plays exactly what it played before there were horns.
    #[test]
    fn no_horns_leaves_the_band_alone() {
        let a = take(Horns::NONE, &[], "sweet_georgia_brown");
        assert!(a.iter().all(|e| e.ch < FIRST_CH));
        // (Any role but Alone, which takes the bass out.)
        let b = take(Horns::ALL, &[Input::ForceLowEnd(Some(LowRole::Pads))], "sweet_georgia_brown");
        let band: Vec<_> = b.iter().filter(|e| e.ch < FIRST_CH).map(|e| (e.ch, e.start, e.end, e.sound)).collect();
        assert_eq!(a.iter().map(|e| (e.ch, e.start, e.end, e.sound)).collect::<Vec<_>>(), band);
    }

    #[test]
    fn every_horn_note_is_in_range() {
        for role in LowRole::ALL {
            for song in ["sweet_georgia_brown", "tiger_rag", "body_and_soul"] {
                for e in take(Horns::ALL, &[Input::ForceLowEnd(Some(role))], song) {
                    let Some(h) = Horn::from_ch(e.ch) else { continue };
                    let (lo, hi) = h.range();
                    for &n in e.sound.notes() {
                        assert!((lo..=hi).contains(&(n as i32)), "{song} {role:?} {h:?}: {n}");
                    }
                }
            }
        }
    }

    /// In unison the bass trombone plays the bass's notes, at the bass's times, at pitch or an
    /// octave up where the line goes under its range.
    #[test]
    fn unison_doubles_the_bass_line() {
        let evs = take(Horns::LOW, &[Input::ForceLowEnd(Some(LowRole::Unison)), Input::ForceChorus(Some(Call { chorus: Chorus::Shout, key_up: false }))], "sweet_georgia_brown");
        let bass: Vec<_> = evs.iter().filter(|e| e.ch == 2 && e.volume >= 6).collect();
        let bone: Vec<_> = evs.iter().filter(|e| e.ch == Horn::BassTrombone.ch()).collect();
        assert!(bone.len() as f64 > bass.len() as f64 * 0.9, "{} of {}", bone.len(), bass.len());
        let ms = 32.0;
        let mut matched = 0;
        for b in &bone {
            let Sound::Note(n) = b.sound else { panic!() };
            if bass.iter().any(|x| matches!(x.sound, Sound::Note(m) if (m as i32 - n as i32) % 12 == 0) && (x.start as f64 - b.start as f64).abs() < 40.0 * ms) {
                matched += 1;
            }
        }
        assert!(matched as f64 > bone.len() as f64 * 0.95, "{matched} of {}", bone.len());
        // Down where the upright is: low D to low G.
        assert!(bone.iter().any(|e| e.sound.notes()[0] < 36), "never low");
    }

    /// Alone, the bass lays out and the horns carry its line.
    #[test]
    fn alone_takes_the_bass_out() {
        let evs = take(Horns::LOW, &[Input::ForceLowEnd(Some(LowRole::Alone))], "sweet_georgia_brown");
        assert!(evs.iter().all(|e| e.ch != 2));
        assert!(evs.iter().filter(|e| e.ch == Horn::BassTrombone.ch()).count() > 50);
    }

    #[test]
    fn tacet_is_silent_and_roles_vary() {
        let evs = take(Horns::ALL, &[Input::ForceLowEnd(Some(LowRole::Tacet))], "tiger_rag");
        assert!(evs.iter().all(|e| e.ch < FIRST_CH));
        // Unforced, a long tune's sections don't all do the same thing.
        let mut e = Engine::with_config(&library::load("tiger_rag").unwrap(), 32_000, EngineConfig { seed: 9, horns: Horns::ALL, ..EngineConfig::default() }).unwrap();
        e.post(Input::LevelStart);
        e.post(Input::SetFreedom { lead: 0.8, comp: 0.8, bass: 0.8, drums: 0.8, dynamics: 0.5 });
        let mut roles = std::collections::HashSet::new();
        let mut buf = [Frame::ZERO; 1024];
        for _ in 0..2000 {
            e.fill(&mut buf);
            roles.extend(e.state().upcoming.iter().filter_map(|b| b.low));
        }
        assert!(roles.len() >= 3, "{roles:?}");
    }
}
