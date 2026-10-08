# Nat Han Adventures — roadmap & handoff

Where the project stands, what's planned (with the agreed designs), and the decisions behind it.
Read with `CLAUDE.md` (how to build, test, run, record) and `docs/han-buddy.md` (Han's spec).

Live: https://rabbiveesh.github.io/nat-han-adventures/ · deploys on every push to `main`.

## What's built (all on `main`)
- **Story**: 10 levels, 5 worlds. Nat (the poo hero) + Han (the plumber, an AI buddy). Golden
  nuggets (ghost nuggets after a death count once), checkpoints, goal flag, level select, saving.
- **Music**: a real-time chiptune engine (`src/audio/live/`) playing public-domain (pre-1931)
  jazz standards from `music/*.song` (MML + chord chart + FamiTracker-style `[instruments]`).
  Four musicians (lead/comp/bass/drums) with freedom dials and a full ornament vocabulary
  (side-slipping, planing, enclosures, digital patterns, hemiola, reharm, hits, trading fours…),
  flourishes on game events (summon crash+fill, checkpoint fill, death wah-wah), and
  musician-chosen feels at high freedom (bossa, samba, rock, funk; music only; `src/audio/live/feel.rs`).
- **The band reacts to play** (`src/audio/director.rs`), and its mode changes the physics
  (`src/game/groove.rs`):
  | Mode | Summon / cause | Physics | Gates |
  |---|---|---|---|
  | Giant Steps (Coltrane changes) | 5 toots | low gravity, slower run | 6-tall giant walls |
  | Fired up (quartal) | 4 quick nuggets | run ×1.35 | 11-tile long gaps |
  | Waltz (3/4) | 3 evenly spaced plain hops | world on the beat; jump on ONE ×1.15 + weak toot | waltz rows (spray tunnels) |
  | Nervous (melodic minor) | 3+ deaths (grip is a level-long layer) | slow-mo; **sweaty grip** on grease | grease chutes |
  | Laughing band (tuning medley) | 2+ deaths since checkpoint; wears off 1 death / 10 s | bouncy landings, a nudge per phrase (below) | — |
  A summon resets progress toward every other summon; summons hold 20 s with keep-alive.
  The laughing band's medley nudges the physics phrase by phrase (`Groove::nudge`; nickname in
  the band readout, what it does on the groove badge): Just = sober (no bounce, no sway, "SOBER FOR A SEC"); Harmonic = toot up to
  ×11/8, never past a perfectly timed toot's apex ("OVERTONES!"); 7-TET = slippery landings +
  camera roll ("SEASICK"); Carlos alpha = Nat drawn at 85%, jumps ×0.95 ("MELTING");
  Bohlen–Pierce = gravity ×1.05 pulses on every third beat ("ALIEN"); and run speed staggers down
  to −8% with the pitch wobble (never faster). Nothing adds reach past normal physics (the gate
  margins are a few px: +8% run speed opens waltz rows), and the campaign stays beatable with the
  melting jump or the alien gravity in force all along (`Physics::laughing`).
- **Death as a mechanic**: splat stains on spikes (standable), rafts in sewage, side splats that
  fade at grease-chute ends; stain pits and chutes in the levels.
- **Han** (`docs/han-buddy.md`): graph navigation, plunger boost (refreshes the toot), mid-air
  intercepts, shield escort through sprays, sewage rafts, per-mode physics, weak boost + grumbles
  near band gates, overuse limit; buddy ledges, shield rows, chain chasms, buddy raft pools.
- **Adaptive difficulty** (`src/adapt/`, `src/game/adaptive.rs`): invisible story-mode assists
  (coyote/buffer/hitboxes/raft life/hidden respawn/Han eagerness & hints), band mood → musician
  freedom. Never shown to the player.
- **Free play** (`src/freeplay/`): procedural rooms from 8 templates, validated as they're
  added, chosen by the adaptive engine; seeds shown and remembered (adaptive, not fixed courses).
  On web the validation runs in a Web Worker (`src/freeplay/offload.rs`, the `roomgen` wasm),
  so a new room never hitches the game; the main thread takes over if the worker fails.
- **Tooling**: level validator library (`src/level/validate.rs`), formal checks (tier 1 in every
  test run, tier 2 `scripts/check-deep` + non-blocking CI), music editor (`cargo run --bin
  editor`), F9 debug dump, phone touch controls (`scripts/phone-shots`), deterministic video
  capture (`scripts/record-run --capture`), headless BRP driving (`scripts/headless-run`).

## In flight
Nothing: everything from the original session is merged. Deadly spray cans landed (a can's tile
kills while firing; Han's plug covers it; free-play waltz rows are floor cans with a low roof),
and the deep checker's chain model now uses Han's real head rules (`KNOWN_BYPASSES` is empty).

## Planned (agreed, not started)
- **Web beat-clock latency**: add the browser's output latency to the beat clock (the world's beat
  runs a few tens of ms ahead of the ear on web).
- **Han's gates in free play**: buddy ledges, shield rows, chain chasms, buddy raft pools as room
  templates (extension point in `src/freeplay/templates.rs`), with a `Buddy` adaptive skill.
- **More band behaviours** (offered, the user hasn't picked yet): arranging across choruses
  (two-feel → walking, shout chorus with backgrounds), endings/tags (I–VI–ii–V tag, Basie ending
  on the jingle, vamp-outs), stop-time choruses, call-and-response comping, pedal/vamp intros,
  real dynamics (drop out under quiet phrases, build into the bridge).

## Known limits / follow-ups
- Validator: Giant Steps reaching an otherwise unreachable ledge and then crossing from it isn't
  chained across modes; the laughing-band bounce isn't modelled for stain pits.
- Hidden respawn (adaptive) can't prove the way on from its spot; guarded heuristically.
- Editor "as played" chord names for quartal/melodic minor are display approximations.

## Decisions (the user's calls — keep them)
- It's a loving prank on the brother (Nathan): Nat + Han. Cartoonish, never genuinely gross.
- Music: public domain only (published ≤ 1930). Melody follows reharms. Feels are music-only.
- Assists only ever loosen things above the floor the validator proves solvable.
- No music theory on screen: the HUD's groove badge says what the physics does (LOW GRAVITY,
  SLIPPERY LANDINGS), the band readout gives the band's mood by nickname (GIANT STEPS!, SEASICK).
- Seeds stay adaptive; last seed is remembered.
- Every mechanic is taught by Han (hint spots) before its first use; the validator enforces it.
- Builds at full parallelism under `nice -n 15`; tests with `cargo nextest run`; push to `main`
  after each finished feature (it deploys); clean up worktrees and stray Xvfbs when done.
