//! The funk bass's riffs: two-bar lines written as text (`music/bass/funk.riffs`, the format
//! at its top), ours, to rhyme with the shapes of real funk lines (measured on about a
//! thousand funk, disco and soul bass parts in the Lakh MIDI Dataset: ~6 notes a bar, the ONE
//! held, the "and" of 2 and beat 3 the backbone, pickups over beats 3-4, low 5ths and b7ths,
//! octave pops on the "and"s, dead notes, bar 2 answering bar 1, the bass doubling ~3/4 of the
//! kicks). A feel section keeps one riff (it grooves); the kick plays along with it
//! ([`Riff::kicks`]). Parsed once, off the audio thread.

use std::sync::OnceLock;

use super::soloist::Fits;
use crate::audio::chart::Family;

/// One note of a riff bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Step {
    /// Its 16th in the bar, and how many 16ths it's written to last.
    pub at: u8,
    pub len: u8,
    /// Semitones from the chord's root (an octave up: a pop).
    pub deg: i8,
    /// A dead note (muted: a click with the pitch before).
    pub dead: bool,
}

/// A two-bar riff.
#[derive(Debug, Clone, Copy)]
pub struct Riff {
    pub fits: Fits,
    steps: [[Step; 16]; 2],
    n: [u8; 2],
}

impl Riff {
    /// Bar `side`'s notes (0 or 1).
    pub fn bar(&self, side: usize) -> &[Step] {
        &self.steps[side][..self.n[side] as usize]
    }

    /// The 16ths the kick doubles in bar `side`: the riff's notes on the beats and the "and"s
    /// (not the dead notes or the pops, not the backbeat's 2 and 4), and always the ONE.
    pub fn kicks(&self, side: usize) -> impl Iterator<Item = u8> + '_ {
        self.bar(side).iter().filter(|s| !s.dead && s.deg < 12 && s.at % 2 == 0 && s.at != 4 && s.at != 12).map(|s| s.at).chain((self.bar(side).first().is_none_or(|s| s.at != 0)).then_some(0))
    }

    pub fn parse(line: &str) -> Result<Riff, String> {
        let (fits, v) = line.split_once('=').ok_or("expected `fits = <bar 1> | <bar 2>`")?;
        let fits = Fits::parse(fits.trim()).ok_or_else(|| format!("`{}`: dom, min, maj, half or any", fits.trim()))?;
        let bars: Vec<&str> = v.split('|').collect();
        if bars.len() != 2 {
            return Err("two bars, split by `|`".into());
        }
        let mut riff = Riff { fits, steps: [[Step::default(); 16]; 2], n: [0; 2] };
        for (side, bar) in bars.iter().enumerate() {
            let mut words = bar.split_whitespace();
            let rhythm = words.next().ok_or("a bar is a rhythm, then its notes")?;
            if rhythm.len() != 16 || !rhythm.chars().all(|c| "x-d.".contains(c)) {
                return Err(format!("`{rhythm}`: 16 steps of x - d ."));
            }
            let mut prev: Option<i8> = None;
            for (k, c) in rhythm.bytes().enumerate() {
                let len = 1 + rhythm.as_bytes()[k + 1..].iter().take_while(|&&b| b == b'-').count() as u8;
                let step = match c {
                    b'x' => {
                        let w = words.next().ok_or_else(|| format!("`{rhythm}`: more notes than degrees"))?;
                        let deg = degree(w).ok_or_else(|| format!("`{w}`: a degree is 1 b2 2 b3 3 4 b5 5 b6 6 b7 7, with ' or ,"))?;
                        prev = Some(deg);
                        Step { at: k as u8, len, deg, dead: false }
                    }
                    b'd' => Step { at: k as u8, len: 1, deg: prev.unwrap_or(0), dead: true },
                    _ => continue,
                };
                let n = &mut riff.n[side];
                riff.steps[side][*n as usize] = step;
                *n += 1;
            }
            if let Some(w) = words.next() {
                return Err(format!("`{w}`: more degrees than notes"));
            }
        }
        Ok(riff)
    }
}

fn degree(w: &str) -> Option<i8> {
    let base = w.trim_end_matches(['\'', ',']);
    let oct = 12 * (w.matches('\'').count() as i8 - w.matches(',').count() as i8);
    let d = ["1", "b2", "2", "b3", "3", "4", "b5", "5", "b6", "6", "b7", "7"].iter().position(|x| *x == base)? as i8;
    Some(d + oct)
}

/// The funk riffs (`music/bass/funk.riffs`), parsed once.
pub fn funk() -> &'static [Riff] {
    static RIFFS: OnceLock<Vec<Riff>> = OnceLock::new();
    RIFFS.get_or_init(|| {
        include_str!("../../../music/bass/funk.riffs")
            .lines()
            .enumerate()
            .filter_map(|(i, l)| {
                let l = l.split_once(';').map_or(l, |(a, _)| a).trim();
                (!l.is_empty()).then(|| Riff::parse(l).unwrap_or_else(|e| panic!("music/bass/funk.riffs line {}: {e}", i + 1)))
            })
            .collect()
    })
}

/// The riff a funk section that started at bar `since` plays over a chord of `family`: one
/// picked for the section, or the next one along that fits the chord (so a section keeps its
/// riff, and each kind of chord its own).
pub fn pick(start: usize, family: Family) -> &'static Riff {
    let all = funk();
    (0..all.len()).map(|i| &all[(start + i) % all.len()]).find(|r| r.fits.fits(family)).unwrap_or(&all[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_riff_parses_and_grooves() {
        let all = funk();
        assert!(all.len() >= 20, "{}", all.len());
        for r in all {
            for side in 0..2 {
                let n = r.bar(side).iter().filter(|s| !s.dead).count();
                assert!((2..=10).contains(&n), "{r:?}");
            }
        }
        // Most bars play the ONE; most riffs have a pop or a low note below the root.
        let ones = all.iter().flat_map(|r| [r.bar(0), r.bar(1)]).filter(|b| b.first().is_some_and(|s| s.at == 0)).count();
        assert!(ones as f32 / (2 * all.len()) as f32 > 0.75, "{ones}");
        assert!(all.iter().filter(|r| r.bar(0).iter().chain(r.bar(1)).any(|s| s.deg >= 12 || s.deg < 0)).count() * 10 > all.len() * 8);
    }

    #[test]
    fn a_riff_reads_its_rhythm_and_degrees() {
        let r = Riff::parse("dom = x-..d.x.x..x.xx. 1 1 b7, 1 b3 3 | -...x.x.x...x.x. 1 1' 1 5, 7,").unwrap();
        let b = r.bar(0);
        assert_eq!((b[0].at, b[0].len, b[0].deg), (0, 2, 0));
        assert!(b[1].dead && b[1].at == 4);
        assert_eq!(b[3].deg, -2, "b7 below");
        assert_eq!(r.bar(1)[1].deg, 12, "a pop");
        assert_eq!(r.kicks(0).collect::<Vec<_>>(), vec![0, 6, 8, 14]);
        assert!(r.kicks(1).any(|k| k == 0), "the kick keeps the ONE when the bass rings over it");
        assert!(Riff::parse("dom = x... 1 | x.......x....... 1 5").is_err());
        assert!(Riff::parse("dom = x.......x....... 1 | x.......x....... 1 9").is_err());
    }
}
