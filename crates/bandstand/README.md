# bandstand

A live jazz band in an NES-style chiptune synth, the music engine behind Nat Han Adventures.

Songs are written in MML with a chord chart. The engine renders them a few milliseconds ahead
of the speaker, so a game can steer the band while it plays: it reharmonises, changes feel
(bossa, samba, rock, funk), arranges choruses (stop-time, breaks, soli, a shout chorus a key
up) and lets soloists play like measured players. It has no asset files: the 12 public-domain
tunes, player profiles and bass lines are compiled in.

- The core has no Bevy and no audio backend, and builds for wasm. `Engine::fill` renders
  stereo `Frame`s.
- `kira`: `live::playback`, a kira `Sound` that runs engines on the audio thread.
- `reflect`: `bevy_reflect::Reflect` on the music types.
- `soundfont`: any channel can play through a General MIDI `.sf2` instead of the chip
  (`Engine::use_font`, via rustysynth), live.

```rust
use bandstand::{Frame, live::{Engine, Input, library}};

let mut engine = Engine::new(&library::load("tiger_rag").unwrap(), 32_000).unwrap();
engine.post(Input::LevelStart);
let mut out = [Frame::ZERO; 512];
engine.fill(&mut out);
```

MIT licensed.
