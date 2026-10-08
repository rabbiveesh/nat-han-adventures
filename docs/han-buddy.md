# Han as an AI buddy — design spec

Status: agreed with the user, not built yet. Builds after the level pass (it touches physics,
hazards and the level validator), alongside the adaptive-engine wiring.

## Body and navigation
- Han is a real physics body: same collisions, gravity and moving-platform riding as Nat, with
  **his own per-mode physics** (below). He's invulnerable to spikes, flies and sprays — but
  **not to sewage** (see "Han in sewage").
- He navigates the level's **jump graph** (the validator's reachability graph of standable cells
  and jump arcs, per mode), planning routes to a **follow slot ~1.5 tiles behind Nat** (on the
  side Nat came from) and executing the walks/jumps/toots with his own physics. Re-plans when
  Nat moves on meaningfully (a few times a second, not per frame); waits for moving platforms.
- **He has 3 toots** (forgiving navigation). **His toots never count for the band.**
- **Lost** (no route, e.g. Nat used a mode Han can't follow): he **parachutes in on his plunger**.
  Never teleports.
- **He refuses to go near giant walls** (Giant Steps gates): his AI keeps away from them, so they
  always need Giant Steps — no buddy shortcut.
- CPU: graph built once per level load (or precomputed at build time if too slow on phones);
  A* over a few thousand nodes well under 1 ms; trivial per-frame steering.

## Mechanics
- **Solid only on top, only from above**: a one-way platform on his head. You can walk and jump
  through him from the side or below.
- **Plunger boost**: land on his head — on the ground *or mid-air* — and press jump: he launches
  you (~6 tiles). **A boost refreshes Nat's toot**, so chains are possible: jump → toot → boost
  → toot → boost… capped by how high/far Han can follow with his 3 toots.
  - Note: a chain's toots count toward the Giant Steps summon (5 toots), so long chains summon
    Giant Steps mid-chain and gravity drops mid-combo. The validator must model this.
- **Intercept**: when Nat is airborne and descending near him, Han tries to get underneath
  (runs/jumps/toots) so you can bounce off him. This is what makes mid-air combos possible.
- **"Lemme check that"**: if Nat stands still facing a hazard for ~1.5 s, Han **goes ahead** into
  it. Invulnerable, he **absorbs hazards**: spray jets stop at his body, flies bounce off him; he
  takes the hit with a comedic splat + wobble. Walking right behind him, you're shielded.
- **Han in sewage**: liquid still gets him — he **splats and sinks** like Nat, leaving a **splat
  raft** at that spot (same as Nat's liquid-death raft: floats ~12 s, then sinks), and
  **parachutes back in** a few seconds later. It is **not** Nat's death: no effect on the band's
  death counters, the death count or the adaptive engine. Combined with "lemme check that"
  (stand still facing a sewage pool → he wades in), the player can use him to **make a raft**
  ("I'm fine! I'm a professional!"). Bottomless drains (under the long quartal gaps) leave no
  raft, so this can't bridge those.
- **Han's raft is bigger and longer-lived** than Nat's (≈3 tiles, ≈30 s vs Nat's 1 tile, 12 s).
  **Buddy raft puzzles**: a long sewage pool with **spikes on the ceiling** (no jumping across)
  and too wide to stain-bridge alone — your own rafts sink before you can walk back from the
  respawn for the next one; Han's raft is the answer. The validator needs a time model for
  these (respawn → walk-back time vs raft lifetime).
- **Raft lifetimes slide with the adaptive assist dial** (up to ~2×) but **never below a floor**
  (Nat 12 s, Han 30 s). **Solvability is always computed with the floors**, so assists only
  loosen things that are already provably solvable.
- **Overuse**: stomping him a lot in a row → he complains ("My back! I'm union, Nat!") and, past a
  limit, needs a short breather (cooldown). The limit/cooldown **slides with the adaptive assist
  dial**. **Carve-out: no overuse limit inside chain-jump chasms** (zones you can only cross by
  chaining).

## Readable behaviour (so players can learn the AI)
- Distinct poses: **braced** (plunger up, feet planted), **intercepting** (arms out like a
  goalkeeper), going-ahead (determined march), parachuting, winded.
- Simple, predictable rules above; Han **teaches every one of his mechanics** himself via hint
  spots at first use ("Need a lift? Land on my head and JUMP! Oof, my back.").

## Adaptive coupling
Han's helpfulness follows the adaptive assist dial: struggling → he intercepts more eagerly,
goes ahead sooner (~0.7 s still instead of 1.5 s), drops hints after repeated failures, looser
overuse limit; doing great → lazier, later, quieter. Never labelled.

## Per-mode Han physics
| Mode | Han |
|---|---|
| Giant Steps | floats even more, paddles his arms |
| Quartal (fired up) | can't keep up: wheezes, falls behind, parachutes in |
| Waltz | moves only on the beat (a little step per beat) |
| Nervous | clings close behind Nat, trembles |
| Laughing band | rolls along laughing, bouncier |

## Level design (gates the validator must prove are exclusive)
- **Buddy ledges** (need the plunger boost): marked with **red plunger-handle notches and Han's
  yellow plumber's tape**; shaped beyond Giant Steps' reach (taller/overhanging).
- **Giant walls**: **gold music-staff trim + a note emblem**. Han won't go near them.
- **Shield rows** (need Han to go ahead): a **"PLUMBERS ONLY"** sign at the start.
- **Chain-jump chasms**: crossable only by chaining boosts and toots (no overuse limit inside).
- Free play gets all of these as rooms; the adaptive engine tracks a **Buddy** skill (and chain
  chasms) like any other.
