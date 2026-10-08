# Adaptive difficulty: tuning notes

The rules are in the module doc (`src/adapt/mod.rs`). This file records how the constants were
picked, using the simulator:

```sh
cargo run --example simulate -- --all --seed 0 --seeds 20     # summary, 20 seeds averaged
cargo run --example simulate -- --profile nervous_beginner --seed 3   # room-by-room
```

The simulator plays the real reducer and `next_room`. Each synthetic player has a logistic
p(clean) curve per skill and band. Assists remove `ASSIST_HELP` = 50% of the remaining failure
chance at a full dial. Skills unlock on the game's likely schedule: three at the start, then one
every 15 rooms. Metrics: **clean** = clean-clear rate; **warm** = after the first 30 rooms;
**near target** = share of rooms whose trailing-20 clean rate is within 60–85%; **osc** = band
changes that reverse the same skill's previous change within 25 rooms (ping-pong); **rev** = all
reversals, however far apart (a skill slowly tracking a level); **maxed** = rooms played with
the dial ≥ 0.9.

## Result (200 rooms, seeds 0..19)

| profile          | clean | warm clean | near target | osc | rev | prom/dem  | frust | assists | maxed |
|------------------|-------|------------|-------------|-----|-----|-----------|-------|---------|-------|
| precise          |   81% |        80% |         76% | 1.6 | 4.6 | 12.4/0.8  |   7.4 |    0.07 |    0% |
| sloppy           |   69% |        69% |         82% | 0.8 | 4.1 |  6.7/2.1  |   4.2 |    0.13 |    1% |
| nervous_beginner |   62% |        63% |         76% | 0.0 | 0.0 |  0.0/0.1  |  30.4 |    0.60 |    9% |
| speedrunner      |   77% |        76% |         85% | 1.0 | 4.0 | 12.6/1.3  |   5.7 |    0.07 |    0% |
| uneven           |   73% |        75% |         86% | 0.9 | 4.1 |  3.8/1.5  |  13.5 |    0.16 |    1% |
| learner          |   86% |        88% |         44% | 0.9 | 3.0 | 20.0/0.3  |   4.8 |    0.02 |    0% |

## What changed while tuning, and why

1. **The starting point was robot-game's rules as written.** Strong players climbed far too
   slowly: eight skills split the evidence, and a precise player was still two bands under its
   level after 200 rooms. Added `AdaptEvent::SkillUnlocked`: a never-played skill starts at the
   mean played center minus `UNLOCK_OFFSET` = 1, instead of at band 1.
2. **Frustration on a stretch room no longer drops the center.** A precise player dying 3 times
   on a +2 stretch room lost a good center, then spent about 20 rooms earning it back (a
   reversal). Now a frustrated stretch room only narrows the spread, so fewer stretch rooms
   follow. A frustrated room at the center still drops it by 1, as specified.
3. **Hysteresis (`REPROMOTE_EVIDENCE` = 8).** A sloppy player whose true level sits on a band
   boundary (p(4)=0.72, p(5)=0.58) went 4→5→4→5 on 4-room luck. Promoting back into a band it
   just fell out of now needs 8 rooms of evidence.
4. **Oscillation metric.** Counting every reversal punished skills that slowly track a level
   (down at room 30, up again at 123). `osc` now counts only reversals within 25 rooms. `rev`
   still shows all of them.
5. **Calibration slips.** A speedrunner's quick death on probe 1 counted as a struggle, which
   placed it at band 2 (true level about 9). During calibration, any single death under 1.5s
   now counts as a slip, because there's no clean run yet to judge it against.
6. **Assist dial variance.** The per-death rise and the frustration bump stacked: one bad room
   could add 0.3, and nervous players hit the max 22% of the time on bad seeds. Now a
   frustrated room gets the larger of the two, not both. The frustration bump also diminishes
   as the dial rises (`0.15·(1−dial)`).
7. **Dial equilibrium.** For a struggling player the dial settles where
   `p·FADE ≈ (1−p)·RISE·E[excess]`. With `ASSIST_FADE` = 0.07 and `ASSIST_RISE` = 0.08,
   nervous_beginner settles at about 63% clean (it was 60% at RISE 0.06, the low edge), with
   the dial maxed in under 10% of rooms. `WIDEN_STEP` went from 0.05 to 0.1 so high performers
   see stretch rooms sooner.

Tried and reverted: counting clean stretch rooms as center evidence. It sped up the learner
only slightly (44%→50% near target), raised precise's oscillation (1.4→2.5), and departed from
the "at center" rule.

## Known limits

- **Fast learners outpace the evidence.** `learner` gains about 4 bands of real ability over
  200 rooms. The engine follows about 2 bands behind, at 88% clean, which is above the 85%
  target. That's the cost of per-skill windows with fresh evidence, and it errs on the easy
  side. Rooms that exercise several skills (e.g. Precision plus the room's main skill) give each
  of them evidence and will help.
- **nervous_beginner** idles after a quarter of its struggles, so it raises about 30 frustration
  cues per 200 rooms. The engine eases off every time; Han's lines should rotate so they don't
  feel repetitive.
- Robot-game's spread rates (widen 0.05, narrow 0.1/0.15) were set by feel, and so are these.
  Re-tune once there's real playtest data.
