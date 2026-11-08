//! Synthetic players run through the real reducer, to tune the constants and to test the
//! engine's behaviour over hundreds of rooms. Used by `examples/simulate.rs` and `tests/adapt.rs`.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use super::choose::{RoomRequest, next_room};
use super::frustration;
use super::profile::{AdaptEvent, Cue, PlayerProfile, RoomResult, reduce};
use super::skill::{Band, Skill};
use super::window::Outcome;

/// A pretend player: per-skill success curves (logistic in the band), plus habits.
#[derive(Debug, Clone, Copy)]
pub struct SyntheticPlayer {
    pub name: &'static str,
    pub about: &'static str,
    /// Per skill: the band at which an unassisted room is cleared cleanly half the time.
    pub ability: [f32; Skill::COUNT],
    /// Bands per logistic unit: bigger is flatter (more inconsistent).
    pub slope: f32,
    /// Ability gained with practice: `+learn · n / (n + LEARN_HALF)` after `n` rooms.
    pub learn: f32,
    /// Chance a clean room still gets one quick slip death.
    pub careless: f32,
    /// Mean time-to-clear as a fraction of par.
    pub speed: f32,
    /// Chance of idling > 15s after a death in a struggling room.
    pub idles: f32,
    /// Chance of restarting a struggling room over and over.
    pub restarts: f32,
}

pub const LEARN_HALF: f32 = 100.0;
/// At full assist dial, this share of the remaining failure chance goes away.
pub const ASSIST_HELP: f32 = 0.5;
pub const PAR_SECS: f32 = 30.0;
/// Rooms ignored by the "after warm-up" metrics (calibration + settling).
pub const WARMUP: usize = 30;
/// Trailing window for "time near target".
pub const TRAILING: usize = 20;
pub const TARGET: (f32, f32) = (0.6, 0.85);
/// A band change that reverses the same skill's previous change within this many rooms is an
/// oscillation (ping-pong). Slower reversals are just tracking a player's level.
pub const OSC_SPAN: usize = 25;

const fn all(a: f32) -> [f32; Skill::COUNT] {
    [a; Skill::COUNT]
}

pub const PLAYERS: [SyntheticPlayer; 6] = [
    SyntheticPlayer {
        name: "precise",
        about: "consistent and good at everything",
        ability: all(8.0),
        slope: 0.7,
        learn: 0.0,
        careless: 0.02,
        speed: 0.9,
        idles: 0.0,
        restarts: 0.0,
    },
    SyntheticPlayer {
        name: "sloppy",
        about: "decent but inconsistent, slips a lot",
        ability: all(5.5),
        slope: 1.6,
        learn: 0.0,
        careless: 0.15,
        speed: 0.8,
        idles: 0.0,
        restarts: 0.05,
    },
    SyntheticPlayer {
        name: "nervous_beginner",
        about: "struggles even at band 1, slow, idles after deaths",
        ability: all(1.0),
        slope: 1.0,
        learn: 0.0,
        careless: 0.0,
        speed: 1.7,
        idles: 0.25,
        restarts: 0.1,
    },
    SyntheticPlayer {
        name: "speedrunner",
        about: "excellent and fast, occasional quick slips",
        ability: all(9.5),
        slope: 0.6,
        learn: 0.0,
        careless: 0.12,
        speed: 0.5,
        idles: 0.0,
        restarts: 0.0,
    },
    SyntheticPlayer {
        name: "uneven",
        about: "great at jumping, bad at waltz",
        ability: [8.5, 5.5, 5.5, 5.5, 5.5, 1.5, 5.5, 5.5, 5.5],
        slope: 0.8,
        learn: 0.0,
        careless: 0.03,
        speed: 0.9,
        idles: 0.05,
        restarts: 0.0,
    },
    SyntheticPlayer {
        name: "learner",
        about: "starts weak, improves steadily with practice",
        ability: all(2.0),
        slope: 0.9,
        learn: 6.0,
        careless: 0.03,
        speed: 1.1,
        idles: 0.05,
        restarts: 0.0,
    },
];

pub fn player(name: &str) -> Option<SyntheticPlayer> {
    PLAYERS.iter().copied().find(|p| p.name == name)
}

impl SyntheticPlayer {
    /// Chance of clearing `skill` at `band` cleanly after `practice` rooms, with assist dial `dial`.
    pub fn p_clean(&self, skill: Skill, band: Band, practice: u32, dial: f32) -> f32 {
        let n = practice as f32;
        let ability = self.ability[skill.index()] + self.learn * n / (n + LEARN_HALF);
        let p = 1.0 / (1.0 + ((band as f32 - ability) / self.slope).exp());
        p + (1.0 - p) * ASSIST_HELP * dial.clamp(0.0, 1.0)
    }
}

/// Deaths a room of this skill is designed to cost.
pub fn expected_deaths(skill: Skill) -> u32 {
    if skill == Skill::Stains { 1 } else { 0 }
}

/// One simulated room.
#[derive(Debug, Clone)]
pub struct SimRoom {
    pub request: RoomRequest,
    pub outcome: Outcome,
    pub deaths: u32,
    /// Assist dial the room was played with.
    pub assists: f32,
    /// Every skill's center after the room.
    pub centers: [Band; Skill::COUNT],
    pub cues: Vec<Cue>,
}

#[derive(Debug, Clone)]
pub struct SimRun {
    pub player: SyntheticPlayer,
    pub rooms: Vec<SimRoom>,
    pub profile: PlayerProfile,
}

/// When each skill unlocks (room index), like the game's progression would: the first three
/// from the start, then one every [`UNLOCK_EVERY`] rooms; Han's gates with the long gap (the
/// story teaches both in level 3).
pub const UNLOCK_EVERY: usize = 15;
pub fn unlock_room(skill: Skill) -> usize {
    let skill = if skill == Skill::Buddy { Skill::FiredUp } else { skill };
    skill.index().saturating_sub(2) * UNLOCK_EVERY
}

/// Play `rooms` rooms (calibration included), unlocking skills on the [`unlock_room`] schedule.
pub fn run(player: SyntheticPlayer, seed: u64, rooms: usize) -> SimRun {
    run_with(player, seed, rooms, unlock_room)
}

/// [`run`] with a custom unlock schedule (`|_| 0` = everything from the start).
pub fn run_with(player: SyntheticPlayer, seed: u64, rooms: usize, unlock_at: impl Fn(Skill) -> usize) -> SimRun {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut profile = PlayerProfile::new();
    let mut out = Vec::with_capacity(rooms);
    let mut unlocked: Vec<Skill> = Vec::new();
    for i in 0..rooms {
        for s in Skill::ALL {
            if unlock_at(s) <= i && !unlocked.contains(&s) {
                unlocked.push(s);
                profile = reduce(profile, AdaptEvent::SkillUnlocked(s));
            }
        }
        let req = next_room(&profile, &unlocked, &mut rng);
        let expected = expected_deaths(req.skill);
        let dial = profile.assists;
        let start = AdaptEvent::RoomStarted {
            room_id: i as u64,
            skills: vec![(req.skill, req.band)],
            expected_deaths: expected,
            par_secs: Some(PAR_SECS),
        };
        profile = reduce(profile, start.clone());
        let mut cues = profile.cues.clone();

        let p = player.p_clean(req.skill, req.band, i as u32, dial);
        let clean = rng.random::<f32>() < p;
        let mut excess = 0;
        let mut first_death = None;
        let mut idle = 0.0;
        if clean {
            if rng.random::<f32>() < player.careless {
                excess = 1;
                first_death = Some(0.8);
            }
        } else {
            excess = 1;
            while excess < 6 && rng.random::<f32>() > p {
                excess += 1;
            }
            first_death = Some(2.0 + 18.0 * rng.random::<f32>());
            if rng.random::<f32>() < player.idles {
                idle = frustration::IDLE_SECS + 5.0;
            }
            if excess >= 2 && rng.random::<f32>() < player.restarts {
                for _ in 0..frustration::RESTARTS {
                    profile = reduce(profile, start.clone());
                    cues.extend(profile.cues.iter().copied());
                }
            }
        }
        if expected > 0 && first_death.is_none() {
            first_death = Some(3.0);
        }
        let deaths = expected + excess;
        let time = PAR_SECS * (player.speed * (0.85 + 0.3 * rng.random::<f32>()) + 0.25 * excess as f32);
        profile = reduce(
            profile,
            AdaptEvent::RoomFinished(RoomResult {
                deaths,
                time_secs: time,
                toots: (time / 6.0) as u32,
                idle_after_death_secs: idle,
                first_death_secs: first_death,
            }),
        );
        cues.extend(profile.cues.iter().copied());
        let outcome = profile.recent.last().map(|r| r.outcome).unwrap_or(Outcome::Struggle);
        out.push(SimRoom {
            request: req,
            outcome,
            deaths,
            assists: dial,
            centers: std::array::from_fn(|k| profile.skills[k].center),
            cues,
        });
    }
    SimRun { player, rooms: out, profile }
}

/// Summary numbers for a run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    pub rooms: usize,
    /// Clean clears over all rooms.
    pub clean_rate: f32,
    /// Clean clears after [`WARMUP`].
    pub clean_after_warmup: f32,
    /// Share of post-warm-up rooms whose trailing-[`TRAILING`] clean rate is within [`TARGET`].
    pub time_near_target: f32,
    /// Band changes that reverse the same skill's previous change within [`OSC_SPAN`] rooms
    /// (summed over skills).
    pub oscillations: usize,
    /// All reversals, however far apart.
    pub reversals: usize,
    pub promotions: usize,
    pub demotions: usize,
    pub frustrations: usize,
    /// Mean assist dial after warm-up.
    pub mean_assists: f32,
    /// Share of post-warm-up rooms played with the dial ≥ 0.9.
    pub maxed_assists: f32,
    pub final_centers: [Band; Skill::COUNT],
}

impl SimRun {
    pub fn metrics(&self) -> Metrics {
        let n = self.rooms.len();
        let clean = |r: &SimRoom| r.outcome == Outcome::Clean;
        let rate = |rs: &[SimRoom]| {
            if rs.is_empty() { 0.0 } else { rs.iter().filter(|r| clean(r)).count() as f32 / rs.len() as f32 }
        };
        let after = &self.rooms[WARMUP.min(n)..];
        let near = (WARMUP.min(n)..n)
            .filter(|&i| {
                let r = rate(&self.rooms[(i + 1).saturating_sub(TRAILING)..=i]);
                (TARGET.0..=TARGET.1).contains(&r)
            })
            .count();
        let (mut oscillations, mut reversals) = (0, 0);
        let mut last_dir = [0i32; Skill::COUNT];
        let mut last_at = [0usize; Skill::COUNT];
        let mut prev = self.rooms.first().map(|r| r.centers).unwrap_or([1; Skill::COUNT]);
        let mut calibrated = false;
        let (mut promotions, mut demotions, mut frustrations) = (0, 0, 0);
        for (i, r) in self.rooms.iter().enumerate() {
            for c in &r.cues {
                match c {
                    Cue::Promoted(..) => promotions += 1,
                    Cue::Demoted(..) => demotions += 1,
                    Cue::Encourage(_) => frustrations += 1,
                    Cue::Calibrated(_) => calibrated = true,
                    Cue::Eased(..) => {}
                }
            }
            if r.cues.iter().any(|c| matches!(c, Cue::Calibrated(_))) {
                prev = r.centers;
                continue;
            }
            if !calibrated {
                continue;
            }
            for k in 0..Skill::COUNT {
                let d = r.centers[k] as i32 - prev[k] as i32;
                if d != 0 {
                    let dir = d.signum();
                    if last_dir[k] != 0 && dir != last_dir[k] {
                        reversals += 1;
                        if i - last_at[k] <= OSC_SPAN {
                            oscillations += 1;
                        }
                    }
                    last_dir[k] = dir;
                    last_at[k] = i;
                }
            }
            prev = r.centers;
        }
        let mean = |f: &dyn Fn(&SimRoom) -> f32| {
            if after.is_empty() { 0.0 } else { after.iter().map(f).sum::<f32>() / after.len() as f32 }
        };
        Metrics {
            rooms: n,
            clean_rate: rate(&self.rooms),
            clean_after_warmup: rate(after),
            time_near_target: if after.is_empty() { 0.0 } else { near as f32 / after.len() as f32 },
            oscillations,
            reversals,
            promotions,
            demotions,
            frustrations,
            mean_assists: mean(&|r| r.assists),
            maxed_assists: mean(&|r| if r.assists >= 0.9 { 1.0 } else { 0.0 }),
            final_centers: self.profile.skills.each_ref().map(|s| s.center),
        }
    }
}
