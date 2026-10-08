//! The engine: a song, a band, a voice bank and a queue of inputs, rendering any block size in
//! real time. No Bevy, no game: [`Input`]s mirror what the game reports, so a native editor can
//! drive the same engine from dials.
//!
//! # Time
//! The playhead is an absolute sample count. Bars are numbered from 0 since the engine
//! started (a looping song keeps counting: bar `n` is bar `n % bars` of loop pass
//! `n / bars`). Bar lines fall where the offline renderer puts them (`round(beats ·
//! samples_per_beat)` within each pass).
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
//! a game can set [`EngineConfig::self_directed`], and the engine runs
//! [`director::choose_filters`] on every [`Input::SetStats`].

use std::collections::VecDeque;

use kira::Frame;

use crate::audio::chart::Chart;
use crate::audio::director::{self, PlayStats};
use crate::audio::synth::soft_clip;
use crate::audio::tuning::{Medley, Tuning};
use crate::audio::{Filters, Harmony};

use super::arrange::{Arrangement, Shape};
use super::musician::{self, BarSlot, Ctx, Musician, PhrasePlan, Role};
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
    /// A step on the waltz's beat (for the Waltz, once it lands).
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
    /// Each musician's freedom, and the dynamics, 0..1.
    SetFreedom { lead: f32, comp: f32, bass: f32, drums: f32, dynamics: f32 },
}

/// Engine settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EngineConfig {
    /// Seeds the generated accompaniment and every free choice.
    pub seed: u64,
    /// How far before its bar line a bar is committed, in beats.
    pub commit_lead_beats: f64,
    /// Run the director on [`Input::SetStats`] (for an editor without a game).
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
        let mut first_bar = self.position.bar as f64 - self.position.song_bar as f64;
        let bars_per_loop = (self.loop_beats / self.beats_per_bar).round();
        if self.loop_beats > 0.0 {
            let passes = (sb / self.loop_beats).floor();
            if first_bar + passes * bars_per_loop < 0.0 {
                return BeatClock { position: Position::default(), beat_index: 0, phase: 0.0, ..*self };
            }
            sb -= passes * self.loop_beats;
            first_bar += passes * bars_per_loop;
            c.position.pass = (first_bar / bars_per_loop.max(1.0)).round() as u64;
        }
        let sb = sb.max(0.0);
        let song_bar = (sb / self.beats_per_bar + 1e-9).floor();
        c.position.song_beat = sb;
        c.position.song_bar = song_bar as usize;
        c.position.bar = (first_bar + song_bar) as u64;
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
const ENERGY_DEATH: f32 = -0.25;
const ENERGY_DECAY: f32 = 0.8;

/// The live engine.
pub struct Engine {
    title: String,
    shape: Shape,
    chart: Option<Chart>,
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
    freedom: Freedom,
    stats: PlayStats,
    energy: f32,
    musicians: [Box<dyn Musician>; 4],
    committed: VecDeque<CommittedBar>,
    next_bar: u64,
    bank: VoiceBank,
    scratch: Vec<NoteEvent>,
    seq: u64,
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
        let arrangements = Harmony::ALL.map(|h| Arrangement::new(song, chart.as_ref(), h, config.seed, &shape).ok());
        if arrangements[0].is_none() {
            return Err(format!("\"{}\" can't be arranged", song.title));
        }
        let loop_secs = song.looping.then_some(shape.len as f64 / sample_rate as f32 as f64);
        let medley = Medley::new(shape.song_hash, shape.beats, loop_secs);
        Ok(Engine {
            title: song.title.clone(),
            bank: VoiceBank::new(sample_rate, medley.clone()),
            medley,
            shape,
            chart,
            arrangements,
            config,
            t: 0,
            inputs: VecDeque::with_capacity(256),
            filters: Filters::default(),
            forced_harmony: None,
            forced_tuning: None,
            freedom: Freedom::default(),
            stats: PlayStats::default(),
            energy: 0.0,
            musicians: musician::band(config.seed),
            committed: VecDeque::with_capacity(16),
            next_bar: 0,
            scratch: Vec::with_capacity(4096),
            seq: 0,
        })
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn shape(&self) -> &Shape {
        &self.shape
    }

    pub fn sample_rate(&self) -> u32 {
        self.shape.sample_rate
    }

    /// Can the song take this harmony (it has a chart)?
    pub fn can_play(&self, h: Harmony) -> bool {
        self.arrangements[harmony_index(h)].is_some()
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
            let pass_start = self.pass_start(self.t);
            if self.shape.looping {
                n = n.min(pass_start + self.shape.len - self.t);
            }
            let seg = &mut out[pos..pos + n as usize];
            seg.fill(Frame::ZERO);
            self.bank.render(seg, self.t, pass_start);
            for f in seg.iter_mut() {
                f.left = soft_clip(f.left);
                f.right = soft_clip(f.right);
            }
            self.t += n;
            pos += n as usize;
            while self.committed.len() > 1 && self.committed[0].slot.end <= self.t {
                self.committed.pop_front();
            }
        }
    }

    /// Start of the loop pass sample `t` is in.
    fn pass_start(&self, t: u64) -> u64 {
        if self.shape.looping { t - t % self.shape.len } else { 0 }
    }

    /// The bar `index` as a slot (`None` past a one-shot's end).
    fn slot(&self, index: u64) -> Option<BarSlot> {
        let (pass, song_bar) = self.shape.split(index);
        if !self.shape.looping && pass > 0 {
            return None;
        }
        let loop_start = pass * self.shape.len;
        Some(BarSlot {
            index,
            pass,
            song_bar,
            start: loop_start + self.shape.bar_starts[song_bar],
            end: loop_start + self.shape.bar_starts[song_bar + 1],
            loop_start,
        })
    }

    /// When the next bar gets committed.
    fn commit_point(&self) -> Option<u64> {
        let s = self.slot(self.next_bar)?;
        let lead = (self.config.commit_lead_beats * self.shape.samples_per_beat).round() as u64;
        Some(s.start.saturating_sub(lead))
    }

    /// The harmony and tuning a bar committed now gets.
    fn current_filters(&self) -> (Harmony, Tuning) {
        let mut h = self.forced_harmony.unwrap_or(self.filters.harmony);
        if !self.can_play(h) {
            h = Harmony::Original;
        }
        let t = self.forced_tuning.unwrap_or(if self.filters.just_intonation { Tuning::Medley } else { Tuning::Equal });
        (h, t)
    }

    fn commit(&mut self) {
        let Some(slot) = self.slot(self.next_bar) else { return };
        let (harmony, tuning) = self.current_filters();
        let intensity = (0.5 + self.energy).clamp(0.0, 1.0);
        self.energy *= ENERGY_DECAY;
        let arrangement = self.arrangements[harmony_index(harmony)].as_ref().expect("checked");
        let ctx = Ctx {
            bar: slot,
            shape: &self.shape,
            arrangement,
            chart: self.chart.as_ref(),
            tuning,
            intensity,
            dynamics: self.freedom.dynamics,
            seed: self.config.seed,
        };
        self.scratch.clear();
        for m in &mut self.musicians {
            if !m.current_plan().is_some_and(|p| p.covers(slot.index)) {
                m.plan(&ctx);
            }
            m.commit_next_bar(&ctx, &mut self.scratch);
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
        self.committed.push_back(CommittedBar { slot, harmony, tuning, intensity, events });
        self.next_bar += 1;
    }

    fn apply(&mut self, input: Input) {
        match input {
            Input::Toot => self.energy += ENERGY_TOOT,
            Input::Nugget => self.energy += ENERGY_NUGGET,
            Input::Checkpoint => self.energy += ENERGY_CHECKPOINT,
            Input::Death => self.energy += ENERGY_DEATH,
            Input::LevelStart | Input::Restart => self.energy = 0.0,
            Input::SetStats(s) => {
                self.stats = s;
                if self.config.self_directed {
                    self.filters = director::choose_filters(&s).0;
                }
            }
            Input::SetFilters(f) => self.filters = f,
            Input::ForceHarmony(h) => self.forced_harmony = h,
            Input::ForceTuning(t) => self.forced_tuning = t,
            Input::SetFreedom { lead, comp, bass, drums, dynamics } => {
                let c = |x: f32| if x.is_finite() { x.clamp(0.0, 1.0) } else { 0.0 };
                self.freedom = Freedom { lead: c(lead), comp: c(comp), bass: c(bass), drums: c(drums), dynamics: c(dynamics) };
                for (m, f) in self.musicians.iter_mut().zip([lead, comp, bass, drums]) {
                    m.set_freedom(c(f));
                }
            }
            Input::Jump { .. } | Input::Land { .. } | Input::WaltzStep => {}
        }
        self.energy = self.energy.clamp(-0.5, 0.5);
        for m in &mut self.musicians {
            m.on_input(&input, self.next_bar);
        }
    }

    /// Where the playhead is.
    pub fn position(&self) -> Position {
        let sh = &self.shape;
        let ps = self.pass_start(self.t);
        let s = (self.t - ps).min(sh.len.saturating_sub(1));
        let pass = if sh.looping { self.t / sh.len } else { 0 };
        let song_bar = sh.bar_at(s);
        let beat = (s - sh.bar_starts[song_bar]) as f64 / sh.samples_per_beat;
        Position {
            sample: self.t,
            pass,
            bar: pass * sh.bars as u64 + song_bar as u64,
            song_bar,
            beat,
            song_beat: song_bar as f64 * sh.bar_beats + beat,
        }
    }

    /// Song position and beat phase at the playhead.
    pub fn beat_clock(&self) -> BeatClock {
        let p = self.position();
        BeatClock {
            position: p,
            bpm: self.shape.bpm,
            beats_per_bar: self.shape.bar_beats,
            loop_beats: if self.shape.looping { self.shape.beats } else { 0.0 },
            beat_index: p.beat.floor() as u32,
            phase: p.beat - p.beat.floor(),
            sample_rate: self.shape.sample_rate,
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
        s.medley_phrase = (t == Tuning::Medley).then(|| self.medley.tuning_at(p.song_beat));
        s.filters = self.filters;
        s.forced_harmony = self.forced_harmony;
        s.forced_tuning = self.forced_tuning;
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
        !self.shape.looping && self.slot(self.next_bar).is_none() && self.t >= self.shape.len && self.bank.idle()
    }
}

fn harmony_index(h: Harmony) -> usize {
    Harmony::ALL.iter().position(|&x| x == h).expect("all harmonies")
}
