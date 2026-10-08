//! The music theory behind the [`super::Filters`]: just intonation, Coltrane changes,
//! melodic-minor sonorities and quartal voicings. All pure functions over [`super::chart`] types.
//!
//! # Just intonation
//! 5-limit ratios above the song's tonic (which keeps its equal-tempered pitch), per semitone:
//! `1, 16/15, 9/8, 6/5, 5/4, 4/3, 45/32, 3/2, 8/5, 5/3, 9/5, 15/8`. The minor seventh is the
//! 5-limit 9/5 (not the 7-limit 7/4 or Pythagorean 16/9): it makes V7 and ii7 slightly sour,
//! which is the point. The tuning never follows modulations, so chords on remote degrees get
//! wolf fifths (e.g. the "ii" D–A is 40/27 ≈ 680 cents in C).
//!
//! # Coltrane changes ([`coltrane`])
//! A *resolution* is a dominant-family chord whose root is a fifth above the next chord, and
//! that next chord (the *target*) is major or minor (so secondary dominants count too). If
//! the chord before the dominant is a minor 7 / half-diminished chord a fifth above the
//! dominant, it's a ii–V and joins the span. The span (ii + V, or just V) plus all but the last
//! bar of the target is rewritten as a cycle through major thirds into the target T. The
//! cycle wants 12 beats: it borrows from the target if the span is shorter (the target always
//! keeps at least its last bar, or all of it when shorter than a bar), and a longer span plays
//! as written until the last 12 beats before the target.
//! - with a ii and the full 12 beats: the *Countdown* matrix `ii7 (T+3)7 | (T+8)maj7 (T+11)7 |
//!   (T+4)maj7 (T+7)7 | T`, two beats each, so `| Dm7 | G7 | Cmaj7 | % |` becomes
//!   `| Dm7 Eb7 | Abmaj7 B7 | Emaj7 G7 | Cmaj7 |`;
//! - otherwise the *Giant Steps* tail `(T+3)7 (T+8)maj7 (T+11)7 (T+4)maj7 (T+7)7 → T`, as
//!   many of its last chords as fit at one chord per 2 beats (never faster); spare time goes
//!   to the final V, so `| G7 | % | C | % |` becomes `| Eb7 Abmaj7 | B7 Emaj7 | G7 | C |`.
//!
//! The total length never changes, so cadences still land — just late; the melody follows the
//! cycle (see [`super::melody`]).
//!
//! # Melodic minor ([`melodic_minor`])
//! Every chord becomes a mode of a melodic-minor scale (parent root in brackets, relative to
//! the chord root `r`):
//! - dominant resolving down a fifth → altered, 7th mode `[r+1]` (G7→C: Ab melodic minor);
//! - other dominants → lydian dominant, 4th mode `[r+7]` (G7: D melodic minor);
//! - major / augmented → lydian augmented (maj7♯5), 3rd mode `[r−3]` (C: A melodic minor);
//! - minor → minor-major (mMaj7 / m6), 1st mode `[r]`;
//! - half-diminished → locrian ♮2, 6th mode `[r+3]` (Dm7♭5: F melodic minor);
//! - diminished → "altered-ish": the altered scale on the root `[r+1]`, which holds three of
//!   the four dim7 tones.
//!
//! # Quartal voicings ([`quartal`])
//! Stacks of perfect fourths (semitones above the root):
//! - minor (dorian): the *So What* voicing — three fourths from the 9th plus a major third:
//!   `9 5 R 11 13` (Dm: E A D G B);
//! - dominant (mixolydian): fourths up from the 3rd `3 13 9 5` (G7: B E A D);
//! - sus: fourths up from the root `R 4 b7 b3`… i.e. `R 11 b7 b10` (G7sus4: G C F Bb);
//! - maj7 (lydian): fourths up from the 7th `7 3 13 9` (Cmaj7: B E A D);
//! - major triad / 6 / aug (pentatonic): fourths up from the 3rd `3 13 9 5` (C: E A D G);
//! - half-diminished (locrian) and diminished: fourths up from the root `R 11 b7 b10`.

use super::chart::{Chart, Chord, Family, Quality, Slot};

/// 5-limit just intonation ratios, per semitone above the tonic.
pub const JI_RATIOS: [f64; 12] = [
    1.0,
    16.0 / 15.0,
    9.0 / 8.0,
    6.0 / 5.0,
    5.0 / 4.0,
    4.0 / 3.0,
    45.0 / 32.0,
    3.0 / 2.0,
    8.0 / 5.0,
    5.0 / 3.0,
    9.0 / 5.0,
    15.0 / 8.0,
];

/// Equal-tempered frequency of a MIDI note.
pub fn et_hz(note: f64) -> f64 {
    440.0 * 2f64.powf((note - 69.0) / 12.0)
}

/// Frequency of a MIDI note, equal-tempered (`ji_key` = None) or justly tuned to the tonic
/// pitch class `ji_key` (0 = C).
pub fn note_hz(note: u8, ji_key: Option<u8>) -> f64 {
    match ji_key {
        None => et_hz(note as f64),
        Some(key) => {
            let d = (note as i32 - key as i32).rem_euclid(12);
            et_hz((note as i32 - d) as f64) * JI_RATIOS[d as usize]
        }
    }
}

/// Cents between two frequencies.
pub fn cents(lo: f64, hi: f64) -> f64 {
    1200.0 * (hi / lo).log2()
}

fn pc(x: i32) -> u8 {
    x.rem_euclid(12) as u8
}

/// Does the chord at `i` resolve (down a fifth) to the chord after it? (`slots` loops.)
fn resolves_down_a_fifth(slots: &[Slot], i: usize) -> bool {
    let next = &slots[(i + 1) % slots.len()];
    pc(slots[i].chord.root as i32 - 7) == next.chord.root
}

/// Beats of a full Coltrane cycle (six chords, two beats each).
const CYCLE: f64 = 12.0;

/// Coltrane-ize a chart (see the module docs). Works on merged slots; the result's slots
/// are contiguous and cover the same length.
pub fn coltrane(chart: &Chart) -> Chart {
    let src = chart.merged();
    let n = src.len();
    let mut out: Vec<Slot> = Vec::new();
    let mut i = 0;
    while i < n {
        // Is there a V (at i or i+1, with a ii at i) resolving to a target?
        let try_at = |ii: Option<usize>, v: usize| -> Option<(Option<usize>, usize, usize)> {
            let t = v + 1;
            if t >= n {
                return None;
            }
            let (vc, tc) = (src[v].chord, src[t].chord);
            let ok = vc.family() == Family::Dominant
                && matches!(tc.family(), Family::Major | Family::Minor)
                && pc(vc.root as i32 - 7) == tc.root;
            let ii_ok = ii.is_none_or(|k| {
                let c = src[k].chord;
                matches!(c.quality, Quality::Minor7 | Quality::Minor | Quality::HalfDim | Quality::Minor6)
                    && pc(c.root as i32 - 7) == vc.root
            });
            (ok && ii_ok).then_some((ii, v, t))
        };
        let Some((ii, v, t)) = try_at(Some(i), i + 1).or_else(|| try_at(None, i)) else {
            out.push(src[i]);
            i += 1;
            continue;
        };
        let span_start = src[ii.unwrap_or(v)].start;
        let target = src[t];
        // Borrow from the target (keeping at least its last bar) only as much as the cycle needs.
        let pre = target.start - span_start;
        let borrow = (target.dur - target.dur.min(4.0)).min((CYCLE - pre).max(0.0));
        let arrival = target.start + borrow;
        let avail = pre + borrow;
        let cyc = avail.min(CYCLE);
        // A span longer than the cycle starts as written.
        let head_end = span_start + (avail - cyc);
        for s in &src[ii.unwrap_or(v)..=v] {
            let end = s.end().min(head_end);
            if end > s.start + 1e-9 {
                out.push(Slot { start: s.start, dur: end - s.start, chord: s.chord });
            }
        }
        let tr = target.chord.root as i32;
        let dom = |x: i32| Chord::new(pc(tr + x), Quality::Dom7);
        let maj = |x: i32| Chord::new(pc(tr + x), Quality::Maj7);
        let mut cycle: Vec<(Chord, f64)> = Vec::new();
        if let Some(k) = ii.filter(|_| cyc >= CYCLE - 1e-9) {
            // Countdown: ii (T+3)7 | (T+8)maj7 (T+11)7 | (T+4)maj7 (T+7)7 | T
            cycle.push((src[k].chord, 2.0));
            for c in [dom(3), maj(8), dom(11), maj(4), dom(7)] {
                cycle.push((c, 2.0));
            }
        } else {
            let tail = [dom(3), maj(8), dom(11), maj(4), dom(7)];
            let k = (((cyc + 1e-9) / 2.0).floor() as usize).clamp(1, 5);
            let mut durs = vec![2.0; k];
            // Spare time goes to the final V.
            durs[k - 1] += cyc - 2.0 * k as f64;
            for (c, d) in tail[5 - k..].iter().zip(durs) {
                cycle.push((*c, d));
            }
        }
        let mut time = head_end;
        for (chord, dur) in cycle {
            if dur > 1e-9 {
                out.push(Slot { start: time, dur, chord });
                time += dur;
            }
        }
        out.push(Slot { start: arrival, dur: target.end() - arrival, chord: target.chord });
        i = t + 1;
    }
    Chart { slots: out, bars: chart.bars, meter: chart.meter }
}

/// Melodic-minor modes we use, by the degree of the parent scale the chord root sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MmMode {
    /// 1st mode: minor-major.
    MinorMajor,
    /// 3rd mode: lydian augmented.
    LydianAugmented,
    /// 4th mode: lydian dominant.
    LydianDominant,
    /// 6th mode: locrian ♮2.
    LocrianNat2,
    /// 7th mode: altered (super-locrian).
    Altered,
}

/// The melodic-minor treatment of one chord.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MmChord {
    /// The chord's root pitch class.
    pub root: u8,
    /// Root pitch class of the parent melodic-minor scale.
    pub parent: u8,
    pub mode: MmMode,
}

pub const MELODIC_MINOR: [u8; 7] = [0, 2, 3, 5, 7, 9, 11];

impl MmChord {
    /// Pitch classes of the parent scale.
    pub fn scale(&self) -> [u8; 7] {
        MELODIC_MINOR.map(|i| (self.parent + i) % 12)
    }

    pub fn in_scale(&self, pc: u8) -> bool {
        self.scale().contains(&(pc % 12))
    }

    /// The sonority to voice, as semitones above the chord root (every one is in the scale).
    pub fn sonority(&self) -> &'static [u8] {
        match self.mode {
            // mMaj7(9): R b3 5 7 9
            MmMode::MinorMajor => &[0, 3, 7, 11, 14],
            // maj7#5(9): R 3 #5 7 9
            MmMode::LydianAugmented => &[0, 4, 8, 11, 14],
            // 9#11: R 3 5 b7 9 #11
            MmMode::LydianDominant => &[0, 4, 7, 10, 14, 18],
            // m9b5: R b3 b5 b7 9
            MmMode::LocrianNat2 => &[0, 3, 6, 10, 14],
            // 7alt: R 3 b7 #9 b13
            MmMode::Altered => &[0, 4, 10, 15, 20],
        }
    }
}

/// Melodic-minor treatment of chord `i` of `slots` (the next chord decides altered vs lydian
/// dominant; the chart loops).
pub fn melodic_minor(slots: &[Slot], i: usize) -> MmChord {
    let c = slots[i].chord;
    let r = c.root as i32;
    let (parent, mode) = match c.family() {
        Family::Dominant if resolves_down_a_fifth(slots, i) => (r + 1, MmMode::Altered),
        Family::Dominant => (r + 7, MmMode::LydianDominant),
        Family::Major | Family::Aug => (r - 3, MmMode::LydianAugmented),
        Family::Minor => (r, MmMode::MinorMajor),
        Family::HalfDim => (r + 3, MmMode::LocrianNat2),
        Family::Dim => (r + 1, MmMode::Altered),
    };
    MmChord { root: c.root, parent: pc(parent), mode }
}

/// Quartal voicing of a chord, as semitones above its root (see the module docs).
pub fn quartal(chord: &Chord) -> &'static [i8] {
    match chord.quality {
        Quality::Minor | Quality::Minor6 | Quality::Minor7 | Quality::MinMaj7 => &[2, 7, 12, 17, 21],
        Quality::Sus4 => &[0, 5, 10, 15],
        Quality::Maj7 => &[-1, 4, 9, 14],
        q if q.family() == Family::Dominant => &[4, 9, 14, 19],
        Quality::HalfDim | Quality::Dim7 => &[0, 5, 10, 15],
        _ => &[4, 9, 14, 19],
    }
}

#[cfg(test)]
mod tests {
    use super::super::chart::{self, parse_chord};
    use super::*;

    #[test]
    fn ji_major_third_is_386_cents() {
        // In C: E above C is 5/4.
        let c4 = note_hz(60, Some(0));
        assert!((c4 - et_hz(60.0)).abs() < 1e-9, "tonic stays equal-tempered");
        let third = cents(c4, note_hz(64, Some(0)));
        assert!((third - 386.3137).abs() < 0.01, "{third}");
        assert!((cents(c4, note_hz(67, Some(0))) - 701.955).abs() < 0.01);
        // Relative to another key and across octaves: in Bb (10), D5 is 5/4 above Bb4.
        let bb4 = note_hz(70, Some(10));
        assert!((cents(bb4, note_hz(74, Some(10))) - 386.3137).abs() < 0.01);
        // Below the tonic's pitch class: A3 in Bb is 15/8 above Bb2.
        assert!((cents(note_hz(58, Some(10)) / 2.0, note_hz(57, Some(10))) - 1088.27).abs() < 0.01);
        // The wolf: D–A in C is 40/27.
        assert!((cents(note_hz(62, Some(0)), note_hz(69, Some(0))) - 680.45).abs() < 0.01);
        // Equal temperament when off.
        assert!((note_hz(69, None) - 440.0).abs() < 1e-9);
    }

    #[test]
    fn countdown_from_a_four_bar_two_five_one() {
        let c = chart::parse("| Dm7 | G7 | Cmaj7 | % |").unwrap();
        let t = coltrane(&c);
        assert_eq!(t.to_text(), "Dm7 Eb7 | Abmaj7 B7 | Emaj7 G7 | Cmaj7");
        let total: f64 = t.slots.iter().map(|s| s.dur).sum();
        assert_eq!(total, 16.0);
        assert_eq!(t.bars, 4);
    }

    #[test]
    fn giant_steps_tails_fit_the_span() {
        // Plain V-I with a 2-bar V: 8 beats (+4 borrowed from the 2-bar target) = 12 beats:
        // Eb7 Ab | B7 E | G7 | C.
        let t = coltrane(&chart::parse("| G7 | % | C | % |").unwrap());
        assert_eq!(t.to_text(), "Eb7 Abmaj7 | B7 Emaj7 | G7 | C");
        // One bar of V into a one-bar target: two chords.
        let t = coltrane(&chart::parse("| F | G7 | C |").unwrap());
        assert_eq!(t.to_text(), "F | Emaj7 G7 | C");
        // Two beats of V: unchanged.
        let t = coltrane(&chart::parse("| Dm7 G7 | C |").unwrap());
        assert_eq!(t.to_text(), "Emaj7 G7 | C");
        // Secondary dominant into a minor chord, and the loop length is kept.
        let src = chart::parse("| C | A7 | Dm7 | G7 | C | % | % | % |").unwrap();
        let t = coltrane(&src);
        assert_eq!(t.bars, 8);
        assert!((t.slots.iter().map(|s| s.dur).sum::<f64>() - 32.0).abs() < 1e-9);
        for w in t.slots.windows(2) {
            assert!((w[0].end() - w[1].start).abs() < 1e-9);
            assert!(w[0].dur >= 2.0 - 1e-9 || w[0].start == 0.0);
        }
        // A7 -> Dm7 is a resolution too: Gbmaj7 A7 | Dm7.
        assert!(t.to_text().contains("A7"), "{}", t.to_text());
    }

    #[test]
    fn melodic_minor_table() {
        let c = chart::parse("| G7 | C | Dm7b5 | G7 | Cm | E7 | Fdim7 | C |").unwrap();
        let s = &c.slots;
        let mm = |i| melodic_minor(s, i);
        assert_eq!((mm(0).parent, mm(0).mode), (8, MmMode::Altered)); // G7->C: Ab mm
        assert_eq!((mm(1).parent, mm(1).mode), (9, MmMode::LydianAugmented)); // C: A mm
        assert_eq!((mm(2).parent, mm(2).mode), (5, MmMode::LocrianNat2)); // Dm7b5: F mm
        assert_eq!((mm(4).parent, mm(4).mode), (0, MmMode::MinorMajor)); // Cm: C mm
        assert_eq!((mm(5).parent, mm(5).mode), (11, MmMode::LydianDominant)); // E7 -> Fdim: B mm
        assert_eq!((mm(6).parent, mm(6).mode), (6, MmMode::Altered));
        // Every sonority tone, and the chord root, is in the parent scale.
        for i in 0..s.len() {
            let m = mm(i);
            assert!(m.in_scale(m.root));
            for iv in m.sonority() {
                assert!(m.in_scale(m.root + iv), "{:?} {iv}", m);
            }
        }
    }

    #[test]
    fn quartal_voicings_stack_fourths() {
        for name in ["Dm7", "G7", "G7sus4", "Cmaj7", "C", "C6", "Bm7b5", "Ddim7", "Am6", "F9", "Caug"] {
            let c = parse_chord(name).unwrap();
            let v = quartal(&c);
            let steps: Vec<i8> = v.windows(2).map(|w| w[1] - w[0]).collect();
            // Perfect fourths, except the So What voicing's major third on top.
            let so_what = c.family() == Family::Minor;
            for (k, s) in steps.iter().enumerate() {
                let want = if so_what && k == steps.len() - 1 { 4 } else { 5 };
                assert_eq!(*s, want, "{name}: {v:?}");
            }
            assert!(v.len() >= 4);
        }
        // So What on D minor: E A D G B.
        let d = parse_chord("Dm7").unwrap();
        let pcs: Vec<u8> = quartal(&d).iter().map(|i| pc(2 + *i as i32)).collect();
        assert_eq!(pcs, [4, 9, 2, 7, 11]);
    }
}
