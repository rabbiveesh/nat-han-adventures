//! The mix under the lead: the comp and the drums step back while the lead is mid-phrase and
//! come back up in its rests, as a rhythm section does behind a soloist (and a mixing
//! engineer's ducking does on a record). Measured on our own renders (each channel alone,
//! A-weighted), the rhythm section sat only 2.4 to 5.7 dB under the lead and was louder in up
//! to a third of the moments the lead played; the drums (the noise channel) the loudest of it.
//! The bass is left alone: it is already about 10 dB under, and it carries the time.

use super::voice::NoteEvent;

/// How far the comp (pulse 2) steps back under a lead phrase, in dB.
pub const COMP_DB: f32 = -3.0;
/// How far the drums step back under a lead phrase, in dB.
pub const DRUMS_DB: f32 = -4.0;
/// A lead rest shorter than this (in beats) is still mid-phrase: the band stays back.
pub const BRIDGE_BEATS: f64 = 0.5;

/// Duck the comp and drums of one committed bar under its lead notes. `beat` is the beat's
/// length in samples. A note ducks when it starts while a lead note sounds, or within
/// [`BRIDGE_BEATS`] of the last one ending (so a phrase's short breaths don't bob the band).
pub fn duck(events: &mut [NoteEvent], beat: f64) {
    let bridge = (beat * BRIDGE_BEATS) as u64;
    let (comp, drums) = (10f32.powf(COMP_DB / 20.0), 10f32.powf(DRUMS_DB / 20.0));
    // A bar has a few dozen events: a scan per note is cheaper than sorting (and allocates nothing).
    let n = events.len();
    for i in 0..n {
        let g = match events[i].ch {
            1 => comp,
            3 => drums,
            _ => continue,
        };
        let t = events[i].start;
        let under = events.iter().any(|l| l.ch == 0 && l.start <= t && t < l.end + bridge);
        if under {
            events[i].gain *= g;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::live::voice::{Fx, Sound};
    use crate::audio::tuning::Tuning;

    fn ev(ch: u8, start: u64, end: u64) -> NoteEvent {
        NoteEvent {
            ch,
            bar: 0,
            start,
            end,
            sound: Sound::Note(60),
            volume: 15,
            duty: 2,
            tie: false,
            slur_out: false,
            gain: 1.0,
            beat: 0.0,
            phrase_beat: 0.0,
            waltz: false,
            loop_start: 0,
            salt: 0,
            tuning: Tuning::Equal,
            anchor: 60,
            inst: 0,
            fx: Fx::NONE,
            seq: 0,
        }
    }

    #[test]
    fn the_band_steps_back_under_a_phrase_and_fills_its_rests() {
        // A beat of 100 samples: the lead plays 0..150 and 160..200, then rests.
        let mut bar = [ev(0, 0, 150), ev(0, 160, 200), ev(1, 100, 140), ev(3, 155, 170), ev(3, 240, 260), ev(1, 300, 340), ev(2, 100, 200)];
        duck(&mut bar, 100.0);
        let g: Vec<f32> = bar.iter().map(|e| e.gain).collect();
        let (c, d) = (10f32.powf(COMP_DB / 20.0), 10f32.powf(DRUMS_DB / 20.0));
        assert_eq!(&g[..2], &[1.0, 1.0], "the lead as played");
        assert_eq!(g[2], c, "comp under a lead note");
        assert_eq!(g[3], d, "drums in a short breath");
        assert_eq!(g[4], d, "drums half a beat after the phrase");
        assert_eq!(g[5], 1.0, "comp fills the rest");
        assert_eq!(g[6], 1.0, "the bass is left alone");
    }
}
