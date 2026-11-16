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
use super::horns::{Horn, Horns, SECTION, changes, close, drop2, four, unit};
use super::musician::Ctx;
use super::soloist::Soloist;
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
                    pick(c(s, 31), &[(Tacet, 0.7), (Pads, 0.3)])
                } else if last {
                    pick(c(s, 32), &[(Riff, 0.6), (Answers, 0.4)])
                } else if s == 1 {
                    pick(c(s, 33), &[(Pads, 0.5), (Answers, 0.5)])
                } else {
                    pick(c(s, 34), &[(Riff, 0.5), (Answers, 0.3), (Pads, 0.2)])
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
        Chorus::Breaks => line(pick(c(s, 36), &[(Pads, 0.4), (Punches, 0.3), (Answers, 0.3)])),
        Chorus::Strolling => line(Tacet),
        _ => head(ctx, horns, s, n),
    }
}

/// A head: the tune passed round the band, section by section.
fn head(ctx: &Ctx, horns: Horns, s: usize, n: usize) -> Section {
    use UpperRole::*;
    let c = |k: usize, what: u64| unit(ctx.seed, ctx.bar.pass, k as u64, what);
    let (tp, al, te) = (horns.has(Horn::Trumpets), horns.has(Horn::Altos), horns.has(Horn::Tenors));
    let two = ctx.band.chorus == Chorus::TwoFeel;
    let mut prev = Take::Line(Melody::Lead);
    let mut out = Section { role: Pads, melody: Melody::Lead };
    for k in 0..=s {
        let take = if k == 0 {
            Take::Line(Melody::Lead)
        } else if k + 1 >= n {
            let tutti = if ctx.bar.pass == 0 { 0.35 } else { 0.5 };
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
            pick(c(k, 41), &opts)
        };
        prev = take;
        out = match take {
            Take::Soli => Section { role: Soli, melody: Melody::Lead },
            Take::Tutti => Section { role: Tutti, melody: Melody::Lead },
            Take::Line(melody) => {
                let role = if two || (k == 0 && ctx.bar.pass == 0) {
                    pick(c(k, 42), &[(Tacet, 0.35), (Pads, 0.45), (Answers, 0.2)])
                } else {
                    pick(c(k, 43), &[(Pads, 0.35), (Answers, 0.4), (Riff, 0.25)])
                };
                Section { role, melody }
            }
        };
    }
    out
}

/// The chorus's riff: one of [`FIGURES`], or the tune's theme (more often in a shout and always
/// in the intro).
pub fn figure(ctx: &Ctx) -> Figure {
    let x = unit(ctx.seed, ctx.bar.pass, 0, 50);
    let theme_odds = if ctx.band.intro.is_some() {
        1.0
    } else if ctx.band.chorus == Chorus::Shout {
        0.6
    } else {
        0.25
    };
    if x < theme_odds {
        let t = theme(ctx);
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
    let arr = ctx.arrangement;
    let bb = ctx.bb();
    let tr = &arr.tracks[0];
    let bars = arr.bar_events[0].len();
    for start in 0..bars.saturating_sub(1).min(8) {
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
    fn new(ctx: &'a Ctx<'b>, free: Horns) -> Self {
        Writer { ctx, free, art: std::cell::Cell::new(Art::PLAIN) }
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
        [0, 0, 0, 0, 1, 1, 2, 2][self.ctx.bar.song_bar % SECTION]
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
        t.volume = (vol as i32 + a.accent as i32).clamp(1, 15) as u8;
        let d = (d * a.len).max(0.2);
        let mut e = self.ctx.make(&t, b, d, Sound::Arp(crate::mml::Arp::new(&ns)));
        e.fx.swell = a.swell;
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
        let (s, d) = (self.ctx.swing8(b), d);
        let brass = group != Group::Saxes && self.brass();
        let saxes = group != Group::Brass && self.saxes();
        let top_tp = octave_into(top, 67, 82);
        if brass {
            self.play(out, Horn::Trumpets, vol, s, d, &close(top_tp, c, passing));
        }
        if saxes {
            let top_sx = if brass { top_tp - 12 } else { octave_into(top, 62, 76) };
            let v = drop2(close(top_sx, c, passing));
            match (self.free.has(Horn::Altos), self.free.has(Horn::Tenors)) {
                (true, true) => {
                    self.play(out, Horn::Altos, vol.saturating_sub(1), s, d, &v[..2]);
                    self.play(out, Horn::Tenors, vol.saturating_sub(1), s, d, &v[2..]);
                }
                (true, false) => self.play(out, Horn::Altos, vol.saturating_sub(1), s, d, &close(top_sx, c, passing)),
                (false, true) => self.play(out, Horn::Tenors, vol.saturating_sub(1), s, d, &close(top_sx - 5, c, passing)),
                _ => {}
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
            (if half == 0 { Group::Brass } else { Group::Saxes }, 13)
        } else if shout {
            (Group::All, 13)
        } else if build && ctx.band.intro.is_none() {
            (Group::All, 12)
        } else {
            (Group::Saxes, 11)
        };
        let group = self.group(want)?;
        let vol = (vol + self.build()).min(14) as u8;
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
            self.chord(out, group, vol, b, d, top_at(h.chord, center, step), h.chord, false);
        }
        Some(())
    }

    /// Answers in the line's rests and under its long notes (`line`: its notes this bar, as
    /// (beat, end)).
    fn answers(&self, out: &mut Vec<NoteEvent>, line: &[(f64, f64)], vol: u8) {
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
        for (i, &(g0, g1)) in gaps.iter().take(2).enumerate() {
            let start = ((g0 + 0.25) * 2.0).ceil() / 2.0;
            let room = g1 - start;
            if room < 1.0 {
                continue;
            }
            let fits: Vec<(usize, f64)> = ANSWERS.iter().enumerate().filter(|(_, a)| a.iter().map(|n| n.0 + n.1).fold(0.0, f64::max) <= room + 0.5).map(|(k, _)| (k, 1.0)).collect();
            if fits.is_empty() {
                continue;
            }
            let k = pick(unit(ctx.seed, ctx.bar.index, i as u64, 60), &fits);
            let want = if (ctx.bar.index as usize + i).is_multiple_of(2) { Group::Brass } else { Group::Saxes };
            let Some(group) = self.group(want) else { return };
            let center = if group == Group::Saxes { 69 } else { 74 };
            let vol = (vol as i32 + self.build()).min(14) as u8;
            let n = ANSWERS[k].len();
            for (j, &(b, d, step)) in ANSWERS[k].iter().enumerate() {
                let b = start + b;
                if b >= g1 - 0.25 || b >= bb {
                    continue;
                }
                let d = d.min(g1 - b - 0.1).max(0.3);
                let Some(h) = ctx.harm_at(b) else { continue };
                let last = j + 1 == n;
                self.phrase(b, d, last, if last { g1 - b - d } else { 0.0 }, false);
                self.chord(out, group, vol, b, d, top_at(h.chord, center, step), h.chord, false);
            }
        }
    }

    /// Guide-tone pads, a long chord's top stepping down halfway through.
    fn pads(&self, out: &mut Vec<NoteEvent>) {
        let Some(group) = self.group(Group::Saxes) else { return };
        let vol = (if group == Group::Brass { 9 } else { 10 }) + self.build() as u8;
        // Every pad swells into the next change.
        self.art.set(Art { swell: 6, len: 1.0, ..Art::PLAIN });
        for (b, d, _, _) in changes(self.ctx) {
            let Some(h) = self.ctx.harm_at(b) else { continue };
            if d >= 4.0 - 1e-9 {
                let half = (d / 2.0).floor();
                self.chord(out, group, vol, b, half - 0.1, top_at(h.chord, 69, 0), h.chord, false);
                self.chord(out, group, vol, b + half, d - half - 0.1, top_at(h.chord, 69, -1), h.chord, false);
            } else {
                self.chord(out, group, vol, b, (d - 0.1).max(0.5), top_at(h.chord, 69, 0), h.chord, false);
            }
        }
    }

    fn punches(&self, out: &mut Vec<NoteEvent>) {
        let ctx = self.ctx;
        let plan = ctx.band;
        let mut at: Vec<f64> = (0..32).filter(|k| plan.hits & 1 << k != 0).map(|k| k as f64 / 4.0).filter(|&b| b < ctx.bb()).collect();
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
            self.art.set(Art { accent: 1, len: 1.0, fall: if stop { -7 } else { 0 }, swell: 0 });
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
            self.play(out, Horn::Trumpets, vol, b, d, &close(top_tp, c, false));
        }
        if saxes && self.saxes() {
            let v = drop2(close(if self.brass() { top_tp - 12 } else { octave_into(top, 62, 77) }, c, false));
            self.play(out, Horn::Altos, vol.saturating_sub(2), b, d, &v[..2]);
            self.play(out, Horn::Tenors, vol.saturating_sub(2), b, d, &v[2..]);
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
            // A phrase ends where the tune rests (unknown past the bar line: only inside it).
            let room = tune.get(i + 1).map_or(if b + d < bb - 0.75 { bb - b - d } else { 0.0 }, |x| x.0 - b - d);
            self.phrase(b, d, room >= 0.75, room, tutti);
            if !tutti {
                let v = drop2(close(octave_into(n, 62, 77), c, passing));
                self.play(out, Horn::Altos, 12, b, d, &v[..2]);
                self.play(out, Horn::Tenors, 12, b, d, &v[2..]);
            } else {
                let v = close(octave_into(n, 67, 84), c, passing);
                self.play(out, Horn::Trumpets, 13, b, d, &v);
                let s = drop2(v.map(|x| x - 12));
                self.play(out, Horn::Altos, 12, b, d, &s[..2]);
                self.play(out, Horn::Tenors, 12, b, d, &s[2..]);
            }
        }
    }
}

/// The upper sections' bar: the section's plan played, the line moved onto the horn that has
/// it. Returns the plan (`None`: no upper sections on the stand).
pub fn write_upper(ctx: &Ctx, horns: Horns, force: Option<UpperRole>, force_melody: Option<Melody>, band: &mut Vec<NoteEvent>, out: &mut Vec<NoteEvent>) -> Option<Section> {
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
    if plan_.tacet || plan_.solo_break {
        return Some(sec);
    }
    let w = Writer::new(ctx, free);
    match sec.role {
        UpperRole::Soli => {
            w.tune(out, &tune, false);
            let held: Vec<(f64, f64)> = tune.iter().map(|&(b, d, _)| (b, b + d)).collect();
            let brass = Writer::new(ctx, if free.has(Horn::Trumpets) { Horns::NONE.with(Horn::Trumpets) } else { Horns::NONE });
            brass.answers(out, &held, 12);
            band.retain(|e| e.ch != 0);
        }
        UpperRole::Tutti => {
            w.tune(out, &tune, true);
            band.retain(|e| e.ch != 0);
        }
        UpperRole::Pads => w.pads(out),
        UpperRole::Punches => w.punches(out),
        UpperRole::Riff => {
            w.riff(out, &figure(ctx), plan_.chorus == Chorus::Shout);
        }
        UpperRole::Answers => w.answers(out, &line, 12),
        UpperRole::Tacet => {}
    }
    Some(sec)
}

