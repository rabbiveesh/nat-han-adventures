//! The live music engine: the soundtrack generated in small blocks just ahead of the playhead,
//! so the band can respond to the game as it happens (and, later, improvise). It runs beside
//! the old pre-rendering engine ([`crate::audio::plugin`]) until the swap; A/B it with
//! `NATHAN_LIVE_MUSIC=1 cargo run` (native).
//!
//! The engine has no dependency on the game: [`Engine`] takes a [`SongFile`] and [`Input`]s
//! that mirror gameplay, so a native music editor can run it from dials. Only [`mod@plugin`] (the
//! Bevy glue) and [`library::stem`] know about the game.
//!
//! # Modules
//! - [`song`]: `.song` files (`music/*.song`, `include_str!`'d by [`library`]); the grammar
//!   is data in [`syntax::CHEAT_SHEET`]; [`mml`] is the MML dialect (the game's, plus `{c e g}`
//!   chords, `;` comments and checked `|` bar lines); [`convert`] made the files from
//!   `songs.rs`.
//! - [`engine`]: [`Engine`], [`Input`], [`EngineState`], [`BeatClock`]: time, the bar-level
//!   scheduler, the inputs.
//! - [`arrange`]: the written parts in each harmony (the existing `accomp` / `melody`
//!   passes, run once per song), read bar by bar.
//! - [`musician`]: the band behind the [`musician::Musician`] trait: plans, commits, freedom.
//! - [`voice`]: the NES voices, streaming.
//! - [`playback`]: a kira [`kira::sound::Sound`] running engines on the audio thread.
//! - [`mod@plugin`]: the Bevy plugin.
//!
//! # Architecture
//! ```text
//!  main thread                                  audio thread (kira renderer; on the web:
//!                                               cpal's WebAudio callback, main thread too)
//!  game messages ─┐                             ┌─────────────────────────────────────────┐
//!  director ──────┼─► LiveHandle ══ rtrb ══►    │ LiveSound: Command queue                 │
//!  NATHAN_MUSIC ──┘   (Input / Play / Volume)   │   └─ Deck (current, outgoing: crossfade) │
//!                                               │        Engine.fill(128 frames @ 32 kHz)  │
//!                                               │          inputs → commit due bars →      │
//!                                               │          musicians → VoiceBank → mix     │
//!                                               │        Hermite resample → device rate    │
//!  Groove, NowPlaying, ◄── Mutex (try_lock) ◄── │   publish EngineState + BeatClock        │
//!  LiveClock                                    └─────────────────────────────────────────┘
//! ```
//! - The engine renders at 32 kHz (the offline renderer's rate, so the output is
//!   bit-comparable) in 128-frame chunks, pulled by the sound as the device needs them: no
//!   rendered-audio ring buffer, so inputs reach the music within a few milliseconds (plus a
//!   bar, see below). On the web cpal's WebAudio host calls the same code from the main
//!   thread between frames; a block costs microseconds, so there's no worker or ring buffer
//!   there either.
//! - Inputs cross over an `rtrb` SPSC ring buffer (lock-free, no allocation); engines are
//!   built on the main thread, sent over boxed, and sent back to be freed. [`Engine::fill`]
//!   doesn't allocate (`tests/live_alloc.rs` checks).
//! - State comes back through a mutex the audio thread only `try_lock`s: it never waits.
//!
//! # Timing and the scheduler
//! Each bar is committed one beat before its bar line ([`EngineConfig::commit_lead_beats`]):
//! the musicians turn it into events in the harmony and tuning in force then, and those
//! events are final. Everything later is a plan ([`musician::PhrasePlan`], 2-8 bars toward a
//! cadence or section end) that inputs may change. A filter change therefore lands on the next
//! bar line that isn't committed yet: the next one, unless the input arrives in the last beat.
//! Harmonies are arranged once per song (symbolic, a few ms) and read bar by bar, so a change
//! needs no re-render and no position mapping.
//!
//! The director stays where it is ([`crate::audio::director`]); its decisions arrive as
//! [`Input::SetFilters`]. ([`EngineConfig::self_directed`] runs it inside the engine from
//! [`Input::SetStats`], for an editor without a game.)
//!
//! # Fidelity
//! At freedom 0 the engine plays exactly what the offline renderer renders: from the second
//! loop pass on, every song through every filter is bit-identical; the first pass differs only
//! in bar 1, where the offline loop has the end's drum tails wrapped onto its start (a fresh
//! engine has no previous pass ringing). Rendering costs ~25 µs per 512-frame block in a
//! release build with everything on (Coltrane + medley + freedom), ~600× real time.
//!
//! # The swap (after the waltz lands)
//! 1. Port the waltz: give [`arrange::Arrangement`] a waltz transform (or a `Harmony::Waltz`
//!    arrangement built by whatever the waltz adds to `accomp` / `melody`); if it changes the
//!    meter, build its [`arrange::Shape`] with 3/4 bars, switch shapes at a bar line, and send
//!    [`Input::WaltzStep`] from the game; expose its beat to physics via [`plugin::LiveClock`]
//!    (the world-on-the-beat reads [`BeatClock`]).
//! 2. Move `Harmony`, `Filters`, `Song`/`Music` out of `audio/mod.rs` into a game-free module
//!    (the engine uses `Harmony`/`Filters`, which today live next to the old plugin).
//! 3. Replace `audio::plugin`'s music half with [`fn@plugin`]: drop `MusicPlayer`, `RenderJob`,
//!    `SongSource` and the silent-song shim; keep sfx on bevy_kira_audio or move them onto the
//!    engine's kira manager (one output stream instead of two).
//! 4. Delete `songs.rs` (and its checker tests) for the `.song` files; make [`mml`] the only
//!    MML parser; fold the copied voice code ([`voice`]) back so `synth.rs` renders offline
//!    through the same voices (the offline renderer becomes "run the engine to the end").
//! 5. Port the old plugin tests (`tests/audio.rs`'s `plugin` module) to the headless
//!    [`plugin::LiveOutput::Headless`] mode (`tests/live_plugin.rs` shows how).

pub mod arrange;
pub mod convert;
pub mod engine;
pub mod library;
pub mod mml;
pub mod musician;
pub mod playback;
pub mod plugin;
pub mod song;
pub mod syntax;
pub mod voice;

pub use engine::{BeatClock, Engine, EngineConfig, EngineState, Input};
pub use plugin::{enabled, plugin};
pub use song::SongFile;
