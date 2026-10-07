//! Chiptune audio: a tiny NES-style synth (2 pulse voices, triangle bass, noise drums) renders
//! songs written in MML to PCM at startup, and hands them to bevy_kira_audio. No audio files.
//!
//! Modules:
//! - [`mml`]: parses the MML dialect below into note events.
//! - [`synth`]: renders a [`Song`] / sound effect to stereo frames.
//! - [`songs`]: the soundtrack — chunky 8-bit takes on public-domain (pre-1931) jazz standards.
//! - [`sfx`]: sound effects (toot, splat, nugget, flush, ...).
//!
//! # MML dialect
//! Whitespace and `|` (bar lines) are ignored.
//! - Notes `c d e f g a b`, optional accidental `+`/`#` (sharp) or `-` (flat), optional length
//!   (`1 2 4 8 16 32`, whole..32nd; default from `l`), optional `.` (dotted, x1.5).
//!   e.g. `c4 e-8 g+16. a`
//! - `r` rest, with the same length rules.
//! - `o<n>` set octave (o4 contains middle-C = c), `>` octave up, `<` octave down.
//! - `l<n>` default note length. `t<bpm>` is NOT used: tempo is [`Song::bpm`].
//! - `v<0-15>` volume. `@<0-3>` pulse duty: 12.5%, 25%, 50%, 75% (pulse channels only).
//! - `&` between two notes ties them (no re-attack), e.g. `c4&c16`.
//! - `[ ... ]<n>` repeats the bracketed part n times (nestable).
//! - Noise channel (drums) uses drum letters instead of notes: `k` kick, `s` snare, `h` closed hat,
//!   `H` open hat, `r` rest — same length rules, e.g. `k8 h8 s8 h8`.
//!
//! Swing ([`Song::swing`]): 0.0 = straight; 0.33 ≈ triplet swing. Off-beat 8th notes are
//! delayed by `swing * (an 8th)` and the preceding on-beat 8th lengthened to match.

pub mod mml;
pub mod sfx;
pub mod songs;
pub mod synth;

use bevy::prelude::*;

pub fn plugin(_app: &mut App) {}

/// Which piece of music to play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum Music {
    Title,
    /// Level music for world 1..=5.
    World(u8),
    /// Short fanfare on reaching the goal (doesn't loop).
    LevelClear,
    /// After level 10: credits music.
    Victory,
}

impl Music {
    pub const ALL: [Music; 8] = [
        Music::Title,
        Music::World(1),
        Music::World(2),
        Music::World(3),
        Music::World(4),
        Music::World(5),
        Music::LevelClear,
        Music::Victory,
    ];
}

/// A song in MML. Tracks may differ in length; the song loops (if `looping`) at the end of the
/// longest one.
#[derive(Debug, Clone)]
pub struct Song {
    /// Title shown in credits, e.g. "Sweet Georgia Brown (1925)".
    pub title: &'static str,
    pub bpm: f32,
    pub swing: f32,
    pub looping: bool,
    /// Lead melody (pulse 1).
    pub pulse1: &'static str,
    /// Harmony / comping (pulse 2).
    pub pulse2: &'static str,
    /// Walking bass (triangle).
    pub triangle: &'static str,
    /// Drums (noise).
    pub noise: &'static str,
}

/// Sound effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum Sfx {
    Jump,
    /// Double jump: a short comedic toot.
    Toot,
    Land,
    Nugget,
    Splat,
    Checkpoint,
    /// Toilet flush on reaching the goal.
    Flush,
    MenuMove,
    MenuSelect,
    /// Gus talking: a little "blip blip" babble.
    GusBlip,
}
