//! [`PlayerProfile`] and its pure reducer, [`reduce`].

use super::assist::AssistLevers;
use super::calibration::{CALIBRATION_SKILL, Calibration, Placement, Probe};
use super::frustration::{self, FrustrationSignal};
use super::skill::{Band, MAX_BAND, MIN_BAND, Skill};
use super::window::{Outcome, RollingWindow, WindowEntry};

// ─── Tuning (see the module docs / README for how these were picked) ─────────

/// Assists at or below this count as "off": promotion needs it, and only rooms played this
/// unassisted count as promotion evidence.
pub const ASSIST_EPS: f32 = 0.02;
/// A clean clear lowers the assist dial by this much.
pub const ASSIST_FADE: f32 = 0.07;
/// Each death beyond expected (up to [`frustration::EXCESS_DEATHS`]) raises the dial this much.
pub const ASSIST_RISE: f32 = 0.08;
/// Frustration raises the dial this much (on top of the per-death rise).
pub const FRUSTRATION_ASSISTS: f32 = 0.15;
/// Rooms after a frustration during which [`super::choose::pick_skill`] favours strengths 80/20.
pub const FRUSTRATION_COOLDOWN: u8 = 4;

/// Promote: clean rate at the center (unassisted, this epoch) at least this...
pub const PROMOTE_AT: f32 = 0.75;
/// ...over at least this many rooms...
pub const MIN_EVIDENCE: usize = 4;
/// ...and, with at least [`MIN_STRETCH`] stretch rooms (above center), a stretch clean rate of
/// at least this.
pub const STRETCH_AT: f32 = 0.6;
pub const MIN_STRETCH: usize = 2;
/// Demote: clean rate at the center (this epoch) below this over [`MIN_EVIDENCE`]+ rooms.
pub const DEMOTE_BELOW: f32 = 0.5;
/// Hysteresis: promoting back into a band the skill was demoted (or eased) out of needs this
/// many rooms of evidence instead of [`MIN_EVIDENCE`]. Stops 4↔5 ping-pong for players whose
/// true level sits on a band boundary.
pub const REPROMOTE_EVIDENCE: usize = 8;

/// Spread: widen by [`WIDEN_STEP`] (up to [`WIDEN_MAX`]) while the skill's whole window is
/// above [`WIDEN_AT`] clean over [`WIDEN_MIN_ROOMS`]+ rooms.
pub const WIDEN_AT: f32 = 0.75;
pub const WIDEN_MIN_ROOMS: usize = 10;
pub const WIDEN_STEP: f32 = 0.1;
pub const WIDEN_MAX: f32 = 0.8;
/// Narrowing on promote / demote / frustration, and the floors.
pub const PROMOTE_NARROW: f32 = 0.1;
pub const PROMOTE_SPREAD_MIN: f32 = 0.2;
pub const DEMOTE_NARROW: f32 = 0.15;
pub const DEMOTE_SPREAD_MIN: f32 = 0.1;
pub const FRUSTRATION_NARROW: f32 = 0.15;
pub const DEFAULT_SPREAD: f32 = 0.5;

/// Carelessness filter: a single excess death this early in a room...
pub const CARELESS_SECS: f32 = 1.5;
/// ...right after this many clean clears is a slip.
pub const CARELESS_AFTER_CLEAN: usize = 2;

/// A newly unlocked skill starts this many bands under the mean played center.
pub const UNLOCK_OFFSET: u8 = 1;

/// Rooms remembered globally (carelessness filter, music mood).
pub const RECENT_ROOMS: usize = 8;

// ─── State ───────────────────────────────────────────────────────────────────

/// One skill's difficulty state.
#[derive(Debug, Clone, PartialEq)]
pub struct SkillState {
    /// Center band; rooms are sampled around it ([`super::choose::band_distribution`]).
    pub center: Band,
    /// 0 = nearly always the center, 1 = lots of reinforcement and stretch.
    pub spread: f32,
    pub window: RollingWindow,
    /// Bumps on every band change; window entries from older epochs don't count toward the
    /// new center.
    pub epoch: u32,
    /// The band this skill last fell out of (demotion or frustration), until it's re-earned.
    /// Promoting back into it needs [`REPROMOTE_EVIDENCE`] rooms.
    pub fell_from: Option<Band>,
}

impl SkillState {
    pub fn new(center: Band) -> Self {
        SkillState { center, spread: DEFAULT_SPREAD, window: RollingWindow::default(), epoch: 0, fell_from: None }
    }

    /// Rooms at the center needed before a promotion.
    pub fn evidence_needed(&self) -> usize {
        if self.fell_from == Some(self.center + 1) { REPROMOTE_EVIDENCE } else { MIN_EVIDENCE }
    }
}

/// A room the game said it started and hasn't finished yet.
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveRoom {
    pub room_id: u64,
    /// Each skill with the band it's played at, plus the skill's center and epoch at start.
    pub skills: Vec<RoomSkill>,
    pub expected_deaths: u32,
    pub par_secs: Option<f32>,
    /// Assist dial when the room started.
    pub assists: f32,
    /// Times the room was started again before finishing.
    pub restarts: u32,
    /// Frustration already handled for this room (only once per room).
    pub frustrated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoomSkill {
    pub skill: Skill,
    pub band: Band,
    pub center: Band,
    pub epoch: u32,
}

/// A finished room, remembered globally.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RecentRoom {
    pub outcome: Outcome,
    pub excess_deaths: u32,
    /// time / par, when the room had a par.
    pub time_ratio: Option<f32>,
    pub toots_per_min: f32,
}

/// Something the caller should react to, produced by the last [`reduce`]. Internal only: none
/// of these is ever shown to the player as a label.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cue {
    /// Han should say something encouraging.
    Encourage(FrustrationSignal),
    /// A skill's center went up (for logs / the simulator).
    Promoted(Skill, Band),
    /// A skill's center went down after sustained struggle.
    Demoted(Skill, Band),
    /// A skill's center was eased down because of frustration.
    Eased(Skill, Band),
    /// The placement rooms are over; bands and assists were set.
    Calibrated(Placement),
}

/// Everything the adaptive engine knows about the player. Update it only with [`reduce`].
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerProfile {
    /// Indexed by [`Skill::index`].
    pub skills: [SkillState; Skill::COUNT],
    /// The global assist dial, 0..1. See [`AssistLevers`].
    pub assists: f32,
    /// Clean clears in a row. Display only: it never moves a band.
    pub streak: u32,
    pub calibration: Calibration,
    /// Last [`RECENT_ROOMS`] finished rooms, oldest first.
    pub recent: Vec<RecentRoom>,
    /// Rooms left of "go easy" skill picking after a frustration.
    pub frustration_cooldown: u8,
    pub room: Option<ActiveRoom>,
    pub rooms_played: u32,
    /// What the last event asked the caller to do.
    pub cues: Vec<Cue>,
}

impl Default for PlayerProfile {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerProfile {
    /// A new player: everything at band 1, calibration pending.
    pub fn new() -> Self {
        PlayerProfile {
            skills: std::array::from_fn(|_| SkillState::new(MIN_BAND)),
            assists: 0.0,
            streak: 0,
            calibration: Calibration::default(),
            recent: Vec::new(),
            frustration_cooldown: 0,
            room: None,
            rooms_played: 0,
            cues: Vec::new(),
        }
    }

    /// A player past calibration with every skill centered at `band` (tests, tools).
    pub fn calibrated(band: Band) -> Self {
        PlayerProfile {
            skills: std::array::from_fn(|_| SkillState::new(band.clamp(MIN_BAND, MAX_BAND))),
            calibration: Calibration::finished(),
            ..Self::new()
        }
    }

    pub fn skill(&self, skill: Skill) -> &SkillState {
        &self.skills[skill.index()]
    }

    pub fn center(&self, skill: Skill) -> Band {
        self.skill(skill).center
    }

    pub fn levers(&self) -> AssistLevers {
        AssistLevers::from_dial(self.assists)
    }

    /// Still in the go-easy period after a frustration.
    pub fn frustrated(&self) -> bool {
        self.frustration_cooldown > 0
    }

    pub fn calibrating(&self) -> bool {
        !self.calibration.done
    }
}

// ─── Events ──────────────────────────────────────────────────────────────────

/// How a room went.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RoomResult {
    /// Deaths in the room, including attempts abandoned by restarting it.
    pub deaths: u32,
    pub time_secs: f32,
    /// Toot (double) jumps.
    pub toots: u32,
    /// The longest stretch with no input right after a death.
    pub idle_after_death_secs: f32,
    /// Seconds into the room of the first death, if any (carelessness filter).
    pub first_death_secs: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AdaptEvent {
    /// A room began. Starting the same `room_id` again before finishing it is a restart.
    RoomStarted {
        room_id: u64,
        /// The skills the room exercises and the band of each.
        skills: Vec<(Skill, Band)>,
        /// Deaths the room is designed to cost (stain rooms need dying). Never held against you.
        expected_deaths: u32,
        /// A reasonable time to clear, if the room has one (calibration, music mood).
        par_secs: Option<f32>,
    },
    RoomFinished(RoomResult),
    /// A frustration signal the caller noticed live, mid-room (e.g. idling after a splat).
    /// Handled at most once per room; [`AdaptEvent::RoomFinished`] also checks by itself.
    Frustrated(FrustrationSignal),
    /// A skill became available to free play. If it was never played, its center starts at the
    /// mean center of the skills that have been played, minus [`UNLOCK_OFFSET`] (so a strong
    /// player doesn't grind a new mechanic up from band 1).
    SkillUnlocked(Skill),
}

// ─── Reducer ─────────────────────────────────────────────────────────────────

/// The next profile after `event`. Pure: same input, same output; `state` is consumed and a new
/// profile is returned (clone first to keep the old one). [`PlayerProfile::cues`] holds what
/// this event asks of the caller.
pub fn reduce(state: PlayerProfile, event: AdaptEvent) -> PlayerProfile {
    let state = PlayerProfile { cues: Vec::new(), ..state };
    match event {
        AdaptEvent::RoomStarted { room_id, skills, expected_deaths, par_secs } => {
            room_started(state, room_id, skills, expected_deaths, par_secs)
        }
        AdaptEvent::RoomFinished(result) => room_finished(state, result),
        AdaptEvent::Frustrated(signal) => match state.room.clone() {
            Some(room) if room.frustrated => state,
            Some(room) => {
                let s = ease(state, &room.skills, signal);
                PlayerProfile { room: Some(ActiveRoom { frustrated: true, ..room }), ..s }
            }
            None => ease(state, &[], signal),
        },
        AdaptEvent::SkillUnlocked(skill) => skill_unlocked(state, skill),
    }
}

fn skill_unlocked(state: PlayerProfile, skill: Skill) -> PlayerProfile {
    if state.calibrating() || !state.skill(skill).window.entries.is_empty() {
        return state;
    }
    let played: Vec<Band> = state.skills.iter().filter(|s| !s.window.entries.is_empty()).map(|s| s.center).collect();
    if played.is_empty() {
        return state;
    }
    let mean = played.iter().map(|b| *b as f32).sum::<f32>() / played.len() as f32;
    let center = super::skill::clamp_band(mean.round() as i32 - UNLOCK_OFFSET as i32);
    let mut skills = state.skills.clone();
    let st = &skills[skill.index()];
    skills[skill.index()] = SkillState { center, epoch: st.epoch + 1, ..st.clone() };
    PlayerProfile { skills, ..state }
}

fn room_started(
    state: PlayerProfile,
    room_id: u64,
    skills: Vec<(Skill, Band)>,
    expected_deaths: u32,
    par_secs: Option<f32>,
) -> PlayerProfile {
    if let Some(room) = state.room.clone().filter(|r| r.room_id == room_id) {
        let room = ActiveRoom { restarts: room.restarts + 1, ..room };
        if !room.frustrated
            && let Some(signal) = frustration::detect(0, 0.0, room.restarts)
        {
            let s = ease(state, &room.skills, signal);
            return PlayerProfile { room: Some(ActiveRoom { frustrated: true, ..room }), ..s };
        }
        return PlayerProfile { room: Some(room), ..state };
    }
    let skills = skills
        .into_iter()
        .map(|(skill, band)| {
            let st = state.skill(skill);
            RoomSkill { skill, band: band.clamp(MIN_BAND, MAX_BAND), center: st.center, epoch: st.epoch }
        })
        .collect();
    let room = ActiveRoom {
        room_id,
        skills,
        expected_deaths,
        par_secs,
        assists: state.assists,
        restarts: 0,
        frustrated: false,
    };
    PlayerProfile { room: Some(room), ..state }
}

fn room_finished(state: PlayerProfile, result: RoomResult) -> PlayerProfile {
    let Some(room) = state.room.clone() else { return state };
    let excess = result.deaths.saturating_sub(room.expected_deaths);
    let after_cleans = state.recent.len() >= CARELESS_AFTER_CLEAN
        && state.recent.iter().rev().take(CARELESS_AFTER_CLEAN).all(|r| r.outcome == Outcome::Clean);
    let quick = result.first_death_secs.is_some_and(|t| t < CARELESS_SECS);
    let outcome = if excess == 0 {
        Outcome::Clean
    } else if excess == 1 && quick && after_cleans {
        Outcome::Careless
    } else {
        Outcome::Struggle
    };
    let time_ratio = room.par_secs.filter(|p| *p > 0.0).map(|p| result.time_secs / p);

    let mut recent = state.recent.clone();
    recent.push(RecentRoom {
        outcome,
        excess_deaths: excess,
        time_ratio,
        toots_per_min: if result.time_secs > 0.0 { result.toots as f32 * 60.0 / result.time_secs } else { 0.0 },
    });
    if recent.len() > RECENT_ROOMS {
        recent.drain(..recent.len() - RECENT_ROOMS);
    }
    let streak = match outcome {
        Outcome::Clean => state.streak + 1,
        Outcome::Careless => state.streak,
        Outcome::Struggle => 0,
    };
    let signal = if room.frustrated {
        None
    } else {
        frustration::detect(excess, result.idle_after_death_secs, room.restarts)
    };

    let s = PlayerProfile {
        recent,
        streak,
        room: None,
        rooms_played: state.rooms_played + 1,
        frustration_cooldown: state.frustration_cooldown.saturating_sub(1),
        ..state
    };

    if s.calibrating() {
        // No clean run to judge against yet, and one slip would misplace a strong player by
        // several bands: during calibration any single quick death is a slip.
        let probe_clean = outcome != Outcome::Struggle || (excess == 1 && quick);
        return calibration_room(s, &room, probe_clean, time_ratio, signal);
    }

    // The assist dial: clean clears fade it, deaths beyond expected raise it. A frustrated
    // room gets the larger of the per-death rise and the frustration bump, not both (stacking
    // them maxed out struggling players' dials in the simulator).
    let death_rise = ASSIST_RISE * excess.min(frustration::EXCESS_DEATHS) as f32;
    let assists = match outcome {
        Outcome::Clean => s.assists - ASSIST_FADE,
        Outcome::Careless => s.assists,
        Outcome::Struggle if signal.is_some() => s.assists + (death_rise - frustration_bump(s.assists)).max(0.0),
        Outcome::Struggle => s.assists + death_rise,
    }
    .clamp(0.0, 1.0);

    let mut skills = s.skills.clone();
    for rs in &room.skills {
        let st = &skills[rs.skill.index()];
        let entry = WindowEntry { outcome, band: rs.band, center: rs.center, epoch: rs.epoch, assists: room.assists };
        skills[rs.skill.index()] = SkillState { window: st.window.push(entry), ..st.clone() };
    }
    let mut s = PlayerProfile { skills, assists, ..s };

    if let Some(signal) = signal {
        return ease(s, &room.skills, signal);
    }
    if room.frustrated || outcome == Outcome::Careless {
        return s;
    }

    // Promote / demote / widen, per skill the room exercised.
    let mut cues = Vec::new();
    let mut skills = s.skills.clone();
    for rs in &room.skills {
        let st = &skills[rs.skill.index()];
        let next = if st.center < MAX_BAND && s.assists <= ASSIST_EPS && should_promote(st) {
            cues.push(Cue::Promoted(rs.skill, st.center + 1));
            SkillState {
                center: st.center + 1,
                spread: (st.spread - PROMOTE_NARROW).max(PROMOTE_SPREAD_MIN),
                epoch: st.epoch + 1,
                fell_from: st.fell_from.filter(|b| *b > st.center + 1),
                ..st.clone()
            }
        } else if st.center > MIN_BAND && should_demote(st) {
            cues.push(Cue::Demoted(rs.skill, st.center - 1));
            SkillState {
                center: st.center - 1,
                spread: (st.spread - DEMOTE_NARROW).max(DEMOTE_SPREAD_MIN),
                epoch: st.epoch + 1,
                fell_from: Some(st.center),
                ..st.clone()
            }
        } else {
            match st.window.clean_rate() {
                (Some(rate), n) if n >= WIDEN_MIN_ROOMS && rate > WIDEN_AT && st.spread < WIDEN_MAX => {
                    SkillState { spread: (st.spread + WIDEN_STEP).min(WIDEN_MAX), ..st.clone() }
                }
                _ => st.clone(),
            }
        };
        skills[rs.skill.index()] = next;
    }
    s.skills = skills;
    s.cues = cues;
    s
}

/// Promote: ≥ [`PROMOTE_AT`] clean over ≥ [`MIN_EVIDENCE`] unassisted rooms at the center since
/// the last band change ([`REPROMOTE_EVIDENCE`] into a band just fallen out of), and stretch
/// rooms (if ≥ [`MIN_STRETCH`]) ≥ [`STRETCH_AT`].
pub fn should_promote(st: &SkillState) -> bool {
    let (at, n) = st.window.at_center(st.center, st.epoch, true);
    if n < st.evidence_needed() || at.unwrap_or(0.0) < PROMOTE_AT {
        return false;
    }
    let (above, m) = st.window.above_center(st.center, st.epoch);
    !(m >= MIN_STRETCH && above.unwrap_or(0.0) < STRETCH_AT)
}

/// Demote: < [`DEMOTE_BELOW`] clean over ≥ [`MIN_EVIDENCE`] rooms at the center since the last
/// band change (assisted or not).
pub fn should_demote(st: &SkillState) -> bool {
    let (at, n) = st.window.at_center(st.center, st.epoch, false);
    n >= MIN_EVIDENCE && at.unwrap_or(1.0) < DEMOTE_BELOW
}

/// The frustration response: ease the skills that were pushing (played at or above their
/// center; the first one if none), narrow their spread, raise assists, go easy on skill picks
/// for a few rooms, and ask Han to encourage. Easing drops the center by 1, except after a
/// *stretch* room (played above the center): then the stretch was the problem, not the
/// center, so only the spread narrows (dropping a sound center there caused re-climb
/// oscillation in the simulator).
fn ease(state: PlayerProfile, room_skills: &[RoomSkill], signal: FrustrationSignal) -> PlayerProfile {
    let mut cues = state.cues.clone();
    cues.push(Cue::Encourage(signal));
    let mut skills = state.skills.clone();
    let pushing: Vec<&RoomSkill> = room_skills.iter().filter(|rs| rs.band >= rs.center).collect();
    let targets: Vec<&RoomSkill> = if pushing.is_empty() { room_skills.iter().take(1).collect() } else { pushing };
    if state.calibration.done {
        for rs in targets {
            let st = &skills[rs.skill.index()];
            let stretch = rs.band > rs.center && st.center == rs.center;
            let center = if stretch { st.center } else { st.center.saturating_sub(1).max(MIN_BAND) };
            if center != st.center {
                cues.push(Cue::Eased(rs.skill, center));
            }
            skills[rs.skill.index()] = SkillState {
                center,
                spread: (st.spread - FRUSTRATION_NARROW).max(DEMOTE_SPREAD_MIN),
                epoch: if center != st.center { st.epoch + 1 } else { st.epoch },
                fell_from: if center != st.center { Some(st.center) } else { st.fell_from },
                ..st.clone()
            };
        }
    }
    PlayerProfile {
        skills,
        assists: (state.assists + frustration_bump(state.assists)).min(1.0),
        frustration_cooldown: FRUSTRATION_COOLDOWN,
        cues,
        ..state
    }
}

/// How much frustration raises the dial from `assists`: [`FRUSTRATION_ASSISTS`], diminishing
/// near the top so repeated frustration doesn't pin the dial at max.
fn frustration_bump(assists: f32) -> f32 {
    FRUSTRATION_ASSISTS * (1.0 - assists)
}

fn calibration_room(
    s: PlayerProfile,
    room: &ActiveRoom,
    clean: bool,
    time_ratio: Option<f32>,
    signal: Option<FrustrationSignal>,
) -> PlayerProfile {
    let band = room
        .skills
        .iter()
        .find(|rs| rs.skill == CALIBRATION_SKILL)
        .or(room.skills.first())
        .map(|rs| rs.band)
        .unwrap_or(MIN_BAND);
    let calibration = s.calibration.record(Probe { band, clean, time_ratio });
    let mut s = PlayerProfile { calibration, ..s };
    if let Some(signal) = signal {
        s = ease(s, &room.skills, signal);
    }
    if !s.calibration.complete() {
        return s;
    }
    let p = s.calibration.placement();
    let skills = std::array::from_fn(|i| {
        let center = if Skill::ALL[i] == CALIBRATION_SKILL { p.band } else { p.band.saturating_sub(1).max(MIN_BAND) };
        SkillState { center, spread: p.spread, window: RollingWindow::default(), epoch: s.skills[i].epoch + 1, fell_from: None }
    });
    let mut cues = s.cues.clone();
    cues.push(Cue::Calibrated(p));
    PlayerProfile {
        skills,
        assists: s.assists.max(p.assists),
        calibration: Calibration { done: true, ..s.calibration.clone() },
        cues,
        ..s
    }
}
