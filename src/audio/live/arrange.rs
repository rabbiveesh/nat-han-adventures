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
use crate::audio::{Harmony, accomp, melody, theory, tuning, waltz};

use super::ornament::{Harm, Scale};
use super::song::SongFile;

/// A song's timing, at one sample rate.
///
/// The tempo drifts the way a band's does ([`Shape::drifting`]): each bar has its own tempo, a
/// slow curve a couple of bpm either side of the song's, the same every loop pass (so a pass is
/// always the same length and starts back at the written tempo). Beats map to samples through
/// it ([`Shape::at`]); without drift every bar is at [`Shape::bpm`]. A pass takes as long as
/// written either way (the band pushes and lays back around the written tempo).
#[derive(Debug, Clone)]
pub struct Shape {
    pub sample_rate: u32,
    /// The written tempo (each bar's own: [`Shape::bpm_at`]).
    pub bpm: f32,
    /// At the written tempo (each bar's own: [`Shape::spb`]).
    pub samples_per_beat: f64,
    /// Samples per beat in each bar (`bars` of them).
    pub bar_spb: Vec<f64>,
    /// Each bar line, unrounded (`bars` of them; empty when the tempo is steady).
    bar_t: Vec<f64>,
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
        Self::build(song, sample_rate, song.bpm, song.beats(), song.bar_beats(), false, None)
    }

    /// The song re-cut into 3/4 ([`waltz`]): [`waltz::WALTZ_BPM`], 1.5x the beats, twice the
    /// bars. Every bar line of [`Shape::new`] is a bar line here (bar `k` is waltz bar `2k`).
    pub fn waltz(song: &SongFile, sample_rate: u32) -> Self {
        let beats = waltz::warp(song.beats());
        Self::build(song, sample_rate, waltz::WALTZ_BPM, beats, waltz::WALTZ_METER as f64, true, None)
    }

    /// [`Shape::new`] (or [`Shape::waltz`]) with the tempo drifting up to `bpm` either way
    /// ([`drift`], from `seed`).
    pub fn drifting(song: &SongFile, sample_rate: u32, waltz: bool, seed: u64, bpm: f32) -> Self {
        let d = (bpm > 0.0).then_some((seed, bpm));
        if waltz {
            Self::build(song, sample_rate, waltz::WALTZ_BPM, waltz::warp(song.beats()), waltz::WALTZ_METER as f64, true, d)
        } else {
            Self::build(song, sample_rate, song.bpm, song.beats(), song.bar_beats(), false, d)
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build(song: &SongFile, sample_rate: u32, bpm: f32, beats: f64, bar_beats: f64, is_waltz: bool, drift: Option<(u64, f32)>) -> Self {
        let rate = sample_rate as f32 as f64 * 60.0;
        let spb = rate / bpm as f64;
        let bars = ((beats - 1e-9) / bar_beats).ceil().max(1.0) as usize;
        let width = |k: usize| if k + 1 == bars { beats - k as f64 * bar_beats } else { bar_beats };
        let mut bar_spb: Vec<f64> = match drift {
            Some((seed, amount)) => self::drift(bars, seed, amount).into_iter().map(|d| rate / (bpm + d) as f64).collect(),
            None => vec![spb; bars],
        };
        // The band breathes inside the song: a pass takes as long as written.
        let total: f64 = bar_spb.iter().enumerate().map(|(k, s)| s * width(k)).sum();
        if drift.is_some() && total > 0.0 {
            let fit = beats * spb / total;
            bar_spb.iter_mut().for_each(|s| *s *= fit);
        }
        // Bar lines where the beats add up (rounded once, from the exact sum; steady, exactly
        // where `beats · samples_per_beat` puts them).
        let mut t = 0.0f64;
        let mut bar_t: Vec<f64> = Vec::with_capacity(bars);
        for (k, &s) in bar_spb.iter().enumerate() {
            bar_t.push(if drift.is_some() { t } else { k as f64 * bar_beats * spb });
            t += s * width(k);
        }
        let len = if drift.is_some() { t.round() as u64 } else { (beats * spb).round() as u64 };
        let mut bar_starts: Vec<u64> = bar_t.iter().map(|t| t.round() as u64).collect();
        bar_starts.push(len);
        Shape {
            sample_rate,
            bpm,
            samples_per_beat: spb,
            bar_spb,
            bar_t: if drift.is_some() { bar_t } else { Vec::new() },
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

    /// Sample (within the loop) of a beat time, rounded like the offline renderer: from the
    /// bar line, at that bar's tempo (past the loop's ends, the first or last bar's).
    #[inline]
    pub fn at(&self, beats: f64) -> u64 {
        if self.bar_t.is_empty() {
            return (beats * self.samples_per_beat).round() as u64;
        }
        let k = ((beats / self.bar_beats + 1e-9).floor().max(0.0) as usize).min(self.bars - 1);
        (self.bar_t[k] + (beats - k as f64 * self.bar_beats) * self.bar_spb[k]).round() as u64
    }

    /// Samples per beat in bar `bar` (within the loop).
    #[inline]
    pub fn spb(&self, bar: usize) -> f64 {
        self.bar_spb[bar.min(self.bars - 1)]
    }

    /// The tempo in bar `bar` (within the loop).
    #[inline]
    pub fn bpm_at(&self, bar: usize) -> f32 {
        (self.sample_rate as f32 as f64 * 60.0 / self.spb(bar)) as f32
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

/// A band's tempo drift over `bars` bars: each bar's offset from the written tempo, in bpm, at
/// most `amount` either way. A slow curve (a wave or two across the song, a smaller one on
/// top), periodic over the loop so the next pass joins on. Seeded.
pub fn drift(bars: usize, seed: u64, amount: f32) -> Vec<f32> {
    let mut rng = accomp::Rng::new(seed ^ 0xD71F_7ED0);
    let waves: [(f64, f64, f64); 3] = std::array::from_fn(|i| {
        let cycles = [1.0, 2.0, 3.0 + rng.below(2) as f64][i];
        let size = [1.0, 0.6, 0.3][i] * (0.6 + 0.4 * rng.f());
        (cycles, size, rng.f() * std::f64::consts::TAU)
    });
    let raw: Vec<f64> = (0..bars)
        .map(|k| {
            let x = std::f64::consts::TAU * k as f64 / bars.max(1) as f64;
            waves.iter().map(|&(c, a, p)| a * (c * x + p).sin()).sum::<f64>()
        })
        .collect();
    // Around the written tempo (the pass keeps its length: [`Shape::drifting`]).
    let mean = raw.iter().sum::<f64>() / bars.max(1) as f64;
    let raw: Vec<f64> = raw.iter().map(|v| v - mean).collect();
    let peak = raw.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    raw.iter().map(|v| if peak > 0.0 { (v / peak) as f32 * amount } else { 0.0 }).collect()
}

/// Give each event of a generated `track` the instrument the `written` one plays at its time
/// (`warp`: the generated track is the waltz's, in 3/4 beats).
fn inherit_instruments(track: &mut Track, written: &Track, warp: bool) {
    if written.events.iter().all(|e| e.inst == 0) {
        return;
    }
    for e in &mut track.events {
        let t = if warp { waltz::unwarp(e.start) } else { e.start };
        let k = written.events.partition_point(|w| w.start <= t + 1e-9);
        e.inst = written.events[k.saturating_sub(1)].inst;
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
    /// The harmony in force, chord by chord, in this harmony's version of the chart (Coltrane's
    /// substitutions, the waltz's 3/4 chart) with the scale the band draws from (the melodic
    /// minor's modes, quartal's lydian): `(start, end, harmony)` in song beats. Empty without a
    /// chart.
    pub harm: Vec<(f64, f64, Harm)>,
    /// The song's swing (off-beat 8ths start this many 8ths late).
    pub swing: f32,
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
        if harmony != Harmony::Original {
            // Generated parts play the instrument the written part plays at that point.
            let warp = harmony == Harmony::Waltz;
            for (t, ch) in [(&mut p1, 0), (&mut p2, 1), (&mut tri, 2), (&mut noise, 3)] {
                inherit_instruments(t, &song.tracks[ch], warp);
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
        let harm = chart.map(|c| harmony_slots(c, harmony)).unwrap_or_default();
        Ok(Arrangement { harmony, tracks, anchors, bar_events, harm, swing: song.swing })
    }

    /// The harmony at song beat `beat` (wrapping round the loop), if the song has a chart.
    pub fn harm_at(&self, beat: f64) -> Option<Harm> {
        let end = self.harm.last()?.1;
        let t = beat.rem_euclid(end);
        let i = self.harm.partition_point(|s| s.0 <= t + 1e-9);
        Some(self.harm[i.saturating_sub(1)].2)
    }
}

/// [`Arrangement::harm`] for a chart in a harmony.
fn harmony_slots(chart: &Chart, harmony: Harmony) -> Vec<(f64, f64, Harm)> {
    let c = match harmony {
        Harmony::Coltrane => theory::coltrane(chart),
        Harmony::Waltz => waltz::warp_chart(chart),
        _ => chart.clone(),
    };
    let slots = c.merged();
    (0..slots.len())
        .map(|i| {
            let s = slots[i];
            let scale = match harmony {
                Harmony::MelodicMinor => {
                    let mm = theory::melodic_minor(&slots, i);
                    Scale::from_pcs(s.chord.root, &mm.scale())
                }
                Harmony::Quartal => Scale::of(&s.chord, true),
                _ => Scale::of(&s.chord, false),
            };
            (s.start, s.end(), Harm { chord: s.chord, scale })
        })
        .collect()
}
