//! The band: lead (pulse 1), comp (pulse 2), bass (triangle) and drums (noise), each behind
//! the [`Musician`] trait.
//!
//! A musician *plans* a phrase of 2, 4 or 8 bars toward a target (a cadence, the end of an
//! 8-bar section, the loop's end), and *commits* one bar at a time, a little ahead of the
//! playhead (see [`super::engine`]). A plan is an intention: inputs and dial changes may
//! replan the bars that aren't committed yet; a committed bar never changes.
//!
//! # Freedom and the vocabulary
//! At freedom 0 every musician plays its written part, in the current harmony, exactly (the
//! engine then matches the offline renderer). Above 0 each draws on its ornaments
//! ([`super::ornament::Orn`]), in three tiers that fade in with the dial ([`band::low`] from
//! 0.05, [`band::mid`] from 0.3, [`band::high`] from 0.6). The plan *rolls* which ornaments
//! each bar means to try (seeded: the same seed, bar and dial always roll the same), and the
//! commit realizes those that fit the bar (a turn needs a long note, a pickup a rest before
//! the next phrase), against the harmony in force ([`Ctx::harm_at`]: the filter's chart, the
//! band's reharmonization), and reports what it played ([`Orns`]).
//! - lead ([`lead`]): grace notes, vibrato, fall-offs, NES echoes (low); turns, mordents,
//!   enclosures, swung pickups, octave displacement, side-slipping, slides, duty sweeps
//!   (mid); planing, digital patterns, pentatonic superimposition, displacement and hemiola,
//!   run fills, arpeggio flourishes and triplet runs, solo choruses (high). Up to 0.5 the head
//!   stays mostly as written: the low and mid ornaments decorate notes, they don't replace
//!   them.
//! - comp ([`comp`]): anticipations and the odd extra stab (low); Charleston and Freddie Green
//!   patterns, moving voicings, hits with the bass (mid); reharmonization with the bass,
//!   side-slipping and planed voicings, rising fourths, polychords (high).
//! - bass ([`bass`]): chromatic approaches, hits (low); walking, two-feel, pedal points,
//!   tritone-sub roots (mid); fills, ostinato vamps, side-slipping walks (high).
//! - drums ([`drums`]): ghost snares, open hats (low); fills every 4 and 8 bars, crashes after
//!   them, kicks on the band's hits (mid); trading fours, press rolls, broken time (high).
//!
//! What the band must agree on (hits, reharmonizations, trading, fills, the game's big moments)
//! is the bar's [`BandPlan`], decided once for all four ([`super::band`]). Musicians may also
//! switch instrument for a phrase or a fill (their channel's palette,
//! [`super::instrument::Instruments::palette`]) and switch back.
//!
//! Dynamics: each bar has an intensity (0..1, from the gameplay, see [`super::engine`]); with
//! the dynamics dial up, musicians scale their volume with it and the drums accent the
//! downbeat.
//!
//! # Choruses, intros, endings
//! The bar's chorus ([`super::chorus`], [`BandPlan::chorus`]) decides each player's role in it
//! (the table in [`super::chorus`]): [`arranged`] turns the rolled ornaments into what the
//! chorus asks (the bass walks or plays in two, the comp Charlestons, the drums play lighter,
//! the lead keeps to the tune for a soli), and each commit plays the chorus's own parts: the
//! lead's solos, breaks and shout line, the comp's riffs, soli line and shout stabs, the
//! band's stop-time hits and breaks. Intros and endings are the same: their changes are the
//! band's substitutions, and each player has its part (the lead lays out and picks up into the
//! head; the plinks are the comp's; everyone plays the last chord). These are arrangements the
//! whole band follows, so they play at freedom 0 too ([`BandPlan::arranged`]), plainly.
//!
//! # Feels
//! The band's feel ([`super::feel`], [`BandPlan::feel`], decided for the whole band at once in
//! [`BandPlan::decide`]) is realized in each player's commit as a rhythm transform of the bar
//! (each player's `feel_*` pattern), on the feel's instrument ([`Ctx::feel_inst`], from the
//! feel's palette; a phrase's [`BarIntent::switch`] picks another entry of it). Every feel is
//! straight: [`Ctx::swing8`] stops swinging, so the ornaments go straight too, and the lead
//! un-swings its line ([`Ctx::straighten`]). A forced feel plays at freedom 0 too (plainly:
//! nothing else is rolled); the band's own never does.

mod bass;
mod comp;
mod drums;
mod lead;

pub use bass::Bass;
pub use comp::Comp;
pub use drums::Drums;
pub use lead::Lead;

use crate::audio::accomp::Rng;
use crate::audio::chart::{Chart, Family};
use crate::audio::mml::{Event, EventKind};
use crate::audio::tuning::{self, Tuning};

use super::arrange::{Arrangement, Shape};
use super::band::{self, BandPlan};
use super::engine::Input;
use super::feel::{self, Feel};
use super::instrument::Instruments;
use super::ornament::{Harm, Orn, Orns};
use super::voice::{Fx, NoteEvent, Sound};

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
    /// The band's shared plan for this bar.
    pub band: &'a BandPlan,
    /// Every musician's freedom (lead, comp, bass, drums).
    pub freedom: [f32; 4],
    /// The song's instruments (palettes).
    pub instruments: &'a Instruments,
}

impl Ctx<'_> {
    /// Volume multiplier from the dynamics (exactly 1 with the dial at 0).
    pub fn gain(&self) -> f32 {
        if self.dynamics == 0.0 { 1.0 } else { 1.0 + self.dynamics * (self.intensity - 0.5) * 0.8 }
    }

    /// Absolute sample of song beat `beat` (of this bar's shape and loop pass), placed from the
    /// bar line (a pass entered mid-song at a meter switch starts at that bar line).
    #[inline]
    pub fn at(&self, beat: f64) -> u64 {
        let line = self.shape.bar_starts[self.bar.song_bar];
        (self.bar.start + self.shape.at(beat)).saturating_sub(line)
    }

    /// A committed event for written event `idx` of channel `ch`.
    fn event(&self, ch: usize, idx: usize, e: &Event, sound: Sound) -> NoteEvent {
        let ls = self.bar.loop_start;
        NoteEvent {
            ch: ch as u8,
            bar: self.bar.index,
            start: self.at(e.start),
            end: self.at(e.start + e.dur),
            sound,
            volume: e.volume,
            duty: e.duty,
            tie: e.tie,
            slur_out: false,
            gain: self.gain(),
            beat: e.start,
            phrase_beat: self.shape.canon(e.start),
            waltz: self.shape.waltz,
            loop_start: ls,
            salt: tuning::salt(self.shape.song_hash, ch, idx, 0),
            tuning: self.tuning,
            anchor: if ch < 3 { self.arrangement.anchors[ch] } else { 0 },
            inst: e.inst,
            fx: Fx::NONE,
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
            if out.len() < out.capacity() {
                out.push(self.event(ch, idx, e, sound));
            }
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

    /// Which way this bar's side-slip anticipations go (+1 or -1 semitone): the same for the
    /// comp and the bass, so the two slip together.
    pub fn slip_dir(&self) -> i32 {
        if unit(self.seed, Role::Comp, self.bar.index, 97) < 0.5 { 1 } else { -1 }
    }

    /// A seeded RNG for this bar's decision `what`.
    pub fn rng(&self, role: Role, what: u64) -> Rng {
        band::rng(self.seed, role as usize, self.bar.index, what)
    }

    /// The song beat of the bar line.
    pub fn line(&self) -> f64 {
        self.bar.song_bar as f64 * self.shape.bar_beats
    }

    /// Beats per bar.
    pub fn bb(&self) -> f64 {
        self.shape.bar_beats
    }

    /// An event's start, in beats from the bar line.
    pub fn rel(&self, e: &NoteEvent) -> f64 {
        e.beat - self.line()
    }

    /// An event's length in beats.
    pub fn len(&self, e: &NoteEvent) -> f64 {
        (e.end - e.start) as f64 / self.shape.samples_per_beat
    }

    /// The harmony at beat `b` of the bar (from the bar line, may run into the next bar): the
    /// band's reharmonization if it has one there, else the filter's chart.
    pub fn harm_at(&self, b: f64) -> Option<Harm> {
        match self.band.sub_at(b) {
            Some(s) => Some(Harm::new(s.chord)),
            None => self.plain_harm_at(b),
        }
    }

    /// Is beats `from`..`to` of the bar one dominant chord all through (as played, a
    /// reharmonization included), and at least two beats long? Where side-slips go.
    pub fn long_dominant(&self, from: f64, to: f64) -> bool {
        let Some(h) = self.harm_at(from) else { return false };
        let same = |b: f64| self.harm_at(b).is_some_and(|x| x.chord == h.chord);
        to - from >= 2.0 - 1e-9 && h.chord.family() == crate::audio::chart::Family::Dominant && (1..4).all(|k| same(from + (to - from) * k as f64 / 4.0)) && same(to - 0.01)
    }

    /// The filter's harmony at beat `b` of the bar (no reharmonization).
    pub fn plain_harm_at(&self, b: f64) -> Option<Harm> {
        self.arrangement.harm_at(self.line() + b)
    }

    /// Where an 8th written at beat `b` of the bar starts, swung like the song (straight in a
    /// feel: every feel is straight).
    pub fn swing8(&self, b: f64) -> f64 {
        if self.band.feel != Feel::Swing {
            return b;
        }
        let k = (b * 2.0).round();
        if (k * 0.5 - b).abs() < 1e-9 && (k as i64) % 2 == 1 { b + self.arrangement.swing.clamp(0.0, 0.9) as f64 * 0.5 } else { b }
    }

    /// The bar's feel.
    pub fn feel(&self) -> Feel {
        self.band.feel
    }

    /// A written (swung) event un-swung: off-beat 8ths back on the 8th (the feels are straight).
    pub fn straighten(&self, e: &mut NoteEvent) {
        let delay = self.arrangement.swing.clamp(0.0, 0.9) as f64 * 0.5;
        if delay == 0.0 {
            return;
        }
        let un = |x: f64| {
            let k = x.floor();
            if (x - k - 0.5 - delay).abs() < 1e-6 { k + 0.5 } else { x }
        };
        let line = self.line();
        let (b, z) = (un(self.rel(e)), un(self.rel(e) + self.len(e)));
        e.start = self.at(line + b);
        e.end = self.at(line + z).max(e.start + 1);
        e.beat = line + b;
        e.phrase_beat = self.shape.canon(e.beat);
    }

    /// The instrument channel `ch` plays this bar's feel on: the feel palette's pick for the
    /// feel (fixed while it lasts), or with `alt` another entry (a phrase's switch). `None` in
    /// the tune's own feel.
    pub fn feel_inst(&self, ch: usize, alt: bool) -> Option<u8> {
        let p = self.instruments.feel_palette(self.band.feel, ch);
        if p.is_empty() {
            return None;
        }
        let k = feel::pick(self.seed, ch, self.band.feel_since, p.len()) + alt as usize;
        Some(p[k % p.len()])
    }

    /// A player's extra feel sound ([`feel::Extra`]).
    pub fn extra(&self, x: feel::Extra) -> u8 {
        self.instruments.feel_extras[x as usize]
    }

    /// A new event like `t` (its volume, duty, instrument, tuning), at beat `b` of the bar for
    /// `d` beats.
    pub fn make(&self, t: &NoteEvent, b: f64, d: f64, sound: Sound) -> NoteEvent {
        let beat = self.line() + b;
        let salt = t.salt ^ ((b * 960.0).round() as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x6f72_6e;
        NoteEvent {
            ch: t.ch,
            bar: self.bar.index,
            start: self.at(beat),
            end: self.at(beat + d.max(0.01)),
            sound,
            tie: false,
            slur_out: false,
            beat,
            phrase_beat: self.shape.canon(beat),
            salt,
            fx: Fx::NONE,
            seq: 0,
            ..*t
        }
    }

    /// The first note channel `ch` writes in the next bar (in this harmony).
    pub fn next_first_note(&self, ch: usize) -> Option<u8> {
        let nb = (self.bar.song_bar + 1) % self.shape.bars;
        let (lo, hi) = self.arrangement.bar_events[ch][nb];
        self.arrangement.tracks[ch].events[lo..hi].iter().find_map(|e| match e.kind {
            EventKind::Note(n) => Some(n),
            EventKind::Arp(a) => a.notes().last().copied(),
            _ => None,
        })
    }

    /// A template for the events a musician on channel `ch` makes up in this bar: `from` (an
    /// event it played, this bar's or an earlier one's) brought up to date, or a plain one.
    pub fn template(&self, ch: usize, from: Option<NoteEvent>) -> NoteEvent {
        let mut t = from.unwrap_or(NoteEvent {
            ch: ch as u8,
            bar: self.bar.index,
            start: self.bar.start,
            end: self.bar.start + 1,
            sound: if ch == 3 { Sound::Drum(crate::audio::mml::Drum::Kick) } else { Sound::Note(60) },
            volume: [12, 7, 12, 9][ch.min(3)],
            duty: 2,
            tie: false,
            slur_out: false,
            gain: 1.0,
            beat: self.line(),
            phrase_beat: self.shape.canon(self.line()),
            waltz: false,
            loop_start: 0,
            salt: tuning::salt(self.shape.song_hash, ch, 1 << 20, 0),
            tuning: self.tuning,
            anchor: 0,
            inst: self.palette(ch).first().copied().unwrap_or(0),
            fx: Fx::NONE,
            seq: 0,
        });
        t.bar = self.bar.index;
        t.loop_start = self.bar.loop_start;
        t.waltz = self.shape.waltz;
        t.tuning = self.tuning;
        t.anchor = if ch < 3 { self.arrangement.anchors[ch] } else { 0 };
        t.gain = self.gain();
        t.tie = false;
        t.fx = Fx::NONE;
        t
    }

    /// Channel `ch`'s palette (instrument numbers; the first is the base).
    pub fn palette(&self, ch: usize) -> &[u8] {
        self.instruments.palette(ch)
    }

    /// The alternate instrument a musician on channel `ch` switches to for the phrase starting
    /// at `phrase` (`None` if the palette has no alternate to `current`).
    pub fn alternate(&self, ch: usize, phrase: u64, current: u8) -> Option<u8> {
        let p = self.palette(ch);
        let alts = p.iter().filter(|&&i| i != current).count();
        if alts == 0 {
            return None;
        }
        let k = (tuning::salt(self.seed, ch, phrase as usize, 77) % alts as u64) as usize;
        p.iter().copied().filter(|&i| i != current).nth(k)
    }
}

/// The ornaments `orns` a player rolled, made to fit the bar's chorus, intro or ending (see the
/// module docs).
pub fn arranged(role: Role, band: &BandPlan, orns: Orns) -> Orns {
    use super::chorus::{Chorus, EndStep, IntroKind};
    let mut o = orns;
    let drop = |o: &mut Orns, list: &[Orn]| {
        for x in list {
            o.0 &= !x.bit();
        }
    };
    let chorus = band.chorus;
    let time = band.ending.is_some_and(|e| e.step() == EndStep::Time);
    match role {
        Role::Lead => match chorus {
            // Tight with the comp: the tune as written, only vibrato on it.
            Chorus::Soli => o = Orns(o.0 & Orn::Vibrato.bit()),
            // The shout line is the tune: no whole-bar transformations.
            Chorus::Shout => drop(
                &mut o,
                &[
                    Orn::Planing,
                    Orn::Digital,
                    Orn::Pentatonic,
                    Orn::Displace,
                    Orn::Hemiola,
                    Orn::SideSlip,
                    Orn::Octave,
                ],
            ),
            _ => {}
        },
        Role::Comp => {
            let charleston = chorus == Chorus::TwoFeel
                || time
                || band.intro.is_some_and(|i| i.kind == IntroKind::Vamp);
            let quarters = band.intro.is_some_and(|i| i.kind == IntroKind::Pedal);
            if charleston || quarters {
                drop(&mut o, &[Orn::Charleston, Orn::FreddieGreen, Orn::Fourths]);
                o.add(if charleston {
                    Orn::Charleston
                } else {
                    Orn::FreddieGreen
                });
            }
        }
        Role::Bass => {
            let two = chorus == Chorus::TwoFeel;
            let pedal = band.intro.is_some_and(|i| i.kind == IntroKind::Pedal);
            let walk = (chorus.walks() && band.intro.is_none())
                || time
                || band.intro.is_some_and(|i| i.kind == IntroKind::Vamp);
            if two || pedal || walk {
                drop(
                    &mut o,
                    &[Orn::Walking, Orn::TwoFeel, Orn::Pedal, Orn::Ostinato],
                );
                o.add(if pedal {
                    Orn::Pedal
                } else if two {
                    Orn::TwoFeel
                } else {
                    Orn::Walking
                });
            }
        }
        Role::Drums => {
            if matches!(chorus, Chorus::TwoFeel | Chorus::Strolling) || band.intro.is_some() {
                drop(&mut o, &[Orn::Ghost, Orn::OpenHat, Orn::BrokenTime]);
            }
            if chorus == Chorus::Shout || band.ending.is_some() {
                drop(&mut o, &[Orn::BrokenTime]);
            }
        }
    }
    o
}

fn merge(i: &mut BarIntent, cue: BarIntent) {
    i.ornament = i.ornament.max(cue.ornament);
    i.fill |= cue.fill;
    i.accent |= cue.accent;
    i.answer |= cue.answer;
    i.wah |= cue.wah;
    i.short_fill |= cue.short_fill;
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
    /// How much to ornament the line, 0..1 (the freedom it was rolled at).
    pub ornament: f32,
    /// The phrase-end gesture: a fill (drums), laying out for it (comp), an approach or fill
    /// (bass), a run (lead).
    pub fill: bool,
    /// Cued: crash the downbeat (drums, after a checkpoint).
    pub accent: bool,
    /// Cued: answer a toot (lead).
    pub answer: bool,
    /// Cued: the comic wah-wah (lead, after a death).
    pub wah: bool,
    /// Cued: a short fill (drums, at a checkpoint).
    pub short_fill: bool,
    /// The ornaments planned for this bar: what the musician means to try.
    pub orns: Orns,
    /// Switch to an alternate instrument from the palette for this bar.
    pub switch: bool,
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
    /// Its first bar within the song, the loop pass, and the song's length in bars.
    pub song_bar: u16,
    pub pass: u32,
    pub song_bars: u16,
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

    /// Every ornament planned over the phrase.
    pub fn orns(&self) -> Orns {
        Orns(self.intents[..self.bars as usize].iter().fold(0u128, |a, i| a | i.orns.0))
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
        } else if end.is_multiple_of(8) {
            Target::SectionEnd
        } else {
            Target::Cadence
        };
        PhrasePlan {
            start: bar.index,
            bars: bars as u8,
            target,
            intents: [BarIntent::default(); 8],
            song_bar: b as u16,
            pass: bar.pass as u32,
            song_bars: shape.bars as u16,
        }
    }
}

/// Does bar `song_bar` start on the home tonic, approached from a dominant?
fn cadence_at(chart: &Chart, shape: &Shape, song_bar: usize) -> bool {
    if song_bar == 0 || song_bar >= shape.bars || chart.meter as f64 != shape.bar_beats {
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

    /// Commit `ctx.bar`: append its events (in start order) to `out`, and say which ornaments
    /// it played.
    fn commit_next_bar(&mut self, ctx: &Ctx, out: &mut Vec<NoteEvent>) -> Orns;

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
        let f = self.freedom;
        let (lo, md, hi) = (band::low(f), band::mid(f), band::high(f));
        let role = self.role;
        // Phrase-wide choices: a pattern, a feel, an instrument.
        let mut pr = band::rng(self.seed, role as usize, plan.start, 30);
        let (pick, pick2) = (pr.f(), pr.f());
        let switch = f > 0.0 && pr.chance(0.25 * md + 0.35 * hi);
        for k in 0..plan.bars as u64 {
            let bar = plan.start + k;
            if bar < from {
                continue;
            }
            let (first, last) = (k == 0, bar == plan.last_bar());
            let song_bar = plan.song_bar as usize + k as usize;
            let mut r = band::rng(self.seed, role as usize, bar, 31);
            let mut o = Orns::default();
            macro_rules! want {
                ($orn:expr, $p:expr) => {
                    if r.f() < $p {
                        o.add($orn);
                    }
                };
            }
            let i = &mut plan.intents[k as usize];
            match role {
                Role::Lead => {
                    want!(Orn::Grace, 0.75 * lo);
                    want!(Orn::Vibrato, 0.85 * lo);
                    want!(Orn::Echo, 0.6 * lo);
                    want!(Orn::Slide, 0.6 * md);
                    want!(Orn::DutySweep, 0.45 * md);
                    want!(Orn::Turn, 0.4 * md);
                    want!(Orn::Mordent, 0.4 * md);
                    want!(Orn::Enclosure, 0.45 * md);
                    want!(Orn::ArpFlourish, 0.5 * hi);
                    if last {
                        want!(Orn::FallOff, 0.85 * lo);
                        want!(Orn::Pickup, 0.8 * md);
                        want!(Orn::RunFill, 0.7 * hi);
                        want!(Orn::TripletRun, 0.45 * hi);
                    } else {
                        want!(Orn::Planing, 0.3 * hi);
                        want!(Orn::Digital, 0.35 * hi);
                        want!(Orn::Pentatonic, 0.4 * hi);
                        want!(Orn::Displace, 0.12 * hi);
                        want!(Orn::Hemiola, 0.15 * hi);
                    }
                    if !first && !last {
                        want!(Orn::Octave, 0.18 * md);
                        want!(Orn::SideSlip, 0.2 * md);
                    }
                    i.fill = o.has(Orn::RunFill) || o.has(Orn::Pickup);
                }
                Role::Comp => {
                    want!(Orn::ExtraStab, 0.35 * lo);
                    want!(Orn::MovingVoicing, 0.6 * md);
                    if !last && pick < 0.55 * md {
                        o.add(if pick2 < 0.5 { Orn::Charleston } else { Orn::FreddieGreen });
                    }
                    want!(Orn::Polychord, 0.35 * hi);
                    if !last && pick2 < 0.3 * hi {
                        o.add(Orn::Fourths);
                    }
                    if last {
                        want!(Orn::PlaneChords, 0.4 * hi);
                        want!(Orn::SlipVoicing, 0.4 * hi);
                    }
                    i.fill = last && f > 0.0;
                }
                Role::Bass => {
                    want!(Orn::Approach, 0.75 * lo);
                    if pick < 0.3 * hi {
                        o.add(Orn::Ostinato);
                    } else if pick < 0.6 * md {
                        o.add(if pick2 < 0.6 {
                            Orn::Walking
                        } else if pick2 < 0.85 {
                            Orn::TwoFeel
                        } else {
                            Orn::Pedal
                        });
                    }
                    if o.has(Orn::Walking) {
                        want!(Orn::SlipWalk, 0.3 * hi);
                    }
                    if last {
                        want!(Orn::BassFill, 0.6 * hi);
                        want!(Orn::SlipBass, 0.4 * hi);
                    }
                    i.fill = last && f > 0.0;
                }
                Role::Drums => {
                    want!(Orn::Ghost, 0.7 * lo);
                    want!(Orn::OpenHat, 0.6 * lo);
                    if !last && pick < 0.4 * hi {
                        o.add(Orn::BrokenTime);
                    }
                    i.fill = band::fill_for(self.seed, bar, song_bar % plan.song_bars.max(1) as usize, plan.song_bars as usize, last, f)
                        != band::Fill::None;
                }
            }
            i.orns = o;
            i.switch = switch;
            i.ornament = if f > 0.0 { f } else { 0.0 };
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

    /// Is `bar` the last of the phrase planned?
    fn phrase_last(&self, bar: u64) -> bool {
        self.plan.is_some_and(|p| p.last_bar() == bar)
    }

    /// Replan the uncommitted bars (after a dial change).
    fn reroll(&mut self, next_bar: u64) {
        if let Some(mut p) = self.plan {
            // (Cues are kept: rolling never touches them.)
            self.roll(&mut p, next_bar);
            self.plan = Some(p);
        }
    }

    /// Add `cue` to the intent of (uncommitted) bar `bar`: now if the plan covers it, else
    /// when it's planned.
    fn cue(&mut self, bar: u64, cue: BarIntent) {
        match self.plan.as_mut().filter(|p| p.covers(bar)) {
            Some(p) => merge(&mut p.intents[(bar - p.start) as usize], cue),
            None => {
                self.cue = Some((
                    bar,
                    self.cue.filter(|c| c.0 == bar).map_or(cue, |(_, mut c)| {
                        merge(&mut c, cue);
                        c
                    }),
                ))
            }
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
            self.p.role
        }
        fn freedom(&self) -> f32 {
            self.p.freedom
        }
        fn set_freedom(&mut self, freedom: f32) {
            self.p.freedom = freedom.clamp(0.0, 1.0);
        }
        fn plan(&mut self, ctx: &Ctx) -> PhrasePlan {
            self.p.make_plan(ctx)
        }
        fn current_plan(&self) -> Option<&PhrasePlan> {
            self.p.plan.as_ref()
        }
    };
}
use musician_common;

/// The range a melodic channel's notes are folded into (lead, comp, bass).
pub fn range(ch: usize) -> (i32, i32) {
    match ch {
        0 => (lead::LO, lead::HI),
        1 => (comp::LO, comp::HI),
        _ => (bass::LO - 4, bass::HI + 4),
    }
}

/// The band, in channel order.
pub fn band(seed: u64) -> [Box<dyn Musician>; 4] {
    [
        Box::new(Lead::new(Player::new(Role::Lead, seed))),
        Box::new(Comp::new(Player::new(Role::Comp, seed))),
        Box::new(Bass::new(Player::new(Role::Bass, seed))),
        Box::new(Drums::new(Player::new(Role::Drums, seed))),
    ]
}

/// Events a player works on in one bar (allocated once: commits never allocate).
const WORK: usize = 192;

fn work() -> Vec<NoteEvent> {
    Vec::with_capacity(WORK)
}

/// Push without ever growing the buffer (a full bar drops the rest).
#[inline]
fn push(v: &mut Vec<NoteEvent>, e: NoteEvent) {
    if v.len() < v.capacity() {
        v.push(e);
    }
}

/// Sort by start (stable), drop zero-length events and trim overlaps (a monophonic voice:
/// each event ends where the next starts).
fn tidy(v: &mut Vec<NoteEvent>) {
    // Insertion sort: small, stable, no allocation.
    for i in 1..v.len() {
        let mut j = i;
        while j > 0 && v[j - 1].start > v[j].start {
            v.swap(j - 1, j);
            j -= 1;
        }
    }
    // Two events on the same sample: the one added last wins.
    let mut w = 0;
    for i in 0..v.len() {
        let keep = v[i].end > v[i].start && !(v[i].ch != 3 && i + 1 < v.len() && v[i + 1].start == v[i].start);
        if keep {
            v[w] = v[i];
            w += 1;
        }
    }
    v.truncate(w);
    let n = v.len();
    for i in 0..n.saturating_sub(1) {
        let next = v[i + 1].start;
        if v[i].end > next && v[i].ch != 3 {
            v[i].end = next;
        }
    }
    v.retain(|e| e.end > e.start);
}

/// `note` folded into `lo..=hi` by octaves.
fn fold(note: i32, lo: i32, hi: i32) -> u8 {
    let mut n = note;
    while n < lo {
        n += 12;
    }
    while n > hi {
        n -= 12;
    }
    n.clamp(0, 127) as u8
}

/// A sound with every note folded into `lo..=hi`.
fn fold_sound(s: Sound, lo: i32, hi: i32) -> Sound {
    match s {
        Sound::Note(n) => Sound::Note(fold(n as i32, lo, hi)),
        Sound::Arp(a) => {
            let mut notes = [0u8; crate::audio::mml::Arp::MAX];
            let src = a.notes();
            for (d, &n) in notes.iter_mut().zip(src) {
                *d = fold(n as i32, lo, hi);
            }
            Sound::Arp(crate::audio::mml::Arp::new(&notes[..src.len()]))
        }
        d => d,
    }
}

/// Cut every event at beat `b` (those starting there or later go).
pub(super) fn clear_from(ctx: &Ctx, v: &mut Vec<NoteEvent>, b: f64) {
    let at = ctx.at(ctx.line() + b);
    v.retain(|e| e.start < at);
    for e in v.iter_mut() {
        e.end = e.end.min(at);
    }
}

/// Make room for something at `a..z`: events starting inside go, ones sounding into it stop.
pub(super) fn clear_span(ctx: &Ctx, v: &mut Vec<NoteEvent>, a: f64, z: f64) {
    let (sa, sz) = (ctx.at(ctx.line() + a), ctx.at(ctx.line() + z));
    v.retain(|e| e.start < sa || e.start >= sz);
    for e in v.iter_mut() {
        if e.start < sa {
            e.end = e.end.min(sa);
        }
    }
}

/// Stop whatever sounds through beat `b`.
pub(super) fn cut_at(ctx: &Ctx, v: &mut [NoteEvent], b: f64) {
    let at = ctx.at(ctx.line() + b);
    for e in v.iter_mut() {
        if e.start < at && e.end > at {
            e.end = at;
        }
    }
}
