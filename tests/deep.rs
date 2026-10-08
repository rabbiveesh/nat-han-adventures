//! Tier 2 formal checks (DEEP): `#[ignore]`d, run with `scripts/check-deep` (or
//! `cargo nextest run --test deep --run-ignored only`, or
//! `cargo test --test deep -- --ignored --nocapture`). Run them when touching the director,
//! physics, the groove, Han or the levels. Each test caches its pass in
//! `target/tmp/deep-check/` keyed by a hash of the levels and the source files it depends on
//! ([`SOURCES`]): unchanged, it prints "cached" and skips (`NATHAN_DEEP_FORCE=1` reruns).
//!
//! - [`deep_director`]: tier 1's exploration, deeper, with two events in one frame and finer
//!   waits.
//! - [`deep_campaign_gates_from_every_music_state`]: music state × physics × level
//!   reachability: for every campaign gate the band opens, from every director state tier 1
//!   reaches (the music you might arrive with), the player can summon the gate's mode with the
//!   level's own runway / nugget line / chute and have it playing (a bar line's latency
//!   included) from take-off to the far side.
//! - [`deep_boost_chains_cant_bypass_band_gates`]: Han's chain boosts near a band gate don't
//!   carry Nat past it.
//! - [`deep_han_reaches_his_gates`]: Han's navigation (in every mode the band can be in)
//!   reaches each of his gates from the start, or his parachute rule covers it.
//!
//! Every test runs on its own (no ordering, no shared state but the cache files, one per
//! test), so they work under nextest's process-per-test model.

mod formal_model;

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use formal_model::*;
use nat_han_adventures::audio::{Harmony, director};
use nat_han_adventures::game::tuning::*;
use nat_han_adventures::game::{Groove, SIDE_STAIN_LIFE};
use nat_han_adventures::level::buddy::{Chasm, ChasmMarks, GsSwitch, chain_plays, chain_try};
use nat_han_adventures::level::nav::{Nav, Route};
use nat_han_adventures::level::validate::{Map, Mode, Options, Physics, check_with};
use nat_han_adventures::level::{GateMark, LEVEL_SOURCES, Level, TILE, ThingKind, Topic};

// --- The cache ---

/// What the deep checks depend on (besides `levels/*.txt`): a change to any reruns them.
const SOURCES: &[&str] = &[
    "src/audio/director.rs",
    "src/audio/plugin.rs",
    "src/game/groove.rs",
    "src/game/physics.rs",
    "src/game/hazards.rs",
    "src/game/mod.rs",
    "src/game/han/mod.rs",
    "src/game/han/brain.rs",
    "src/game/han/world.rs",
    "src/level.rs",
    "src/level/validate.rs",
    "src/level/buddy.rs",
    "src/level/nav.rs",
    "tests/deep.rs",
    "tests/formal_model/mod.rs",
];

/// FNV-1a: stable across toolchains (unlike `DefaultHasher`).
fn fnv(h: &mut u64, bytes: &[u8]) {
    for &b in bytes {
        *h ^= b as u64;
        *h = h.wrapping_mul(0x100000001b3);
    }
}

fn inputs_hash() -> u64 {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files: Vec<PathBuf> = SOURCES.iter().map(|s| root.join(s)).collect();
    let mut levels: Vec<PathBuf> = std::fs::read_dir(root.join("levels"))
        .map(|d| {
            d.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect()
        })
        .unwrap_or_default();
    levels.sort();
    files.extend(levels);
    let mut h = 0xcbf29ce484222325;
    for f in files {
        fnv(&mut h, f.strip_prefix(root).unwrap_or(&f).to_string_lossy().as_bytes());
        // A missing file (moved, say) hashes as missing: the check reruns.
        fnv(&mut h, &std::fs::read(&f).unwrap_or_else(|_| b"<missing>".to_vec()));
    }
    h
}

/// Run `check` unless it passed with these exact inputs before; remember a pass.
fn cached(name: &str, check: impl FnOnce()) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("deep-check");
    let file = dir.join(format!("{name}.pass"));
    let key = format!("{:016x}", inputs_hash());
    let force = std::env::var("NATHAN_DEEP_FORCE").is_ok_and(|v| !v.is_empty() && v != "0");
    if !force && std::fs::read_to_string(&file).is_ok_and(|k| k.trim() == key) {
        eprintln!("{name}: cached (inputs {key} unchanged; NATHAN_DEEP_FORCE=1 reruns)");
        return;
    }
    let t0 = std::time::Instant::now();
    check();
    eprintln!("{name}: passed in {:.1?}", t0.elapsed());
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(&file, key);
}

fn threads() -> usize {
    std::thread::available_parallelism().map_or(4, |n| n.get())
}

fn campaign() -> Vec<Level> {
    LEVEL_SOURCES.iter().map(|s| Level::parse(s).expect("campaign level parses")).collect()
}

/// Tier 1's reachable director states: the music you might arrive at a gate with.
fn arrival_states() -> Vec<(Trace, Sim)> {
    let (ex, _) = explore_with(&Config::tier1(5), threads(), false, &|_, _| Ok(()));
    ex.states().map(|(i, s)| (ex.trace(i), s.clone())).collect()
}

// --- The director, deeper ---

/// Depth of the deep director search (tier 1's acts plus two-event frames and finer waits).
const DEEP_DEPTH: usize = 5;

/// Known director issues: (the frame that triggers it, the property, why it's accepted for
/// now). The search runs twice: with every act (known issues printed with their shortest
/// counterexample, anything else fails), and without the known issues' frames, where every
/// property must hold. Remove an entry once the director is fixed.
const KNOWN: &[(Act, &str, &str)] = &[(
    Act::Frame(TOOT | NUGGET),
    "b",
    "a toot and a nugget in the same frame that complete both summons at once: Giant Steps wins \
     and the nuggets' counter isn't cleared, so the fired-up rule stays met and 4 more quick \
     nuggets can't summon it until those nuggets age out (≤ 20 s; then the next check picks it). \
     Fix in `director::Band::step`/`begin`: clear (or re-arm) every rule met in the frame",
)];

#[test]
#[ignore = "tier 2 (deep): scripts/check-deep"]
fn deep_director() {
    cached("deep_director", || {
        let check = |s: &Sim, init: &Key| check_state(s, init);
        let all = Config::deep(DEEP_DEPTH);
        let mut clean = all.clone();
        clean.acts.retain(|a| !KNOWN.iter().any(|k| k.0 == *a));
        let mut fail = Vec::new();
        for (cfg, what) in [(&clean, "without the known issues' frames"), (&all, "every act")] {
            let (ex, found) = explore_with(cfg, threads(), false, &check);
            let c = &ex.coverage;
            eprintln!(
                "deep director ({what}): {} states (depth {}, {} acts, per layer {:?}) in {:.1?}; {} music states (filters × grip)",
                c.states,
                c.depth,
                cfg.acts.len(),
                c.layers,
                c.elapsed,
                c.music.len()
            );
            for ce in &found {
                let known = KNOWN.iter().find(|k| k.1 == ce.violation.prop && ce.state.0.contains(&k.0));
                match known {
                    Some((_, _, why)) if cfg.acts.len() == all.acts.len() => {
                        eprintln!("KNOWN ISSUE ({why}):\n{ce}\n")
                    }
                    _ => {
                        eprintln!("COUNTEREXAMPLE ({what}):\n{ce}\n");
                        fail.push(ce.violation.prop);
                    }
                }
            }
        }
        assert!(fail.is_empty(), "deep director: properties {fail:?} fail (counterexamples above)");
    });
}

// --- Music state × physics × levels ---

/// One bar of the slowest song (132 bpm, 4/4: 1.8 s), rounded up: a decision switches the
/// music (and the physics) in at the next bar line.
const BAR_TICKS: i32 = 8;

/// Seconds in the air of a ground jump in `g` (jump held `hold` s, toot at `toot` s).
fn air_time(g: &Groove, hold: f32, toot: Option<f32>) -> f32 {
    let dt = 1.0 / 60.0;
    let (gravity, max_fall) = (GRAVITY * g.gravity_scale, MAX_FALL * g.fall_scale());
    let (mut y, mut vy, mut t, mut cut, mut tooted) = (0.0f32, JUMP_SPEED, 0.0f32, false, false);
    while t < 5.0 {
        t += dt;
        if !cut && t >= hold {
            cut = true;
            if vy > 0.0 {
                vy *= JUMP_CUT;
            }
        }
        if !tooted && toot.is_some_and(|x| t >= x) {
            tooted = true;
            vy = DOUBLE_JUMP_SPEED;
        }
        vy = (vy - gravity * dt).max(-max_fall);
        y += vy * dt;
        if y <= 0.0 {
            return t;
        }
    }
    t
}

/// What a gate needs playing.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Need {
    Band(Harmony),
    Grip,
}

impl Need {
    fn met(self, s: &Sim) -> bool {
        match self {
            Need::Band(h) => s.harmony() == h,
            Need::Grip => s.grip(),
        }
    }

    /// Ticks the need must already hold before take-off (the bar line's latency; grip is a
    /// layer the plugin sets at once).
    fn lead(self) -> i32 {
        match self {
            Need::Band(_) => BAR_TICKS,
            Need::Grip => 0,
        }
    }
}

/// A way to summon a gate's mode and cross: acts ending at the take-off, then `cross` ticks
/// to the far side.
#[derive(Clone, Debug)]
struct Recipe {
    name: String,
    acts: Vec<Act>,
    cross: i32,
}

/// Times (s, from the recipe's start) of frames → acts on the grid.
fn timeline(events: &[(f32, Act)]) -> Vec<Act> {
    let mut acts = Vec::new();
    let mut tick = 0;
    for &(t, a) in events {
        let at = ticks(t).max(tick);
        if at > tick {
            acts.push(Act::Wait((at - tick) as u16));
            tick = at;
        }
        acts.push(a);
    }
    acts
}

/// How a recipe failed: what happened, and whether another summon took over after the
/// gate's mode had come (the interesting kind; the others are just too hasty for the bar line).
struct Failed {
    msg: String,
    stolen: bool,
}

/// Does `r` cross from `s`: `need` holding from `lead` ticks before the take-off (the last
/// act) to the far side? On failure: the trace from `s`, and what played.
fn attempt(s: &Sim, r: &Recipe, need: Need) -> Result<(), Failed> {
    let mut t = s.clone();
    let mut history: Vec<(i32, bool, Harmony)> = vec![(t.tick, need.met(&t), t.harmony())];
    for &a in &r.acts {
        t.apply_with(a, |x, _| history.push((x.tick, need.met(x), x.harmony())));
    }
    let takeoff = t.tick;
    t.apply_with(Act::Wait(r.cross as u16), |x, _| history.push((x.tick, need.met(x), x.harmony())));
    // The state just before each tick from take-off − lead on (the last frame of each tick).
    let from = takeoff - need.lead();
    let mut bad: Vec<(i32, Harmony)> = Vec::new();
    for (k, &(tick, ok, h)) in history.iter().enumerate() {
        let last_of_tick = history.get(k + 1).is_none_or(|n| n.0 != tick);
        if tick >= from && last_of_tick && !ok {
            bad.push((tick - takeoff, h));
        }
    }
    if bad.is_empty() {
        return Ok(());
    }
    let came = history.iter().skip(1).position(|e| e.1);
    let stolen = came
        .is_some_and(|i| history[i + 1..].iter().any(|e| !e.1 && Need::Band(e.2) != need && director::summonable(e.2)));
    let mut acts = r.acts.clone();
    acts.push(Act::Wait(r.cross as u16));
    Err(Failed {
        msg: format!(
            "{}: {:?} missing at take-off{:+.2}s (playing {:?}); from the state: {}",
            r.name,
            need,
            bad[0].0 as f32 * TICK,
            bad[0].1,
            Trace(acts)
        ),
        stolen,
    })
}

/// Recipes for a giant wall: five jump-toot cycles on the runway (each `pause` after the
/// landing), then the wall jump after `wait`.
fn giant_recipes(g: &Groove) -> Vec<Recipe> {
    // On the grid: the air time rounded up (a moment on the ground before the next jump).
    let cycle = (air_time(g, f32::INFINITY, Some(0.27)) / TICK).ceil() * TICK;
    let mut out = Vec::new();
    for pause in [0.0, 0.25, 0.5, 1.0] {
        for wait in [0.0, 1.0, BAR_TICKS as f32 * TICK] {
            let mut ev = Vec::new();
            let mut t = 0.0;
            for _ in 0..director::GIANT_STEPS_TOOTS {
                ev.push((t, Act::JUMP));
                ev.push((t + 0.27, Act::TOOT));
                t += cycle + pause;
            }
            ev.push((t - pause + wait, Act::JUMP));
            out.push(Recipe {
                name: format!("5 jump-toots ({cycle:.2}s + {pause}s), wall jump after {wait}s"),
                acts: timeline(&ev),
                cross: 6,
            });
        }
    }
    out
}

/// Recipes for a long gap: the level's nugget line (times at a human 90% run in `g`), the
/// run-up, and the jump after `wait`.
fn gap_recipes(g: &Groove, nuggets: &[f32], run_up: f32) -> Vec<Recipe> {
    let speed = RUN_SPEED * g.speed_scale * 0.9;
    let mut out = Vec::new();
    for wait in [0.0, BAR_TICKS as f32 * TICK] {
        let mut ev: Vec<(f32, Act)> = nuggets.iter().map(|&d| (d * TILE / speed, Act::NUGGET)).collect();
        let last = ev.last().map_or(0.0, |e| e.0);
        ev.push((last + run_up * TILE / speed + wait, Act::JUMP));
        out.push(Recipe {
            name: format!("{} nuggets at {:.0} px/s, jump after {wait}s", nuggets.len(), speed),
            acts: timeline(&ev),
            cross: 6,
        });
    }
    out
}

/// Recipes for a waltz row: three even ground jumps (as quick as the hop in `g` allows),
/// then walk to the row and dash through (`dash` s); or first wait out what's held.
fn waltz_recipes(g: &Groove, dash: f32) -> Vec<Recipe> {
    let hop = air_time(g, 0.05, None);
    let mut out = Vec::new();
    for pre in [0, HOLD_TICKS + director::MUSIC_CHECK_SECS as i32 * 4] {
        for gap in [0.5f32, 0.75, 1.0] {
            if gap < hop {
                continue;
            }
            let p = pre as f32 * TICK;
            let ev =
                [(p, Act::JUMP), (p + gap, Act::JUMP), (p + 2.0 * gap, Act::JUMP), (p + 2.0 * gap + 2.5, Act::Wait(0))];
            let acts: Vec<Act> = timeline(&ev).into_iter().filter(|a| *a != Act::Wait(0)).collect();
            out.push(Recipe {
                name: format!(
                    "{}3 jumps {gap}s apart (hop {hop:.2}s), 2.5s to the row",
                    if pre > 0 { "wait 40s, " } else { "" }
                ),
                acts,
                cross: ticks(dash) + 1,
            });
        }
    }
    out
}

/// Recipes for a grease chute: `laps` × (splat, respawn, walk back `walk` s grabbing
/// `ghosts` ghost nuggets, maybe tooting 5 times), then the slide that crosses.
fn chute_recipes(walk: f32, ghosts: usize) -> Vec<Recipe> {
    let mut out = Vec::new();
    for toots in [0u32, 5] {
        let mut ev = Vec::new();
        let mut t = 0.0;
        for _ in 0..director::NERVOUS_DEATHS {
            ev.push((t, Act::DEATH));
            let back = t + RESPAWN_DELAY;
            for k in 0..ghosts {
                ev.push((back + walk * (k + 1) as f32 / (ghosts + 1) as f32, Act::NUGGET));
            }
            for k in 0..toots {
                ev.push((back + walk * (k as f32 + 0.5) / toots as f32, Act::TOOT));
            }
            t = back + walk;
        }
        ev.sort_by(|a, b| a.0.total_cmp(&b.0));
        ev.push((t, Act::Wait(0)));
        let acts: Vec<Act> = timeline(&ev).into_iter().filter(|a| *a != Act::Wait(0)).collect();
        out.push(Recipe {
            name: format!(
                "{} chute laps ({walk:.1}s back, {ghosts} ghost nuggets, {toots} toots each)",
                director::NERVOUS_DEATHS
            ),
            acts,
            cross: 8,
        });
    }
    out
}

/// Nuggets on the approach to `m` (within the validator's nugget-line reach, rows near the
/// take-off): distances (tiles) along the run, and the run-up from the last to the take-off.
fn nugget_line(level: &Level, m: &GateMark) -> (Vec<f32>, f32) {
    let row = m.r1;
    let side = |dir: i32| -> Vec<i32> {
        let edge = if dir > 0 { m.c0 + 1 } else { m.c1 - 1 };
        let mut v: Vec<i32> = level
            .things
            .iter()
            .filter(|t| t.kind == ThingKind::Nugget && (row - 4..=row + 1).contains(&(t.row as i32)))
            .map(|t| (edge - t.col as i32) * dir)
            .filter(|b| (0..=30).contains(b))
            .collect();
        v.sort();
        v
    };
    let (r, l) = (side(1), side(-1));
    let mut back = if r.len() >= l.len() { r } else { l };
    back.truncate(director::FIRED_UP_NUGGETS as usize);
    let far = back.last().copied().unwrap_or(0) as f32;
    let dists: Vec<f32> = back.iter().rev().map(|&b| far - b as f32).collect();
    (dists, back.first().copied().unwrap_or(0) as f32)
}

/// Walk back to a chute from the nearest respawn before it (s at 90% run), and the nuggets on
/// the way (ghosts after a splat).
fn chute_lap(level: &Level, m: &GateMark) -> (f32, usize) {
    let respawns = std::iter::once(level.start.0 as i32).chain(level.checkpoints().map(|t| t.col as i32));
    let (from, dist) = respawns
        .map(|c| (c, if c <= m.c0 { m.c0 - c } else { c - m.c1 }))
        .min_by_key(|x| x.1)
        .expect("a level has a start");
    let (a, b) = if from <= m.c0 { (from, m.c0) } else { (m.c1, from) };
    let ghosts = level
        .things
        .iter()
        .filter(|t| {
            t.kind == ThingKind::Nugget
                && (a..=b).contains(&(t.col as i32))
                && (m.r0 - 6..=m.r1 + 2).contains(&(t.row as i32))
        })
        .count();
    // Path cost is at least the columns between, and the slide down the chute.
    let walk = (dist + (m.c1 - m.c0) / 2) as f32 * TILE / (RUN_SPEED * 0.9);
    (walk, ghosts)
}

/// The band gates' marks in a level, with what they need.
fn band_gates(level: &Level) -> Vec<(GateMark, Need, &'static str)> {
    level
        .gates
        .iter()
        .filter_map(|m| {
            let (need, what) = match m.topic {
                Topic::Giant => (Need::Band(Harmony::Coltrane), "giant wall"),
                Topic::Gap => (Need::Band(Harmony::Quartal), "long gap"),
                Topic::Waltz => (Need::Band(Harmony::Waltz), "waltz row"),
                Topic::Grip => (Need::Grip, "grease chute"),
                _ => return None,
            };
            Some((*m, need, what))
        })
        .collect()
}

/// A gate's recipes for a player arriving with the physics `g`.
fn recipes(level: &Level, m: &GateMark, g: &Groove) -> Vec<Recipe> {
    match m.topic {
        Topic::Giant => giant_recipes(g),
        Topic::Gap => {
            let (line, run_up) = nugget_line(level, m);
            gap_recipes(g, &line, run_up)
        }
        Topic::Waltz => waltz_recipes(g, (m.c1 - m.c0 + 1) as f32 * TILE / (RUN_SPEED * g.speed_scale * 0.9)),
        _ => {
            let (walk, ghosts) = chute_lap(level, m);
            chute_recipes(walk, ghosts)
        }
    }
}

#[test]
#[ignore = "tier 2 (deep): scripts/check-deep"]
fn deep_campaign_gates_from_every_music_state() {
    cached("deep_campaign_gates_from_every_music_state", || {
        let levels = campaign();
        let phys = Physics::new();
        let states = arrival_states();
        let mut errors = Vec::new();
        let mut gates = 0;
        let mut attempts = 0usize;
        for (li, level) in levels.iter().enumerate() {
            let report = check_with(level, &Options::default(), &phys);
            assert!(report.errs.is_empty(), "level {}: the validator fails: {:?}", li + 1, report.errs);
            for (m, need, what) in band_gates(level) {
                gates += 1;
                let gate = format!("L{} {what} (mark {},{} {},{})", li + 1, m.c0, m.r0, m.c1, m.r1);
                if m.topic == Topic::Grip {
                    // The side splat must have faded by the time you slide in again.
                    let back = RESPAWN_DELAY + chute_lap(level, &m).0;
                    if back <= SIDE_STAIN_LIFE {
                        errors.push(format!(
                            "{gate}: back on the chute {back:.1}s after a splat, but a side splat lasts {SIDE_STAIN_LIFE}s"
                        ));
                    }
                }
                // Per state: did some recipe work (a chute: all of them), and what failed.
                let results: Vec<(bool, Vec<(usize, Failed)>)> = std::thread::scope(|scope| {
                    let chunk = states.len().div_ceil(threads());
                    let hs: Vec<_> = states
                        .chunks(chunk)
                        .map(|part| {
                            scope.spawn(move || {
                                part.iter()
                                    .map(|(_, s)| {
                                        let rs = recipes(level, &m, &s.groove());
                                        let fails: Vec<(usize, Failed)> = rs
                                            .iter()
                                            .enumerate()
                                            .filter_map(|(k, r)| attempt(s, r, need).err().map(|e| (k, e)))
                                            .collect();
                                        let ok =
                                            if need == Need::Grip { fails.is_empty() } else { fails.len() < rs.len() };
                                        (ok, fails)
                                    })
                                    .collect::<Vec<_>>()
                            })
                        })
                        .collect();
                    hs.into_iter().flat_map(|h| h.join().expect("recipe panicked")).collect()
                });
                attempts += results.iter().map(|r| r.1.len()).sum::<usize>();
                if let Some(i) = results.iter().position(|r| !r.0) {
                    errors.push(format!(
                        "{gate}: can't be crossed from the state `{}`:\n    {}",
                        states[i].0,
                        results[i].1.iter().map(|f| f.1.msg.as_str()).collect::<Vec<_>>().join("\n    ")
                    ));
                }
                // Fragile ways: recipes where another summon takes over from the gate's mode.
                let mut fragile: BTreeMap<usize, (usize, usize)> = BTreeMap::new();
                for (i, (_, fails)) in results.iter().enumerate() {
                    for (k, _) in fails.iter().filter(|f| f.1.stolen) {
                        fragile.entry(*k).or_insert((0, i)).0 += 1;
                    }
                }
                let ok = results.iter().filter(|r| r.0).count();
                eprintln!("{gate}: crossable from {ok}/{} arrival states", states.len());
                // The most common one, with an example (the first state, the shortest trace).
                if let Some((&k, &(n, i))) = fragile.iter().max_by_key(|e| (e.1.0, std::cmp::Reverse(*e.0))) {
                    let msg = &results[i].1.iter().find(|f| f.0 == k).expect("recorded").1.msg;
                    eprintln!(
                        "  note: in {} way(s) of crossing another summon takes over (up to {n} states), e.g. after `{}`:\n    {msg}",
                        fragile.len(),
                        states[i].0
                    );
                }
            }
        }
        eprintln!("{gates} band gates × {} arrival states ({attempts} failed attempts noted)", states.len());
        assert!(errors.is_empty(), "gates not crossable from every music state:\n{}", errors.join("\n"));
    });
}

// --- Boost chains near band gates ---

/// Chain-boost bypasses accepted for now: (level, gate topic word). Printed every run. Empty:
/// with Han's real head rules (the weak boost and no mid-air catches in band zones, the catch
/// height cap) no chain carries Nat past a band gate.
const KNOWN_BYPASSES: &[(usize, &str)] = &[];

/// Han's chain boosts (the validator's own chain simulation, `buddy::chain_try`: Nat jumps,
/// Han intercepts, Nat boosts off his head and toots, again, with the game's head rules: the
/// weak boost and no mid-air catches in band zones, the catch height cap, the level's own
/// chasm marks), taken off from anywhere Nat can stand with Han's boost full strength behind
/// him on the way to a band gate, must not carry Nat past the gate's mark. Written against the
/// public buddy API only, so a change to Han's head rules is picked up as it lands.
#[test]
#[ignore = "tier 2 (deep): scripts/check-deep"]
fn deep_boost_chains_cant_bypass_band_gates() {
    cached("deep_boost_chains_cant_bypass_band_gates", || {
        let levels = campaign();
        // (level, topic word) → (bypassing take-offs, an example).
        let mut found: BTreeMap<(usize, &str), (usize, String)> = BTreeMap::new();
        let mut tried = 0;
        for (li, level) in levels.iter().enumerate() {
            let map = Map::new(level);
            for (m, _, what) in band_gates(level) {
                // Heading the way you come to it from the start; take-offs on the mark's rows
                // up to two chasms' width (28 tiles) before it; landing past its far side.
                let dir = if (level.start.0 as i32) < m.c0 { 1 } else { -1 };
                for r in m.r0..=m.r1 {
                    for k in 0..=28 {
                        let edge = if dir > 0 { m.c0 - k } else { m.c1 + k };
                        if !map.standable((edge, r)) || !level.han_allowed((edge - dir * 2, r)) {
                            continue;
                        }
                        let chasm = if dir > 0 {
                            Chasm { row: r, c0: edge + 1, c1: m.c1 }
                        } else {
                            Chasm { row: r, c0: m.c0, c1: edge - 1 }
                        };
                        for play in chain_plays() {
                            tried += 1;
                            if let Some(land) = chain_try(level, &chasm, dir, play, GsSwitch::Never, ChasmMarks::Level) {
                                let e = found.entry((li + 1, m.topic.word())).or_insert((0, String::new()));
                                e.0 += 1;
                                if e.1.is_empty() {
                                    e.1 = format!(
                                        "L{} {what} (mark {},{} {},{}): e.g. a chain from col {edge} row {r} heading {dir:+} ({play:?}) lands at {land:?}",
                                        li + 1,
                                        m.c0,
                                        m.r0,
                                        m.c1,
                                        m.r1
                                    );
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }
        eprintln!("{tried} chain plays tried before the campaign's band gates");
        let mut fail = Vec::new();
        for (key, (n, msg)) in &found {
            if KNOWN_BYPASSES.contains(key) {
                eprintln!("KNOWN BYPASS ({n} take-offs): {msg}");
            } else {
                eprintln!("BYPASS ({n} take-offs): {msg}");
                fail.push(msg.clone());
            }
        }
        for k in KNOWN_BYPASSES.iter().filter(|k| !found.contains_key(k)) {
            eprintln!("STALE: L{} {} no longer bypassed: remove it from KNOWN_BYPASSES", k.0, k.1);
        }
        assert!(fail.is_empty(), "{} new boost-chain bypasses of band gates (above)", fail.len());
    });
}

// --- Han reaches his gates ---

/// The modes the band can put the game in (from tier 1's reachable music).
fn reachable_modes() -> Vec<Mode> {
    let mut modes: Vec<Mode> = arrival_states()
        .iter()
        .flat_map(|(_, s)| {
            let m = match s.harmony() {
                Harmony::Original => Mode::Normal,
                Harmony::Coltrane => Mode::GiantSteps,
                Harmony::Quartal => Mode::FiredUp,
                Harmony::Waltz => Mode::Waltz,
                Harmony::MelodicMinor => Mode::Nervous,
            };
            [Some(m), s.grip().then_some(Mode::Nervous)]
        })
        .flatten()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    modes.sort();
    modes
}

/// Parachute landings near `at` (the rule of `game::han`'s `parachute_spot`: behind or ahead
/// of Nat by up to 6 tiles, floating down to the first floor, outside the band's gates).
fn parachute_landings(map: &Map, level: &Level, at: (i32, i32)) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    for dc in [-6, -4, -3, -1, 0, 1, 3, 4, 6] {
        let c = at.0 + dc;
        let mut r = at.1 - 4;
        while r < level.height as i32 && !Nav::node(map, level, (c, r)) && !level.tile(c, r).is_solid() {
            r += 1;
        }
        if Nav::node(map, level, (c, r)) {
            out.push((c, r));
        }
    }
    out
}

/// Where Han works at each of his gates must be reachable by him: routed from the level start
/// with his own physics in every mode the band can be in, or (where the band's gates are in the
/// way) by parachuting in next to Nat and walking the rest.
#[test]
#[ignore = "tier 2 (deep): scripts/check-deep"]
fn deep_han_reaches_his_gates() {
    cached("deep_han_reaches_his_gates", || {
        let levels = campaign();
        let modes = reachable_modes();
        eprintln!("modes the band can be in: {modes:?}");
        let mut errors = Vec::new();
        let (mut routed, mut chuted) = (0, 0);
        for (li, level) in levels.iter().enumerate() {
            let map = Map::for_han(level);
            let start = (level.start.0 as i32, level.start.1 as i32);
            for m in level
                .gates
                .iter()
                .filter(|g| matches!(g.topic, Topic::Boost | Topic::Shield | Topic::Chain | Topic::BuddyRaft))
            {
                // His spots: the mark's cells he can stand in.
                let spots: Vec<(i32, i32)> = (m.r0..=m.r1)
                    .flat_map(|r| (m.c0..=m.c1).map(move |c| (c, r)))
                    .filter(|&c| Nav::node(&map, level, c))
                    .collect();
                let gate = format!("L{} {} (mark {},{} {},{})", li + 1, m.topic.word(), m.c0, m.r0, m.c1, m.r1);
                if spots.is_empty() {
                    errors.push(format!("{gate}: no cell Han can stand in"));
                    continue;
                }
                for &mode in &modes {
                    let mut nav = Nav::new(mode);
                    let target = spots[0];
                    if matches!(nav.route(&map, start, target, usize::MAX), Route::Found(_)) {
                        routed += 1;
                        continue;
                    }
                    // Parachute in next to Nat (standing at any of the spots), then walk.
                    let ok = spots.iter().any(|&nat| {
                        parachute_landings(&map, level, nat)
                            .into_iter()
                            .any(|p| matches!(nav.route(&map, p, target, usize::MAX), Route::Found(_)))
                    });
                    if ok {
                        chuted += 1;
                    } else {
                        errors.push(format!("{gate} in {mode:?}: no route from the start to {target:?}, and no parachute landing near it routes there"));
                    }
                }
            }
        }
        eprintln!("Han's gates × modes: {routed} routed from the start, {chuted} by parachute");
        assert!(errors.is_empty(), "Han can't reach:\n{}", errors.join("\n"));
    });
}
