//! The adaptive difficulty engine: pure logic, no Bevy systems (wiring comes later).
//!
//! Adapted from robot-game's adaptive learning engine (band blending, ADR-001). The pieces:
//!
//! - [`PlayerProfile`] + [`reduce`]: a pure reducer over [`AdaptEvent`]s. Per [`Skill`]: a center
//!   band 1..=10, a spread 0..1 and a rolling window of the last 20 rooms. Globally: the assist
//!   dial 0..1 (separate from difficulty), calibration, a display-only streak, recent rooms.
//! - [`next_room`]: [`pick_skill`] (strengths vs growth, 60/40, 80/20 while frustrated) among the
//!   caller's unlocked skills, then a band from [`band_distribution`], as a [`RoomRequest`].
//! - [`calibration`]: the first rooms are a disguised placement test.
//! - [`band_mood`]: how hard the player is pushing, for the music.
//! - [`story`]: the same assists and frustration logic for the hand-made levels, without bands.
//! - [`sim`]: synthetic players for tuning and tests (`cargo run --example simulate -- --all`).
//!
//! Never tell the player: nothing here produces a label, and [`Cue`]s are for the game (Han's
//! lines, logs), not for display.
//!
//! # Rules
//!
//! - A room is a **clean clear** when deaths ≤ the room's expected deaths. Deaths within the
//!   expectation never count against you (stain rooms need dying). Time never matters: slow but
//!   clean is clean.
//! - **Carelessness filter**: exactly one excess death, under 1.5s into the room, right after 2
//!   clean clears is a slip ([`Outcome::Careless`]): it doesn't count in any accuracy, doesn't
//!   move bands or assists, and doesn't break the streak.
//! - **Assists fade before difficulty rises.** A clean clear lowers the dial by
//!   [`profile::ASSIST_FADE`]; each excess death raises it by [`profile::ASSIST_RISE`]. A
//!   promotion needs the dial ≤ [`profile::ASSIST_EPS`], and only rooms played that unassisted
//!   count as promotion evidence. So while assists are on, good play lowers assists instead of
//!   raising the band.
//! - **Promote** a skill at ≥75% clean over ≥4 rooms at its center (and ≥60% on stretch rooms if
//!   there were ≥2); **demote** at <50% over ≥4. Evidence is per epoch: a band change bumps the
//!   epoch, so old window entries never count toward the new center. Promote/demote narrow the
//!   spread; >75% clean over the skill's last ≥10 rooms widens it.
//! - **Frustration** ([`frustration`]): 3+ excess deaths in a room, idling >15s after a death,
//!   or 3 restarts of one room. Response, once per room: drop the pushing skill's band by 1,
//!   narrow its spread, raise assists by [`profile::FRUSTRATION_ASSISTS`], favour strengths
//!   80/20 for [`profile::FRUSTRATION_COOLDOWN`] rooms, and emit [`Cue::Encourage`] so Han says
//!   something nice.
//!
//! # Tuning
//!
//! See `src/adapt/README.md` for the simulator results the constants were tuned against.

pub mod assist;
pub mod calibration;
pub mod choose;
pub mod frustration;
pub mod mood;
pub mod profile;
pub mod sim;
pub mod skill;
pub mod story;
pub mod window;

pub use assist::AssistLevers;
pub use calibration::{Calibration, Placement, Probe};
pub use choose::{RoomRequest, band_distribution, next_room, pick_skill, sample_band};
pub use frustration::FrustrationSignal;
pub use mood::{BandMood, band_mood};
pub use profile::{AdaptEvent, Cue, PlayerProfile, RoomResult, SkillState, reduce};
pub use skill::{Band, Skill};
pub use story::{StoryAssist, StoryEvent, reduce_story};
pub use window::{Outcome, RollingWindow, WindowEntry};
