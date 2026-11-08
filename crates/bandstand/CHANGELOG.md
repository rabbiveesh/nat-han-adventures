# Changelog

All notable changes to bandstand are documented here. Format loosely follows
[Keep a Changelog](https://keepachangelog.com/); versions are the published crate
versions. A release needs a `## v<version> — <date>` section: its body becomes the
GitHub Release's notes.

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
