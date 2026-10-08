//! Han the plumber, Nat's AI buddy (`docs/han-buddy.md`).
//!
//! **Body.** A real physics body ([`Body`], the shared [`step_body`](super::step_body)) with his
//! own per-mode physics ([`HanPhys`]): floatier under Giant Steps, can't keep up when the band
//! is fired up, steps on the beat in the waltz, clings and trembles when it's nervous, rolls
//! and bounces with the laughing band. Spikes, flies and spray jets don't hurt him; sewage does.
//! He has [`HAN_TOOTS`] toots, and nothing he does is a [`Jumped`](crate::events::Jumped): the
//! band never counts him.
//!
//! **Navigation** ([`brain`]). He plans routes on the level's jump graph
//! ([`Nav`](crate::level::nav::Nav): the validator's standable cells and simulated arcs with
//! his physics, built lazily and cached) to a follow slot [`FOLLOW_GAP`] behind Nat on the side
//! Nat came from, re-planning a few times a second when Nat moves on, and steers his body along
//! the walks, jumps and toots (with corrections), waiting for moving platforms to come round.
//! He follows Nat right up to the band's gates (the level's `gate:` marks), keeping out only of
//! a waltz row and a grease chute's grease ([`Level::han_keeps_out`](crate::level::Level)).
//! He takes off on the run when the jump still lands right from there at that speed (else he
//! stops at the take-off point, as planned). **Lost** (no route, or stuck for [`STUCK_SECS`]),
//! or left behind for [`FALL_BEHIND_SECS`], and out of sight, he
//! **parachutes in** on his plunger from above Nat; he never teleports where you can see.
//!
//! **Mechanics.**
//! - His head is a one-way platform ([`HanHead`]); jumping off it is the **plunger boost**
//!   ([`BOOST_SPEED`](super::BOOST_SPEED), which also refreshes Nat's toot). He **braces**
//!   (plunger up) when Nat is coming down for one. In mid-air his head holds Nat only up to
//!   [`HAN_CATCH_RISE`](crate::level::buddy::HAN_CATCH_RISE) above where he last stood.
//! - **The band zone** ([`HAN_BERTH`](crate::level::HAN_BERTH) columns around a band or death
//!   gate's mark, `Level::in_band_zone`): his boost is the weak one
//!   ([`WEAK_BOOST_SPEED`](super::WEAK_BOOST_SPEED), about a normal jump) and he grumbles
//!   ([`grumble_line`]); his head only holds Nat there while he's standing, he doesn't go for
//!   intercepts or go ahead. So the band is still what opens its gates.
//! - **Intercept**: Nat in the air, coming down near him (or over a drop): he runs, jumps and
//!   toots to get under Nat (the goalkeeper; [`intercept`](crate::level::buddy::intercept)), so
//!   mid-air chains work.
//! - **"Lemme check that"**: Nat standing still facing a hazard for [`go_ahead_delay`] → Han
//!   marches ahead into it, waiting for Nat to keep up ([`ESCORT_LEAD`]). Spray jets (and the
//!   cans firing them) stop at his body and stay plugged while he escorts Nat through (and [`HAN_PLUG_LINGER`] after:
//!   [`SprayPlug`]), flies bounce off him, and Nat walking behind him (he's solid from the side
//!   while he marches, so Nat can't overtake him into the jets) is safe. Into sewage: he splats and sinks, leaving a
//!   big raft ([`HAN_RAFT_WIDTH`] tiles, [`HAN_RAFT_LIFE_FLOOR`] s × the assist), and
//!   parachutes back [`HAN_SEWAGE_RESPAWN`] s later. Not Nat's death: nothing counts it.
//! - **Overuse**: more than [`overuse_limit`] boosts in a row and his back goes ("My back! I'm
//!   union, Nat!"): a [`breather`] with no boosts. No limit inside chain chasms.
//! - **Eagerness** ([`Assists::han_eagerness`](super::Assists)) scales all of it: brace and
//!   intercept range, the go-ahead delay, the overuse limit. (Repeating a hint after repeated
//!   deaths is the adaptive engine's `han_hint` lever, `game::adaptive`.)

mod brain;
mod world;

use bevy::prelude::*;

pub use brain::{HanBrain, HanMode, HanNav};
pub use world::{FlySpin, HanRaft, SprayPlug, han_raft_life};

use super::{ActiveLevel, GameSet};
use crate::level::{TILE, Topic};
pub use crate::level::buddy::{HAN_MARCH_SPEED, HAN_RUN_SPEED, HAN_TOOTS, HanPhys};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
            FixedUpdate,
            (brain::think, world::han_hazards)
                .chain()
                .in_set(GameSet::World)
                .after(super::platforms::move_platforms)
                .after(super::hazards::float_rafts)
                .after(super::hazards::move_flies)
                .after(super::hazards::update_sprays),
        )
        .add_systems(FixedUpdate, nervous_line.in_set(GameSet::Interact));
}

/// Han's follow slot: this far (px) behind Nat.
pub const FOLLOW_GAP: f32 = 1.5 * TILE;
/// ...when the band is nervous he clings closer.
pub const NERVOUS_GAP: f32 = 0.8 * TILE;
/// No progress toward his slot for this long (s): he's lost.
pub const STUCK_SECS: f32 = 3.0;
/// Running along to catch up, far from his slot, he goes up to this × his top speed.
pub const HAN_CATCH_UP: f32 = 1.3;
/// Left behind out of sight (Nat ran on) this long (s): he parachutes in, like when he's lost.
pub const FALL_BEHIND_SECS: f32 = 1.5;
/// Re-plan at most this often (s).
pub const REPLAN_SECS: f32 = 0.25;
/// Parachute: falling speed (px/s) and how high above Nat he starts (px).
pub const PARACHUTE_FALL: f32 = 70.0;
pub const PARACHUTE_HEIGHT: f32 = 8.5 * TILE;
/// Sinking in sewage (s), then out of sight before he parachutes back (s).
pub const HAN_SINK_TIME: f32 = 1.0;
pub const HAN_SEWAGE_RESPAWN: f32 = 3.0;
/// Han's raft: width (tiles) and how long it floats at least (s; × `Assists::raft_life_mult`).
pub const HAN_RAFT_WIDTH: usize = 3;
pub const HAN_RAFT_LIFE_FLOOR: f32 = 30.0;
/// Going ahead, Han waits for Nat when he's this far (px) in front of him.
pub const ESCORT_LEAD: f32 = 1.5 * TILE;
/// A spray jet Han walked through stays plugged (sputtering) this long after him (s), and all
/// the while he's escorting Nat through.
pub const HAN_PLUG_LINGER: f32 = 0.5;
/// How far ahead (tiles) a hazard counts as "facing" it.
pub const LOOK_AHEAD: i32 = 4;

/// What Han says.
pub const LEMME_LINE: &str = "Lemme check that.";
pub const PRO_LINE: &str = "I'm fine! I'm a professional!";
pub const BACK_WARN_LINE: &str = "Oof. My back...";
pub const UNION_LINE: &str = "My back! I'm union, Nat!";
/// What Han grumbles when Nat jumps off his head in a band zone (the weak boost), in turn with
/// the lines of the gate's kind ([`grumble_lines`]).
pub const GRUMBLE_LINES: &[&str] = &[
    "Nope. Need more music in my soul for that one.",
    "Too tired, Nat. Get the band going.",
    "That's band work, Nat. I'm just the plumber.",
];

/// Han's grumbles for a band zone of gate kind `topic` (on top of [`GRUMBLE_LINES`]).
pub fn grumble_lines(topic: Topic) -> &'static [&'static str] {
    match topic {
        Topic::Giant => &["My back says no. My heart says Giant Steps.", "Toot it up, Nat! Five toots fetch the giant."],
        Topic::Gap => &["Grab nuggets, get the band fired up!", "That gap wants a fired-up band, not a plumber."],
        Topic::Waltz => &["That's a waltz job, Nat!", "Hop in threes, Nat. ONE-two-three!"],
        Topic::Grip => &["Grease? Only a sweaty band gets a grip on that.", "Slippery job, Nat. Get the band nervous."],
        Topic::Stain => &["Spikes? That's a splat job, Nat. Sorry!", "Make your own stepping stones, Nat. Splat!"],
        _ => &[],
    }
}

/// Grumble number `n` (counting from 0) in a band zone of kind `topic` (`None`: a generated
/// room's zone): the kind's lines and the general ones, in turn.
pub fn grumble_line(topic: Option<Topic>, n: u32) -> &'static str {
    let own = topic.map_or(&[][..], grumble_lines);
    // Alternating: the gate's own line, a general one, ...
    let order: Vec<&str> = (0..own.len().max(GRUMBLE_LINES.len()))
        .flat_map(|i| [own.get(i), GRUMBLE_LINES.get(i)])
        .flatten()
        .copied()
        .collect();
    order[n as usize % order.len()]
}

/// At most one grumble this often (s).
pub const GRUMBLE_EVERY: f32 = 2.0;
pub const WHEEZE_LINE: &str = "Wheeze... too fast... go on, Nat!";
pub const JET_LINES: &[&str] = &["Pssht! Ha! Tickles!", "Smells like... lavender?", "Pssht yourself!"];
pub const FLY_LINE: &str = "Shoo! Shoo! Union rules!";

/// What Han says when the player splats (every 3rd death).
pub const DEATH_LINES: &[&str] = &[
    "Don't worry, Nat. I've unclogged worse.",
    "Nat! That's not how plumbing works!",
    "Shake it off, Nat. Gravity's just a big drain.",
    "Ooh. I'm puttin' that one on the invoice.",
    "Happens to the best of us. Mostly to you, Nat.",
    "Nat, you go AROUND the pointy stuff.",
];

/// What Han says when the band turns nervous (3+ deaths) on a level with grease: sweaty grip.
pub const GRIP_LINE: &str = "Band's nervous! Sweaty grip: now you can stop AND jump!";
/// ...and on a level without grease.
pub const NERVOUS_LINE: &str = "Band's sweatin' bullets. Slow-mo! Breathe, Nat.";

/// Eagerness `e` (0 lazy, 0.5 neutral, 1 very eager) → seconds Nat must stand facing a hazard
/// before Han goes ahead: 2.0 s lazy, 1.5 s neutral, 0.7 s eager.
pub fn go_ahead_delay(e: f32) -> f32 {
    let e = e.clamp(0.0, 1.0);
    if e <= 0.5 { 2.0 - e } else { 1.5 - 1.6 * (e - 0.5) }
}

/// Boosts in a row before his back goes: 3 lazy, 5 neutral, 7 eager.
pub fn overuse_limit(e: f32) -> u32 {
    3 + (4.0 * e.clamp(0.0, 1.0)).round() as u32
}

/// His breather (s, no boosts): 3 s lazy .. 1.5 s eager.
pub fn breather(e: f32) -> f32 {
    3.0 - 1.5 * e.clamp(0.0, 1.0)
}

/// How far (px, sideways) he braces for a Nat coming down: 1.5 tiles lazy .. 3.5 eager.
pub fn brace_range(e: f32) -> f32 {
    (1.5 + 2.0 * e.clamp(0.0, 1.0)) * TILE
}

/// How far (px, sideways) he goes for an intercept: 3 tiles lazy .. 7 eager.
pub fn intercept_range(e: f32) -> f32 {
    (3.0 + 4.0 * e.clamp(0.0, 1.0)) * TILE
}

/// Where Han starts: his slot behind (left of) Nat at `start`, else ahead, else on Nat.
pub fn spawn_spot(level: &crate::level::Level, start: Vec2) -> Vec2 {
    let half = crate::level::buddy::HALF;
    [-FOLLOW_GAP, FOLLOW_GAP]
        .into_iter()
        .map(|dx| start + Vec2::new(dx, 0.0))
        .find(|p| {
            let (a, b) = (level.cell_at(*p - half + 0.5), level.cell_at(*p + half - 0.5));
            (a.0..=b.0).all(|c| (b.1..=a.1).all(|r| !level.tile(c, r).is_solid()))
        })
        .unwrap_or(start)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
pub enum HanPose {
    #[default]
    Idle,
    Run,
    Jump,
    /// Plunger up, feet planted: come on down.
    Braced,
    /// Arms out like a goalkeeper.
    Intercept,
    /// Determined march into a hazard.
    March,
    /// Floating down on his plunger.
    Parachute,
    /// Hands on knees: his back (overuse), or wheezing (fired-up band).
    Winded,
    /// Splatted in sewage, sinking.
    Splat,
    /// Giant Steps: paddling in the air.
    Paddle,
    /// The laughing band: rolling along.
    Roll,
}

/// For visuals: what Han is doing and which way he faces.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Reflect)]
#[reflect(Component)]
pub struct HanAnim {
    pub pose: HanPose,
    pub facing_left: bool,
    /// The nervous band: he trembles.
    pub tremble: bool,
    /// Seconds of comedic wobble left (a jet or a fly just hit him).
    pub wobble: f32,
    /// Hidden (sunk in sewage, not back yet).
    pub hidden: bool,
}

/// Han's line the moment the band turns nervous.
fn nervous_line(
    groove: Res<super::Groove>,
    active: Res<ActiveLevel>,
    mut was: Local<crate::audio::Harmony>,
    mut says: MessageWriter<crate::events::HanSays>,
) {
    if groove.harmony == *was {
        return;
    }
    *was = groove.harmony;
    if groove.grip() {
        let text = if active.level.has_grease() { GRIP_LINE } else { NERVOUS_LINE };
        says.write(crate::events::HanSays { text: text.to_string() });
    }
}
