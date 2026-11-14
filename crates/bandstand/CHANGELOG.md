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
- **Big band.** A Freddie Green rhythm guitar (four to the bar, root-7-3 shells) and the
  upper sections (four trumpets, two altos, two tenors, a channel each), voiced from the
  arranging books: four-way close under the tune, drop 2, diminished passing chords,
  guide-tone pads, brass punches on the band's hits. Per section a role (soli, tutti,
  pads, punches, tacet), or `Input::ForceUpper`. `Horns::BIG_BAND`, `Horns::ALL`.
- **The arranger** (`live::arranger`) plans the upper sections a chorus at a time. In a
  head the tune is passed round (the lead, a sax soli, a trumpet, tenor or alto standing
  up, tutti on the last eight); in a solo chorus the soloist stands up from a section
  that suits them (Miles and Pops on trumpet, Trane, Sonny and Wayne on tenor, Bird on
  alto) and the backgrounds build behind them: alone, pads, then a two-bar riff (one
  per chorus, sometimes the tune's own opening rhythm, its theme) or answers in the
  soloist's rests. The theme plays the intro; in a shout the brass call and the saxes
  answer. New roles `Riff` and `Answers`; `Input::ForceMelody`; `CommittedBar::melody`.
- **Phrasing** (SoundFont voices): `Fx::swell` (a crescendo, or fp-crescendo), and the
  SoundFont now plays `Fx` slides and falls with the pitch bend (a 12-semitone range) and
  expression, so the lead's falls sound too. The sections phrase like a band: an 8th on
  the and accented, a phrase's last note clipped, quarters detached, long notes swelling,
  falls and doits off phrase ends, a build through each 8 bars; the low horns' held hits
  are struck and swell back.
- `FontVoicing::big_band` balances the band as big band records do (drums up, bass and
  guitar down); `FontVoicing::trims` sets any channel's level (MIDI expression).
- `Yes! We Have No Bananas` (1923) joins the library.
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
