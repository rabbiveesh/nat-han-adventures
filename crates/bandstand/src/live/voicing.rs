//! How a section voices a chord under its top note: the shapes an arranger picks from (the big
//! band's sections today, [`super::arranger`]; pure, so the comp can borrow them).
//!
//! **Shapes** ([`Style`]). Four-way close and drop 2 (the swing-era sound, the default); drop
//! 2 and 4 (wider, Basie's saxes); spread (every other chord tone, open like a pianist's two
//! hands); and the ones built from an interval rather than the chord: fourths (quartal, the
//! *So What* sound), stacked fifths with a crunch (a second rubbing at the top, then open
//! fifths: Kenny Barron's two-handed fifths, modern big band writing), clusters (scale steps
//! packed tight, Gil Evans and Thad Jones), an upper-structure triad over the guide tones
//! (polychords on the dominants), and the theme's own intervals stacked down from the top (the
//! voicing grown from the tune's opening, so the harmony rhymes with the melody). An
//! interval-built stack takes each step as near its interval as the chord-scale allows (no
//! avoid notes: a major chord's 4th, a dominant's 11th), and doesn't double a tone while
//! another is free.
//!
//! **Passing notes** ([`Passing`]): a short note off the chord gets a diminished 7th under it
//! (the classic way), the shape it would have had on the nearest chord tone slid there whole
//! (planing: exact transposition, as Berkman's chapter on parallel chords has it), or plain
//! chord tones under it.
//!
//! **Balance** ([`Balance`]): how loud each voice of a section is against the others (each
//! section is one channel, so this is per-note velocity, [`super::voice::Fx::balance`]): the
//! lead on top over shaded inner voices, all even, or a pyramid (the bottom fullest, the
//! classic big band balance that keeps a voicing from sounding top-heavy).

use crate::chart::{Chord, Family, Quality};

/// A section's voicing shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Style {
    /// Trumpets four-way close, saxes drop 2 an octave down (the default sound).
    #[default]
    Drop2,
    /// Close with the 2nd and 4th voices down an octave.
    Drop24,
    /// Every other chord tone down from the top.
    Spread,
    /// Stacked fourths.
    Quartal,
    /// A second at the top, then stacked fifths.
    Fifths,
    /// Scale steps packed tight.
    Cluster,
    /// A triad from the chord's upper tensions over its 3rd and 7th.
    Upper,
    /// The tune's opening intervals stacked down from the top.
    Theme,
}

impl Style {
    pub const ALL: [Style; 8] = [Style::Drop2, Style::Drop24, Style::Spread, Style::Quartal, Style::Fifths, Style::Cluster, Style::Upper, Style::Theme];

    pub fn slug(self) -> &'static str {
        match self {
            Style::Drop2 => "drop2",
            Style::Drop24 => "drop24",
            Style::Spread => "spread",
            Style::Quartal => "fourths",
            Style::Fifths => "fifths",
            Style::Cluster => "cluster",
            Style::Upper => "upper",
            Style::Theme => "theme",
        }
    }

    pub fn parse(s: &str) -> Option<Style> {
        Style::ALL.into_iter().find(|x| x.slug() == s)
    }
}

/// What goes under a short note off the chord.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Passing {
    /// A diminished 7th built down from it.
    #[default]
    Dim,
    /// The shape on the nearest chord tone, slid there whole.
    Planing,
    /// The chord's own tones.
    Plain,
    /// Any chord that doesn't clash with it (a major 7th, minor 7th, dominant, half
    /// diminished, diminished, a stack of fourths, minor b6 or augmented major 7th, the
    /// passing note on any of its tones;
    /// nothing a half step or a minor 9th under it): the funkier the higher the freedom.
    Free,
    /// Barry Harris's sixth-diminished: every note of the line its own block chord, the
    /// chord's sixth (a major or minor 6th, a dominant's 7th) on its tones and the diminished
    /// 7th a half step under the root on the notes between, now and then a voice of the
    /// diminished borrowed a half step down (a 7b9 colour), more often the freer the band.
    Harris,
    /// The next chord's tones under it, early (a free anticipation: the harmony leaning
    /// where it's going); the diminished without a next chord.
    Lean,
}

impl Passing {
    pub const ALL: [Passing; 6] = [Passing::Dim, Passing::Planing, Passing::Plain, Passing::Free, Passing::Harris, Passing::Lean];

    pub fn slug(self) -> &'static str {
        match self {
            Passing::Dim => "dim",
            Passing::Planing => "planing",
            Passing::Plain => "plain",
            Passing::Free => "free",
            Passing::Harris => "harris",
            Passing::Lean => "lean",
        }
    }

    pub fn parse(s: &str) -> Option<Passing> {
        Passing::ALL.into_iter().find(|x| x.slug() == s)
    }
}

/// How loud a section's voices are against each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Balance {
    /// The top a little over the rest (as the sections always played).
    #[default]
    Top,
    /// The lead well over shaded inner voices.
    Lead,
    /// All alike.
    Even,
    /// The bottom fullest, thinning to the top.
    Pyramid,
}

impl Balance {
    pub const ALL: [Balance; 4] = [Balance::Top, Balance::Lead, Balance::Even, Balance::Pyramid];

    pub fn slug(self) -> &'static str {
        match self {
            Balance::Top => "top",
            Balance::Lead => "lead",
            Balance::Even => "even",
            Balance::Pyramid => "pyramid",
        }
    }

    pub fn parse(s: &str) -> Option<Balance> {
        Balance::ALL.into_iter().find(|x| x.slug() == s)
    }

    /// The velocity offset of the voice `rank` from the top (0) of `n`.
    pub fn offset(self, rank: usize, n: usize) -> i32 {
        let bottom = n.saturating_sub(1);
        match self {
            Balance::Top => if rank == 0 { 6 } else { 0 },
            Balance::Lead => if rank == 0 { 12 } else if rank == bottom { 0 } else { -6 },
            Balance::Even => 0,
            Balance::Pyramid => {
                // Top -4 up to bottom +8, evenly.
                (12 * rank).checked_div(bottom).map_or(0, |x| x as i32 - 4)
            }
        }
    }

    /// How much softer the saxes play than the brass in a tutti (dynamic steps).
    pub fn saxes_under(self) -> u8 {
        match self {
            Balance::Top => 1,
            Balance::Lead => 2,
            Balance::Even => 1,
            Balance::Pyramid => 0,
        }
    }
}

/// A voicing choice: the shape, passing chords, balance, and the theme's intervals (for
/// [`Style::Theme`]: downward steps in semitones).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Voicing {
    pub style: Style,
    pub passing: Passing,
    pub balance: Balance,
    pub theme: [i32; 3],
    /// Seeds the free passing chords' choice (with the note).
    pub salt: u64,
    /// How many voices a section plays (2 to 4; 0 reads as 4): [`finish`] thins to it.
    pub voices: u8,
    /// How often a [`Passing::Harris`] diminished borrows a note (0 to 1).
    pub borrow: f32,
    /// The chord the line is heading for (set note by note; [`Passing::Lean`] and the
    /// Harris diminished's borrowing lean on it).
    pub next: Option<Chord>,
}

/// The overrides a page or test can force (`None`: the arranger rolls).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Force {
    pub style: Option<Style>,
    pub passing: Option<Passing>,
    pub balance: Option<Balance>,
    pub voices: Option<u8>,
}

/// The chord's four tones (pitch classes; a sixth on a triad).
pub fn chord_tones(c: Chord) -> [i32; 4] {
    super::horns::four(c)
}

/// Pitch classes a stack may use: the chord tones and its available tensions (no avoid
/// notes), chord tones first.
pub fn available(c: Chord) -> Vec<i32> {
    let r = c.root as i32;
    let mut v: Vec<i32> = chord_tones(c).to_vec();
    let tensions: &[i32] = match c.quality {
        Quality::Major | Quality::Six | Quality::Maj7 => &[2, 9],
        Quality::Dom7 | Quality::Dom9 => &[2, 9],
        Quality::Sus4 => &[2, 5, 9],
        Quality::Dom7b9 => &[1, 3, 9],
        Quality::Dom7s9 => &[1, 3, 8],
        Quality::Dom7s5 => &[1, 3, 8],
        Quality::Minor | Quality::Minor7 | Quality::Minor6 => &[2, 5],
        Quality::MinMaj7 => &[2, 5, 9],
        Quality::HalfDim => &[5, 8],
        Quality::Dim7 => &[2, 5, 8, 11],
        Quality::Aug => &[2, 6],
    };
    for t in tensions {
        let pc = (r + t) % 12;
        if !v.contains(&pc) {
            v.push(pc);
        }
    }
    v
}

fn pc(n: i32) -> i32 {
    n.rem_euclid(12)
}

/// `n` voices (top first) under `top` stepping down by `cell` (cycled), each step as near its
/// interval as `ok` allows (within two semitones), not doubling a pitch class while another
/// fits. Steps of a second may land a half or whole step down; wider ones keep at least a
/// minor 3rd.
fn stack(top: i32, n: usize, cell: &[i32], ok: &[i32]) -> Vec<i32> {
    let mut v = vec![top];
    let mut prev = top;
    for i in 1..n {
        let step = cell[(i - 1) % cell.len()].max(1);
        let ideal = prev - step;
        let min_gap = if step <= 2 { 1 } else { 3 };
        // (A second prefers to stay close; a wider step to open out.)
        let order: [i32; 5] = if step <= 2 { [0, 1, -1, 2, -2] } else { [0, -1, 1, -2, 2] };
        let cands: Vec<i32> = order.iter().map(|d| ideal + d).filter(|&x| prev - x >= min_gap && ok.contains(&pc(x))).collect();
        let fresh = cands.iter().copied().find(|x| !v.iter().any(|y| pc(*y) == pc(*x)));
        let x = fresh.or(cands.first().copied()).unwrap_or(ideal);
        v.push(x);
        prev = x;
    }
    v
}

/// Four voices (top first) under `top` for `c` in `style`; `passing` when `top` is a short
/// note off the chord ([`Passing`] decides what then).
pub fn four_voices(v: &Voicing, top: i32, c: Chord, passing: bool) -> [i32; 4] {
    if v.passing == Passing::Harris
        && let Some(x) = harris(v, top, c)
    {
        return x;
    }
    let avail = available(c);
    if passing && !avail.contains(&pc(top)) {
        match v.passing {
            Passing::Dim => return [top, top - 3, top - 6, top - 9],
            Passing::Planing => {
                // The nearest chord tone's shape, slid onto the passing note.
                let tones = chord_tones(c);
                if let Some(t) = [1, -1, 2, -2].iter().map(|d| top + d).find(|x| tones.contains(&pc(*x))) {
                    return shape(v, t, c).map(|x| x + top - t);
                }
            }
            Passing::Plain => {}
            Passing::Free => return free_passing(v.salt, top),
            Passing::Harris => {}
            Passing::Lean => return lean(v, top),
        }
    }
    shape(v, top, c)
}

/// Barry Harris's sixth-diminished block chord under `top` (close, top first), `None` on a
/// chord the system doesn't cover (half diminished, diminished, augmented).
pub fn harris(v: &Voicing, top: i32, c: Chord) -> Option<[i32; 4]> {
    let r = c.root as i32;
    let sixth: [i32; 4] = match c.family() {
        Family::Major => [0, 4, 7, 9],
        Family::Minor => [0, 3, 7, 9],
        Family::Dominant => [0, 4, 7, 10],
        _ => return None,
    };
    let six: Vec<i32> = sixth.iter().map(|i| (r + i) % 12).collect();
    if six.contains(&pc(top)) {
        // The sixth chord's inversion with the note on top.
        let below: Vec<i32> = (top - 12..top).rev().filter(|n| six.contains(&pc(*n))).take(3).collect();
        return Some([top, below[0], below[1], below[2]]);
    }
    // The diminished between: on its own notes (a half step under the root's), or on the
    // others (a borrowed diminished: the line's chromatic notes).
    let mut d = [top, top - 3, top - 6, top - 9];
    let k = (v.salt ^ (top as u64).wrapping_mul(0xA24B_AED4_963E_E407)).wrapping_mul(0x9FB2_1C65_1E98_DF25) >> 40;
    if (k % 1000) as f32 / 1000.0 < v.borrow {
        // One voice borrowed a half step from the next diminished: onto a tone of the chord
        // the line is heading for if one's a half step off (a free suspension, resolving
        // early), else down (a dominant 7b9).
        let ahead = v.next.map(chord_tones);
        let toward = ahead.and_then(|t| (1..4).flat_map(|i| [(i, -1), (i, 1)]).find(|&(i, s)| t.contains(&pc(d[i] + s)) && (top - (d[i] + s)).rem_euclid(12) != 1 && d[i] + s < top));
        match toward {
            Some((i, s)) => d[i] += s,
            None => d[1 + (k % 3) as usize] -= 1,
        }
        d[1..].sort_by(|a, b| b.cmp(a));
    }
    Some(d)
}

/// [`Passing::Lean`]: the next chord's tones close under `top`, nothing a half step or a
/// minor 9th under it (the diminished if there's no next chord or nothing fits).
pub fn lean(v: &Voicing, top: i32) -> [i32; 4] {
    let dim = [top, top - 3, top - 6, top - 9];
    let Some(n) = v.next else { return dim };
    let ok = available(n);
    let below: Vec<i32> = (top - 12..top).rev().filter(|x| ok.contains(&pc(*x)) && (top - x) % 12 != 1 && top - x > 1).collect();
    // Prefer the next chord's own tones, spaced: every other one if there's room.
    let tones = chord_tones(n);
    let core: Vec<i32> = below.iter().copied().filter(|x| tones.contains(&pc(*x))).take(3).collect();
    if core.len() == 3 {
        return [top, core[0], core[1], core[2]];
    }
    if below.len() >= 3 { [top, below[0], below[1], below[2]] } else { dim }
}

/// The free passing chords' shapes, intervals above their root.
/// (Major 7th, minor 7th, dominant, half diminished, diminished, fourths, minor b6,
/// augmented major 7th.)
const PASSING_SHAPES: [[i32; 4]; 8] = [[0, 4, 7, 11], [0, 3, 7, 10], [0, 4, 7, 10], [0, 3, 6, 10], [0, 3, 6, 9], [0, 5, 10, 15], [0, 3, 7, 8], [0, 4, 8, 11]];

/// A passing chord of any quality under `top` that doesn't clash with it, close under the top
/// (the top on any of its tones), picked by `salt` and the note.
pub fn free_passing(salt: u64, top: i32) -> [i32; 4] {
    let mut all: Vec<[i32; 4]> = Vec::new();
    for sh in PASSING_SHAPES {
        for &on in &sh {
            // The chord's tones under the top, close.
            let root = top - on;
            let pcs: Vec<i32> = sh.iter().map(|i| pc(root + i)).collect();
            let below: Vec<i32> = (top - 12..top).rev().filter(|n| pcs.contains(&pc(*n))).take(3).collect();
            if below.len() < 3 || below.iter().any(|n| (top - n) % 12 == 1) {
                continue;
            }
            all.push([top, below[0], below[1], below[2]]);
        }
    }
    let k = (salt ^ (top as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) >> 33;
    all.get(k as usize % all.len().max(1)).copied().unwrap_or([top, top - 3, top - 6, top - 9])
}

/// [`four_voices`] off the passing-note question.
fn shape(v: &Voicing, top: i32, c: Chord) -> [i32; 4] {
    use super::horns::{close, drop2};
    let avail = available(c);
    let tones = chord_tones(c).to_vec();
    let four = |s: Vec<i32>| [s[0], s[1], s[2], s[3]];
    match v.style {
        Style::Drop2 => drop2(close(top, c, false)),
        Style::Drop24 => {
            let k = close(top, c, false);
            let mut d = [k[0], k[2], k[1] - 12, k[3] - 12];
            d[1..].sort_by(|a, b| b.cmp(a));
            d
        }
        Style::Spread => {
            // Every other tone of the close stack below (chord tones, the top's tension kept).
            let below: Vec<i32> = (top - 36..top).rev().filter(|n| tones.contains(&pc(*n)) && top - n > 1).collect();
            four(vec![top, below[1], below[3], below[5]])
        }
        Style::Quartal => four(stack(top, 4, &[5], &avail)),
        Style::Fifths => four(stack(top, 4, &[2, 7, 7], &avail)),
        Style::Cluster => four(stack(top, 4, &[2, 1, 2], &avail)),
        Style::Theme => four(stack(top, 4, &v.theme, &avail)),
        Style::Upper => {
            let r = c.root as i32;
            // The triad over the guide tones: a dominant's on the 9th (9 #11 13; on an
            // altered one the b6's: b13 R #9), a major chord's on the 9th (lydian), a minor
            // chord's on the 4th (11 13 R) or the 7th (b7 9 11).
            let triads: Vec<[i32; 3]> = match c.family() {
                Family::Dominant if matches!(c.quality, Quality::Dom7b9 | Quality::Dom7s9 | Quality::Dom7s5) => vec![[8, 0, 3], [6, 10, 1]],
                Family::Dominant => vec![[2, 6, 9], [9, 1, 4]],
                Family::Major => vec![[2, 6, 9], [7, 11, 2]],
                Family::Minor => vec![[5, 9, 0], [10, 2, 5]],
                _ => vec![],
            };
            let found = triads.iter().map(|t| t.map(|i| (r + i) % 12)).find(|t| t.contains(&pc(top)));
            match found {
                Some(t) => {
                    let mut s = stack(top, 3, &[3], &t);
                    let third = tones[1];
                    let seventh = tones[3];
                    // One guide tone under the triad (the 3rd or the 7th, whichever is nearer).
                    let low = *s.last().unwrap();
                    let under = |p: i32| low - 1 - (low - 1 - p).rem_euclid(12);
                    let g = under(third).max(under(seventh));
                    s.push(g);
                    four(s)
                }
                None => four(stack(top, 4, &[2, 7, 7], &avail)),
            }
        }
    }
}

/// The trumpets' four voices in a tutti: close for [`Style::Drop2`] (the saxes have the drop
/// 2), the style's shape otherwise.
pub fn brass(v: &Voicing, top: i32, c: Chord, passing: bool) -> [i32; 4] {
    if v.style == Style::Drop2 {
        return close_or_passing(v, top, c, passing);
    }
    four_voices(v, top, c, passing)
}

/// The saxes' four voices under `top` (drop 2 off the close stack for [`Style::Drop2`]).
pub fn saxes(v: &Voicing, top: i32, c: Chord, passing: bool) -> [i32; 4] {
    if v.style == Style::Drop2 {
        return super::horns::drop2(close_or_passing(v, top, c, passing));
    }
    four_voices(v, top, c, passing)
}

/// Close, its passing note handled by `v`.
fn close_or_passing(v: &Voicing, top: i32, c: Chord, passing: bool) -> [i32; 4] {
    use super::horns::close;
    match v.passing {
        Passing::Dim => close(top, c, passing),
        Passing::Plain => close(top, c, false),
        Passing::Planing => {
            let tones = chord_tones(c);
            if passing
                && !tones.contains(&pc(top))
                && let Some(t) = [1, -1, 2, -2].iter().map(|d| top + d).find(|x| tones.contains(&pc(*x)))
            {
                return close(t, c, false).map(|x| x + top - t);
            }
            close(top, c, false)
        }
        Passing::Free if passing && !chord_tones(c).contains(&pc(top)) => free_passing(v.salt, top),
        Passing::Free => close(top, c, false),
        Passing::Harris => harris(v, top, c).unwrap_or_else(|| close(top, c, passing)),
        Passing::Lean if passing && !chord_tones(c).contains(&pc(top)) => lean(v, top),
        Passing::Lean => close(top, c, false),
    }
}

/// A section's chord made playable (top first): nothing a half step or a minor 9th under the
/// top (it unsettles the lead's pitch: Fratina on lead trumpet), no seconds down low (they
/// mud: the crunch belongs at the top, Busby), then thinned to `v.voices` (dropping a doubled
/// tone first, then the root, then the 5th, never the top or both guide tones): a section
/// doesn't fill every gap.
pub fn finish(v: &Voicing, x: [i32; 4], c: Chord) -> Vec<i32> {
    let top = x[0];
    let mut out: Vec<i32> = vec![top];
    for &n in &x[1..] {
        if (top - n).rem_euclid(12) == 1 || out.contains(&n) {
            continue;
        }
        out.push(n);
    }
    // Low seconds: the lower of two voices a second apart under F3 goes.
    let mut i = out.len();
    while i > 2 {
        i -= 1;
        if out[i] < 55 && out[i - 1] - out[i] <= 2 {
            out.remove(i);
        }
    }
    let want = if v.voices == 0 { 4 } else { v.voices.clamp(1, 4) as usize };
    let r = c.root as i32;
    let tones = chord_tones(c);
    let fifth = (r + 7) % 12;
    while out.len() > want {
        let doubled = (1..out.len()).rev().find(|&k| out[..k].iter().any(|y| pc(*y) == pc(out[k])));
        let root = (1..out.len()).rev().find(|&k| pc(out[k]) == r);
        let five = (1..out.len()).rev().find(|&k| pc(out[k]) == fifth && tones.contains(&fifth));
        // Otherwise an inner voice that isn't a guide tone, else the lowest.
        let guide = |n: i32| pc(n) == tones[1] || pc(n) == tones[3];
        let inner = (1..out.len() - 1).find(|&k| !guide(out[k]));
        let k = doubled.or(root).or(five).or(inner).unwrap_or(out.len() - 1);
        out.remove(k);
    }
    out
}

/// The downward cell of a theme: its first three leaps (a repeated note skipped; sizes folded
/// into an octave, at least a whole step: a theme of half steps would stack into a smear).
pub fn theme_cell(notes: &[i32]) -> [i32; 3] {
    let mut cell = Vec::new();
    for w in notes.windows(2) {
        let d = (w[1] - w[0]).abs() % 12;
        if d == 0 {
            continue;
        }
        cell.push(d.max(2));
        if cell.len() == 3 {
            break;
        }
    }
    while cell.len() < 3 {
        cell.push(*cell.last().unwrap_or(&5));
    }
    // A stack that would sit inside a fifth reads as a cluster: open its widest step.
    if cell.iter().sum::<i32>() < 7 {
        cell[0] += 5;
    }
    [cell[0], cell[1], cell[2]]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chord(s: &str) -> Chord {
        crate::chart::parse_chord(s).unwrap()
    }

    fn v(style: Style) -> Voicing {
        Voicing { style, theme: [5, 3, 4], ..Voicing::default() }
    }

    #[test]
    fn every_style_is_four_descending_voices_with_no_avoid_notes() {
        for s in Style::ALL {
            for (name, top) in [("C7", 79), ("Cmaj7", 76), ("Dm7", 77), ("G7", 81), ("F6", 81), ("Bb7", 77)] {
                let c = chord(name);
                let x = four_voices(&v(s), top, c, false);
                assert_eq!(x[0], top, "{s:?} {name}: {x:?}");
                assert!(x.windows(2).all(|w| w[0] > w[1]), "{s:?} {name}: not descending {x:?}");
                if !matches!(s, Style::Drop2 | Style::Drop24 | Style::Spread | Style::Upper) {
                    let ok = available(c);
                    for n in &x[1..] {
                        assert!(ok.contains(&n.rem_euclid(12)), "{s:?} {name}: {n} not available in {x:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn the_interval_shapes() {
        // C7 under a G: fourths G D A E (A the 13th, E the 3rd).
        assert_eq!(four_voices(&v(Style::Quartal), 79, chord("C7"), false), [79, 74, 69, 64]);
        // Cmaj7 under a D: the crunch (C), then a fifth down (the F an avoid note: E), A.
        assert_eq!(four_voices(&v(Style::Fifths), 74, chord("Cmaj7"), false), [74, 72, 64, 57]);
        // Dm7 under a D: D C, then fifths: F and A.
        assert_eq!(four_voices(&v(Style::Fifths), 74, chord("Dm7"), false), [74, 72, 65, 57]);
        // A cluster stays inside a fourth.
        let k = four_voices(&v(Style::Cluster), 76, chord("Cmaj7"), false);
        assert!(k[0] - k[3] <= 6, "{k:?}");
        // The upper structure on C7 under an A: D F# A over the Bb.
        assert_eq!(four_voices(&v(Style::Upper), 81, chord("C7"), false), [81, 78, 74, 70]);
    }

    #[test]
    fn passing_notes() {
        let c = chord("C7");
        let dim = Voicing { passing: Passing::Dim, ..v(Style::Quartal) };
        assert_eq!(four_voices(&dim, 78, c, true), [78, 75, 72, 69]);
        // Planing: G's fourths slid down to the F#.
        let plane = Voicing { passing: Passing::Planing, ..v(Style::Quartal) };
        assert_eq!(four_voices(&plane, 78, c, true), [78, 73, 68, 63]);
        // Free: never a half step or a minor 9th under the note.
        for salt in 0..40 {
            let f = four_voices(&Voicing { passing: Passing::Free, salt, ..v(Style::Quartal) }, 78, c, true);
            assert!(f[1..].iter().all(|n| (78 - n) % 12 != 1 && *n < 78), "{f:?}");
        }
    }

    #[test]
    fn finishing_keeps_the_lead_clear_and_thins() {
        let c = chord("C7");
        // A half step under the top goes.
        assert_eq!(finish(&Voicing::default(), [76, 75, 72, 70], c), vec![76, 72, 70]);
        // Thinned to two: the root and the 5th go, the guide tone stays.
        let two = Voicing { voices: 2, ..Voicing::default() };
        assert_eq!(finish(&two, [79, 76, 72, 67], c), vec![79, 76]);
        // A low second goes.
        assert_eq!(finish(&Voicing::default(), [62, 58, 52, 50], c), vec![62, 58, 52]);
    }

    #[test]
    fn harris_sixth_diminished() {
        let h = Voicing { passing: Passing::Harris, ..Voicing::default() };
        let c = chord("C6");
        // C6 under an E: E C A G.
        assert_eq!(four_voices(&h, 76, c, false), [76, 72, 69, 67]);
        // Under the D: the B diminished.
        assert_eq!(four_voices(&h, 74, c, true), [74, 71, 68, 65]);
        // Borrowing always: one voice a half step down, a dominant 7b9.
        let b = Voicing { borrow: 1.0, ..h };
        let x = four_voices(&b, 74, c, true);
        assert_eq!(x.iter().zip([74, 71, 68, 65]).filter(|(a, b)| *a != b).count(), 1, "{x:?}");
        // Drop 2 of it for the saxes.
        assert_eq!(saxes(&h, 76, c, false), [76, 69, 67, 60]);
    }

    #[test]
    fn leaning_on_the_next_chord() {
        // An F# passing over C7 into Fmaj7: Fmaj7's tones under it (E C A? the E is a
        // half step... no: under F# the F is the half step, so E C A).
        let l = Voicing { passing: Passing::Lean, next: Some(chord("Fmaj7")), ..Voicing::default() };
        assert_eq!(four_voices(&l, 78, chord("C7"), true), [78, 76, 72, 69]);
        // Harris borrowing toward the next chord: B dim's G# up to A (F6's 3rd).
        let h = Voicing { passing: Passing::Harris, borrow: 1.0, next: Some(chord("F6")), ..Voicing::default() };
        let x = four_voices(&h, 74, chord("C6"), true);
        let f6 = chord_tones(chord("F6"));
        assert!(x[1..].iter().any(|n| f6.contains(&n.rem_euclid(12))), "{x:?}");
    }

    #[test]
    fn balance_shapes() {
        assert!(Balance::Lead.offset(0, 4) > Balance::Lead.offset(1, 4));
        assert!(Balance::Pyramid.offset(3, 4) > Balance::Pyramid.offset(0, 4));
        assert_eq!(Balance::Even.offset(2, 4), 0);
    }

    #[test]
    fn theme_cells() {
        assert_eq!(theme_cell(&[60, 64, 67, 72]), [4, 3, 5]);
        assert_eq!(theme_cell(&[60, 60, 62, 63]), [7, 2, 2]);
    }
}
