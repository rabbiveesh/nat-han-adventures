# Nat Han Adventures

A 2D platformer (crate `nat-han-adventures`): a brave little poo named Nat (the hero) runs, jumps and toot-double-jumps through 10 levels
of increasingly unsanitary terrain, followed everywhere by **Han the plumber**, his sidekick. The hero is named **Nat** (Nat + Han = the brother, Nathan).
Golden nuggets are the coins; toilet-paper holders are checkpoints; a plunger with a flag is the goal.
The soundtrack is chunky 8-bit covers of public-domain (pre-1931) jazz standards, synthesized at runtime.
It's a (loving) prank on the dev's brother: keep it cartoonish and silly, never genuinely gross.

## Ground rules
- **Rust everywhere. Use maintained crates where they fit**; hand-rolled where feel matters
  (platformer physics is hand-rolled on purpose: tile AABB collision gives tight, responsive control).
- Versions are pinned to **Bevy 0.19.1**. Model knowledge of Bevy APIs is often stale, so before
  using an API, grep the crate source/examples in `~/.cargo/registry/src/*/<crate>-<ver>/`.
  (Bevy 0.17+ calls buffered events *messages*: `#[derive(Message)]`, `add_message`, `MessageReader/Writer`.)
- **No asset files** except the font (`assets/fonts/PressStart2P-Regular.ttf`, OFL, loaded via
  `include_bytes!`). Sprites are generated from ASCII pixel grids (`src/art`), music/sfx from a synth
  (`src/audio`), levels are `include_str!`'d text (`levels/`). The web build loads nothing at runtime.
- **Never use xdotool or anything that grabs the user's screen/focus/keyboard.** The user works on this
  machine. Avoid `trunk build --release` (slow fat-LTO) unless asked; CI does that.
- Builds use `kache` as rustc-wrapper (machine-local `.cargo/config.local.toml`, gitignored).

## Layout
- `src/lib.rs`: `gameplay` (headless-testable: state, input, events, save, game) vs `presentation`
  (art, visuals, ui, audio). Tests build an app from `gameplay` alone on `MinimalPlugins`.
- `src/state.rs` AppState (Title, LevelSelect, Playing{Running,Paused}, LevelComplete, Victory) + `CurrentLevel`.
- `src/input.rs` leafwing `Action`s on one global entity: `Single<&ActionState<Action>>`.
- `src/events.rs` gameplay messages (NuggetCollected, Jumped, Landed, PlayerDied, ...).
- `src/level.rs` the ASCII level format (documented at the top of the file, gate marks
  included) and parser; `src/level/validate.rs` the level validator (reachability, platform carry, gates,
  splat stains, grease chutes, Han's gates, teaching hints); `src/level/nav.rs` Han's lazily
  built nav graph; `src/level/buddy.rs` Han's physics and reflexes (shared by the game and the
  validator). Pure and wasm-safe; `tests/levels.rs` runs the validator on the campaign.
- `src/game/` simulation (+ `visuals_plugin`: sprites, animation, camera, particles).
  `src/game/han/` is Han, the AI buddy (`docs/han-buddy.md`); `tests/han.rs` and
  `tests/han_campaign.rs` play him headless.
- `src/adapt/` the adaptive difficulty engine (pure reducers); `src/game/adaptive.rs` wires it
  into play: story deaths/checkpoints → the invisible `Assists` (coyote, buffer, hitboxes, rafts,
  Han's eagerness, a hidden midway respawn, hint repeats), Han's encouragement, the band's
  freedom. Story segments are also per-skill evidence in the profile free play uses (every
  `PlayerDied` carries its cause and skill, `hazards::death_skill` from the gate marks); a death
  at a skill the profile finds easy is a *slip*, and only slips set the band laughing
  (`EasySkills`). `AssistMode` picks story / free play / manual (tests). Never shown on screen.
- `src/art/` `Sprites` resource + `SpriteId`: pixel art built at startup.
- `src/audio/` MML parser, NES-style synth blocks, sfx, the director, and the Bevy plugin
  (`audio::plugin`: music + sfx on one kira manager; `AudioOutput::Headless` for tests).
- `src/audio/live/` the real-time music engine (game-free): the game's music, generated a few
  ms ahead of the speaker so the band follows play (filters land on the next bar line; the
  waltz switches meter at a shared bar line). Songs are `music/*.song` (with FamiTracker-style
  `[instruments]`: macros, kits, palettes; `@i name` in the MML). The musicians ornament by
  their freedom dial (`musician/`, `ornament.rs`; the bar's shared `band.rs` plan keeps hits,
  reharms, trades, flourishes and the feel in step); at high freedom the band picks feels on
  its own (`feel.rs`: bossa, samba, rock, funk sections; music only, never in the waltz); each pass is
  a chorus the band arranges by the dial (`chorus.rs`: two-feel, stop-time, breaks, riffs,
  strolling, soli, shout + key-up), level tunes open with a pedal/vamp intro, `Input::End`
  plays an ending (the jingle gets a Basie one);
  `cargo run --release --example ornaments` renders listening WAVs with a log of what fired
  per bar (`-- <dir> feels`: the feels; `-- <dir> choruses`: the choruses, combos with feels, whole shows). Offline renders
  (`synth::render_song_with`, `examples/render_audio`, `examples/live_render`) run the same
  engine. `NATHAN_MUSIC=waltz+ji cargo run` forces the filters.
- `src/freeplay/` FREE PLAY: room templates per skill (registry in `templates.rs`), generator
  (seeded dice + `validate::check_room` with a lean jump set, re-roll on failure), rooms stitched
  pipe-to-pipe into one growing level, the adaptive engine picks each next room. On web the
  validation runs in a Web Worker (`offload.rs`; `src/bin/roomgen.rs`, a second Trunk wasm in
  `index.html`): only the plan and the verdict cross as text, the game redraws the room and
  checks its fingerprint, and falls back to the main thread on any mismatch or worker error.
  `cargo run --release --example freeplay_rooms` reports validity/timing per template and band.
- `src/game/sideb.rs` SIDE B, a hidden prototype: Nat's off screen, Han walks
  `levels/side-b/` alone and the player calls the band from each level's set list (`setlist:`
  header; Left/Right pick, Down call; `melt` holds the laughing band on its melting phrase and
  shrinks Han; a call holds 6 s, not the story's 20). Han just runs and jumps at what's ahead
  (`han::brain::charge`), so a wrong call and he comically dies, the level rewinds. Each
  level's solution is a plan in `tests/side_b.rs` (`try_a_plan` tries one). Ways in: Up Up Down Down on
  the title, `?sideb=1` on the web, `NATHAN_SIDE_B=1 cargo run`. `tests/side_b.rs`.
- `src/bin/level_editor/` the native level editor (`cargo run --bin level-editor [-- levels/x.txt]`;
  the `editor` feature): egui on Bevy, the level drawn with the game's sprites, palette from
  `level::GLYPHS` (a new tile is a `Tile` + a row there), gate marks, Han's lines, the tweak
  layer, Side B set lists; Validate (F7, on demand) draws the validator's reach map; Play (F5)
  runs the real game on it in a second window (`NATHAN_LEVEL=<file> cargo run`, reloads on
  save). Its document (`level::doc`) saves untouched lines byte for byte; new levels go in
  `levels/custom/`. Per-tile physics and Han's zones: `level::tweak` (a second `---` grid of
  letters); Nat and Han obey them, the validator and Han's nav don't. `tests/tweaks.rs`.
- `src/ui/` title, level select, HUD, the band's counter strip (`strip.rs`: charge, then Down
  calls the band) and its lesson cards (`lesson.rs`: every mechanic, since Han only barks 1-3 words; `NATHAN_LESSONS=0` turns them off; off in
  capture unless `=1`), pause, results, victory, Han's speech bubble. `src/save.rs` progress.
- `src/capture/` deterministic video capture (native dev, the `capture` feature): frame-stepped
  time, screenshots, lockstep audio, the input timeline (Dev loop 4).
- `src/bin/editor/` the native music editor (`cargo run --bin editor`; the `editor` feature, in
  `dev`, never in the game binary or the web build): egui on Bevy, the full live engine with
  gameplay simulated by buttons/dials (the same `Input`s and director as the game), tracker /
  piano roll / text views of one `.song` text, hot-swapped into the engine on every parse,
  Save writes `music/<song>.song`; an Instruments tab edits `[instruments]` (macro graphs,
  previews, try-outs). `--help` lists the scripting options; screenshots:
  `NATHAN_AUDIO=headless VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json
  xvfb-run -a cargo run --bin editor -- --view piano --play --screenshot out.png`.

## Look
- Virtual resolution: 216px tall (13.5 tiles), width follows the window aspect. Camera2d with
  `ScalingMode::FixedVertical { viewport_height: 216.0 }`. Nearest-neighbour sampling.
- Z order: backdrop -100, tiles 0, moving platforms 1, things (nuggets, hazards, checkpoints, goal) 2,
  Han 4, player 5, particles 6, speech bubble 10.
- Text: Press Start 2P at multiples of 8px.

## Dev loop
1. `cargo build` with no warnings. Run cargo under `nice -n 15` (full parallelism, low priority:
   the user's desktop stays responsive).
2. `cargo nextest run` (unit tests + `tests/*.rs` headless gameplay/level tests, all binaries in
   parallel, ~30 s; `cargo test --doc` for doctests). Wall-time budget tests sit in a serial
   `timing` group (`.config/nextest.toml`): name new timing tests so its filter catches them.
   Plain `cargo test` still works (~50 s, binaries in sequence). Prefer adding a test over eyeballing.
3. To see the real game: `scripts/headless-run` (Xvfb + lavapipe, no window), then drive it with
   `scripts/brp` on port **15799** (`brp_extras/send_keys`, `brp_extras/screenshot`). Never use 15702
   (the user's own game).
4. To record a video of a scripted run (smooth, audio in sync, the same every take, however
   busy the machine): `scripts/record-run --capture out.mp4 scripts/tours/flourish.timeline [level]`
   (`CAPTURE_FPS=60`, `CAPTURE_DIR=dir` keeps the PNG frames + `audio.wav`). It runs the game
   in capture mode (`NATHAN_CAPTURE=<dir>`, the `capture` feature in `dev`; `src/capture/`)
   under Xvfb + lavapipe: game time steps exactly one frame per rendered frame, each frame's
   screenshot is saved before the next update, music + sfx render in lockstep into a WAV
   (`src/audio/capture.rs`: 48000/fps samples a frame, so its length is exactly frames/fps),
   and keys come from a timeline in game time (`wait 4`, `hold Right Space 300ms`, `tap R`,
   `t=4.1 press Space`, `repeat 5` ... `end`; format in `src/capture/timeline.rs`) sent as
   keyboard messages, the path real keys take. Quits at the timeline's end; ~2x faster than
   real time here. `tests/capture.rs` checks it headless. The old real-time recorder (x11grab
   + a PulseAudio null sink + BRP keys, choppy under load) is `scripts/record-run out.mp4
   tours/flourish.sh` without `--capture`.
5. To see the phone/touch UI: `scripts/phone-shots [out-dir] [url]` drives headless Chrome at a
   landscape phone size with touch emulation (live Pages build with `?touch=1` by default) and
   saves screenshots of title → level select → level 1 with the stick and jump zone in use.
   `?touch=1`/`?touch=0` force touch mode on or off in any browser; `NATHAN_TOUCH=1 cargo run`
   does it natively. `?debug=1` shows an on-screen debug overlay (phones have no console; add lines with
   `nathanDebug.line(label, fn)` in `index.html`): the audio lag the beat clock compensates.
6. F9 in the game writes a debug dump (every reflected resource, Nat/Han state, director, raw
   save, recent events): a download on web, a file next to the save on native.

## Formal checks
Two tiers check the reachable states of the music × physics × levels, so soft-locks (a mode
you can never summon, grip that never comes) are caught before a player finds them.
- **Tier 1** (`tests/formal.rs`, every `cargo test` / `cargo nextest run`, ≤ 2 s CPU, enforced by
  `tier1_director_properties_within_budget`): the real `director::Band` (driven as
  `audio::plugin` does, physics via `Groove` + the grip layer) explored breadth-first over
  toots, nuggets, deaths, slips, checkpoints, ground jumps, calls, restarts and waits on a 0.25 s
  grid (depth 5, ~48k distinct states, deduplicated by an abstraction of the band that a test
  proves sound). From every state: (a) deaths always reach grip (also with ghost nuggets
  re-grabbed every lap); (b) 5 toots / 4 quick nuggets / 3 even ground jumps put their mode on
  the call (the full counter ticked last) and a call plays it at once; (c) with no input the harmony changes at most once; (d) holds expire without
  keep-alive input; (e) restart returns to the initial state; (f) the laughing band never
  blocks grip; (g) only a slip starts the laughing band. Mutants (the old chute soft-lock, a
  leaky restart, lazy calls, a band laughing at every death) must be caught.
- **Tier 2** (`tests/deep.rs`, `#[ignore]`d: `scripts/check-deep`, or `cargo nextest run --test
  deep --run-ignored only --no-capture`): the director deeper and with two events per frame;
  every campaign band gate crossable from every tier-1 music state with the level's own runway
  / nugget line / chute laps (a bar line's latency included); Han's chain boosts near band
  gates; Han's nav (every mode) or his parachute reaching each of his gates. Each test caches
  its pass in `target/tmp/deep-check/` keyed by a hash of `levels/*.txt` and the sources it
  reads (`SOURCES` in `tests/deep.rs`); unchanged, it prints "cached" (`--force` reruns). CI
  runs it on pushes to main and daily (`.github/workflows/deep-check.yml`, non-blocking).
  Known, accepted findings are listed in `tests/deep.rs` (`KNOWN`, `KNOWN_BYPASSES`) and
  printed every run; remove an entry once fixed (a stale one is reported).
- **Agents: run tier 2 (`scripts/check-deep`) whenever you touch the director, physics, the
  groove, Han or the levels**, and don't leave new counterexamples behind.
- **Reading a counterexample**: `property (a) fails: …`, then `state:` (the shortest trace
  from a level start to the state the property fails from), `full:` (with the property's own
  witness appended) and `because:`. A trace reads `t=0 nugget ×4 | t=1 death | t=2.25 wait`:
  frames at play time t (s; `×n`: n frames at that time; `a+b`: two events in one frame;
  `restart`), a final `wait` for when it ends. Turn one into a regression test by pasting its
  `state:` into `regressions` in `tests/formal.rs` (`regression("…")` replays it and checks
  every property from each state on the way); `formal_model::replay` gives the `Sim` to
  assert on directly.
