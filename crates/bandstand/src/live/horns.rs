//! The horns: a big band's sections, each player on a channel of their own ([`FIRST_CH`] on),
//! written over the band as each bar is committed. Music only: no physics, no director. Off
//! unless [`super::EngineConfig::horns`] puts someone on the stand (Nat Han stays a combo).
//!
//! Who can be on the stand: the low end (bass trombone, bari, tuba), a Freddie Green rhythm
//! guitar, and the upper sections (four trumpets, two altos, two tenors; each section one
//! channel, its voices a chord).
//!
//! **The low end**: the bass trombone, the bari and the tuba, the horns that share the
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
//!
//! **The guitar** ([`Horn::Guitar`]) plays four to the bar in swing, Freddie Green style: a
//! short three-note shell (root, seventh or sixth, third) on every beat, the root low on the
//! sixth string, beats two and four a touch harder. It lays out in the other feels and the
//! breaks.
//!
//! **The upper sections** are planned a chorus at a time by [`super::arranger`]: who has the
//! line ([`Melody`]: the tune passed round in a head, a soloist standing up from a section) and
//! an [`UpperRole`] per section behind it ([`super::Input::ForceUpper`],
//! [`super::Input::ForceMelody`]):
//!
//! | Role | What they play |
//! |---|---|
//! | Soli | the saxes voice the tune: the lead alto on it, four-way close under it, dropped two; the brass answer in its rests |
//! | Tutti | the trumpets on the tune an octave up, four-way close; the saxes the same an octave down, dropped two |
//! | Pads | guide-tone pads behind the line, a long chord's top stepping down |
//! | Punches | brass stabs: the band's hits, or one figure a bar |
//! | Riff | the chorus's two-bar riff (or the tune's theme), voiced through the changes |
//! | Answers | short figures in the line's rests and under its long notes, brass and saxes in turn |
//! | Tacet | the rhythm section and the soloist |
//!
//! Voicing is the arranging books' four-way close: the melody on top, the next three chord tones
//! of the chord's four-note form under it (a sixth for a triad, the ninth for the root in a ninth
//! chord); a short passing note gets the diminished chord, a long tension takes the place of the
//! chord tone under it. Drop 2 moves the second voice down an octave. In soli and tutti the
//! sections carry the tune, so the lead lays out.

use super::band::{BandPlan, HitKind};
use super::chorus::Chorus;
use super::musician::Ctx;
use super::voice::{NoteEvent, Sound};

pub use super::arranger::{Melody, Section, UpperRole};

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
    /// Freddie Green: four to the bar.
    Guitar,
    /// Four trumpets, one channel (a chord).
    Trumpets,
    /// Two altos.
    Altos,
    /// Two tenors.
    Tenors,
}

impl Horn {
    pub const ALL: [Horn; 7] = [Horn::BassTrombone, Horn::Bari, Horn::Tuba, Horn::Guitar, Horn::Trumpets, Horn::Altos, Horn::Tenors];

    /// The bass's register: the low-end roles are theirs.
    pub fn is_low(self) -> bool {
        matches!(self, Horn::BassTrombone | Horn::Bari | Horn::Tuba)
    }

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
            Horn::Guitar => (40, 67),
            Horn::Trumpets => (55, 86),
            Horn::Altos => (49, 80),
            Horn::Tenors => (44, 76),
        }
    }

    /// Where its root-position notes sit, lowest MIDI note of a twelve-note window.
    fn home(self) -> i32 {
        match self {
            Horn::BassTrombone => 33,
            Horn::Bari => 41,
            Horn::Tuba => 26,
            _ => 48,
        }
    }

    /// Its level against the band (a section sits around the bass, not over it: FluidR3's
    /// bari and tuba are much hotter than its upright).
    pub(super) fn level(self) -> f32 {
        match self {
            Horn::BassTrombone => 0.32,
            Horn::Bari => 0.21,
            Horn::Tuba => 0.21,
            Horn::Guitar => 0.35,
            Horn::Trumpets => 0.65,
            Horn::Altos => 0.4,
            Horn::Tenors => 0.4,
        }
    }

    /// Its General MIDI program (0-based: GM has no bass trombone, so the trombone).
    pub fn program(self) -> u8 {
        match self {
            Horn::BassTrombone => 57,
            Horn::Bari => 67,
            Horn::Tuba => 58,
            Horn::Guitar => 25,
            Horn::Trumpets => 56,
            Horn::Altos => 65,
            Horn::Tenors => 66,
        }
    }

    /// File names and labels: `bone4`, `bari`, `tuba`, `guitar`, `trumpets`, `altos`, `tenors`.
    pub fn slug(self) -> &'static str {
        match self {
            Horn::BassTrombone => "bone4",
            Horn::Bari => "bari",
            Horn::Tuba => "tuba",
            Horn::Guitar => "guitar",
            Horn::Trumpets => "trumpets",
            Horn::Altos => "altos",
            Horn::Tenors => "tenors",
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
    /// The low three.
    pub const LOW_END: Horns = Horns(0b111);
    /// A swing big band: everyone but the tuba.
    pub const BIG_BAND: Horns = Horns(0b111_1011);
    /// Everyone.
    pub const ALL: Horns = Horns(0b111_1111);

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
pub(super) const SECTION: usize = 8;

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
    let low = Horns(horns.0 & Horns::LOW_END.0);
    if low.is_empty() {
        return None;
    }
    let horns = low;
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
    for (k, h) in horns.iter().filter(|h| h.is_low()).enumerate() {
        let ch = h.ch();
        let at = |e: NoteEvent| NoteEvent { ch, inst: 0, anchor: 0, gain: e.gain * h.level(), ..e };
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
                for k in 0..32 {
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
                    let mut e = NoteEvent { volume: 15, ..at(ctx.make(&t, b, d, Sound::Note(n as u8))) };
                    // A held hit: the "bwaah", struck and swelling back.
                    if d >= 1.0 {
                        e.fx.swell = -6;
                    }
                    push(out, e);
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
pub(super) fn changes(ctx: &Ctx) -> Vec<(f64, f64, i32, bool)> {
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

/// The guitar's bar: four to the bar in swing (the band's two-feel too), nothing in the other
/// feels, a break or a tacet bar.
pub fn write_guitar(ctx: &Ctx, horns: Horns, out: &mut Vec<NoteEvent>) {
    let plan = ctx.band;
    if !horns.has(Horn::Guitar) || plan.tacet || plan.solo_break || ctx.feel() != super::feel::Feel::Swing || plan.hit_kind == HitKind::Stop {
        return;
    }
    let h = Horn::Guitar;
    let mut t = ctx.template(1, None);
    t.ch = h.ch();
    t.inst = 0;
    t.anchor = 0;
    t.gain *= h.level();
    let bb = ctx.bb() as usize;
    for k in 0..bb {
        let b = k as f64;
        let Some(hm) = ctx.harm_at(b) else { continue };
        let notes = shell(hm.chord);
        let vol = if k % 2 == 1 { 10 } else { 8 };
        push(out, NoteEvent { volume: vol, ..ctx.make(&t, b, 0.42, Sound::Arp(crate::mml::Arp::new(&notes))) });
    }
}

/// Freddie Green's three-note shell: the root on the sixth string (E2 to Eb3), the seventh (or
/// sixth) and the third over it.
fn shell(c: crate::chart::Chord) -> [u8; 3] {
    use crate::chart::Quality::*;
    let root = 40 + (c.root as i32 + 12 - 4) % 12;
    let iv = c.quality.intervals();
    let third = if c.quality == Sus4 { 5 } else { iv[1] as i32 };
    let seventh = match c.quality {
        Major | Six | Minor6 | Dim7 => 9,
        Maj7 | MinMaj7 => 11,
        Minor | Aug => 10,
        _ => 10,
    };
    [root as u8, (root + seventh) as u8, (root + 12 + third) as u8]
}

/// A chord's four-note form, as pitch classes over the root (a sixth on a triad, the ninth for
/// the root in a ninth chord).
pub(super) fn four(c: crate::chart::Chord) -> [i32; 4] {
    use crate::chart::Quality::*;
    let iv: [i32; 4] = match c.quality {
        Major | Six => [0, 4, 7, 9],
        Maj7 => [0, 4, 7, 11],
        Dom9 => [2, 4, 7, 10],
        Dom7b9 => [1, 4, 7, 10],
        Dom7s9 => [3, 4, 7, 10],
        Minor | Minor7 => [0, 3, 7, 10],
        Minor6 => [0, 3, 7, 9],
        Aug => [0, 4, 8, 10],
        q => {
            let v = q.intervals();
            [v[0] as i32, v[1] as i32, v[2] as i32, v[3] as i32]
        }
    };
    iv.map(|i| (c.root as i32 + i) % 12)
}

/// Four-way close under `top` (top first): the next three tones of `c` below it; a short
/// passing note (`passing`) gets the diminished chord, and a tension's next tone down skips a
/// chord tone a half step under it.
pub(super) fn close(top: i32, c: crate::chart::Chord, passing: bool) -> [i32; 4] {
    let pcs = four(c);
    if !pcs.contains(&top.rem_euclid(12)) && passing {
        return [top, top - 3, top - 6, top - 9];
    }
    let mut v = [top; 4];
    let mut n = top;
    for slot in v.iter_mut().skip(1) {
        n -= 1;
        while !pcs.contains(&n.rem_euclid(12)) || top - n == 1 {
            n -= 1;
        }
        *slot = n;
    }
    v
}

/// Drop 2: the second voice from the top down an octave (returned top first, then the rest
/// high to low).
pub(super) fn drop2(v: [i32; 4]) -> [i32; 4] {
    let mut d = [v[0], v[2], v[3], v[1] - 12];
    d[1..].sort_by(|a, b| b.cmp(a));
    d
}

fn push(out: &mut Vec<NoteEvent>, e: NoteEvent) {
    out.push(e);
}

/// A deterministic coin in [0, 1).
pub(super) fn unit(seed: u64, a: u64, b: u64, what: u64) -> f64 {
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
        let b = take(Horns::ALL, &[Input::ForceLowEnd(Some(LowRole::Pads)), Input::ForceUpper(Some(UpperRole::Pads)), Input::ForceMelody(Some(Melody::Lead))], "sweet_georgia_brown");
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

    /// Every upper-section note in its range, the sections' chords under the tune in soli and
    /// tutti, and the lead out of the way there.
    #[test]
    fn upper_sections_voice_the_tune() {
        for role in UpperRole::ALL {
            for song in ["sweet_georgia_brown", "body_and_soul", "yes_we_have_no_bananas"] {
                let evs = take(Horns::BIG_BAND, &[Input::ForceUpper(Some(role)), Input::ForceMelody(Some(Melody::Lead))], song);
                for e in &evs {
                    let Some(h) = Horn::from_ch(e.ch) else { continue };
                    let (lo, hi) = h.range();
                    for &n in e.sound.notes() {
                        assert!((lo..=hi).contains(&(n as i32)), "{song} {role:?} {h:?}: {n}");
                    }
                }
                let lead = evs.iter().filter(|e| e.ch == 0).count();
                let tp = evs.iter().filter(|e| e.ch == Horn::Trumpets.ch()).count();
                let sx = evs.iter().filter(|e| e.ch == Horn::Altos.ch()).count();
                match role {
                    UpperRole::Tutti => assert!(tp > 40 && sx > 40 && lead < 10, "{song}: {tp} {sx} {lead}"),
                    UpperRole::Soli => assert!(tp * 2 < sx && sx > 40 && lead < 10, "{song}: {tp} {sx} {lead}"),
                    UpperRole::Tacet => assert!(tp == 0 && sx == 0, "{song}"),
                    _ => assert!(tp + sx > 10, "{song} {role:?}"),
                }
            }
        }
    }

    #[test]
    fn four_way_close_and_drop_two() {
        use crate::chart::{Chord, Quality};
        let c7 = Chord { root: 0, quality: Quality::Dom7, bass: None };
        // G on top of C7: G E C Bb.
        assert_eq!(close(79, c7, false), [79, 76, 72, 70]);
        assert_eq!(drop2(close(79, c7, false)), [79, 72, 70, 64]);
        // A passing F#: the diminished chord.
        assert_eq!(close(78, c7, true), [78, 75, 72, 69]);
        // Freddie's shell for C7: C2-ish root, Bb, E.
        assert_eq!(shell(c7), [48, 58, 64]);
    }

    /// The guitar plays every beat of a swing bar, short, in the low mids.
    #[test]
    fn freddie_green_four_to_the_bar() {
        let evs = take(Horns::NONE.with(Horn::Guitar), &[Input::ForceFeel(Some(crate::live::feel::Feel::Swing))], "sweet_georgia_brown");
        let gt: Vec<_> = evs.iter().filter(|e| e.ch == Horn::Guitar.ch()).collect();
        assert!(gt.len() >= 32 * 4 * 9 / 10, "{}", gt.len());
        assert!(gt.iter().all(|e| e.sound.notes().len() == 3 && e.sound.notes()[0] < 52));
    }

    #[test]
    fn tacet_is_silent_and_roles_vary() {
        let evs = take(Horns::LOW_END, &[Input::ForceLowEnd(Some(LowRole::Tacet))], "tiger_rag");
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

    /// A horn standing up takes the lead's line: one note at a time, in its range, the lead
    /// out of the way.
    #[test]
    fn the_line_moves_to_a_horn() {
        for (m, h) in [(Melody::Tenor, Horn::Tenors), (Melody::Trumpet, Horn::Trumpets), (Melody::Alto, Horn::Altos)] {
            let evs = take(Horns::BIG_BAND, &[Input::ForceUpper(Some(UpperRole::Tacet)), Input::ForceMelody(Some(m))], "sweet_georgia_brown");
            let lead = evs.iter().filter(|e| e.ch == 0).count();
            let line: Vec<_> = evs.iter().filter(|e| e.ch == h.ch()).collect();
            assert!(lead == 0 && line.len() > 40, "{m:?}: lead {lead}, {}", line.len());
            let (lo, hi) = h.range();
            for e in &line {
                assert!(e.sound.notes().len() == 1 && (lo..=hi).contains(&(e.sound.notes()[0] as i32)), "{m:?}: {:?}", e.sound);
            }
        }
    }

    /// Answers come in where the line rests or holds, not on its attacks.
    #[test]
    fn answers_fill_the_rests() {
        let evs = take(Horns::BIG_BAND, &[Input::ForceUpper(Some(UpperRole::Answers)), Input::ForceMelody(Some(Melody::Lead))], "body_and_soul");
        let lead: Vec<_> = evs.iter().filter(|e| e.ch == 0).collect();
        let horns: Vec<_> = evs.iter().filter(|e| [Horn::Trumpets, Horn::Altos, Horn::Tenors].iter().any(|h| h.ch() == e.ch)).collect();
        assert!(horns.len() > 20, "{}", horns.len());
        let clash = horns.iter().filter(|h| lead.iter().any(|l| l.start.abs_diff(h.start) < 2_000)).count();
        assert!(clash * 10 < horns.len(), "{clash} of {}", horns.len());
    }

    /// Over a long take the arranger uses its whole kit: the line passed round, riffs, answers,
    /// pads; and every tune has a theme.
    #[test]
    fn the_chart_passes_the_line_round() {
        let mut melodies = std::collections::HashSet::new();
        let mut roles = std::collections::HashSet::new();
        for seed in [3, 9] {
            let mut e = Engine::with_config(&library::load("sweet_georgia_brown").unwrap(), 32_000, EngineConfig { seed, horns: Horns::BIG_BAND, ..EngineConfig::default() }).unwrap();
            e.post(Input::LevelStart);
            e.post(Input::SetFreedom { lead: 0.8, comp: 0.8, bass: 0.8, drums: 0.8, dynamics: 0.5 });
            let mut buf = [Frame::ZERO; 1024];
            while e.beat_clock().position.bar < 32 * 6 {
                e.fill(&mut buf);
                for b in &e.state().upcoming {
                    melodies.extend(b.melody);
                    roles.extend(b.upper);
                }
            }
        }
        assert!(melodies.len() >= 3, "{melodies:?}");
        for r in [UpperRole::Riff, UpperRole::Answers, UpperRole::Pads] {
            assert!(roles.contains(&r), "{r:?} never: {roles:?}");
        }
    }
}
