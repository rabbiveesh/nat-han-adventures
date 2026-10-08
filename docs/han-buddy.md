# Han as an AI buddy — design spec

Status: built (see "As built" at the end for the decisions and numbers).

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
- **He's no use at the band's gates** (giant walls and the rest): he follows Nat right up to
  them, but there his plunger boost is a feeble hop and he grumbles ("Nope. Need more music in
  my soul for that one."), so they always need their mode — no buddy shortcut.
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
- **Giant walls**: **gold music-staff trim + a note emblem**. Han's boost is feeble near them.
- **Shield rows** (need Han to go ahead): a **"PLUMBERS ONLY"** sign at the start.
- **Chain-jump chasms**: crossable only by chaining boosts and toots (no overuse limit inside).
- Free play gets all of these as rooms (`src/freeplay/templates.rs`: buddy ledge, buddy raft
  pool, shield row, chain chasm), served once the story level that teaches each is unlocked;
  the adaptive engine tracks them as one **Buddy** skill. Each keeps its gate a band zone's
  width from the room's ends, so a neighbouring room's band gate never weakens Han there.
  `tests/han_campaign.rs` plays them with the real Han too.

## As built
Code: `src/game/han/` (brain, hazards), `src/level/buddy.rs` (Han's per-mode physics, his
controller and the goalkeeper reflex, pure: the game and the validator run the same code),
`src/level/nav.rs` (the nav graph), `src/level/validate.rs` (Han's gates), `src/art/buddy.rs`.
Tests: `tests/han.rs` (mechanics), `tests/han_campaign.rs` (every Han gate of the campaign
played with the real Han), `tests/han_nav.rs` (nav timings), `tests/levels.rs` (validator).

- **Body**: `game::step_body` is the one body step (gravity, carriers, tile collision) for Nat
  and Han. Han: run 165 px/s (Nat 150), 3 toots, invulnerable to spikes/flies/sprays; his
  jumps write no `Jumped` (the band never counts him). Han's head is a one-way carrier for Nat.
- **Plunger boost**: 560 px/s, 7 tiles above Han's head (~7.9 above his floor); the refreshed
  toot adds ~2.4: ~10.3 tiles ideal, >= 9.3 for a human. Buddy ledges are 9 tiles: out of
  Giant Steps' reach (~8.7), comfortably in the boost's. Not a `Jumped`: a `HanBoosted`.
- **Navigation**: A* (seconds of travel) on a lazily simulated graph of Han's moves (66 jump /
  walk-off moves from a standing start, up to 3 toots) on the validator's map with his physics,
  per mode (normal; Giant Steps built on demand), re-planned 4x/s, at most 24 new cells
  simulated per step. ~100-160 us per cell natively; a whole level 12-34 ms eagerly (est.
  20-70 ms in wasm), so it's lazy: a cold 6-tile route costs ~0.5-1 ms. Moving platforms: he
  waits until the platform will be under the landing cell (real air time) and steers onto it.
  Lost (no route / stuck 3 s) and out of sight: parachute from ~8.5 tiles above, behind Nat.
- **Giant walls etc.: the band zone**: levels mark every gate (`gate: <topic> c0,r0 c1,r1`,
  validated against the crossings). Within 14 columns / 10 rows of a band or death gate mark
  (giant, gap, waltz, grip, stain) Han follows as usual, but jumping off his head is the
  **weak boost**: 300 px/s (`WEAK_BOOST_SPEED`, Nat's feet ~2.9 tiles over Han's floor, about a
  normal jump; it refreshes the toot like any landing: ≤ 85 px with the toot, 11 px under a
  giant wall), even when he's winded, and from the coyote time after stepping off him too (no
  laughing-band bounce off him there either). He grumbles (`grumble_line`: the gate's own lines
  and general ones in turn, at most every 2 s) and his back doesn't count it. In the zone his
  head only holds Nat while he stands (no mid-air catches, no intercepts, no going ahead), and
  he keeps out of a waltz row's mark and a chute's grease (`Level::han_keeps_out`). Everywhere,
  his head holds Nat in mid-air only 2 tiles above the floor he left (5 over a chain chasm), so
  chains of full boosts can't climb out of reach.
- **The proof** (`validate::boost_leak`, per band/death gate crossing, every mode, ideal input):
  full boosts from every cell outside the zones Han may be in (standing, or in mid-air up to
  that catch height and 16 columns from a floor: Nat's before the gate, a chain's, or an
  unreached ledge in a zone that Nat's own jumps get near), weak boosts (and steps off his
  head, then a toot) from every zone cell Nat stands in; whatever unreached cells those land on
  (or pass in a zone) are added, with Nat's own jumps from there, until nothing new: a chain of
  boosts and toots is a chain of such launches. Nothing may land past the gate.
- **Intercept**: a goalkeeper dive (1.4x speed and accel), stays under Nat (cuts his jump
  while Nat rises close above), toots into a descending Nat when lined up at the moment of
  contact. Brace range 1.5-3.5 tiles, intercept range 3-7 tiles (eagerness).
- **Lemme check that**: Nat still and facing sewage, a can or a swarm within 4 tiles for
  2.0 / 1.5 / 0.7 s (lazy / neutral / eager), outside the band zones. It's an escort: Han never leads by more than
  1.5 tiles, he's solid from the side so Nat can't overtake him into the jets, every jet he's
  plugged stays plugged while he escorts (and 0.5 s after); flies circle away from him.
- **Sewage**: sinks 1 s, gone 3 s, parachutes back ("I'm fine! I'm a professional!"); his raft
  is 3 tiles from the splat on, 30 s x `raft_life_mult`. Not Nat's death.
- **Overuse**: 3 / 5 / 7 boosts in a row (lazy / neutral / eager; reset after 1.2 s on real
  ground or 4 s without a boost) → "My back! I'm union, Nat!", a breather of 3 to 1.5 s with
  no boosts (his head still holds you). No limit inside `chain` marks.
- **Modes**: Giant Steps gravity x0.8 more than Nat's and paddling; fired up he stays at his
  speed (falls behind, "Wheeze..."), the waltz: steps only in the first 40% of each beat;
  nervous: 0.8-tile follow gap, trembling; laughing band: bounces (x1.3), rolls.
- **Gates**: buddy ledges (a boost reaches it, no mode does); shield rows (24+ adjacent cans
  on the floor or under a grating, with a low ceiling: no mode dashes them, the waltz
  included; Han plugs both the jets and the cans); chain chasms
  (14+ bottomless columns; crossed by a co-simulated chain with the real reflex, for every
  moment the band might switch to Giant Steps after the chain's toots; one boost must not do
  it); buddy raft pools (10+ sewage tiles under ceiling spikes hanging from solid; Han's rafts
  tile it; Nat's own: fewest rafts x (respawn + walk back at full speed) must exceed 12 s).
  Teaching topics `boost`, `shield`, `chain`, `buddyraft`.
- **Markers**: from the marks: gold staff trim + note emblem (giant), red notches + yellow tape
  (boost), a PLUMBERS ONLY sign (shield).
- Repeating a hint after repeated deaths is the adaptive engine's `han_hint` lever.

