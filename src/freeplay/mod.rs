//! FREE PLAY: a run of procedurally generated rooms that adapts to the player, endless or
//! [`run::FIXED_ROOMS`] long, from a 6-digit seed anyone can type in again.
//!
//! - [`templates`]: one hand-shaped room per [`crate::adapt::Skill`] (jump gauntlets, flies and
//!   sprays, moving platforms, a giant wall, a long gap, a waltz row, a stain pit, a grease
//!   chute, and Han's gates: a buddy ledge, a buddy raft pool, a shield row, a chain chasm)
//!   with parameters scaled by the band; [`templates::TEMPLATES`] is the registry new
//!   kinds of rooms go in.
//! - [`canvas`]: the column builder rooms are drawn with, in the ordinary level format, framed
//!   by entry and exit pipes.
//! - [`generate`]: a plan becomes a room: seeded dice draw it, the level validator
//!   ([`crate::level::validate::check_room`]) checks it on its own (every gate, reachability
//!   start to end, the teaching rule, the deaths it needs), and a failure re-rolls.
//! - [`course`]: rooms stitched pipe to pipe into one growing level.
//! - [`run`]: the run in the game: the adaptive engine picks each next room
//!   ([`crate::adapt::next_room`]) as Nat enters the one before it, rooms are generated an
//!   attempt at a time off the main thread and streamed in a few columns per frame, rooms far
//!   behind are sealed off and unloaded, and every room posts `RoomStarted`/`RoomFinished` to
//!   the [`crate::game::AdaptiveProfile`].
//! - [`ui`]: the setup screen, the results card.
//!
//! Never tell the player the difficulty: nothing here shows a band.

pub mod canvas;
pub mod course;
pub mod dice;
pub mod generate;
pub mod run;
pub mod templates;
pub mod ui;

pub use run::{FIXED_ROOMS, FreePlayRun, FreePlaySettings, StartFreePlay};

/// The simulation half (part of [`crate::gameplay`]).
pub fn plugin(app: &mut bevy::prelude::App) {
    run::plugin(app);
}
