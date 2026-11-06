//! Audio: the music engine lives in the `bandstand` crate (`crates/bandstand`: the synth, MML,
//! charts, theory, the live band, the director); its modules are re-exported here so
//! `crate::audio::live::...` paths keep working. This module adds the Bevy side.
//!
//! # Playback
//! See [`mod@plugin`]: music follows the app state; the [`director`] picks new [`Filters`] from
//! how the player is doing, and the engine plays them from the next bar line (the waltz from
//! the next bar line both meters share), the tune carrying on. [`NowPlaying`] says what's on;
//! [`MusicStarted`] / [`MusicChanged`] fire when a track starts / switches filters; every frame
//! the music's clock (bar, beat, phase) goes into [`crate::game::Groove`], so the world can
//! dance to it (the waltz).
//!
//! Dev override (native only): `NATHAN_MUSIC=coltrane|quartal|melodic|waltz|original[+ji]` (or just
//! `ji`, the laughing band's [`tuning::Tuning::Medley`]) forces the filters of every looping song.

#[cfg(feature = "capture")]
pub mod capture;
pub mod plugin;

pub use bandstand::{Filters, Frame, Harmony, Music, Sfx};
pub use bandstand::{
    accomp, chart, demo, director, live, melody, mml, music, sfx, synth, theory, tuning, waltz,
};
pub use plugin::{
    AudioOutput, BandMeters, Calibrating, Director, LiveClock, LivePlayer, MusicChanged,
    MusicOverride, MusicStarted, NowPlaying, SfxCount, desired_music, land_db, plugin,
};
