//! Who the lead solos like: a profile of how a player builds lines, measured on their
//! transcribed solos (the Weimar Jazz Database v2.1: 456 solos, note by note, with the
//! chords). Only statistics: how long their phrases run and how long they rest between, which
//! rhythms and intervals they use, how often a strong beat gets a chord tone, a half-step
//! approach or an enclosure, how often a line runs through a pentatonic, where they sit
//! against the beat and how hard they swing. No phrase of theirs is played back: the line is
//! the lead's own, drawn from the numbers each bar.
//!
//! - [`Soloist::Band`]: the lead's own 8th-note lines (no profile; the default).
//! - [`Soloist::Miles`]: Miles Davis, 1951-59 (10 solos): space (44% silence, ~3.3 beats
//!   between phrases), steps and half steps, repeated notes, half-step approaches into the
//!   strong beats, light swing (1.36), laid back.
//! - [`Soloist::Trane`]: John Coltrane, 1960-64 (9 solos, the quartet years): dense, short
//!   cells often starting before the beat, thirds and leaps, triplets and 16ths, a third of
//!   the line pentatonic, on the beat.
//! - [`Soloist::Bird`]: Charlie Parker, 1945-51 (17 solos): long bebop lines, short rests,
//!   chromatic approaches from above, enclosures, swung hard (1.60).
//! - [`Soloist::Sonny`]: Sonny Rollins, 1954-72 (13 solos): long 8th-note lines low on the
//!   tenor, thirds, enclosures, laid way back, and the theme developed phrase to phrase.
//! - [`Soloist::Wayne`]: Wayne Shorter, 1961-66 (10 solos): whole steps and turns, repeated
//!   notes, the most pentatonic of all, few approaches, an even tone, and the most variants.
//!
//! The line carries over bar lines (a phrase, then a rest, then the next), so it's a [`Line`]
//! the lead keeps between bars. Pure, no allocation.

use crate::audio::accomp::Rng;
use crate::audio::chart::Family;
use crate::audio::live::ornament::{Harm, next_in};

/// Who the lead solos like.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Soloist {
    #[default]
    Band,
    Miles,
    Trane,
    Bird,
    Sonny,
    Wayne,
}

impl Soloist {
    pub const ALL: [Soloist; 6] = [Soloist::Band, Soloist::Miles, Soloist::Trane, Soloist::Bird, Soloist::Sonny, Soloist::Wayne];

    pub fn slug(self) -> &'static str {
        match self {
            Soloist::Band => "band",
            Soloist::Miles => "miles",
            Soloist::Trane => "trane",
            Soloist::Bird => "bird",
            Soloist::Sonny => "sonny",
            Soloist::Wayne => "wayne",
        }
    }

    pub fn parse(s: &str) -> Option<Soloist> {
        Soloist::ALL.into_iter().find(|p| p.slug() == s)
    }

    /// The measured profile (none for the band's own lines).
    pub fn profile(self) -> Option<&'static Profile> {
        match self {
            Soloist::Band => None,
            Soloist::Miles => Some(&MILES),
            Soloist::Trane => Some(&TRANE),
            Soloist::Bird => Some(&BIRD),
            Soloist::Sonny => Some(&SONNY),
            Soloist::Wayne => Some(&WAYNE),
        }
    }

    /// The lead's lay-back on the beat against the band's average soloist ([`super::pocket`]).
    pub fn lay_back(self) -> f32 {
        match self {
            Soloist::Band => 1.0,
            // On-beat notes +4.6 ms (Miles), +0.8 (Trane, quartet −0.2), +2.4 (Bird), +6.2
            // (Sonny), about +1 (Wayne: post-bop +1.5, the 2nd Quintet +0.8), against +2.8.
            Soloist::Miles => 1.6,
            Soloist::Trane => 0.3,
            Soloist::Bird => 0.9,
            Soloist::Sonny => 2.2,
            Soloist::Wayne => 0.4,
        }
    }
}

/// How a player builds lines. Lengths in beats; shares 0..1.
#[derive(Debug, Clone, Copy)]
pub struct Profile {
    /// Phrase length: median and interquartile range (log-normal).
    pub phrase: (f64, f64, f64),
    /// Rest between phrases, likewise.
    pub rest: (f64, f64, f64),
    /// How often each rhythm cell starts (relative): [`Cell`] order.
    pub cells: [f32; 7],
    /// Interval weights by size in semitones: 0 (repeat), 1, 2, 3, 4, 5, 6, 7, and 8-12.
    pub intervals: [f32; 9],
    /// After a leap (≥ 5), how often the line steps back the other way.
    pub step_back: f32,
    /// Extra pull to a chord tone on beats 1 and 3 (on top of where the steps land anyway).
    pub strong_chord_tone: f32,
    /// A phrase's last note on a chord tone.
    pub end_chord_tone: f32,
    /// Half-step approach into a strong-beat chord tone, and the share of them from below.
    pub approach: f32,
    pub from_below: f32,
    /// An enclosure into a strong-beat chord tone (one above, one below).
    pub enclosure: f32,
    /// A run of four through the chord's pentatonic.
    pub pentatonic: f32,
    /// A phrase after a rest starts on the "and" before the bar line.
    pub anticipation: f32,
    /// Where the line lives (MIDI, the lead's octave): centre, soft floor, soft ceiling.
    pub register: (i32, i32, i32),
    /// Sounding length against the time to the next note.
    pub legato: f32,
    /// Swing ratio (long 8th over short; 2.0 a full triplet).
    pub swing: f32,
    /// A fall off a phrase's last note.
    pub falls: f32,
    /// Dynamics, dB against the solo's median: the phrase's arch (first note, early, middle,
    /// late, last note), louder going up (dB per octave above the register's centre), the
    /// offbeat 8th ghosted (−4 dB) this often or else accented this much, long notes (≥ 1.2
    /// beats), and each note's own scatter (a standard deviation).
    pub arch: [f32; 5],
    pub per_octave: f32,
    pub ghost: f32,
    pub accent: f32,
    pub long: f32,
    pub scatter: f32,
    /// Motifs: how often a phrase restates the last one's head (and then goes its own way), and
    /// how often a cell of 3-4 notes is restated straight after inside a phrase (a sequence),
    /// with the sequences' transpositions (semitones, by weight).
    pub motif: f32,
    pub sequence: f32,
    pub moves: [(i8, f32); 5],
}

/// The rhythm cells a line is built from, each starting on a beat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    /// Two 8ths.
    Eighths,
    /// A quarter.
    Quarter,
    /// Three triplet 8ths.
    Triplet,
    /// Four 16ths.
    Sixteenths,
    /// A dotted 8th and a 16th.
    Dotted,
    /// Three quarter-note triplets over two beats.
    QuarterTriplet,
    /// A half note.
    Long,
}

const CELLS: [Cell; 7] = [Cell::Eighths, Cell::Quarter, Cell::Triplet, Cell::Sixteenths, Cell::Dotted, Cell::QuarterTriplet, Cell::Long];

impl Cell {
    /// Its notes' lengths (beats), and how many there are.
    fn rhythm(self) -> ([f64; 4], usize) {
        match self {
            Cell::Eighths => ([0.5, 0.5, 0.0, 0.0], 2),
            Cell::Quarter => ([1.0, 0.0, 0.0, 0.0], 1),
            Cell::Triplet => ([1.0 / 3.0; 4], 3),
            Cell::Sixteenths => ([0.25; 4], 4),
            Cell::Dotted => ([0.75, 0.25, 0.0, 0.0], 2),
            Cell::QuarterTriplet => ([2.0 / 3.0; 4], 3),
            Cell::Long => ([2.0, 0.0, 0.0, 0.0], 1),
        }
    }

    fn beats(self) -> f64 {
        let (d, n) = self.rhythm();
        d[..n].iter().sum()
    }
}

// The cell weights come from each player's inter-onset intervals (share of 8ths, quarters,
// triplets, 16ths, dotted 8ths, quarter triplets, longer) divided by the notes in a cell; the
// intervals from their interval histograms; the rest as measured (phrases, rests, chord-tone
// shares, approaches, enclosures, pentatonic 4-note windows, anticipations, swing), tuned so
// the line's own statistics come out near theirs (`soloists_play_like_their_numbers`). The
// steps along the chord-scale already put a chord tone on ~2 strong beats in 3 (as they
// measured: 52-58%), so none of them needs the extra pull.

/// Miles Davis, 1951-59: 8.5-note phrases over 6.1 beats [3.9-10.2], 3.3 beats' rest
/// [2.0-4.9]; 8ths 40%, quarters 11%, longer 12%; 9% repeats, 62% steps, 7.6% leaps; 58%
/// chord tones on the strong beats, 70% of phrases end on one; 34% half-step approaches (2 in
/// 3 from below), 12% enclosures; trumpet around concert A4-Bb4.
pub const MILES: Profile = Profile {
    phrase: (6.1, 3.9, 10.2),
    rest: (3.3, 2.0, 4.9),
    cells: [19.8, 11.2, 2.7, 2.0, 5.7, 1.9, 5.9],
    intervals: [9.4, 28.0, 33.5, 13.1, 8.4, 3.8, 0.7, 1.8, 1.3],
    step_back: 0.24,
    strong_chord_tone: 0.0,
    end_chord_tone: 0.70,
    approach: 0.24,
    from_below: 0.69,
    enclosure: 0.10,
    pentatonic: 0.05,
    anticipation: 0.10,
    register: (72, 64, 82),
    legato: 0.88,
    swing: 1.36,
    falls: 0.25,
    // Shapes the most: a soft first note, louder early, fading out; the top note +4.4 dB;
    // the "and" louder than its beat 55% of the time (19% ghosted); spread 5.0 dB.
    arch: [-1.46, 1.06, 0.58, -0.83, -1.31],
    per_octave: 4.5,
    ghost: 0.19,
    accent: 1.66,
    long: -1.0,
    scatter: 3.0,
    motif: 0.15,
    sequence: 0.04,
    // His sequences mostly stay put (77%), else step down.
    moves: [(0, 7.7), (-2, 0.8), (-3, 0.8), (-1, 0.5), (2, 0.3)],
};

/// John Coltrane, 1960-64: 9-note cells over 5.3 beats [3.2-10.3], 2.0 beats' rest
/// [1.5-3.0], 16% before the beat; 8ths 34%, 16ths 16%, triplets 16%; 47% steps, 30% thirds,
/// 19% leaps; 54% chord tones on the strong beats (the 5th first), 57% of phrases end on one;
/// 14% half-step approaches, 6% enclosures; 34% of 4-note windows pentatonic; tenor.
pub const TRANE: Profile = Profile {
    phrase: (5.3, 3.2, 10.3),
    rest: (2.0, 1.5, 3.0),
    cells: [16.8, 6.5, 5.2, 3.5, 5.0, 1.3, 4.5],
    intervals: [4.4, 18.4, 28.2, 17.4, 12.7, 8.3, 2.5, 3.3, 4.7],
    step_back: 0.18,
    strong_chord_tone: 0.0,
    end_chord_tone: 0.57,
    approach: 0.08,
    from_below: 0.44,
    enclosure: 0.04,
    pentatonic: 0.12,
    anticipation: 0.16,
    register: (67, 58, 80),
    legato: 0.87,
    swing: 1.51,
    falls: 0.0,
    // Even: the flattest arch, no louder going up (1960 on), an even, sustained tone; 3.5 dB.
    arch: [0.65, 0.16, 0.0, -0.14, -0.68],
    per_octave: 0.0,
    ghost: 0.17,
    accent: 0.59,
    long: -0.74,
    scatter: 3.0,
    // 42% of his ideas are variants of the one before (the most of anyone); sequences stay
    // put or drop a minor third.
    motif: 0.3,
    sequence: 0.06,
    moves: [(0, 4.8), (-3, 1.4), (-2, 0.9), (7, 0.8), (-5, 0.5)],
};

/// Charlie Parker, 1945-51: 16-note lines over 6.6 beats [3.7-10.0], 1.6 beats' rest
/// [1.0-2.7], 16% before the beat; 16ths 21% (and double time), 8ths 26%, triplets 10%; 59%
/// steps, 27% thirds; 52% chord tones on the strong beats, 69% of phrases end on one; 36%
/// half-step approaches (most from above), 15% enclosures; alto.
pub const BIRD: Profile = Profile {
    phrase: (6.6, 3.7, 10.0),
    rest: (1.6, 1.0, 2.7),
    cells: [12.9, 2.2, 3.5, 7.0, 3.6, 1.3, 0.9],
    intervals: [2.3, 30.3, 28.8, 19.0, 8.1, 4.9, 1.0, 2.0, 3.6],
    step_back: 0.34,
    strong_chord_tone: 0.0,
    end_chord_tone: 0.69,
    approach: 0.22,
    from_below: 0.40,
    enclosure: 0.13,
    pentatonic: 0.05,
    anticipation: 0.16,
    register: (70, 62, 82),
    legato: 0.92,
    swing: 1.60,
    falls: 0.05,
    // Decaying lines: loud in, falling all the way to a soft last note (−3.7 dB); offbeats
    // ghosted 31% of the time; long notes much softer; 5.3 dB.
    arch: [1.28, 1.12, 0.07, -0.70, -3.68],
    per_octave: 4.5,
    ghost: 0.31,
    accent: 0.27,
    long: -4.0,
    scatter: 3.8,
    // The least motivic (a vocabulary, not development); his sequences step down.
    motif: 0.05,
    sequence: 0.03,
    moves: [(-2, 3.6), (0, 1.4), (-3, 1.4), (-5, 0.7), (-7, 0.7)],
};

/// Sonny Rollins, 1954-72: 11.5-note lines over 6.6 beats [3.9-13.3], 2.4 beats' rest
/// [1.5-3.4]; 8ths 46% (the longest 8th runs of anyone, 10 notes at p90), 16ths 14%;
/// 50% steps, 31% thirds, 15% leaps; 56% chord tones on the strong beats, 62% of phrases end
/// on one; 28% half-step approaches (either side), 14% enclosures; low on the tenor.
pub const SONNY: Profile = Profile {
    phrase: (6.6, 3.9, 13.3),
    rest: (2.4, 1.5, 3.4),
    cells: [22.8, 7.8, 3.4, 3.0, 4.8, 1.4, 2.0],
    intervals: [3.8, 23.3, 26.1, 21.0, 10.3, 7.9, 1.8, 2.8, 2.6],
    step_back: 0.19,
    strong_chord_tone: 0.0,
    end_chord_tone: 0.62,
    approach: 0.18,
    from_below: 0.51,
    enclosure: 0.11,
    pentatonic: 0.08,
    anticipation: 0.11,
    register: (64, 55, 77),
    legato: 0.88,
    swing: 1.50,
    falls: 0.1,
    // A mild arch with a soft last note (−1.7), the top note +1.5 dB, offbeats ghosted 21%.
    arch: [0.36, 0.39, -0.03, -0.10, -1.66],
    per_octave: 2.0,
    ghost: 0.21,
    accent: 0.45,
    long: -1.1,
    scatter: 3.2,
    // A motif developer: cells shared phrase to phrase at 1.9x chance, short-phrase relations
    // 17% (1-5% by chance).
    motif: 0.25,
    sequence: 0.05,
    moves: [(0, 4.8), (-2, 1.4), (-3, 1.0), (-1, 0.8), (5, 0.6)],
};

/// Wayne Shorter, 1961-66: 10-note phrases over 6.6 beats [4.1-10.2], 2.1 beats' rest
/// [1.4-3.4]; 16ths 19% and faster runs 11%, 8ths 32%, long notes 8%; 59% steps (38% whole
/// steps), 6% repeats, 14% leaps; turns (−2 +2 −2) his commonest cell; 52% chord tones on the
/// strong beats, 56% of phrases end on one; only 15% half-step approaches, 4% enclosures; 34%
/// of 4-note windows pentatonic; tenor.
pub const WAYNE: Profile = Profile {
    phrase: (6.6, 4.1, 10.2),
    rest: (2.4, 1.6, 3.6),
    cells: [15.9, 7.5, 3.5, 4.0, 3.8, 1.2, 5.5],
    intervals: [6.2, 21.1, 37.5, 14.7, 6.7, 7.7, 1.4, 1.6, 2.8],
    step_back: 0.20,
    strong_chord_tone: 0.0,
    end_chord_tone: 0.56,
    approach: 0.10,
    from_below: 0.35,
    enclosure: 0.03,
    pentatonic: 0.12,
    anticipation: 0.14,
    register: (67, 57, 82),
    legato: 0.88,
    swing: 1.42,
    falls: 0.12,
    // Even like late Trane: the top note barely louder (+0.3 dB), a soft last note (−1.7).
    arch: [0.12, 0.47, 0.33, -0.42, -1.70],
    per_octave: 0.5,
    ghost: 0.20,
    accent: 0.3,
    long: -1.0,
    scatter: 3.1,
    // The motif developer: 32% of his ideas are variants (13% of them transposed), shared cells
    // 2.5x chance, the most theme quotes; sequences mostly stay put.
    motif: 0.28,
    sequence: 0.06,
    moves: [(0, 5.0), (-2, 1.2), (2, 0.8), (-3, 0.8), (-5, 0.5)],
};

/// A note the line plays: where in the bar (beats), how long it sounds, the pitch, whether
/// it ends a phrase (for a fall), and how loud (dB against the solo's median).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Played {
    pub at: f64,
    pub len: f64,
    pub note: u8,
    pub last: bool,
    pub db: f32,
}

/// A phrase as played, for restating: its first note, then each note's interval from the one
/// before and its length (beats).
#[derive(Debug, Clone, Copy, Default)]
struct Motif {
    first: u8,
    iv: [i8; 32],
    dur: [f32; 32],
    n: usize,
}

impl Motif {
    fn push(&mut self, prev: Option<i32>, note: u8, d: f64) {
        if self.n == 0 {
            self.first = note;
        }
        if self.n < 32 {
            self.iv[self.n] = prev.map_or(0, |c| (note as i32 - c).clamp(-24, 24) as i8);
            self.dur[self.n] = d as f32;
            self.n += 1;
        }
    }

    /// Notes `from..from+len` as a motif of their own.
    fn slice(&self, notes: &[u8; 32], from: usize, len: usize) -> Motif {
        let mut m = Motif { first: notes[from], n: len, ..Motif::default() };
        m.iv[..len].copy_from_slice(&self.iv[from..from + len]);
        m.dur[..len].copy_from_slice(&self.dur[from..from + len]);
        m.iv[0] = 0;
        m
    }
}

/// A motif being restated: from note `i`, until note `until`, moved `by` semitones.
#[derive(Debug, Clone, Copy, Default)]
struct Replay {
    src: Motif,
    i: usize,
    until: usize,
    by: i32,
}

/// The line, carried over bar lines.
#[derive(Debug, Clone, Copy, Default)]
pub struct Line {
    /// The last note played (none at the start: the register's centre).
    cur: Option<i32>,
    dir: i32,
    /// Beats left in the phrase; when it's 0, beats left of the rest.
    phrase: f64,
    rest: f64,
    /// Notes queued (an approach into a target, an enclosure), next first.
    queue: [u8; 3],
    queued: usize,
    /// Free notes to play before the queued ones start.
    skip: u8,
    /// Pentatonic notes left in a run.
    penta: u8,
    /// The last interval was a leap (≥ 5).
    leapt: bool,
    /// A chromatic passing note must carry on by half step in its direction.
    passing: bool,
    started: bool,
    /// The phrase's length (beats), whether its first note is still to come, and the rest
    /// before it (for the dynamics).
    total: f64,
    first: bool,
    breath: f64,
    /// This phrase so far (and its pitches), the last phrase, and a restatement under way.
    now: Motif,
    pitches: [u8; 32],
    last_phrase: Motif,
    replay: Option<Replay>,
}

/// What the line needs from the bar: its length, and the harmony at a beat (past the bar
/// line too: an approach leads into the next bar's first chord).
pub trait Changes {
    fn bar_beats(&self) -> f64;
    fn harm(&self, beat: f64) -> Option<Harm>;
    /// Is beat `b` of the bar a strong one (1 or 3 in 4/4; the downbeat otherwise)?
    fn strong(&self, b: f64) -> bool {
        let bb = self.bar_beats();
        let k = b.round();
        (b - k).abs() < 1e-6 && (k.rem_euclid(bb) == 0.0 || bb == 4.0 && k == 2.0)
    }
}

impl Line {
    pub fn reset(&mut self) {
        *self = Line::default();
    }

    /// The bar's notes, by `p`, into `out` (beats from the bar line). `land`: the song's phrase
    /// ends with this bar (a phrase still going ends in it, more often than not).
    pub fn bar(&mut self, p: &Profile, ch: &impl Changes, r: &mut Rng, land: bool, out: &mut [Played; 24]) -> usize {
        let bb = ch.bar_beats();
        let mut n = 0;
        if !self.started {
            // A soloist comes in after a breath, not on the downbeat.
            self.started = true;
            self.dir = 1;
            self.rest = 0.5 + r.f() * 1.5;
            self.phrase = 0.0;
        }
        if land && self.phrase > bb && r.chance(0.6) {
            self.phrase = (bb - 0.5).max(1.0);
        }
        let mut b = 0.0;
        while b < bb - 1e-9 && n < out.len() {
            if self.phrase <= 1e-9 {
                if self.rest <= 1e-9 {
                    self.start_phrase(p, r);
                    continue;
                }
                let d = self.rest.min(bb - b);
                b += d;
                self.rest -= d;
                // A breath that would end on the bar line comes in on the "and" before it.
                if self.rest > 0.4 && (b + self.rest - bb).abs() < 1e-6 && r.chance(p.anticipation as f64 * 2.0) {
                    self.rest -= 0.5;
                }
                continue;
            }
            // Off the beat (after an anticipation or an odd rest): one 8th to the next beat.
            let off = b - b.floor();
            let replaying = self.replay.is_some();
            let (durs, k) = if let Some(rp) = self.replay {
                // A restatement keeps the motif's rhythm.
                let mut d = [0.0; 4];
                d[0] = rp.src.dur[rp.i] as f64;
                (d, 1)
            } else if off > 1e-6 {
                let mut d = [0.0; 4];
                d[0] = 1.0 - off;
                (d, 1)
            } else {
                let mut cell = pick_cell(p, r);
                if b + cell.beats() > bb + 1e-9 {
                    cell = Cell::Eighths;
                }
                if cell == Cell::Sixteenths && self.penta == 0 && r.chance(p.pentatonic as f64 * 1.5) {
                    self.penta = 4;
                } else if cell == Cell::Eighths && self.penta == 0 && r.chance(p.pentatonic as f64 * 0.6) {
                    self.penta = 4;
                }
                cell.rhythm()
            };
            let cell_end = b + durs[..k].iter().sum::<f64>();
            // Into a strong beat at the cell's end: maybe an enclosure or an approach.
            if !replaying && self.queued == 0 && self.penta == 0 && ch.strong(cell_end) && cell_end <= bb + 1e-9 && self.phrase > cell_end - b + 0.1 {
                if let Some(h) = ch.harm(cell_end) {
                    let cur = self.cur.unwrap_or(p.register.0);
                    let target = h.nearest_chord_tone((cur + 2 * self.dir).clamp(0, 127) as u8);
                    if k >= 2 && r.chance(p.enclosure as f64) {
                        let above = h.scale.step(target, 1).max(target + 1).min(target + 2);
                        self.load(&[above, target.saturating_sub(1), target], k - 2);
                    } else if r.chance(p.approach as f64) {
                        let a = if r.chance(p.from_below as f64) { target.saturating_sub(1) } else { target + 1 };
                        self.load(&[a, target], k - 1);
                    }
                }
            }
            for &d in &durs[..k] {
                if b >= bb - 1e-9 || n >= out.len() {
                    break;
                }
                let d = d.min(bb - b);
                let last = self.phrase <= d + 1e-6;
                let before = self.cur;
                let note = self.next(p, ch, r, b, last);
                if let Some(note) = note {
                    if self.now.n < 32 {
                        self.pitches[self.now.n] = note;
                    }
                    self.now.push(before.filter(|_| self.now.n > 0), note, d);
                    let hold = if last { (d + self.rest_hint(p).min(1.0)).min(bb - b) } else { d };
                    let db = self.loudness(p, r, b, d, note, last);
                    out[n] = Played { at: b, len: (hold * p.legato as f64).max(0.05), note, last, db };
                    n += 1;
                }
                b += d;
                self.phrase -= d;
                if self.phrase <= 1e-6 {
                    self.end_phrase(p, r);
                    break;
                }
            }
            // A free cell of 3-4 notes with a turn or a leap in it, maybe restated straight
            // after: a sequence.
            if !replaying && self.replay.is_none() && self.phrase > 1.0 && (3..=4).contains(&k) && self.now.n >= k {
                let from = self.now.n - k;
                let ivs = &self.now.iv[from + 1..self.now.n];
                let shaped = ivs.iter().any(|i| i.abs() >= 3) || ivs.windows(2).any(|w| w[0].signum() * w[1].signum() < 0);
                if shaped && r.chance(p.sequence as f64 * 4.0) {
                    let src = self.now.slice(&self.pitches, from, k);
                    self.replay = Some(Replay { src, i: 0, until: k, by: pick_move(p, r) });
                }
            }
        }
        n
    }

    /// How loud a note at beat `b` (length `d`) is, dB against the solo's median.
    fn loudness(&mut self, p: &Profile, r: &mut Rng, b: f64, d: f64, note: u8, last: bool) -> f32 {
        let done = 1.0 - self.phrase / self.total.max(1e-6);
        let pos = if std::mem::take(&mut self.first) {
            0
        } else if last {
            4
        } else {
            1 + (done * 3.0).clamp(0.0, 2.0) as usize
        };
        let mut db = p.arch[pos] + p.per_octave * (note as f32 - p.register.0 as f32) / 12.0;
        if ((b - b.floor()) - 0.5).abs() < 1e-6 {
            db += if r.chance(p.ghost as f64) { -4.0 } else { p.accent };
        }
        if d >= 1.2 {
            db += p.long;
        }
        // Louder after a longer breath (−0.9 dB after under half a beat, −0.3 after 4).
        db += 0.17 * (self.breath.min(4.0) as f32 - 2.0);
        db += p.scatter * normal(r) as f32;
        db.clamp(-9.0, 9.0)
    }

    fn load(&mut self, notes: &[u8], skip: usize) {
        // `skip` free notes first, then the queued ones (the last lands on the strong beat).
        self.queued = notes.len().min(3);
        self.queue[..self.queued].copy_from_slice(&notes[..self.queued]);
        self.skip = skip as u8;
    }

    fn rest_hint(&self, p: &Profile) -> f64 {
        p.rest.0 * 0.5
    }

    fn start_phrase(&mut self, p: &Profile, r: &mut Rng) {
        self.phrase = lognormal(r, p.phrase).clamp(1.0, 24.0);
        self.total = self.phrase;
        self.first = true;
        self.queued = 0;
        self.skip = 0;
        self.penta = 0;
        self.passing = false;
        self.now = Motif::default();
        self.replay = None;
        let prev = self.last_phrase;
        if prev.n >= 5 && r.chance(p.motif as f64) {
            // The last idea again: its head, then a new ending (most break off at note 5 or 6;
            // a fifth carry it whole, and stop or go on).
            let x = r.f();
            let until = if x < 0.41 { 4 } else if x < 0.61 { 5 } else if x < 0.8 { 6 + r.below(3) } else { prev.n };
            let until = until.min(prev.n);
            let head: f64 = prev.dur[..until].iter().map(|&d| d as f64).sum();
            self.phrase = self.phrase.max(head + if until < prev.n { 1.0 } else { 0.0 });
            self.total = self.phrase;
            self.replay = Some(Replay { src: prev, i: 0, until, by: pick_move(p, r) });
            return;
        }
        // A new phrase starts from somewhere new: up or down, or back towards the middle.
        let cur = self.cur.unwrap_or(p.register.0);
        self.dir = if cur > p.register.0 + 4 { -1 } else if cur < p.register.0 - 4 { 1 } else if r.chance(0.5) { 1 } else { -1 };
        if r.chance(0.5) {
            self.cur = Some(cur + self.dir * (2 + r.below(4) as i32));
        }
    }

    fn end_phrase(&mut self, p: &Profile, r: &mut Rng) {
        self.phrase = 0.0;
        self.replay = None;
        if self.now.n >= 4 {
            self.last_phrase = self.now;
        }
        self.queued = 0;
        self.skip = 0;
        self.penta = 0;
        // Whole 8ths of rest, so the next phrase starts on the 8th grid.
        self.rest = (lognormal(r, p.rest).clamp(0.5, 10.0) * 2.0).round() / 2.0;
        self.breath = self.rest;
    }

    /// The next note (none when there's no harmony to play on).
    fn next(&mut self, p: &Profile, ch: &impl Changes, r: &mut Rng, b: f64, last: bool) -> Option<u8> {
        let h = ch.harm(b)?;
        let (centre, floor, ceil) = p.register;
        let cur = self.cur.unwrap_or(centre);
        if self.skip > 0 {
            self.skip -= 1;
        } else if self.queued > 0 {
            let note = self.queue[0];
            self.queue.copy_within(1..3, 0);
            self.queued -= 1;
            self.dir = (note as i32 - cur).signum().max(-1).min(1);
            self.dir = if self.dir == 0 { 1 } else { self.dir };
            self.cur = Some(note as i32);
            return Some(note);
        }
        let strong = ch.strong(b);
        if let Some(rp) = self.replay.as_mut() {
            // The motif's shape at the new spot, bent to the chord where it doesn't fit.
            let want = if rp.i == 0 { rp.src.first as i32 + rp.by } else { cur + rp.src.iv[rp.i] as i32 };
            let n = want.clamp(floor - 6, ceil + 6).clamp(0, 127) as u8;
            let n = if h.scale.contains(n % 12) || !strong && r.chance(0.5) { n } else { h.scale.snap(n) };
            let n = if last && r.chance(p.end_chord_tone as f64) { h.nearest_chord_tone(n) } else { n };
            rp.i += 1;
            if rp.i >= rp.until.min(rp.src.n) {
                self.replay = None;
            }
            if n as i32 != cur {
                self.dir = (n as i32 - cur).signum();
            }
            self.leapt = (n as i32 - cur).abs() >= 5;
            self.passing = false;
            self.cur = Some(n as i32);
            return Some(n);
        }
        if self.passing {
            // A chromatic passing note goes on through by half step.
            self.passing = false;
            let n = (cur + self.dir) as u8;
            self.cur = Some(n as i32);
            return Some(n);
        }
        if cur > ceil {
            self.dir = -1;
        } else if cur < floor {
            self.dir = 1;
        }
        let mut note = if self.penta > 0 {
            self.penta -= 1;
            next_in(&pentatonic(&h), cur.clamp(0, 127) as u8, self.dir) as i32
        } else {
            let size = pick(&p.intervals, r);
            if self.leapt && r.chance(p.step_back as f64) {
                self.dir = -self.dir;
            } else if size > 0 && r.chance(0.2) {
                self.dir = -self.dir;
            }
            let size = if size == 8 { 8 + r.below(5) as i32 } else { size as i32 };
            let want = cur + self.dir * size;
            match size {
                0 => cur,
                1 => {
                    let n = want.clamp(0, 127) as u8;
                    if h.scale.contains(n % 12) {
                        want
                    } else if !strong && !last && r.chance(0.5) {
                        // Chromatic: through to the next scale note.
                        self.passing = h.scale.contains(((want + self.dir).rem_euclid(12)) as u8);
                        want
                    } else {
                        h.scale.step(cur.clamp(0, 127) as u8, self.dir) as i32
                    }
                }
                2 => h.scale.step(cur.clamp(0, 127) as u8, self.dir) as i32,
                // Thirds and fourths along the chord, most of the time.
                3..=5 if r.chance(0.6) => chord_tone_toward(&h, cur, self.dir, size),
                _ => h.scale.snap(want.clamp(0, 127) as u8) as i32,
            }
        };
        if (strong && r.chance(p.strong_chord_tone as f64)) || (last && r.chance(p.end_chord_tone as f64)) {
            note = h.nearest_chord_tone(note.clamp(0, 127) as u8) as i32;
        }
        note = note.clamp(floor - 6, ceil + 6);
        self.leapt = (note - cur).abs() >= 5;
        if note != cur {
            self.dir = (note - cur).signum();
        }
        self.cur = Some(note);
        Some(note as u8)
    }
}

/// The chord tone `size`-ish semitones from `cur` in `dir` (the nearest past `cur`).
fn chord_tone_toward(h: &Harm, cur: i32, dir: i32, size: i32) -> i32 {
    let want = cur + dir * size;
    (0..=4)
        .flat_map(|d| [want + d * dir, want - d * dir])
        .find(|&n| (n - cur) * dir > 0 && (0..=127).contains(&n) && h.is_chord_tone(n as u8))
        .unwrap_or(want)
}

/// The chord's pentatonic: major on major and dominant chords, minor on minor ones.
fn pentatonic(h: &Harm) -> [u8; 5] {
    let semis: [u8; 5] = match h.chord.family() {
        Family::Minor | Family::HalfDim | Family::Dim => [0, 3, 5, 7, 10],
        _ => [0, 2, 4, 7, 9],
    };
    semis.map(|s| (h.chord.root + s) % 12)
}

/// A sequence's (or a restatement's) transposition.
fn pick_move(p: &Profile, r: &mut Rng) -> i32 {
    let w: [f32; 5] = p.moves.map(|m| m.1);
    p.moves[pick(&w, r)].0 as i32
}

fn pick_cell(p: &Profile, r: &mut Rng) -> Cell {
    CELLS[pick(&p.cells, r)]
}

/// An index into `w`, by weight.
fn pick(w: &[f32], r: &mut Rng) -> usize {
    let total: f32 = w.iter().sum();
    let mut x = r.f() as f32 * total;
    for (i, &v) in w.iter().enumerate() {
        if x < v {
            return i;
        }
        x -= v;
    }
    w.len() - 1
}

/// A log-normal draw with `median` and interquartile range `q1..q3`.
fn lognormal(r: &mut Rng, (median, q1, q3): (f64, f64, f64)) -> f64 {
    let sigma = (q3 / q1).ln() / 1.349;
    median * (sigma * normal(r)).exp()
}

/// A standard normal draw.
fn normal(r: &mut Rng) -> f64 {
    let (u, v) = (r.f().max(1e-9), r.f());
    (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::chart::{Chord, Quality};

    /// ii-V-I-I in C, two beats a chord.
    struct Turnaround(u64);

    impl Changes for Turnaround {
        fn bar_beats(&self) -> f64 {
            4.0
        }

        fn harm(&self, beat: f64) -> Option<Harm> {
            let k = (self.0 * 2 + (beat / 2.0).floor() as u64) % 8;
            let (root, q) = [(2, Quality::Minor7), (7, Quality::Dom7), (0, Quality::Maj7), (0, Quality::Maj7)][(k / 2) as usize];
            Some(Harm::new(Chord::new(root, q)))
        }
    }

    #[derive(Debug, Default)]
    struct Stats {
        notes_per_bar: f64,
        silent: f64,
        repeats: f64,
        steps: f64,
        leaps: f64,
        strong_chord_tones: f64,
        pentatonic: f64,
        /// Pitch-loudness correlation, the first and last notes' mean dB, the spread.
        pitch_db: f64,
        first_db: f64,
        last_db: f64,
        spread_db: f64,
    }

    fn play(p: &Profile, bars: u64) -> Stats {
        let mut line = Line::default();
        let mut r = Rng::new(11);
        let mut all: Vec<(f64, Played, Harm)> = Vec::new();
        for bar in 0..bars {
            let ch = Turnaround(bar);
            let mut out = [Played::default(); 24];
            let n = line.bar(p, &ch, &mut r, bar % 8 == 7, &mut out);
            all.extend(out[..n].iter().map(|k| (bar as f64 * 4.0 + k.at, *k, ch.harm(k.at).unwrap())));
        }
        let sounding: f64 = all.iter().map(|(_, k, _)| k.len).sum();
        let iv: Vec<i32> = all.windows(2).filter(|w| !w[0].1.last).map(|w| w[1].1.note as i32 - w[0].1.note as i32).collect();
        let share = |f: &dyn Fn(i32) -> bool| iv.iter().filter(|&&i| f(i)).count() as f64 / iv.len() as f64;
        let strong: Vec<_> = all.iter().filter(|(t, ..)| (t - t.round()).abs() < 1e-6 && (t.round() as i64) % 2 == 0).collect();
        let windows = all.windows(4).filter(|w| w.iter().all(|x| !x.1.last || std::ptr::eq(x, &w[3])));
        let (mut penta, mut wins) = (0, 0);
        for w in windows {
            wins += 1;
            let pcs: Vec<u8> = w.iter().map(|x| x.1.note % 12).collect();
            let fits = (0..12u8).any(|root| [[0, 2, 4, 7, 9], [0, 3, 5, 7, 10]].iter().any(|s| pcs.iter().all(|pc| s.contains(&((pc + 12 - root) % 12)))));
            penta += fits as u32;
        }
        let db: Vec<f64> = all.iter().map(|x| x.1.db as f64).collect();
        let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
        let pitch: Vec<f64> = all.iter().map(|x| x.1.note as f64).collect();
        let (mp, md) = (mean(&pitch), mean(&db));
        let cov: f64 = pitch.iter().zip(&db).map(|(a, b)| (a - mp) * (b - md)).sum();
        let sd = |v: &[f64], m: f64| v.iter().map(|x| (x - m).powi(2)).sum::<f64>().sqrt();
        let lasts: Vec<f64> = all.iter().filter(|x| x.1.last).map(|x| x.1.db as f64).collect();
        let firsts: Vec<f64> = all.windows(2).filter(|w| w[0].1.last).map(|w| w[1].1.db as f64).collect();
        Stats {
            pitch_db: cov / (sd(&pitch, mp) * sd(&db, md)),
            first_db: mean(&firsts),
            last_db: mean(&lasts),
            spread_db: sd(&db, md) / (db.len() as f64).sqrt(),
            notes_per_bar: all.len() as f64 / bars as f64,
            silent: 1.0 - sounding / (bars as f64 * 4.0),
            repeats: share(&|i| i == 0),
            steps: share(&|i| (1..=2).contains(&i.abs())),
            leaps: share(&|i| i.abs() >= 5),
            strong_chord_tones: strong.iter().filter(|(_, k, h)| h.is_chord_tone(k.note)).count() as f64 / strong.len() as f64,
            pentatonic: penta as f64 / wins as f64,
        }
    }

    #[test]
    fn soloists_play_like_their_numbers() {
        let (m, t, b) = (play(&MILES, 400), play(&TRANE, 400), play(&BIRD, 400));
        eprintln!("miles {m:?}\ntrane {t:?}\nbird {b:?}");
        // Miles: space and steps; Trane: dense, thirds and leaps, pentatonic; Bird: densest.
        assert!(m.silent > t.silent + 0.05 && m.notes_per_bar < t.notes_per_bar - 1.0, "{m:?} {t:?}");
        assert!((2.5..5.5).contains(&m.notes_per_bar) && (4.0..7.5).contains(&t.notes_per_bar), "{m:?} {t:?}");
        assert!(b.notes_per_bar > m.notes_per_bar + 1.0, "{b:?}");
        assert!(m.steps > t.steps + 0.08 && m.repeats > t.repeats && t.leaps > m.leaps + 0.04, "{m:?} {t:?}");
        assert!(t.pentatonic > m.pentatonic + 0.05, "{m:?} {t:?}");
        // Dynamics: Miles louder going up and arching, Trane even, Bird fading to the end.
        assert!(m.pitch_db > 0.3 && t.pitch_db.abs() < 0.15, "{m:?} {t:?}");
        assert!(b.first_db - b.last_db > 3.0 && t.first_db - t.last_db < 2.5, "{b:?} {t:?}");
        assert!(m.spread_db > t.spread_db + 0.5, "{m:?} {t:?}");
        // Sonny: long 8th lines, thirds; Wayne: whole steps and pentatonic, fewer leaps than Trane.
        let (so, w) = (play(&SONNY, 400), play(&WAYNE, 400));
        eprintln!("sonny {so:?}\nwayne {w:?}");
        assert!((4.0..7.0).contains(&so.notes_per_bar) && (3.5..6.5).contains(&w.notes_per_bar), "{so:?} {w:?}");
        assert!(w.steps > t.steps && w.pentatonic > m.pentatonic, "{w:?}");
        for s in [&m, &t, &b, &so, &w] {
            assert!((0.45..0.8).contains(&s.strong_chord_tones), "{s:?}");
        }
    }
}
