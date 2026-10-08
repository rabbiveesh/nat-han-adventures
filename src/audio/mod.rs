//! Chiptune audio: a tiny NES-style synth (2 pulse voices, triangle bass, noise drums) plays
//! songs written in MML, generated live a few milliseconds ahead of the speaker so the band
//! can follow the game ([`live`]). No audio files.
//!
//! Modules:
//! - [`music`]: what the music is, game-free: [`Music`], [`Filters`] / [`Harmony`], [`Sfx`].
//! - [`live`]: the engine (the band, the scheduler, the streaming voices, the kira sound) and
//!   the `.song` files in `music/` ([`live::library`], [`live::song`]).
//! - [`mml`]: parses the MML dialect below into note events.
//! - [`synth`]: the synth's building blocks, and offline rendering (the engine run to the end).
//! - [`sfx`]: sound effects (toot, splat, nugget, flush, ...).
//! - [`chart`]: chord charts; [`theory`]: just intonation, Coltrane changes, melodic-minor and
//!   quartal harmony; [`accomp`]: generated comping + bass for the reharmonizing [`Filters`];
//!   [`melody`]: the melody following the new chords; [`waltz`]: the song re-cut into 3/4 (a
//!   time warp); [`tuning`]: the laughing band's tunings; [`director`]: picks the filters from
//!   how the player is doing; [`demo`]: a ii-V-I exercise for hearing the filters.
//! - [`mod@plugin`]: the Bevy plugin (music, the director, sfx; see there).
//!
//! # MML dialect
//! Whitespace is ignored; `;` starts a comment to the end of the line; `|` is a bar line
//! (checked in `.song` files, see [`mml`]).
//! - Notes `c d e f g a b`, optional accidental `+`/`#` (sharp) or `-` (flat), optional length
//!   (`1 2 4 8 16 32`, whole..32nd; default from `l`), optional `.` (dotted, x1.5).
//!   e.g. `c4 e-8 g+16. a`
//! - `{c e g}8`: a chord, played as a fast arpeggio.
//! - `r` rest, with the same length rules.
//! - `o<n>` set octave (o4 contains middle-C = c), `>` octave up, `<` octave down.
//! - `l<n>` default note length. `t<bpm>` is NOT used: tempo is the song's `bpm`.
//! - `v<0-15>` volume. `@<0-3>` pulse duty: 12.5%, 25%, 50%, 75% (pulse channels only).
//! - `@i <name>` instrument from the song's `[instruments]` ([`live::instrument`]; `default`
//!   is the channel's built-in).
//! - `&` between two notes ties them (no re-attack), e.g. `c4&c16`.
//! - `[ ... ]<n>` repeats the bracketed part n times (nestable).
//! - Noise channel (drums) uses drum letters instead of notes: `k` kick, `s` snare, `h` closed hat,
//!   `H` open hat, `x` crash, `r` rest — same length rules, e.g. `k8 h8 s8 h8`.
//!
//! Swing (the song's `swing`): 0.0 = straight; 0.33 ≈ triplet swing. Off-beat 8th notes are
//! delayed by `swing * (an 8th)` and the preceding on-beat 8th lengthened to match.
//!
//! ## Dialect details (decisions where the above leaves room)
//! - Lengths: any `n` in `1..=96` means a 1/n note (so `l12` / `c12` are 8th-note triplets),
//!   and more than one dot is allowed (`c4..` = 4 + 8 + 16). A dot needs an explicit length.
//! - Octaves range over `o0..=o8`; `c-` / `b+` cross into the neighbouring octave.
//! - `v` scales every channel, the triangle and drums included (the real NES triangle has no
//!   volume, but it's handy for balancing). `@` is accepted but ignored on triangle/noise, and so
//!   are `o < >` on the noise channel.
//! - `&`: tying to the *same* pitch makes one longer note; tying to a *different* pitch is a slur
//!   (pitch changes, no re-attack). Commands may sit between `&` and the note (`b4& >c4`).
//!   `r4&r8` extends a rest. `&` before a rest/drum after a note is an error.
//! - `[ ... ]` without a count repeats twice. State changes inside a repeat (octave, volume, ...)
//!   carry over exactly as if the body were written out n times.
//! - Swing (see [`synth::apply_swing`]) moves only 8th notes/rests that start on an off-beat 8th
//!   position; the event just before is lengthened to meet them. 16ths, dotted rhythms and
//!   downbeats never move, and each event is placed from its own unswung time (no drift).
//! - Notes play their full written length (with a ~2ms attack and ~8ms release inside it).
//!   Drums ring for their natural length regardless of the written length (open hats are choked
//!   by the next hit). A looping song's length is the longest track's length (trailing rests
//!   count), and anything ringing past the loop point rings on over the next pass.
//! - Melodic channels are case-sensitive: notes are lowercase only; `t` is rejected.
//!
//! # Chord charts
//! One entry per 4/4 bar, bars separated by `|` (leading/trailing `|` and whitespace ignored),
//! covering the song from its first bar to its loop point — exactly `length_in_beats / 4` bars.
//! A bar holds 1, 2 or 4 space-separated chord tokens that split it evenly (4, 2+2, 1+1+1+1 beats).
//! `%` repeats the previous chord (as a whole bar `| % |` or inside one, `C % F C/E`).
//! Chord token: root `A`–`G` with optional `#`/`b`, then a quality:
//! `` (major triad), `6`, `maj7`, `7`, `9`, `7b9`, `7#9`, `7#5`, `7sus4`, `m`, `m6`, `m7`,
//! `mMaj7`, `m7b5`, `dim7`, `aug`; optionally `/<note>` for a slash bass. E.g.
//! `| D7 | % | G7 | % | C7 | % | F6 | Am7b5 D7 |`. Parsed by [`chart::parse`]; errors name
//! the bar and token. For harmonic analysis `6`/`maj7`/triads are all "major" (tonics are
//! usually written `F6`), and the dominant family is `7 9 7b9 7#9 7#5 7sus4`.
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

pub mod accomp;
pub mod chart;
pub mod demo;
pub mod director;
pub mod live;
pub mod melody;
pub mod mml;
pub mod music;
pub mod plugin;
pub mod sfx;
pub mod synth;
pub mod theory;
pub mod tuning;
pub mod waltz;

pub use music::{Filters, Harmony, Music, Sfx};
pub use plugin::{
    AudioOutput, Director, LiveClock, LivePlayer, MusicChanged, MusicOverride, MusicStarted, NowPlaying, SfxCount, desired_music,
    land_db, plugin,
};
