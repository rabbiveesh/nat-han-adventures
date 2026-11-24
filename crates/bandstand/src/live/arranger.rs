//! The big band's arranger: what the trumpets, altos and tenors do, planned a chorus at a time,
//! the way a chart is written (SoundFont only, like the rest of [`super::horns`]).
//!
//! Each 8-bar section of a pass gets a [`Section`]: who has the line ([`Melody`]) and what the
//! other sections do behind it ([`UpperRole`]).
//!
//! **Who has the line.** In a head the tune is passed round: the lead takes the first eight,
//! then a section or a single horn takes a middle eight (the saxes in soli, a trumpet, a tenor
//! or an alto playing the lead's line), never the same one twice running, and the last eight
//! goes back to the lead or to the whole band (tutti). In a solo chorus the soloist stands up
//! from a section that suits them (Miles and Pops on trumpet; Trane, Sonny and Wayne on tenor;
//! Bird on alto; now and then a guest on the lead's own sound) and keeps it for the chorus.
//!
//! **What's behind it** builds through a solo chorus: the soloist alone with the rhythm section
//! first, then pads, then a riff or answers, and a riff into the next chorus. The riff
//! ([`Figure`]) is two bars, one per chorus (an ostinato, like Gil Evans's figure under Miles
//! on *Summertime*), voiced through the changes on guide tones, and now and then the tune's own
//! opening rhythm (its theme), which also plays the intro and shouts in the shout chorus,
//! brass calling and saxes answering. Answers fill the line's rests and long notes with a short
//! figure, brass and saxes taking turns.

use super::band::HitKind;
use super::chorus::Chorus;
use super::horns::{Horn, Horns, SECTION, changes, drop2, four, unit};
use super::musician::Ctx;
use super::ornament::Harm;
use super::soloist::{LineAhead, Soloist};
use super::voicing::{self, Balance, Passing, Style, Voicing};
use super::voice::{NoteEvent, Sound};
use crate::chart::Chord;
use crate::mml::EventKind;

/// What the upper sections do in a section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UpperRole {
    /// Nothing.
    Tacet,
    /// The saxes have the tune in four parts (drop 2); the brass answer in its rests.
    Soli,
    /// Everyone has the tune: trumpets in four-way close, the saxes in drop 2 an octave down.
    Tutti,
    /// Guide-tone pads behind the line.
    Pads,
    /// Short punches on the band's hits.
    Punches,
    /// A two-bar riff, the chorus's [`Figure`], through the section.
    Riff,
    /// Short figures in the line's rests and under its long notes.
    Answers,
}

impl UpperRole {
    pub const ALL: [UpperRole; 7] = [UpperRole::Tacet, UpperRole::Soli, UpperRole::Tutti, UpperRole::Pads, UpperRole::Punches, UpperRole::Riff, UpperRole::Answers];

    pub fn slug(self) -> &'static str {
        match self {
            UpperRole::Tacet => "tacet",
            UpperRole::Soli => "soli",
            UpperRole::Tutti => "tutti",
            UpperRole::Pads => "pads",
            UpperRole::Punches => "punches",
            UpperRole::Riff => "riff",
            UpperRole::Answers => "answers",
        }
    }

    pub fn parse(s: &str) -> Option<UpperRole> {
        UpperRole::ALL.into_iter().find(|r| r.slug() == s)
    }
}

/// Who plays the lead's line (the tune, or the solo): the lead on its own channel, or a horn
/// standing up from its section (on that section's channel).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Melody {
    #[default]
    Lead,
    Trumpet,
    Alto,
    Tenor,
}

impl Melody {
    pub const ALL: [Melody; 4] = [Melody::Lead, Melody::Trumpet, Melody::Alto, Melody::Tenor];

    pub fn slug(self) -> &'static str {
        match self {
            Melody::Lead => "lead",
            Melody::Trumpet => "trumpet",
            Melody::Alto => "alto",
            Melody::Tenor => "tenor",
        }
    }

    pub fn parse(s: &str) -> Option<Melody> {
        Melody::ALL.into_iter().find(|m| m.slug() == s)
    }

    /// The section it stands up from.
    pub fn horn(self) -> Option<Horn> {
        match self {
            Melody::Lead => None,
            Melody::Trumpet => Some(Horn::Trumpets),
            Melody::Alto => Some(Horn::Altos),
            Melody::Tenor => Some(Horn::Tenors),
        }
    }

    /// Its gain on the lead's line (about the lead's level).
    fn level(self) -> f32 {
        match self {
            Melody::Lead => 1.0,
            Melody::Trumpet => 1.5,
            Melody::Alto => 1.1,
            Melody::Tenor => 0.9,
        }
    }

    /// Octaves the lead's line moves to sit in the horn (a tenor an octave down).
    fn shift(self) -> i32 {
        if self == Melody::Tenor { -12 } else { 0 }
    }
}

/// A section's plan: who has the line, what's behind it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Section {
    pub role: UpperRole,
    pub melody: Melody,
}

/// A riff: notes as (bar 0 or 1, beat in the bar, beats, chord-tone steps from the guide tone).
pub type Figure = Vec<(u8, f64, f64, i32)>;

/// The riffs a chorus can take (two bars of 4/4; a waltz bar drops beat 4).
const FIGURES: [&[(u8, f64, f64, i32)]; 7] = [
    // Rocking long-short-long between two tones (the Summertime kind).
    &[(0, 0.0, 1.5, 0), (0, 1.5, 0.5, 1), (0, 2.0, 1.75, 0), (1, 0.0, 1.5, -1), (1, 1.5, 0.5, 0), (1, 2.0, 1.75, -1)],
    // Basie: the and of two into three.
    &[(0, 1.5, 0.5, 0), (0, 2.0, 1.0, 0), (1, 1.5, 0.5, 1), (1, 2.0, 1.5, 0)],
    // Charleston.
    &[(0, 0.0, 1.5, 0), (0, 1.5, 0.5, 0), (1, 0.0, 1.5, 1), (1, 1.5, 0.5, 0)],
    // A long note, then a kick on the and of four.
    &[(0, 0.0, 3.0, 0), (1, 3.5, 0.5, 1)],
    // Stabs and a landing.
    &[(0, 0.5, 0.4, 0), (0, 1.5, 0.4, 0), (0, 3.0, 1.0, 1), (1, 0.5, 0.4, 0), (1, 1.5, 0.4, 0), (1, 3.0, 1.0, -1)],
    // Doo-wah: two quarters and a half.
    &[(0, 2.0, 1.0, 0), (0, 3.0, 1.0, 1), (1, 0.0, 2.0, 0)],
    // Call in the second bar only (room for the soloist's first).
    &[(1, 0.5, 0.5, 1), (1, 1.0, 0.5, 0), (1, 1.5, 1.0, -1), (1, 3.0, 0.75, 0)],
];

/// Short answers: (beat from the gap's start, beats, steps).
const ANSWERS: [&[(f64, f64, i32)]; 5] = [
    &[(0.0, 0.5, 0), (0.5, 1.0, 1)],
    &[(0.0, 1.0, 0), (1.0, 0.5, -1), (1.5, 1.0, 0)],
    &[(0.0, 0.5, 1), (0.5, 0.5, 0), (1.0, 1.5, -1)],
    &[(0.0, 2.0, 0)],
    &[(0.0, 0.4, 0), (1.0, 0.4, 0)],
];

/// Weighted pick with coin `x` in [0, 1).
fn pick<T: Copy>(x: f64, opts: &[(T, f64)]) -> T {
    let total: f64 = opts.iter().map(|o| o.1).sum();
    let mut x = x * total;
    for &(v, w) in opts {
        if x < w {
            return v;
        }
        x -= w;
    }
    opts[opts.len() - 1].0
}

/// Sections in the tune.
fn sections(ctx: &Ctx) -> usize {
    ctx.shape.bars.div_ceil(SECTION).max(1)
}

/// The horn a solo chorus's soloist stands up from.
fn solo_horn(ctx: &Ctx, horns: Horns) -> Melody {
    let c = |what: u64| unit(ctx.seed, ctx.bar.pass, 0, what);
    if c(20) < 0.2 {
        return Melody::Lead;
    }
    let want = match ctx.soloist {
        Soloist::Miles | Soloist::Pops => Melody::Trumpet,
        Soloist::Trane | Soloist::Sonny | Soloist::Wayne => Melody::Tenor,
        Soloist::Bird => Melody::Alto,
        _ => pick(c(21), &[(Melody::Trumpet, 1.0), (Melody::Tenor, 1.0), (Melody::Alto, 0.7)]),
    };
    if want.horn().is_some_and(|h| horns.has(h)) { want } else { Melody::Lead }
}

/// Who takes the tune in a head's section.
#[derive(Clone, Copy, PartialEq)]
enum Take {
    Line(Melody),
    Soli,
    Tutti,
}

/// The plan for the section `ctx`'s bar is in (`force`, `force_melody`: overrides).
pub fn plan(ctx: &Ctx, horns: Horns, force: Option<UpperRole>, force_melody: Option<Melody>) -> Section {
    let band = ctx.band;
    let mut sec = if band.intro.is_some() {
        Section { role: UpperRole::Riff, melody: Melody::Lead }
    } else if band.ending.is_some() {
        Section { role: UpperRole::Punches, melody: Melody::Lead }
    } else {
        chorus_plan(ctx, horns, ctx.bar.song_bar / SECTION)
    };
    if let Some(r) = force {
        sec.role = r;
    }
    if let Some(m) = force_melody
        && m.horn().is_none_or(|h| horns.has(h))
    {
        sec.melody = m;
    }
    if matches!(sec.role, UpperRole::Soli | UpperRole::Tutti) {
        sec.melody = Melody::Lead;
    }
    sec
}

fn chorus_plan(ctx: &Ctx, horns: Horns, s: usize) -> Section {
    let n = sections(ctx);
    let last = s + 1 >= n;
    let c = |k: usize, what: u64| unit(ctx.seed, ctx.bar.pass, k as u64, what);
    let chorus = ctx.band.chorus;
    use UpperRole::*;
    if chorus.lead_solos(ctx.freedom[0]) {
        let melody = solo_horn(ctx, horns);
        let role = match chorus {
            Chorus::Strolling => Tacet,
            Chorus::StopTime => Punches,
            Chorus::Riffs => {
                if last && n > 1 && c(s, 30) < 0.4 {
                    Answers
                } else {
                    Riff
                }
            }
            // A blowing chorus builds: alone, then pads or answers, then the riff.
            _ => {
                if s == 0 && n > 1 {
                    pick(c(s, 31), &[(Tacet, 0.5), (Pads, 0.25), (Answers, 0.25)])
                } else if last {
                    pick(c(s, 32), &[(Riff, 0.6), (Answers, 0.35), (Tacet, 0.05)])
                } else if s == 1 {
                    pick(c(s, 33), &[(Tacet, 0.15), (Pads, 0.3), (Answers, 0.35), (Riff, 0.2)])
                } else {
                    pick(c(s, 34), &[(Tacet, 0.1), (Riff, 0.45), (Answers, 0.45)])
                }
            }
        };
        return Section { role, melody };
    }
    let line = |role| Section { role, melody: Melody::Lead };
    match chorus {
        Chorus::Soli => line(Soli),
        Chorus::Shout => line(if last || c(s, 35) < 0.5 { Tutti } else { Riff }),
        Chorus::StopTime => line(Punches),
        Chorus::Riffs => line(Riff),
        Chorus::Breaks => line(pick(c(s, 36), &[(Tacet, 0.3), (Pads, 0.2), (Punches, 0.3), (Answers, 0.2)])),
        Chorus::Strolling => line(Tacet),
        _ => head(ctx, horns, s, n),
    }
}

/// A head: the tune passed round the band, section by section.
fn head(ctx: &Ctx, horns: Horns, s: usize, n: usize) -> Section {
    use UpperRole::*;
    let c = |k: usize, what: u64| unit(ctx.seed, ctx.bar.pass, k as u64, what);
    let two = ctx.band.chorus == Chorus::TwoFeel;
    match head_take(ctx.seed, ctx.bar.pass, two, horns, s, n) {
        Take::Soli => Section { role: Soli, melody: Melody::Lead },
        Take::Tutti => Section { role: Tutti, melody: Melody::Lead },
        Take::Line(melody) => {
            // Less is more: the tune often alone with the rhythm section.
            let role = if two || (s == 0 && ctx.bar.pass == 0) {
                pick(c(s, 42), &[(Tacet, 0.4), (Pads, 0.3), (Answers, 0.3)])
            } else {
                pick(c(s, 43), &[(Tacet, 0.15), (Pads, 0.25), (Answers, 0.4), (Riff, 0.2)])
            };
            Section { role, melody }
        }
    }
}

/// Whether the saxes take section `s` of a head in soli (the band plans the quartet's
/// counterpoint for it then: [`super::band`]). Needs `bars` (the song's), the chorus, the
/// lead's freedom and the band's own plan's intro and ending, as [`chorus_plan`] reads them.
#[allow(clippy::too_many_arguments)]
pub fn head_soli(seed: u64, pass: u64, chorus: Chorus, lead_freedom: f32, horns: Horns, song_bar: usize, bars: usize, framed: bool) -> bool {
    if framed || chorus.lead_solos(lead_freedom) {
        return false;
    }
    if matches!(chorus, Chorus::Soli | Chorus::Shout | Chorus::StopTime | Chorus::Riffs | Chorus::Breaks | Chorus::Strolling) {
        return false;
    }
    let n = bars.div_ceil(SECTION).max(1);
    head_take(seed, pass, chorus == Chorus::TwoFeel, horns, song_bar / SECTION, n) == Take::Soli
}

/// Who takes section `s` (of `n`) of a head: the lead first, a section or a horn in the
/// middle (never the same twice running), the lead or everyone last.
fn head_take(seed: u64, pass: u64, two: bool, horns: Horns, s: usize, n: usize) -> Take {
    let c = |k: usize, what: u64| unit(seed, pass, k as u64, what);
    let (tp, al, te) = (horns.has(Horn::Trumpets), horns.has(Horn::Altos), horns.has(Horn::Tenors));
    let mut prev = Take::Line(Melody::Lead);
    for k in 0..=s {
        let take = if k == 0 {
            Take::Line(Melody::Lead)
        } else if k + 1 >= n {
            let tutti = if pass == 0 { 0.35 } else { 0.5 };
            if tp && (al || te) && !two && c(k, 40) < tutti { Take::Tutti } else { Take::Line(Melody::Lead) }
        } else {
            let opts: Vec<(Take, f64)> = [
                (Take::Line(Melody::Lead), 3.0, true),
                (Take::Soli, 2.0, al && te && !two),
                (Take::Line(Melody::Trumpet), 1.5, tp),
                (Take::Line(Melody::Tenor), 1.5, te),
                (Take::Line(Melody::Alto), 1.0, al),
            ]
            .into_iter()
            .filter(|&(t, _, ok)| ok && t != prev)
            .map(|(t, w, _)| (t, w))
            .collect();
            if opts.is_empty() { Take::Line(Melody::Lead) } else { pick(c(k, 41), &opts) }
        };
        if k == s {
            return take;
        }
        prev = take;
    }
    prev
}

/// The chorus's riff: one of [`FIGURES`], or the tune's theme (more often in a shout and always
/// in the intro).
pub fn figure(ctx: &Ctx) -> Figure {
    let x = unit(ctx.seed, ctx.bar.pass, 0, 50);
    let theme_odds = if ctx.band.intro.is_some() {
        1.0
    } else if ctx.band.chorus == Chorus::Shout {
        0.75
    } else {
        0.5
    };
    if x < theme_odds {
        // The intro takes the tune's opening; a chorus any of its first phrases' motifs.
        let k = if ctx.band.intro.is_some() { 0 } else { (unit(ctx.seed, ctx.bar.pass, 0, 52) * 4.0) as usize };
        let t = theme_at(ctx, k);
        if t.len() >= 3 {
            return t;
        }
    }
    let k = (unit(ctx.seed, ctx.bar.pass, 0, 51) * FIGURES.len() as f64) as usize % FIGURES.len();
    FIGURES[k].to_vec()
}

/// The tune's theme: the rhythm and contour of its first two bars with three notes or more,
/// on 8ths, its steps the melody's ups and downs.
pub fn theme(ctx: &Ctx) -> Figure {
    theme_at(ctx, 0)
}

/// The `k`th two-bar motif of the tune's first eight bars (bars 0-1, 2-3, ...: the first with
/// three notes or more from there, or the opening's).
fn theme_at(ctx: &Ctx, k: usize) -> Figure {
    let arr = ctx.arrangement;
    let bb = ctx.bb();
    let tr = &arr.tracks[0];
    let bars = arr.bar_events[0].len();
    let first = (2 * k).min(bars.saturating_sub(2));
    for start in (first..bars.saturating_sub(1).min(8)).chain(0..first) {
        let (lo, _) = arr.bar_events[0][start];
        let (_, hi) = arr.bar_events[0][start + 1];
        let notes: Vec<(f64, f64, i32)> = tr.events[lo..hi]
            .iter()
            .filter_map(|e| match e.kind {
                EventKind::Note(n) => Some((e.start - start as f64 * bb, e.dur, n as i32)),
                _ => None,
            })
            .collect();
        if notes.len() < 3 {
            continue;
        }
        // On 8ths (un-swung).
        let q = |b: f64| {
            let k = b.floor();
            let f = b - k;
            k + if f < 0.25 {
                0.0
            } else if f < 0.9 {
                0.5
            } else {
                1.0
            }
        };
        let mut out: Figure = Vec::new();
        let mut step = 0i32;
        let mut prev: Option<i32> = None;
        for (i, &(b, d, n)) in notes.iter().enumerate() {
            let b = q(b);
            if b >= 2.0 * bb - 1e-9 || out.last().is_some_and(|l| (l.0 as f64 * bb + l.1 - b).abs() < 1e-9) {
                continue;
            }
            let next = notes.get(i + 1).map_or(2.0 * bb, |x| q(x.0));
            let d = ((d * 2.0).round() / 2.0).max(0.5).min((next - b).max(0.5));
            if let Some(p) = prev {
                let iv = n - p;
                if iv != 0 {
                    step += iv.signum() * ((iv.abs() as f64 / 3.5).round() as i32).max(1);
                }
            }
            step = step.clamp(-3, 3);
            prev = Some(n);
            let bar = (b / bb).floor();
            out.push((bar as u8, b - bar * bb, d.min(bb * 2.0 - b), step));
        }
        return out;
    }
    Vec::new()
}

/// The line's last figure before `gap` (its last two to four notes, the last no more than a
/// beat and a half before the gap), as an answer fitting `room`: on 8ths, its steps the
/// line's ups and downs (`invert`: the other way).
fn echo(heard: &[(f64, f64, i32)], gap: f64, room: f64, invert: bool) -> Option<Vec<(f64, f64, i32)>> {
    let before: Vec<&(f64, f64, i32)> = heard.iter().filter(|n| n.0 < gap - 0.1).collect();
    let last = before.last()?;
    if gap - (last.0 + last.1) > 1.5 || before.len() < 2 {
        return None;
    }
    let tail = &before[before.len().saturating_sub(4)..];
    let t0 = tail[0].0;
    let q = |b: f64| (b * 2.0).round() / 2.0;
    let mut out = Vec::new();
    let mut step = 0;
    for (k, n) in tail.iter().enumerate() {
        if k > 0 {
            let iv = n.2 - tail[k - 1].2;
            if iv != 0 {
                step += iv.signum() * ((iv.abs() as f64 / 3.5).round() as i32).max(1) * if invert { -1 } else { 1 };
            }
        }
        let b = q(n.0 - t0);
        let next = tail.get(k + 1).map_or(b + n.1, |m| q(m.0 - t0));
        let d = q(n.1).max(0.5).min((next - b).max(0.4));
        if b + d > room + 0.25 || out.last().is_some_and(|l: &(f64, f64, i32)| l.0 == b) {
            break;
        }
        out.push((b, d, step.clamp(-3, 3)));
    }
    (out.len() >= 2).then_some(out)
}

/// Two to four notes of a motif (a [`Figure`]) fitting `room`, from its first.
fn motif(fig: &Figure, room: f64) -> Option<Vec<(f64, f64, i32)>> {
    let t0 = fig.first()?.0 as f64 * 4.0 + fig.first()?.1;
    let out: Vec<(f64, f64, i32)> = fig.iter().map(|f| (f.0 as f64 * 4.0 + f.1 - t0, f.2, f.3)).take_while(|f| f.0 + f.1 <= room + 0.25).take(4).collect();
    (out.len() >= 2).then_some(out)
}

/// A top note for chord `c` near `center`: its nearest guide tone (third or seventh), moved
/// `step` chord tones up or down.
fn top_at(c: Chord, center: i32, step: i32) -> i32 {
    let pcs = four(c);
    let guide = [pcs[1], pcs[3]];
    let mut n = (center - 6..=center + 6).filter(|n| guide.contains(&n.rem_euclid(12))).min_by_key(|n| (n - center).abs()).unwrap_or(center);
    let mut k = step;
    while k > 0 {
        n += 1;
        while !pcs.contains(&n.rem_euclid(12)) {
            n += 1;
        }
        k -= 1;
    }
    while k < 0 {
        n -= 1;
        while !pcs.contains(&n.rem_euclid(12)) {
            n -= 1;
        }
        k += 1;
    }
    n
}

fn octave_into(n: i32, lo: i32, hi: i32) -> i32 {
    let mut n = n;
    while n < lo {
        n += 12;
    }
    while n > hi {
        n -= 12;
    }
    n
}

/// Who plays a figure.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Group {
    Brass,
    Saxes,
    All,
}

/// The upper sections' writer for one bar.
struct Writer<'a, 'b> {
    ctx: &'a Ctx<'b>,
    /// The sections free to play (the one with the line standing out).
    free: Horns,
    /// How the notes about to be played are phrased ([`Writer::phrase`]).
    art: std::cell::Cell<Art>,
    /// How the chords are voiced ([`voicing`]).
    v: Voicing,
    /// The chord the line is heading for (the tune sets it note by note).
    next: std::cell::Cell<Option<Chord>>,
}

/// How a note is played: louder or not, how much of its length, a fall (negative) or doit
/// (positive) off its end (semitones, the trumpets'; the saxes go two thirds as far), a swell
/// (dB, [`super::voice::Fx::swell`]).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
struct Art {
    accent: i8,
    len: f64,
    fall: i8,
    swell: i8,
}

impl Art {
    const PLAIN: Art = Art { accent: 0, len: 1.0, fall: 0, swell: 0 };
}

impl<'a, 'b> Writer<'a, 'b> {
    fn new(ctx: &'a Ctx<'b>, free: Horns, v: Voicing) -> Self {
        Writer { ctx, free, art: std::cell::Cell::new(Art::PLAIN), v, next: std::cell::Cell::new(None) }
    }

    /// The chorus's voicing, heading for the next chord.
    fn vv(&self) -> Voicing {
        Voicing { next: self.next.get(), ..self.v }
    }

    /// The trumpets' voicing under `top`.
    fn brass_v(&self, top: i32, c: Chord, passing: bool) -> Vec<i32> {
        voicing::finish(&self.v, voicing::brass(&self.vv(), top, c, passing), c)
    }

    /// The saxes' voicing under `top`.
    fn sax_v(&self, top: i32, c: Chord, passing: bool) -> Vec<i32> {
        voicing::finish(&self.v, voicing::saxes(&self.vv(), top, c, passing), c)
    }

    /// One sax section's voices on its own (close for drop 2, the style otherwise).
    fn one_sax_v(&self, top: i32, c: Chord, passing: bool) -> Vec<i32> {
        let v = self.vv();
        let x = if v.style == Style::Drop2 { voicing::brass(&v, top, c, passing) } else { voicing::four_voices(&v, top, c, passing) };
        voicing::finish(&self.v, x, c)
    }

    /// A section's top note on `h` near `near`, aimed as the featured soloist aims their
    /// strong beats ([`super::soloist::target_weights`]: 3 5 7 and 9ths, a dominant's
    /// tensions as often as they take them), the records' even spread under the band's own
    /// line. Seeded by the bar and the beat.
    fn aim(&self, h: &Harm, near: i32, b: f64) -> i32 {
        use super::soloist::{Moment, choose, target_note, target_weights};
        let ctx = self.ctx;
        let m = Moment { bpm: ctx.shape.bpm, family: Some(h.chord.family()), ..Moment::default() };
        let w = match ctx.soloist.profile() {
            Some(p) => target_weights(p, &m),
            None => [0.142, 0.136, 0.147, 0.136, 0.178, 0.0, 0.0, 0.0, 0.0, 0.0],
        };
        let mut r = crate::accomp::Rng::new(ctx.seed ^ ctx.bar.index.wrapping_mul(0x9E37_79B9) ^ ((b * 8.0) as u64) << 40 ^ 0xa1);
        let k = choose(&w, &mut r);
        let n = target_note(h, k, near.clamp(0, 127) as u8) as i32;
        // (A root on top sounds like an ending: the guide tone it was, then.)
        if k == 0 { near } else { n }
    }

    /// Phrase the next notes like a big band section: an 8th on the and accented, a phrase's
    /// last short note clipped ("dat"), quarters detached, an anticipation accented, a long
    /// note swelling (soft: a crescendo; loud: fp, crescendo) or, ending a phrase with room
    /// after it, falling off (or a doit, in a shout). `last`: it ends a phrase; `room`: beats
    /// of rest after it; `loud`: a shout or tutti.
    fn phrase(&self, b: f64, d: f64, last: bool, room: f64, loud: bool) {
        let off = (b.fract() - 0.5).abs() < 0.01;
        let coin = unit(self.ctx.seed, self.ctx.bar.index, (b * 8.0) as u64, 70);
        let mut a = Art::PLAIN;
        if off {
            a.accent = 2;
        } else if b.fract() < 0.01 && d <= 0.55 && !last {
            // The 8th before an and: ghosted.
            a.accent = -2;
        }
        if d <= 0.55 {
            a.len = if last { 0.55 } else { 0.9 };
        } else if d <= 1.1 {
            a.len = if last { 0.6 } else { 0.75 };
            if last && loud && coin < 0.3 {
                a.fall = 4;
            }
        } else if last && room >= 0.75 && coin < if loud { 0.5 } else { 0.25 } {
            a.fall = if loud { -7 } else { -5 };
            a.len = 0.95;
        } else {
            a.swell = if loud { -6 } else { 5 };
            a.len = 0.95;
        }
        self.art.set(a);
    }

    /// The section's dynamics: a step up in its fifth and sixth bars, two in its last two.
    fn build(&self) -> i32 {
        [0, 0, 0, 1, 1, 2, 3, 3][self.ctx.bar.song_bar % SECTION]
    }

    fn brass(&self) -> bool {
        self.free.has(Horn::Trumpets)
    }

    fn saxes(&self) -> bool {
        self.free.has(Horn::Altos) || self.free.has(Horn::Tenors)
    }

    /// The group that plays when `want` can't.
    fn group(&self, want: Group) -> Option<Group> {
        match (want, self.brass(), self.saxes()) {
            (_, false, false) => None,
            (Group::All, true, true) => Some(Group::All),
            (Group::Brass, true, _) | (Group::All, true, false) => Some(Group::Brass),
            (Group::Saxes, _, true) | (Group::All, false, true) => Some(Group::Saxes),
            (Group::Brass, false, true) => Some(Group::Saxes),
            (Group::Saxes, true, false) => Some(Group::Brass),
        }
    }

    fn play(&self, out: &mut Vec<NoteEvent>, h: Horn, vol: u8, b: f64, d: f64, notes: &[i32]) {
        if !self.free.has(h) || notes.is_empty() {
            return;
        }
        let (lo, hi) = h.range();
        let ns: Vec<u8> = notes.iter().map(|&n| octave_into(n, lo, hi) as u8).collect();
        let a = self.art.get();
        let mut t = self.ctx.template(1, None);
        t.ch = h.ch();
        t.inst = 0;
        t.anchor = 0;
        t.gain *= h.level();
        // (Two under what's asked, so the accents and the build have room.)
        t.volume = (vol as i32 - 2 + a.accent as i32).clamp(1, 15) as u8;
        let d = (d * a.len).max(0.2);
        let mut e = self.ctx.make(&t, b, d, Sound::Arp(crate::mml::Arp::new(&ns)));
        e.fx.swell = a.swell;
        e.fx.balance = self.v.balance;
        if a.fall != 0 {
            let fall = if h == Horn::Trumpets { a.fall } else { a.fall * 2 / 3 };
            // Over the note's last half, a quarter second at most.
            let frames_per_beat = 3600.0 / self.ctx.shape.bpm.max(1.0) as f64;
            e.fx.fall = fall;
            e.fx.fall_frames = (d * frames_per_beat * 0.5).clamp(5.0, 16.0) as u8;
        }
        out.push(e);
    }

    /// A chord under `top` by `group`: trumpets in four-way close; the saxes in drop 2 an octave
    /// under the trumpets (or on the top with no brass), split altos over tenors, or all four in
    /// one sax section if it's alone.
    #[allow(clippy::too_many_arguments)]
    fn chord(&self, out: &mut Vec<NoteEvent>, group: Group, vol: u8, b: f64, d: f64, top: i32, c: Chord, passing: bool) {
        let s = self.ctx.swing8(b);
        for (h, notes, under) in self.voices(group, top, c, passing) {
            self.play(out, h, vol.saturating_sub(under), s, d, &notes);
        }
    }

    /// [`Self::chord`]'s voicing: each section's notes (and how much softer it plays).
    fn voices(&self, group: Group, top: i32, c: Chord, passing: bool) -> Vec<(Horn, Vec<i32>, u8)> {
        let mut out = Vec::new();
        let brass = group != Group::Saxes && self.brass();
        let saxes = group != Group::Brass && self.saxes();
        let top_tp = octave_into(top, 67, 82);
        let under = self.v.balance.saxes_under();
        if brass {
            out.push((Horn::Trumpets, self.brass_v(top_tp, c, passing).to_vec(), 0));
        }
        if saxes {
            let top_sx = if brass { top_tp - 12 } else { octave_into(top, 62, 76) };
            let v = self.sax_v(top_sx, c, passing);
            match (self.free.has(Horn::Altos), self.free.has(Horn::Tenors)) {
                (true, true) => {
                    let k = v.len().div_ceil(2);
                    out.push((Horn::Altos, v[..k].to_vec(), under));
                    out.push((Horn::Tenors, v[k..].to_vec(), under));
                }
                (true, false) => out.push((Horn::Altos, self.one_sax_v(top_sx, c, passing).to_vec(), under)),
                (false, true) => out.push((Horn::Tenors, self.one_sax_v(top_sx - 5, c, passing).to_vec(), under)),
                _ => {}
            }
        }
        out
    }

    /// One note in unison: the trumpets on it, the saxes an octave under (a line, not a
    /// chord: how a band plays its fast notes and its calls).
    fn unison(&self, out: &mut Vec<NoteEvent>, group: Group, vol: u8, b: f64, d: f64, top: i32) {
        let b = self.ctx.swing8(b);
        let brass = group != Group::Saxes && self.brass();
        let top_tp = octave_into(top, 67, 82);
        if brass {
            self.play(out, Horn::Trumpets, vol + 1, b, d, &[top_tp]);
        }
        if group != Group::Brass {
            let n = if brass { top_tp - 12 } else { octave_into(top, 62, 76) };
            self.play(out, Horn::Altos, vol, b, d, &[n]);
            self.play(out, Horn::Tenors, vol, b, d, &[n - 12 * brass as i32]);
        }
    }

    /// A wedge: the voicing's top climbs and its bottom sinks a half step at a time while the
    /// inner voices hold, the chord opening outward (a phrase's last long note).
    #[allow(clippy::too_many_arguments)]
    fn wedge(&self, out: &mut Vec<NoteEvent>, group: Group, vol: u8, b: f64, d: f64, top: i32, c: Chord) {
        let voices = self.voices(group, top, c, false);
        let all: Vec<i32> = voices.iter().flat_map(|v| v.1.iter().copied()).collect();
        let (Some(&hi), Some(&lo)) = (all.iter().max(), all.iter().min()) else { return };
        let b = self.ctx.swing8(b);
        let steps = if d >= 3.0 { 3 } else { 2 };
        let seg = (d / (steps as f64 + 1.0)).max(0.25);
        for k in 0..=steps {
            let at = b + k as f64 * seg;
            let len = if k == steps { b + d - at } else { seg };
            self.art.set(if k == steps { Art { swell: 6, len: 1.0, ..Art::PLAIN } } else { Art { len: 0.98, ..Art::PLAIN } });
            for (h, ns, under) in &voices {
                let moved: Vec<i32> = ns.iter().map(|&n| if n == hi { n + k } else if n == lo { n - k } else { n }).collect();
                self.play(out, *h, vol.saturating_sub(*under), at, len, &moved);
            }
        }
    }

    /// One pad, shaped by the section's inner-voice device ([`Device`]): `b`, `d` beats,
    /// the chord `c` under `top`.
    #[allow(clippy::too_many_arguments)]
    fn pad(&self, out: &mut Vec<NoteEvent>, group: Group, vol: u8, b: f64, d: f64, top: i32, c: Chord, device: Device) {
        let voices = self.voices(group, top, c, false);
        let all: Vec<i32> = voices.iter().flat_map(|v| v.1.iter().copied()).collect();
        if voices.is_empty() || all.is_empty() {
            return;
        }
        let device = if d < 2.0 - 1e-9 { Device::Plain } else { device };
        let swell = Art { swell: 6, len: 1.0, ..Art::PLAIN };
        let plain = Art::PLAIN;
        // Each voice by itself, as a note on its section's channel.
        let one = |out: &mut Vec<NoteEvent>, h: Horn, under: u8, at: f64, len: f64, n: i32| self.play(out, h, vol.saturating_sub(under), at, len.max(0.25), &[n]);
        match device {
            Device::Plain => {
                self.art.set(swell);
                self.chord(out, group, vol, b, d, top, c, false);
            }
            Device::Bloom => {
                // Unison, then out: every voice starts on the voicing's middle note and fans out
                // to its place in two steps, contrary motion round the middle.
                let mid = all.iter().sum::<i32>() / all.len() as i32;
                for (k, x) in [0.0, 0.5, 1.0].into_iter().enumerate() {
                    let at = b + k as f64 * 0.5;
                    let len = if k == 2 { d - 1.0 } else { 0.5 };
                    self.art.set(if k == 2 { swell } else { Art { len: 0.98, ..plain } });
                    for (h, notes, under) in &voices {
                        let mut ns: Vec<i32> = notes.iter().map(|&t| mid + ((t - mid) as f64 * x).round() as i32).collect();
                        ns.dedup();
                        self.play(out, *h, vol.saturating_sub(*under), at, len, &ns);
                    }
                }
            }
            Device::Pyramid => {
                // The voices come in one by one, bottom up, and hold.
                let mut order: Vec<(i32, Horn, u8)> = voices.iter().flat_map(|(h, ns, u)| ns.iter().map(move |&n| (n, *h, *u))).collect();
                order.sort_by_key(|o| o.0);
                let step = (d / (order.len() as f64 + 1.0)).min(0.5);
                self.art.set(swell);
                for (i, &(n, h, under)) in order.iter().enumerate() {
                    let at = b + i as f64 * step;
                    one(out, h, under, at, b + d - at, n);
                }
            }
            Device::Cliche | Device::Suspension => {
                // One inner voice moves while the rest hold: a line cliché off the fifth (5 #5
                // 6 #5) or the root (1 7 b7 6), or the third suspended (4, then 3).
                let root = c.root as i32;
                let third = four(c)[1];
                let pick = |want: &dyn Fn(i32) -> bool| voices.iter().enumerate().find_map(|(vi, (_, ns, _))| ns.iter().position(|&n| want(n.rem_euclid(12))).map(|ni| (vi, ni)));
                let (moving, line): (Option<(usize, usize)>, Vec<(f64, i32)>) = if device == Device::Suspension {
                    let m = pick(&|pc| pc == third && (pc - root).rem_euclid(12) >= 3);
                    let up = (root + 5 - third).rem_euclid(12);
                    (m, vec![(0.0, up), (1.0, 0)])
                } else if let Some(m) = pick(&|pc| pc == (root + 7) % 12) {
                    (Some(m), (0..4).map(|k| (k as f64 * d / 4.0, [0, 1, 2, 1][k])).collect())
                } else {
                    (pick(&|pc| pc == root), (0..4).map(|k| (k as f64 * d / 4.0, [0, -1, -2, -3][k])).collect())
                };
                let Some((vi, ni)) = moving else {
                    self.art.set(swell);
                    self.chord(out, group, vol, b, d, top, c, false);
                    return;
                };
                self.art.set(swell);
                for (k, (h, ns, under)) in voices.iter().enumerate() {
                    let rest: Vec<i32> = ns.iter().enumerate().filter(|&(j, _)| !(k == vi && j == ni)).map(|(_, &n)| n).collect();
                    if !rest.is_empty() {
                        self.play(out, *h, vol.saturating_sub(*under), b, d, &rest);
                    }
                }
                let (h, ns, under) = &voices[vi];
                let n0 = ns[ni];
                self.art.set(Art { len: 0.98, ..plain });
                for (j, &(at, off)) in line.iter().enumerate() {
                    let to = line.get(j + 1).map_or(d, |x| x.0);
                    one(out, *h, under.saturating_sub(1), b + at, to - at, n0 + off);
                }
            }
        }
    }

    /// This bar's half of the chorus's riff.
    fn riff(&self, out: &mut Vec<NoteEvent>, fig: &Figure, shout: bool) -> Option<()> {
        let ctx = self.ctx;
        let bb = ctx.bb();
        let half = (ctx.bar.song_bar % 2) as u8;
        let both_bars = fig.iter().any(|f| f.0 == 0) && fig.iter().any(|f| f.0 == 1);
        let build = ctx.bar.song_bar % SECTION >= SECTION - 2;
        let (want, vol) = if shout && both_bars {
            // Brass call, saxes answer.
            (if half == 0 { Group::Brass } else { Group::Saxes }, 15)
        } else if shout {
            (Group::All, 15)
        } else if build && ctx.band.intro.is_none() {
            (Group::All, 14)
        } else {
            (Group::Saxes, 13)
        };
        let group = self.group(want)?;
        // Behind a soloist with the band held in the riff breathes: two bars on, two off (a
        // free band keeps it going, as do the Riffs chorus, a shout, an intro and a
        // section's last bars).
        let ostinato = ctx.freedom[0] >= 0.5 || ctx.band.chorus == Chorus::Riffs || shout || ctx.band.intro.is_some() || build;
        if !ostinato && ctx.bar.song_bar % 4 >= 2 {
            return None;
        }
        let vol = (vol + self.build()).min(15) as u8;
        let center = if group == Group::Saxes { 69 } else { 74 };
        let at = |f: &(u8, f64, f64, i32)| f.0 as f64 * 4.0 + f.1;
        for (i, f) in fig.iter().enumerate().filter(|(_, f)| f.0 == half) {
            let &(_, b, d, step) = f;
            if b >= bb - 1e-9 {
                continue;
            }
            let Some(h) = ctx.harm_at(b) else { continue };
            let d = d.min(bb - b + if half == 0 { bb } else { 0.25 });
            // The figure repeats: after its last note, the rest up to its first again.
            let next = fig.get(i + 1).map_or(8.0 + at(&fig[0]), at);
            let room = next - (at(f) + d);
            self.phrase(b, d, room >= 0.75, room, shout);
            let mut top = top_at(h.chord, center, step);
            if d >= 1.5 && b.fract() < 0.01 && (b as usize).is_multiple_of(2) {
                top = self.aim(&h, top, b);
            }
            let colour = Colour::of(ctx);
            if d >= 1.5 && colour.device != Device::Plain {
                // A long note opens up inside: the chorus's device.
                self.pad(out, group, vol, ctx.swing8(b), d, top, h.chord, colour.device);
            } else if d < 0.75 && colour.unison {
                self.unison(out, group, vol, b, d, top);
            } else {
                self.chord(out, group, vol, b, d, top, h.chord, false);
            }
        }
        Some(())
    }

    /// Answers in the line's rests and under its long notes (`line`: its notes this bar, as
    /// (beat, end)).
    fn answers(&self, out: &mut Vec<NoteEvent>, line: &[(f64, f64)], heard: &[(f64, f64, i32)], ahead: Option<LineAhead>, vol: u8) {
        let ctx = self.ctx;
        let bb = ctx.bb();
        let mut gaps: Vec<(f64, f64)> = Vec::new();
        let mut cursor = 0.0f64;
        let mut sorted = line.to_vec();
        sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
        for &(s, e) in &sorted {
            if s - cursor >= 1.5 {
                gaps.push((cursor, s));
            }
            if e - s >= 2.5 {
                gaps.push((s + 1.0, e.min(bb)));
            }
            cursor = cursor.max(e);
        }
        if bb - cursor >= 1.5 {
            gaps.push((cursor, bb));
        }
        // What the soloist has planned past the bar line ([`LineAhead`]): a phrase still
        // going is only breathing at the bar's end (leave it be, unless the whole bar's
        // open); a rest runs on to the next phrase's start, and an answer can ring into it.
        if let (Some(a), Some(last)) = (ahead, gaps.last_mut())
            && (last.1 - bb).abs() < 1e-9
        {
            match a.end {
                Some(_) if last.1 - last.0 < 3.0 => {
                    gaps.pop();
                }
                Some(_) => {}
                None => last.1 = bb + a.next_start.clamp(0.0, bb),
            }
        }
        // One answer a bar at most, in a real rest: room for the line to breathe too.
        for (i, &(g0, g1)) in gaps.iter().take(1).enumerate() {
            let start = ((g0 + 0.25) * 2.0).ceil() / 2.0;
            // (Notes start in this bar; only the last rings past the bar line.)
            let room = g1.min(bb + 1.0) - start;
            if room < 1.5 {
                continue;
            }
            // Call and response: the band answers with what it just heard, the line's last
            // figure in its rhythm (its contour, or turned upside down); else one of the
            // tune's own motifs; else a stock answer.
            let coin = unit(ctx.seed, ctx.bar.index, i as u64, 63);
            let fig = echo(heard, g0, room, coin < 0.3)
                .filter(|_| coin < 0.7)
                .or_else(|| (coin < 0.85).then(|| motif(&theme_at(ctx, ctx.bar.index as usize % 4), room)).flatten())
                .or_else(|| {
                    let fits: Vec<(usize, f64)> = ANSWERS.iter().enumerate().filter(|(_, a)| a.iter().map(|n| n.0 + n.1).fold(0.0, f64::max) <= room + 0.5).map(|(k, _)| (k, 1.0)).collect();
                    (!fits.is_empty()).then(|| ANSWERS[pick(unit(ctx.seed, ctx.bar.index, i as u64, 60), &fits)].to_vec())
                });
            let Some(fig) = fig else { continue };
            let want = if (ctx.bar.index as usize + i).is_multiple_of(2) { Group::Brass } else { Group::Saxes };
            let Some(group) = self.group(want) else { return };
            let center = if group == Group::Saxes { 69 } else { 74 };
            let vol = (vol as i32 + self.build()).min(14) as u8;
            let n = fig.len();
            for (j, &(b, d, step)) in fig.iter().enumerate() {
                let b = start + b;
                if b >= g1 - 0.25 || b >= bb {
                    continue;
                }
                let d = d.min(g1 - b - 0.1).max(0.3);
                let Some(h) = ctx.harm_at(b) else { continue };
                let last = j + 1 == n || fig.get(j + 1).is_some_and(|f| start + f.0 >= bb);
                self.phrase(b, d, last, if last { g1 - b - d } else { 0.0 }, false);
                let mut top = top_at(h.chord, center, step);
                if last && d >= 0.9 {
                    // Where the answer lands, aimed as the soloist aims a phrase's end.
                    top = self.aim(&h, top, b);
                }
                // Some answers start in unison and open into harmony.
                if j == 0 && n > 1 && unit(ctx.seed, ctx.bar.index, i as u64, 61) < 0.4 {
                    self.unison(out, group, vol, b, d, top);
                } else if last && d >= 1.5 && unit(ctx.seed, ctx.bar.index, i as u64, 62) < 0.4 {
                    self.pad(out, group, vol, ctx.swing8(b), d, top, h.chord, Device::Bloom);
                } else {
                    self.chord(out, group, vol, b, d, top, h.chord, false);
                }
            }
        }
    }

    /// Guide-tone pads, a long chord's top stepping down halfway through.
    fn pads(&self, out: &mut Vec<NoteEvent>) {
        let Some(group) = self.group(Group::Saxes) else { return };
        let vol = (if group == Group::Brass { 9 } else { 10 }) + self.build() as u8;
        let ctx = self.ctx;
        let device = Device::of(ctx);
        for (b, d, _, _) in changes(ctx) {
            let Some(h) = ctx.harm_at(b) else { continue };
            let top = top_at(h.chord, 69, 0);
            if d >= 4.0 - 1e-9 && device == Device::Plain {
                // Every pad swells into the next change, a long one's top stepping down.
                let half = (d / 2.0).floor();
                self.pad(out, group, vol, b, half - 0.1, top, h.chord, Device::Plain);
                self.pad(out, group, vol, b + half, d - half - 0.1, top_at(h.chord, 69, -1), h.chord, Device::Plain);
            } else {
                self.pad(out, group, vol, b, (d - 0.1).max(0.5), top, h.chord, device);
            }
        }
    }

    fn punches(&self, out: &mut Vec<NoteEvent>, tune: &[(f64, f64, i32)]) {
        let ctx = self.ctx;
        let plan = ctx.band;
        let mut at: Vec<f64> = (0..32).filter(|k| plan.hits & 1 << k != 0).map(|k| k as f64 / 4.0).filter(|&b| b < ctx.bb()).collect();
        if at.is_empty() {
            // The tune's own kicks: its pushed notes (an 8th ahead of a beat, held) punched
            // with it, as an ensemble kicks the melody's anticipations.
            at = tune.iter().filter(|n| (n.0.fract() - 0.5).abs() < 0.1 && n.1 >= 0.9).map(|n| n.0).take(2).collect();
        }
        if at.is_empty() {
            let figures: [&[f64]; 4] = [&[1.5], &[2.5], &[0.0, 1.5], &[3.5]];
            let f = figures[(unit(ctx.seed, ctx.bar.index, 0, 13) * 4.0) as usize % 4];
            at = f.iter().copied().filter(|&b| b < ctx.bb()).collect();
            if plan.hit_kind == HitKind::Stop {
                at = vec![0.0];
            }
        }
        for b in at {
            let Some(h) = ctx.harm_at(b) else { continue };
            let pcs = four(h.chord);
            let top = (70..=80).rev().find(|n| pcs.contains(&(n % 12))).unwrap_or(76);
            // (Already on the grid: the hits are where they sound.)
            let stop = plan.hit_kind == HitKind::Stop && unit(ctx.seed, ctx.bar.index, 0, 71) < 0.3;
            // On the beat a stab; ahead of it (an anticipation) leaned on; louder through
            // the section.
            let push = if b.fract() > 0.01 { 2 } else { 0 };
            self.art.set(Art { accent: push + self.build() as i8 - 1, len: 1.0, fall: if stop { -7 } else { 0 }, swell: 0 });
            self.chord_at(out, Group::All, 14, b, if stop { 1.0 } else { 0.4 }, top, h.chord);
        }
    }

    /// [`Self::chord`] at a beat already placed (no swing).
    fn chord_at(&self, out: &mut Vec<NoteEvent>, group: Group, vol: u8, b: f64, d: f64, top: i32, c: Chord) {
        let Some(group) = self.group(group) else { return };
        let brass = group != Group::Saxes;
        let saxes = group != Group::Brass;
        let top_tp = octave_into(top, 67, 84);
        if brass {
            self.play(out, Horn::Trumpets, vol, b, d, &self.brass_v(top_tp, c, false));
        }
        if saxes && self.saxes() {
            let v = self.sax_v(if self.brass() { top_tp - 12 } else { octave_into(top, 62, 77) }, c, false);
            let under = self.v.balance.saxes_under() + 1;
            let k = v.len().div_ceil(2);
            self.play(out, Horn::Altos, vol.saturating_sub(under), b, d, &v[..k]);
            self.play(out, Horn::Tenors, vol.saturating_sub(under), b, d, &v[k..]);
        }
    }

    /// The tune in the sections (soli: the saxes; tutti: everyone).
    fn tune(&self, out: &mut Vec<NoteEvent>, tune: &[(f64, f64, i32)], tutti: bool) {
        let ctx = self.ctx;
        let bb = ctx.bb();
        for (i, &(b, d, n)) in tune.iter().enumerate() {
            let Some(h) = ctx.harm_at(b) else { continue };
            let c = h.chord;
            let passing = d < 0.75 && b.fract() > 0.1;
            // Where the line's heading: the chord under its next note.
            let to = tune.get(i + 1).map_or(b + d, |x| x.0);
            self.next.set(ctx.harm_at(to).map(|h| h.chord).filter(|n| *n != c));
            // A phrase ends where the tune rests (unknown past the bar line: only inside it).
            let room = tune.get(i + 1).map_or(if b + d < bb - 0.75 { bb - b - d } else { 0.0 }, |x| x.0 - b - d);
            self.phrase(b, d, room >= 0.75, room, tutti);
            let colour = Colour::of(ctx);
            if tutti && d < 0.75 && colour.unison {
                // The fast notes in octaves, the long ones voiced: the line pops out.
                self.unison(out, Group::All, 13, b, d, octave_into(n, 67, 84));
                continue;
            }
            if tutti && d >= 1.5 && room >= 0.75 {
                match colour.ending {
                    Ending::Wedge => {
                        self.wedge(out, Group::All, 13, b, d, octave_into(n, 67, 84), c);
                        continue;
                    }
                    Ending::Bloom => {
                        self.pad(out, Group::All, 13, ctx.swing8(b), d, octave_into(n, 67, 84), c, Device::Bloom);
                        continue;
                    }
                    Ending::Plain => {}
                }
            }
            if !tutti {
                // (Soli up front: the section has the tune.)
                let v = self.sax_v(octave_into(n, 62, 77), c, passing);
                let k = v.len().div_ceil(2);
                self.play(out, Horn::Altos, 15, b, d, &v[..k]);
                self.play(out, Horn::Tenors, 14, b, d, &v[k..]);
            } else {
                let top = octave_into(n, 67, 84);
                let v = self.brass_v(top, c, passing);
                self.play(out, Horn::Trumpets, 13, b, d, &v);
                let s = if self.v.style == Style::Drop2 && v.len() == 4 { voicing::finish(&self.v, drop2([v[0], v[1], v[2], v[3]].map(|x| x - 12)), c) } else { self.sax_v(top - 12, c, passing) };
                let sx = 13 - self.v.balance.saxes_under();
                let k = s.len().div_ceil(2);
                self.play(out, Horn::Altos, sx, b, d, &s[..k]);
                self.play(out, Horn::Tenors, sx, b, d, &s[k..]);
            }
        }
    }
}

/// The upper sections' bar: the section's plan played, the line moved onto the horn that has
/// it. Returns the plan (`None`: no upper sections on the stand).
#[allow(clippy::too_many_arguments)]
pub fn write_upper(ctx: &Ctx, horns: Horns, force: Option<UpperRole>, force_melody: Option<Melody>, force_v: voicing::Force, ahead: Option<LineAhead>, band: &mut Vec<NoteEvent>, out: &mut Vec<NoteEvent>) -> Option<Section> {
    let upper = Horns::NONE.with(Horn::Trumpets).with(Horn::Altos).with(Horn::Tenors);
    let mine = horns.iter().filter(|h| upper.has(*h)).fold(Horns::NONE, Horns::with);
    if mine.is_empty() {
        return None;
    }
    let mut sec = plan(ctx, mine, force, force_melody);
    let plan_ = ctx.band;
    // The written tune (soli and tutti voice it).
    let mut tune: Vec<(f64, f64, i32)> = Vec::new();
    let mut written = Vec::with_capacity(64);
    ctx.written(0, &mut written);
    for w in &written {
        let Sound::Note(n) = w.sound else { continue };
        let (b, d) = (ctx.rel(w), ctx.len(w));
        match tune.last_mut() {
            Some(last) if w.tie && last.2 == n as i32 => last.1 = b + d - last.0,
            _ => tune.push((b, d, n as i32)),
        }
    }
    if matches!(sec.role, UpperRole::Soli | UpperRole::Tutti) && tune.is_empty() {
        sec.role = UpperRole::Pads;
    }
    // The line as played this bar (where answers go).
    let line: Vec<(f64, f64)> = band.iter().filter(|e| e.ch == 0 && !matches!(e.sound, Sound::Drum(_))).map(|e| (ctx.rel(e), ctx.rel(e) + ctx.len(e))).collect();
    // Its notes (what the answers echo).
    let mut heard: Vec<(f64, f64, i32)> = band
        .iter()
        .filter(|e| e.ch == 0)
        .filter_map(|e| match e.sound {
            Sound::Note(n) => Some((ctx.rel(e), ctx.len(e), n as i32)),
            Sound::Arp(a) => a.notes().iter().max().map(|&n| (ctx.rel(e), ctx.len(e), n as i32)),
            _ => None,
        })
        .collect();
    heard.sort_by(|a, b| a.0.total_cmp(&b.0));
    // The line onto the horn that has it.
    let mut free = mine;
    if let Some(h) = sec.melody.horn() {
        free = Horns::NONE;
        for x in mine.iter().filter(|&x| x != h) {
            free = free.with(x);
        }
        let (lo, hi) = h.range();
        let fit = |n: u8| octave_into(n as i32 + sec.melody.shift(), lo, hi) as u8;
        for e in band.iter_mut().filter(|e| e.ch == 0) {
            e.sound = match e.sound {
                Sound::Note(n) => Sound::Note(fit(n)),
                // (One horn, one note: an arpeggio's top.)
                Sound::Arp(a) => a.notes().iter().max().map_or(e.sound, |&n| Sound::Note(fit(n))),
                s => s,
            };
            e.ch = h.ch();
            e.inst = 0;
            e.anchor = 0;
            e.gain *= sec.melody.level();
        }
    }
    if !plan_.chorus.lead_solos(ctx.freedom[0]) {
        shape_line(ctx, band);
    }
    if plan_.tacet || plan_.solo_break {
        return Some(sec);
    }
    let v = voicing_of(ctx, force_v);
    let w = Writer::new(ctx, free, v);
    match sec.role {
        UpperRole::Soli => {
            if ctx.band.soli.is_some_and(|s| s.texture == super::soli::Texture::Locked) && free.has(Horn::Altos) {
                // The quartet's locked section is block chords under the tune: the section
                // voices it the chorus's way (the comp's and the tenor's lines drop out).
                band.retain(|e| !matches!((e.ch, e.sound), (1 | 4, Sound::Note(_))));
                w.tune(out, &tune, false);
            } else if ctx.band.soli.is_some() && free.has(Horn::Altos) {
                // The quartet's soli, orchestrated: the tune and the comp's alto line on the
                // altos, its tenor line on the tenors (the counterpoint as written: front line,
                // canon, chorale...), each a player, up front.
                saxes_take_the_soli(band, free);
            } else {
                w.tune(out, &tune, false);
            }
            let held: Vec<(f64, f64)> = tune.iter().map(|&(b, d, _)| (b, b + d)).collect();
            let brass = Writer::new(ctx, if free.has(Horn::Trumpets) { Horns::NONE.with(Horn::Trumpets) } else { Horns::NONE }, v);
            brass.answers(out, &held, &tune, None, 13);
            band.retain(|e| e.ch != 0);
        }
        UpperRole::Tutti => {
            w.tune(out, &tune, true);
            band.retain(|e| e.ch != 0);
        }
        UpperRole::Pads => w.pads(out),
        UpperRole::Punches => w.punches(out, &tune),
        UpperRole::Riff => {
            w.riff(out, &figure(ctx), plan_.chorus == Chorus::Shout);
        }
        UpperRole::Answers => w.answers(out, &line, &heard, ahead, 13),
        UpperRole::Tacet => {}
    }
    piano_in_the_holes(ctx, band, out);
    Some(sec)
}

/// The pianist in a big band "ignores the horn figures and plays in the holes": a comp
/// chord struck with a section's chord or under one held drops out (its single notes, a
/// line, stay). Two chord instruments in one register is mud, and the horns' voicings are
/// what the arrangement is for.
fn piano_in_the_holes(ctx: &Ctx, band: &mut Vec<NoteEvent>, out: &[NoteEvent]) {
    let horns: Vec<(f64, f64)> = out.iter().filter(|e| matches!(e.sound, Sound::Arp(_)) && Horn::from_ch(e.ch).is_some_and(|h| !h.is_low() && h != Horn::Guitar)).map(|e| (ctx.rel(e), ctx.rel(e) + ctx.len(e))).collect();
    if horns.is_empty() {
        return;
    }
    band.retain(|e| {
        if e.ch != 1 || !matches!(e.sound, Sound::Arp(_)) {
            return true;
        }
        let b = ctx.rel(e);
        !horns.iter().any(|&(s, end)| (b - s).abs() < 0.3 || (b > s && b < end - 0.1))
    });
}

/// The tune played like a horn player plays it, not as written: louder up high, softer down
/// low; an 8th on the and leaned on and the one before it ghosted; a long note leaned into;
/// a phrase's last short note let go; the 8-bar section rising to its sixth and seventh
/// bars. (The line's notes this bar, wherever it went; a soloist shapes their own.)
fn shape_line(ctx: &Ctx, band: &mut [NoteEvent]) {
    let line_ch = [0, Horn::Trumpets.ch(), Horn::Altos.ch(), Horn::Tenors.ch()];
    let mut idx: Vec<usize> = (0..band.len()).filter(|&i| line_ch.contains(&band[i].ch) && matches!(band[i].sound, Sound::Note(_))).collect();
    if idx.is_empty() {
        return;
    }
    idx.sort_by(|&a, &b| band[a].start.cmp(&band[b].start));
    let pitch = |e: &NoteEvent| if let Sound::Note(n) = e.sound { n as f64 } else { 0.0 };
    let (lo, hi) = idx.iter().fold((f64::MAX, f64::MIN), |(l, h), &i| (l.min(pitch(&band[i])), h.max(pitch(&band[i]))));
    let mid = (lo + hi) / 2.0;
    let arc = [-1, -1, 0, 0, 0, 1, 1, 0][ctx.bar.song_bar % SECTION];
    let starts: Vec<f64> = idx.iter().map(|&i| ctx.rel(&band[i])).collect();
    for (k, &i) in idx.iter().enumerate() {
        let e = &band[i];
        let (b, d) = (starts[k], ctx.len(e));
        let next = starts.get(k + 1).copied().unwrap_or(f64::MAX);
        let room = next - (b + d);
        let off = (b.fract() - 0.5).abs() < 0.1 || (b.fract() - 0.67).abs() < 0.05;
        let on_before_and = b.fract() < 0.05 && d <= 0.7 && (next - b - 0.5).abs() < 0.2;
        let mut v = arc;
        // Up high, louder (a horn has to push to get there).
        v += ((pitch(e) - mid) / 4.0).round().clamp(-2.0, 2.0) as i32;
        if off && d <= 0.7 {
            v += 1;
        }
        if on_before_and {
            v -= 2;
        }
        if d >= 1.5 {
            v += 1;
        }
        if d < 0.75 && room >= 1.0 {
            v -= 1;
        }
        let e = &mut band[i];
        e.volume = (e.volume as i32 - 1 + v).clamp(2, 15) as u8;
        if d >= 1.5 && e.fx.swell == 0 && e.fx.fall == 0 {
            e.fx.swell = 6;
        }
    }
}

/// The quartet's soli ([`super::soli`]) onto the saxes: the lead's tune and the comp's alto
/// line (its single notes; the piano keeps its chords) to the altos, the third pulse's tenor
/// line to the tenors (or the altos, with no tenors). Lines, not chords: each at a soloist's
/// level, and no slides (a section shares its channel's bend).
fn saxes_take_the_soli(band: &mut [NoteEvent], free: Horns) {
    let tenors = if free.has(Horn::Tenors) { Horn::Tenors } else { Horn::Altos };
    for e in band.iter_mut() {
        let to = match (e.ch, e.sound) {
            (0, Sound::Note(_) | Sound::Arp(_)) => (Horn::Altos, Melody::Alto.level()),
            (1, Sound::Note(_)) => (Horn::Altos, Melody::Alto.level() * 0.85),
            (4, Sound::Note(_)) => (tenors, Melody::Tenor.level() * 1.6),
            _ => continue,
        };
        let (h, level) = to;
        let (lo, hi) = h.range();
        e.sound = match e.sound {
            Sound::Arp(a) => Sound::Note(a.notes().iter().copied().max().unwrap_or(60)),
            s => s,
        };
        if let Sound::Note(n) = e.sound {
            e.sound = Sound::Note(octave_into(n as i32, lo, hi) as u8);
        }
        e.ch = h.ch();
        e.inst = 0;
        e.anchor = 0;
        e.gain *= level;
        e.volume = e.volume.max(10);
        e.fx.slide = 0;
        e.fx.slide_frames = 0;
    }
}

/// What a section's pads do inside the chord ([`Writer::pad`]): held plain; a bloom (from a
/// unison out to the voicing, contrary motion); a pyramid (the voices entering bottom up); a
/// line cliché in an inner voice; a suspended fourth resolving to the third. One per section.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Device {
    Plain,
    Bloom,
    Pyramid,
    Cliche,
    Suspension,
}

impl Device {
    fn of(ctx: &Ctx) -> Device {
        let x = unit(ctx.seed, ctx.bar.pass, (ctx.bar.song_bar / SECTION) as u64, 80);
        pick(x, &[(Device::Plain, 0.2), (Device::Bloom, 0.25), (Device::Pyramid, 0.2), (Device::Cliche, 0.2), (Device::Suspension, 0.15)])
    }
}

/// The chorus's voicing ([`voicing`]), rolled once a pass (`force`: overrides): the shape
/// (drop 2 most often, the interval-built ones more as the freedom rises), the passing chords
/// (any chord that fits, more often the freer the band), and the balance between the voices
/// (the denser the shape, the more the lead stands over shaded inner voices or the bottom
/// carries the weight).
pub fn voicing_of(ctx: &Ctx, force: voicing::Force) -> Voicing {
    let c = |what: u64| unit(ctx.seed, ctx.bar.pass, 0, what);
    let f = ctx.freedom[0].clamp(0.0, 1.0) as f64;
    let style = force.style.unwrap_or_else(|| {
        pick(
            c(100),
            &[
                (Style::Drop2, 0.45 - 0.25 * f),
                (Style::Drop24, 0.1),
                (Style::Spread, 0.1),
                (Style::Quartal, 0.05 + 0.08 * f),
                (Style::Fifths, 0.05 + 0.08 * f),
                (Style::Cluster, 0.02 + 0.05 * f),
                (Style::Upper, 0.05 + 0.08 * f),
                (Style::Theme, 0.05 + 0.06 * f),
                (Style::Power, 0.03 + 0.07 * f),
            ],
        )
    });
    let passing = force.passing.unwrap_or_else(|| pick(c(101), &[(Passing::Dim, 0.5 - 0.3 * f), (Passing::Planing, 0.25), (Passing::Plain, 0.15), (Passing::Free, 0.05 + 0.4 * f), (Passing::Harris, 0.2), (Passing::Lean, 0.1 + 0.2 * f)]));
    let dense = matches!(style, Style::Cluster | Style::Fifths | Style::Quartal | Style::Upper | Style::Power) || passing == Passing::Free;
    let balance = force.balance.unwrap_or_else(|| {
        if dense {
            pick(c(102), &[(Balance::Lead, 0.5), (Balance::Pyramid, 0.35), (Balance::Top, 0.15)])
        } else {
            pick(c(102), &[(Balance::Top, 0.5), (Balance::Lead, 0.2), (Balance::Pyramid, 0.2), (Balance::Even, 0.1)])
        }
    });
    // The theme: the tune's opening notes.
    let notes: Vec<i32> = ctx.arrangement.tracks[0]
        .events
        .iter()
        .filter_map(|e| match e.kind {
            EventKind::Note(n) => Some(n as i32),
            _ => None,
        })
        .take(12)
        .collect();
    // Four voices mostly; three or two more often the denser the shape (a section doesn't
    // fill every gap).
    let voices = force.voices.unwrap_or_else(|| if dense { pick(c(103), &[(4u8, 0.5), (3, 0.35), (2, 0.15)]) } else { pick(c(103), &[(4u8, 0.75), (3, 0.2), (2, 0.05)]) });
    Voicing { style, passing, balance, theme: voicing::theme_cell(&notes), salt: ctx.seed ^ ctx.bar.pass.wrapping_mul(0x5851_F42D_4C95_7F2D), voices, borrow: 0.1 + 0.5 * f as f32, next: None }
}

/// How a phrase's last long note ends in a tutti.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ending {
    Plain,
    Wedge,
    Bloom,
}

/// A chorus's colour, rolled once a pass: its riffs' long notes opening by a [`Device`], fast
/// notes in unison (octaves) or voiced, how a tutti phrase ends.
#[derive(Clone, Copy, Debug)]
struct Colour {
    device: Device,
    unison: bool,
    ending: Ending,
}

impl Colour {
    fn of(ctx: &Ctx) -> Colour {
        let c = |what: u64| unit(ctx.seed, ctx.bar.pass, 0, what);
        let device = pick(c(90), &[(Device::Plain, 0.3), (Device::Bloom, 0.2), (Device::Pyramid, 0.15), (Device::Cliche, 0.2), (Device::Suspension, 0.15)]);
        let ending = pick(c(92), &[(Ending::Plain, 0.4), (Ending::Wedge, 0.3), (Ending::Bloom, 0.3)]);
        Colour { device, unison: c(91) < 0.5, ending }
    }
}

