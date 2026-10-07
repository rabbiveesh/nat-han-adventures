# durhay (working title)

A 2D platformer: a brave little poo named Nat (the hero) runs, jumps and toot-double-jumps through 10 levels
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
- `src/level.rs` the ASCII level format (documented at the top of the file) and parser.
- `src/game/` simulation (+ `visuals_plugin`: sprites, animation, camera, particles).
- `src/art/` `Sprites` resource + `SpriteId`: pixel art built at startup.
- `src/audio/` MML parser, NES-style synth, songs, sfx, playback via bevy_kira_audio.
- `src/ui/` title, level select, HUD, pause, results, victory, Gus's speech bubble. `src/save.rs` progress.

## Look
- Virtual resolution: 216px tall (13.5 tiles), width follows the window aspect. Camera2d with
  `ScalingMode::FixedVertical { viewport_height: 216.0 }`. Nearest-neighbour sampling.
- Z order: backdrop -100, tiles 0, moving platforms 1, things (nuggets, hazards, checkpoints, goal) 2,
  Gus 4, player 5, particles 6, speech bubble 10.
- Text: Press Start 2P at multiples of 8px.

## Dev loop
1. `cargo build` with no warnings.
2. `cargo test` (unit tests + `tests/*.rs` headless gameplay/level tests). Prefer adding a test over eyeballing.
3. To see the real game: `scripts/headless-run` (Xvfb + lavapipe, no window), then drive it with
   `scripts/brp` on port **15799** (`brp_extras/send_keys`, `brp_extras/screenshot`). Never use 15702
   (the user's own game).
