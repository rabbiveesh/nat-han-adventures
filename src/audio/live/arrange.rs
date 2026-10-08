//! The written parts, per harmony, indexed by bar.
//!
//! A reharmonization is *symbolic*: [`accomp::generate`] writes new comping and bass from the
//! chart, [`melody::reharmonize`] remaps the melody. Both are whole-song passes (the melody's
//! voice leading is a Viterbi over the line, the bass walks from note to note), so the engine
//! runs them once per harmony when a song is loaded (a few ms, no audio) and keeps the results
//! side by side. The musicians then read whichever harmony is current *bar by bar*, so a
//! filter change lands on the next bar committed: no re-render, no position mapping.
//!
//! The waltz ([`Harmony::Waltz`], [`waltz`]) changes the meter: its arrangement is laid out on
//! its own [`Shape`] ([`Shape::waltz`]: 3/4 at [`waltz::WALTZ_BPM`], twice the bars), and the
//! engine switches shapes at a bar line both share (see [`super::engine`]).

use crate::audio::chart::Chart;
use crate::audio::mml::{EventKind, Track};
use crate::audio::synth::apply_swing;
use crate::audio::{Harmony, accomp, melody, tuning, waltz};

use super::song::SongFile;

/// A song's timing, at one sample rate.
#[derive(Debug, Clone)]
pub struct Shape {
    pub sample_rate: u32,
    pub bpm: f32,
    pub samples_per_beat: f64,
    /// Beats (quarter notes) per bar.
    pub bar_beats: f64,
    /// Loop length in beats (the longest track).
    pub beats: f64,
    /// Bars per loop.
    pub bars: usize,
    /// Loop length in samples.
    pub len: u64,
    pub looping: bool,
    /// Sample of each bar line within the loop (`bars + 1` of them; the last is `len`).
    pub bar_starts: Vec<u64>,
    pub key: u8,
    /// [`tuning::hash_str`] of the title (seeds the tunings, like the offline renderer).
    pub song_hash: u64,
    /// The waltz's shape ([`Shape::waltz`]): beats are 3/4 beats through [`waltz::warp`].
    pub waltz: bool,
}

impl Shape {
    pub fn new(song: &SongFile, sample_rate: u32) -> Self {
        Self::build(song, sample_rate, song.bpm, song.beats(), song.bar_beats(), false)
    }

    /// The song re-cut into 3/4 ([`waltz`]): [`waltz::WALTZ_BPM`], 1.5x the beats, twice the
    /// bars. Every bar line of [`Shape::new`] is a bar line here (bar `k` is waltz bar `2k`).
    pub fn waltz(song: &SongFile, sample_rate: u32) -> Self {
        let beats = waltz::warp(song.beats());
        Self::build(song, sample_rate, waltz::WALTZ_BPM, beats, waltz::WALTZ_METER as f64, true)
    }

    fn build(song: &SongFile, sample_rate: u32, bpm: f32, beats: f64, bar_beats: f64, is_waltz: bool) -> Self {
        let spb = sample_rate as f32 as f64 * 60.0 / bpm as f64;
        let bars = ((beats - 1e-9) / bar_beats).ceil().max(1.0) as usize;
        let at = |b: f64| (b * spb).round() as u64;
        let len = at(beats);
        let bar_starts = (0..=bars).map(|k| if k == bars { len } else { at(k as f64 * bar_beats) }).collect();
        Shape {
            sample_rate,
            bpm,
            samples_per_beat: spb,
            bar_beats,
            beats,
            bars,
            len,
            looping: song.looping,
            bar_starts,
            key: song.key,
            song_hash: tuning::hash_str(&song.title),
            waltz: is_waltz,
        }
    }

    /// The 4/4 ("canonical") beat of beat `b` of this shape (the medley's phrases count in
    /// them, so they change where they would have).
    #[inline]
    pub fn canon(&self, b: f64) -> f64 {
        if self.waltz { waltz::unwarp(b) } else { b }
    }

    /// Sample (within the loop) of a beat time, rounded like the offline renderer.
    #[inline]
    pub fn at(&self, beats: f64) -> u64 {
        (beats * self.samples_per_beat).round() as u64
    }

    /// Absolute bar `index` → (loop pass, bar within the song).
    pub fn split(&self, index: u64) -> (u64, usize) {
        (index / self.bars as u64, (index % self.bars as u64) as usize)
    }

    /// Absolute sample of the start of bar `index` (`None` past the end of a one-shot).
    pub fn bar_start(&self, index: u64) -> Option<u64> {
        let (pass, bar) = self.split(index);
        if !self.looping && pass > 0 {
            return (pass == 1 && bar == 0).then_some(self.len);
        }
        Some(pass * self.len + self.bar_starts[bar])
    }

    /// Bar within the loop at sample `s` (within the loop).
    pub fn bar_at(&self, s: u64) -> usize {
        self.bar_starts[..self.bars].partition_point(|&b| b <= s).saturating_sub(1)
    }
}

/// The four parts in one harmony, swung, with each bar's events.
#[derive(Debug, Clone)]
pub struct Arrangement {
    pub harmony: Harmony,
    /// pulse1, pulse2, triangle, noise; swung ([`apply_swing`]) like the offline renderer.
    pub tracks: [Track; 4],
    /// Per melodic channel, the tonic the tunings anchor on ([`tuning::anchor_tonic`]).
    pub anchors: [u8; 3],
    /// Per channel and bar: the range of event indices starting in that bar.
    pub bar_events: [Vec<(usize, usize)>; 4],
}

impl Arrangement {
    /// The song in `harmony` (`seed` varies the generated comping and bass), laid out on
    /// `shape` ([`Shape::waltz`] for the waltz). Errors if the song can't be reharmonized (no
    /// chart).
    pub fn new(song: &SongFile, chart: Option<&Chart>, harmony: Harmony, seed: u64, shape: &Shape) -> Result<Self, String> {
        let [mut p1, mut p2, mut tri, mut noise] = song.tracks.clone();
        if harmony != Harmony::Original {
            let chart = chart.ok_or_else(|| format!("\"{}\" has no chord chart to reharmonize", song.title))?;
            if harmony == Harmony::Waltz {
                // Re-cut into 3/4: the melody's rhythm warped, a new band (see [`waltz`]).
                if !shape.waltz || song.meter.beats != 4 || song.meter.unit != 4 {
                    return Err(format!("\"{}\": only a 4/4 song waltzes, on the waltz's shape", song.title));
                }
                let w = waltz::warp_chart(chart);
                (p2, tri) = accomp::waltz(&w, seed);
                p1 = waltz::warp_track(&p1);
                noise = waltz::drums(w.beats());
            } else {
                (p2, tri) = accomp::generate(chart, harmony, song.key, seed);
                p1 = melody::reharmonize(&p1, chart, harmony);
            }
        }
        let tracks = [p1, p2, tri, noise].map(|t| apply_swing(&t, song.swing));
        let anchors = std::array::from_fn(|ch| {
            let notes = || {
                tracks[ch].events.iter().flat_map(|e| match &e.kind {
                    EventKind::Note(n) => std::slice::from_ref(n),
                    EventKind::Arp(a) => a.notes(),
                    _ => &[],
                })
            };
            let n = notes().count().max(1) as f64;
            tuning::anchor_tonic(song.key % 12, notes().map(|&x| x as f64).sum::<f64>() / n)
        });
        let bar_events = std::array::from_fn(|ch| {
            let ev = &tracks[ch].events;
            (0..shape.bars)
                .map(|b| {
                    let (lo, hi) = (b as f64 * shape.bar_beats - 1e-9, (b + 1) as f64 * shape.bar_beats - 1e-9);
                    let from = ev.partition_point(|e| e.start < lo);
                    let to = ev.partition_point(|e| e.start < hi);
                    (from, to)
                })
                .collect()
        });
        Ok(Arrangement { harmony, tracks, anchors, bar_events })
    }
}
