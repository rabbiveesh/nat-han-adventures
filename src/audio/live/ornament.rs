//! The band's vocabulary: the ornaments each musician knows ([`Orn`]), and the pure harmony
//! helpers they're built from (chord-scales, voicings, enclosures, digital patterns,
//! pentatonic superimposition, planing, substitutions). Nothing here allocates: musicians
//! call it on the audio thread.
//!
//! Everything is generated *against the harmony in force* ([`Harm`]: the chord of the
//! current filter's chart, Coltrane's substitutions and the waltz included, or the band's own
//! reharmonization, [`super::band::BandPlan::sub`]), so ornaments follow every filter; the
//! tuning is applied by the voices to whatever notes come out.

use crate::audio::chart::{Chord, Family, Quality};
use crate::audio::mml::Arp;

/// Every ornament, for logs, UIs and tests: each musician reports the ones it played in a bar
/// (a bit set, [`Orns`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Orn {
    // Lead.
    Grace,
    Vibrato,
    FallOff,
    Echo,
    Turn,
    Mordent,
    Enclosure,
    Pickup,
    Octave,
    SideSlip,
    Planing,
    Digital,
    Pentatonic,
    Displace,
    Hemiola,
    RunFill,
    Solo,
    Slide,
    DutySweep,
    ArpFlourish,
    TripletRun,
    WahWah,
    LayOut,
    Switch,
    // Comp.
    Anticipation,
    ExtraStab,
    Charleston,
    FreddieGreen,
    MovingVoicing,
    Hit,
    Reharm,
    SlipVoicing,
    PlaneChords,
    Fourths,
    Polychord,
    // Bass.
    Approach,
    Walking,
    Pedal,
    TwoFeel,
    TritoneRoot,
    BassFill,
    Ostinato,
    SlipWalk,
    // Drums.
    Ghost,
    OpenHat,
    Fill,
    Crash,
    ShortFill,
    Trade,
    PressRoll,
    BrokenTime,
    // Feels ([`super::feel`]): what each player's groove was.
    Bend,
    HornStab,
    BossaComp,
    PartidoAlto,
    PowerChords,
    Clav,
    BossaBass,
    Surdo,
    Pumping,
    Slap,
    Clave,
    Batucada,
    Backbeat,
    FunkGroove,
    Cuica,
}

impl Orn {
    pub const ALL: [Orn; 66] = [
        Orn::Grace,
        Orn::Vibrato,
        Orn::FallOff,
        Orn::Echo,
        Orn::Turn,
        Orn::Mordent,
        Orn::Enclosure,
        Orn::Pickup,
        Orn::Octave,
        Orn::SideSlip,
        Orn::Planing,
        Orn::Digital,
        Orn::Pentatonic,
        Orn::Displace,
        Orn::Hemiola,
        Orn::RunFill,
        Orn::Solo,
        Orn::Slide,
        Orn::DutySweep,
        Orn::ArpFlourish,
        Orn::TripletRun,
        Orn::WahWah,
        Orn::LayOut,
        Orn::Switch,
        Orn::Anticipation,
        Orn::ExtraStab,
        Orn::Charleston,
        Orn::FreddieGreen,
        Orn::MovingVoicing,
        Orn::Hit,
        Orn::Reharm,
        Orn::SlipVoicing,
        Orn::PlaneChords,
        Orn::Fourths,
        Orn::Polychord,
        Orn::Approach,
        Orn::Walking,
        Orn::Pedal,
        Orn::TwoFeel,
        Orn::TritoneRoot,
        Orn::BassFill,
        Orn::Ostinato,
        Orn::SlipWalk,
        Orn::Ghost,
        Orn::OpenHat,
        Orn::Fill,
        Orn::Crash,
        Orn::ShortFill,
        Orn::Trade,
        Orn::PressRoll,
        Orn::BrokenTime,
        Orn::Bend,
        Orn::HornStab,
        Orn::BossaComp,
        Orn::PartidoAlto,
        Orn::PowerChords,
        Orn::Clav,
        Orn::BossaBass,
        Orn::Surdo,
        Orn::Pumping,
        Orn::Slap,
        Orn::Clave,
        Orn::Batucada,
        Orn::Backbeat,
        Orn::FunkGroove,
        Orn::Cuica,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Orn::Grace => "grace",
            Orn::Vibrato => "vibrato",
            Orn::FallOff => "fall-off",
            Orn::Echo => "echo",
            Orn::Turn => "turn",
            Orn::Mordent => "mordent",
            Orn::Enclosure => "enclosure",
            Orn::Pickup => "pickup run",
            Orn::Octave => "octave",
            Orn::SideSlip => "side-slip",
            Orn::Planing => "planing",
            Orn::Digital => "digital 1-2-3-5",
            Orn::Pentatonic => "pentatonic +1/2",
            Orn::Displace => "displaced",
            Orn::Hemiola => "hemiola",
            Orn::RunFill => "run fill",
            Orn::Solo => "solo",
            Orn::Slide => "slide",
            Orn::DutySweep => "duty sweep",
            Orn::ArpFlourish => "arp flourish",
            Orn::TripletRun => "triplet arps",
            Orn::WahWah => "wah-wah",
            Orn::LayOut => "lays out",
            Orn::Switch => "instrument",
            Orn::Anticipation => "anticipation",
            Orn::ExtraStab => "extra stab",
            Orn::Charleston => "charleston",
            Orn::FreddieGreen => "freddie green",
            Orn::MovingVoicing => "moving voicing",
            Orn::Hit => "hit",
            Orn::Reharm => "reharm",
            Orn::SlipVoicing => "side-slip voicing",
            Orn::PlaneChords => "planing chords",
            Orn::Fourths => "rising fourths",
            Orn::Polychord => "polychord",
            Orn::Approach => "approach",
            Orn::Walking => "walking",
            Orn::Pedal => "pedal",
            Orn::TwoFeel => "two-feel",
            Orn::TritoneRoot => "tritone-sub root",
            Orn::BassFill => "bass fill",
            Orn::Ostinato => "ostinato",
            Orn::SlipWalk => "side-slip walk",
            Orn::Ghost => "ghost snare",
            Orn::OpenHat => "open hat",
            Orn::Fill => "fill",
            Orn::Crash => "crash",
            Orn::ShortFill => "short fill",
            Orn::Trade => "trading fours",
            Orn::PressRoll => "press roll",
            Orn::BrokenTime => "broken time",
            Orn::Bend => "blues bend",
            Orn::HornStab => "horn stabs",
            Orn::BossaComp => "bossa batida",
            Orn::PartidoAlto => "partido-alto",
            Orn::PowerChords => "power chords",
            Orn::Clav => "clav stabs",
            Orn::BossaBass => "bossa root-fifth",
            Orn::Surdo => "surdo",
            Orn::Pumping => "pumping 8ths",
            Orn::Slap => "slap and pop",
            Orn::Clave => "bossa clave",
            Orn::Batucada => "batucada",
            Orn::Backbeat => "rock backbeat",
            Orn::FunkGroove => "funk groove",
            Orn::Cuica => "cuica",
        }
    }

    pub fn bit(self) -> u128 {
        1 << self as u8
    }
}

/// A set of [`Orn`]s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Orns(pub u128);

impl Orns {
    pub fn add(&mut self, o: Orn) {
        self.0 |= o.bit();
    }

    pub fn has(self, o: Orn) -> bool {
        self.0 & o.bit() != 0
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn iter(self) -> impl Iterator<Item = Orn> {
        Orn::ALL.into_iter().filter(move |o| self.has(*o))
    }

    /// `"grace, echo"`.
    pub fn names(self) -> String {
        self.iter().map(Orn::name).collect::<Vec<_>>().join(", ")
    }
}

/// A chord-scale as pitch classes above its root (7 or 8 notes, ascending). Copy, no heap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scale {
    pub root: u8,
    semis: [u8; 8],
    len: u8,
}

impl Scale {
    pub fn new(root: u8, semis: &[u8]) -> Scale {
        let mut s = [0; 8];
        let len = semis.len().min(8);
        s[..len].copy_from_slice(&semis[..len]);
        Scale { root: root % 12, semis: s, len: len as u8 }
    }

    /// The chord-scale of a chord (as [`crate::audio::melody::chord_scale`] reads them): major
    /// → ionian (lydian with `lydian`, the quartal colour), dominant → mixolydian, altered
    /// dominants → half-whole, minor → dorian, ...
    pub fn of(chord: &Chord, lydian: bool) -> Scale {
        let r = chord.root;
        match chord.quality {
            Quality::Major | Quality::Six | Quality::Maj7 if lydian => Scale::new(r, &[0, 2, 4, 6, 7, 9, 11]),
            Quality::Major | Quality::Six | Quality::Maj7 => Scale::new(r, &[0, 2, 4, 5, 7, 9, 11]),
            Quality::Dom7 | Quality::Dom9 | Quality::Sus4 => Scale::new(r, &[0, 2, 4, 5, 7, 9, 10]),
            Quality::Dom7b9 | Quality::Dom7s9 => Scale::new(r, &[0, 1, 3, 4, 6, 7, 9, 10]),
            Quality::Dom7s5 => Scale::new(r, &[0, 1, 3, 4, 6, 8, 10]),
            Quality::Minor | Quality::Minor7 | Quality::Minor6 => Scale::new(r, &[0, 2, 3, 5, 7, 9, 10]),
            Quality::MinMaj7 => Scale::new(r, &[0, 2, 3, 5, 7, 9, 11]),
            Quality::HalfDim => Scale::new(r, &[0, 1, 3, 5, 6, 8, 10]),
            Quality::Dim7 => Scale::new(r, &[0, 2, 3, 5, 6, 8, 9, 11]),
            Quality::Aug => Scale::new(r, &[0, 2, 4, 6, 8, 9, 11]),
        }
    }

    /// A parent scale's pitch classes, read from `root`.
    pub fn from_pcs(root: u8, pcs: &[u8]) -> Scale {
        let mut semis = [0u8; 8];
        let n = pcs.len().min(8);
        for (s, p) in semis.iter_mut().zip(pcs) {
            *s = ((*p as i32 - root as i32).rem_euclid(12)) as u8;
        }
        semis[..n].sort_unstable();
        Scale::new(root, &semis[..n])
    }

    pub fn semis(&self) -> &[u8] {
        &self.semis[..self.len as usize]
    }

    pub fn contains(&self, pc: u8) -> bool {
        let s = (pc as i32 - self.root as i32).rem_euclid(12) as u8;
        self.semis().contains(&s)
    }

    /// The scale note nearest `note` (below on a tie).
    pub fn snap(&self, note: u8) -> u8 {
        (0..=6)
            .flat_map(|d| [note as i32 - d, note as i32 + d])
            .find(|&n| (0..=127).contains(&n) && self.contains(n as u8))
            .unwrap_or(note as i32) as u8
    }

    /// `steps` scale steps from `note` (snapped first): +1 is the next scale note up.
    pub fn step(&self, note: u8, steps: i32) -> u8 {
        let mut n = self.snap(note) as i32;
        let dir = steps.signum();
        for _ in 0..steps.abs() {
            n += dir;
            while (0..=127).contains(&n) && !self.contains(n as u8) {
                n += dir;
            }
        }
        n.clamp(0, 127) as u8
    }
}

/// The harmony at a moment: the chord, and the scale to draw from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Harm {
    pub chord: Chord,
    pub scale: Scale,
}

impl Harm {
    pub fn new(chord: Chord) -> Harm {
        Harm { chord, scale: Scale::of(&chord, false) }
    }

    /// Is `note` a chord tone (root, 3rd, 5th, 7th; 6ths count for 6 chords)?
    pub fn is_chord_tone(&self, note: u8) -> bool {
        self.chord.quality.intervals().iter().take(4).any(|i| (self.chord.root + i) % 12 == note % 12)
    }

    /// The chord tone nearest `note` (below on a tie).
    pub fn nearest_chord_tone(&self, note: u8) -> u8 {
        (0..=6)
            .flat_map(|d| [note as i32 - d, note as i32 + d])
            .find(|&n| (0..=127).contains(&n) && self.is_chord_tone(n as u8))
            .unwrap_or(note as i32) as u8
    }

    /// The nearest chord tone at or above `note`.
    pub fn chord_tone_above(&self, note: u8) -> u8 {
        (0..12).map(|d| note.saturating_add(d)).find(|&n| self.is_chord_tone(n)).unwrap_or(note)
    }

    /// The `deg`-th scale note above the root (0 = root, 1 = 2nd, 2 = 3rd, 4 = 5th ...), the
    /// root placed at or above `floor`.
    pub fn degree(&self, deg: usize, floor: u8) -> u8 {
        let s = self.scale.semis();
        let semi = s[deg % s.len()] as i32 + 12 * (deg / s.len()) as i32;
        let root = floor as i32 + (self.chord.root as i32 - floor as i32).rem_euclid(12);
        (root + semi).clamp(0, 127) as u8
    }
}

/// The note with pitch class `pc` nearest `near`, inside `lo..=hi`.
pub fn nearest_pc(pc: u8, near: i32, lo: i32, hi: i32) -> u8 {
    let mut best = lo + (pc as i32 - lo).rem_euclid(12);
    let mut n = best;
    while n <= hi {
        if (n - near).abs() < (best - near).abs() {
            best = n;
        }
        n += 12;
    }
    best.clamp(0, 127) as u8
}

/// A turn on `note`: upper neighbour, the note, lower neighbour, the note (scale steps).
pub fn turn(note: u8, scale: &Scale) -> [u8; 4] {
    [scale.step(note, 1), note, scale.step(note, -1), note]
}

/// A mordent: the note, its lower (or upper) neighbour, the note.
pub fn mordent(note: u8, scale: &Scale, upper: bool) -> [u8; 3] {
    [note, scale.step(note, if upper { 1 } else { -1 }), note]
}

/// A bebop enclosure of `target`: the scale note above, then the chromatic note below.
pub fn enclosure(target: u8, scale: &Scale) -> [u8; 2] {
    [scale.step(target, 1).max(target + 1), target.saturating_sub(1)]
}

/// Digital pattern 1-2-3-5 of a chord's scale, from the root at or above `floor` (descending:
/// 5-3-2-1).
pub fn digital(h: &Harm, floor: u8, down: bool) -> [u8; 4] {
    let root = h.degree(0, floor);
    let third = h.chord.root + if matches!(h.chord.family(), Family::Minor | Family::HalfDim | Family::Dim) { 3 } else { 4 };
    let fifth = h.chord.root + if matches!(h.chord.family(), Family::HalfDim | Family::Dim) { 6 } else { 7 };
    let up = |pc: u8| root + ((pc as i32 - root as i32).rem_euclid(12)) as u8;
    let p = [root, h.scale.step(root, 1), up(third % 12), up(fifth % 12)];
    if down { [p[3], p[2], p[1], p[0]] } else { p }
}

/// Pentatonic superimposition over a dominant: the major pentatonic a half step above its root
/// (over G7: Ab Bb C Eb F: b9 #9 11 b13 b7, the altered sound), as pitch classes.
pub fn pentatonic_up_half(root: u8) -> [u8; 5] {
    [0, 2, 4, 7, 9].map(|i| (root + 1 + i) % 12)
}

/// The note of `pcs` nearest `note` moving `dir` (+1 up, -1 down), strictly past it.
pub fn next_in(pcs: &[u8], note: u8, dir: i32) -> u8 {
    let mut n = note as i32;
    for _ in 0..12 {
        n += dir;
        if pcs.contains(&((n.rem_euclid(12)) as u8)) {
            break;
        }
    }
    n.clamp(0, 127) as u8
}

/// How a planed melody note is harmonized (always below the melody, the melody on top).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plane {
    /// Parallel perfect fourths: note, note-5, note-10.
    Fourths,
    /// The diatonic triad with the melody on top (scale steps -2, -4).
    Triad,
    /// A cluster: note, a scale step below, two steps below.
    Cluster,
}

/// The planed voicing of `note` (lowest first, as an arpeggio plays).
pub fn plane(note: u8, kind: Plane, scale: &Scale) -> Arp {
    let n = note;
    match kind {
        Plane::Fourths => Arp::new(&[n.saturating_sub(10), n.saturating_sub(5), n]),
        Plane::Triad => Arp::new(&[scale.step(n, -4), scale.step(n, -2), n]),
        Plane::Cluster => Arp::new(&[scale.step(n, -2), scale.step(n, -1), n]),
    }
}

/// A chord voiced in close position around `center`: its first four chord tones inside one
/// octave from `center - 7` (each inversion starts that octave three semitones higher), lowest
/// first.
pub fn voice(chord: &Chord, center: u8, inversion: usize) -> Arp {
    let iv = chord.quality.intervals();
    let n = iv.len().min(4);
    let lo = center as i32 - 7 + 3 * (inversion % 4) as i32;
    let mut notes = [0u8; 4];
    for (x, i) in notes.iter_mut().zip(iv) {
        let pc = (chord.root + i) as i32 % 12;
        *x = (lo + (pc - lo).rem_euclid(12)).clamp(0, 127) as u8;
    }
    notes[..n].sort_unstable();
    Arp::new(&notes[..n])
}

/// An upper-structure triad over a chord (a polychord's top): over a dominant the major triad
/// a whole step up (9 #11 13); over a major chord the one a whole step up (lydian); over a
/// minor chord the major triad a minor 3rd up... the 7th chord's relative (b3 5 b7 = 9); with
/// the chord's 3rd and 7th underneath. Voiced around `center`.
pub fn upper_structure(chord: &Chord, center: u8) -> Arp {
    let r = chord.root as i32;
    let (top, guide): (i32, [i32; 2]) = match chord.family() {
        Family::Dominant => (r + 2, [4, 10]),
        Family::Major => (r + 2, [4, 11]),
        Family::Minor => (r + 3, [3, 10]),
        _ => (r + 3, [3, 9]),
    };
    let lo = center as i32 - 9;
    let at = |pc: i32, floor: i32| floor + (pc - floor).rem_euclid(12);
    let g0 = at(r + guide[0], lo);
    let g1 = at(r + guide[1], g0 + 1);
    let t0 = at(top, g1 + 1);
    let t1 = at(top + 4, t0 + 1);
    let t2 = at(top + 7, t1 + 1);
    Arp::new(&[g0, g1, t0, t1, t2].map(|n| n.clamp(0, 127) as u8))
}

/// A stack of three perfect fourths on `bottom` (McCoy Tyner).
pub fn fourths(bottom: u8) -> Arp {
    Arp::new(&[bottom, bottom + 5, bottom + 10])
}

/// The tritone substitute of a dominant (G7 → Db7).
pub fn tritone_sub(chord: &Chord) -> Chord {
    Chord::new((chord.root + 6) % 12, Quality::Dom7)
}

/// The backdoor dominant into a tonic (→ C: Bb7), and its ii (Fm7).
pub fn backdoor(tonic_root: u8) -> (Chord, Chord) {
    (Chord::new((tonic_root + 5) % 12, Quality::Minor7), Chord::new((tonic_root + 10) % 12, Quality::Dom7))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::chart::parse_chord;

    fn h(name: &str) -> Harm {
        Harm::new(parse_chord(name).unwrap())
    }

    #[test]
    fn scales_step_and_snap() {
        let c = h("C6").scale;
        assert_eq!(c.step(60, 1), 62);
        assert_eq!(c.step(60, -1), 59);
        assert_eq!(c.step(64, 2), 67);
        assert_eq!(c.snap(61), 60);
        let g = h("G7").scale;
        assert_eq!(g.step(65, 1), 67);
        assert!(g.contains(5) && !g.contains(6));
        assert!(Scale::of(&parse_chord("F").unwrap(), true).contains(11), "lydian #4");
    }

    #[test]
    fn chord_tones_and_degrees() {
        let g = h("G7");
        assert!(g.is_chord_tone(71) && g.is_chord_tone(65) && !g.is_chord_tone(64));
        assert_eq!(g.nearest_chord_tone(64), 65);
        assert_eq!(g.degree(0, 60), 67);
        assert_eq!(g.degree(2, 60), 71);
        assert_eq!(digital(&g, 55, false), [55, 57, 59, 62]);
        assert_eq!(digital(&h("Dm7"), 60, true), [69, 65, 64, 62]);
    }

    #[test]
    fn ornament_shapes() {
        let c = h("C6").scale;
        assert_eq!(turn(64, &c), [65, 64, 62, 64]);
        assert_eq!(mordent(67, &c, false), [67, 65, 67]);
        assert_eq!(enclosure(64, &c), [65, 63]);
        assert_eq!(pentatonic_up_half(7), [8, 10, 0, 3, 5]);
        assert_eq!(tritone_sub(&parse_chord("G7").unwrap()).root, 1);
        let (ii, v) = backdoor(0);
        assert_eq!((ii.root, v.root), (5, 10));
        // Planing keeps the melody on top; fourths are perfect.
        let p = plane(72, Plane::Fourths, &c);
        assert_eq!(p.notes(), [62, 67, 72]);
        let t = plane(72, Plane::Triad, &c);
        assert_eq!(t.notes(), [65, 69, 72]);
        assert_eq!(fourths(50).notes(), [50, 55, 60]);
    }

    #[test]
    fn voicings_sit_around_their_center() {
        for name in ["C6", "G7", "Dm7", "Bb7", "F#m7b5", "Ebmaj7"] {
            let c = parse_chord(name).unwrap();
            for inv in 0..4 {
                let v = voice(&c, 64, inv);
                let n = v.notes();
                assert!(n.windows(2).all(|w| w[0] < w[1]), "{name}: {n:?}");
                assert!(n.iter().all(|&x| (57..=77).contains(&x)), "{name} {inv}: {n:?}");
                assert!(n.iter().all(|&x| Harm::new(c).is_chord_tone(x)), "{name}: {n:?}");
            }
            let u = upper_structure(&c, 66);
            assert!(u.notes().windows(2).all(|w| w[0] < w[1]), "{name}: {:?}", u.notes());
        }
    }
}
