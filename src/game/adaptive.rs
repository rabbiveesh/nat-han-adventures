//! Wiring for the adaptive engine ([`crate::adapt`]): gameplay messages in, invisible
//! [`Assists`], Han's lines and the band's freedom out. Nothing here is ever shown.
//!
//! - **Story mode** ([`AssistMode::Story`], the default) feeds [`StoryAssist`] (in
//!   [`StoryAssistState`]): level start (expected deaths per checkpoint segment from the
//!   level's `deaths:` header), deaths, idling > [`IDLE_SECS`] after a death, checkpoints,
//!   restarts, the goal. Its dial becomes [`AssistLevers`], written into [`Assists`].
//! - **Free play** ([`AssistMode::FreePlay`]): free play reduces [`AdaptiveProfile`] itself;
//!   this module turns the profile's dial into [`Assists`], its [`Cue::Encourage`] into a Han
//!   line, and its [`adapt::band_mood`] into the band's freedom.
//! - [`AssistMode::Manual`]: nothing writes [`Assists`] (tests, tools).
//!
//! Every lever only loosens things above the floors the level validator proves solvable
//! ([`Assists::default`]); the validator never reads any of this.
//!
//! Levers, from the dial `d` (0..1, see [`AssistLevers::from_dial`]):
//! coyote ×(1+d), jump buffer ×(1+0.75d), hazard hitboxes −round(3d) px a side, rafts last
//! ×(1+d), Han's eagerness ([`han_eagerness`]), a hidden extra respawn point midway through a
//! segment at d ≥ [`adapt::assist::EXTRA_CHECKPOINT_AT`] ([`HiddenRespawn`]), and at d ≥
//! [`adapt::assist::HAN_HINT_AT`] Han repeats the nearest hint after repeated deaths at one
//! spot.
//!
//! The band ([`BandFreedom`]) is decided at level start, every death and checkpoint, and every
//! [`MUSIC_CHECK_SECS`] of play: struggling → calmer and sparser, flying → hotter
//! ([`band_freedom`]).

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::hazards::RaftLife;
use super::physics::{Body, Dead, Finished, PlayerControl};
use super::{ActiveLevel, Assists, Checkpoint, Fly, GameSet, Goal, Groove, HintSpot, LevelRun, Player, Pos, Raft, Spray};
use crate::adapt::frustration::IDLE_SECS;
use crate::adapt::profile::{ASSIST_EPS, RecentRoom};
use crate::adapt::{self, AssistLevers, BandMood, Cue, Outcome, PlayerProfile, StoryAssist, StoryEvent, reduce_story};
use crate::audio::Harmony;
use crate::events::{BandFreedom, CheckpointReached, HanSays, Jumped, LevelCompleted, LevelStarted, PlayerDied};
use crate::input::Action;
use crate::level::{Level, TILE};
use crate::state::PlayState;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<StoryAssistState>()
        .init_resource::<AdaptiveProfile>()
        .init_resource::<AssistMode>()
        .init_resource::<HiddenRespawn>()
        .init_resource::<Tracker>()
        .add_systems(Startup, register_debug)
        .add_systems(
            Update,
            (feed_story, idle_after_death, profile_cues, decide_music, write_assists, sync_raft_life).chain(),
        )
        .add_systems(
            FixedUpdate,
            record_hidden_respawn.in_set(GameSet::Interact).after(super::hazards::respawn),
        );
}

// ─── Shared state (free play reads and writes these too) ─────────────────────

/// Story mode's assist state. The dial ([`StoryAssist::assists`]) is saved with progress.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct StoryAssistState(pub StoryAssist);

/// Free play's adaptive profile ([`adapt::reduce`] it). Saved with progress.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct AdaptiveProfile(pub PlayerProfile);

/// Which adaptive state drives [`Assists`] (and the band, and Han's encouragement).
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
#[reflect(Resource)]
pub enum AssistMode {
    /// The hand-made levels: [`StoryAssistState`], fed here from gameplay messages.
    #[default]
    Story,
    /// Free play: [`AdaptiveProfile`], fed by free play.
    FreePlay,
    /// Nothing writes [`Assists`] (tests, tools).
    Manual,
}

/// A hidden extra respawn point (the `extra_checkpoint` lever): the first safe spot Nat stood
/// on past the middle of the current checkpoint segment. Used instead of the checkpoint while
/// `armed` (the dial is high enough). Never drawn, never announced.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct HiddenRespawn {
    /// Box center to respawn at.
    pub pos: Option<Vec2>,
    /// The segment it belongs to ([`LevelRun::checkpoint`] when it was found).
    pub segment: Option<usize>,
    /// [`LevelRun::time`] when it was found (a reload resets the run clock, and the spot).
    pub at_time: f32,
    pub armed: bool,
}

impl HiddenRespawn {
    /// Where to respawn instead of checkpoint `checkpoint`, if anywhere.
    pub fn respawn_at(&self, checkpoint: Option<usize>, run_time: f32) -> Option<Vec2> {
        self.pos.filter(|_| self.armed && self.segment == checkpoint && run_time >= self.at_time)
    }
}

// ─── Tuning ──────────────────────────────────────────────────────────────────

/// Seconds of play between periodic music decisions.
pub const MUSIC_CHECK_SECS: f32 = 20.0;
/// Han's eagerness when cruising (a run of clean segments with no assists).
pub const EAGERNESS_CRUISING: f32 = 0.3;
/// Clean segments in a row (with the dial off) to reach [`EAGERNESS_CRUISING`].
pub const CRUISE_SEGMENTS: u32 = 4;
/// Deaths within this distance of the last one count as "the same spot" (for `han_hint`).
pub const SAME_SPOT: f32 = 3.0 * TILE;
/// Deaths at one spot before Han repeats the nearest hint (with the `han_hint` lever on).
pub const HINT_REPEAT_DEATHS: u32 = 2;
/// Han only repeats a hint this close to the death.
pub const HINT_REPEAT_RADIUS: f32 = 12.0 * TILE;
/// Raft lifetime ×(1 + this × dial).
pub const RAFT_LIFE_PER_DIAL: f32 = 1.0;
/// Segments remembered for the music mood.
const RECENT_SEGMENTS: usize = 8;

/// What Han says when the player seems frustrated (the encourage cue). Rotates, no repeats
/// until the list wraps. PG, warm, plumber humour, ≤ 60 characters.
pub const ENCOURAGE_LINES: &[&str] = &[
    "Deep breath, Nat. No clog lasts forever.",
    "Rome wasn't plumbed in a day, buddy.",
    "Take your time. The meter's not running.",
    "We'll snake through this one together, Nat.",
    "You're doin' great. I've seen pipes quit sooner.",
    "Slow and steady, Nat. Like a good drain.",
    "Every pro started with a leaky first job.",
    "Shake it off. Then wash your hands. Then GO!",
];

// ─── Mappings ────────────────────────────────────────────────────────────────

/// Han's eagerness from the dial: 0.5 neutral, up to 1.0 at a full dial; with the dial off,
/// down to [`EAGERNESS_CRUISING`] over [`CRUISE_SEGMENTS`] clean segments (or rooms) in a row.
pub fn han_eagerness(dial: f32, clean_streak: u32) -> f32 {
    let dial = dial.clamp(0.0, 1.0);
    if dial > ASSIST_EPS {
        0.5 + 0.5 * dial
    } else {
        let k = clean_streak.min(CRUISE_SEGMENTS) as f32 / CRUISE_SEGMENTS as f32;
        0.5 - (0.5 - EAGERNESS_CRUISING) * k
    }
}

/// The [`Assists`] for a dial (with a clean streak, for Han).
pub fn assists_for(dial: f32, clean_streak: u32) -> Assists {
    let l = AssistLevers::from_dial(dial);
    Assists {
        coyote_mult: l.coyote_mult,
        jump_buffer_mult: l.jump_buffer_mult,
        hitbox_forgiveness_px: l.hitbox_forgiveness_px,
        raft_life_mult: 1.0 + RAFT_LIFE_PER_DIAL * dial.clamp(0.0, 1.0),
        han_eagerness: han_eagerness(dial, clean_streak),
    }
}

/// The band's freedom for a mood, scaled lightly so the tunes stay recognizable: at a calm,
/// sparse mood (struggling) everyone plays (nearly) as written; flying, the lead ornaments up
/// to ~0.35 and the drums fill up to ~0.4.
pub fn band_freedom(mood: BandMood) -> BandFreedom {
    let f = mood.freedom.clamp(0.0, 1.0);
    let i = mood.intensity.clamp(0.0, 1.0);
    BandFreedom { lead: 0.35 * f, comp: 0.3 * f, bass: 0.15 * f, drums: 0.25 * f + 0.15 * i, dynamics: 0.5 * i }
}

/// The story's mood: [`adapt::band_mood`]'s formula over the last checkpoint segments (and the
/// one in progress, once it's gone past its expected deaths), with the story's dial.
pub fn story_mood(story: &StoryAssist, recent: &[RecentRoom]) -> BandMood {
    let mut recent = recent.to_vec();
    let excess = story.segment_deaths.saturating_sub(story.expected_deaths);
    if excess > 0 {
        recent.push(RecentRoom { outcome: Outcome::Struggle, excess_deaths: excess, time_ratio: None, toots_per_min: 0.0 });
    }
    let profile = PlayerProfile {
        assists: story.assists,
        frustration_cooldown: u8::from(story.segment_frustrated),
        ..PlayerProfile::new()
    };
    adapt::mood::band_mood_from(&profile, &recent)
}

/// Expected deaths per checkpoint segment: the level's `deaths:` split evenly (rounded up).
pub fn expected_per_segment(level: &Level) -> u32 {
    let segments = level.checkpoints().count() as u32 + 1;
    level.deaths.unwrap_or(0).div_ceil(segments)
}

// ─── Driver ──────────────────────────────────────────────────────────────────

/// The driver's bookkeeping (not saved).
#[derive(Resource, Debug, Default)]
struct Tracker {
    /// The level's `deaths:` and its even split per segment.
    level_deaths: u32,
    per_segment: u32,
    /// Deaths since the level (re)started.
    deaths_in_level: u32,
    /// Finished segments (story) for the music, oldest first.
    recent: Vec<RecentRoom>,
    /// Clean segments in a row.
    clean_streak: u32,
    segment_toots: u32,
    segment_start: f32,
    /// Since the last death: (seconds, seconds with no input). None: not watching.
    idle: Option<(f32, f32)>,
    /// The last death spot and how many deaths in a row were there.
    death_spot: Option<(Vec2, u32)>,
    /// Hints already repeated in this segment.
    repeated: Vec<Vec2>,
    next_encourage: usize,
    /// A music decision is due.
    decide: bool,
    last_decision: f32,
}

impl Tracker {
    /// Expected deaths for the segment starting now: the even split, or what's left of the
    /// level's budget if that's more (designed deaths often all sit in one segment).
    fn expected_now(&self) -> u32 {
        self.per_segment.max(self.level_deaths.saturating_sub(self.deaths_in_level))
    }

    fn encourage_line(&mut self) -> String {
        let line = ENCOURAGE_LINES[self.next_encourage % ENCOURAGE_LINES.len()];
        self.next_encourage = (self.next_encourage + 1) % ENCOURAGE_LINES.len();
        line.to_string()
    }

    fn end_segment(&mut self, story: &StoryAssist, now: f32) {
        let excess = story.segment_deaths.saturating_sub(story.expected_deaths);
        let minutes = ((now - self.segment_start) / 60.0).max(1.0 / 60.0);
        self.recent.push(RecentRoom {
            outcome: if excess == 0 { Outcome::Clean } else { Outcome::Struggle },
            excess_deaths: excess,
            time_ratio: None,
            toots_per_min: self.segment_toots as f32 / minutes,
        });
        if self.recent.len() > RECENT_SEGMENTS {
            self.recent.remove(0);
        }
        self.clean_streak = if excess == 0 { self.clean_streak + 1 } else { 0 };
        self.segment_toots = 0;
        self.segment_start = now;
        self.repeated.clear();
        self.death_spot = None;
    }
}

fn step(story: &mut StoryAssistState, t: &mut Tracker, says: &mut MessageWriter<HanSays>, event: StoryEvent) {
    story.0 = reduce_story(std::mem::take(&mut story.0), event);
    if story.0.encourage.is_some() {
        says.write(HanSays { text: t.encourage_line() });
    }
}

#[allow(clippy::too_many_arguments)]
fn feed_story(
    mode: Res<AssistMode>,
    active: Option<Res<ActiveLevel>>,
    run: Option<Res<LevelRun>>,
    profile: Res<AdaptiveProfile>,
    hints: Query<&HintSpot>,
    mut story: ResMut<StoryAssistState>,
    mut t: ResMut<Tracker>,
    mut hidden: ResMut<HiddenRespawn>,
    mut started: MessageReader<LevelStarted>,
    mut died: MessageReader<PlayerDied>,
    mut jumped: MessageReader<Jumped>,
    mut checkpoints: MessageReader<CheckpointReached>,
    mut completed: MessageReader<LevelCompleted>,
    mut says: MessageWriter<HanSays>,
) {
    let story_mode = *mode == AssistMode::Story;
    let now = run.as_ref().map_or(0.0, |r| r.time);
    let t = &mut *t;
    for s in started.read() {
        t.decide = true;
        t.last_decision = 0.0;
        t.deaths_in_level = 0;
        t.segment_toots = 0;
        t.segment_start = 0.0;
        t.idle = None;
        t.death_spot = None;
        t.repeated.clear();
        *hidden = HiddenRespawn { armed: hidden.armed, ..default() };
        if let Some(a) = active.as_ref() {
            t.level_deaths = a.level.deaths.unwrap_or(0);
            t.per_segment = expected_per_segment(&a.level);
        }
        if !story_mode {
            continue;
        }
        let event = if s.restart {
            StoryEvent::LevelRestarted
        } else {
            StoryEvent::LevelStarted { expected_deaths_per_segment: t.expected_now() }
        };
        step(&mut story, t, &mut says, event);
        story.0.expected_deaths = t.expected_now();
    }
    t.segment_toots += jumped.read().filter(|j| j.double).count() as u32;
    for d in died.read() {
        t.decide = true;
        t.idle = Some((0.0, 0.0));
        t.deaths_in_level += 1;
        let count = match t.death_spot {
            Some((at, n)) if at.distance(d.pos) <= SAME_SPOT => n + 1,
            _ => 1,
        };
        t.death_spot = Some((d.pos, count));
        if story_mode {
            step(&mut story, t, &mut says, StoryEvent::Died);
        }
        // The han_hint lever: repeated deaths at one spot → the nearest hint again.
        let hint_on = match *mode {
            AssistMode::Story => story.0.levers().han_hint,
            AssistMode::FreePlay => profile.0.levers().han_hint,
            AssistMode::Manual => false,
        };
        if hint_on && count >= HINT_REPEAT_DEATHS {
            let nearest = hints
                .iter()
                .filter(|h| h.center.distance(d.pos) <= HINT_REPEAT_RADIUS)
                .filter(|h| !t.repeated.iter().any(|c| c.distance(h.center) < 1.0))
                .min_by(|a, b| a.center.distance(d.pos).total_cmp(&b.center.distance(d.pos)));
            if let Some(h) = nearest {
                t.repeated.push(h.center);
                says.write(HanSays { text: h.text.clone() });
            }
        }
    }
    for _ in checkpoints.read() {
        t.decide = true;
        if story_mode {
            t.end_segment(&story.0, now);
            step(&mut story, t, &mut says, StoryEvent::CheckpointReached);
            story.0.expected_deaths = t.expected_now();
        }
    }
    for _ in completed.read() {
        if story_mode {
            t.end_segment(&story.0, now);
            step(&mut story, t, &mut says, StoryEvent::LevelCompleted);
        }
    }
}

/// Input this soon after a death (still holding the keys while splatting and respawning) only
/// restarts the idle clock; later input means the player is back at it.
const IDLE_GRACE: f32 = super::tuning::RESPAWN_DELAY + 0.5;

/// Idling more than [`IDLE_SECS`] (real time, while running) right after a death.
fn idle_after_death(
    time: Res<Time<Real>>,
    mode: Res<AssistMode>,
    play: Option<Res<State<PlayState>>>,
    input: Option<Single<&ActionState<Action>>>,
    mut story: ResMut<StoryAssistState>,
    mut t: ResMut<Tracker>,
    mut says: MessageWriter<HanSays>,
) {
    if play.is_none_or(|p| *p.get() != PlayState::Running) {
        return;
    }
    let Some((since, idle)) = t.idle else { return };
    let since = since + time.delta_secs();
    if input.is_some_and(|i| !i.get_pressed().is_empty()) {
        t.idle = (since < IDLE_GRACE).then_some((since, 0.0));
        return;
    }
    let idle = idle + time.delta_secs();
    if idle <= IDLE_SECS {
        t.idle = Some((since, idle));
        return;
    }
    t.idle = None;
    if *mode == AssistMode::Story {
        step(&mut story, &mut t, &mut says, StoryEvent::IdleAfterDeath { secs: idle });
    }
}

/// Free play: the profile's encourage cue → a Han line.
fn profile_cues(
    mode: Res<AssistMode>,
    profile: Res<AdaptiveProfile>,
    mut t: ResMut<Tracker>,
    mut says: MessageWriter<HanSays>,
) {
    if *mode != AssistMode::FreePlay || !profile.is_changed() {
        return;
    }
    if profile.0.cues.iter().any(|c| matches!(c, Cue::Encourage(_))) {
        let text = t.encourage_line();
        says.write(HanSays { text });
    }
}

/// The band's freedom, on decisions: level start, death, checkpoint, every
/// [`MUSIC_CHECK_SECS`] of play.
fn decide_music(
    mode: Res<AssistMode>,
    run: Option<Res<LevelRun>>,
    story: Res<StoryAssistState>,
    profile: Res<AdaptiveProfile>,
    mut t: ResMut<Tracker>,
    mut out: MessageWriter<BandFreedom>,
) {
    let Some(run) = run else { return };
    if run.time - t.last_decision >= MUSIC_CHECK_SECS {
        t.decide = true;
    }
    if !t.decide {
        return;
    }
    t.decide = false;
    t.last_decision = run.time;
    let mood = match *mode {
        AssistMode::FreePlay => adapt::band_mood(&profile.0),
        AssistMode::Story | AssistMode::Manual => story_mood(&story.0, &t.recent),
    };
    out.write(band_freedom(mood));
}

/// The active dial → [`Assists`] and the hidden respawn's arming.
fn write_assists(
    mode: Res<AssistMode>,
    story: Res<StoryAssistState>,
    profile: Res<AdaptiveProfile>,
    t: Res<Tracker>,
    mut assists: ResMut<Assists>,
    mut hidden: ResMut<HiddenRespawn>,
) {
    let (dial, streak) = match *mode {
        AssistMode::Story => (story.0.assists, t.clean_streak),
        AssistMode::FreePlay => (profile.0.assists, profile.0.streak),
        AssistMode::Manual => return,
    };
    let want = assists_for(dial, streak);
    if *assists != want {
        *assists = want;
    }
    let armed = AssistLevers::from_dial(dial).extra_checkpoint;
    if hidden.armed != armed {
        hidden.armed = armed;
    }
}

/// `RaftLife.scale` ← `Assists.raft_life_mult` (whoever wrote it).
fn sync_raft_life(assists: Res<Assists>, mut life: ResMut<RaftLife>) {
    let scale = assists.raft_life_mult.max(1.0);
    if life.scale != scale {
        life.scale = scale;
    }
}

// ─── The hidden respawn point ────────────────────────────────────────────────

/// Look for the segment's hidden respawn point: the first spot Nat stands on that's at least
/// as close to the next checkpoint (or the goal) as to the last one, and safe: still ground (no
/// platform, raft or grease), no hazard close by, plain music (no gate mode on: Giant Steps,
/// fired up or the waltz), no raft afloat, no nugget to re-collect nearby (a nugget line before
/// a long gap must stay in front of the respawn).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn record_hidden_respawn(
    active: Res<ActiveLevel>,
    run: Res<LevelRun>,
    groove: Res<Groove>,
    at_risk: Res<super::pickups::NuggetsAtRisk>,
    mut hidden: ResMut<HiddenRespawn>,
    player: Query<(&Pos, &Body, &PlayerControl), (With<Player>, Without<Dead>, Without<Finished>)>,
    checkpoints: Query<(&Checkpoint, &Transform)>,
    goal: Query<&Transform, With<Goal>>,
    rafts: Query<(), With<Raft>>,
    flies: Query<&Fly>,
    sprays: Query<&Transform, With<Spray>>,
) {
    if run.time < hidden.at_time || (hidden.pos.is_some() && hidden.segment != run.checkpoint) {
        *hidden = HiddenRespawn { armed: hidden.armed, ..default() };
    }
    if hidden.pos.is_some() {
        return;
    }
    let Ok((pos, body, ctl)) = player.single() else { return };
    if !body.on_ground || body.riding.is_some() || ctl.on_grease || !rafts.is_empty() {
        return;
    }
    if !matches!(groove.harmony, Harmony::Original | Harmony::MelodicMinor) {
        return;
    }
    let level = &active.level;
    let (start_col, start_row) = run
        .checkpoint
        .and_then(|i| level.checkpoints().nth(i))
        .map_or(level.start, |c| (c.col, c.row));
    let start = super::lifecycle::stand_pos(level, start_col, start_row);
    let ahead = checkpoints
        .iter()
        .filter(|(c, _)| !c.active)
        .map(|(_, tf)| tf.translation.truncate())
        .chain(goal.iter().map(|tf| tf.translation.truncate()))
        .min_by(|a, b| a.distance(start).total_cmp(&b.distance(start)));
    let Some(next) = ahead else { return };
    let p = pos.0;
    if p.distance(start) < p.distance(next).max(4.0 * TILE) {
        return;
    }
    let (col, row) = level.cell_at(p);
    let below = level.tile(col, row + 1);
    if !(below.is_solid() || below.is_one_way()) || below == crate::level::Tile::Grease {
        return;
    }
    let deadly_near = (col - 2..=col + 2).any(|c| (row - 2..=row + 1).any(|r| level.tile(c, r).is_deadly()));
    let hazard_near = flies.iter().any(|f| f.center.distance(p) < 3.0 * TILE)
        || sprays.iter().any(|tf| {
            let s = tf.translation.truncate();
            (s.x - p.x).abs() < 3.0 * TILE && (s.y - p.y).abs() < 5.0 * TILE
        });
    let nugget_near = at_risk.0.iter().any(|n| n.distance(p) < 12.0 * TILE);
    if deadly_near || hazard_near || nugget_near || col < 0 || row < 0 {
        return;
    }
    *hidden = HiddenRespawn {
        pos: Some(super::lifecycle::stand_pos(level, col as usize, row as usize)),
        segment: run.checkpoint,
        at_time: run.time,
        armed: hidden.armed,
    };
}

// ─── Debug dump ──────────────────────────────────────────────────────────────

fn register_debug(sections: Option<ResMut<crate::debug::DebugSections>>) {
    let Some(mut sections) = sections else { return };
    sections.0.push(("adaptive difficulty", debug_text));
}

/// The adaptive state, for the F9 dump.
pub fn debug_text(world: &World) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let mode = world.get_resource::<AssistMode>().copied().unwrap_or_default();
    let _ = writeln!(out, "mode: {mode:?}");
    if let Some(s) = world.get_resource::<StoryAssistState>() {
        let s = &s.0;
        let _ = writeln!(
            out,
            "story dial {:.3} · segment deaths {}/{} expected · restarts {} · frustrated {}",
            s.assists, s.segment_deaths, s.expected_deaths, s.restarts, s.segment_frustrated
        );
        let _ = writeln!(out, "story levers: {:?}", s.levers());
    }
    if let Some(t) = world.get_resource::<Tracker>() {
        let _ = writeln!(out, "idle after death: {:?}", t.idle);
        let _ = writeln!(
            out,
            "level deaths: {} ({} / segment) · deaths this run {} · clean streak {} · recent segments {:?}",
            t.level_deaths,
            t.per_segment,
            t.deaths_in_level,
            t.clean_streak,
            t.recent.iter().map(|r| (r.outcome, r.excess_deaths)).collect::<Vec<_>>()
        );
    }
    if let Some(a) = world.get_resource::<Assists>() {
        let _ = writeln!(out, "assists: {a:?}");
    }
    if let Some(h) = world.get_resource::<HiddenRespawn>() {
        let _ = writeln!(out, "hidden respawn: {h:?}");
    }
    if let Some(p) = world.get_resource::<AdaptiveProfile>() {
        let p = &p.0;
        let _ = writeln!(
            out,
            "free-play profile: dial {:.3} · calibrated {} ({} probes) · rooms {} · streak {} · cooldown {}",
            p.assists,
            p.calibration.done,
            p.calibration.probes.len(),
            p.rooms_played,
            p.streak,
            p.frustration_cooldown
        );
        for skill in adapt::Skill::ALL {
            let st = p.skill(skill);
            let (rate, n) = st.window.clean_rate();
            let _ = writeln!(
                out,
                "  {:<9} center {:>2} spread {:.2} epoch {} window {n} clean {}",
                skill.name(),
                st.center,
                st.spread,
                st.epoch,
                rate.map_or("-".into(), |r| format!("{:.0}%", r * 100.0))
            );
        }
        let _ = writeln!(out, "  mood {:?}", adapt::band_mood(p));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eagerness_is_neutral_rises_and_cruises() {
        assert_eq!(han_eagerness(0.0, 0), 0.5);
        assert_eq!(han_eagerness(1.0, 0), 1.0);
        assert!((han_eagerness(0.0, 10) - EAGERNESS_CRUISING).abs() < 1e-6);
        assert!(han_eagerness(0.5, 10) > 0.5);
    }

    #[test]
    fn assists_never_below_the_floors() {
        let floor = Assists::default();
        for i in 0..=20 {
            let a = assists_for(i as f32 / 20.0, 0);
            assert!(a.coyote_mult >= floor.coyote_mult && a.jump_buffer_mult >= floor.jump_buffer_mult);
            assert!(a.hitbox_forgiveness_px >= 0.0 && a.raft_life_mult >= 1.0);
        }
        assert_eq!(assists_for(0.0, 0), floor);
    }

    #[test]
    fn encourage_lines_are_short() {
        for l in ENCOURAGE_LINES {
            assert!(l.chars().count() <= crate::level::MAX_LINE, "{l}");
        }
    }

    #[test]
    fn struggling_is_calmer_than_flying() {
        let clean = RecentRoom { outcome: Outcome::Clean, excess_deaths: 0, time_ratio: None, toots_per_min: 10.0 };
        let bad = RecentRoom { outcome: Outcome::Struggle, excess_deaths: 3, time_ratio: None, toots_per_min: 0.0 };
        let flying = band_freedom(story_mood(&StoryAssist::new(0.0), &[clean; 6]));
        let struggling = band_freedom(story_mood(&StoryAssist::new(0.6), &[bad; 6]));
        assert!(struggling.lead < flying.lead && struggling.drums < flying.drums);
        assert!(struggling.dynamics < flying.dynamics);
        assert!(flying.lead <= 0.35 && flying.drums <= 0.4, "{flying:?}");
    }
}
