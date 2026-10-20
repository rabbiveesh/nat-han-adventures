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
//! He follows Nat right up to the band's gates (the level's `gate:` marks), keeping off only a grease
//! chute's grease ([`Level::han_keeps_out`](crate::level::Level)).
//! He takes off on the run when the jump still lands right from there at that speed (else he
//! stops at the take-off point, as planned). **Lost** (no route, or stuck for [`STUCK_SECS`]),
//! or left behind for [`FALL_BEHIND_SECS`], and out of sight, he
//! **parachutes in** on his plunger from above Nat; he never teleports where you can see.
//!
//! **Mechanics.**
//! - His head is a one-way platform ([`HanHead`]); jumping off it is the **plunger boost**
//!   ([`BOOST_SPEED`](super::BOOST_SPEED), which also refreshes Nat's toot). He **braces**
//!   (plunger up) when Nat is coming down for one.
//! - **Fired up** he runs as fast as Nat does and jumping off his head is the **running
//!   throw** ([`THROW_SPEED`](super::THROW_SPEED), flat, Nat keeping
//!   [`THROW_CARRY`](super::THROW_CARRY) on top of his run). Over a fired-up chasm (its
//!   `chain` mark) he catches Nat in mid-air to throw him again, up to
//!   [`HAN_CHASM_CATCH_RISE`](crate::level::buddy::HAN_CHASM_CATCH_RISE) above where he last
//!   stood; nowhere else does his head hold Nat in mid-air.
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

pub use brain::{HanBrain, HanMode, HanNav, nav_cell, stand};
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
                .after(super::crushers::move_crushers)
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
/// How fast Han scrambles up a cling shaft's wall (px/s).
pub const CLIMB_SPEED: f32 = 120.0;
/// The longest Han clings to a wall before he slides off (s).
pub const CLING_SECS: f32 = 8.0;
/// Han's bark when Nat boings off his head (the laughing band).
pub const BOING_LINE: &str = "BOING!";
/// Han's bark when a crusher flattens him (he's back by parachute [`HAN_SQUASH_RESPAWN`] s
/// later; on Side B it's his end).
pub const SQUASH_LINE: &str = "Pancaked!";
pub const HAN_SQUASH_RESPAWN: f32 = 2.0;
/// How fast Han reels himself up his plunger handle onto the trapeze (px/s).
pub const REEL_SPEED: f32 = 320.0;
/// Han swings on the trapeze at most this long (s) before he lets go.
pub const TRAPEZE_SECS: f32 = 15.0;
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

/// What Han says: barks of 1-3 words ([`crate::level::MAX_BARK_WORDS`]); lesson cards teach,
/// he doesn't.
pub const LEMME_LINE: &str = "Lemme check.";
pub const PRO_LINE: &str = "I'm fine!";
pub const BACK_WARN_LINE: &str = "Oof. Back...";
pub const UNION_LINE: &str = "Union break!";
/// What Han grumbles when Nat jumps off his head in a band zone (the weak boost), in turn with
/// the lines of the gate's kind ([`grumble_lines`]).
pub const GRUMBLE_LINES: &[&str] = &["Nope.", "Too tired.", "Band work!"];

/// Han's grumbles for a band zone of gate kind `topic` (on top of [`GRUMBLE_LINES`]).
pub fn grumble_lines(topic: Topic) -> &'static [&'static str] {
    match topic {
        Topic::Giant => &["Toot it up!", "Giant Steps!"],
        Topic::Gap => &["Nuggets first!", "Fire 'em up!"],
        Topic::Waltz => &["Waltz job!", "ONE-two-three!"],
        Topic::Grip => &["Too slippy!", "Get sweaty!"],
        Topic::Stain => &["Splat job!", "Splat it!"],
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
pub const JET_LINES: &[&str] = &["Pssht! Ha!", "Lavender?", "Tickles!"];
pub const FLY_LINE: &str = "Shoo! Shoo!";

/// What Han says when the player splats (every 3rd death).
pub const DEATH_LINES: &[&str] = &["Oof, Nat!", "Not plumbing!", "Shake it off!", "On the invoice!", "Happens!", "Around, Nat!"];

/// What Han says when the band turns nervous (3+ deaths) on a level with grease: sweaty grip.
pub const GRIP_LINE: &str = "Sweaty grip!";
/// ...and on a level without grease.
pub const NERVOUS_LINE: &str = "Slow-mo! Breathe.";

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
    /// Hands on knees: his back (overuse).
    Winded,
    /// Splatted in sewage, sinking.
    Splat,
    /// Giant Steps: paddling in the air.
    Paddle,
    /// The laughing band: rolling along.
    Roll,
    /// Clinging to a wall (a cling shaft): plunger stuck in it, cap up for Nat.
    Cling,
    /// On the trapeze (the waltz): hanging from his plunger stuck in the ceiling.
    Trapeze,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::is_bark;

    /// Han is nearly silent: everything he says (intros aside) is a bark.
    #[test]
    fn han_only_barks() {
        let topics = crate::level::Topic::ALL.into_iter().flat_map(|t| grumble_lines(t).iter());
        let lines = [LEMME_LINE, PRO_LINE, BACK_WARN_LINE, UNION_LINE, FLY_LINE, GRIP_LINE, NERVOUS_LINE, BOING_LINE]
            .into_iter()
            .chain(GRUMBLE_LINES.iter().copied())
            .chain(topics.copied())
            .chain(JET_LINES.iter().copied())
            .chain(DEATH_LINES.iter().copied())
            .chain(crate::game::pickups::CHECKPOINT_QUIPS.iter().copied())
            .chain(crate::freeplay::generate::CHECKPOINT_LINES.iter().copied())
            .chain(crate::freeplay::generate::CALIBRATION_LINES.iter().copied())
            .chain([crate::game::groove::WALTZ_ONE_LINE]);
        for l in lines {
            assert!(is_bark(l), "{l:?}");
        }
    }
}
