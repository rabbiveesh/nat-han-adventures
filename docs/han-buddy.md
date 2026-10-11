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
- **Plunger boost**: land on his head and press jump: he launches you (~7 tiles up). **A boost
  refreshes Nat's toot.**
- **Fired up: the running throw** (step 4 of the buddy-vs-band plan): while the band's fired
  up Han runs as fast as Nat, and jumping off his head is a throw instead: flat (380 px/s up)
  and far (Nat keeps 120 px/s on top of his run until he lands, like a platform's speed).
  A throw and a toot clear a **fired-up chasm** (15-18 bottomless tiles; the fired-up band
  alone does 11). Over a chasm's mark he also catches Nat in mid-air (and throws again):
  jump → toot → throw → toot…; nowhere else does his head hold Nat in mid-air.
  - Note: a chain's toots count toward the Giant Steps counter (5 toots), and a player with
    it charged may call the band (Down) mid-chain, so gravity can drop mid-combo
    (`buddy::GsSwitch` in the chain simulation).
- **Intercept**: when Nat is airborne and descending near him, Han tries to get underneath
  (runs/jumps/toots) so you can land on him.
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
  dial**. **Carve-out: no overuse limit inside fired-up chasms**.

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
| Quartal (fired up) | as fast as Nat; his boost is the running throw |
| Waltz | moves only on the beat (a little step per beat) |
| Nervous | clings close behind Nat, trembles |
| Laughing band | rolls along laughing, bouncier (not while the band sobers up for a phrase); his head is a spring: the boing boost |

## Level design (gates the validator must prove are exclusive)
- **Mega walls** (need Giant Steps and Han's mega-boost): marked with **red plunger-handle
  notches and Han's yellow plumber's tape**; 11 tiles, beyond Giant Steps' reach and the plain
  boost's. Their zone weakens his boost like a band gate's, except while Giant Steps plays.
  (They replaced buddy ledges, which the plain boost climbed.)
- **Cling shafts** (need the nervous band's grip and Han's wall-cling): a narrow shaft whose
  walls are out of every mode's reach and the plain boost's. Their zone weakens his boost like
  a band gate's, except with the grip. The band gets nervous from deaths anywhere in the level,
  so a shaft after a grease chute needs no splats of its own (level 9); a free-play shaft room
  has a sunken spike strip to splat on.
- **Trapezes** (need the waltz and Han's swing): a bottomless gap, its far side 3 rows up
  behind 3 spikes (so no mode alone, fired up included, makes it), under a ceiling that ends
  over the gap for his plunger. Their zone weakens his boost except in the waltz.
- **Giant walls**: **gold music-staff trim + a note emblem**. Han's boost is feeble near them.
- **Shield rows** (need Han to go ahead): a **"PLUMBERS ONLY"** sign at the start.
- **Fired-up chasms**: crossable only by the fired-up band's throw off Han (no overuse limit
  inside).
- Free play gets all of these as rooms (`src/freeplay/templates.rs`: mega wall, buddy raft
  pool, shield row, fired-up chasm, cling shaft, trapeze), served once the story level that teaches each is unlocked;
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
  toot adds ~2.4: ~10.3 tiles ideal, >= 9.3 for a human. Not a `Jumped`: a `HanBoosted`.
- **The mega-boost** (Giant Steps): the same 560 px/s in Giant Steps' gravity (x0.65): ~10.8
  tiles over Han's head, ~15 over his floor with the toot (the level's ceiling caps it). By a
  mega wall (`gate: mega`) his boost is full strength only while Giant Steps plays
  (`Level::weak_boost`). Mega walls are 11 tiles: out of Giant Steps' reach (~8.7) and the
  plain boost's (~10.3), comfortably in the mega-boost's.
- **The wall-cling** (nervous): in a cling shaft (`gate: shaft`, `Level::in_shaft`) with Nat
  standing in it near him, Han climbs the wall on his side (`CLIMB_SPEED`) to `CLING_ROWS` (3)
  above the floor and clings there (`HanMode::Cling`, up to `CLING_SECS`). Nat hops onto his
  head; jumping off is the full boost plus `KICK_CARRY` (90 px/s) away from the wall
  (`HanHead::kick`). He lets go once kicked, when Nat leaves the shaft or makes it up, dies, or
  the grip goes. The validator models it as `Gate::ClingShaft` (`cling_found`).
- **The trapeze** (the waltz): by a trapeze lip (`gate: trapeze`), Nat standing near it, Han
  reels up his plunger handle (`REEL_SPEED`) into the ceiling's edge `TRAPEZE_OUT` (3) columns
  past the lip and swings (`HanMode::Trapeze`, `buddy::trapeze_at`): a pendulum a bar long,
  through the bottom (Nat's feet `TRAPEZE_LIFT` rows above the lip) swinging forward on ONE at
  `TRAPEZE_SPEED` (240 px/s). Nat rides his cap; a jump off it is the boost plus the swing's
  speed then (`HanHead::kick`). He lets go (and falls, and parachutes back) once kicked, when
  Nat lands past the swing or leaves, dies, or the waltz ends. The validator's
  `Gate::Trapeze` (`trapeze_found`) takes the jump at the bottom with a human share (80%) of
  the speed.
- **The boing boost** (the laughing band, not fired up): dropping onto Han's head where he'd
  boost springs Nat off it at the full boost (`BOOST_SPEED`), no jump needed, with a "BOING!"
  bark and its own sfx (`Sfx::Boing`); once per landing chain (coming back down from it, it's
  the band's plain bounce). No gate needs it (the laughing band stays a surprise), and it
  reaches no further than the boost the validator already proves.
- **The running throw** (fired up): `THROW_SPEED` 380 px/s up, `THROW_CARRY` 120 px/s kept
  (`Body::launch`) towards where Nat faces; `HanBoosted { throw: true }`. Han runs at
  165 x 1.35 px/s then (his own nav graph for it, `HanNav::fired`); his goalkeeper dive brakes
  in time to stop under Nat.
- **Navigation**: A* (seconds of travel) on a lazily simulated graph of Han's moves (66 jump /
  walk-off moves from a standing start, up to 3 toots) on the validator's map with his physics,
  per mode (normal; Giant Steps built on demand), re-planned 4x/s, at most 24 new cells
  simulated per step. ~100-160 us per cell natively; a whole level 12-34 ms eagerly (est.
  20-70 ms in wasm), so it's lazy: a cold 6-tile route costs ~0.5-1 ms. Moving platforms: he
  waits until the platform will be under the landing cell (real air time) and steers onto it.
  Lost (no route / stuck 3 s) and out of sight: parachute from ~8.5 tiles above, behind Nat.
- **Keeping up with a running Nat** (`tests/han.rs` `han_keeps_up_with_a_zooming_nat`: ≤ 8
  tiles behind over a run of hops; it was ~90 and a parachute): he takes off on the run when
  the planned jump (or another one the same way) still lands on his route from there at that
  speed (`Nav::lands_rolling`), else he stops at the take-off point as planned; when the slot
  moves he keeps going on the old route until the new one is in; he keeps to a running Nat's
  slot between re-plans (right in it, as the chain simulation assumes) and runs on when Nat jumps;
  far behind a Nat running away he runs up to 1.3x faster on the walks (not when the band's
  fired up); a slot that keeps moving no longer hides that he's stuck; and left behind out of
  sight for 1.5 s he parachutes in like a lost Han. Running after Nat, he stops 2.5 tiles short
  of a fired-up chasm's drop, so a mid-air intercept still has its runway.
- **Giant walls etc.: the band zone**: levels mark every gate (`gate: <topic> c0,r0 c1,r1`,
  validated against the crossings). Within 14 columns / 10 rows of a band or death gate mark
  (giant, gap, waltz, grip, stain) Han follows as usual, but jumping off his head is the
  **weak boost**: 300 px/s (`WEAK_BOOST_SPEED`, Nat's feet ~2.9 tiles over Han's floor, about a
  normal jump; it refreshes the toot like any landing: ≤ 85 px with the toot, 11 px under a
  giant wall), even when he's winded, and from the coyote time after stepping off him too (no
  laughing-band bounce off him there either). He grumbles (`grumble_line`: the gate's own lines
  and general ones in turn, at most every 2 s) and his back doesn't count it. In the zone his
  head only holds Nat while he stands (no mid-air catches, no intercepts, no going ahead), and
  he keeps off a chute's grease (`Level::han_keeps_out`). Everywhere, his head holds Nat in
  mid-air only while the band's fired up, over a fired-up chasm's mark,
  5 tiles above the floor he left at most (`head_holds`), so chains of throws can't climb
  out of reach.
- **The proof** (`validate::boost_leak`, per band/death gate crossing, every mode, ideal input;
  fired up the full boosts are throws): full boosts from every cell outside the zones Han may
  be in (standing, or, fired up, in mid-air over a chasm up to that catch height and 16
  columns from a floor: Nat's before the gate, a chain's, or an
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
- **Modes**: Giant Steps gravity x0.8 more than Nat's and paddling; fired up he speeds up as
  Nat does (x1.35) and throws, the waltz: steps only in the first 40% of each beat;
  nervous: 0.8-tile follow gap, trembling; laughing band: bounces (x1.3), rolls.
- **Gates**: mega walls (the mega-boost reaches it from a mega wall's zone with room to toot
  up Giant Steps; no mode alone, no plain boost from outside a zone and no weak one inside
  it; unmarked, a plain boost from outside any zone that climbs something is reported as one,
  to be marked; tier 2 checks Giant Steps can be summoned there from every music state, with a
  hop onto Han); shield rows (24+ adjacent cans
  on the floor or under a grating, with a low ceiling: no mode dashes them, the waltz
  included; Han plugs both the jets and the cans); fired-up chasms
  (14+ bottomless columns with a nugget line before them, as a long gap has; crossed by a
  human's throw off Han standing at the edge or up to 3 tiles back; no plain boost, and no
  mode alone, may do it; tier 2 checks the band can be fired up there from every music
  state, with a wait for Han and a hop onto his head); buddy raft pools (10+ sewage tiles under ceiling spikes hanging from solid; Han's rafts
  tile it; Nat's own: fewest rafts x (respawn + walk back at full speed) must exceed 12 s).
  Teaching topics `boost`, `shield`, `chain`, `buddyraft`.
- **Markers**: from the marks: gold staff trim + note emblem (giant), red notches + yellow tape
  (boost), a PLUMBERS ONLY sign (shield).
- Repeating a hint after repeated deaths is the adaptive engine's `han_hint` lever.

