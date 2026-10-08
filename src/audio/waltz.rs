//! The jazz waltz ([`super::Harmony::Waltz`]): any song re-cut into 3/4.
//!
//! # The time warp
//! Every 4/4 bar becomes two 3/4 bars: each half-bar (2 beats) is stretched to 3 beats with a
//! long first beat (ONE-two-three). Within a half-bar, beat positions map piecewise-linearly
//! `0 → 0`, `1 → 2`, `2 → 3`: the first beat is doubled, the second keeps its length. So a
//! 4/4 song of `n` beats is a waltz of `1.5 n` beats (`2 × bars` bars of 3/4), and a 4/4 bar
//! line at beat `4k` is the waltz bar line at `6k`. [`warp`] / [`unwarp`] map positions
//! ("canonical" 4/4 beats ↔ waltz beats); the mid-song switch maps the song position through
//! them, so the band picks up at the same point of the tune.
//!
//! # The arrangement
//! - Melody (pulse 1): as written, its rhythm warped ([`warp_track`]); pitches untouched.
//! - Chart: the same warp ([`warp_chart`]): twice the bars, 3 beats each.
//! - Bass + comping ([`super::accomp::waltz`]): "oom-pah-pah": triangle root on 1, pulse-2
//!   chord stabs (arpeggios) on 2 and 3.
//! - Drums ([`drums`]): kick on 1, brushed hats on 2, a soft snare on 3; the first bar of each
//!   pair (where the 4/4 downbeat lands, "the big ONE") gets a harder kick.
//! - Swing applies as usual ([`super::synth::apply_swing`]: off-beat 8ths).
//!
//! # Tempo
//! Every song waltzes at the same [`WALTZ_BPM`]: quarter = 120, i.e. dotted half = 40 bars a
//! minute (1.5 s a bar, 0.5 s a beat). The songs are written at 132–192, so the beat is a
//! little longer than the original's (lilting, never frantic) and the long ONE of the warp
//! makes the bar breathe. One fixed tempo also means the world dances at the same speed in
//! every level, so the waltz gates (`crate::game::Groove`) have one set of numbers.

use super::chart::{Chart, Slot};
use super::mml::{Drum, Event, EventKind, Track};

/// The waltz tempo (quarter notes a minute), for every song: dotted half = 40.
pub const WALTZ_BPM: f32 = 120.0;
/// Beats per waltz bar.
pub const WALTZ_METER: u32 = 3;

/// A canonical (4/4) beat position → its waltz beat position.
pub fn warp(b: f64) -> f64 {
    let half = (b / 2.0).floor();
    let x = b - 2.0 * half;
    3.0 * half + if x < 1.0 { 2.0 * x } else { x + 1.0 }
}

/// A waltz beat position → its canonical (4/4) beat position (the inverse of [`warp`]).
pub fn unwarp(w: f64) -> f64 {
    let bar = (w / 3.0).floor();
    let y = w - 3.0 * bar;
    2.0 * bar + if y < 2.0 { y / 2.0 } else { y - 1.0 }
}

/// A track's rhythm through the warp: every event's start and end are warped (so contiguous
/// events stay contiguous), pitches, volumes and slurs untouched.
pub fn warp_track(t: &Track) -> Track {
    let events = t
        .events
        .iter()
        .map(|e| {
            let start = warp(e.start);
            Event { start, dur: warp(e.start + e.dur) - start, ..*e }
        })
        .filter(|e| e.dur > 1e-9)
        .collect();
    Track { events, length: warp(t.length) }
}

/// A 4/4 chart as a 3/4 chart: twice the bars, every chord boundary warped.
pub fn warp_chart(c: &Chart) -> Chart {
    assert_eq!(c.meter, 4, "only 4/4 charts waltz");
    let slots = c
        .slots
        .iter()
        .map(|s| {
            let start = warp(s.start);
            Slot { start, dur: warp(s.end()) - start, chord: s.chord }
        })
        .collect();
    Chart { slots, bars: c.bars * 2, meter: WALTZ_METER }
}

/// The waltz drum track, `beats` long (a whole number of 3/4 bars): kick on 1, brushed hats on
/// 2 (and its swung "and"), a soft snare on 3; the big ONE (every other bar) kicks harder and
/// the second bar of each pair varies the brushes. Contiguous, so swing treats it like MML.
pub fn drums(beats: f64) -> Track {
    let bars = (beats / WALTZ_METER as f64).round() as usize;
    let hit = |start: f64, dur: f64, d: Drum, volume: u8| Event {
        start,
        dur,
        kind: EventKind::Drum(d),
        volume,
        duty: 0,
        tie: false,
        inst: 0,
    };
    let mut events = Vec::with_capacity(bars * 4);
    for k in 0..bars {
        let t = (k * WALTZ_METER as usize) as f64;
        if k % 2 == 0 {
            events.push(hit(t, 1.0, Drum::Kick, 14));
            events.push(hit(t + 1.0, 0.5, Drum::ClosedHat, 9));
            events.push(hit(t + 1.5, 0.5, Drum::ClosedHat, 6));
            events.push(hit(t + 2.0, 1.0, Drum::Snare, 6));
        } else {
            events.push(hit(t, 1.0, Drum::Kick, 10));
            events.push(hit(t + 1.0, 1.0, Drum::ClosedHat, 9));
            events.push(hit(t + 2.0, 0.5, Drum::Snare, 5));
            events.push(hit(t + 2.5, 0.5, Drum::ClosedHat, 7));
        }
    }
    Track { events, length: beats }
}

#[cfg(test)]
mod tests {
    use super::super::{chart, mml};
    use super::*;

    #[test]
    fn the_warp_doubles_the_first_beat_of_each_half_bar() {
        // 0→0, 0.5→1, 1→2, 1.5→2.5, 2→3, 3→5, 4→6 (the next 4/4 bar is the 3rd waltz bar).
        for (b, w) in [(0.0, 0.0), (0.5, 1.0), (1.0, 2.0), (1.5, 2.5), (2.0, 3.0), (2.5, 4.0), (3.0, 5.0), (4.0, 6.0), (9.0, 14.0)] {
            assert!((warp(b) - w).abs() < 1e-12, "warp({b}) = {}, want {w}", warp(b));
            assert!((unwarp(w) - b).abs() < 1e-12, "unwarp({w}) = {}, want {b}", unwarp(w));
        }
        // Monotonic and invertible everywhere.
        let mut prev = -1.0;
        for i in 0..800 {
            let b = i as f64 * 0.0137;
            let w = warp(b);
            assert!(w > prev);
            assert!((unwarp(w) - b).abs() < 1e-9);
            prev = w;
        }
        // 4/4 bar lines are waltz bar lines (every other one).
        for k in 0..10 {
            assert_eq!(warp(4.0 * k as f64), 6.0 * k as f64);
        }
    }

    #[test]
    fn warped_tracks_keep_their_shape() {
        let t = mml::parse("o5 c8 d8 e8 f8 g4 a8& b8 | c1", mml::Channel::Melodic).unwrap();
        let w = warp_track(&t);
        assert_eq!(w.length, 12.0, "two 4/4 bars = four 3/4 bars");
        assert_eq!(w.events.len(), t.events.len());
        let starts: Vec<f64> = w.events.iter().map(|e| e.start).collect();
        // c8 d8 on the long ONE become quarters; e8 f8 stay 8ths (the swung "and" of 3 included).
        assert_eq!(starts, [0.0, 1.0, 2.0, 2.5, 3.0, 5.0, 5.5, 6.0]);
        for p in w.events.windows(2) {
            assert!((p[0].start + p[0].dur - p[1].start).abs() < 1e-12, "still contiguous");
        }
        assert!(w.events[6].tie, "slurs survive");
        assert_eq!(w.events.last().unwrap().dur, 6.0, "a whole note is a whole 4/4 bar: two waltz bars");
    }

    #[test]
    fn the_chart_doubles_its_bars() {
        let c = chart::parse("| Dm7 | G7 | Cmaj7 A7 | Dm7 G7 C6 C6 |").unwrap();
        let w = warp_chart(&c);
        assert_eq!((w.bars, w.meter, w.beats()), (8, 3, 24.0));
        assert_eq!(w.beats(), warp(c.beats()));
        // A 4-chord bar splits ONE-two | three, ONE | two-three.
        assert_eq!(w.to_text(), "Dm7 | Dm7 | G7 | G7 | Cmaj7 | A7 | Dm7:2 G7:1 | C6:2 C6:1");
        // Each chord sounds over the warped span of where it was.
        for t in [0.0, 0.5, 1.0, 2.7, 3.0, 13.2, 15.9] {
            assert_eq!(w.at(warp(t)), c.at(t), "at {t}");
        }
    }

    #[test]
    fn drums_are_a_waltz() {
        let d = drums(12.0);
        assert_eq!(d.length, 12.0);
        let kicks: Vec<f64> = d.events.iter().filter(|e| e.kind == EventKind::Drum(Drum::Kick)).map(|e| e.start).collect();
        assert_eq!(kicks, [0.0, 3.0, 6.0, 9.0], "kick on every ONE");
        for e in &d.events {
            let beat = e.start.rem_euclid(3.0);
            if beat >= 1.0 {
                assert_ne!(e.kind, EventKind::Drum(Drum::Kick), "brushes on 2 and 3");
            }
        }
        assert!(d.events[0].volume > d.events[4].volume, "the big ONE kicks harder");
    }
}
