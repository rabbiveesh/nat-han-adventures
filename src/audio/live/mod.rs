//! The live music engine: the soundtrack generated in small blocks just ahead of the playhead,
//! so the band can respond to the game as it happens (and, later, improvise). It's the game's
//! music ([`crate::audio::plugin`] plays it), and the offline renderer
//! ([`crate::audio::synth::render_song_with`]) is the same engine run to the end.
//!
//! The engine has no dependency on the game: [`Engine`] takes a [`SongFile`] and [`Input`]s
//! that mirror gameplay, so a native music editor can run it from dials. Only the Bevy plugin
//! ([`crate::audio::plugin`]) and [`library::stem`] know about the game.
//!
//! # Modules
//! - [`song`]: `.song` files (`music/*.song`, `include_str!`'d by [`library`]); the grammar
//!   is data in [`syntax::CHEAT_SHEET`]; [`crate::audio::mml`] is the MML dialect (with
//!   `{c e g}` chords, `;` comments and checked `|` bar lines).
//! - [`engine`]: [`Engine`], [`Input`], [`EngineState`], [`BeatClock`]: time, the bar-level
//!   scheduler, the inputs.
//! - [`arrange`]: the written parts in each harmony (the `accomp` / `melody` / `waltz`
//!   passes, run once per song), read bar by bar; the waltz on its own 3/4 [`arrange::Shape`].
//! - [`musician`]: the band behind the [`musician::Musician`] trait: plans, commits, freedom.
//! - [`voice`]: the NES voices, streaming.
//! - [`playback`]: a kira [`kira::sound::Sound`] running engines on the audio thread.
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
//! needs no re-render. The waltz has its own 3/4 shape: switching into or out of it waits for a
//! bar line both meters share (see [`engine`]).
//!
//! The director stays where it is ([`crate::audio::director`]); its decisions arrive as
//! [`Input::SetFilters`]. ([`EngineConfig::self_directed`] runs it inside the engine, for an
//! editor without a game.)
//!
//! # Fidelity
//! At freedom 0 the engine plays exactly what the old pre-rendering engine rendered (checked
//! bit for bit, every song through every filter, the waltz included, before that engine was
//! retired; `tests/live.rs` keeps fingerprints of those renders). A fresh engine's first bar
//! lacks only the previous pass's drum tails. Rendering costs ~25 µs per 512-frame block in a
//! release build with everything on (Coltrane + medley + freedom), ~600× real time.

pub mod arrange;
pub mod engine;
pub mod instrument;
pub mod library;
pub mod musician;
pub mod playback;
pub mod song;
pub mod syntax;
pub mod voice;

pub use engine::{BeatClock, Engine, EngineConfig, EngineState, Input};
pub use song::SongFile;
