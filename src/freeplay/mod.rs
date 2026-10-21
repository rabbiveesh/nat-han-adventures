//! FREE PLAY: a run of procedurally generated rooms that adapts to the player, endless or
//! [`run::FIXED_ROOMS`] long, from a 6-digit seed anyone can type in again.
//!
//! - [`templates`]: hand-shaped rooms per [`crate::adapt::Skill`] (jump gauntlets, shelf climbs,
//!   flies and sprays, moving platforms, a giant wall, a long gap, a glide launch, a stain pit, a
//!   grease chute, and Han's gates: a mega wall, a buddy raft pool, a fired-up chasm, a cling shaft, a trapeze; and, from the level design checklist, a
//!   crossroads whose road the call picks, and a gadget duet: two gadgets taught, then combined)
//!   with parameters scaled by the band, the band's rooms opening on rolling ground (humps and
//!   dips) and the gauntlets climbing up to 7 tiles; [`templates::TEMPLATES`] is the registry
//!   new kinds of rooms go in.
//! - [`alone`]: Side B rooms (Han alone, the player calls the band), mixed in now and then
//!   ([`run::ALONE_CHANCE`]) once B1 is cleared, with the pieces of the Side B levels cleared,
//!   built with their solution instead of validated. Nat waits in the pipe: in the game it's a
//!   Side B level in miniature (`crate::game::sideb::start_room`), whose rewinds rewind only
//!   the room.
//! - [`canvas`]: the column builder rooms are drawn with, in the ordinary level format, framed
//!   by entry and exit pipes.
//! - [`generate`]: a plan becomes a room: seeded dice draw it, the level validator
//!   ([`crate::level::validate::check_room`]) checks it on its own (every gate, reachability
//!   start to end, the teaching rule, the deaths it needs), and a failure re-rolls.
//! - [`offload`]: on web, rooms are validated in a Web Worker (the `roomgen` wasm) so the game
//!   never hitches; elsewhere, or if the worker fails, on the task pool as before.
//! - [`course`]: rooms stitched pipe to pipe into one growing level, as tall as a climb, every
//!   room on its floor.
//! - [`run`]: the run in the game: the adaptive engine picks each next room
//!   ([`crate::adapt::next_room`]) as Nat enters the one before it, rooms are generated an
//!   attempt at a time off the main thread and streamed in a few columns per frame, rooms far
//!   behind are sealed off and unloaded, and every room posts `RoomStarted`/`RoomFinished` to
//!   the [`crate::game::AdaptiveProfile`].
//! - [`ui`]: the setup screen, the results card.
//!
//! Never tell the player the difficulty: nothing here shows a band.

pub mod alone;
pub mod canvas;
pub mod course;
pub mod dice;
pub mod generate;
pub mod offload;
pub mod run;
pub mod sets;
pub mod templates;
pub mod ui;

pub use run::{FIXED_ROOMS, FreePlayRun, FreePlaySettings, StartFreePlay};

/// The simulation half (part of [`crate::gameplay`]).
pub fn plugin(app: &mut bevy::prelude::App) {
    run::plugin(app);
}
