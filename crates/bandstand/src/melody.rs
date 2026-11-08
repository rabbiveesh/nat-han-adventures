//! The melody (pulse 1) following a reharmonization, so a filter sounds like the tune played
//! over new changes rather than the old tune clashing with a new band.
//!
//! # Function, then re-expression
//! Every melody note is read against the chord sounding under it in the *original* chart (a
//! note that starts up to an 8th before a chord change and is held mostly past it belongs to
//! the new chord: an anticipation), as a degree of that chord's chord-scale ([`chord_scale`]):
//! its letter above the root (1..7, so chord tones are 1 3 5 7 and tensions 2 4 6) plus an
//! alteration. Chromatic notes outside the scale are either *approach notes* (a step from the
//! next note, sounding straight into it) or alterations of the scale note just above (blue
//! notes: a b3 over a mixolydian chord stays a b3). The note is then re-expressed as the same
//! degree of the *new* chord's scale; approach notes keep their written distance from the
//! next note's new pitch, so they still lead into it.
//!
//! # Voice leading
//! Each mapped pitch class still needs an octave: a small Viterbi pass picks, for every note,
//! one within a fifth (7 semitones) of the written pitch and within o3–o6 (MIDI 48..=95),
//! minimising the distance from the written pitch, the change of each melodic interval and
//! (heavily) any change of direction, so the contour survives and repeated notes stay
//! repeated. Rhythm, volume, duty and slurs are never touched: only `Note` pitches change.
//!
//! # Per filter
//! - **Coltrane**: wherever [`theory::coltrane`] substituted a chord, the melody moves through
//!   the cycle with it (dorian degree 3 of Dm7 → mixolydian degree 3 of Eb7, ...), i.e. the
//!   phrase is transposed, chord by chord, like a horn player blowing the head over Giant
//!   Steps. Where the chart is unchanged the notes are exactly as written.
//! - **Melodic minor**: the same degree map into the chord's melodic-minor mode
//!   ([`theory::melodic_minor`], spelled from the chord root: G7→C gets G altered = G Ab Bb Cb
//!   Db Eb F, so the 3rd becomes the #9 and the 4th the 3rd). Chromatic notes are snapped onto
//!   the scale, except approach notes, which stay chromatic but at most a semitone from their
//!   target.
//! - **Quartal**: the roots don't change, so the melody stays as written except for the avoid
//!   notes that fight the fourths ([`quartal_avoid`]) on strong beats (1 and 3) or held long:
//!   the 4th over a major chord becomes the #11 (lydian, like the comping), the 4th over a
//!   dominant the 5th, the 3rd over a sus chord the 4th, the b6 over a minor chord the 5th, the
//!   b2 over a half-diminished chord the 9th.
//! - Just intonation is a tuning and doesn't touch the notes.

use super::Harmony;
use super::chart::{Chart, Chord, Quality, Slot};
use super::mml::{EventKind, Track};
use super::theory::{self, MmChord};

/// Melody range: o3 c .. o6 b.
pub const LO: i32 = 48;
pub const HI: i32 = 95;
/// Never move a note further than this from where it was written.
pub const MAX_SHIFT: i32 = 7;

/// Major-scale semitones per letter: the reference for alterations.
const MAJOR: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];

/// A chord-scale: its notes as (semitones above the chord root, letter 0..=6 above the root),
/// ascending. Seven notes for the modes; eight for the symmetric scales (two notes share a
/// letter, e.g. b9 and #9 of the half-whole scale). Every letter has at least one note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChordScale {
    pub root: u8,
    notes: Vec<(u8, u8)>,
}

/// A note's function in a chord-scale: letter (0 = root, 2 = 3rd, ...) and alteration from
/// the major scale (b3 = -1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Degree {
    pub letter: u8,
    pub alt: i32,
}

impl ChordScale {
    /// A seven-note scale (semitones above the root, ascending): letters in order.
    fn heptatonic(root: u8, semis: [u8; 7]) -> Self {
        ChordScale { root: root % 12, notes: semis.iter().enumerate().map(|(l, &s)| (s, l as u8)).collect() }
    }

    fn spelled(root: u8, notes: &[(u8, u8)]) -> Self {
        ChordScale { root: root % 12, notes: notes.to_vec() }
    }

    /// Pitch classes of the scale.
    pub fn pitch_classes(&self) -> impl Iterator<Item = u8> + '_ {
        self.notes.iter().map(|&(s, _)| (self.root + s) % 12)
    }

    pub fn contains(&self, pc: u8) -> bool {
        self.pitch_classes().any(|p| p == pc % 12)
    }

    /// The degree of a pitch class, if it's in the scale.
    pub fn degree(&self, pc: u8) -> Option<Degree> {
        let semi = (pc as i32 - self.root as i32).rem_euclid(12);
        self.notes
            .iter()
            .find(|&&(s, _)| s as i32 == semi)
            .map(|&(s, l)| Degree { letter: l, alt: s as i32 - MAJOR[l as usize] })
    }

    /// The pitch class of a degree: the scale's note on that letter, the one with the closest
    /// alteration when there are two.
    pub fn pc_of(&self, d: Degree) -> u8 {
        let (s, _) = self
            .notes
            .iter()
            .filter(|&&(_, l)| l == d.letter)
            .min_by_key(|&&(s, l)| (s as i32 - MAJOR[l as usize] - d.alt).abs())
            .copied()
            .expect("every letter is in the scale");
        (self.root + s) % 12
    }

    /// A chromatic pitch class (not in the scale) as an alteration of a scale note: the
    /// nearest one, the note above on a tie (blue notes are flats). Returns that note's
    /// degree and the extra semitones (`pc = pc_of(degree) + extra`).
    fn chromatic(&self, pc: u8) -> (Degree, i32) {
        let semi = (pc as i32 - self.root as i32).rem_euclid(12);
        // Signed distance from each scale note to the chromatic one, wrapped to -6..=6.
        let (extra, s) = self
            .notes
            .iter()
            .map(|&(s, _)| ((semi - s as i32 + 6).rem_euclid(12) - 6, s))
            .min_by_key(|&(extra, _)| (extra.abs(), extra > 0))
            .expect("scales aren't empty");
        (self.degree(self.root + s).expect("a scale note"), extra)
    }
}

/// The chord-scale a melody note is read against, per quality:
/// major / 6 / maj7 → ionian; 7 / 9 / 7sus4 → mixolydian; 7b9 / 7#9 → half-whole diminished;
/// 7#5 → altered (super-locrian); m / m7 / m6 → dorian; mMaj7 → melodic minor; m7b5 →
/// locrian; dim7 → whole-half diminished; aug → lydian augmented.
pub fn chord_scale(chord: &Chord) -> ChordScale {
    let r = chord.root;
    match chord.quality {
        Quality::Major | Quality::Six | Quality::Maj7 => ChordScale::heptatonic(r, [0, 2, 4, 5, 7, 9, 11]),
        Quality::Dom7 | Quality::Dom9 | Quality::Sus4 => ChordScale::heptatonic(r, [0, 2, 4, 5, 7, 9, 10]),
        // 1 b9 #9 3 #11 5 13 b7
        Quality::Dom7b9 | Quality::Dom7s9 => {
            ChordScale::spelled(r, &[(0, 0), (1, 1), (3, 1), (4, 2), (6, 3), (7, 4), (9, 5), (10, 6)])
        }
        Quality::Dom7s5 => ChordScale::heptatonic(r, [0, 1, 3, 4, 6, 8, 10]),
        Quality::Minor | Quality::Minor7 | Quality::Minor6 => ChordScale::heptatonic(r, [0, 2, 3, 5, 7, 9, 10]),
        Quality::MinMaj7 => ChordScale::heptatonic(r, [0, 2, 3, 5, 7, 9, 11]),
        Quality::HalfDim => ChordScale::heptatonic(r, [0, 1, 3, 5, 6, 8, 10]),
        // 1 2 b3 4 b5 b6 6(bb7) 7
        Quality::Dim7 => ChordScale::spelled(r, &[(0, 0), (2, 1), (3, 2), (5, 3), (6, 4), (8, 5), (9, 5), (11, 6)]),
        Quality::Aug => ChordScale::heptatonic(r, [0, 2, 4, 6, 8, 9, 11]),
    }
}

/// A melodic-minor treatment as a chord-scale: the parent scale read from the chord root
/// (the mode), letters in order.
pub fn mm_scale(mm: &MmChord) -> ChordScale {
    let mut semis: Vec<u8> = mm.scale().iter().map(|&p| (p as i32 - mm.root as i32).rem_euclid(12) as u8).collect();
    semis.sort_unstable();
    ChordScale::heptatonic(mm.root, semis.try_into().expect("seven notes"))
}

/// Quartal avoid notes of a chord: (semitones above the root, replacement).
pub fn quartal_avoid(chord: &Chord) -> &'static [(u8, u8)] {
    match chord.quality {
        Quality::Major | Quality::Six | Quality::Maj7 => &[(5, 6)],
        Quality::Dom7 | Quality::Dom9 | Quality::Dom7b9 | Quality::Dom7s9 | Quality::Dom7s5 => &[(5, 7)],
        Quality::Sus4 => &[(4, 5)],
        Quality::Minor | Quality::Minor6 | Quality::Minor7 | Quality::MinMaj7 => &[(8, 7)],
        Quality::HalfDim => &[(1, 2)],
        Quality::Dim7 | Quality::Aug => &[],
    }
}

/// Does a note at `start` (beats) sit on a strong beat (1 or 3 of the bar)?
pub fn strong_beat(start: f64) -> bool {
    let x = start.rem_euclid(2.0);
    !(1e-6..=2.0 - 1e-6).contains(&x)
}

/// The slot a note belongs to harmonically: the one sounding at its start, or the next one
/// when the note anticipates it (starts at most an 8th before the change and is held longer
/// after it than before).
pub fn sounding(slots: &[Slot], beats: f64, start: f64, dur: f64) -> usize {
    let at = |t: f64| slots.partition_point(|s| s.start <= t.rem_euclid(beats) + 1e-9).saturating_sub(1);
    let i = at(start);
    let change = slots[i].end();
    let before = change - start.rem_euclid(beats);
    if before > 1e-9 && before <= 0.5 + 1e-9 && dur - before > before + 1e-9 { (i + 1) % slots.len() } else { i }
}

/// What a note becomes before octaves are chosen.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Plan {
    /// Exactly as written.
    Keep,
    /// This pitch class, octave to be chosen.
    Pc(u8),
    /// Chromatic approach: this many semitones from the next note's new pitch.
    Approach(i32),
}

/// The melody `track` (pulse 1) re-expressed over `harmony`'s version of `chart` (see the
/// module docs). [`Harmony::Original`] returns it unchanged.
pub fn reharmonize(track: &Track, chart: &Chart, harmony: Harmony) -> Track {
    if harmony == Harmony::Original {
        return track.clone();
    }
    let beats = chart.beats();
    let orig = chart.merged();
    let new = match harmony {
        Harmony::Coltrane => theory::coltrane(chart).merged(),
        _ => orig.clone(),
    };
    let src_scales: Vec<ChordScale> = orig.iter().map(|s| chord_scale(&s.chord)).collect();
    let dst_scales: Vec<ChordScale> = match harmony {
        Harmony::MelodicMinor => (0..new.len()).map(|j| mm_scale(&theory::melodic_minor(&new, j))).collect(),
        _ => new.iter().map(|s| chord_scale(&s.chord)).collect(),
    };

    // The notes: (event index, start, dur, pitch).
    let notes: Vec<(usize, f64, f64, i32)> = track
        .events
        .iter()
        .enumerate()
        .filter_map(|(k, e)| match e.kind {
            EventKind::Note(n) => Some((k, e.start, e.dur, n as i32)),
            _ => None,
        })
        .collect();

    let plans: Vec<Plan> = notes
        .iter()
        .enumerate()
        .map(|(k, &(_, start, dur, pitch))| {
            let pc = pitch.rem_euclid(12) as u8;
            let i = sounding(&orig, beats, start, dur);
            let j = sounding(&new, beats, start, dur);
            // A chromatic step straight into the next note.
            let approach = notes.get(k + 1).and_then(|&(_, ns, _, np)| {
                let step = pitch - np;
                ((ns - (start + dur)).abs() < 1e-6 && (1..=2).contains(&step.abs())).then_some(step)
            });
            match harmony {
                Harmony::Quartal => {
                    let chord = orig[i].chord;
                    let semi = (pc as i32 - chord.root as i32).rem_euclid(12) as u8;
                    let swap = quartal_avoid(&chord).iter().find(|(a, _)| *a == semi);
                    match swap {
                        Some(&(_, to)) if strong_beat(start) || dur >= 1.5 - 1e-9 => Plan::Pc((chord.root + to) % 12),
                        _ => Plan::Keep,
                    }
                }
                // Unchanged chart: as written, except chromatic approaches into changed notes.
                Harmony::Coltrane if orig[i].chord == new[j].chord => match approach {
                    Some(step) if src_scales[i].degree(pc).is_none() => Plan::Approach(step),
                    _ => Plan::Keep,
                },
                _ => {
                    let (src, dst) = (&src_scales[i], &dst_scales[j]);
                    match src.degree(pc) {
                        Some(d) => Plan::Pc(dst.pc_of(d)),
                        None => match (approach, harmony) {
                            (Some(step), Harmony::MelodicMinor) => Plan::Approach(step.signum()),
                            (Some(step), _) => Plan::Approach(step),
                            (None, _) => {
                                let (d, extra) = src.chromatic(pc);
                                let extra = if harmony == Harmony::MelodicMinor { 0 } else { extra };
                                Plan::Pc((dst.pc_of(d) as i32 + extra).rem_euclid(12) as u8)
                            }
                        },
                    }
                }
            }
        })
        .collect();

    let mut out: Vec<i32> = choose_octaves(&notes, &plans);
    // Approach notes, right to left (so chains of them lead into each other).
    for k in (0..notes.len()).rev() {
        if let Plan::Approach(step) = plans[k] {
            let next = out[k + 1];
            out[k] = if (LO..=HI).contains(&(next + step)) { next + step } else { next - step };
        }
    }

    let mut t = track.clone();
    for (k, &(idx, ..)) in notes.iter().enumerate() {
        t.events[idx].kind = EventKind::Note(out[k] as u8);
    }
    t
}

/// Octaves for the planned notes (approach notes get a placeholder): a Viterbi pass over the
/// non-approach notes, see the module docs.
fn choose_octaves(notes: &[(usize, f64, f64, i32)], plans: &[Plan]) -> Vec<i32> {
    let idx: Vec<usize> = (0..notes.len()).filter(|&k| !matches!(plans[k], Plan::Approach(_))).collect();
    let cands: Vec<Vec<i32>> = idx
        .iter()
        .map(|&k| {
            let orig = notes[k].3;
            match plans[k] {
                Plan::Pc(pc) => {
                    let all: Vec<i32> = (LO..=HI).filter(|n| n.rem_euclid(12) == pc as i32).collect();
                    let near: Vec<i32> = all.iter().copied().filter(|n| (n - orig).abs() <= MAX_SHIFT).collect();
                    if near.is_empty() { vec![*all.iter().min_by_key(|n| (*n - orig).abs()).unwrap()] } else { near }
                }
                _ => vec![orig],
            }
        })
        .collect();
    let mut out: Vec<i32> = notes.iter().map(|n| n.3).collect();
    if idx.is_empty() {
        return out;
    }
    // cost[c], back[step][c]
    let mut cost: Vec<f64> = cands[0].iter().map(|&c| (c - notes[idx[0]].3).abs() as f64).collect();
    let mut back: Vec<Vec<usize>> = vec![vec![0; cands[0].len()]];
    for s in 1..idx.len() {
        let (pk, k) = (idx[s - 1], idx[s]);
        let (po, o) = (notes[pk].3, notes[k].3);
        let d_o = o - po;
        // Across a rest of more than a beat the phrase may start afresh.
        let gap = notes[k].1 - (notes[pk].1 + notes[pk].2);
        let w = if gap > 1.0 + 1e-9 { 0.3 } else { 1.0 };
        let mut next_cost = Vec::with_capacity(cands[s].len());
        let mut next_back = Vec::with_capacity(cands[s].len());
        for &c in &cands[s] {
            let node = (c - o).abs() as f64;
            let (best, arg) = cands[s - 1]
                .iter()
                .enumerate()
                .map(|(a, &p)| {
                    let d_n = c - p;
                    let mut e = 0.5 * (d_n - d_o).abs() as f64;
                    if d_n.signum() != d_o.signum() {
                        e += 6.0;
                    }
                    (cost[a] + w * e, a)
                })
                .fold((f64::INFINITY, 0), |acc, x| if x.0 < acc.0 { x } else { acc });
            next_cost.push(best + node);
            next_back.push(arg);
        }
        cost = next_cost;
        back.push(next_back);
    }
    let mut c = (0..cost.len()).fold(0, |b, a| if cost[a] < cost[b] { a } else { b });
    for s in (0..idx.len()).rev() {
        out[idx[s]] = cands[s][c];
        c = back[s][c];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::chart::{self, parse_chord};
    use super::super::mml::{self, Channel};
    use super::*;

    fn ch(s: &str) -> Chord {
        parse_chord(s).unwrap()
    }

    /// Map one pitch class from one chord to another by degree.
    fn map(pc: u8, from: &str, to: &str) -> u8 {
        let d = chord_scale(&ch(from)).degree(pc).expect("in scale");
        chord_scale(&ch(to)).pc_of(d)
    }

    fn pitches(t: &Track) -> Vec<i32> {
        t.events
            .iter()
            .filter_map(|e| match e.kind {
                EventKind::Note(n) => Some(n as i32),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn degree_tables() {
        // F, the minor 3rd of Dm7, is the major 3rd G of Eb7.
        assert_eq!(map(5, "Dm7", "Eb7"), 7);
        // Root, 5th, b7 of Dm7 over Eb7: Eb, Bb, Db.
        assert_eq!([2, 9, 0].map(|p| map(p, "Dm7", "Eb7")), [3, 10, 1]);
        // G7 mixolydian over Abmaj7 ionian: B (3rd) -> C, F (b7) -> G (maj7), A (9) -> Bb.
        assert_eq!([11, 5, 9].map(|p| map(p, "G7", "Abmaj7")), [0, 7, 10]);
        // Cmaj7 over B7: E (3rd) -> D#, B (7th) -> A (b7).
        assert_eq!([4, 11].map(|p| map(p, "Cmaj7", "B7")), [3, 9]);
        // Half-whole over C7b9: the b9 and #9 keep their own alterations.
        let hw = chord_scale(&ch("C7b9"));
        assert_eq!(hw.degree(1), Some(Degree { letter: 1, alt: -1 }));
        assert_eq!(hw.degree(3), Some(Degree { letter: 1, alt: 1 }));
        assert_eq!(chord_scale(&ch("F7b9")).pc_of(hw.degree(3).unwrap()), 8);
        // Every scale covers all seven letters.
        for (q, _) in Quality::ALL {
            let s = chord_scale(&Chord::new(0, q));
            for l in 0..7 {
                assert!(s.notes.iter().any(|&(_, x)| x == l), "{q:?} letter {l}");
            }
            for &(semi, _) in &s.notes {
                assert_eq!(s.pc_of(s.degree(semi).unwrap()), semi, "{q:?}");
            }
            // Chord tones are in their own chord-scale.
            for pc in Chord::new(0, q).pitch_classes() {
                assert!(s.contains(pc), "{q:?} {pc}");
            }
        }
        // Blue notes: Eb over C7 is an alteration of the 3rd -> Gb over Eb7.
        let (d, extra) = chord_scale(&ch("C7")).chromatic(3);
        assert_eq!((d.letter, extra), (2, -1));
        assert_eq!((chord_scale(&ch("Eb7")).pc_of(d) as i32 + extra).rem_euclid(12), 6);
    }

    #[test]
    fn melodic_minor_modes_by_degree() {
        let c = chart::parse("| G7 | C |").unwrap();
        let alt = mm_scale(&theory::melodic_minor(&c.slots, 0));
        // G altered: G Ab Bb Cb Db Eb F.
        assert_eq!(alt.pitch_classes().collect::<Vec<_>>(), [7, 8, 10, 11, 1, 3, 5]);
        let mixo = chord_scale(&ch("G7"));
        // B (3rd) -> Bb, C (4th) -> B, D (5th) -> Db.
        assert_eq!([11, 0, 2].map(|p| alt.pc_of(mixo.degree(p).unwrap())), [10, 11, 1]);
    }

    #[test]
    fn coltrane_transposes_a_two_five_one_through_the_cycle() {
        let c = chart::parse("| Dm7 | G7 | Cmaj7 | % |").unwrap();
        let mel = mml::parse("o5 d4 f4 a4 >c4 | <b4 a4 g4 f4 | e2 g4 b4 | e2. r4", Channel::Melodic).unwrap();
        let t = reharmonize(&mel, &c, Harmony::Coltrane);
        let p = pitches(&t);
        let pcs: Vec<i32> = p.iter().map(|n| n % 12).collect();
        // Dm7 (kept) Eb7 | Abmaj7 B7 | Emaj7 G7 | Cmaj7 (kept).
        assert_eq!(&p[..2], &[74, 77], "the ii is untouched");
        assert_eq!(&pcs[2..4], &[10, 1], "A C over Eb7 -> Bb Db");
        assert_eq!(&pcs[4..6], &[0, 10], "B A over Abmaj7 -> C Bb");
        assert_eq!(&pcs[6..8], &[11, 9], "G F over B7 -> B A");
        assert_eq!(pcs[8], 8, "E over Emaj7 -> G#");
        assert_eq!(&pcs[9..11], &[2, 5], "G B over G7 -> D F");
        assert_eq!(p[11], 76, "the I is untouched");
        // Each substituted pair moves together (a transposition), with the written intervals.
        assert_eq!(p[3] - p[2], 3);
        assert_eq!(p[4] - p[5], 2);
        assert_eq!(p[6] - p[7], 2);
        for (a, b) in p.iter().zip(pitches(&mel)) {
            assert!((a - b).abs() <= MAX_SHIFT);
        }
    }

    #[test]
    fn approaches_lead_into_the_new_target_and_rhythm_is_kept() {
        let c = chart::parse("| Dm7 | G7 | Cmaj7 | % |").unwrap();
        // C# is a chromatic approach to D, then a repeated note and a slur.
        let mel = mml::parse("o5 r4 c+8 d8 a8 a8 a4& | b4 a4 g4 f4 | e1 | e1", Channel::Melodic).unwrap();
        for h in [Harmony::Coltrane, Harmony::MelodicMinor, Harmony::Quartal] {
            let t = reharmonize(&mel, &c, h);
            assert_eq!(t.events.len(), mel.events.len());
            for (a, b) in t.events.iter().zip(&mel.events) {
                assert_eq!((a.start, a.dur, a.tie, a.volume, a.duty), (b.start, b.dur, b.tie, b.volume, b.duty));
                assert_eq!(matches!(a.kind, EventKind::Note(_)), matches!(b.kind, EventKind::Note(_)));
            }
            let p = pitches(&t);
            assert_eq!(p[1] - p[0], 1, "{h:?}: approach still a semitone below its target");
            assert_eq!(p[3], p[4], "{h:?}: repeated notes stay repeated");
        }
    }

    #[test]
    fn quartal_only_fixes_strong_avoid_notes() {
        let c = chart::parse("| C | G7 |").unwrap();
        // F on beat 1 over C: avoid -> F#. F on the "and" of 2: passing, kept.
        // C on beat 1 over G7: avoid -> D.
        let mel = mml::parse("o5 f4 e8 f8 e4 d4 | c4 <b4 >d4 g4", Channel::Melodic).unwrap();
        let p = pitches(&reharmonize(&mel, &c, Harmony::Quartal));
        assert_eq!(p, [78, 76, 77, 76, 74, 74, 71, 74, 79]);
    }
}
