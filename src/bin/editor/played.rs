//! "As played": the song through the harmony in force, as the engine arranges it (the same
//! [`Arrangement`] passes, unswung so it lines up with the written grid), and the chord chart
//! the band is playing over.

use nat_han_adventures::audio::chart::{Chart, Family, pc_name};
use nat_han_adventures::audio::live::SongFile;
use nat_han_adventures::audio::live::arrange::{Arrangement, Shape};
use nat_han_adventures::audio::theory::{self, MmMode};
use nat_han_adventures::audio::{Harmony, waltz};

use crate::model::Part;

/// The four parts as the band plays them in `harmony`, in the song's own beats.
pub fn parts(song: &SongFile, harmony: Harmony) -> Option<[Part; 4]> {
    let mut plain = song.clone();
    plain.swing = 0.0;
    let shape = if harmony == Harmony::Waltz { Shape::waltz(&plain, 32_000) } else { Shape::new(&plain, 32_000) };
    let a = Arrangement::new(&plain, plain.chart.as_ref(), harmony, 1, &shape).ok()?;
    let (len, bb) = (song.beats(), song.bar_beats());
    Some(std::array::from_fn(|ch| {
        let mut t = a.tracks[ch].clone();
        if harmony == Harmony::Waltz {
            for e in &mut t.events {
                let end = waltz::unwarp(e.start + e.dur);
                e.start = waltz::unwarp(e.start);
                e.dur = end - e.start;
            }
        }
        Part::from_track(ch, &t, len, bb)
    }))
}

/// One bar of a chart as text (`D7`, `G7 C7`, `%` for the bar before repeated).
fn bar_texts(chart: &Chart, label: impl Fn(usize) -> String) -> Vec<String> {
    let m = chart.meter as f64;
    let mut bars: Vec<String> = Vec::with_capacity(chart.bars);
    let mut prev = String::new();
    for b in 0..chart.bars {
        let (a, z) = (b as f64 * m, (b + 1) as f64 * m);
        let mut names: Vec<String> = Vec::new();
        for (i, s) in chart.slots.iter().enumerate() {
            if s.end() > a + 1e-9 && s.start < z - 1e-9 {
                let n = label(i);
                if names.last() != Some(&n) {
                    names.push(n);
                }
            }
        }
        let text = names.join(" ");
        // "%": one chord, the same as the whole bar before.
        bars.push(if names.len() == 1 && text == prev { "%".into() } else { text.clone() });
        prev = text;
    }
    bars
}

/// The written chart, bar by bar.
pub fn written(chart: &Chart) -> Vec<String> {
    bar_texts(chart, |i| chart.slots[i].chord.to_string())
}

/// The chart the band plays over in `harmony`, bar by bar of the written song.
pub fn played(chart: &Chart, harmony: Harmony) -> Vec<String> {
    match harmony {
        Harmony::Coltrane => {
            let c = theory::coltrane(chart);
            bar_texts(&c, |i| c.slots[i].chord.to_string())
        }
        Harmony::MelodicMinor => {
            let slots = chart.slots.clone();
            bar_texts(chart, |i| {
                let mm = theory::melodic_minor(&slots, i);
                let q = match mm.mode {
                    MmMode::MinorMajor => "mMaj9",
                    MmMode::LydianAugmented => "maj7#5",
                    MmMode::LydianDominant => "9#11",
                    MmMode::LocrianNat2 => "m9b5",
                    MmMode::Altered => "7alt",
                };
                format!("{}{q}", pc_name(mm.root))
            })
        }
        Harmony::Quartal => bar_texts(chart, |i| {
            let c = chart.slots[i].chord;
            let q = match c.family() {
                Family::Minor => "m11",
                Family::Dominant => "13sus",
                _ => "4ths",
            };
            format!("{}{q}", pc_name(c.root))
        }),
        // Two 3/4 bars to each written bar.
        Harmony::Waltz => written(chart).into_iter().map(|t| if t == "%" { t } else { format!("¾ {t}") }).collect(),
        Harmony::Original => written(chart),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nat_han_adventures::audio::live::library;

    #[test]
    fn charts_and_parts_as_played() {
        let song = library::load("sweet_georgia_brown").unwrap();
        let c = song.chart.as_ref().unwrap();
        let w = written(c);
        assert_eq!(w.len(), 32);
        assert_eq!((w[0].as_str(), w[1].as_str()), ("D7", "%"));
        assert_eq!(played(c, Harmony::Original), w);
        assert_ne!(played(c, Harmony::Coltrane), w);
        for h in Harmony::ALL {
            let p = parts(&song, h).unwrap();
            assert!(p.iter().all(|p| p.notes.iter().all(|n| n.end() <= song.beats() + 1e-6)), "{h:?}");
        }
    }
}
