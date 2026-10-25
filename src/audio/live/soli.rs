//! The soli chorus, in four voices. Music only: no physics, no director.
//!
//! The lead plays the tune as written (pulse 1); the comp writes three more lines under it:
//! the alto (pulse 2), the tenor (pulse 3, a VRC6-style extra pulse that only sounds in a
//! soli) and, in most textures, the bass (the triangle; otherwise the bass walks). Each 8-bar
//! section of the chorus has its own [`Texture`], picked from the seed and the pass, so no two
//! solis are alike, and the chorus builds: the first section in three voices (the tenor
//! joins in the second), a little softer, the last section's second half the locked soli
//! voicing, louder ([`plan`]).
//!
//! | Texture | Alto | Tenor | Bass |
//! |---|---|---|---|
//! | Front line | a clarinet obbligato: runs while the tune holds, long notes while it moves | a trombone: guide tones, smeared into | two-beat (root, fifth) |
//! | Canon | the tune a bar later (two beats in the section's last two bars: stretto), an octave down, bent to the chords | guide tones | walks |
//! | Chorale | guide tones, suspended over the bar line | the other guide tones | half notes against the tune |
//! | Answers | a background figure behind the busy tune, and in each gap an answer: its last motif (inverted or moved a step) | under both | walks |
//! | Line cliché | a long chord tone | 5 ♯5 6 ♭7 (minor: 1 7 ♭7 6) under a long chord | a pedal |
//! | Locked | drop-2 under every tune note | (a diminished chord under a passing note) | hits with the line |
//!
//! Every free note is chosen against the others ([`pick`]): consonant on the beat, no
//! parallel fifths or octaves, contrary motion and steps preferred, sevenths falling, each
//! voice in its register and under the one above it.

use super::chorus::in_section;
use super::ornament::Harm;
use crate::audio::accomp::Rng;
use crate::audio::chart::{Chord, Family};

/// How the voices share a section of the soli.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Texture {
    /// New Orleans: clarinet runs over, trombone under, tuba in two.
    #[default]
    FrontLine,
    /// The alto imitates the tune.
    Canon,
    /// Slow guide-tone lines with suspensions, the bass against the tune.
    Chorale,
    /// Call and response: the inner voices answer the tune's gaps.
    Answers,
    /// A chromatic inner line under long chords.
    Cliche,
    /// The big-band soli voicing: drop-2 under the tune.
    Locked,
}

impl Texture {
    /// The textures a section can roll (the locked voicing is the climax's).
    pub const ROLLED: [Texture; 5] = [Texture::FrontLine, Texture::Canon, Texture::Chorale, Texture::Answers, Texture::Cliche];

    pub fn label(self) -> &'static str {
        match self {
            Texture::FrontLine => "front line",
            Texture::Canon => "canon",
            Texture::Chorale => "chorale",
            Texture::Answers => "answers",
            Texture::Cliche => "line cliche",
            Texture::Locked => "locked",
        }
    }

    /// Does the soli write the bass (else it walks)?
    pub fn writes_bass(self) -> bool {
        !matches!(self, Texture::Canon | Texture::Answers)
    }
}

/// A bar of a soli chorus: its section's texture and where it sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SoliBar {
    pub texture: Texture,
    /// The section (of 8 bars) and the bar in it, of `len`.
    pub section: u8,
    pub bar: u8,
    pub len: u8,
    /// The tenor plays (from the second section on).
    pub tenor: bool,
    /// A canon's entries two beats apart instead of a bar.
    pub stretto: bool,
    /// The chorus's last section (its climax).
    pub last: bool,
}

impl SoliBar {
    /// How much louder (or softer) the band plays the bar.
    pub fn swell(&self) -> f32 {
        if self.texture == Texture::Locked {
            0.04
        } else if self.section == 0 && !self.last {
            -0.06
        } else {
            0.0
        }
    }
}

/// The soli plan for `song_bar` of pass `pass` (a song `bars` long).
pub fn plan(seed: u64, pass: u64, song_bar: usize, bars: usize) -> SoliBar {
    let (k, n) = in_section(song_bar, bars);
    let section = song_bar / 8;
    let sections = bars.div_ceil(8).max(1);
    let last = section + 1 >= sections;
    // A shuffle of the textures for the pass, the first section never the canon's (it has
    // nothing to imitate yet but the head's last bar).
    let mut order = Texture::ROLLED;
    let mut r = super::band::rng(seed, 11, pass, 0x5011);
    for i in (1..order.len()).rev() {
        order.swap(i, r.below(i + 1));
    }
    if order[0] == Texture::Canon {
        order.swap(0, 1);
    }
    let mut texture = order[section % order.len()];
    if last && n >= 4 && k >= n / 2 {
        texture = Texture::Locked;
    }
    let tenor = section > 0 || sections == 1 || texture == Texture::Locked;
    let stretto = texture == Texture::Canon && n >= 4 && k + 2 >= n;
    SoliBar { texture, section: section as u8, bar: k as u8, len: n as u8, tenor, stretto, last }
}

/// A note of a line: its beat from the bar line, its length in beats, its pitch; `tie`: held
/// on from the note before (no attack); `slide`: smeared up into from this many semitones
/// under.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tone {
    pub b: f64,
    pub d: f64,
    pub n: u8,
    pub tie: bool,
    pub slide: i8,
}

impl Tone {
    fn new(b: f64, d: f64, n: u8) -> Tone {
        Tone { b, d, n, tie: false, slide: 0 }
    }

    fn end(&self) -> f64 {
        self.b + self.d
    }
}

/// What the soli writer knows about a bar.
pub struct Bar<'a> {
    pub plan: SoliBar,
    /// Beats in the bar.
    pub bb: f64,
    /// The tune this bar, and the bar before (for the canon and the answers).
    pub lead: &'a [Tone],
    pub prev_lead: &'a [Tone],
    /// The harmony at a beat from the bar line (may run into the next bar).
    pub harm: &'a dyn Fn(f64) -> Option<Harm>,
    /// Where an 8th written at a beat starts (swung).
    pub swing: &'a dyn Fn(f64) -> f64,
}

/// The answers' background figures behind a busy tune (beat, length; the off-beats swung):
/// Charleston pads, off-beat punches, a long-short-long.
const BACKGROUNDS: [&[(f64, f64)]; 3] = [
    &[(0.0, 1.4), (1.5, 0.45), (2.5, 1.4)],
    &[(0.5, 0.35), (1.5, 0.35), (2.5, 0.35), (3.5, 0.4)],
    &[(0.0, 0.9), (1.5, 1.0), (3.0, 0.9)],
];

/// The voices' registers.
pub const ALTO: (i32, i32) = (55, 84);
pub const TENOR: (i32, i32) = (48, 67);
pub const BASS: (i32, i32) = (31, 52);

const A: usize = 0;
const T: usize = 1;
const B: usize = 2;

/// Writes the soli's alto, tenor and bass, bar by bar (it remembers where each voice was).
pub struct Writer {
    pub alto: Vec<Tone>,
    pub tenor: Vec<Tone>,
    pub bass: Vec<Tone>,
    last: [Option<u8>; 3],
    prev_harm: Option<Harm>,
    /// The clarinet's running direction.
    run: i32,
    /// A line cliché going on: its chord and how far along.
    cliche: Option<(Chord, usize)>,
    changes: Vec<f64>,
}

impl Default for Writer {
    fn default() -> Self {
        Writer::new()
    }
}

/// The note sounding at beat `b` of a line.
fn at(line: &[Tone], b: f64) -> Option<u8> {
    line.iter().rev().find(|t| t.b <= b + 1e-6 && b < t.end() - 1e-6).map(|t| t.n)
}

/// The note sounding just before beat `b` (or `fallback`, the voice's last note).
fn before(line: &[Tone], b: f64, fallback: Option<u8>) -> Option<u8> {
    line.iter().rev().find(|t| t.b < b - 1e-6).map(|t| t.n).or(fallback)
}

fn push(v: &mut Vec<Tone>, t: Tone) {
    if v.len() < v.capacity() {
        v.push(t);
    }
}

/// Each note lasts until the next one starts (or the bar's end), at most `max` beats.
fn legato(v: &mut [Tone], bb: f64, max: f64) {
    for i in 0..v.len() {
        let next = if i + 1 < v.len() { v[i + 1].b } else { bb };
        v[i].d = (next - v[i].b).min(max).max(0.05);
    }
}

/// Repeated notes held instead (a common tone kept is good voice leading).
fn merge(v: &mut Vec<Tone>) {
    let mut w = 0;
    for i in 0..v.len() {
        if w > 0 && v[w - 1].n == v[i].n && (v[w - 1].end() - v[i].b).abs() < 1e-6 && v[i].slide == 0 {
            v[w - 1].d += v[i].d;
        } else {
            v[w] = v[i];
            w += 1;
        }
    }
    v.truncate(w);
}

/// The 4th note a plain triad gets in four parts: the 6th on a major one, the 7th on a minor
/// one.
fn added(h: &Harm) -> Option<u8> {
    match h.chord.quality.intervals().len() {
        3 => match h.chord.family() {
            Family::Major => Some(9),
            Family::Minor => Some(10),
            _ => None,
        },
        _ => None,
    }
}

/// Is `n` a chord tone of `h` (a triad's added 6th or 7th too)?
pub fn is_tone(h: &Harm, n: u8) -> bool {
    h.is_chord_tone(n) || added(h).is_some_and(|a| (h.chord.root + a) % 12 == n % 12)
}

/// Is `n` a guide tone (the 3rd, or the 7th or 6th) of `h`?
pub fn is_guide(h: &Harm, n: u8) -> bool {
    let iv = h.chord.quality.intervals();
    let rel = (n as i32 - h.chord.root as i32).rem_euclid(12) as u8;
    iv.get(1) == Some(&rel) || iv.get(3).is_some_and(|x| x % 12 == rel) || added(h) == Some(rel)
}

/// Is `n` the 7th of `h` (a note that wants to fall)?
fn is_seventh(h: &Harm, n: u8) -> bool {
    let rel = (n as i32 - h.chord.root as i32).rem_euclid(12);
    h.chord.quality.intervals().get(3).is_some_and(|&x| x == 10 || x == 11) && h.chord.quality.intervals()[3] as i32 == rel
}

/// What a voice wants from its next note.
#[derive(Clone, Copy)]
pub struct Want<'a> {
    pub lo: i32,
    pub hi: i32,
    /// Where in the register it likes to be.
    pub near: i32,
    /// Its note before.
    pub prev: Option<u8>,
    pub h: Harm,
    /// On the beat: a chord tone.
    pub strong: bool,
    /// Rather a guide tone.
    pub guide: bool,
    /// A note it's after, and how much it cares per semitone off it.
    pub target: Option<(i32, f32)>,
    /// Stay under this note (the voice above), and over this one (the voice below).
    pub below: Option<u8>,
    pub above: Option<u8>,
    /// The other voices: each one's note before, and its note now.
    pub against: &'a [(Option<u8>, u8)],
    /// Rather moving this way (-1, 0, +1).
    pub dir: i32,
    /// What a repeated note costs.
    pub repeat: f32,
    /// The note before was a 7th: fall a step.
    pub resolve: bool,
}

impl<'a> Want<'a> {
    fn new(range: (i32, i32), near: i32, prev: Option<u8>, h: Harm, strong: bool) -> Want<'a> {
        Want { lo: range.0, hi: range.1, near, prev, h, strong, guide: false, target: None, below: None, above: None, against: &[], dir: 0, repeat: 1.0, resolve: false }
    }
}

/// The cost of `n` for `w` (lower is better): the counterpoint rules, as penalties.
pub fn cost(w: &Want, n: i32) -> f32 {
    let mut c = 0.0;
    let u = n.clamp(0, 127) as u8;
    if !w.h.scale.contains(u % 12) {
        c += 6.0;
    }
    if !is_tone(&w.h, u) {
        c += if w.strong { 5.0 } else { 1.2 };
    }
    if w.guide && !is_guide(&w.h, u) {
        c += 2.5;
    }
    if let Some(p) = w.prev {
        let d = n - p as i32;
        c += match d.abs() {
            0 => w.repeat,
            1 | 2 => 0.0,
            3 | 4 => 0.6,
            5..=7 => 1.8,
            8..=12 => 3.5,
            _ => 9.0,
        };
        if w.resolve && !(d == -1 || d == -2) {
            c += 2.5;
        }
        if w.dir != 0 && d.signum() == w.dir {
            c -= 0.5;
        }
    }
    for &(op, on) in w.against {
        let iv = (n - on as i32).abs();
        let ic = iv % 12;
        if iv == 0 {
            c += 6.0;
        } else if ic == 0 {
            c += 2.0;
        }
        match ic {
            1 | 11 => c += if w.strong { 3.5 } else { 1.0 },
            3 | 4 | 8 | 9 => c -= 0.7,
            2 | 10 => c += 0.3,
            _ => {}
        }
        if iv > 0 && iv < 3 {
            c += 1.0;
        }
        // Motion against the other voice: parallel fifths and octaves are out, a leap into
        // one in similar motion is a hidden one, contrary motion is best.
        if let (Some(op), Some(p)) = (op, w.prev) {
            let (sd, od) = (n - p as i32, on as i32 - op as i32);
            let was = (p as i32 - op as i32).abs() % 12;
            if sd != 0 && sd.signum() == od.signum() {
                if was == ic && (ic == 0 || ic == 7) {
                    c += 12.0;
                } else if (ic == 0 || ic == 7) && sd.abs() > 2 {
                    c += 2.0;
                } else {
                    c += 0.25;
                }
            } else if sd != 0 && od != 0 {
                c -= 0.6;
            }
        }
    }
    if w.below.is_some_and(|b| n >= b as i32) || w.above.is_some_and(|a| n <= a as i32) {
        c += 30.0;
    }
    c += ((n - w.near).abs() as f32 / 7.0).powi(2);
    if let Some((t, k)) = w.target {
        c += (n - t).abs() as f32 * k;
    }
    c
}

/// The best note for `w` (a little seeded jitter between near-equals).
pub fn pick(w: &Want, r: &mut Rng) -> u8 {
    let mut best = (f32::MAX, w.near.clamp(w.lo, w.hi));
    for n in w.lo..=w.hi {
        let c = cost(w, n) + r.f() as f32 * 0.5;
        if c < best.0 {
            best = (c, n);
        }
    }
    best.1.clamp(0, 127) as u8
}

/// The note with pitch class `pc` nearest `near` in `range`.
fn nearest(pc: u8, near: i32, range: (i32, i32)) -> u8 {
    super::ornament::nearest_pc(pc % 12, near, range.0, range.1)
}

impl Writer {
    pub fn new() -> Writer {
        Writer {
            alto: Vec::with_capacity(48),
            tenor: Vec::with_capacity(48),
            bass: Vec::with_capacity(32),
            last: [None; 3],
            prev_harm: None,
            run: 1,
            cliche: None,
            changes: Vec::with_capacity(16),
        }
    }

    /// Write the bar's lines into [`Writer::alto`], [`Writer::tenor`] and [`Writer::bass`] (the
    /// bass left empty where it walks).
    pub fn write(&mut self, bar: &Bar, r: &mut Rng) {
        self.alto.clear();
        self.tenor.clear();
        self.bass.clear();
        self.find_changes(bar);
        if self.changes.is_empty() {
            return;
        }
        let p = bar.plan;
        if p.texture.writes_bass() {
            self.bass_line(bar, r);
        }
        match p.texture {
            Texture::FrontLine => {
                self.trombone(bar, r);
                self.clarinet(bar, r);
            }
            Texture::Canon => {
                self.canon(bar, r);
                self.guide_line(bar, r, T, false);
            }
            Texture::Chorale => {
                self.guide_line(bar, r, T, false);
                self.guide_line(bar, r, A, true);
            }
            Texture::Answers => self.answers(bar, r),
            Texture::Cliche => {
                self.cliche_line(bar, r);
                self.guide_line(bar, r, A, false);
            }
            Texture::Locked => self.locked(bar),
        }
        if p.texture != Texture::Cliche {
            self.cliche = None;
        }
        if !p.tenor {
            self.tenor.clear();
        }
        for (v, k) in [(&self.alto, A), (&self.tenor, T), (&self.bass, B)] {
            if let Some(t) = v.last() {
                self.last[k] = Some(t.n);
            }
        }
        self.prev_harm = (bar.harm)(bar.bb - 0.01);
    }

    /// Where the bar's chords start (on the 8th grid; the first at 0).
    fn find_changes(&mut self, bar: &Bar) {
        self.changes.clear();
        let mut prev: Option<Chord> = None;
        let mut b = 0.0;
        while b < bar.bb - 1e-9 {
            if let Some(h) = (bar.harm)(b)
                && prev != Some(h.chord)
            {
                self.changes.push(b);
                prev = Some(h.chord);
            }
            b += 0.5;
        }
    }

    /// The chord starting at `b` lasts until here (the bar's end at most).
    fn chord_end(&self, b: f64, bb: f64) -> f64 {
        self.changes.iter().copied().find(|&c| c > b + 1e-6).unwrap_or(bb)
    }

    fn voice(&mut self, k: usize) -> &mut Vec<Tone> {
        match k {
            A => &mut self.alto,
            T => &mut self.tenor,
            _ => &mut self.bass,
        }
    }

    /// The others' (before, now) at `b`, for voice `k`: the lead, and whichever of the bass,
    /// tenor and alto are written.
    fn against(&self, bar: &Bar, b: f64, k: usize) -> ([(Option<u8>, u8); 3], usize) {
        let mut out = [(None, 0u8); 3];
        let mut n = 0;
        if let Some(l) = at(bar.lead, b) {
            out[n] = (before(bar.lead, b, None), l);
            n += 1;
        }
        for (j, v) in [(B, &self.bass), (T, &self.tenor), (A, &self.alto)] {
            if j != k
                && n < 3
                && let Some(x) = at(v, b)
            {
                out[n] = (before(v, b, self.last[j]), x);
                n += 1;
            }
        }
        (out, n)
    }

    /// The note voice `k` had before beat `b`.
    fn prev_of(&self, k: usize, b: f64) -> Option<u8> {
        let v = match k {
            A => &self.alto,
            T => &self.tenor,
            _ => &self.bass,
        };
        before(v, b, self.last[k])
    }

    // --- the bass ---------------------------------------------------------------------------

    fn bass_line(&mut self, bar: &Bar, r: &mut Rng) {
        let bb = bar.bb;
        let tex = bar.plan.texture;
        let mut prev = self.last[B].map_or(40, |n| n as i32);
        // The attacks: each chord, and the half bar (two-beat).
        let mut hits: [(f64, bool); 12] = [(0.0, false); 12];
        let mut n = 0;
        for &c in &self.changes {
            if n < 12 {
                hits[n] = (c, true);
                n += 1;
            }
        }
        if bb >= 4.0 && !self.changes.iter().any(|&c| (c - 2.0).abs() < 0.6) {
            hits[n] = (2.0, false);
            n += 1;
        }
        if tex == Texture::Locked {
            // Hits with the line: its long notes on the beat.
            for t in bar.lead.iter().filter(|t| !t.tie && (t.b - t.b.round()).abs() < 1e-6 && t.d >= 0.9) {
                if n < 12 && !hits[..n].iter().any(|h| (h.0 - t.b).abs() < 1e-6) {
                    hits[n] = (t.b, false);
                    n += 1;
                }
            }
        }
        let hits = &mut hits[..n];
        hits.sort_by(|a, b| a.0.total_cmp(&b.0));
        let lead_dir = |from: f64, to: f64| match (at(bar.lead, from).or(before(bar.lead, from, None)), at(bar.lead, to)) {
            (Some(a), Some(b)) => (b as i32 - a as i32).signum(),
            _ => 0,
        };
        for (i, &(b, start)) in hits.iter().enumerate() {
            let Some(h) = (bar.harm)(b) else { continue };
            // Near the last note, pulled toward the middle of the register.
            let home = (prev + 41) / 2;
            let next = if i + 1 < hits.len() { hits[i + 1].0 } else { bb };
            let note = match tex {
                Texture::Cliche => nearest(h.chord.bass_pc(), home, BASS),
                Texture::FrontLine if !start => nearest(h.chord.root + 7, home, BASS),
                Texture::Chorale if !start => {
                    let (ag, k) = ([(before(bar.lead, b, None), at(bar.lead, b).unwrap_or(0))], at(bar.lead, b).is_some() as usize);
                    let mut w = Want::new(BASS, prev, Some(prev as u8), h, true);
                    w.against = &ag[..k];
                    w.dir = -lead_dir(0.0, b);
                    w.repeat = 2.0;
                    pick(&w, r)
                }
                _ => nearest(h.chord.bass_pc(), home, BASS),
            };
            let d = match tex {
                Texture::Locked => ((next - b) * 0.6).min(0.6),
                Texture::FrontLine => (next - b) * 0.85,
                _ => next - b,
            };
            push(&mut self.bass, Tone::new(b, d.max(0.1), note));
            prev = note as i32;
        }
        // A chromatic approach into the next bar's chord (the chorale's walking moment).
        if tex == Texture::Chorale
            && bb >= 4.0
            && let (Some(now), Some(nh)) = ((bar.harm)(bb - 1.0), (bar.harm)(bb))
            && nh.chord.root != now.chord.root
            && r.chance(0.6)
        {
            let target = nearest(nh.chord.bass_pc(), prev, BASS) as i32;
            let n = if prev > target { target + 1 } else { target - 1 };
            let n = if n == prev { 2 * target - n } else { n };
            if let Some(t) = self.bass.last_mut() {
                t.d = t.d.min(bb - 1.0 - t.b).max(0.1);
            }
            push(&mut self.bass, Tone::new(bb - 1.0, 0.9, n.clamp(BASS.0, BASS.1) as u8));
        }
    }

    // --- slow lines ---------------------------------------------------------------------------

    /// A guide-tone line for voice `k`: a note at each chord, held; a voice that holds a whole
    /// bar's chord moves at the half bar (one of the two, so the other is oblique). `suspend`:
    /// the alto may hold over the bar line into the new chord and resolve down a step.
    fn guide_line(&mut self, bar: &Bar, r: &mut Rng, k: usize, suspend: bool) {
        let bb = bar.bb;
        let (range, near) = if k == A { (ALTO, at(bar.lead, 0.0).map_or(66, |l| l as i32 - 7).clamp(58, 74)) } else { (TENOR, 57) };
        let mut beats = [0.0f64; 12];
        let mut n = 0;
        for &c in &self.changes {
            beats[n] = c;
            n += 1;
            let end = self.chord_end(c, bb);
            // The half-bar move: the alto in even bars, the tenor in odd ones.
            if end - c >= 4.0 - 1e-6 && ((bar.plan.bar % 2 == 0) == (k == A)) && n < 12 {
                beats[n] = c + 2.0;
                n += 1;
            }
        }
        for (i, &b) in beats[..n].iter().enumerate() {
            let Some(h) = (bar.harm)(b) else { continue };
            let prev = self.prev_of(k, b);
            let new_chord = self.changes.iter().any(|&c| (c - b).abs() < 1e-6);
            // The suspension: the note before isn't in the new chord, a step above one: hold
            // it a beat, then fall.
            if suspend
                && b == 0.0
                && i + 1 < n.max(2)
                && let Some(p) = prev
                && !is_tone(&h, p)
                && let Some(res) = [p.saturating_sub(1), p.saturating_sub(2)].into_iter().find(|&x| is_tone(&h, x))
                && at(&self.tenor, 1.0).is_none_or(|t| t < res)
                && at(bar.lead, 0.0).is_none_or(|l| l > p && l != res)
                && r.chance(0.7)
            {
                push(self.voice(k), Tone { b: 0.0, d: 1.0, n: p, tie: true, slide: 0 });
                push(self.voice(k), Tone::new(1.0, 1.0, res));
                continue;
            }
            let (ag, m) = self.against(bar, b, k);
            let mut w = Want::new(range, near, prev, h, true);
            w.guide = true;
            w.against = &ag[..m];
            if k == A {
                w.below = at(bar.lead, b).or(before(bar.lead, b, None));
                w.above = at(&self.tenor, b);
            } else {
                w.below = at(&self.alto, b);
                // (Clear of the bass: the low register muddies fast.)
                w.above = at(&self.bass, b).map(|x| x.saturating_add(4));
            }
            w.repeat = if new_chord { 0.2 } else { 3.0 };
            w.resolve = new_chord && prev.is_some_and(|p| self.prev_harm.is_some_and(|ph| is_seventh(&ph, p)) || is_seventh(&h, p) && !new_chord);
            w.dir = at(bar.lead, b).zip(before(bar.lead, b, None)).map_or(0, |(a, z)| -(a as i32 - z as i32).signum());
            let note = pick(&w, r);
            push(self.voice(k), Tone::new(b, 1.0, note));
        }
        let v = self.voice(k);
        legato(v, bb, 4.0);
        merge(v);
    }

    /// The trombone: the guide-tone line, every chord smeared into (a big one into a phrase).
    fn trombone(&mut self, bar: &Bar, r: &mut Rng) {
        self.guide_line(bar, r, T, false);
        let phrase = bar.plan.bar % 4 == 0 || at(bar.lead, 0.0).is_none();
        for t in &mut self.tenor {
            if t.b == 0.0 && phrase {
                t.slide = -5;
            } else if r.chance(0.5) {
                t.slide = -2;
            }
        }
    }

    // --- busy lines --------------------------------------------------------------------------

    /// The clarinet: per beat, two swung 8ths (a triplet now and then) running while the tune
    /// holds, one 8th against the tune's one attack, nothing (the note before held) while it
    /// moves.
    fn clarinet(&mut self, bar: &Bar, r: &mut Rng) {
        let bb = bar.bb;
        let mean = {
            let (s, c) = bar.lead.iter().fold((0i32, 0i32), |(s, c), t| (s + t.n as i32, c + 1));
            if c > 0 { s / c } else { 72 }
        };
        let near = (mean + 5).clamp(66, 80);
        let mut slots = [0.0f64; 24];
        let mut n = 0;
        let mut k = 0.0;
        while k < bb - 1e-9 {
            let attacks = bar.lead.iter().filter(|t| !t.tie && t.b >= k - 1e-6 && t.b < k + 1.0 - 1e-6).count();
            let on_beat = bar.lead.iter().any(|t| !t.tie && (t.b - k).abs() < 1e-6);
            let mut add = |b: f64| {
                if n < slots.len() {
                    slots[n] = b;
                    n += 1;
                }
            };
            match attacks {
                0 if r.chance(0.25) => {
                    add(k);
                    add(k + 1.0 / 3.0);
                    add(k + 2.0 / 3.0);
                }
                0 => {
                    add(k);
                    add((bar.swing)(k + 0.5));
                }
                1 if on_beat => add((bar.swing)(k + 0.5)),
                1 => add(k),
                _ => {}
            }
            k += 1.0;
        }
        // A tune busy from the bar line: the clarinet holds a long note under it.
        if n == 0 || slots[0] >= 1.0 - 1e-6 {
            slots.copy_within(0..n.min(23), 1);
            slots[0] = 0.0;
            n = (n + 1).min(slots.len());
        }
        for &b in &slots[..n] {
            let Some(h) = (bar.harm)(b) else { continue };
            let prev = self.prev_of(A, b);
            if let Some(p) = prev
                && (p as i32 - near).abs() > 7
            {
                self.run = if (p as i32) > near { -1 } else { 1 };
            }
            let (ag, m) = self.against(bar, b, A);
            let mut w = Want::new((ALTO.0.max(60), ALTO.1), near, prev, h, (b - b.round()).abs() < 1e-6);
            w.against = &ag[..m];
            w.dir = self.run;
            w.repeat = 2.5;
            let note = pick(&w, r);
            push(&mut self.alto, Tone::new(b, 0.5, note));
        }
        legato(&mut self.alto, bb, 2.0);
    }

    /// The canon: the tune a bar later (stretto: two beats), an octave down, each note bent
    /// to the chord it lands on.
    fn canon(&mut self, bar: &Bar, r: &mut Rng) {
        let bb = bar.bb;
        let delay = if bar.plan.stretto && bb >= 4.0 { 2.0 } else { bb };
        let mut src = [Tone::new(0.0, 0.0, 0); 32];
        let mut n = 0;
        for t in bar.prev_lead.iter().filter(|t| t.b >= bb - delay - 1e-6) {
            if n < src.len() {
                src[n] = Tone { b: t.b - (bb - delay), ..*t };
                n += 1;
            }
        }
        if delay < bb {
            for t in bar.lead.iter().filter(|t| t.b < bb - delay - 1e-6) {
                if n < src.len() {
                    src[n] = Tone { b: t.b + delay, ..*t };
                    n += 1;
                }
            }
        }
        for t in &src[..n] {
            let Some(h) = (bar.harm)(t.b) else { continue };
            let prev = self.prev_of(A, t.b);
            if t.tie && prev.is_some() {
                push(&mut self.alto, Tone { n: prev.unwrap(), ..*t });
                continue;
            }
            let m = t.n as i32 - 12;
            let (ag, k) = self.against(bar, t.b, A);
            let mut w = Want::new(ALTO, m, prev, h, (t.b - t.b.round()).abs() < 1e-6 || t.d >= 1.0);
            w.against = &ag[..k];
            w.target = Some((m, 1.6));
            w.repeat = 0.0;
            let note = pick(&w, r);
            push(&mut self.alto, Tone { n: note, slide: 0, ..*t });
        }
        let end = self.alto.last().map_or(0.0, |t| t.end());
        if let Some(t) = self.alto.last_mut() {
            t.d = (bb - t.b).min(t.d.max(end - t.b));
        }
    }

    /// The answers: in each gap of the tune (a beat and more with no attack), the alto plays
    /// the tune's last motif back (inverted, or a step lower), the tenor under it.
    fn answers(&mut self, bar: &Bar, r: &mut Rng) {
        let bb = bar.bb;
        // The attacks, then the gaps after them.
        let mut gaps = [(0.0f64, 0.0f64); 6];
        let mut g = 0;
        let mut from = if at(bar.lead, 0.0).is_none() { Some(0.0) } else { None };
        let attacks = bar.lead.iter().filter(|t| !t.tie);
        let mut last_attack: Option<f64> = None;
        for t in attacks.chain(std::iter::once(&Tone::new(bb, 0.0, 0))) {
            let start = from.take().or(last_attack.map(|a| ((a + 1.0) * 2.0).ceil() / 2.0));
            if let Some(s) = start
                && t.b - 0.25 - s >= 1.0 - 1e-6
                && g < gaps.len()
            {
                gaps[g] = (s, t.b - 0.25);
                g += 1;
            }
            last_attack = Some(t.b);
        }
        // Behind the tune where it's busy: a background figure (the section's own, as a band's
        // backgrounds go), guide tones in both voices, out of the gaps' way.
        let figure = BACKGROUNDS[bar.plan.section as usize % BACKGROUNDS.len()];
        for &(b0, d) in figure {
            let b = (bar.swing)(b0);
            if b >= bb - 1e-6 || gaps[..g].iter().any(|&(s, e)| b < e + 0.25 && b + d > s) {
                continue;
            }
            let Some(h) = (bar.harm)(b) else { continue };
            let d = d.min(bb - b);
            let (ag, k) = self.against(bar, b, A);
            let mut w = Want::new(ALTO, at(bar.lead, b).map_or(64, |l| l as i32 - 7), self.prev_of(A, b), h, true);
            w.guide = true;
            w.against = &ag[..k];
            w.below = at(bar.lead, b).or(before(bar.lead, b, None));
            w.repeat = 0.5;
            let a = pick(&w, r);
            push(&mut self.alto, Tone::new(b, d, a));
            if bar.plan.tenor {
                let (ag, k) = self.against(bar, b, T);
                let mut w = Want::new(TENOR, a as i32 - 6, self.prev_of(T, b), h, true);
                w.guide = true;
                w.against = &ag[..k];
                w.below = Some(a);
                w.above = at(&self.bass, b).map(|x| x.saturating_add(4));
                w.repeat = 0.5;
                let t = pick(&w, r);
                push(&mut self.tenor, Tone::new(b, d, t));
            }
        }
        for &(s, e) in &gaps[..g] {
            // The motif: the tune's last notes before the gap (this bar's, else the last bar's).
            let mut motif = [0u8; 4];
            let mut m = 0;
            for t in bar.lead.iter().rev().filter(|t| !t.tie && t.b < s).chain(bar.prev_lead.iter().rev().filter(|t| !t.tie)) {
                if m == 4 {
                    break;
                }
                motif[m] = t.n;
                m += 1;
            }
            motif[..m].reverse();
            let invert = r.chance(0.5);
            let count = (((e - s) / 0.5).floor() as usize).clamp(2, 4);
            let start = before(bar.lead, s + 1e-3, None).map_or(66, |l| l as i32 - 5);
            let mut target = start;
            for j in 0..count {
                let b = (bar.swing)(s + j as f64 * 0.5);
                let Some(h) = (bar.harm)(b) else { continue };
                if j > 0 {
                    let step = if j < m { motif[j] as i32 - motif[j - 1] as i32 } else { -2 };
                    target += if invert { -step } else { step };
                }
                let t0 = if j == 0 && !invert { target - 2 } else { target };
                let (ag, k) = self.against(bar, b, A);
                let mut w = Want::new(ALTO, start, self.prev_of(A, b), h, (b - b.round()).abs() < 1e-6 || j + 1 == count);
                w.against = &ag[..k];
                w.target = Some((t0, 1.2));
                w.below = before(bar.lead, b + 1e-3, None);
                w.repeat = 1.5;
                let a = pick(&w, r);
                let d = if j + 1 == count { (e - b).max(0.4) } else { 0.5 };
                push(&mut self.alto, Tone::new(b, d, a));
                target = a as i32;
                if bar.plan.tenor {
                    let (ag, k) = self.against(bar, b, T);
                    let mut w = Want::new(TENOR, a as i32 - 7, self.prev_of(T, b), h, true);
                    w.against = &ag[..k];
                    w.below = Some(a);
                    w.above = at(&self.bass, b).map(|x| x.saturating_add(4));
                    w.repeat = 1.0;
                    let t = pick(&w, r);
                    push(&mut self.tenor, Tone::new(b, d, t));
                }
            }
        }
        self.alto.sort_unstable_by(|x, y| x.b.total_cmp(&y.b));
        self.tenor.sort_unstable_by(|x, y| x.b.total_cmp(&y.b));
    }

    /// The line cliché: under a chord a whole bar long (or more), the tenor creeps 5 ♯5 6 ♭7
    /// (a major chord: 5 ♯5 6 ♯5; minor: 1 7 ♭7 6), a beat a step, on into the next bar if
    /// the chord lasts; under moving chords, guide tones.
    fn cliche_line(&mut self, bar: &Bar, r: &mut Rng) {
        let bb = bar.bb;
        let Some(h) = (bar.harm)(0.0) else { return };
        if self.changes.len() > 1 {
            self.cliche = None;
            self.guide_line(bar, r, T, false);
            return;
        }
        let steps: [u8; 8] = match h.chord.family() {
            Family::Minor => [12, 11, 10, 9, 10, 11, 12, 11],
            Family::Dominant => [7, 8, 9, 10, 9, 8, 7, 8],
            _ => [7, 8, 9, 8, 7, 8, 9, 8],
        };
        let from = match self.cliche {
            Some((c, k)) if c == h.chord => k,
            _ => 0,
        };
        let base = nearest(h.chord.root, 50, (43, 55)) as i32;
        let mut k = from;
        let mut b = 0.0;
        while b < bb - 1e-9 {
            let n = (base + steps[k % 8] as i32).clamp(TENOR.0, TENOR.1) as u8;
            push(&mut self.tenor, Tone::new(b, 1.0, n));
            k += 1;
            b += 1.0;
        }
        self.cliche = Some((h.chord, k));
    }

    /// Planing into the next phrase over a long dominant: the alto and tenor leave the line
    /// for three swung 8ths that slide chromatically, in parallel, into the next chord's
    /// locked voicing under `top` (the tune's next note).
    pub fn plane(&mut self, bar: &Bar, next: &Harm, top: u8) {
        let from = bar.bb - 1.5;
        let (a, t) = voicing(next, top, 1.0);
        let voices = if bar.plan.tenor { 2 } else { 1 };
        for (v, n) in [(&mut self.alto, a), (&mut self.tenor, t)].into_iter().take(voices) {
            v.retain(|x| x.b < from - 1e-6);
            if let Some(x) = v.last_mut() {
                x.d = x.d.min(from - x.b);
            }
            for (k, semis) in [-3i32, -2, -1].into_iter().enumerate() {
                let b = (bar.swing)(from + k as f64 * 0.5);
                push(v, Tone::new(b, 0.45, (n as i32 + semis).clamp(0, 127) as u8));
            }
        }
        legato(&mut self.alto, bar.bb, 0.5);
        legato(&mut self.tenor, bar.bb, 0.5);
        self.last = [Some(a), Some(t), self.last[B]];
    }

    // --- the locked soli -----------------------------------------------------------------------

    /// Drop-2 under every tune note: the two chord tones below it in close position, the upper
    /// one dropped an octave (the tenor) and the lower the alto; a passing note on a dominant
    /// (or any short one off the chord) gets a diminished chord under it.
    fn locked(&mut self, bar: &Bar) {
        let mut prev: Option<(u8, u8)> = None;
        for t in bar.lead {
            let Some(h) = (bar.harm)(t.b.max(0.0)) else { continue };
            let (a, tn) = match prev {
                Some(v) if t.tie => v,
                _ => voicing(&h, t.n, t.d),
            };
            push(&mut self.alto, Tone { b: t.b, d: t.d, n: a, tie: t.tie && prev.is_some(), slide: 0 });
            push(&mut self.tenor, Tone { b: t.b, d: t.d, n: tn, tie: t.tie && prev.is_some(), slide: 0 });
            prev = Some((a, tn));
        }
    }
}

/// The locked voicing under melody note `n` (`d` beats long): (alto, tenor).
pub fn voicing(h: &Harm, n: u8, d: f64) -> (u8, u8) {
    let passing = !is_tone(h, n) && (h.chord.family() == Family::Dominant || d <= 0.5);
    let (v2, v3) = if passing {
        // A diminished 7th built down from the melody.
        (n as i32 - 3, n as i32 - 6)
    } else {
        // The next two chord tones under the melody (a 6th added to a plain triad).
        let mut pcs = [0u8; 4];
        let iv = h.chord.quality.intervals();
        let k = iv.len().min(4);
        for (x, i) in pcs.iter_mut().zip(iv) {
            *x = (h.chord.root + i) % 12;
        }
        let k = match added(h) {
            Some(a) if k == 3 => {
                pcs[3] = (h.chord.root + a) % 12;
                4
            }
            _ => k,
        };
        let tones = &pcs[..k];
        let below = |from: i32| (1..12).map(|d| from - d).find(|&x| x >= 0 && tones.contains(&((x % 12) as u8)) && (x % 12) as u8 != n % 12).unwrap_or(from - 3);
        let v2 = below(n as i32);
        (v2, below(v2))
    };
    let fold = |x: i32, (lo, hi): (i32, i32)| {
        let mut x = x;
        while x < lo {
            x += 12;
        }
        while x > hi {
            x -= 12;
        }
        x as u8
    };
    // Drop 2: the second voice from the top goes down an octave.
    let alto = fold(v3, (ALTO.0, (n as i32 - 1).min(ALTO.1)));
    let tenor = fold(v2 - 12, (TENOR.0, (alto as i32 - 1).min(TENOR.1)));
    (alto, tenor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::chart::Quality;

    fn harm(root: u8, q: Quality) -> Harm {
        Harm::new(Chord { root, quality: q, bass: None })
    }

    #[test]
    fn plan_builds_through_the_chorus() {
        for seed in 0..20 {
            let bars: Vec<SoliBar> = (0..32).map(|b| plan(seed, 1, b, 32)).collect();
            assert!(!bars[0].tenor && bars[8].tenor, "the tenor joins in the second section");
            assert_ne!(bars[0].texture, Texture::Canon);
            assert_eq!(bars[31].texture, Texture::Locked);
            assert_ne!(bars[24].texture, Texture::Locked);
            let rolled: std::collections::HashSet<_> = [0, 8, 16, 24].map(|b| bars[b].texture).into();
            assert_eq!(rolled.len(), 4, "every section its own texture");
            assert!(bars[6].stretto == (bars[6].texture == Texture::Canon));
        }
        let firsts: std::collections::HashSet<_> = (0..6).map(|p| plan(3, p, 0, 32).texture).collect();
        assert!(firsts.len() >= 3, "the passes differ: {firsts:?}");
    }

    #[test]
    fn no_parallel_fifths() {
        // Lead C5 → D5 over C → Dm: the alto, a fifth under (F4 → G4 would be parallel).
        let h = harm(2, Quality::Minor7);
        let ag = [(Some(72u8), 74u8)];
        let mut w = Want::new(ALTO, 67, Some(65), h, true);
        w.against = &ag;
        assert!(cost(&w, 67) > cost(&w, 65) + 5.0, "F-G under C-D is parallel fifths");
    }

    #[test]
    fn locked_voicing_is_drop_two() {
        // C6 under E5: close C A G → drop 2: alto G4? close position under E: C, A; drop the C.
        let h = harm(0, Quality::Six);
        let (a, t) = voicing(&h, 76, 1.0);
        assert_eq!(a % 12, 9, "alto: the A");
        assert_eq!(t % 12, 0, "tenor: the C, an octave down");
        assert!(t < a && a < 76);
        // A passing note on G7: a diminished chord under it.
        let (a, t) = voicing(&harm(7, Quality::Dom7), 75, 0.5);
        assert_eq!(((75 - a as i32).rem_euclid(12), (75 - t as i32).rem_euclid(12)), (6, 3));
    }

    #[test]
    fn guide_tones() {
        let g7 = harm(7, Quality::Dom7);
        assert!(is_guide(&g7, 71) && is_guide(&g7, 65) && !is_guide(&g7, 67));
        assert!(is_seventh(&g7, 65) && !is_seventh(&g7, 71));
    }
}
