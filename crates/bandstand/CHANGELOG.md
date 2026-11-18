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
- **Sections sound like players** (SoundFont): each voice of a section chord a few ms
  off and its own velocity, the lead voice on top; the guitar strums; held horn notes
  grow a vibrato (CC 1). `big_band` picks the jazz kit (the drum channel's program now
  picks the kit) and the guitar plays the GM jazz guitar. Tried with GeneralUser GS.
- **Inner voices**: each section's pads take a device: a bloom (from a unison out to the
  voicing in contrary motion), a pyramid (voices entering bottom up), a line cliché in
  an inner voice, or a 4-3 suspension.
- A new note takes its channel's bend, expression and vibrato over (a fall no longer
  drags the next note flat).
- **Call and response**: an answer echoes what the band just heard, the line's last two
  to four notes in their rhythm and contour (or upside down), else one of the tune's own
  motifs, else a stock answer; a riff is the tune's motif half the time (three quarters
  in a shout), taken from any of its first four phrases; punches with no planned hits
  kick the tune's anticipations. The sections are busier (they lay out far less, a free
  band's riff plays on without breathing, an answer can come every bar) and the riffs
  louder.
- **Mixed like a big band record** (SoundFont): `big_band` puts the horns on top (the
  trumpets the apex, the saxes a few dB under, the low horns and drums under them, the
  bass, piano and guitar the floor), the stage panned (saxes left, brass right, piano
  left, guitar right, soloist, bass and drums centred) with a room round the horns and
  the rhythm section drier, and the piano plays in the holes: with upper sections on the
  stand its chords drop out where a section chord is struck or held.
- **Dynamics you can hear** (SoundFont): the velocity curve follows the musicians'
  volume about as a player's level goes (12 against 15 about 3 dB, 6 about 13 dB; it was
  under 3 dB across the whole range), every single note struck a little differently, and
  the default gain up to match (3.0). With sections the tune is shaped like a horn
  player shapes it (louder up high, the 8th before an and ghosted, long notes swelling,
  the section rising to its seventh bar); the sections play two under what's asked, so
  accents and the build (now to +3) have room, and the punches build and lean on
  anticipations. `big_band`'s trims rebalanced to keep each part's level.
- **The devices in more places**: each pass picks its colour (a device, unison or not, an
  ending). A riff's long notes open up with the device and its quick ones go unison; an
  answer can start in unison and bloom on its last note; the shout plays its fast notes in
  octaves (trumpets over saxes) and ends a phrase in a wedge (top up, bottom down a half
  step at a time) or a bloom.
- **Less is more**: the sections and low horns lay out far more, a riff behind a soloist
  breathes two bars on and two off, one answer a bar at most.
- **Soli with sections**: in a soli chorus the saxes play the quartet's counterpoint
  itself (the tune and the alto line on the altos, the tenor line on the tenors), up
  front; the block-chord soli is louder.
- With upper sections on the stand the band reharms a third as often, and only behind
  a solo (`BandInput::sections`).
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
