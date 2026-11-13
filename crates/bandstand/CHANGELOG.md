# Changelog

All notable changes to bandstand are documented here. Format loosely follows
[Keep a Changelog](https://keepachangelog.com/); versions are the published crate
versions. A release needs a `## v<version> — <date>` section: its body becomes the
GitHub Release's notes.

## Unreleased

- **Low horns.** `EngineConfig::horns` puts a bass trombone, a bari and a tuba on the
  stand (`live::horns`), each on a channel of its own (5 on). Per 8-bar section they
  double the bass line (unison or octaves), hold a pedal, pad root-position long notes,
  hit ONE, lay out, or take the line while the bass lays out; rolled from the seed by
  the chorus, or `Input::ForceLowEnd`. No horns (the default) changes nothing.
- **Stems** (`soundfont`): `Engine::tap` / `take_tapped` collect every committed note;
  `live::stems` renders each part dry and mono through its own synth (they sum to the
  mix exactly) and writes its notes as text. `examples/stems.rs` makes seeded takes.
- `Input::SetChannelGain` mutes or levels any channel, the horns' too.
- **Breaking:** `font::CHANNELS` is 16 (a MIDI channel per engine channel), so
  `FontVoicing::programs` is `[Option<u8>; 16]`; `FontVoicing::with(ch, program)` sets
  one. `FontVoicing::combo` voices the horns too.

## v0.1.0 — 2026-11-07

The first release: the music engine behind Nat Han Adventures, as its own crate.

- **A live band in an NES-style synth.** Songs in MML with a chord chart, rendered a
  few milliseconds ahead of the speaker so a game can steer the band as it plays:
  reharmonised filters, feels (bossa, samba, rock, funk), arranged choruses
  (stop-time, breaks, riffs, soli in four voices, a shout chorus a key up), intros
  and endings.
- **Soloists like measured players.** Miles, Trane, Bird, Sonny, Wayne and Pops,
  from statistics of their transcribed solos (never their phrases), with each
  soloist's bassist walking under them, and the band reharmonising to suit them.
- **Off the grid.** Tempo drift, each player's pocket, and the mix ducking under
  the lead, measured from records.
- **12 public-domain tunes compiled in,** with their player profiles and bass lines.
- **No Bevy and no audio backend in the core;** builds for wasm. Features: `kira`
  (a kira `Sound` that plays engines on the audio thread), `reflect`
  (`bevy_reflect::Reflect` on the music types), `soundfont` (any channel through a
  General MIDI `.sf2`, live, via rustysynth).
