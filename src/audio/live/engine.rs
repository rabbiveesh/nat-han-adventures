//! The engine: a song, a band, a voice bank and a queue of inputs, rendering any block size in
//! real time. No Bevy, no game: [`Input`]s mirror what the game reports, so a native editor can
//! drive the same engine from dials.
//!
//! # Time
//! The playhead is an absolute sample count. Bars are numbered from 0 since the engine
//! started and keep counting through loop passes and meter changes. Bar lines fall where the
//! offline renderer puts them (`round(beats · samples_per_beat)` within each pass).
//!
//! # The waltz: two shapes
//! The waltz ([`Harmony::Waltz`]) is the song re-cut into 3/4 at its own tempo, on its own
//! [`Shape`] (twice the bars). The timeline is a run of [`Span`]s, each in one shape: a loop
//! pass, or the part of one from a meter switch. A switch happens when a bar is committed, at a
//! bar line both shapes share (as the old renderer's `switch_point` did): every bar line of the
//! song's own shape is one (bar `k` becomes waltz bar `2k`, the first of a pair, where the 4/4
//! downbeat lands); a waltz bar line only if it starts a pair (so leaving the waltz may finish
//! the pair first). The tune carries on from the same point; the [`BeatClock`] turns 3/4 at
//! the waltz's tempo; the medley keeps its phrases where the 4/4 song has them.
//!
//! # Scheduling
//! Each bar is *committed* [`EngineConfig::commit_lead_beats`] before its bar line (default one
//! beat): the musicians ([`super::musician`]) turn it into note events, in the harmony and
//! tuning in force at that moment, and those events are final. At that moment the committed
//! horizon reaches the end of that bar, about a bar and a beat ahead of the playhead; everything
//! after is the musicians' plans, which inputs may still change. So a filter change takes
//! effect at the next bar line that isn't committed yet: the next one, unless it arrives in the
//! last beat before it. (A whole-bar lead would make every change wait a further bar.)
//!
//! Commits happen at their exact sample (blocks are split there), and inputs are applied in
//! order at the start of the [`Engine::fill`] after they're posted, so the output depends only
//! on the inputs and the sample positions they arrive at, never on the block size.
//!
//! # The director
//! The engine doesn't decide the filters: the game's director ([`crate::audio::director`]) stays
//! where it is and sends its decisions as [`Input::SetFilters`]; [`Input::ForceHarmony`] /
//! [`Input::ForceTuning`] override them (dev overrides, the editor's dials). An editor without
//! a game can set [`EngineConfig::self_directed`]: the engine then runs the director's
//! [`director::Band`] itself on the gameplay inputs, on its own clock (summons, holds, the jump
//! in threes spotted in [`Input::Jump`]s, [`Input::WaltzStep`] counting as one, a check every
//! [`director::MUSIC_CHECK_SECS`]), and [`director::choose_filters`] on every
//! [`Input::SetStats`] (stats dials).

use std::collections::VecDeque;

use kira::Frame;

use crate::audio::chart::Chart;
use crate::audio::director::{self, PlayStats};
use crate::audio::synth::soft_clip;
use crate::audio::tuning::{Medley, Tuning};
use crate::audio::{Filters, Harmony};

use super::arrange::{Arrangement, Shape};
use super::band::{BandInput, BandPlan};
use super::feel::{self, Feel};
use super::instrument::Instruments;
use super::musician::{self, BarSlot, Ctx, Musician, PhrasePlan, Role};
use super::ornament::Orns;
use super::song::SongFile;
use super::voice::{MAX_SEGMENT, NoteEvent, VoiceBank};

/// Something that happened, or a dial that moved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Input {
    /// A double jump.
    Toot,
    Nugget,
    Death,
    Checkpoint,
    Jump { on_ground: bool },
    /// Landed, falling at `speed` px/s.
    Land { speed: f32 },
    /// A jump in threes (the director spotted one; the game sends it). Self-directed, it counts
    /// toward the waltz like the director's own.
    WaltzStep,
    LevelStart,
    Restart,
    /// The director's rolling-window stats.
    SetStats(PlayStats),
    /// The director's decision.
    SetFilters(Filters),
    /// Override the harmony (`None`: follow the filters).
    ForceHarmony(Option<Harmony>),
    /// Override the tuning (`None`: follow the filters: the laughing band plays the medley).
    ForceTuning(Option<Tuning>),
    /// Override the band's feel ([`super::feel`]; `None`: the band picks, `Some(Feel::Swing)`:
    /// never). A dev override (the editor's chips); it lands on the next bar committed.
    ForceFeel(Option<Feel>),
    /// Each musician's freedom, and the dynamics, 0..1.
    SetFreedom { lead: f32, comp: f32, bass: f32, drums: f32, dynamics: f32 },
    /// Each channel's level (pulse 1, pulse 2, triangle, noise): 1 as written, 0 muted. A mixer
    /// (an editor's mute / solo / volume): it applies at once, not at a bar line.
    SetMix([f32; 4]),
}

/// Engine settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EngineConfig {
    /// Seeds the generated accompaniment and every free choice.
    pub seed: u64,
    /// How far before its bar line a bar is committed, in beats.
    pub commit_lead_beats: f64,
    /// Run the director inside the engine (for an editor without a game; see the module docs).
    pub self_directed: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        EngineConfig { seed: 1, commit_lead_beats: 1.0, self_directed: false }
    }
}

/// The freedom dials.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Freedom {
    pub lead: f32,
    pub comp: f32,
    pub bass: f32,
    pub drums: f32,
    pub dynamics: f32,
}

/// Where the playhead is.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Position {
    /// Absolute sample.
    pub sample: u64,
    /// Loop pass, absolute bar and bar within the song.
    pub pass: u64,
    pub bar: u64,
    pub song_bar: usize,
    /// Beats into the bar, and into the song (within the pass).
    pub beat: f64,
    pub song_beat: f64,
}

/// Song position and beat phase, for physics that move with the music.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BeatClock {
    pub position: Position,
    pub bpm: f32,
    pub beats_per_bar: f64,
    /// Loop length in beats (0 for a one-shot).
    pub loop_beats: f64,
    /// Whole beats into the bar (0-based) and the phase within the beat, 0..1.
    pub beat_index: u32,
    pub phase: f64,
    pub sample_rate: u32,
}

impl BeatClock {
    /// The clock `secs` later (or earlier, if negative) at the same tempo: to extrapolate
    /// between audio callbacks, or to step back to what's being heard. Never before the start.
    pub fn advanced(&self, secs: f64) -> BeatClock {
        let mut c = *self;
        let mut sb = self.position.song_beat + secs * self.bpm as f64 / 60.0;
        // Bars and passes move on from where they are (absolute bars needn't line up with the
        // song's: a meter switch re-enters the song mid-pass).
        let (mut pass, mut bar_shift) = (self.position.pass as i64, 0i64);
        if self.loop_beats > 0.0 {
            let passes = (sb / self.loop_beats).floor();
            sb -= passes * self.loop_beats;
            pass += passes as i64;
            bar_shift = passes as i64 * (self.loop_beats / self.beats_per_bar).round() as i64;
        }
        let sb = sb.max(0.0);
        let song_bar = (sb / self.beats_per_bar + 1e-9).floor();
        let bar = self.position.bar as i64 + bar_shift + song_bar as i64 - self.position.song_bar as i64;
        if pass < 0 || bar < 0 {
            return BeatClock { position: Position::default(), beat_index: 0, phase: 0.0, ..*self };
        }
        c.position.pass = pass as u64;
        c.position.song_beat = sb;
        c.position.song_bar = song_bar as usize;
        c.position.bar = bar as u64;
        c.position.beat = (sb - song_bar * self.beats_per_bar).max(0.0);
        c.position.sample = (self.position.sample as i64 + (secs * self.sample_rate as f64).round() as i64).max(0) as u64;
        c.beat_index = c.position.beat.floor() as u32;
        c.phase = c.position.beat - c.position.beat.floor();
        c
    }
}

/// A committed bar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CommittedBar {
    pub slot: BarSlot,
    pub harmony: Harmony,
    pub tuning: Tuning,
    pub intensity: f32,
    /// Events committed for it.
    pub events: u32,
    /// What each musician played beyond the written part (lead, comp, bass, drums).
    pub orns: [Orns; 4],
    /// The band's shared plan for it.
    pub band: BandPlan,
}

/// One musician, for UIs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MusicianState {
    pub role: Role,
    pub freedom: f32,
    pub plan: Option<PhrasePlan>,
}

/// A snapshot for UIs ([`Engine::state`]).
#[derive(Debug, Clone, PartialEq)]
pub struct EngineState {
    pub position: Position,
    /// What's sounding at the playhead.
    pub harmony: Harmony,
    pub tuning: Tuning,
    /// The medley's tuning for the phrase at the playhead (when the tuning is the medley).
    pub medley_phrase: Option<Tuning>,
    /// The inputs as they stand (they apply from the next bar committed).
    pub filters: Filters,
    pub forced_harmony: Option<Harmony>,
    pub forced_tuning: Option<Tuning>,
    pub forced_feel: Option<Feel>,
    /// The feel sounding at the playhead.
    pub feel: Feel,
    pub freedom: Freedom,
    pub stats: PlayStats,
    /// Committed bars from the one at the playhead on.
    pub upcoming: Vec<CommittedBar>,
    pub musicians: [MusicianState; 4],
    /// The song is over (one-shots only).
    pub finished: bool,
}

impl Default for EngineState {
    fn default() -> Self {
        EngineState {
            position: Position::default(),
            harmony: Harmony::Original,
            tuning: Tuning::Equal,
            medley_phrase: None,
            filters: Filters::default(),
            forced_harmony: None,
            forced_tuning: None,
            forced_feel: None,
            feel: Feel::Swing,
            freedom: Freedom::default(),
            stats: PlayStats::default(),
            upcoming: Vec::with_capacity(8),
            musicians: Role::ALL.map(|role| MusicianState { role, freedom: 0.0, plan: None }),
            finished: false,
        }
    }
}

/// Gameplay energy per event (dynamics), and its decay per bar.
const ENERGY_TOOT: f32 = 0.06;
const ENERGY_NUGGET: f32 = 0.05;
const ENERGY_CHECKPOINT: f32 = 0.1;
const ENERGY_WALTZ_STEP: f32 = 0.05;
const ENERGY_DEATH: f32 = -0.25;
const ENERGY_DECAY: f32 = 0.8;

/// A stretch of the timeline played in one shape: a loop pass, or the part of one from a meter
/// switch on (or up to one).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    /// In the waltz's shape ([`Shape::waltz`]), else the song's own.
    pub waltz: bool,
    /// Absolute sample of its first bar line. Identifies the pass to the voices (oscillator
    /// phases and the noise restart at it, like a one-loop render).
    pub start: u64,
    /// The song bar it starts at: 0, unless it starts at a meter switch.
    pub entry: usize,
    /// Absolute end: the loop's end, or a later meter switch.
    pub end: u64,
    /// Loop passes before it.
    pub pass: u64,
    /// Absolute bar number of bar `entry`.
    pub first_bar: u64,
}

impl Span {
    /// Absolute sample of bar line `b` (`b` up to `shape.bars`) of this span's pass.
    fn bar_start(&self, shape: &Shape, b: usize) -> u64 {
        self.start + shape.bar_starts[b] - shape.bar_starts[self.entry]
    }
}

/// Spans kept (the playhead's to the committed horizon's: a few).
const SPANS: usize = 8;

/// The live engine.
pub struct Engine {
    title: String,
    shape: Shape,
    /// The waltz's shape, if the song can waltz.
    waltz: Option<Shape>,
    /// The chart, and the waltz's (warped) chart.
    charts: [Option<Chart>; 2],
    /// Indexed like [`Harmony::ALL`]; `None` where the song can't take the harmony.
    arrangements: [Option<Arrangement>; 5],
    medley: Medley,
    config: EngineConfig,
    /// The playhead (absolute sample).
    t: u64,
    inputs: VecDeque<Input>,
    filters: Filters,
    forced_harmony: Option<Harmony>,
    forced_tuning: Option<Tuning>,
    forced_feel: Option<Feel>,
    freedom: Freedom,
    stats: PlayStats,
    /// [`EngineConfig::self_directed`]: the director, on the engine's clock.
    band: director::Band,
    energy: f32,
    musicians: [Box<dyn Musician>; 4],
    committed: VecDeque<CommittedBar>,
    /// The timeline from the playhead's span to the one being committed (the last).
    spans: VecDeque<Span>,
    /// The next bar to commit: absolute, and within the last span's shape.
    next_bar: u64,
    next_song_bar: usize,
    bank: VoiceBank,
    scratch: Vec<NoteEvent>,
    seq: u64,
    instruments: Instruments,
    /// The last bar committed: its band plan and harmony (fills crash into the next bar; a
    /// harmony switch is a summon's flourish).
    prev_band: BandPlan,
    prev_harmony: Option<Harmony>,
}

impl Engine {
    pub fn new(song: &SongFile, sample_rate: u32) -> Result<Engine, String> {
        Self::with_config(song, sample_rate, EngineConfig::default())
    }

    pub fn with_config(song: &SongFile, sample_rate: u32, config: EngineConfig) -> Result<Engine, String> {
        if !(8_000..=192_000).contains(&sample_rate) {
            return Err(format!("sample rate {sample_rate} out of range"));
        }
        let shape = Shape::new(song, sample_rate);
        if shape.len == 0 {
            return Err(format!("\"{}\" is empty", song.title));
        }
        let chart = song.chart.clone();
        let wshape = Shape::waltz(song, sample_rate);
        let arrangements = Harmony::ALL.map(|h| {
            let on = if h == Harmony::Waltz { &wshape } else { &shape };
            Arrangement::new(song, chart.as_ref(), h, config.seed, on).ok()
        });
        if arrangements[0].is_none() {
            return Err(format!("\"{}\" can't be arranged", song.title));
        }
        let can_waltz = arrangements[harmony_index(Harmony::Waltz)].is_some() && wshape.bars == 2 * shape.bars;
        let waltz = can_waltz.then_some(wshape);
        let charts = [chart.clone(), chart.as_ref().filter(|_| can_waltz).map(crate::audio::waltz::warp_chart)];
        let secs = |sh: &Shape| song.looping.then_some(sh.len as f64 / sample_rate as f32 as f64);
        let medley = Medley::new(shape.song_hash, shape.beats, secs(&shape));
        let waltz_medley = waltz.as_ref().map_or_else(|| medley.clone(), |w| Medley::new(shape.song_hash, shape.beats, secs(w)));
        let mut spans = VecDeque::with_capacity(SPANS);
        spans.push_back(Span { waltz: false, start: 0, entry: 0, end: shape.len, pass: 0, first_bar: 0 });
        // The song's instruments and the feels' (appended: the song's own numbers don't move).
        let instruments = feel::equip(&song.instruments);
        Ok(Engine {
            title: song.title.clone(),
            bank: VoiceBank::new(sample_rate, [medley.clone(), waltz_medley], &instruments),
            medley,
            shape,
            waltz,
            charts,
            arrangements,
            config,
            t: 0,
            inputs: VecDeque::with_capacity(256),
            filters: Filters::default(),
            forced_harmony: None,
            forced_tuning: None,
            forced_feel: None,
            freedom: Freedom::default(),
            stats: PlayStats::default(),
            band: director::Band::default(),
            energy: 0.0,
            musicians: musician::band(config.seed ^ crate::audio::tuning::hash_str(&song.title)),
            committed: VecDeque::with_capacity(16),
            spans,
            next_bar: 0,
            next_song_bar: 0,
            scratch: Vec::with_capacity(4096),
            seq: 0,
            instruments,
            prev_band: BandPlan::default(),
            prev_harmony: None,
        })
    }

    /// Start the first pass at sample `sample` (of the song's own shape) instead of the top:
    /// an editor's "play from here", or a hot swap that carries on where the last engine was.
    /// Only before the first [`Engine::fill`]. Bars before it are skipped; if it falls inside a
    /// bar, that bar stays silent and the band comes in at the next bar line. Past the end it's
    /// ignored.
    pub fn start_at(&mut self, sample: u64) {
        if self.t != 0 || !self.committed.is_empty() || sample >= self.shape.len {
            return;
        }
        let bar = self.shape.bar_at(sample);
        let bar = if self.shape.bar_starts[bar] < sample { bar + 1 } else { bar };
        self.t = sample;
        self.next_song_bar = bar;
        self.next_bar = bar as u64;
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    /// The song's own shape (4/4, or whatever its meter is).
    pub fn shape(&self) -> &Shape {
        &self.shape
    }

    /// The waltz's shape, if the song can waltz.
    pub fn waltz_shape(&self) -> Option<&Shape> {
        self.waltz.as_ref()
    }

    /// The shape at the playhead.
    pub fn playing_shape(&self) -> &Shape {
        self.shape_of(self.span_at(self.t).waltz)
    }

    fn shape_of(&self, waltz: bool) -> &Shape {
        match (&self.waltz, waltz) {
            (Some(w), true) => w,
            _ => &self.shape,
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.shape.sample_rate
    }

    /// The song arranged in harmony `h` (its written parts and the harmony in force), if it can
    /// take it.
    pub fn arrangement(&self, h: Harmony) -> Option<&Arrangement> {
        self.arrangements[harmony_index(h)].as_ref()
    }

    /// The song's instruments, with the feels' ([`feel::equip`]).
    pub fn instruments(&self) -> &Instruments {
        &self.instruments
    }

    /// Can the song take this harmony (it has a chart)?
    pub fn can_play(&self, h: Harmony) -> bool {
        self.arrangements[harmony_index(h)].is_some() && (h != Harmony::Waltz || self.waltz.is_some())
    }

    /// Queue an input; it applies at the start of the next [`Engine::fill`].
    pub fn post(&mut self, input: Input) {
        if self.inputs.len() == self.inputs.capacity() {
            // Full (nobody's rendering?): drop the oldest gameplay event rather than allocate.
            self.inputs.pop_front();
        }
        self.inputs.push_back(input);
    }

    /// Render the next `out.len()` frames.
    pub fn fill(&mut self, out: &mut [Frame]) {
        while let Some(i) = self.inputs.pop_front() {
            self.apply(i);
        }
        let mut pos = 0;
        loop {
            while self.commit_point().is_some_and(|c| c <= self.t) {
                self.commit();
            }
            if pos == out.len() {
                break;
            }
            let mut n = (out.len() - pos).min(MAX_SEGMENT) as u64;
            if let Some(c) = self.commit_point() {
                n = n.min(c - self.t);
            }
            let span = *self.span_at(self.t);
            if self.t < span.end {
                // Each segment stays within one pass (or span).
                n = n.min(span.end - self.t);
            }
            let seg = &mut out[pos..pos + n as usize];
            seg.fill(Frame::ZERO);
            self.bank.render(seg, self.t, span.start);
            for f in seg.iter_mut() {
                f.left = soft_clip(f.left);
                f.right = soft_clip(f.right);
            }
            self.t += n;
            pos += n as usize;
            while self.committed.len() > 1 && self.committed[0].slot.end <= self.t {
                self.committed.pop_front();
            }
            while self.spans.len() > 1 && self.spans[0].end <= self.t {
                self.spans.pop_front();
            }
        }
    }

    /// The span sample `t` is in (the last one past the end of a one-shot).
    fn span_at(&self, t: u64) -> &Span {
        self.spans.iter().find(|s| t < s.end).unwrap_or_else(|| self.spans.back().expect("a span"))
    }

    /// The next bar to commit, in the current shape (`None` past a one-shot's end). At a loop's
    /// end it's bar 0 of the next pass (a meter switch may still move it: see [`Engine::commit`]).
    fn next_slot(&self) -> Option<BarSlot> {
        let sp = self.spans.back().expect("a span");
        let sh = self.shape_of(sp.waltz);
        let b = self.next_song_bar;
        if b < sh.bars {
            return Some(BarSlot {
                index: self.next_bar,
                pass: sp.pass,
                song_bar: b,
                start: sp.bar_start(sh, b),
                end: sp.bar_start(sh, b + 1),
                loop_start: sp.start,
            });
        }
        sh.looping.then(|| BarSlot {
            index: self.next_bar,
            pass: sp.pass + 1,
            song_bar: 0,
            start: sp.end,
            end: sp.end + sh.bar_starts[1],
            loop_start: sp.end,
        })
    }

    /// When the next bar gets committed.
    fn commit_point(&self) -> Option<u64> {
        let s = self.next_slot()?;
        let sh = self.shape_of(self.spans.back().expect("a span").waltz);
        let lead = (self.config.commit_lead_beats * sh.samples_per_beat).round() as u64;
        Some(s.start.saturating_sub(lead))
    }

    /// The harmony and tuning a bar committed now gets (before the meter has its say).
    fn current_filters(&self) -> (Harmony, Tuning) {
        let mut h = self.forced_harmony.unwrap_or(self.filters.harmony);
        if !self.can_play(h) {
            h = Harmony::Original;
        }
        let t = self.forced_tuning.unwrap_or(if self.filters.just_intonation { Tuning::Medley } else { Tuning::Equal });
        (h, t)
    }

    /// Commit the next bar. Into or out of the waltz, the meter changes here if this bar line is
    /// one both shapes share: every bar line of the song's own shape is one (bar `k` is waltz
    /// bar `2k`, the first of a pair, where the 4/4 downbeat lands); a waltz bar line is one
    /// only if it's the first of a pair. Otherwise the bar stays in the waltz, as a waltz bar.
    fn commit(&mut self) {
        let Some(slot) = self.next_slot() else { return };
        let (mut harmony, tuning) = self.current_filters();
        let sp = *self.spans.back().expect("a span");
        let mut replan = false;
        if slot.song_bar == 0 && slot.loop_start != sp.start {
            // A new loop pass.
            let len = self.shape_of(sp.waltz).len;
            self.push_span(Span { start: slot.start, entry: 0, end: slot.start + len, pass: slot.pass, first_bar: slot.index, ..sp });
            self.next_song_bar = 0;
        }
        let cur = *self.spans.back().expect("a span");
        let want_waltz = harmony == Harmony::Waltz;
        if want_waltz != cur.waltz {
            let b = self.next_song_bar;
            if want_waltz || b.is_multiple_of(2) {
                let entry = if want_waltz { 2 * b } else { b / 2 };
                let to = self.shape_of(want_waltz);
                let end = slot.start + to.len - to.bar_starts[entry];
                // The span in progress ends at this bar line.
                self.spans.back_mut().expect("a span").end = slot.start;
                self.push_span(Span { waltz: want_waltz, start: slot.start, entry, end, pass: slot.pass, first_bar: slot.index });
                self.next_song_bar = entry;
                replan = true;
            } else {
                // The second bar of a waltz pair: finish the pair.
                harmony = Harmony::Waltz;
            }
        }
        let slot = self.next_slot().expect("a bar to commit");
        let waltz = self.spans.back().expect("a span").waltz;
        if self.config.self_directed {
            self.direct(director::Events::default());
        }
        let intensity = (0.5 + self.energy).clamp(0.0, 1.0);
        self.energy *= ENERGY_DECAY;
        let shape = match (&self.waltz, waltz) {
            (Some(w), true) => w,
            _ => &self.shape,
        };
        let arrangement = self.arrangements[harmony_index(harmony)].as_ref().expect("checked");
        let freedom = [self.freedom.lead, self.freedom.comp, self.freedom.bass, self.freedom.drums];
        let chart = self.charts[waltz as usize].as_ref();
        let mut ctx = Ctx {
            bar: slot,
            shape,
            arrangement,
            chart,
            tuning,
            intensity,
            dynamics: self.freedom.dynamics,
            seed: self.config.seed ^ shape.song_hash,
            band: &self.prev_band,
            freedom,
            instruments: &self.instruments,
        };
        for m in &mut self.musicians {
            if replan || !m.current_plan().is_some_and(|p| p.covers(slot.index)) {
                m.plan(&ctx);
            }
        }
        // The band's plan for the bar: from the dials, the cues the musicians hold, the
        // harmony switching (a summon), the bar before.
        let intent = |r: Role| self.musicians[r as usize].current_plan().map(|p| p.intent(slot.index)).unwrap_or_default();
        let phrase = self.musicians[Role::Drums as usize].current_plan().copied().unwrap_or_else(|| PhrasePlan::shape_from(slot, shape, chart));
        let harm = |b: f64| arrangement.harm_at(slot.song_bar as f64 * shape.bar_beats + b);
        let band = BandPlan::decide(&BandInput {
            seed: self.config.seed ^ shape.song_hash,
            slot,
            bar_beats: shape.bar_beats,
            bars: shape.bars,
            phrase,
            freedom,
            harm: &harm,
            key: shape.key,
            switched: self.prev_harmony.is_some_and(|h| h != harmony),
            checkpoint: intent(Role::Drums).short_fill,
            death: intent(Role::Lead).wah,
            prev: &self.prev_band,
            waltz,
            looping: shape.looping,
            force_feel: self.forced_feel,
        });
        ctx.band = &band;
        self.scratch.clear();
        let mut orns = [Orns::default(); 4];
        for (m, o) in self.musicians.iter_mut().zip(&mut orns) {
            *o = m.commit_next_bar(&ctx, &mut self.scratch);
        }
        let events = self.scratch.len() as u32;
        for mut e in self.scratch.drain(..) {
            e.seq = self.seq;
            self.seq += 1;
            self.bank.push(e);
        }
        if self.committed.len() == self.committed.capacity() {
            self.committed.pop_front();
        }
        self.committed.push_back(CommittedBar { slot, harmony, tuning, intensity, events, orns, band });
        self.prev_band = band;
        self.prev_harmony = Some(harmony);
        self.next_bar += 1;
        self.next_song_bar += 1;
    }

    fn push_span(&mut self, s: Span) {
        if self.spans.len() == self.spans.capacity() {
            self.spans.pop_front();
        }
        self.spans.push_back(s);
    }

    /// Play time for the self-directed director: the engine's clock.
    fn play_secs(&self) -> f32 {
        (self.t as f64 / self.shape.sample_rate as f64) as f32
    }

    /// [`EngineConfig::self_directed`]: run the director on what just happened.
    fn direct(&mut self, ev: director::Events) {
        if let Some((f, _)) = self.band.step(self.play_secs(), ev) {
            self.filters = f;
        }
    }

    fn apply(&mut self, input: Input) {
        let ev = |f: fn(&mut director::Events)| {
            let mut e = director::Events::default();
            f(&mut e);
            e
        };
        let happened = match input {
            Input::Toot => {
                self.energy += ENERGY_TOOT;
                Some(ev(|e| e.toots = 1))
            }
            Input::Nugget => {
                self.energy += ENERGY_NUGGET;
                Some(ev(|e| e.nuggets = 1))
            }
            Input::Checkpoint => {
                self.energy += ENERGY_CHECKPOINT;
                Some(ev(|e| e.checkpoints = 1))
            }
            Input::Death => {
                self.energy += ENERGY_DEATH;
                Some(ev(|e| e.deaths = 1))
            }
            Input::WaltzStep => {
                self.energy += ENERGY_WALTZ_STEP;
                Some(ev(|e| e.waltz_steps = 1))
            }
            Input::Jump { on_ground: true } => Some(ev(|e| e.ground_jumps = 1)),
            Input::LevelStart | Input::Restart => {
                self.energy = 0.0;
                if self.config.self_directed {
                    self.filters = self.band.start(self.play_secs()).0;
                }
                None
            }
            Input::SetStats(s) => {
                self.stats = s;
                if self.config.self_directed {
                    self.filters = director::choose_filters(&s).0;
                }
                None
            }
            Input::SetFilters(f) => {
                self.filters = f;
                None
            }
            Input::ForceHarmony(h) => {
                self.forced_harmony = h;
                None
            }
            Input::ForceTuning(t) => {
                self.forced_tuning = t;
                None
            }
            Input::ForceFeel(f) => {
                self.forced_feel = f;
                None
            }
            Input::SetFreedom { lead, comp, bass, drums, dynamics } => {
                let c = |x: f32| if x.is_finite() { x.clamp(0.0, 1.0) } else { 0.0 };
                self.freedom = Freedom { lead: c(lead), comp: c(comp), bass: c(bass), drums: c(drums), dynamics: c(dynamics) };
                for (m, f) in self.musicians.iter_mut().zip([lead, comp, bass, drums]) {
                    m.set_freedom(c(f));
                }
                None
            }
            Input::SetMix(gains) => {
                for (ch, g) in gains.into_iter().enumerate() {
                    self.bank.set_channel_gain(ch, g);
                }
                None
            }
            Input::Jump { on_ground: false } | Input::Land { .. } => None,
        };
        if let Some(e) = happened
            && self.config.self_directed
        {
            // A toot in the air isn't a ground jump; the director counts both.
            self.direct(e);
            self.stats = self.band.stats;
        }
        self.energy = self.energy.clamp(-0.5, 0.5);
        for m in &mut self.musicians {
            m.on_input(&input, self.next_bar);
        }
    }

    /// Where the playhead is.
    pub fn position(&self) -> Position {
        let sp = self.span_at(self.t);
        let sh = self.shape_of(sp.waltz);
        let entry = sh.bar_starts[sp.entry];
        let s = (self.t.saturating_sub(sp.start) + entry).min(sh.len.saturating_sub(1));
        let song_bar = sh.bar_at(s).max(sp.entry);
        let beat = s.saturating_sub(sh.bar_starts[song_bar]) as f64 / sh.samples_per_beat;
        Position {
            sample: self.t,
            pass: sp.pass,
            bar: sp.first_bar + (song_bar - sp.entry) as u64,
            song_bar,
            beat,
            song_beat: song_bar as f64 * sh.bar_beats + beat,
        }
    }

    /// Song position and beat phase at the playhead (in the waltz: its 3/4 bars and beats).
    pub fn beat_clock(&self) -> BeatClock {
        let p = self.position();
        let sh = self.playing_shape();
        BeatClock {
            position: p,
            bpm: sh.bpm,
            beats_per_bar: sh.bar_beats,
            loop_beats: if sh.looping { sh.beats } else { 0.0 },
            beat_index: p.beat.floor() as u32,
            phase: p.beat - p.beat.floor(),
            sample_rate: sh.sample_rate,
        }
    }

    /// The committed bar at the playhead, if any.
    fn bar_at_playhead(&self) -> Option<&CommittedBar> {
        self.committed.iter().find(|b| b.slot.start <= self.t && self.t < b.slot.end)
    }

    /// A snapshot for UIs.
    pub fn state(&self) -> EngineState {
        let mut s = EngineState::default();
        self.state_into(&mut s);
        s
    }

    /// [`Engine::state`] into an existing snapshot (doesn't allocate once `upcoming` has room).
    pub fn state_into(&self, s: &mut EngineState) {
        let (h, t) = self.bar_at_playhead().map_or_else(|| self.current_filters(), |b| (b.harmony, b.tuning));
        let p = self.position();
        s.position = p;
        s.harmony = h;
        s.tuning = t;
        s.medley_phrase = (t == Tuning::Medley).then(|| self.medley.tuning_at(self.playing_shape().canon(p.song_beat)));
        s.filters = self.filters;
        s.forced_harmony = self.forced_harmony;
        s.forced_tuning = self.forced_tuning;
        s.forced_feel = self.forced_feel;
        s.feel = self.bar_at_playhead().map_or(Feel::Swing, |b| b.band.feel);
        s.freedom = self.freedom;
        s.stats = self.stats;
        s.upcoming.clear();
        s.upcoming.extend(self.committed.iter().filter(|b| b.slot.end > self.t).copied());
        for (m, st) in self.musicians.iter().zip(&mut s.musicians) {
            *st = MusicianState { role: m.role(), freedom: m.freedom(), plan: m.current_plan().copied() };
        }
        s.finished = self.finished();
    }

    /// Committed events that haven't started yet (for tests and UIs).
    pub fn pending_events(&self) -> impl Iterator<Item = &NoteEvent> {
        self.bank.pending()
    }

    /// A one-shot that has played out.
    pub fn finished(&self) -> bool {
        !self.shape.looping && self.next_slot().is_none() && self.t >= self.spans.back().expect("a span").end && self.bank.idle()
    }
}

fn harmony_index(h: Harmony) -> usize {
    Harmony::ALL.iter().position(|&x| x == h).expect("all harmonies")
}
