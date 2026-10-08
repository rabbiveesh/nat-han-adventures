//! The band: lead (pulse 1), comp (pulse 2), bass (triangle) and drums (noise), each behind
//! the [`Musician`] trait.
//!
//! A musician *plans* a phrase of 2, 4 or 8 bars toward a target (a cadence, the end of an
//! 8-bar section, the loop's end), and *commits* one bar at a time, a little ahead of the
//! playhead (see [`super::engine`]). A plan is an intention: inputs and dial changes may
//! replan the bars that aren't committed yet; a committed bar never changes.
//!
//! At freedom 0 every musician plays its written part, in the current harmony, exactly (the
//! engine then matches the offline renderer). Above 0, each has a tiny placeholder behaviour
//! that proves the plan / replan path, pending real improvisers:
//! - lead: grace notes (a semitone below, slurred) on long notes, more after a toot;
//! - comp: lays out for the second half of a phrase's last bar (room for the fill);
//! - bass: a chromatic approach into the next phrase on the phrase's last beat;
//! - drums: a snare-roll fill over the last two beats of a phrase; a crash after a death or a
//!   checkpoint.
//!
//! Dynamics: each bar has an intensity (0..1, from the gameplay, see [`super::engine`]); with
//! the dynamics dial up, musicians scale their volume with it and the drums accent the
//! downbeat.

use crate::audio::chart::{Chart, Family};
use crate::audio::mml::{Drum, Event, EventKind};
use crate::audio::tuning::{self, Tuning};

use super::arrange::{Arrangement, Shape};
use super::engine::Input;
use super::voice::{NoteEvent, Sound};

/// Who plays what.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    Lead,
    Comp,
    Bass,
    Drums,
}

impl Role {
    pub const ALL: [Role; 4] = [Role::Lead, Role::Comp, Role::Bass, Role::Drums];

    /// The channel it plays (0 pulse1, 1 pulse2, 2 triangle, 3 noise).
    pub fn channel(self) -> usize {
        self as usize
    }
}

/// A bar of the timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BarSlot {
    /// Absolute bar number since the engine started.
    pub index: u64,
    /// Loop pass, and bar within the song.
    pub pass: u64,
    pub song_bar: usize,
    /// Absolute samples: the bar, and its loop pass's start.
    pub start: u64,
    pub end: u64,
    pub loop_start: u64,
}

/// What a musician knows when it plans or commits a bar.
pub struct Ctx<'a> {
    pub bar: BarSlot,
    pub shape: &'a Shape,
    /// The song in the harmony this bar is played in.
    pub arrangement: &'a Arrangement,
    /// The original chart (phrase targets), if any.
    pub chart: Option<&'a Chart>,
    pub tuning: Tuning,
    /// This bar's intensity, 0..1.
    pub intensity: f32,
    /// The dynamics dial, 0..1.
    pub dynamics: f32,
    /// Seeds every free choice (deterministic for a given seed, bar and dial setting).
    pub seed: u64,
}

impl Ctx<'_> {
    /// Volume multiplier from the dynamics (exactly 1 with the dial at 0).
    pub fn gain(&self) -> f32 {
        if self.dynamics == 0.0 { 1.0 } else { 1.0 + self.dynamics * (self.intensity - 0.5) * 0.8 }
    }

    /// A committed event for written event `idx` of channel `ch`.
    fn event(&self, ch: usize, idx: usize, e: &Event, sound: Sound) -> NoteEvent {
        let ls = self.bar.loop_start;
        NoteEvent {
            ch: ch as u8,
            bar: self.bar.index,
            start: ls + self.shape.at(e.start),
            end: ls + self.shape.at(e.start + e.dur),
            sound,
            volume: e.volume,
            duty: e.duty,
            tie: e.tie,
            slur_out: false,
            gain: self.gain(),
            beat: e.start,
            loop_start: ls,
            salt: tuning::salt(self.shape.song_hash, ch, idx, 0),
            tuning: self.tuning,
            anchor: if ch < 3 { self.arrangement.anchors[ch] } else { 0 },
            seq: 0,
        }
    }

    /// The written part of channel `ch` in this bar (rests left out; drums only when audible).
    pub fn written(&self, ch: usize, out: &mut Vec<NoteEvent>) {
        let (from, to) = self.arrangement.bar_events[ch][self.bar.song_bar];
        let events = &self.arrangement.tracks[ch].events;
        for (idx, e) in events.iter().enumerate().take(to).skip(from) {
            let sound = match e.kind {
                EventKind::Note(n) => Sound::Note(n),
                EventKind::Arp(a) => Sound::Arp(a),
                EventKind::Drum(d) if e.volume > 0 => Sound::Drum(d),
                _ => continue,
            };
            out.push(self.event(ch, idx, e, sound));
        }
    }

    /// Beat within the bar of an absolute sample.
    pub fn beat_of(&self, sample: u64) -> f64 {
        (sample - self.bar.start) as f64 / self.shape.samples_per_beat
    }

    /// A deterministic coin in [0, 1) for decision `what` about this bar.
    pub fn chance(&self, role: Role, what: usize) -> f64 {
        unit(self.seed, role, self.bar.index, what)
    }
}

fn merge(i: &mut BarIntent, cue: BarIntent) {
    i.ornament = i.ornament.max(cue.ornament);
    i.fill |= cue.fill;
    i.accent |= cue.accent;
    i.answer |= cue.answer;
}

fn unit(seed: u64, role: Role, bar: u64, what: usize) -> f64 {
    (tuning::salt(seed, role as usize, bar as usize, what) >> 11) as f64 / (1u64 << 53) as f64
}

/// Where a phrase is heading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A dominant resolving home.
    Cadence,
    /// The end of an 8-bar section.
    SectionEnd,
    /// The end of the song (the loop point).
    LoopEnd,
}

/// What a musician means to do in one bar of its plan.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BarIntent {
    /// How much to ornament the line (lead), 0..1. Rolled from the freedom.
    pub ornament: f32,
    /// Fill (drums), lay out (comp) or approach (bass): the phrase-end gesture. Rolled.
    pub fill: bool,
    /// Cued: crash the downbeat (drums, after a death or a checkpoint).
    pub accent: bool,
    /// Cued: answer a toot (lead).
    pub answer: bool,
}

/// A musician's plan for the next phrase.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhrasePlan {
    /// First bar (absolute).
    pub start: u64,
    /// 2, 4 or 8 (or 1 at an odd section end).
    pub bars: u8,
    pub target: Target,
    pub intents: [BarIntent; 8],
}

impl PhrasePlan {
    pub fn covers(&self, bar: u64) -> bool {
        (self.start..self.start + self.bars as u64).contains(&bar)
    }

    pub fn last_bar(&self) -> u64 {
        self.start + self.bars as u64 - 1
    }

    pub fn intent(&self, bar: u64) -> BarIntent {
        if self.covers(bar) { self.intents[(bar - self.start) as usize] } else { BarIntent::default() }
    }

    /// The phrase from `bar` on: up to the next target, 8 bars at most.
    pub fn shape_from(bar: BarSlot, shape: &Shape, chart: Option<&Chart>) -> PhrasePlan {
        let b = bar.song_bar;
        let to_loop = shape.bars - b;
        let to_section = (8 - b % 8).min(to_loop);
        let mut bars = [8, 4, 2, 1].into_iter().find(|&l| l <= to_section).unwrap_or(1);
        // A cadence inside: arrive on it (a phrase ends on the bar before the tonic lands).
        if let Some(c) = chart
            && let Some(j) = (2..bars).find(|&j| cadence_at(c, shape, b + j))
        {
            bars = j;
        }
        let end = b + bars;
        let target = if end == shape.bars {
            Target::LoopEnd
        } else if end % 8 == 0 {
            Target::SectionEnd
        } else {
            Target::Cadence
        };
        PhrasePlan { start: bar.index, bars: bars as u8, target, intents: [BarIntent::default(); 8] }
    }
}

/// Does bar `song_bar` start on the home tonic, approached from a dominant?
fn cadence_at(chart: &Chart, shape: &Shape, song_bar: usize) -> bool {
    if song_bar == 0 || song_bar >= shape.bars {
        return false;
    }
    let t = song_bar as f64 * shape.bar_beats;
    let (prev, now) = (chart.at(t - 0.5), chart.at(t));
    prev.family() == Family::Dominant && now.root == shape.key && matches!(now.family(), Family::Major | Family::Minor)
}

/// A member of the band.
pub trait Musician: Send {
    fn role(&self) -> Role;

    /// 0 = the written part, exactly; 1 = as free as it gets.
    fn freedom(&self) -> f32;
    fn set_freedom(&mut self, freedom: f32);

    /// Plan the phrase starting at `ctx.bar` (and keep it).
    fn plan(&mut self, ctx: &Ctx) -> PhrasePlan;

    /// The plan in force, if any.
    fn current_plan(&self) -> Option<&PhrasePlan>;

    /// Commit `ctx.bar`: append its events (in start order) to `out`.
    fn commit_next_bar(&mut self, ctx: &Ctx, out: &mut Vec<NoteEvent>);

    /// React to an input. `next_bar` is the first bar not committed yet: only bars from there on
    /// may be replanned.
    fn on_input(&mut self, input: &Input, next_bar: u64);
}

/// What every musician shares: a role, a dial, a plan.
#[derive(Debug, Clone)]
pub struct Player {
    pub role: Role,
    pub freedom: f32,
    pub plan: Option<PhrasePlan>,
    pub seed: u64,
    /// A cue for a bar the plan doesn't reach yet: applied when it's planned.
    pub cue: Option<(u64, BarIntent)>,
}

impl Player {
    fn new(role: Role, seed: u64) -> Self {
        Player { role, freedom: 0.0, plan: None, seed, cue: None }
    }

    /// Roll this role's intents for the plan's bars from `from` on (cues are kept).
    fn roll(&self, plan: &mut PhrasePlan, from: u64) {
        let f = self.freedom as f64;
        for k in 0..plan.bars as u64 {
            let bar = plan.start + k;
            if bar < from {
                continue;
            }
            let last = bar == plan.last_bar();
            let i = &mut plan.intents[k as usize];
            i.fill = f > 0.0 && last && unit(self.seed, self.role, bar, 1) < 0.3 + 0.7 * f;
            i.ornament = if f > 0.0 { self.freedom } else { 0.0 };
        }
    }

    fn make_plan(&mut self, ctx: &Ctx) -> PhrasePlan {
        let mut plan = PhrasePlan::shape_from(ctx.bar, ctx.shape, ctx.chart);
        let start = plan.start;
        self.roll(&mut plan, start);
        if let Some((bar, cue)) = self.cue.take() {
            if plan.covers(bar) {
                merge(&mut plan.intents[(bar - plan.start) as usize], cue);
            } else if bar > plan.last_bar() {
                self.cue = Some((bar, cue));
            }
        }
        self.plan = Some(plan);
        plan
    }

    fn intent(&self, bar: u64) -> BarIntent {
        self.plan.map(|p| p.intent(bar)).unwrap_or_default()
    }

    /// Replan the uncommitted bars (after a dial change).
    fn reroll(&mut self, next_bar: u64) {
        if let Some(mut p) = self.plan {
            self.roll(&mut p, next_bar);
            self.plan = Some(p);
        }
    }

    /// Add `cue` to the intent of (uncommitted) bar `bar`: now if the plan covers it, else
    /// when it's planned.
    fn cue(&mut self, bar: u64, cue: BarIntent) {
        match self.plan.as_mut().filter(|p| p.covers(bar)) {
            Some(p) => merge(&mut p.intents[(bar - p.start) as usize], cue),
            None => self.cue = Some((bar, self.cue.filter(|c| c.0 == bar).map_or(cue, |(_, mut c)| {
                merge(&mut c, cue);
                c
            }))),
        }
    }

    fn on_input(&mut self, input: &Input, next_bar: u64) {
        match input {
            Input::SetFreedom { .. } => self.reroll(next_bar),
            Input::LevelStart | Input::Restart => {
                self.plan = None;
                self.cue = None;
            }
            _ => {}
        }
    }
}

macro_rules! musician_common {
    () => {
        fn role(&self) -> Role {
            self.0.role
        }
        fn freedom(&self) -> f32 {
            self.0.freedom
        }
        fn set_freedom(&mut self, freedom: f32) {
            self.0.freedom = freedom.clamp(0.0, 1.0);
        }
        fn plan(&mut self, ctx: &Ctx) -> PhrasePlan {
            self.0.make_plan(ctx)
        }
        fn current_plan(&self) -> Option<&PhrasePlan> {
            self.0.plan.as_ref()
        }
    };
}

/// Pulse 1: the tune.
pub struct Lead(pub Player);
/// Pulse 2: chords.
pub struct Comp(pub Player);
/// Triangle: the bass line.
pub struct Bass(pub Player);
/// Noise: the kit.
pub struct Drums(pub Player);

/// The band, in channel order.
pub fn band(seed: u64) -> [Box<dyn Musician>; 4] {
    [
        Box::new(Lead(Player::new(Role::Lead, seed))),
        Box::new(Comp(Player::new(Role::Comp, seed))),
        Box::new(Bass(Player::new(Role::Bass, seed))),
        Box::new(Drums(Player::new(Role::Drums, seed))),
    ]
}

/// Grace note length, in beats.
const GRACE: f64 = 0.125;

impl Musician for Lead {
    musician_common!();

    fn commit_next_bar(&mut self, ctx: &Ctx, out: &mut Vec<NoteEvent>) {
        let from = out.len();
        ctx.written(0, out);
        let p = &self.0;
        if p.freedom <= 0.0 {
            return;
        }
        // Grace notes: a semitone below, slurred into long notes on the beat.
        let intent = p.intent(ctx.bar.index);
        let ornament = if intent.answer { 1.0 } else { intent.ornament };
        let mut k = from;
        while k < out.len() {
            let e = out[k];
            let beat = ctx.beat_of(e.start);
            let long = (e.end - e.start) as f64 >= ctx.shape.samples_per_beat * 0.99;
            if let Sound::Note(n) = e.sound
                && long
                && !e.tie
                && (beat - beat.round()).abs() < 1e-6
                && n > 0
                && ctx.chance(Role::Lead, k - from) < (0.25 + 0.5 * ornament as f64)
            {
                let g = ctx.bar.loop_start + ctx.shape.at(e.beat + GRACE);
                let grace = NoteEvent { sound: Sound::Note(n - 1), end: g, salt: e.salt ^ 0x67_7261_6365, ..e };
                let main = NoteEvent { start: g, tie: true, ..e };
                out[k] = grace;
                out.insert(k + 1, main);
                k += 1;
            }
            k += 1;
        }
    }

    fn on_input(&mut self, input: &Input, next_bar: u64) {
        self.0.on_input(input, next_bar);
        if matches!(input, Input::Toot) {
            // Answer the toot in the next bar.
            self.0.cue(next_bar, BarIntent { answer: true, ..BarIntent::default() });
        }
    }
}

impl Musician for Comp {
    musician_common!();

    fn commit_next_bar(&mut self, ctx: &Ctx, out: &mut Vec<NoteEvent>) {
        let from = out.len();
        ctx.written(1, out);
        if self.0.freedom > 0.0 && self.0.intent(ctx.bar.index).fill {
            // Lay out for the second half of the phrase's last bar.
            let half = ctx.bar.start + ((ctx.bar.end - ctx.bar.start) / 2);
            let mut k = from;
            while k < out.len() {
                if out[k].start >= half {
                    out.remove(k);
                } else {
                    k += 1;
                }
            }
        }
    }

    fn on_input(&mut self, input: &Input, next_bar: u64) {
        self.0.on_input(input, next_bar);
    }
}

impl Musician for Bass {
    musician_common!();

    fn commit_next_bar(&mut self, ctx: &Ctx, out: &mut Vec<NoteEvent>) {
        let from = out.len();
        ctx.written(2, out);
        if self.0.freedom <= 0.0 || !self.0.intent(ctx.bar.index).fill {
            return;
        }
        // A chromatic approach into the next bar's first note, on the last beat.
        let next_bar = (ctx.bar.song_bar + 1) % ctx.shape.bars;
        let (lo, hi) = ctx.arrangement.bar_events[2][next_bar];
        let target = ctx.arrangement.tracks[2].events[lo..hi].iter().find_map(|e| match e.kind {
            EventKind::Note(n) => Some(n),
            _ => None,
        });
        let bb = ctx.shape.bar_beats;
        let last_beat = ctx.bar.loop_start + ctx.shape.at(ctx.bar.song_bar as f64 * bb + bb - 1.0);
        if let (Some(t), Some(last)) = (target, out[from..].last_mut())
            && last.start >= last_beat
            && matches!(last.sound, Sound::Note(_))
            && (1..127).contains(&t)
        {
            let above = ctx.chance(Role::Bass, 0) < 0.5;
            last.sound = Sound::Note(if above { t + 1 } else { t - 1 });
        }
    }

    fn on_input(&mut self, input: &Input, next_bar: u64) {
        self.0.on_input(input, next_bar);
    }
}

impl Musician for Drums {
    musician_common!();

    fn commit_next_bar(&mut self, ctx: &Ctx, out: &mut Vec<NoteEvent>) {
        let from = out.len();
        ctx.written(3, out);
        let p = &self.0;
        let intent = p.intent(ctx.bar.index);
        // Dynamics: accent the downbeat.
        if ctx.dynamics > 0.0 {
            let accent = 1.0 + ctx.dynamics * ctx.intensity * 0.3;
            for e in out[from..].iter_mut().filter(|e| e.start == ctx.bar.start) {
                e.gain *= accent;
            }
        }
        if p.freedom <= 0.0 {
            return;
        }
        let template = out.get(from).copied();
        let Some(template) = template else { return };
        if intent.accent {
            // Crash: an open hat on the one.
            match out[from..].iter_mut().find(|e| e.start == ctx.bar.start) {
                Some(e) => e.sound = Sound::Drum(Drum::OpenHat),
                None => {
                    let e = NoteEvent { sound: Sound::Drum(Drum::OpenHat), start: ctx.bar.start, end: ctx.bar.start + 1, ..template };
                    out.insert(from, e);
                }
            }
        }
        if intent.fill {
            // A snare roll over the last two beats, crescendo, kick on the last 16th.
            let bb = ctx.shape.bar_beats;
            let roll_from = ctx.bar.loop_start + ctx.shape.at(ctx.bar.song_bar as f64 * bb + bb - 2.0);
            out.truncate(from + out[from..].iter().take_while(|e| e.start < roll_from).count());
            for k in 0..8 {
                let beat = ctx.bar.song_bar as f64 * bb + bb - 2.0 + k as f64 * 0.25;
                let start = ctx.bar.loop_start + ctx.shape.at(beat);
                let end = ctx.bar.loop_start + ctx.shape.at(beat + 0.25);
                let drum = if k == 7 { Drum::Kick } else { Drum::Snare };
                let volume = (template.volume as i32 - 3 + k as i32 * 3 / 4).clamp(1, 15) as u8;
                out.push(NoteEvent { sound: Sound::Drum(drum), start, end, beat, volume, tie: false, ..template });
            }
        }
    }

    fn on_input(&mut self, input: &Input, next_bar: u64) {
        self.0.on_input(input, next_bar);
        if matches!(input, Input::Death | Input::Checkpoint) {
            self.0.cue(next_bar, BarIntent { accent: true, ..BarIntent::default() });
        }
    }
}
