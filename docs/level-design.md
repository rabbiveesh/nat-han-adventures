# NAT ALONE: how a level is built

The pilot is `levels/solo/breezy.txt` ("Breezy Does It", v3). The other nine follow the same rules.
NAT ALONE is the third world, unlocked after the story and Side B, with no Han and no assists:
it is about **challenge from mechanics in combination, with choices**, not a gadget showcase.

## Checklist status (all ten levels)
Every level has the kishotenketsu idea, a band fork whose routes each need their mode, a
second fork of another kind, a secret (Han behind a grate), `shout:` over its closing stretch,
three or four checkpoints and tours that clear each route without a death (`tests/showcase.rs`).
Flow is the worst held-key stretch of each route's tour; nuggets are per route (40 a level,
every one on some tour's line from Breezy's v3 on, Disco 39).

| Level | Band fork | Second fork | Flow (worst held) | Nuggets per route | Runs |
|---|---|---|---|---|---|
| 1. Breezy Does It | GS rooftop, fired canal, no call tunnel | nerve (spiky low line or the climb) | 3.8 s (no call), 3.75 s (GS fan hovers) | 40 in all | GS 56, fired 38, no call 48 s |
| 2. Bouncy Castle | waltz keep, fired moat, no call dungeon | backwards (into the cushion, onto the roof) | 2.3 s; no call 3.3 s (the chimney ride) | 40 in all | waltz 45, fired 39, no call 49 s |
| 3. Up the Spout! | GS ledge, fired vault, no call stair | greed (the brick vault) | 2.5 s (elevator walk-in) | 40 in all | GS 41, fired 44, no call 52 s |
| 4. Brush Hour | GS gantry, fired express, no call underpass | braid (fast lane over slow lane) | GS 2.9, no call 2.9, fired < 2 s | GS 18, fired 19, no call 16 | GS 57, fired 36, no call 54 s |
| 5. Tilt! | waltz beat stairs, fired outlane, no call outlane | rhythm (the doors or the gallery) | 2.95 s (plunger lane), fired 3.6 s | waltz 22, fired 27, no call 25 | waltz 51.5, fired 42.6, no call 51.7 s |
| 6. Gone With the Wind | GS high road, fired low road, no call sewer | wind (against it or round with it) | GS 2.03, fired 2.19, no call 2.28 s | GS 25, fired 24, no call 20 | GS 51.7, fired 36.3, no call 43.9 s |
| 7. Crusher Disco | GS riser, fired dance floor, no call cellar | ride (the lift crusher or the dodge row) | 2.17 s | GS 25, fired 26, no call 24 | GS 57.3, fired 41.6, no call 49.6 s |
| 8. Spin Cycle | waltz dryer, fired express, no call sump | lap (drop out now or round again) | waltz 2.0, fired 2.0, no call 2.27 s | waltz 23, fired 24, no call 21 | waltz 56.2, fired 44.5, no call 47.9 s |
| 9. Ping Pong Palace | GS gallery, fired islands, no call cellar | bank shot (or the tunnel) | GS 2.4, fired 2.3, no call 2.39 s | GS 27, fired 28, no call 24 | GS 50.2, fired 35.5, no call 43.3 s |
| 10. The Grand Flush | four ways: GS balcony, waltz roof, fired islands, no call sump | nerve (the high slab or the low stones) | GS 2.5, waltz 2.1, fired 2.38, no call 2.1 s | GS 22, waltz 21, fired 24, no call 26 | GS 72.1, waltz 64.2, fired 56.0, no call 65.4 s |

Breezy's v3 status against each point, the pilot all the others follow:

| Point | Status |
|---|---|
| 1. One idea, kishotenketsu | done: wind carries you, the floor won't wait |
| 2. Band fork, three routes, all visible | done: rooftop (Giant Steps), canal (fired up), tunnel (no call) |
| 2. Mode's 20 s fully spent | done: both modes last to the plunger (GS refilled by toots, fired up by nuggets) |
| 2. A second fork of another kind | done: the nerve fork (spiky low line vs the safe climb) |
| 3. Organic terrain, stacked routes, landmarks | done |
| 4. Flow: no held-key stretch over 2 s | mostly: worst gaps are 3.8 s (no-call, the bank + nugget lane walk) and 3.75 s (GS hovering in fans) |
| 5. Nuggets on the lines, best ones risky | done: 40 nuggets; under the rooftop spikes, the nerve line's holes |
| 6. Size 45 to 60 s | GS 56 s, no call 48 s, fired up 38 s (the fast route, on purpose); 400 columns |
| 7. Checkpoints 12 to 20 s apart | done: start, 10 s, 18 s, 39 s (GS: the mode stretch runs 21 s), goal 56 s |
| 7. Readable hard bits | fans brighter, with a dark shaft and thicker wisps |
| 8. Two characters, each mode changes them | done |
| Song form | done: the GS call lands on bar 8 (first turn), the merge checkpoint near bar 16, the shout chorus from the closing stretch (`shout:`) |
| A secret | done: Han behind a grate in a nook under the canal's last island |

## 1. One idea, developed (kishotenketsu)
A level is about one thing, told with two characters (section 8). Breezy: **the wind carries
you; the floor won't wait** (fans and crumbling floors).

**Teaching is for the story only** (Veesh, 2026-10-21). The learning progression (introduce a
mechanic where failing is safe, then build on it level by level) belongs to the story levels.
The post-game worlds (Side B, NAT ALONE, World 4) and free play assume the player knows the
mechanics: a level can open on an interesting idea with stakes, and the "Ki" beat below is a
quick statement of the level's idea, not a tutorial.
- **Ki (introduce), where failing is safe.** A fan dip too wide to jump floats Nat up the step.
  A crumbling bridge over a dry ditch has a hole to hop; dawdle and it drops you onto the
  ditch's fan, which floats you back out.
- **Shō (develop), with stakes, one variable at a time.**
  - Over sewage: hop onto a fan's updraft, hover, toot off it onto a crumbling stone, again.
  - Against you: a crumbling bridge in a headwind (walk and it drops you; hop off before the
    wall).
  - Too high: jump up into a vent's gust to cross the sewage.
  - Pressed down: ceiling vents blow down on a crumbling bridge; a jump under them goes
    nowhere, so hop each hole from the clear spot before it.
- **Ten (twist): the band, as a fork** (section 2), then the nerve fork.
- **Ketsu (conclude): everything together, as the shout chorus.** Fan, crumbling stone, gust,
  then a climb out: stone, updraft, stone, updraft, stone, plunger. Every mode can play it.

## 2. The band is the twist, and the call is a choice
- **A fork per level.** Before it, the route offers two ways to charge, and the counter filled
  *last* is the one Down plays. Use that rule on purpose:
  - Breezy's high lane is crumbling stones a toot apart (charges Giant Steps); the low lane under
    it is a quick nugget line (charges fired up). Falling off the stones lands you in the
    nuggets, so the temptation is real.
- **Each mode gets its own route, both visible from the fork.**
  - Giant Steps: the fan pit at the fork blows twice as far and lifts Nat onto the rooftop:
    the slow, safe-ish route with the best nuggets and toot steps.
  - Fired up: the canal, the fast but risky route: 11-tile gaps, a crumbling island, a
    headwind across one gap (toot early, above the wind's rows), a ceiling vent pressing on
    the last island (jump from its clear end).
  - No call: drop into the tunnel under the canal: spikes, crumbling pits in headwinds,
    hurdles, a low crawl, an updraft and a crumbling ledge to climb out. The secret is here.
- **The normal-wind way must be a different route (slower or riskier), not the same one.**
  A miscall (or a missed canal jump) drops you into the tunnel: it costs time, never a
  soft-lock, and never a wait.
- **The calls land on the next bar line.** Leave room (a plateau to hop on, a fan to hover in)
  between the natural call spot and the first place the mode is needed.
- **Keep the mode's 20 s busy, and spend it all.** Toots refill Giant Steps and nuggets refill
  fired up, so put readable toot spots (a pit with a nugget over the toot point) and nugget
  arcs on the mode's route. Breezy's tours stay in their mode to the plunger.
- **Exclusive routes, validated.** A mode route's cells must be unreachable in other modes:
  - Giant Steps (fan reach 12): the rooftop is 11 rows above the fork, and it is a solid slab
    over the canal so a Giant Steps player can't drop onto the fired-up islands.
  - Fired up (speed): the canal gaps are 11 wide and the tunnel floor is 11 rows below the
    islands (Giant Steps jumps about 9 rows). No raft, stain or fan under a gap.
- **A second fork of another kind: a nerve fork.** After the merge, both visible from the
  checkpoint: a low line of crumbling stones over sewage under a ceiling of spikes (little
  hops only, no stopping: one full jump and you're spiked), or the safe climb (a crumbling
  ledge, a toot up onto the roof, its spikes and a crumbling bridge to hop). Any mode can take
  either; the low line is quicker, the climb has a nugget.

## 3. Organic terrain
- Every stretch has a different shape: dips, steps, ditches, posts, pillars, rooftop, canal,
  tunnel, mesa. No copy-pasted blocks.
- Heights wander, and nothing is walled off into rooms. Routes stack (rooftop, canal, tunnel).
- Rest spots (the checkpoints) sit on flat ground before and after the forks.
- Landmarks (the hanging vent pillars, the rooftop, the canal islands, the spiky roof) say
  where you are.

## 4. Flow
- Nat moves forward almost all the time. Hovering in an updraft is a choice, not a wait.
- No waits for timers, and no "hold Right and watch". **Check: no stretch over 2 s in the tour
  with only a held direction key** (`scripts/held-gaps <tour>`: the timeline's press/tap/release times).
  Breezy v3's longest are under 4 s (walking a nugget lane, hovering in a fan).
- Each beat asks for a jump, a toot or a hover at a readable spot.
- **Density, measured:** `scripts/density [tour-prefix]` plays every NAT ALONE tour headless
  (`density_report` in `tests/showcase.rs`) and lists, per tour, the quiet share (time beyond
  1 s in each stretch between actions, over the run), the longest stretch and how many run
  over 2 s; `DENSITY_GAPS=1` lists every stretch over 1 s. An action is a key pressed or
  released, a nugget, a fan's gust entered, or a kick from a duck, cushion or spring. Aim for
  under 10% quiet and nothing over 2 s. To fill a stretch, a spike tuft or a crate the tour hops
  (not on a mode's runway: it breaks the fired-up readiness; and a stretch walked both ways
  needs a hop each way). The 2026-10-23 pass: `gadgets/nat-alone/density.md` (worst before:
  Breezy 18%, Brush 17%, Ping Pong's cellar 17%; after: Brush's Giant Steps 16%, the fast lane
  under its rollers has no room, then Spin's sump 12%; the rest 10% or less).

## 5. Nuggets
- They trace the lines the good runs take. Check this with each tour's trace.
- The best ones reward a risk: under the rooftop's spiky ceiling, a late toot in the gust; over
  the nerve line's holes.
- 15 to 40 per level, with at most 10% unreachable to the validator. Every mode gate needs 3+
  behind it (or the goal).

## 6. Size
- About 45 to 60 s for a good run. Breezy v3: 400 columns; Giant Steps 56 s, no call 48 s,
  fired up 38 s (the fast route is allowed to be fast; it is the risky one).
- The scripted tours clear it with 0 deaths.

## 7. Challenge
- **`deaths:` is not a difficulty target.** It is the number of splats the design *needs*
  (stain pits, grease chutes), the validator enforces it, and the tours must not exceed it. So
  it stays 0 for Breezy. Difficulty is in execution: a first run should die a few times.
- **Checkpoint spacing.** About 12 to 20 s apart, on flat ground. Breezy: the start, after the
  stakes (10 s), before the fork's approach (18 s), at the merge before the nerve fork (39 s
  on Giant Steps, 28 s fired up, 35 s no call). Respawn segments stay under 150 path cost.
- **An optional hard line.** The best nuggets, off the safe line, readable as risky.
- **Two forks** (section 2).
- **Readable hard bits.** Check the stills: you should see the spikes, the gap, the fan's
  reach, the crumble cracking.

## 8. Two characters
Name the lead and the counter-gadget, and say how each mode changes their interaction.
- **Breezy: fans (lead) and crumbling floors (counter).** Fans hold Nat up, crumbles won't let
  him stand still: hover, then commit. Headwinds slow you on a crumbling floor until it drops.
  - Giant Steps: fans reach twice as far (the lift to the rooftop, high hovers), but walking
    is slow, so crumbles need hops.
  - Fired up: gusts are 1.5x and Nat runs 1.35x: long gaps, overshoots; let go of Right to
    land on a crumble.
  - No call: the everyday fans, headwinds you must hop through.

## 9. The band's form
- The song's sections are 8 bars (about 14.5 s). Where it costs little space, put the checkpoints and the calls near the
  turns (bar 8 ≈ 22 s, bar 16 ≈ 36 s, bar 24 ≈ 51 s): Breezy's natural call spot lands Giant
  Steps on bar 8.
- **`shout: <col>`**: from that column on the band plays shout choruses (the closing stretch).
- A stretch where toots on readable spots keep Giant Steps alive: the rooftop's toot pits.
- **Balance lining up to the music against density** (Veesh, 2026-10-21). Matching the music
  leads to too much rest: stretching or padding a level so a call, a checkpoint or a phrase
  lands on an 8-bar turn leaves blank runs. Aim for something to do about every second and a
  call chance about every 3 s. Line up to the band where it costs little space (the shout
  chorus over the closing stretch, a crusher phrase to echo); elsewhere fill the stretch and
  let the turn fall where it falls.

## 10. Secrets
World 3 has Nat lost alone; the secret is a glimpse of Han on the other side of a wall (World 4:
they're in sight but apart). `secret@c,r: line` in the level header puts Han behind a grate
(the cell and the one above must be solid); the first time in a visit Nat comes within 1.5
tiles, Han barks the line, a little fanfare plays and the save remembers it
(`Progress::solo_secrets`, `solo-secrets` in the save text). The payoff (Veesh, 2026-10-19): finding one
sets the band loose at once (full freedom, the instant feedback, until the adaptive engine's next
decision), and all ten open World 4, the double game where Nat and Han are in sight but apart
(`SECRETS_FOR_WORLD_4`, `Progress::world_4_unlocked`; World 4 itself isn't on main yet).
- **Hidden but fair.** Breezy's: in the tunnel, a lone nugget over a fan in the floor, the fan
  blowing up into what looks like the island's solid underside. Stop in it and it lifts Nat up a
  slot into a nook; a hole drops him back out. The validator checks Nat can get close.

## Workflow that worked
- **Generate the level from a small Python script.** Use `ground`/`fill`/`sewage`/`nug`/
  `crumble`/`fanpost` helpers, one commented block per beat.
- **Check it with the validator.** `SHOWCASE_CHECK=<file>` prints the errors, the gates
  (`GiantWall`, `LongGap`, ...), the respawn segments and the lessons.
  - Every mode gate must lead to the goal or 3+ nuggets ("leads nowhere much").
  - A route back up to a mode's ground can itself be a gate (a giant wall backwards): give
    it an everyday way (a crumbling ledge).
  - A run of 8+ floor spikes is a stain pit; sunk into a slab its edges aren't "reached", so
    split it.
  - `deaths:` must equal what the design needs.
  - The first contact with each mechanic needs a `hint@` within 80 path cost before it (a
    fan you can jump past doesn't count as contact).
- **Write each tour as latched keys plus timed taps,** section by section, re-tracing after
  each change (`SHOWCASE_TRACE=1 SHOWCASE_LEVEL=... SHOWCASE_TOUR=...`; the trace prints
  toots, nuggets, calls, secrets, the harmony and the bar lines).
  - Keep taps short. A held Space swallows the next tap, so a toot never fires.
  - A hanging vent right above the walking row bonks the jump: leave a free row, and a floor
    tile past the pillar to jump from.
  - Updraft posts must be 3 wide to hover in; let go of Right over them. To get off a fan onto
    a higher stone, jump from the stone before it and toot over the fan.
  - A crumble only cracks under the foot samples; standing on its last half pixel doesn't
    crack it. Crumbles are one-way: a jump that arrives below the top passes through.
  - One-wide holes eat a walking Nat: close them or make them 2+ wide and meant.
  - Fired up crosses a 3-wide ledge in a quarter second: let go of Right to land on it.
  - Under a low spiky ceiling only a ~100 ms tap is safe; even stones 4 apart give a steady
    hop rhythm.
- **One tour per route** (Breezy: `breezy.timeline` Giant Steps, `breezy-alt.timeline` fired
  up, `breezy-tunnel.timeline` no call plus the secret), each a test in `tests/showcase.rs`.
- **Record and judge stills.** Record the clip, pull stills with ffmpeg and judge them against
  this page.

## Lessons from the pilots
- v1 was "a really good demo": one line, the mode stretch passable in normal wind, 18 s of held
  Right after the call. A mode stretch must be a different route that needs the mode.
- v2's fired-up canal was the easy way. A fast route should also be the risky one.
- Giant Steps runs at 0.65 speed: its routes are slow, so give them the best nuggets and keep
  the no-call route riskier rather than counting on it being slower.
- Inserting a beat early shifts every later timing (and which bar a call lands on): add beats
  at the end, or re-trace everything after the insertion.

## Per level

| Level | Characters | Band fork (call / route) | Second fork | Secret | Runs |
|---|---|---|---|---|---|
| 1. Breezy Does It (v3) | fans + crumbling floors | Giant Steps: rooftop; fired up: canal; no call: tunnel | nerve fork at the merge | nook under the canal's last island | GS 56 s, fired 38 s, no call 48 s |
| 2. Bouncy Castle (v1 under these rules) | wall cushions + belts | waltz: the keep (trampolines spring on ONE, battlements hopped in threes); fired up: the moat (belt islands 12-13 apart, faster belts); no call: the dungeon | backwards fork at the merge: jump back into the step's cushion and toot onto the roof, or the stepping stones under it (belts drag back, cushion ceiling bonks you down) | the dungeon's low passage, back under the courtyard where you land | waltz 45 s, fired 39 s, no call 49 s |
| 3. Up the Spout! (v1 under these rules) | rubber ducks + bricks, a tower | out of the 4-wide chimney (toot at every bat), the landing's "toot or grab": Giant Steps: jump to the ledge and up the 6-wide shaft only floaty ricochets climb; fired up: the long gap and the elevator; no call: drop into the basement, the brick stair storey by storey | greed fork at the merge: the safe elevator by the checkpoint, or hop the kicker, get thrown over the spikes and the nugget arc into the brick vault, smash up its four lids | the basement's far end, back under the landing (a lone nugget at its mouth) | GS 41 s, fired 44 s, no call 52 s |
| 4. Brush Hour (v1 under these rules) | belts + wall brushes, a car wash at rush hour | the plateau's "toot or grab": Giant Steps: the gantry on the deck (toot pits whose belts carry you into a brushed wall, a canopy of brush rollers over the best nuggets); fired up: the express (a runway belt under the plateau, islands 12 apart); no call: off the runway's end into the underpass, against the traffic, crates on the belt | braid fork after the merge: the fast lane (a belt with you, rollers hanging over it, holes) over the slow lane (a belt against you, brushed hurdles, a ceiling too low for more than a hop); swap any time (holes drop you, the on-ramp's block lets you toot back up) | the low passage under the runway, back against the belt from where you land (a lone nugget in it) | GS 57 s, fired 36 s, no call 54 s |
| 5. Tilt! (v1 under these rules) | rubber ducks + beat blocks, a pinball machine | the landing's "hop or grab": three even hops up the beat stairs charge the waltz, the nugget line under them fired up; waltz: the flipper (a trampoline that springs on ONE) up to the top lane (beat stones over its hole, ducks to hop); fired up: the launch lane's belt over the outlane, island to island; no call: off the launch lane into the outlane (ducks, a drain, spikes), up the drain's duck shaft | rhythm fork after the merge: the doors (a blue door then a red one, about a bar's walk apart: pass each in its bar) or the gallery over them (jump up into the hanging lane, toot over its spikes, hop the second pair) | behind a red beat door in the outlane's back wall, a short lane with a lone nugget | waltz 51.5 s, fired 42.6 s, no call 51.7 s |
| 6. Gone With the Wind (v1 under these rules) | wall fans + crumbling floors, sideways wind in lanes | the floor under the ledges' "toot or grab": five toots up the crumbling ledges charge Giant Steps, the nugget line on the floor under them fired up; Giant Steps: the high road on the slab (socks blow twice as far over its gutters, a headwind block hopped through, toots over three spiky slots); fired up: the leap off the edge, then the low road's stair down under the slab (each riser a wall fan: a tailwind over the crumbling floor below it), up through a lid; no call: off the edge into the sewer (pits with crumbling stones, a headwind off a block), up crumbling steps through a lid | wind fork at the merge: against the wind (the crumbling causeway, a headwind off the wall at its end: hop it, toot over it) or the long way round with it (back up a stair of ledges to the top lane, two socks' lanes over the causeway) | the low passage back under the fork's edge, where the sewer route lands (a lone nugget at its mouth) | GS 51.7 s, fired 36.3 s, no call 43.9 s |
| 7. Crusher Disco (v1 under these rules) | beat crushers + beat blocks, the song level (the crushers play the beat, the floor the bar) | the riser's steps' "toot or grab": five toots up the one-way steps charge Giant Steps, the nugget line on the floor under them fired up; Giant Steps: up the drum riser (hurdles that kick on the ONE, a blue stone in a sewage slot, toots over two spiky slots); fired up: the leap onto the dance floor's islands, run under their beat crushers in stride; no call: off the edge into the cellar, stones two a bar over sewage, stairs up | ride fork at the merge: hop onto the lift crusher while it's down and ride it up to the high lane (hop the gaps over the row's crushers), or dodge under the row slamming 1, 2, 3 | the cellar's nook back under the dance floor's edge, where no call lands (a lone nugget at its mouth) | GS 57.3 s, fired 41.6 s, no call 49.6 s |
| 8. Spin Cycle (v1 under these rules) | trampolines + belts, a laundromat (drums that zigzag up) | the table's "hop or grab": three even hops over its baskets charge the waltz, the lint lane's nuggets under it fired up; waltz: caught on the trampoline at the dryer's 14-tall wall till ONE, its roof, pit trampolines on ONE (a toot at each apex for the high nuggets); fired up: the express belt off the lint lane, a toot off its end and off each belt island; no call: off the express into the sump (drains against its belt), steps and a trampoline up through a one-way hatch | lap fork in drum 2: drop out the hole in its floor now (the spin-dry's belt islands under it, a trampoline back up) or go round again (floor against you, spring, shelf back, spring, out along the top) | the lint trap in drum 1's back wall: climb off the shelf trampoline without a toot and you're in it | waltz 56.2 s, fired 44.5 s, no call 47.9 s |
| 9. Ping Pong Palace (v1 under these rules) | rubber ducks + ceiling cushions, a ping pong palace (the ducks bat you up and back, the cushions bat you down) | out of the 4-wide chimney (toot at every bat: Giant Steps), the landing's "toot or grab" (its trench's four nuggets fire the band up); Giant Steps: floaty legs up onto the gallery, low hops under its cushions over spiky slots, jump and toot over open pits, onto a duck, ducks under the cushions; fired up: off the runway's end, islands ten apart, a nugget on each arc; no call: off the runway into the cellar, low hops under its cushion ceiling, two ducks, up the elevator and toot through its hatch | bank shot after the merge: jump and toot into the wall's duck, it bats you back up onto the shelf behind you (toot again at the top of its bat without Giant Steps), the high score table on the wall (nuggets over spiky slots, a duck); or the tunnel under it (a cushion ceiling, low hops over two holes) | the low passage back under the landing, where no call lands (a lone nugget at its mouth) | GS 50.2 s, fired 35.5 s, no call 43.3 s |
| 10. The Grand Flush (the finale) | a medley of the nine: one section per earlier level in order, each a remix (Breezy's fan dip and crumbling bridge, Bouncy's belt back into the bank's cushion, the Spout's duck elevator and brick plug, Brush Hour's islands into brushed posts, Tilt!'s red and blue drain), the fork's routes recalling 6-9, the Champion's Road | the bandstand's "toot, grab or hop": five toots up its crumbling steps (Giant Steps), the lint lane's four quick nuggets under them (fired up), three even hops over the table's baskets (waltz). Giant Steps: floaty legs onto the balcony (Gone With the Wind), the sock's gust over its spike bed, a spiky slot; waltz: caught on the dryer's trampoline till ONE, onto the roof, its pit trampolines (Spin Cycle); fired up: off the express's end, a toot off each belt island over the sump (Crusher Disco's dance floor); no call: into the sump, its ducks and holes, up through the merge's hatch (Ping Pong Palace) | nerve fork after the merge: the high slab (crumbling ledge, toot up, brushes under it, spikes on top) or the low stones over sewage under a brush ceiling (hop, never high) | the tenth: back under the express from the sump, a low passage, Han behind its grate calling "Nat! Over here!" (the others: "Nat?!") | GS 72.1 s, waltz 64.2 s, fired 56.0 s, no call 65.4 s |

### Bouncy Castle notes
- The idea: **the walls throw you, the floor moves you.** Ki: the kickback (run back into the
  castle wall, fly the dry moat) and the loader (a ditch whose belt carries you back into the
  bank's cushion). Sho: read the floor (belt islands that carry or drag), the slingshots (drop
  onto an island, its belt loads you into the ledge's cushion, toot across), the elevator (a
  cushion chimney up to the ramparts). Ten: the fork. Ketsu (`shout:`): a last slingshot over
  the sewage and the tower's chimney to the plunger.
- The fork is the walkway's gap after the rampart hops: hop it (the hops were the last count,
  Down calls the waltz) or fall in onto the nugget line (Down calls fired up). No call: ride
  the moat belt off into the dungeon.
- Checkpoints: start, 11 s (after the belt islands), 21 s (the ramparts' end, just before the
  fork), 33 s (the merge), goal 45 s. The validator's 150 path-cost segments keep them a bit
  under 12 s apart here.
- Flow: the worst held stretches are ~2.3 s (the walkway into the keep's trampoline, the
  battlements' drop) and 3.3 s on the no-call route (the dungeon chimney's ride up).
- Clips: `nat-alone/w3-bouncy-waltz.mp4`, `w3-bouncy-fired.mp4`, `w3-bouncy-dungeon.mp4`.
- Lessons: a cushion's fling refreshes the toot, so jump, toot into a tall face and toot again
  at the fling's apex; flings are fast (26 t/s, 39 fired up) but drop quickly, so toot at the
  apex, not on the way up. A chimney's right wall must not be cushion near the top if a fling
  left from there reaches a mode's island (the validator calls it a giant wall). A chimney
  entrance under a hanging wall needs 3 tiles of floor inside, or the jump window is 0.2 s.
  Under a cushion ceiling with one free row, hops between stones need gaps of 2 (a 3-gap needs
  a toot that bonks and lands short).


### Up the Spout! notes
- The idea: **the ducks bat you up, the bricks are in the way, until you smash them.** Ki:
  the brick row (jump into it, it breaks), the 3-wide elevator (hold a way, the ducks bat you
  up; a toot at the top smashes the plug). Sho: the kicker (back into a duck, then away and
  toot: it throws you ~17 tiles over a spike pit), the second elevator, the 4-wide chimney
  (it only climbs if you toot at every bat). Ten: the fork hall. Ketsu (`shout: row 36`, the
  new tower form of `shout:`): a kick into the last elevator, the spikes, the last chimney.
- The fork: the chimney's toots fill Giant Steps, its two nuggets and the landing's line fill
  fired up; Down calls the one filled last (a toot on the landing to hop the chimney's hole
  makes it Giant Steps). No call: the pit under the ledge drops into the basement.
- Checkpoints: floor 3 (9 s), the landing (22 s), the merge (GS 28 s, fired 29 s, no call
  40 s), goal 41-52 s.
- Flow: the elevator rides are batting, not holding (E1 2.5 s with the walk in, the GS shaft
  and K ~2 s); the chimney is tooting all the way. Floor runs over 2 s got a spike pair or
  rubble to hop.
- Clips: `nat-alone/w3-spout-giant.mp4`, `w3-spout-fired.mp4`, `w3-spout-stair.mp4`.
- Lessons: a brick smashes only the cell under Nat's head, so a 3-wide brick plug leaves a
  1-wide hole: lids work as storeys 4 rows apart (jump, toot, drift onto a neighbouring
  brick), never 5. A shaft's plug row should have plain walls, not ducks, or you bounce in its
  hole forever. Fired up runs as fast as a duck's bat, so holding a way inside a 3-wide shaft
  stalls: let go after a bat and it climbs at ~12 rows/s (and an inner plug stops it dead).
  Normal mode can't climb a 4-wide shaft quickly (5 s for 10 rows): 3 wide unless the toots
  are the point. Giant Steps runs at 6 tiles/s: a 5-wide spike strip is too long for its hop.
  A ledge over a long gap caps the jump under it: give the gap clear headroom.


### Brush Hour notes
- The idea: **the floor carries you, the brushes bite.** A brush kills only on its open face,
  so every post is brushed on the face the belt carries you into. Ki: the wash (a belt under
  brush rollers: jump is fine, a toot into them isn't), a drag belt with a brushed crate, a
  one-sided hurdle. Sho: belt islands that carry you into a brushed post (jump before it),
  short drag islands (keep hopping), the car wash bridge (a fast belt under low rollers, hop
  its holes from just past a roller). Ten: the plateau fork. Ketsu (`shout:`): two more carry
  islands with rollers over them, the drag ramp up to the plunger.
- The fork: the high lane's drag ledges (a toot up to each) fill Giant Steps, the low lane's
  nugget line under them fills fired up; Down calls the one filled last. No call: the
  plateau's hole drops onto the runway, ride it off the end into the underpass.
- Second fork, a braid rather than a choice made once: both lanes run side by side to the
  shout, you can change lanes at every hole or on-ramp. The fast lane is quicker and has the
  nuggets under its rollers (hop between rollers, never under one); the slow lane is safe and
  slow.
- Optional hard line: the express itself (12-wide gaps at fired-up belt speed) and the
  gantry's canopy nuggets (Giant Steps floats into the rollers if you toot too early).
- Checkpoints: start, the bank (12 s), the plateau before the hole (18.5 s), the merge (fired
  26 s, no call ~40 s, GS ~44 s), goal 36-57 s. Segments 114/67/148/146 path cost.
- Flow: brushed crates break the walks (the plateau's bank and hole, the gantry step twice,
  the merge bank); the runway's last two nuggets sit a hop up. Worst held stretches: GS 2.9 s
  (the drop off the gantry onto the bank), no call 2.9 s (the plateau), fired up none over 2 s.
- Nuggets trace the tours (placed at the tour's jump apexes): GS 18, fired up 19, no call 16
  of 40; the canopy's three under the rollers are GS's risky best.
- Clips: `nat-alone/w3-brush-giant.mp4`, `w3-brush-fired.mp4`, `w3-brush-underpass.mp4`.
- Lessons: a held Space released less than ~50 ms before the next tap swallows it (no toot).
  A post brushed on both faces is unfair to jump going with the belt (you land on the far
  brush): one face, the one you meet. A fast belt off a ledge under a roof caps the jump at
  ~3 rows; a gap of 4 off a normal belt island needs a toot. A slab over a secret's passage
  must stop short of the crates you hop to get there.

### Tilt! notes
- The idea: **the ducks bat you around, the beat decides which way is open.** No sewage floor
  at first; the plunger lane (a 3-wide duck shaft) bats Nat up onto the first ledge. Ki: the
  beat stones over a safe dip (red on even bars, blue on odd), the kicker (run back into a
  duck wall, it throws you forward, toot at the top), ducks in the road. Sho: a kicker and a
  beat bridge over the drain, a kick onto a stone in the drain's middle, the second lane up to
  the fork's landing. Ten: the band fork. Ketsu (`shout:`): one stepping stone the beat moves
  (red near on even bars, blue far on odd: a toot from the near one, a hop from the far one),
  a post to hop, run back into it and jump out of the kick over the last drain, toot up the
  last lane to the plunger.
- The fork: the beat stairs (three even hops) fill the waltz, the nugget line under them
  (moved to the landing's end so the call lands while you're still on the launch lane's belt)
  fills fired up. Fired up's islands are 12 apart (a jump and a toot at belt speed). No call:
  off the launch lane's end into the outlane.
- Second fork, rhythm: the doors are blue (open on even bars) then red (open on odd), 12
  columns apart, about a bar's walk: pass the first late in its bar and the second is open.
  At fired-up speed the window is ~0.9 of a bar; arrive in the wrong bar and you wait. The
  gallery over them is the beat-free way: jump straight up into the hanging lane (3-wide, its
  ducks bat you up and out onto the gallery), toot over the first spikes, hop the second.
- Duck shafts and the band: a 3-wide shaft climbs normally and in the waltz; **fired up, Nat
  hovers in it** (enter the hanging lane fired up and you fall back out), so the fired-up run
  takes the doors. Toots work in a shaft (each bat gives the toot back): tooting at every bat
  climbs faster in any mode, and fired up needs it for the last lane.
- Optional hard line: the outlane's islands at fired-up belt speed, the gallery's spikes, the
  near stone's toot.
- Checkpoints: start, the kicker ledge (11 s), the landing (27 s), past the drain's hole where
  all three routes touch it (waltz 43 s, fired 35 s, no call 42 s), goal 42-52 s. Segments
  86/135/144/90. The no-call tour lets go of Right at the top of the drain's shaft so it drops
  onto it.
- Flow: ducks on the top lane and the outlane's road, nuggets a hop up between the doors,
  toots up the duck lanes, two nuggets a hop up over the landing's end and the launch lane's
  belt (the no-call run's beats on its way to the outlane; it also toots up the second lane).
  Worst held stretches: the opening plunger lane (2.95 s, the ducks bat you), the climb to the
  landing for the call routes (waltz 2.95 s, fired 3.6 s: 1.3 s of each is the second lane's
  ducks batting; a toot there would tick the toot counter and change the call), no call's
  walk to the drain (2.04 s). The no-call run's old 5.75 s ride is now hops and a toot.
- Nuggets: waltz 22, fired up 27, no call 25 of 40, and every nugget is on some tour's line
  (the fired-up toot over the top lane's gap waits a beat to grab one). Validator quirk: a
  nugget a hop up on the launch lane's belt at 215-221 made the outlane's island gap "lead
  nowhere much" (the gate's nuggets are counted from its newly reached cells); 213 is fine.
- The secret: a red door in the outlane's back wall (open on odd bars), four columns of lane
  with a lone nugget, Han behind the grate at its end. Go in at the start of an odd bar and
  you're back out before it shuts.
- Clips: `nat-alone/w3-tilt-waltz.mp4`, `w3-tilt-fired.mp4`, `w3-tilt-outlane.mp4`.
- Lessons: a checkpoint triggers only on contact with its tile (Nat's feet at least in its
  row), so one that a shaft's pop-out flies over doesn't count; keep `put` of checkpoints
  after any `air()` carve that covers them. A 1-row-tall kicker is too short to run back into
  and jump out of: make it a 3-tall post you hop first. A jump under a door's wall bonks: hop
  after you're through it.

### Gone With the Wind notes
- The idea: **the wind comes in lanes.** A wall fan blows its own rows only, so the level is
  about the height you travel at: jump into a sock's lane and it carries you, hop over a
  headwind's lane, and never stand still on a floor that crumbles. Wind socks (pillars hanging
  from the ceiling, their bottom rows fans) make lanes at jump height over gaps. No updrafts:
  every fan is sideways (`e`/`w`) or a ceiling downdraft (`!`). Ki: a dry ditch with a sock
  over its edge (fall in and walk out), a crumbling bridge with a low headwind off the mesa.
  Sho: the same over sewage (socks over three islands' edges), a headwind bridge, then the
  pressed-down tunnel (roof fans where you can't jump, crumbling floor, hop the holes under the
  quiet roof). Ten: the band fork. Ketsu (`shout:`): a sock off the wall onto an island, the
  downdraft roof over a crumbling floor (hop the three holes where the roof is quiet), a
  one-row headwind off the last wall over a crumbling floor (jump, toot at the apex).
- The fork: the crumbling ledges over the floor give five toots (Giant Steps), the nugget line
  on the floor under them fires the band up. The line sits a bar's run before the edge so the
  call lands on the runway at full speed (a call lands on the next bar line, and one pressed
  ~0.15 s before it waits for the one after). Giant Steps: the high road on the slab, where its
  0.65 speed makes the 12-tile headwind a hop-and-toot slog and the socks' 12-tile lanes carry
  far; each toot over a spiky slot keeps Coltrane on. Fired up: the 11-wide leap (no sock: a
  sock hanging in a gap's first column bonks every takeoff, and a lane only 6 tiles long
  can't carry a 12 gap), then the low road, a stair down whose risers are wall fans: fired-up
  gusts plus fired-up legs sprint the crumbling floor below each riser (~23 t/s), and it exits
  up through a lid. No call: off the edge into the sewer.
- Second fork, wind (direction): at the merge, against the wind (the crumbling causeway, a
  headwind off the wall at its end: hop through, toot onto it; fired up's way, two nuggets) or
  the long way round with the wind (a stair of crumbling ledges up to the top lane, two socks'
  lanes over the causeway; Giant Steps' and no call's way, a nugget in the first lane).
- Exclusive routes: the low road runs on under the bank and comes up through a one-way lid,
  and a wall stands at the bank's left end, so nothing drops back into it from the merge (the
  bank's end was a drop-in, which let plain reach the leap's landing and erased the gate).
  The high road's end is raised so Giant Steps can't come back from the merge.
- Optional hard line: the toot nuggets over the high road's slots, fired up's leap and the
  stair at ~23 t/s, the causeway's headwind.
- Checkpoints: start, the mesa (6 s), the floor before the fork's ledges (16.5 s, where every
  route is on the ground: the call routes hop and toot over the ledges), past the merge's lid
  (GS 35 s, fired 27.5 s, no call 32.7 s), goal 36-52 s. Segments 59/116/134/119.
- Flow: hop nuggets on the floor before the ledges, on the raised end, on the merge and on the
  shout's island. Worst held stretches: Giant Steps 2.03 s (off the raised end onto the merge),
  fired up 2.19 s (the bank after the lid), no call 2.28 s (the nugget line's run to the edge).
- Nuggets: Giant Steps 25, fired up 24, no call 20 of 40, every nugget on some tour's line.
  Validator quirks: a gate counts its nuggets 0-2 rows above the cells first reached through it,
  in normal physics: the high road's toot nuggets didn't count, so it got two low ones (on the
  slab and the headwind block) to stop "leads nowhere much". A long gap is 11: 12 with a sock
  passed, without one only fired up's toot gets 11.
- The secret: the low passage back under the fork's edge, where the sewer route lands; walk
  back in (a lone nugget at its mouth) and Han's behind the grate at its end.
- Clips: `nat-alone/w3-wind-giant.mp4`, `w3-wind-fired.mp4`, `w3-wind-sewer.mp4`.
- Lessons: a hanging sock blocks rows 0.44 tile under its bottom too: take off at least 1.4
  columns past its column. A headwind on a passage you must walk into is a 2.4 s slog at any
  length (6-tile reach, net 2.5 t/s): don't put the secret at its end. A tall headwind (3 rows)
  in front of a wall leaves no time to toot over it; one row does. Watch `air()`/`fill('.')`
  carves that run after stairs or ledges: the sewer's under-bank carve erased the climb out
  (the validator then reported the goal behind the Giant Steps gate).

### Crusher Disco notes
- The idea: **the song level.** The crushers play the beat, the floor plays the bar (red stones
  on even bars, blue on odd), and the sections follow one chorus of I Got Rhythm. Ki (the
  pedal): red, a solid rest, blue over a dry dip, a bar's walk each (miss the bar and you
  drop in the dip and walk out). A1, call and response: one slow crusher to meet, then the
  call (three crushers slamming 1, 2, 3 and resting: go when the first one's home) and its
  answer (blue then red stones over a dry ditch, on the bar). A2, the answer with stakes: the
  same phrase on every bar's beat 2 over sewage islands, the beat stones over the sewage,
  the arcs between the islands' crushers carry the nuggets. Ten: the band fork. Ketsu
  (`shout:`): red then blue over sewage (cross the bar line on them), an island whose crusher
  slams on the odd bars (the rest bar: wait beside it, pass when it's home), again, home.
- The fork: the riser's three one-way steps over the floor give the toots (Giant Steps), the
  nugget line on the floor under them fires the band up. Giant Steps: up the drum riser's
  7-row face, where the hurdles kick (sideways crushers on the ONE every other bar), a blue
  stone floats in a sewage slot, and toots carry over two spiky slots (each keeps Coltrane
  on); its end is raised so nothing climbs back from the merge. Fired up: the leap off the
  dance floor's edge onto an 18-wide island with two crushers on the beat (one on the ONE,
  one on the and of 3: run under them in stride), a second leap to a short island with a
  crusher every other bar, a third to the merge. No call: off the edge into the cellar,
  stones two a bar over its sewage, two 4-row stairs (toot up each) to the merge.
- Each mode changes the characters: Giant Steps' floaty jumps clear a red stone onto its blue
  in one arc and ride the lift's slow rise; fired up runs under beat crushers in stride and
  outruns the beat stones (it has to wait for a stone's bar); no call walks the stones in time.
- Second fork, ride (use the hazard as a vehicle): at the merge, the lift crusher slams on
  every other ONE; hop onto its top while it's down and ride it up to the high lane (three
  lane nuggets; hop the gaps where the dodge row's crushers' tops are the stepping stones, and
  a mistimed hop rides one down to the floor), or dodge under the row of three crushers slamming
  1, 2, 3 (three floor nuggets). Giant Steps and no call ride, fired up dodges.
- Secret: the cellar's nook back under the dance floor's edge (a lone nugget at its mouth),
  where no call lands: walk back left and Han's behind the grate.
- Checkpoints: start, the hall's end (12 s), the floor before the fork's steps (20 s), the
  merge past the riser's end (GS 39 s, fired 32 s, no call 37 s), goal. Segments 107/70/146/96.
- Flow: hops over the speaker boxes, the merge, the lane's gaps and the dodge row. Worst held
  stretches 2.17 s on every route (the start's run to the dip).
- Nuggets: 39; Giant Steps 25, fired up 26, no call 24, every nugget on some tour's line.
- Lessons: Giant Steps is too slow for stones a bar apart (7.6 tiles a bar): give it a leap
  over the first stone onto the second, and islands to rest beside a crusher; a beat-1
  crusher's 0.5 s home window is too short for a 3-wide face at 6.1 t/s, so the islands'
  crushers are beat 2 (1 s home). A crusher kills a little past its face: wait a full tile
  clear of it. Releasing Right in the air stops Nat dead: release over the landing, not
  before the gap. A crusher's top is ridable all the way up (it carries you from the floor to
  its home), the cleanest "ride" gadget in the set.

### Spin Cycle notes
- The idea: **the floor drags you back, the springs throw you up.** Every drum is a zigzag:
  along its floor against the belt (hop its spikes), off the trampoline at its end onto the
  front wall, back along the shelf (its belt with you now), off the shelf's trampoline onto
  the back wall, out along the roof. Ki: the spring step (a wall too tall to jump, a
  trampoline at its foot) and the belt's long jump (fall in and the pit's trampoline puts you
  back). Sho: the rinse (trampoline posts over the suds, islands between), drum 1. Ten: the
  band fork. Ketsu (`shout:` from drum 2's high exit): a ledge against its belt, a post up onto
  the fast belt, off its end into a spiky trough's trampoline, the last post, the plunger.
- The fork: the folding table's three baskets give the waltz its even hops; the hatch before
  them drops into the lint lane, whose four nuggets fire the band up. Waltz: the table's
  trampoline holds you till ONE and springs you 12.7 rows onto the dryer; its pits' trampolines
  catch you till ONE (a bar apart at a run, so the second one fires as you land) and a toot at
  each apex takes the nuggets up high. Fired up: the express belt (with you, x1.5), a toot off
  its end and off each belt island, 12-13 apart, to the merge. No call: off the express's end
  into the sump, its belt against you over two drains, a stair and a step to a trampoline that
  springs you through the merge's one-way hatch.
- Each mode changes the characters: the waltz makes trampolines wait for ONE (and twice as
  tall); fired up makes the belts carry you over gaps nothing else crosses (and faster against
  you too: release Right over the trough's trampoline); no call is the belts against you.
- Second fork, lap (a loop: out now or round again): drum 2 has a 2-wide hole in its floor.
  Drop in (release Right in the hole, land on the spin-dry's first island, toot across to the
  second, jump to the trampoline back up beside the meeting floor), or hop it and go round for
  the high exit (4 nuggets against 2). The waltz and fired up go round, no call drops out.
- Secret: the lint trap, a nook in drum 1's back wall partway up the climb off the shelf's
  trampoline. Spring without a toot holding Left and you're in it (a lone nugget at its
  mouth); out: walk off onto the trampoline, let go, toot at the top, onto the roof.
- Checkpoints: start, the rinse's landing (11 s), the table's step (20 s), drum 2's door
  (waltz 35 s, fired 28 s, no call 33 s), goal. Segments 119/88/125/145.
- Flow: blocks to hop on each ledge and roof, a second spike on each drum floor, a toot on
  every post and apex. Worst held stretches: waltz 2.0 s, fired 2.0 s, no call 2.27 s (the
  merge to drum 2's hole). The trampoline catches wait for ONE (up to 1.3 s, on the music).
- Nuggets: 40; waltz 23, fired up 24, no call 21, every nugget on some tour's line.
- Lessons: the validator can't chain trampolines or reverse mid-air, so drums are zigzags
  (each spring rides up a wall's face and lands on its top) and posts alternate with islands.
  A basket on the waltz's runway breaks the SpringOnOne gate: keep 6 flat tiles before the
  trampoline. A long gap's nugget line must sit within 30 tiles of each take-off (the second
  island needed its own fourth nugget). A call lands at the bar after next: put the nugget
  line early in the lane so the mode's on before the express's end. Jumping off a left belt
  is short (6.9 t/s): take off late over its spikes.

### Ping Pong Palace notes
- The idea: **you're the ball.** The ducks bat you up and back, the ceiling cushions bat you
  down. Ki: a duck in the road ("Boing!"), the cushion hall ("Low hops!": jump high and the
  ceiling slams you down; its dips are dry), the kicker ("Back into it!": drop onto the low
  ledge, run back into the step's duck, toot at the top of the throw). Sho: the same over
  sewage (duck islands, the net: toot only once you're past the cushion hanging over the drink,
  the low corridor's islands under cushions), the 4-wide chimney. Ten: the band fork. Ketsu
  (`shout:` at the kicker step): off the step onto the ledge, back into the duck, a toot over
  the drain and under the net, a hop for the nugget, the 4-wide duck shaft up to the plunger.
- The fork: the chimney's bats give Giant Steps its five toots; the landing's trench has four
  quick nuggets (fired up). Giant Steps: the gallery's floor is six rows over the runway (only
  floaty legs), low hops under its cushions over 2-wide spiky slots, jump and toot over the
  open sky's 5-wide pits (hop up onto the duck between them), ducks under the cushions again.
  Fired up: off the runway's end, islands ten apart (11 needed a 50 ms window), a toot over
  each gap and a nugget on each arc. No call: off the runway into the cellar: a hole, low hops
  under its cushion ceiling (a 2-wide hole under it), two ducks, the elevator (plain walls at
  its top: toot at the top of the last bat, up through the hatch).
- Each mode changes the characters: Giant Steps' low gravity makes the duck shafts climb
  themselves and the throws carry farther, and Giant Steps softens the ceiling cushions
  (`GIANT_CUSHION_BAT`: they bat you down at a tenth of the speed, and still give the toot
  back), so the gallery's first stretch is a rally: a spike bed under the cushions (a stone in
  it), toot up into the cushion about every 0.36 s (hold each toot till the bonk: a released
  toot is cut in half and falls short) and ride along under it. Fired up outruns the ducks'
  bats (let go of Right in the shafts so both ducks bat). The waltz's ×1.4 fling on ONE isn't
  used here (gap).
- Second fork, the bank shot: the merge ends at a wall with a duck's face up it. Jump, toot
  into the duck, it bats you back up and refills your toot; toot at the top of its bat (Giant
  Steps doesn't need to) up through the one-way shelf behind you, onto the high score table
  (nuggets over 3-wide spiky slots, a duck to hop). Or the tunnel at its foot under a cushion
  ceiling: hop its two holes low. Giant Steps and fired up bank, no call tunnels.
- A spike bed of 8 or more under a ceiling the validator can't rally across is a stain pit
  ("leads nowhere much"): split it with a stone.
- The umpire's chair: a cushion block hanging over the merge stops the bank shot's duck from
  batting you back onto the gallery's end.
- Secret: the low passage back under the landing, where the cellar route lands (a lone nugget
  at its mouth).
- Checkpoints: start, the kicker's ledge (7.5 s), the landing (19.6 s), the merge (GS 37 s,
  fired 29 s, no call 36 s), goal. Segments 73/121/122/77.
- Flow: worst held stretches: GS 2.4 s (the duck shaft to the plunger), fired up 2.3 s, no
  call 2.39 s. No waits.
- Nuggets: 40; GS 27, fired up 28, no call 24, every nugget on some tour's line.
- Lessons: a gate's nuggets count only 0-2 rows over its newly reached stand cells: high
  nuggets over the gallery's pits didn't count, nor ones over spike slots. A 3-wide duck shaft
  holding Right never reaches the left duck (4 wide does, at normal speed). Under a cushion
  ceiling 3 rows up a hop clears 2 wide, not 3. A held jump swallows a toot tapped too soon.

### The Grand Flush notes
- The idea: **the whole world, flushed in one go.** A medley, one section per earlier level
  in level order, each remixed rather than copied: Breezy (a fan dip, a crumbling bridge with
  a hole over a fan ditch, fan posts over sewage with a crumbling stone between), Bouncy Castle
  (drop onto the belt island, it carries you back into the bank's cushion, flung over the
  moat), Up the Spout! (two ducks bat you up a 3-wide elevator, a toot smashes the brick plug;
  the checkpoint on its ledge), Brush Hour (belt islands carry you into brushed posts: jump
  onto each post), Tilt! (red and blue beat blocks across the drain, rest stones between). Ten:
  the band, four ways. Ketsu (`shout:` from 412, the Champion's Road): a fan post over sewage,
  a crumbling stone, the bank's cushion and its belt island, the landing, the duck shaft, a belt
  into a brushed post, two beat crushers half a bar apart, the plunger.
- The fork: the bandstand. Five crumbling ledges a toot apart (Giant Steps), the lint lane
  under them with four quick nuggets (fired up), the folding table above it with three baskets
  to hop evenly (waltz); the lane's checkpoint, then a trampoline up to the table (a toot at
  its top). Giant Steps: walk the express off its plain end, jump and toot at the apex onto
  the balcony 7.6 rows up (only floaty legs), under the sock and off it at once (its gust,
  twice as far, carries you over the 6-wide spike bed), a jump over the spiky slot. Waltz:
  wait on the table for the band, caught on the dryer's trampoline till ONE, a toot at the very
  top onto its roof 14 rows up (a 0.1 s window), hop into each pit and let go of Right: its
  trampoline springs you on ONE out over the spikes. Fired up: the express, a toot off its end
  and off each belt island (11 apart) over the sump. No call: off the express into the sump,
  back under it to the secret, hop its ducks and holes, steps and a trampoline up through the
  merge's one-way hatch.
- Each mode changes the characters: Giant Steps' low gravity carries the fan post's lift up
  and away (no hover) and the cushion's fling needs a toot to reach the landing; the waltz
  catches you on every trampoline and runs the crushers in three-four; fired up crosses the
  islands nothing else can.
- Second fork, the nerve fork: the high slab (a crumbling ledge, jump and toot up past its
  underside's brushes, a jump over its two spikes for the nugget above them) or the low stones
  over sewage under a brush ceiling (short hops, five apart: the waltz hops them in threes).
  GS and fired up take the slab, the waltz and no call the stones.
- Optional hard line: the slab's spike nugget, the nugget hop between the last two crushers
  (wait there for the second one), the sump's holes.
- Secret, the tenth (finding it with the other nine opens World 4): a low passage back under
  the express from the sump, a lone nugget at its mouth, Han behind the grate calling "Nat!
  Over here!" instead of "Nat?!".
- Checkpoints, fewer for the finale: start, the Spout's ledge (16.4 s), the lint lane
  (32-33 s), the merge (fired 41.6 s, waltz 49.6 s, no call 50.1 s, GS 51 s), goal. GS's last
  stretch is the longest, 21 s. Segments 147/139/134/150.
- Flow: hops on the express, the balcony, the merge, the slab and between the crushers.
  Worst held stretches: GS 2.5 s (the duck shaft to the belt jump), waltz 2.1 s, fired 2.38 s,
  no call 2.1 s. The waltz waits on the table for its call (a bar and a half, on the music).
- Nuggets: 40; GS 22, waltz 21, fired up 24, no call 26, every nugget on some tour's line.
- Runs: GS 72.1 s, waltz 64.2 s, no call 65.4 s, fired up 56.0 s (it waits out one crusher
  cycle before the plunger). Clips: `nat-alone/w3-flush-giant.mp4`, `w3-flush-waltz.mp4`,
  `w3-flush-fired.mp4`, `w3-flush-sump.mp4`.
- The encore's promise: the ceiling over the goal's stage is built into the level (row 21, all
  but the grate's hole and the first four columns, where the Giant Steps arrival's arc flies
  through) and the grate is drawn from the start, with light falling through it onto the
  throne (it's above the screen as you come in; the beam isn't). The encore adds the missing
  seam tiles and slams the shutter, which puts the light out.
- Lessons: a jump and toot rises about 8 rows in Giant Steps (4.6 + the toot at the apex): the
  balcony 8 up needed lowering to 7.6. A sock hung over the walking row walls it off: hang it
  two rows up and leave two clear tiles after it to jump from. After a waltz spring a toot only
  helps once the rise has slowed under the toot's speed (at the top); earlier it does nothing.
  In a duck shaft the side that bats you last sends you out: time the jump in so it's the far
  one. Nuggets one row above a walking Nat need a hop timed to pass through them (0.1 s after
  take-off); put a walking route's nuggets in its row.
