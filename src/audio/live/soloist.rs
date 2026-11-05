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
//!   notes, few approaches, an even tone, and the most variants.
//! - [`Soloist::Pops`]: Louis Armstrong, 1925-28 (8 solos): short breaths, phrases pushed in
//!   on the "and", repeated notes and arpeggios, the most pentatonic of all, high and well
//!   behind the beat.
//!
//! Each one's numbers (and their licks: ours, written to rhyme with theirs) are in
//! `music/players/<slug>.player`.
//!
//! The line carries over bar lines (a phrase, then a rest, then the next), so it's a [`Line`]
//! the lead keeps between bars. Pure, no allocation.

use crate::audio::accomp::Rng;
use crate::audio::chart::Family;
use std::sync::OnceLock;

use crate::audio::live::ornament::{Harm, nearest_pc, next_in};
use crate::audio::live::player_file::PlayerFile;

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
    Pops,
    /// The funk feel's soloists (not one player: what they share), in the band's funk sections.
    Funk,
    /// The Latin feel's, in its bossa and samba sections.
    Latin,
}

impl Soloist {
    pub const ALL: [Soloist; 9] =
        [Soloist::Band, Soloist::Miles, Soloist::Trane, Soloist::Bird, Soloist::Sonny, Soloist::Wayne, Soloist::Pops, Soloist::Funk, Soloist::Latin];
    /// Those who solo over swing (the solo goes round them: [`take_the_solo`]).
    pub const SWING: [Soloist; 7] = [Soloist::Band, Soloist::Miles, Soloist::Trane, Soloist::Bird, Soloist::Sonny, Soloist::Wayne, Soloist::Pops];

    pub fn slug(self) -> &'static str {
        match self {
            Soloist::Band => "band",
            Soloist::Miles => "miles",
            Soloist::Trane => "trane",
            Soloist::Bird => "bird",
            Soloist::Sonny => "sonny",
            Soloist::Wayne => "wayne",
            Soloist::Pops => "pops",
            Soloist::Funk => "funk",
            Soloist::Latin => "latin",
        }
    }

    pub fn parse(s: &str) -> Option<Soloist> {
        Soloist::ALL.into_iter().find(|p| p.slug() == s)
    }

    /// The player's file, parsed once (none for the band's own lines).
    pub fn player(self) -> Option<&'static PlayerFile> {
        static PLAYERS: OnceLock<Vec<(Soloist, PlayerFile)>> = OnceLock::new();
        let all = PLAYERS.get_or_init(|| {
            FILES.iter().map(|(s, text)| (*s, PlayerFile::parse(text).unwrap_or_else(|e| panic!("music/players/{}.player: {e}", s.slug())))).collect()
        });
        all.iter().find(|(s, _)| *s == self).map(|(_, p)| p)
    }

    /// The measured profile (none for the band's own lines).
    pub fn profile(self) -> Option<&'static Profile> {
        self.player().map(|p| &p.profile)
    }

    /// The lead's lay-back on the beat against the band's average soloist ([`super::pocket`]).
    pub fn lay_back(self) -> f32 {
        self.player().map_or(1.0, |p| p.lay_back)
    }

    /// The tempos the player's measured numbers hold at (bpm): about the range of their
    /// transcribed solos. The band's own line fits anything.
    fn tempos(self) -> (f32, f32) {
        match self {
            Soloist::Band => (0.0, f32::INFINITY),
            Soloist::Pops => (80.0, 230.0),
            Soloist::Miles => (70.0, 270.0),
            Soloist::Bird => (140.0, 330.0),
            Soloist::Sonny => (90.0, 300.0),
            Soloist::Trane => (110.0, 340.0),
            Soloist::Wayne => (100.0, 290.0),
            // Picked by the feel, not passed the solo.
            Soloist::Funk | Soloist::Latin => (f32::INFINITY, 0.0),
        }
    }

    /// Plays at `bpm` (and in three: the waltz is no place for 1920s two-beat or bebop).
    pub fn fits(self, bpm: f32, waltz: bool) -> bool {
        let (lo, hi) = self.tempos();
        (lo..=hi).contains(&bpm) && !(waltz && matches!(self, Soloist::Pops | Soloist::Bird))
    }

    /// Calm (space, few notes: they suit a night the band plays it safe) or busy.
    fn calm(self) -> bool {
        matches!(self, Soloist::Pops | Soloist::Miles | Soloist::Wayne)
    }

    /// Where the player sits in the story of the music, so a handover can go to a neighbour.
    fn era(self) -> Option<i32> {
        [Soloist::Pops, Soloist::Bird, Soloist::Miles, Soloist::Sonny, Soloist::Trane, Soloist::Wayne].iter().position(|s| *s == self).map(|i| i as i32)
    }
}

/// Who takes the solo in loop pass `pass` (a chorus each, as a band passes the solo round):
/// one of those who [`Soloist::fits`] the tune, never the one before (`prev`), a neighbour in
/// the music's story more likely than a jump across it, and the calm players more likely when
/// the band's `freedom` is low, the busy ones when it's high. The tunes are from the 1920s, so
/// Pops takes the first chorus when he fits. Seeded, the same every time.
pub fn take_the_solo(seed: u64, pass: u64, prev: Option<Soloist>, bpm: f32, waltz: bool, freedom: f32) -> Soloist {
    if prev.is_none() && Soloist::Pops.fits(bpm, waltz) {
        return Soloist::Pops;
    }
    let f = freedom.clamp(0.0, 1.0);
    let weight = |s: Soloist| -> f32 {
        if Some(s) == prev || !s.fits(bpm, waltz) {
            return 0.0;
        }
        let mood = if s == Soloist::Band { 0.6 } else if s.calm() { 1.4 - f } else { 0.4 + f };
        let near = match (prev.and_then(Soloist::era), s.era()) {
            (Some(a), Some(b)) => [1.0, 1.0, 0.6, 0.35][((a - b).abs() as usize).min(3)],
            _ => 0.8,
        };
        mood * near
    };
    let total: f32 = Soloist::SWING.iter().map(|&s| weight(s)).sum();
    if total <= 0.0 {
        return Soloist::Band;
    }
    let mut r = Rng::new(seed ^ pass.wrapping_mul(0x50_10_15_75));
    let mut x = r.f() as f32 * total;
    for s in Soloist::SWING {
        x -= weight(s);
        if x < 0.0 && weight(s) > 0.0 {
            return s;
        }
    }
    Soloist::Band
}

// The profiles, one file each. In them, the cell weights come from each player's inter-onset
// intervals (share of 8ths, quarters, triplets, 16ths, dotted 8ths, quarter triplets, longer)
// divided by the notes in a cell; the intervals from their interval histograms; the rest as
// measured (phrases, rests, chord-tone shares, approaches, enclosures, pentatonic 4-note
// windows, anticipations, swing, dynamics, motifs), tuned so the line's own statistics come
// out near theirs (`soloists_play_like_their_numbers`). The steps along the chord-scale already
// put a chord tone on ~2 strong beats in 3 (as they measured: 52-58%), so none of them needs
// the extra pull.
const FILES: [(Soloist, &str); 8] = [
    (Soloist::Miles, include_str!("../../../music/players/miles.player")),
    (Soloist::Trane, include_str!("../../../music/players/trane.player")),
    (Soloist::Bird, include_str!("../../../music/players/bird.player")),
    (Soloist::Sonny, include_str!("../../../music/players/sonny.player")),
    (Soloist::Wayne, include_str!("../../../music/players/wayne.player")),
    (Soloist::Pops, include_str!("../../../music/players/pops.player")),
    (Soloist::Funk, include_str!("../../../music/players/funk.player")),
    (Soloist::Latin, include_str!("../../../music/players/latin.player")),
];

/// How a player builds lines. Lengths in beats; shares 0..1. Written in `.player` files
/// ([`super::player_file`]).
#[derive(Debug, Clone, Copy, Default)]
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
    /// Where a phrase ends (all zero: anywhere, its last note snapped to a chord tone
    /// `end_chord_tone` of the time). The beat its target lands on (1, 2, 3, 4, or off the
    /// beat: a short note on an "and"), and how often an on-beat target is anticipated (its
    /// note an 8th early, held into the beat, against the beat's chord). The target's degree:
    /// R 3 5 7 9 11 13; and how often the last note is a half step off its chord tone instead,
    /// left unresolved, with the share of those from above.
    pub end_beats: [f32; 5],
    pub end_push: f32,
    pub end_tones: [f32; 7],
    pub end_chromatic: f32,
    pub end_above: f32,
    /// Where a phrase starts (all zero: wherever the rest runs out). On the beat, on the "and"
    /// or elsewhere (a 16th or a triplet before the beat), and the beat (1-4) for an on-beat
    /// start and for an off-beat one (the "and" of 4: a pickup into the bar). Its first note's
    /// degree (R 3 5 7 9 11 13), or a half step off a chord tone, stepping into it; and how
    /// often the line then goes up.
    pub start_place: [f32; 3],
    pub start_on: [f32; 4],
    pub start_and: [f32; 4],
    pub start_tones: [f32; 7],
    pub start_chromatic: f32,
    pub start_up: f32,
    /// Notes per bar at their tempos: under 140 bpm, 140-200, 200-260, over 260, and over all
    /// their solos (all zero: the same line at any tempo). The line thickens or thins its fast
    /// cells to match the tune's tempo ([`Profile::tempo_density`]).
    pub density: [f32; 5],
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
    /// The licks a phrase can open with.
    pub licks: Licks,
}

impl Profile {
    /// Does the player aim their phrase endings (`end_beats`, `end_tones`)?
    pub fn aims(&self) -> bool {
        self.end_beats.iter().sum::<f32>() > 0.0
    }

    /// Where a phrase that could end `len` beats after bar position `b` ends instead: on a
    /// target beat (beats from `b`, at least `min`), whether it's anticipated, and whether
    /// it's off the beat (a short note on an "and").
    fn aim(&self, r: &mut Rng, b: f64, bb: f64, len: f64, min: f64) -> (f64, bool, bool) {
        let i = pick(&self.end_beats, r);
        let beats = bb.round().max(1.0) as usize;
        let (pos, push) = if i == 4 { ((r.below(beats)) as f64 + 0.5, false) } else { ((i % beats) as f64, r.chance(self.end_push as f64)) };
        let at = pos - if push { 0.5 } else { 0.0 };
        let mut t = at + ((b + len - at) / bb).round() * bb;
        while t - b < min.max(1.0) {
            t += bb;
        }
        (t - b, push, i == 4)
    }

    /// The last note of a phrase near `note` over `h`: a target degree, or a half step off it.
    fn end_note(&self, h: &Harm, note: u8, r: &mut Rng) -> u8 {
        let chromatic = r.chance(self.end_chromatic as f64);
        let tones = if chromatic { [self.end_tones[0], self.end_tones[1], self.end_tones[2], self.end_tones[3], 0.0, 0.0, 0.0] } else { self.end_tones };
        let n = tone_near(h, pick(&tones, r), note) as i32;
        let n = if chromatic { n + if r.chance(self.end_above as f64) { 1 } else { -1 } } else { n };
        n.clamp(0, 127) as u8
    }

    /// Does the player place their phrase starts (`start_place` ...)?
    pub fn starts(&self) -> bool {
        self.start_place.iter().sum::<f32>() > 0.0
    }

    /// Their notes per bar at `bpm` against their figure over all their solos (1 with no tempo
    /// or no `density`): between the tempo bands' centres, flat past the ends.
    fn tempo_density(&self, bpm: f32) -> f32 {
        let d = &self.density;
        if bpm <= 0.0 || d[4] <= 0.0 {
            return 1.0;
        }
        const C: [f32; 4] = [115.0, 170.0, 230.0, 290.0];
        let at = if bpm <= C[0] {
            d[0]
        } else if bpm >= C[3] {
            d[3]
        } else {
            let i = (0..3).find(|&i| bpm < C[i + 1]).unwrap_or(2);
            d[i] + (d[i + 1] - d[i]) * (bpm - C[i]) / (C[i + 1] - C[i])
        };
        at / d[4]
    }

    /// A start slot in the bar (beats from the bar line).
    fn start_slot(&self, r: &mut Rng, bb: f64) -> f64 {
        let beats = bb.round().max(1.0) as usize;
        let (w, off) = match pick(&self.start_place, r) {
            0 => (&self.start_on, 0.0),
            1 => (&self.start_and, 0.5),
            // A 16th or a triplet into the next beat, or a 16th after this one.
            _ => (&self.start_and, [0.75, 2.0 / 3.0, 0.25][r.below(3)]),
        };
        (pick(w, r) % beats) as f64 + off
    }

    /// A phrase's first note near `note` over `h`, and the chord tone it steps into when it's a
    /// half step off one.
    fn start_note(&self, h: &Harm, note: u8, r: &mut Rng) -> (u8, Option<u8>) {
        if r.chance(self.start_chromatic as f64) {
            let t = tone_near(h, pick(&self.start_tones[..4], r), note);
            let a = if r.chance(self.from_below as f64) { t.saturating_sub(1) } else { t.saturating_add(1).min(127) };
            return (a, Some(t));
        }
        (tone_near(h, pick(&self.start_tones, r), note), None)
    }
}

/// Degree `k` (R 3 5 7 9 11 13) of `h` nearest `note`.
fn tone_near(h: &Harm, k: usize, note: u8) -> u8 {
    let iv = h.chord.quality.intervals();
    let fam = h.chord.family();
    let seventh = iv.get(3).copied().unwrap_or(if fam == Family::Major { 9 } else { 10 }) % 12;
    let pc = match k {
        0 => h.chord.root,
        1 => (h.chord.root + iv[1]) % 12,
        2 => (h.chord.root + iv[2]) % 12,
        3 => (h.chord.root + seventh) % 12,
        k => {
            // The scale's 9th, 11th or 13th (no natural 11 over a major 3rd: the 9th).
            let mut d = [1, 3, 5][k - 4];
            if d == 3 && matches!(fam, Family::Major | Family::Dominant) && h.degree(3, 0) % 12 == (h.chord.root + 5) % 12 {
                d = 1;
            }
            h.degree(d, 0) % 12
        }
    };
    nearest_pc(pc, note as i32, note as i32 - 6, note as i32 + 6)
}

/// What chord a lick is written over (its family).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Fits {
    #[default]
    Any,
    Major,
    Dominant,
    Minor,
    HalfDim,
}

impl Fits {
    pub fn parse(s: &str) -> Option<Fits> {
        Some(match s {
            "any" => Fits::Any,
            "maj" => Fits::Major,
            "dom" => Fits::Dominant,
            "min" => Fits::Minor,
            "half" => Fits::HalfDim,
            _ => return None,
        })
    }

    pub fn fits(self, f: Family) -> bool {
        match self {
            Fits::Any => true,
            Fits::Major => f == Family::Major,
            Fits::Dominant => f == Family::Dominant,
            Fits::Minor => f == Family::Minor,
            Fits::HalfDim => matches!(f, Family::HalfDim | Family::Dim),
        }
    }
}

/// A player's licks: phrases written over a C chord of their family, played from the chord's
/// root (the octave nearest the line). A phrase opens with one `rate` of the time when one fits
/// the chord it starts on; the lick plays exactly as written, then the line goes on its own.
#[derive(Debug, Clone, Copy, Default)]
pub struct Licks {
    pub rate: f32,
    list: [(Fits, Motif); Licks::MAX],
    n: usize,
}

impl Licks {
    pub const MAX: usize = 24;
    /// The most notes in a lick.
    pub const NOTES: usize = 16;

    /// Add a lick: its notes (MIDI, length in beats).
    pub fn add(&mut self, fits: Fits, notes: &[(u8, f64)]) -> Result<(), String> {
        if self.n == Licks::MAX {
            return Err(format!("at most {} licks", Licks::MAX));
        }
        if !(2..=Licks::NOTES).contains(&notes.len()) {
            return Err(format!("a lick is 2 to {} notes", Licks::NOTES));
        }
        let mut m = Motif::default();
        let mut prev = None;
        for &(note, d) in notes {
            m.push(prev, note, d);
            prev = Some(note as i32);
        }
        self.list[self.n] = (fits, m);
        self.n += 1;
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// One that fits `family`, at random.
    fn pick(&self, family: Family, r: &mut Rng) -> Option<Motif> {
        let fitting = || self.list[..self.n].iter().filter(|(f, _)| f.fits(family));
        let count = fitting().count();
        (count > 0).then(|| fitting().nth(r.below(count)).unwrap().1)
    }
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
    /// The chord root it was played over, if known (a restatement moves with the changes).
    root: Option<u8>,
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
        let mut m = Motif { first: notes[from], n: len, root: self.root, ..Motif::default() };
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
    /// A lick: as written, never bent to the scale.
    exact: bool,
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
    /// The phrase's last note is anticipated: an 8th early, against the next beat's chord.
    push: bool,
    /// Its last note is short, off the beat.
    short: bool,
    /// The way the line heads after a phrase's first note.
    start_dir: Option<i32>,
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
    /// The bar can see ahead ([`Changes::ahead`]): the line plans past it.
    look: bool,
    /// Where the solo is in its arc over the chorus (0 calm .. 1 the peak; none: no arc), and
    /// this bar's weight on the fast cells (the tempo and the arc).
    heat: Option<f32>,
    dense: f32,
    /// The phrase's skeleton: target notes at beats into the phrase (chord changes and bar
    /// lines on strong beats, voice-led), and the next one still ahead.
    goals: [(f64, u8); 8],
    ngoals: usize,
    gi: usize,
    /// How many times running the idea has been restated (a third time turns to something new).
    idea: u8,
    /// The phrase ends on a band hit (accented).
    hit: bool,
    /// The comp's figure last bar (a phrase can pick it up).
    figure: Figure,
}

/// The arc's mean over a chorus (its effects are centred on it, so a chorus averages out
/// where the player's numbers are).
const HEAT_MEAN: f32 = 0.59;

/// How hot the solo is `x` of the way through the chorus: it starts calm, builds to a peak
/// three quarters through, and eases off into the next chorus.
fn arc(x: f64) -> f32 {
    let x = x.clamp(0.0, 1.0) as f32;
    if x < 0.75 { 0.25 + 0.75 * (x / 0.75).powf(1.5) } else { 1.0 - 0.6 * (x - 0.75) / 0.25 }
}

/// A phrase picks up the comp's last figure this often (when the bar can see ahead).
const ECHO: f64 = 0.12;

/// Fast cells: triplets and 16ths.
fn dense_cell(c: Cell) -> bool {
    matches!(c, Cell::Triplet | Cell::Sixteenths)
}

/// Notes a beat from the cells `w`, the fast ones weighted `k`.
fn cell_rate(w: &[f32; 7], k: f32) -> f32 {
    let (mut notes, mut beats) = (0.0, 0.0);
    for (i, c) in CELLS.iter().enumerate() {
        let wi = w[i] * if dense_cell(*c) { k } else { 1.0 };
        let (_, n) = c.rhythm();
        notes += wi * n as f32;
        beats += wi * c.beats() as f32;
    }
    if beats > 0.0 { notes / beats } else { 0.0 }
}

/// The weight on the fast cells that makes the line `ratio` times as dense as `w` plays.
fn dense_weight(w: &[f32; 7], ratio: f32) -> f32 {
    if (ratio - 1.0).abs() < 1e-4 {
        return 1.0;
    }
    let want = cell_rate(w, 1.0) * ratio;
    let (mut lo, mut hi) = (0.05f32, 6.0f32);
    for _ in 0..24 {
        let mid = 0.5 * (lo + hi);
        if cell_rate(w, mid) < want { lo = mid } else { hi = mid }
    }
    0.5 * (lo + hi)
}

/// What a soloist hears from the band in a bar: its hits (bit `k`: the `k`-th 16th), a crash
/// on the downbeat (a drum fill just set them up), where the drummer's fill starts (beats from
/// the bar line), and the comp's figure in the bar before.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Heard {
    pub hits: u16,
    pub crash: bool,
    pub fill_from: Option<f64>,
    pub comp: Figure,
}

/// A figure heard: its onsets (beats from its bar line, on the 8th grid) and each one's top
/// note.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Figure {
    pub at: [f32; 8],
    pub top: [u8; 8],
    pub n: usize,
}

impl Figure {
    /// Add an onset at `at` (to the nearest 8th; one note an 8th), topped by `top`.
    pub fn push(&mut self, at: f64, top: u8) {
        let at = ((at * 2.0).round() / 2.0) as f32;
        if self.n > 0 && self.at[self.n - 1] >= at {
            self.top[self.n - 1] = self.top[self.n - 1].max(top);
            return;
        }
        if self.n < 8 {
            self.at[self.n] = at;
            self.top[self.n] = top;
            self.n += 1;
        }
    }
}

/// What the line needs from the bar: its length, and the harmony at a beat (past the bar
/// line too: an approach leads into the next bar's first chord).
pub trait Changes {
    fn bar_beats(&self) -> f64;
    fn harm(&self, beat: f64) -> Option<Harm>;
    /// The harmony `beat` beats from this bar's line, bars ahead too, as written (none: the
    /// line can't see ahead, and plans nothing past the bar: no skeleton, no developed ideas).
    fn ahead(&self, _beat: f64) -> Option<Harm> {
        None
    }
    /// Where the bar sits in the soloist's chorus: bars in, and the chorus's length in bars
    /// (none: no arc over the chorus).
    fn form(&self) -> Option<(f64, f64)> {
        None
    }
    /// The tempo (0: unknown, and the line the same at any tempo).
    fn bpm(&self) -> f32 {
        0.0
    }
    /// What the soloist hears from the band in this bar.
    fn heard(&self) -> Heard {
        Heard::default()
    }
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
        self.look = ch.ahead(0.0).is_some();
        self.heat = ch.form().map(|(at, len)| arc(at / len.max(1.0)));
        // The fast cells, thinned or thickened to the tempo, and by the arc.
        let heat = self.heat.map_or(1.0, |h| 1.0 + 0.8 * (h - HEAT_MEAN));
        self.dense = dense_weight(&p.cells, p.tempo_density(ch.bpm())) * heat;
        if !self.started {
            // A soloist comes in after a breath, not on the downbeat.
            self.started = true;
            self.dir = 1;
            self.rest = 0.5 + r.f() * 1.5;
            self.phrase = 0.0;
        }
        if land && self.phrase > bb && r.chance(0.6) {
            if p.aims() {
                // On a target beat of this bar (beat 1: the next bar's ONE).
                let (t, push, short) = p.aim(r, 0.0, bb, bb - 1.0, 1.0);
                if t + 0.01 < self.phrase {
                    self.land_at(t + 0.01);
                    (self.push, self.short) = (push, short);
                }
            } else {
                self.land_at((bb - 0.5).max(1.0));
            }
        }
        self.listen(p, ch, r, bb);
        let mut b = 0.0;
        while b < bb - 1e-9 && n < out.len() {
            if self.phrase <= 1e-9 {
                if self.rest <= 1e-9 {
                    self.start_phrase(p, ch, r, b, bb);
                    continue;
                }
                let d = self.rest.min(bb - b);
                b += d;
                self.rest -= d;
                // A breath that would end on the bar line comes in on the "and" before it.
                if !p.starts() && self.rest > 0.4 && (b + self.rest - bb).abs() < 1e-6 && r.chance(p.anticipation as f64 * 2.0) {
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
                let mut cell = self.pick_cell(p, r);
                // (No cell across an aimed ending: its last note starts on the target.)
                if b + cell.beats() > bb + 1e-9 || p.aims() && cell.beats() > self.phrase - 0.01 + 1e-6 && self.phrase > 0.5 {
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
                    // (Onto the skeleton's note, if the phrase planned one there.)
                    let at = self.total - self.phrase + (cell_end - b);
                    let target = self.goal_at(at).unwrap_or_else(|| h.nearest_chord_tone((cur + 2 * self.dir).clamp(0, 127) as u8));
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
                    // The last note rings into the rest (an anticipation over the bar line too).
                    let hold = if last && !self.short { (d + self.rest_hint(p).min(1.0)).min(bb - b + if self.push { 1.0 } else { 0.0 }) } else { d };
                    let db = self.loudness(p, r, b, d, note, last);
                    out[n] = Played { at: b, len: (hold * p.legato as f64).max(0.05), note, last, db };
                    n += 1;
                }
                b += d;
                self.phrase -= d;
                if self.phrase <= 1e-6 {
                    self.end_phrase(p, r, b, bb);
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
                    self.replay = Some(Replay { src, i: 0, until: k, by: pick_move(p, r), exact: false });
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
        // Caught with the band's hit.
        if last && self.hit {
            db += 3.0;
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

    fn start_phrase(&mut self, p: &Profile, ch: &impl Changes, r: &mut Rng, b: f64, bb: f64) {
        let h = ch.harm(b);
        // Longer lines as the solo builds.
        let heat = self.heat.map_or(1.0, |x| 1.0 + 0.6 * (x - HEAT_MEAN) as f64);
        let len = (lognormal(r, p.phrase) * heat).clamp(1.0, 24.0);
        self.phrase = len;
        (self.push, self.short) = (false, false);
        self.begin(p, r, h);
        if p.aims() {
            // Its end moved to the nearest target beat (no shorter than a restatement needs).
            let min = if self.phrase > len { self.phrase } else { 1.0 };
            let (t, push, short) = p.aim(r, b, bb, self.phrase, min);
            (self.phrase, self.push, self.short) = (t + 0.01, push, short);
            self.total = self.phrase;
        }
        self.plan(p, ch, r, b, bb);
    }

    /// The phrase's skeleton: a note on each chord change and bar line it crosses on a strong
    /// beat, at least two beats apart, voice-led from the last (a guide tone mostly: the 3rd or
    /// the 7th) and drifting along the phrase's shape (an arch, a climb or a descent).
    fn plan(&mut self, p: &Profile, ch: &impl Changes, r: &mut Rng, b: f64, bb: f64) {
        (self.ngoals, self.gi) = (0, 0);
        if self.replay.is_some() || !self.look {
            return;
        }
        let (centre, floor, ceil) = self.register(p);
        let mut prev = self.cur.unwrap_or(centre);
        let shape = if prev > centre + 5 { 2 } else if prev < centre - 5 { 1 } else { r.below(3) };
        let total = self.phrase;
        let mut t = (b.ceil() - b).max(1.0 - (b - b.floor()));
        let (mut last_goal, mut last_chord) = (f64::NEG_INFINITY, None);
        while t < total - 1.0 && self.ngoals < self.goals.len() {
            let a = b + t;
            let k = a.rem_euclid(bb);
            let downbeat = k.abs() < 1e-6 || (k - bb).abs() < 1e-6;
            let strong = downbeat || bb == 4.0 && (k - 2.0).abs() < 1e-6;
            if strong && let Some(h) = ch.ahead(a) {
                if t - last_goal >= 2.0 - 1e-9 && (downbeat || last_chord != Some(h.chord)) {
                    let degree = [1, 3, 2, 4][pick(&[0.45, 0.35, 0.1, 0.1], r)];
                    let lean = match shape {
                        0 if t < total * 0.5 => 3,
                        0 => -3,
                        1 => 2,
                        _ => -2,
                    };
                    let g = tone_near(&h, degree, (prev + lean).clamp(floor, ceil).clamp(0, 127) as u8);
                    self.goals[self.ngoals] = (t, g);
                    self.ngoals += 1;
                    (prev, last_goal) = (g as i32, t);
                }
                last_chord = Some(h.chord);
            }
            t += 1.0;
        }
    }

    /// The skeleton's note at `t` beats into the phrase, if one's planned there.
    fn goal_at(&self, t: f64) -> Option<u8> {
        self.goals[self.gi..self.ngoals].iter().find(|g| (g.0 - t).abs() < 0.05).map(|g| g.1)
    }

    /// Where the line lives now: the register, its middle higher as the solo heats up and the
    /// top kept back for the peak.
    fn register(&self, p: &Profile) -> (i32, i32, i32) {
        let (c, f, top) = p.register;
        match self.heat {
            None => (c, f, top),
            Some(h) => {
                let up = ((h - HEAT_MEAN) * 8.0).round() as i32;
                let held = ((1.0 - h) * 4.0).round() as i32;
                (c + up, f, (top - held).max(c + up + 3))
            }
        }
    }

    /// A rhythm cell, the fast ones weighted for the tempo and the arc (the long ones the
    /// other way with the arc).
    fn pick_cell(&self, p: &Profile, r: &mut Rng) -> Cell {
        let slow = self.heat.map_or(1.0, |h| 1.0 - 0.8 * (h - HEAT_MEAN));
        let mut w = p.cells;
        for (i, c) in CELLS.iter().enumerate() {
            if dense_cell(*c) {
                w[i] *= self.dense;
            } else if matches!(c, Cell::Quarter | Cell::Long | Cell::QuarterTriplet) {
                w[i] *= slow;
            }
        }
        CELLS[pick(&w, r)]
    }

    /// What the band plays this bar: land the phrase on its hit or come in on it, come in on a
    /// drummer's crash, leave the drummer room for a fill.
    fn listen(&mut self, p: &Profile, ch: &impl Changes, r: &mut Rng, bb: f64) {
        let heard = ch.heard();
        self.figure = heard.comp;
        if !self.look || !self.started {
            return;
        }
        let first_hit = (0..16).find(|k| heard.hits >> k & 1 == 1).map(|k| k as f64 * 0.25).filter(|&h| h < bb);
        if let Some(hb) = first_hit {
            if self.phrase > hb + 0.5 && hb >= 0.5 && r.chance(0.5) {
                self.cut_to(hb + 0.01);
                (self.push, self.short, self.hit) = (false, false, true);
                return;
            }
            if self.phrase <= 1e-9 && self.rest > hb + 1e-6 && r.chance(0.35) {
                self.rest = hb;
                return;
            }
        }
        if heard.crash && self.phrase <= 1e-9 && self.rest > 1e-6 && self.rest < bb + 1.0 && r.chance(0.5) {
            // In on the ONE, with the drummer.
            self.rest = 0.0;
            return;
        }
        if let Some(ff) = heard.fill_from
            && self.phrase > ff + 0.5
            && ff >= 1.0
            && r.chance(0.6)
        {
            self.cut_to(ff + 0.01);
            (self.push, self.short) = (false, false);
        }
        let _ = p;
    }

    /// Land the phrase with the tune's: `left` beats more (the skeleton kept in step when the
    /// line plans ahead).
    fn land_at(&mut self, left: f64) {
        if self.look { self.cut_to(left) } else { self.phrase = left }
    }

    /// The phrase cut short: `left` beats more (the skeleton past its new end dropped).
    fn cut_to(&mut self, left: f64) {
        let elapsed = self.total - self.phrase;
        self.phrase = left;
        self.total = elapsed + left;
        while self.ngoals > self.gi && self.goals[self.ngoals - 1].0 > self.total - 1.0 {
            self.ngoals -= 1;
        }
    }

    fn begin(&mut self, p: &Profile, r: &mut Rng, h: Option<Harm>) {
        let root = h.map(|h| h.chord.root);
        self.total = self.phrase;
        self.first = true;
        self.queued = 0;
        self.skip = 0;
        self.penta = 0;
        self.passing = false;
        self.now = Motif { root, ..Motif::default() };
        self.replay = None;
        let prev = self.last_phrase;
        // Seeing ahead, an idea is developed: stated, restated, then a fragment of it spun out,
        // then something new (the rule of three).
        let again = if !self.look {
            p.motif
        } else {
            match self.idea {
                0 => p.motif,
                1 => (p.motif * 2.0).min(0.6),
                _ => 0.0,
            }
        };
        if prev.n >= 5 && r.chance(again as f64) {
            // The last idea again: its head, then a new ending (most break off at note 5 or 6;
            // a fifth carry it whole, and stop or go on).
            let x = r.f();
            let fragment = self.look && self.idea == 1;
            let until = if fragment {
                3 + r.below(2)
            } else if x < 0.41 {
                4
            } else if x < 0.61 {
                5
            } else if x < 0.8 {
                6 + r.below(3)
            } else {
                prev.n
            };
            let until = until.min(prev.n);
            let head: f64 = prev.dur[..until].iter().map(|&d| d as f64).sum();
            self.phrase = self.phrase.max(head + if fragment { 2.0 } else if until < prev.n { 1.0 } else { 0.0 });
            self.total = self.phrase;
            // Moved with the changes (the same shape on the new chord), or by a measured move.
            let by = match (prev.root, root) {
                (Some(a), Some(b)) if self.look && r.chance(0.6) => {
                    let d = (b as i32 - a as i32).rem_euclid(12);
                    if d > 6 { d - 12 } else { d }
                }
                _ => pick_move(p, r),
            };
            self.replay = Some(Replay { src: prev, i: 0, until, by, exact: false });
            self.idea += 1;
            return;
        }
        self.idea = 0;
        if self.look
            && self.figure.n >= 2
            && let Some(h) = h
            && r.chance(ECHO)
        {
            // The comp's last figure picked up: its rhythm and its top line's shape, from a chord
            // tone near the line.
            let f = self.figure;
            let cur = self.cur.unwrap_or(p.register.0);
            let mut m = Motif { first: h.nearest_chord_tone(cur.clamp(0, 127) as u8), n: f.n.min(8), root, ..Motif::default() };
            for i in 0..m.n {
                let d = if i + 1 < m.n { f.at[i + 1] - f.at[i] } else { 0.5 };
                m.dur[i] = d.clamp(0.5, 2.0);
                m.iv[i] = if i == 0 { 0 } else { (f.top[i] as i32 - f.top[i - 1] as i32).clamp(-4, 4) as i8 };
            }
            let head: f64 = m.dur[..m.n].iter().map(|&d| d as f64).sum();
            self.phrase = self.phrase.max(head + 1.0);
            self.total = self.phrase;
            self.replay = Some(Replay { src: m, i: 0, until: m.n, by: 0, exact: false });
            self.idea = 1;
            return;
        }
        if let Some(h) = h.filter(|_| !p.licks.is_empty() && r.chance(p.licks.rate as f64))
            && let Some(mut lick) = p.licks.pick(h.chord.family(), r)
        {
            // From the chord's root, in the octave nearest the line.
            let cur = self.cur.unwrap_or(p.register.0);
            let pc = (h.chord.root as i32 + lick.first as i32).rem_euclid(12);
            let mut first = cur - (cur - pc).rem_euclid(12);
            if cur - first > 6 {
                first += 12;
            }
            lick.first = first.clamp(0, 127) as u8;
            let len: f64 = lick.dur[..lick.n].iter().map(|&d| d as f64).sum();
            self.phrase = self.phrase.max(len + 0.5);
            self.total = self.phrase;
            self.replay = Some(Replay { src: lick, i: 0, until: lick.n, by: 0, exact: true });
            return;
        }
        // A new phrase starts from somewhere new: up or down, or back towards the middle.
        let cur = self.cur.unwrap_or(p.register.0);
        let centre = self.register(p).0;
        self.dir = if cur > centre + 4 { -1 } else if cur < centre - 4 { 1 } else if r.chance(0.5) { 1 } else { -1 };
        if r.chance(0.5) {
            self.cur = Some(cur + self.dir * (2 + r.below(4) as i32));
        }
    }

    fn end_phrase(&mut self, p: &Profile, r: &mut Rng, b: f64, bb: f64) {
        self.phrase = 0.0;
        self.replay = None;
        if self.now.n >= 4 {
            self.last_phrase = self.now;
        }
        self.queued = 0;
        self.skip = 0;
        self.penta = 0;
        (self.ngoals, self.gi, self.hit) = (0, 0, false);
        // Whole 8ths of rest, so the next phrase starts on the 8th grid (shorter breaths as the
        // solo builds).
        let heat = self.heat.map_or(1.0, |h| 1.0 - 0.6 * (h - HEAT_MEAN) as f64);
        self.rest = (lognormal(r, p.rest).clamp(0.5, 10.0) * heat * 2.0).round().max(1.0) / 2.0;
        if p.starts() {
            // The rest nearest its length that ends on a start slot.
            let slot = p.start_slot(r, bb);
            let first = (slot - b).rem_euclid(bb);
            let k = ((self.rest - first) / bb).round().max(0.0);
            let mut rest = first + k * bb;
            if rest < 0.5 - 1e-9 {
                rest += bb;
            }
            self.rest = rest;
        }
        self.breath = self.rest;
    }

    /// The next note (none when there's no harmony to play on).
    fn next(&mut self, p: &Profile, ch: &impl Changes, r: &mut Rng, b: f64, last: bool) -> Option<u8> {
        let h = ch.harm(b)?;
        let push = self.push;
        let (centre, floor, ceil) = self.register(p);
        // The skeleton: past goals dropped, the next one ahead.
        let elapsed = self.total - self.phrase;
        while self.gi < self.ngoals && self.goals[self.gi].0 < elapsed - 0.05 {
            self.gi += 1;
        }
        let goal = (self.gi < self.ngoals).then(|| self.goals[self.gi]);
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
            let n = if rp.exact {
                want.clamp(0, 127) as u8
            } else {
                let n = want.clamp(floor - 6, ceil + 6).clamp(0, 127) as u8;
                let n = if h.scale.contains(n % 12) || !strong && r.chance(0.5) { n } else { h.scale.snap(n) };
                if last { end_of(push, p, ch, r, b, h, n) } else { n }
            };
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
            if self.now.n == 1 && p.starts() {
                // (The second note keeps the way the first one set.)
            } else if self.leapt && r.chance(p.step_back as f64) {
                self.dir = -self.dir;
            } else if size > 0 && r.chance(0.2) {
                self.dir = -self.dir;
            }
            // Heading for the skeleton's next note: always when it's too far to reach by step.
            if let Some((t, g)) = goal {
                let (dist, left) = (g as i32 - cur, t - elapsed);
                if dist != 0 && left > 0.05 && (dist.abs() as f64 > left * 4.0 || r.chance(0.35)) {
                    self.dir = dist.signum();
                }
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
        if last {
            note = end_of(self.push, p, ch, r, b, h, note.clamp(0, 127) as u8) as i32;
        } else if self.first && self.now.n == 0 && p.starts() {
            // The phrase's first note: a measured degree, or a half step into one.
            let (n, into) = p.start_note(&h, note.clamp(0, 127) as u8, r);
            // (A chromatic first note steps into its chord tone about a third of the time, as
            // theirs do; the rest leave it.)
            if let Some(t) = into.filter(|_| r.chance(0.3)) {
                self.load(&[t], 0);
            }
            note = n as i32;
            self.start_dir = Some(if r.chance(p.start_up as f64) { 1 } else { -1 });
        } else if let Some((_, g)) = goal.filter(|&(t, _)| strong && (t - elapsed).abs() < 0.05 && r.chance(0.75)) {
            // On the skeleton's note.
            note = g as i32;
        } else if strong && r.chance(p.strong_chord_tone as f64) {
            note = h.nearest_chord_tone(note.clamp(0, 127) as u8) as i32;
        }
        note = note.clamp(floor - 6, ceil + 6);
        self.leapt = (note - cur).abs() >= 5;
        if let Some(d) = self.start_dir.take() {
            // After the first note, the line heads off the way the player's do.
            self.dir = d;
        } else if note != cur {
            self.dir = (note - cur).signum();
        }
        self.cur = Some(note);
        Some(note as u8)
    }
}

/// A phrase's last note: aimed (against the next beat's chord when anticipated), or a chord
/// tone `end_chord_tone` of the time.
fn end_of(push: bool, p: &Profile, ch: &impl Changes, r: &mut Rng, b: f64, h: Harm, note: u8) -> u8 {
    if p.aims() {
        let h = if push { ch.harm(b + 0.5).unwrap_or(h) } else { h };
        p.end_note(&h, note, r)
    } else if r.chance(p.end_chord_tone as f64) {
        h.nearest_chord_tone(note)
    } else {
        note
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

    #[test]
    fn the_solo_goes_round_a_chorus_each() {
        for (bpm, waltz) in [(100.0, false), (184.0, false), (300.0, false), (184.0, true)] {
            let mut prev = None;
            let mut seen = Vec::new();
            for pass in 0..40 {
                let s = take_the_solo(9, pass, prev, bpm, waltz, (pass % 5) as f32 / 4.0);
                assert_eq!(s, take_the_solo(9, pass, prev, bpm, waltz, (pass % 5) as f32 / 4.0), "seeded");
                assert!(s.fits(bpm, waltz), "{s:?} at {bpm} bpm, waltz {waltz}");
                assert_ne!(Some(s), prev, "never the same twice running");
                if pass == 0 {
                    assert_eq!(s == Soloist::Pops, Soloist::Pops.fits(bpm, waltz), "Pops opens when he fits");
                }
                if !seen.contains(&s) {
                    seen.push(s);
                }
                prev = Some(s);
            }
            assert!(seen.len() >= 4, "{bpm} bpm: the solo goes round: {seen:?}");
        }
        assert!(!Soloist::Pops.fits(184.0, true) && !Soloist::Bird.fits(300.0, true), "no two-beat or bebop in three");
        assert!(!Soloist::Bird.fits(100.0, false) && Soloist::Miles.fits(100.0, false));
    }
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

    fn prof(s: Soloist) -> &'static Profile {
        s.profile().unwrap()
    }

    #[test]
    fn every_player_file_parses() {
        for s in Soloist::ALL {
            assert_eq!(s.profile().is_some(), s != Soloist::Band, "{s:?}");
        }
        assert_eq!(Soloist::Miles.player().unwrap().name, "Miles '56");
        for s in &Soloist::ALL[1..] {
            assert!(s.profile().unwrap().licks.len() >= 6, "{s:?}");
        }
    }

    #[test]
    fn licks_play_as_written_from_the_root() {
        // One lick (b9 and all), always: it shows up whole, from each chord's root.
        let mut p = *prof(Soloist::Bird);
        p.licks = Licks { rate: 1.0, ..Licks::default() };
        (p.motif, p.sequence) = (0.0, 0.0);
        p.licks.add(Fits::Any, &[(64, 0.25), (67, 0.25), (70, 0.25), (73, 0.25), (72, 0.5)]).unwrap();
        let mut line = Line::default();
        let mut r = Rng::new(3);
        let mut found = 0;
        let mut notes: Vec<(u8, u8)> = Vec::new();
        for bar in 0..200 {
            let ch = Turnaround(bar);
            let mut out = [Played::default(); 24];
            let n = line.bar(&p, &ch, &mut r, false, &mut out);
            notes.extend(out[..n].iter().map(|k| (k.note, ch.harm(k.at).unwrap().chord.root)));
        }
        for w in notes.windows(5) {
            let iv: Vec<i32> = w.windows(2).map(|x| x[1].0 as i32 - x[0].0 as i32).collect();
            if iv == [3, 3, 3, -1] {
                found += 1;
                assert_eq!((w[0].0 + 12 - w[0].1) % 12, 4, "starts on the chord's 3rd");
            }
        }
        assert!(found > 40, "{found}");
    }

    /// Each phrase's target: its beat (1-4; an "and" held into the next beat counts as that
    /// beat, a short one is off) and its degree against the chord there (0 R, 1 3, 2 5, 3 7,
    /// 4 another note), shares.
    fn endings(p: &Profile, bars: u64) -> ([f64; 5], [f64; 5]) {
        let mut line = Line::default();
        let mut r = Rng::new(5);
        let (mut beats, mut degs, mut n) = ([0.0; 5], [0.0; 5], 0.0);
        for bar in 0..bars {
            let ch = Turnaround(bar);
            let mut out = [Played::default(); 24];
            let k = line.bar(p, &ch, &mut r, bar % 8 == 7, &mut out);
            for e in out[..k].iter().filter(|e| e.last) {
                let and = (e.at - e.at.floor() - 0.5).abs() < 1e-6;
                let held = and && e.len / p.legato as f64 > 0.5 + 1e-6;
                let on = (e.at - e.at.round()).abs() < 1e-6;
                let (slot, t) = if on { (e.at.round() as usize % 4, e.at) } else if held { (e.at.ceil() as usize % 4, e.at.ceil()) } else { (4, e.at) };
                beats[slot] += 1.0;
                // The chord at the target (the next bar's, past the bar line).
                let h = Turnaround(bar + (t / 4.0).floor() as u64).harm(t % 4.0).unwrap();
                let i = (e.note as i32 - h.chord.root as i32).rem_euclid(12) as u8;
                let iv = h.chord.quality.intervals();
                degs[iv.iter().take(4).position(|x| x % 12 == i).unwrap_or(4)] += 1.0;
                n += 1.0;
            }
        }
        (beats.map(|x| x / n), degs.map(|x| x / n))
    }

    #[test]
    fn soloists_aim_their_endings_like_their_numbers() {
        let (m, t, so, po) = (endings(prof(Soloist::Miles), 1500), endings(prof(Soloist::Trane), 1500), endings(prof(Soloist::Sonny), 1500), endings(prof(Soloist::Pops), 1500));
        eprintln!("beats 1 2 3 4 off; degrees R 3 5 7 other\nmiles {m:.2?}\ntrane {t:.2?}\nsonny {so:.2?}\npops {po:.2?}");
        // Trane on the ONE, Sonny on 2, Pops on 3 (hardly ever 2), Miles on 3.
        assert!(t.0[0] > m.0[0] + 0.05 && t.0[0] > t.0[1] + 0.1, "{t:?} {m:?}");
        assert!(so.0[1] > so.0[2] && so.0[1] > t.0[1] + 0.08, "{so:?}");
        assert!(po.0[2] > 2.0 * po.0[1] && m.0[2] > m.0[1], "{po:?} {m:?}");
        // Miles on the root most, Pops on the 3rd more than the root, Trane on the 5th.
        assert!(m.1[0] > m.1[1] && po.1[1] > po.1[0] && t.1[2] > t.1[1], "{m:?} {po:?} {t:?}");
        for e in [&m, &t, &so, &po] {
            assert!((0.5..0.85).contains(&(e.1[..4].iter().sum::<f64>())), "chord tones on most endings: {e:?}");
        }
    }

    /// Each phrase's first note: on the beat, the "and" or elsewhere; the beat of an on-beat
    /// start (1-4); the line going up after it, shares.
    fn starts(p: &Profile, bars: u64) -> ([f64; 3], [f64; 4], f64) {
        let mut line = Line::default();
        let mut r = Rng::new(9);
        let (mut place, mut on, mut up, mut n, mut prev_last, mut first) = ([0.0; 3], [0.0; 4], 0.0, 0.0, true, None::<u8>);
        for bar in 0..bars {
            let ch = Turnaround(bar);
            let mut out = [Played::default(); 24];
            let k = line.bar(p, &ch, &mut r, bar % 8 == 7, &mut out);
            for e in &out[..k] {
                if let Some(f) = first.take() {
                    up += (e.note > f) as u8 as f64;
                }
                if prev_last && bar > 0 {
                    let frac = e.at - e.at.floor();
                    let slot = if frac < 1e-6 { 0 } else if (frac - 0.5).abs() < 1e-6 { 1 } else { 2 };
                    place[slot] += 1.0;
                    if slot == 0 {
                        on[e.at.round() as usize % 4] += 1.0;
                    }
                    n += 1.0;
                    first = Some(e.note);
                }
                prev_last = e.last;
            }
        }
        let on_n: f64 = on.iter().sum();
        (place.map(|x| x / n), on.map(|x| x / on_n), up / n)
    }

    #[test]
    fn soloists_start_their_phrases_like_their_numbers() {
        // (Licks and restated motifs start their own way: none here.)
        let free = |s: Soloist| {
            let mut p = *prof(s);
            (p.licks.rate, p.motif) = (0.0, 0.0);
            starts(&p, 1500)
        };
        let (m, b, po, so) = (free(Soloist::Miles), free(Soloist::Bird), free(Soloist::Pops), free(Soloist::Sonny));
        eprintln!("on and other; on-beat starts on 1 2 3 4; up\nmiles {m:.2?}\nbird {b:.2?}\npops {po:.2?}\nsonny {so:.2?}");
        // Miles comes in on 2, Pops on the ONE; Bird hardly ever on the beat.
        assert!(m.1[1] > m.1[0] + 0.1 && po.1[0] > po.1[1] + 0.15, "{m:?} {po:?}");
        assert!(b.0[0] < 0.3 && b.0[2] > b.0[0] && po.0[0] > b.0[0] + 0.15, "{b:?} {po:?}");
        // Sonny heads up from his first note the most.
        assert!(so.2 > m.2 + 0.05 && so.2 > 0.65 && m.2 > 0.55, "{so:?} {m:?}");
    }

    #[test]
    fn soloists_play_like_their_numbers() {
        let (m, t, b) = (play(prof(Soloist::Miles), 400), play(prof(Soloist::Trane), 400), play(prof(Soloist::Bird), 400));
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
        let (so, w, po) = (play(prof(Soloist::Sonny), 400), play(prof(Soloist::Wayne), 400), play(prof(Soloist::Pops), 400));
        eprintln!("sonny {so:?}\nwayne {w:?}\npops {po:?}");
        // Pops: repeated notes, pentatonic, high.
        assert!(po.repeats > t.repeats && po.pentatonic > m.pentatonic && (4.0..7.5).contains(&po.notes_per_bar), "{po:?}");
        assert!((4.0..7.0).contains(&so.notes_per_bar) && (3.5..6.5).contains(&w.notes_per_bar), "{so:?} {w:?}");
        assert!(w.steps > t.steps && w.pentatonic > m.pentatonic, "{w:?}");
        for s in [&m, &t, &b, &so, &w, &po] {
            assert!((0.45..0.8).contains(&s.strong_chord_tones), "{s:?}");
        }
    }

    /// A chart the line can see ahead in (the turnaround), with a chorus of `bars`, a tempo
    /// and what the band plays.
    struct Planned {
        bar: u64,
        bars: u64,
        bpm: f32,
        heard: Heard,
    }

    impl Changes for Planned {
        fn bar_beats(&self) -> f64 {
            4.0
        }
        fn harm(&self, beat: f64) -> Option<Harm> {
            Turnaround(self.bar).harm(beat.min(3.99))
        }
        fn ahead(&self, beat: f64) -> Option<Harm> {
            let bars = (beat / 4.0).floor() as u64;
            Turnaround(self.bar + bars).harm(beat - bars as f64 * 4.0)
        }
        fn form(&self) -> Option<(f64, f64)> {
            (self.bars > 0).then(|| ((self.bar % self.bars) as f64, self.bars as f64))
        }
        fn bpm(&self) -> f32 {
            self.bpm
        }
        fn heard(&self) -> Heard {
            self.heard
        }
    }

    /// Every note (absolute beat, note) of `bars` bars by `p`, with the band as `heard(bar)`.
    fn planned(p: &Profile, bars: u64, chorus: u64, bpm: f32, heard: impl Fn(u64) -> Heard) -> Vec<(f64, Played)> {
        let mut line = Line::default();
        let mut r = Rng::new(5);
        let mut all = Vec::new();
        for bar in 0..bars {
            let ch = Planned { bar, bars: chorus, bpm, heard: heard(bar) };
            let mut out = [Played::default(); 24];
            let n = line.bar(p, &ch, &mut r, bar % 8 == 7, &mut out);
            all.extend(out[..n].iter().map(|k| (bar as f64 * 4.0 + k.at, *k)));
            assert!(line.idea <= 2, "an idea restated twice at most, then something new");
        }
        all
    }

    #[test]
    fn the_line_thins_out_at_fast_tempos() {
        let fast16 = |s: Soloist, bpm: f32| {
            let all = planned(prof(s), 1500, 0, bpm, |_| Heard::default());
            let iv: Vec<f64> = all.windows(2).filter(|w| !w[0].1.last).map(|w| w[1].0 - w[0].0).collect();
            iv.iter().filter(|&&d| d < 0.3).count() as f64 / iv.len() as f64
        };
        let (b_slow, b_fast) = (fast16(Soloist::Bird, 120.0), fast16(Soloist::Bird, 240.0));
        let (m_slow, m_fast) = (fast16(Soloist::Miles, 120.0), fast16(Soloist::Miles, 240.0));
        eprintln!("16ths: bird {b_slow:.2} at 120, {b_fast:.2} at 240; miles {m_slow:.2}, {m_fast:.2}");
        // Bird doubles up at ballad tempos and thins out fast; Miles keeps about the same.
        assert!(b_slow > b_fast + 0.15, "{b_slow} {b_fast}");
        assert!((m_slow - m_fast).abs() < 0.08, "{m_slow} {m_fast}");
    }

    #[test]
    fn a_solo_builds_over_its_chorus() {
        let p = prof(Soloist::Sonny);
        let all = planned(p, 32 * 40, 32, 180.0, |_| Heard::default());
        let at = |from: u64, to: u64| {
            let notes: Vec<&(f64, Played)> = all.iter().filter(|(t, _)| ((*t / 4.0) as u64 % 32) >= from && ((*t / 4.0) as u64 % 32) < to).collect();
            let per_bar = notes.len() as f64 / ((to - from) * 40) as f64;
            let pitch = notes.iter().map(|(_, k)| k.note as f64).sum::<f64>() / notes.len() as f64;
            (per_bar, pitch)
        };
        let (calm, peak) = (at(0, 8), at(18, 26));
        let flat = planned(p, 32 * 40, 0, 180.0, |_| Heard::default()).len() as f64 / (32.0 * 40.0);
        let whole = all.len() as f64 / (32.0 * 40.0);
        eprintln!("calm {calm:.2?}, peak {peak:.2?}; a chorus {whole:.2} a bar, without the arc {flat:.2}");
        assert!(peak.0 > calm.0 * 1.25 && peak.1 > calm.1 + 1.5, "{calm:?} {peak:?}");
        assert!((whole / flat - 1.0).abs() < 0.15, "the arc averages out: {whole} {flat}");
    }

    #[test]
    fn the_skeleton_lands_guide_tones_on_the_changes() {
        // Downbeats where the chord changes: the share on a 3rd or a 7th.
        let guide = |seeing: bool| {
            let p = prof(Soloist::Miles);
            let (mut on, mut all) = (0, 0);
            let notes: Vec<(f64, Played)> = if seeing {
                planned(p, 2000, 0, 0.0, |_| Heard::default())
            } else {
                let mut line = Line::default();
                let mut r = Rng::new(5);
                let mut v = Vec::new();
                for bar in 0..2000u64 {
                    let mut out = [Played::default(); 24];
                    let n = line.bar(p, &Turnaround(bar), &mut r, bar % 8 == 7, &mut out);
                    v.extend(out[..n].iter().map(|k| (bar as f64 * 4.0 + k.at, *k)));
                }
                v
            };
            for (t, k) in notes.iter().filter(|(t, k)| t.fract().abs() < 1e-6 && (*t as u64) % 2 == 0 && !k.last) {
                let h = Turnaround((*t / 4.0) as u64).harm(t % 4.0).unwrap();
                let iv = h.chord.quality.intervals();
                let pc = (k.note + 12 - h.chord.root) % 12;
                all += 1;
                on += (pc == iv[1] % 12 || iv.get(3).is_some_and(|&s| pc == s % 12)) as usize;
            }
            on as f64 / all as f64
        };
        let (plain, planned) = (guide(false), guide(true));
        eprintln!("guide tones on the strong beats: {plain:.2} without the skeleton, {planned:.2} with it");
        assert!(planned > plain + 0.08, "{plain} {planned}");
    }

    #[test]
    fn the_soloist_catches_the_bands_hits_and_comes_in_off_a_fill() {
        let p = prof(Soloist::Trane);
        let ends_on = |hits: u16| {
            let all = planned(p, 1500, 0, 0.0, |_| Heard { hits, ..Heard::default() });
            let lasts: Vec<f64> = all.iter().filter(|(_, k)| k.last).map(|(t, _)| t % 4.0).collect();
            lasts.iter().filter(|&&b| (b - 1.5).abs() < 1e-6).count() as f64 / lasts.len() as f64
        };
        let (plain, hit) = (ends_on(0), ends_on(1 << 6));
        eprintln!("phrases ending on beat 2's \"and\": {plain:.2}, with a hit there {hit:.2}");
        assert!(hit > plain + 0.15, "{plain} {hit}");
        // A crash every fourth bar: phrases come in on its ONE.
        let starts_on_one = |crash: bool| {
            let all = planned(p, 1500, 0, 0.0, |bar| Heard { crash: crash && bar % 4 == 0, ..Heard::default() });
            let firsts: Vec<f64> = all.windows(2).filter(|w| w[0].1.last).map(|w| w[1].0).collect();
            firsts.iter().filter(|&&t| t % 16.0 == 0.0).count() as f64 / firsts.len() as f64
        };
        let (plain, crashed) = (starts_on_one(false), starts_on_one(true));
        eprintln!("phrases in on a crash bar's ONE: {plain:.2}, with the crash {crashed:.2}");
        assert!(crashed > plain * 2.0, "{plain} {crashed}");
    }
}
