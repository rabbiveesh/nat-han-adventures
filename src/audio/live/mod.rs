//! The live music engine (work in progress; see the bottom of this comment for the plan).

pub mod arrange;
pub mod convert;
pub mod engine;
pub mod library;
pub mod mml;
pub mod musician;
pub mod song;
pub mod syntax;
pub mod voice;

pub use engine::{BeatClock, Engine, EngineConfig, EngineState, Input};
pub use song::SongFile;
