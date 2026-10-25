//! The walking bass's players ([`Walker`]): how the bassist behind each soloist walks, as text
//! (`music/bass/walking.bass`, the format at its top), measured in the Weimar Jazz Database
//! (the bass's pitch on each beat under its swing solos): how often they repeat a note or sit
//! on a pedal, half steps against leaps, what they land on at a chord change and how they walk
//! into it, how chromatic they are inside a chord. Paul Chambers walks behind Miles, Jimmy
//! Garrison behind Trane, Ron Carter behind Wayne, Tommy Potter behind Bird, Doug Watkins
//! behind Sonny, Ray Brown behind the band's own line; Armstrong's records have no walking bass
//! to measure, so Pops gets ours ([`Soloist::bassist`]). Parsed once, off the audio thread.

use std::sync::OnceLock;

use super::soloist::Soloist;

/// One bassist's walking, in shares of beats (each list sums to 1).
#[derive(Debug, Clone, PartialEq)]
pub struct Walker {
    pub slug: String,
    pub name: String,
    /// Beat to beat: the same note, a half step, a whole step, a 3rd, a 4th or 5th, wider.
    pub steps: [f32; 6],
    /// On a chord change: the root, the 3rd, the 5th, another note.
    pub lands: [f32; 4],
    /// The beat before a change: a half step below the next root, a half step above, a whole
    /// step, its 5th, the same note again, another note.
    pub approach: [f32; 6],
    /// Within a chord: chord tones, scale tones, chromatic notes.
    pub inside: [f32; 3],
    /// Beats in a pedal point (four or more on one pitch class).
    pub pedal: f32,
    /// The beat before a change already on the new root (an anticipation), the change's beat
    /// still on the note before (a suspension), bars in two (half notes): shares.
    pub push: f32,
    pub hold: f32,
    pub two: f32,
    /// Rhythm (from a note-level pass on the bassist's records, where measured): beats with a
    /// skip note at a medium tempo (fewer when fast), and where in the beat it falls (`None`:
    /// on the tune's swung 8th); beats raked in triplets; notes ghosted.
    pub skip: f32,
    pub skip_at: Option<f32>,
    pub rake: f32,
    pub ghost: f32,
    /// Bars of paired quarters (a two-feel struck in four: beat 2 the note of 1, 4 of 3), and
    /// bars paired in their first half only.
    pub pair: [f32; 2],
}

impl Walker {
    /// The root on a chord change: the measured root (and part of the "other" notes, which on
    /// automatic pitch estimates are mostly the root misheard) read against the whole
    /// database's, around a walker who lands on the root four times in five.
    pub fn root_share(&self) -> f32 {
        let base = &all()[0];
        let own = |w: &Walker| w.lands[0] + 0.6 * w.lands[3];
        (0.8 + own(self) - own(base)).clamp(0.6, 0.95)
    }

    pub fn parse(line: &str) -> Result<Walker, String> {
        let (slug, v) = line.split_once('=').ok_or("expected `slug = Name | steps .. | lands .. | approach .. | inside .. | pedal ..`")?;
        let mut parts = v.split('|').map(str::trim);
        let name = parts.next().filter(|n| !n.is_empty()).ok_or("a name first")?;
        let mut w = Walker { slug: slug.trim().into(), name: name.into(), steps: [0.0; 6], lands: [0.0; 4], approach: [0.0; 6], inside: [0.0; 3], pedal: -1.0, push: -1.0, hold: -1.0, two: -1.0, skip: 0.12, skip_at: None, rake: 0.0, ghost: 0.0, pair: [0.0; 2] };
        for part in parts {
            let (key, nums) = part.split_once(' ').ok_or_else(|| format!("`{part}`: a field and its numbers"))?;
            let nums: Vec<f32> = nums.split_whitespace().map(|x| x.parse::<f32>().map_err(|_| format!("`{x}`: not a number"))).collect::<Result<_, _>>()?;
            if nums.iter().any(|x| !x.is_finite() || *x < 0.0) {
                return Err(format!("`{part}`: percentages are 0 or more"));
            }
            let fill = |dst: &mut [f32]| {
                if nums.len() != dst.len() {
                    return Err(format!("`{key}`: {} numbers", dst.len()));
                }
                let sum: f32 = nums.iter().sum();
                if sum <= 0.0 {
                    return Err(format!("`{key}`: all zero"));
                }
                for (d, x) in dst.iter_mut().zip(&nums) {
                    *d = x / sum;
                }
                Ok(())
            };
            match key {
                "steps" => fill(&mut w.steps)?,
                "lands" => fill(&mut w.lands)?,
                "approach" => fill(&mut w.approach)?,
                "inside" => fill(&mut w.inside)?,
                "pedal" | "push" | "hold" | "two" if nums.len() == 1 && nums[0] <= 100.0 => {
                    let x = nums[0] / 100.0;
                    *match key {
                        "pedal" => &mut w.pedal,
                        "push" => &mut w.push,
                        "hold" => &mut w.hold,
                        _ => &mut w.two,
                    } = x;
                }
                "pedal" | "push" | "hold" | "two" => return Err(format!("`{key}`: one percentage")),
                "skip" if nums.len() == 2 && nums[0] <= 100.0 && (0.5..1.0).contains(&nums[1]) => (w.skip, w.skip_at) = (nums[0] / 100.0, Some(nums[1])),
                "skip" => return Err("`skip`: a percentage, then where in the beat (0.5..1)".into()),
                "rake" | "ghost" if nums.len() == 1 && nums[0] <= 100.0 => *(if key == "rake" { &mut w.rake } else { &mut w.ghost }) = nums[0] / 100.0,
                "rake" | "ghost" => return Err(format!("`{key}`: one percentage")),
                "pair" if nums.len() == 2 && nums[0] + nums[1] <= 100.0 => w.pair = [nums[0] / 100.0, nums[1] / 100.0],
                "pair" => return Err("`pair`: bars paired, then bars half paired (%)".into()),
                _ => return Err(format!("`{key}`: steps, lands, approach, inside, pedal, push, hold, two, skip, rake, ghost or pair")),
            }
        }
        if w.steps[1] + w.steps[2] == 0.0 || w.lands[0] == 0.0 || w.inside[0] == 0.0 || [w.pedal, w.push, w.hold, w.two].iter().any(|x| *x < 0.0) {
            return Err(format!("{}: every field (steps, lands, approach, inside, pedal, push, hold, two)", w.slug));
        }
        Ok(w)
    }
}

/// Every walker in `music/bass/walking.bass`, the whole database's first.
pub fn all() -> &'static [Walker] {
    static WALKERS: OnceLock<Vec<Walker>> = OnceLock::new();
    WALKERS.get_or_init(|| {
        let all: Vec<Walker> = include_str!("../../../music/bass/walking.bass")
            .lines()
            .enumerate()
            .filter_map(|(i, l)| {
                let l = l.split_once(';').map_or(l, |(a, _)| a).trim();
                (!l.is_empty()).then(|| Walker::parse(l).unwrap_or_else(|e| panic!("music/bass/walking.bass line {}: {e}", i + 1)))
            })
            .collect();
        assert!(all.first().is_some_and(|w| w.slug == "all"), "music/bass/walking.bass: the whole database (`all`) first");
        all
    })
}

/// The walker called `slug`.
pub fn by(slug: &str) -> Option<&'static Walker> {
    all().iter().find(|w| w.slug == slug)
}

impl Soloist {
    /// Who walks behind them: the bassist on their measured records.
    pub fn bassist(self) -> &'static Walker {
        let slug = match self {
            Soloist::Miles => "chambers",
            Soloist::Trane => "garrison",
            Soloist::Wayne => "carter",
            Soloist::Bird => "potter",
            Soloist::Sonny => "watkins",
            Soloist::Pops => "foster",
            Soloist::Band | Soloist::Funk | Soloist::Latin => "brown",
        };
        by(slug).expect("a walker for every soloist")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_walker_parses_and_each_soloist_has_one() {
        assert!(all().len() >= 8);
        for s in Soloist::ALL {
            let w = s.bassist();
            assert!((0.6..=0.95).contains(&w.root_share()), "{}", w.name);
        }
        // What sets them apart, as measured.
        let (pc, jg, dw) = (by("chambers").unwrap(), by("garrison").unwrap(), by("watkins").unwrap());
        assert!(jg.pedal > 5.0 * pc.pedal && jg.steps[0] > 2.0 * pc.steps[0], "Garrison sits on pedals, Chambers hardly repeats");
        assert!(pc.steps[1] > jg.steps[1] && dw.steps[4] > pc.steps[4], "Chambers walks half steps, Watkins leaps");
        assert!(by("heath").unwrap().root_share() > pc.root_share());
        assert!(jg.push > 2.0 * pc.push && jg.hold > 4.0 * dw.hold, "Garrison blurs the bar line, the hard-boppers land on it");
        assert!(by("foster").unwrap().two > 4.0 * pc.two, "Pops' bass in two");
        assert!(by("bryant").unwrap().pair[0] > 0.1 && pc.pair == [0.0; 2], "Bryant pairs his quarters");
        assert!(pc.skip_at.is_some_and(|x| (0.6..0.7).contains(&x)) && pc.ghost > 0.0, "Chambers' triplet skips, measured");
    }

    #[test]
    fn a_walker_reads_its_fields() {
        let w = Walker::parse("x = X | steps 1 1 1 1 1 5 | lands 2 1 1 0 | approach 1 1 1 1 0 0 | inside 2 1 1 | pedal 10 | push 5 | hold 2 | two 50").unwrap();
        assert_eq!((w.name.as_str(), w.steps[5], w.lands[0], w.approach[3], w.inside[0], w.pedal), ("X", 0.5, 0.5, 0.25, 0.5, 0.1));
        assert_eq!((w.push, w.hold, w.two), (0.05, 0.02, 0.5));
        assert!(Walker::parse("x = X | steps 1 1 | lands 1 1 1 1").is_err());
    }
}
