//! The campaign's levels against the level validator (`nat_han_adventures::level::validate`,
//! which documents the rules): every level parses, is valid and beatable, matches the campaign
//! plan, declares its deaths, and every mechanic is taught (a hint spot) in the first level
//! where it appears.
//!
//! `LEVEL_DUMP=1 cargo test --test levels -- --nocapture` prints every level with the reachable
//! cells marked (add `LEVEL_ONLY=3` for just level 3):
//! `+` standable & reachable, `,` passed through by some safe arc, `X` unreachable nugget,
//! `!` unreachable checkpoint/goal, `@` moving platform (start), `-` its path, `%` a splat
//! stain the search made, `?` a hint spot, and the take-off of each gate crossing: `W` giant
//! wall (Giant Steps), `R` long gap (fired up), `Z` waltz row, `Y` grease chute (grip), `K`
//! stain pit (splats).

use std::time::Instant;

use nat_han_adventures::game::tuning::RUN_SPEED;
use nat_han_adventures::level::validate::*;
use nat_han_adventures::level::*;

/// Name, world and the gates the level must use.
const PLAN: [(&str, u8, &[Gate]); LEVEL_COUNT] = [
    ("Bathroom Floor", 1, &[Gate::GiantWall]),
    ("The Bowl", 1, &[]),
    ("U-Bend", 2, &[Gate::LongGap, Gate::StainPit]),
    ("Pipe Maze", 2, &[Gate::GiantWall]),
    ("Main Sewer", 3, &[Gate::LongGap, Gate::StainPit]),
    ("Rat Kingdom", 3, &[Gate::GiantWall, Gate::GreaseChute]),
    ("Septic Tank", 4, &[Gate::WaltzRow]),
    ("Porta-Potty Festival", 4, &[Gate::GiantWall, Gate::WaltzRow, Gate::StainPit]),
    ("Treatment Plant", 5, &[Gate::LongGap, Gate::WaltzRow, Gate::GreaseChute]),
    ("The Golden Throne", 5, &[Gate::GiantWall]),
];
/// Levels whose goal can only be reached through their gate (the tutorials of each mechanic).
const GATED_GOAL: [usize; 2] = [0, 2];

#[test]
fn levels_are_valid_and_beatable() {
    let t0 = Instant::now();
    let dump = std::env::var("LEVEL_DUMP").is_ok();
    let only = std::env::var("LEVEL_ONLY").ok().and_then(|v| v.parse::<usize>().ok());
    let phys = Physics::new();
    let levels: Vec<Result<Level, String>> = LEVEL_SOURCES.iter().map(|s| Level::parse(s)).collect();
    // Every level on its own thread.
    let reports: Vec<Option<Report>> = std::thread::scope(|scope| {
        let handles: Vec<_> = levels
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let phys = &phys;
                scope.spawn(move || {
                    let opts = Options { dump: dump && only.is_none_or(|o| o == i + 1) };
                    l.as_ref().ok().map(|l| check_with(l, &opts, phys))
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().expect("validator panicked")).collect()
    });
    eprintln!("validated {} levels in {:.2?}", LEVEL_COUNT, t0.elapsed());

    let mut failures = Vec::new();
    for (i, (level, report)) in levels.iter().zip(&reports).enumerate() {
        let (level, report) = match (level, report) {
            (Err(e), _) => {
                failures.push(format!("levels/{:02}.txt: parse error: {e}", i + 1));
                continue;
            }
            (Ok(l), Some(r)) => (l, r),
            _ => unreachable!(),
        };
        let mut errs = report.errs.clone();
        let (name, world, gates) = PLAN[i];
        if level.name != name {
            errs.push(format!("name is {:?}, expected {name:?}", level.name));
        }
        if level.world != world {
            errs.push(format!("world is {}, expected {world}", level.world));
        }
        if level.deaths.is_none() {
            errs.push("no `deaths:` line (the deaths the design expects; 0 is fine)".into());
        }
        if report.errs.is_empty() {
            // The campaign's gates: exactly the planned kinds.
            let mut got: Vec<Gate> = report.gates.iter().map(|g| g.0).collect();
            got.sort();
            got.dedup();
            let mut want = gates.to_vec();
            want.sort();
            if got != want {
                errs.push(format!("gates {got:?}, planned {want:?}"));
            }
            if GATED_GOAL.contains(&i) && !report.gated_goal {
                errs.push("the goal must be behind the level's gate (it teaches it)".into());
            }
            for &(gate, _, _, splats) in &report.gates {
                if gate == Gate::StainPit && !(1..=MAX_PIT_DEATHS).contains(&splats) {
                    errs.push(format!("a stain pit takes {splats} splats (want 1..={MAX_PIT_DEATHS})"));
                }
            }
        }
        if let Some(d) = &report.dump
            && (dump && (only.is_none_or(|o| o == i + 1) || !errs.is_empty()))
        {
            println!("levels/{:02}.txt {d}", i + 1);
            for l in &report.lessons {
                println!("  {:?} at {:?} (cost {}), hint {:?}", l.topic, l.at, l.dist, l.hint);
            }
        }
        for e in errs {
            failures.push(format!("levels/{:02}.txt ({}): {e}", i + 1, level.name));
        }
    }

    // Teaching: the first level where a mechanic appears teaches it with a hint before it.
    for topic in Topic::ALL {
        let first = reports
            .iter()
            .enumerate()
            .find_map(|(i, r)| r.as_ref()?.lessons.iter().find(|l| l.topic == topic).map(|l| (i, l)));
        match first {
            Some((i, lesson)) if !lesson.taught() => failures.push(format!(
                "levels/{:02}.txt: first {} (col {} row {}, path cost {}) needs a `hint@... {}:` before it (hint: {:?})",
                i + 1,
                topic.word(),
                lesson.at.0,
                lesson.at.1,
                lesson.dist,
                topic.word(),
                lesson.hint
            )),
            None => failures.push(format!("no level has {} at all", topic.word())),
            _ => {}
        }
    }
    assert!(
        failures.is_empty(),
        "level problems (LEVEL_DUMP=1 LEVEL_ONLY=N cargo test --test levels -- --nocapture to see a map):\n{}",
        failures.join("\n")
    );
}

/// A test level: `rows` (grid lines), plus 15 nuggets in row `nugget_row` cols 3..18 so it
/// meets the nugget minimum.
fn level_of(header: &str, rows: &[String], nugget_row: usize) -> Level {
    let src = format!("name: Test\nworld: 1\nintro: hi\n{header}---\n{}\n", rows.join("\n"));
    let mut l = Level::parse(&src).unwrap();
    for c in 3..18 {
        l.things.push(Thing { kind: ThingKind::Nugget, col: c, row: nugget_row });
    }
    l
}

fn run(l: &Level) -> Report {
    static PHYS: std::sync::OnceLock<Physics> = std::sync::OnceLock::new();
    check_with(l, &Options::default(), PHYS.get_or_init(Physics::new))
}

fn gates(r: &Report) -> Vec<Gate> {
    r.gates.iter().map(|g| g.0).collect()
}

/// The validator itself must reject obviously broken levels.
#[test]
fn validator_catches_broken_levels() {
    let raw = "say@19,13: hey\n---\n\
              ..........................................\n\
              .P.................C.....................G\n\
              ##########################################\n";
    let pad = |s: &str| {
        // Pad to the minimum height with sky rows.
        let (h, g) = s.split_once("---\n").unwrap();
        format!("{h}---\n{}{g}", format!("{}\n", ".".repeat(42)).repeat(12))
    };
    let ok = &pad(raw);
    let check_with_nuggets = |s: &str, nuggets: &[usize]| {
        let (header, grid) = s.split_once("---\n").unwrap();
        let rows: Vec<String> = grid.lines().map(String::from).collect();
        let mut l = level_of(header, &rows, 13);
        for &c in nuggets {
            l.things.push(Thing { kind: ThingKind::Nugget, col: c, row: 13 });
        }
        run(&l)
    };
    let errs_of = |s: &str| check_with_nuggets(s, &[]).errs;
    let modes_of = |s: &str| gates(&check_with_nuggets(s, &[]));
    assert_eq!(errs_of(ok), Vec::<String>::new());
    assert!(modes_of(ok).is_empty());

    let with_gap = |n: usize| {
        let mut lines: Vec<String> = ok.lines().map(String::from).collect();
        let last = lines.len() - 1;
        let mut floor: Vec<char> = lines[last].chars().collect();
        for c in floor.iter_mut().skip(25).take(n) {
            *c = '.';
        }
        lines[last] = floor.into_iter().collect();
        lines.join("\n") + "\n"
    };
    assert!(errs_of(&with_gap(4)).is_empty(), "{:?}", errs_of(&with_gap(4)));
    assert!(errs_of(&with_gap(7)).is_empty(), "double jump clears 7: {:?}", errs_of(&with_gap(7)));
    assert!(modes_of(&with_gap(7)).is_empty());
    // Gate marks go in the header (Han keeps clear of the band's gates).
    let mark = |s: &str, m: &str| s.replace("say@19,13: hey\n", &format!("say@19,13: hey\n{m}\n"));
    // A long gap: only with the band fired up, so it needs a nugget line after the checkpoint.
    let long = mark(&with_gap(11), "gate: gap 20,13 40,13");
    assert!(errs_of(&long).iter().any(|e| e.contains("goal")), "11 tiles is too far without a nugget line");
    let fired = check_with_nuggets(&long, &[20, 21, 22, 23]);
    assert!(fired.errs.is_empty(), "a nugget line fires up the band for 11 tiles: {:?}", fired.errs);
    assert_eq!(gates(&fired), [Gate::LongGap]);
    assert!(fired.gated_goal);
    assert!(
        check_with_nuggets(&mark(&with_gap(14), "gate: gap 20,13 40,13"), &[20, 21, 22, 23]).errs.iter().any(|e| e.contains("goal")),
        "a 14-tile gap must make the goal unreachable"
    );

    // A wall the player can't climb.
    let wall = |n: usize| {
        let mut lines: Vec<String> = ok.lines().map(String::from).collect();
        let floor_idx = lines.len() - 1;
        for k in 0..n {
            let idx = floor_idx - 1 - k;
            let mut row: Vec<char> = lines[idx].chars().collect();
            for c in row.iter_mut().skip(30) {
                if *c == '.' || *c == 'G' {
                    *c = '#';
                }
            }
            lines[idx] = row.into_iter().collect();
        }
        // Put the goal on top of the wall.
        let top = floor_idx - 1 - n;
        let mut row: Vec<char> = lines[top].chars().collect();
        *row.last_mut().unwrap() = 'G';
        lines[top] = row.into_iter().collect();
        lines.join("\n") + "\n"
    };
    assert!(errs_of(&wall(3)).is_empty(), "single jump climbs 3: {:?}", errs_of(&wall(3)));
    assert!(errs_of(&wall(5)).is_empty(), "double jump climbs 5: {:?}", errs_of(&wall(5)));
    assert!(modes_of(&wall(5)).is_empty(), "5 tiles is a normal wall");
    // A giant wall: only in Giant Steps, summoned on the runway before it. (Exclusive: not
    // with a waltz jump on ONE plus its weak toot either.)
    let giant_mark = "gate: giant 20,5 41,13";
    let giant = check_with_nuggets(&mark(&wall(6), giant_mark), &[]);
    assert!(giant.errs.is_empty(), "giant steps climbs 6: {:?}", giant.errs);
    assert_eq!(gates(&giant), [Gate::GiantWall]);
    assert!(giant.gated_goal);
    // 9 tiles: beyond Giant Steps, so Han keeping clear of it makes it a dead end...
    assert!(errs_of(&mark(&wall(9), giant_mark)).iter().any(|e| e.contains("goal")), "9 tiles is too tall");
    // ...but a buddy ledge if he comes along: the plunger boost (and it must be marked).
    let ledge = check_with_nuggets(&mark(&wall(9), "gate: boost 20,3 41,13"), &[]);
    assert!(ledge.errs.is_empty(), "the boost climbs 9: {:?}", ledge.errs);
    assert_eq!(gates(&ledge), [Gate::BuddyLedge]);
    assert!(errs_of(&wall(9)).iter().any(|e| e.contains("`gate: boost")), "unmarked: {:?}", errs_of(&wall(9)));
    assert!(errs_of(&wall(11)).iter().any(|e| e.contains("goal")), "11 tiles is too tall even for the boost");
    // No runway (ceiling spikes 5 tiles up all the way to the wall: tooting would splat):
    // no Giant Steps, no way up.
    let mut spiked: Vec<String> = wall(6).lines().map(String::from).collect();
    let row = spiked.len() - 7;
    let mut chars: Vec<char> = spiked[row].chars().collect();
    for ch in chars.iter_mut().take(30) {
        *ch = 'v';
    }
    spiked[row] = chars.into_iter().collect();
    let spiked = mark(&(spiked.join("\n") + "\n"), giant_mark);
    assert!(errs_of(&spiked).iter().any(|e| e.contains("goal")), "a giant wall needs a runway");

    // Lines: say@ must sit on a checkpoint; every checkpoint needs a line.
    let off = ok.replace("say@19,13", "say@18,13");
    assert!(errs_of(&off).iter().any(|e| e.contains("not on a checkpoint")), "{:?}", errs_of(&off));
    let plain = ok.replace("say@19,13: hey", "say: hey");
    assert!(errs_of(&plain).is_empty(), "plain say: still works: {:?}", errs_of(&plain));
    let none = ok.replace("say@19,13: hey\n", "");
    assert!(errs_of(&none).iter().any(|e| e.contains("has no line")));
    // A hint about something the level doesn't have.
    let fly = ok.replace("say@19,13: hey\n", "say@19,13: hey\nhint@5,13 fly: Flies!\n");
    assert!(errs_of(&fly).iter().any(|e| e.contains("teaches fly")), "{:?}", errs_of(&fly));
}

/// The waltz row gate: the numbers, and the validator telling good rows from bad ones.
#[test]
fn validator_knows_waltz_rows() {
    // The numbers (see `dash_through`): a row of n adjacent cans is a danger zone of 16n px.
    // Normal timing leaves 1.5 s to cross it; the waltz 2.5 s (on for the big ONE's beat).
    let row = |n: usize| WaltzRow { row: 11, c0: 20, c1: 20 + n as i32 - 1 };
    for n in [4, 10, 13] {
        assert!(dash_through(n, RUN_SPEED, Mode::Normal), "{n} cans: time enough at normal timing");
    }
    assert!(!dash_through(15, RUN_SPEED, Mode::Normal), "15 cans: 240px take 1.6s > 1.5s");
    assert!(dash_through(18, RUN_SPEED * 1.35, Mode::FiredUp), "fired up outruns 18 (288px in 1.42s)");
    assert!(!dash_through(20, RUN_SPEED * 1.35, Mode::FiredUp), "but not 20 (320px in 1.58s)");
    assert!(dash_through(21, RUN_SPEED * 0.9, Mode::Waltz), "the waltz's 2.5s carries a human 21 cans");
    assert!(!dash_through(22, RUN_SPEED * 0.9, Mode::Waltz));
    assert!(!dash_through(20, RUN_SPEED * 0.65, Mode::GiantSteps));
    assert!(waltz_row_timing(&row(20)).is_empty(), "{:?}", waltz_row_timing(&row(20)));
    assert!(waltz_row_timing(&row(12)).iter().any(|e| e.contains("Normal")));
    assert!(waltz_row_timing(&row(18)).iter().any(|e| e.contains("FiredUp")));
    assert!(waltz_row_timing(&row(25)).iter().any(|e| e.contains("too long")));

    // A tunnel: 20 cans under a grating, a low ceiling (a wall to the sky above it), a runway.
    let level = |cans: usize, ceiling: bool, runway_spikes: bool| {
        let (c0, c1) = (20, 20 + cans - 1);
        let w = 60;
        let mut rows: Vec<Vec<char>> = vec![vec!['.'; w]; 14];
        for r in 0..=7 {
            for c in c0..=c1 {
                rows[r][c] = if ceiling || r < 6 { '#' } else { '.' };
            }
        }
        for c in 0..w {
            rows[10][c] = if (c0..=c1).contains(&c) { '=' } else { '#' };
            rows[11][c] = if (c0..=c1).contains(&c) { 'S' } else { '#' };
            rows[12][c] = '#';
            rows[13][c] = '#';
        }
        rows[9][2] = 'P';
        rows[9][8] = 'C';
        rows[9][w - 2] = 'G';
        if runway_spikes {
            // A spike strip right before the row: no room to jump in threes.
            for c in 15..20 {
                rows[10][c] = '^';
            }
            rows[10][14] = '#';
        }
        let rows: Vec<String> = rows.iter().map(|r| r.iter().collect()).collect();
        run(&level_of("say@8,9: hey\ngate: waltz 15,9 45,9\n", &rows, 9))
    };
    let good = level(20, true, false);
    assert!(good.errs.is_empty(), "{:?}", good.errs);
    assert_eq!(gates(&good), [Gate::WaltzRow]);
    assert!(good.gated_goal, "the only way to the goal is the dash");
    let short = level(12, true, false);
    assert!(short.errs.iter().any(|e| e.contains("run through with Normal")), "{:?}", short.errs);
    let open = level(20, false, false);
    assert!(open.errs.iter().any(|e| e.contains("low ceiling")), "{:?}", open.errs);
    let cramped = level(20, true, true);
    assert!(cramped.errs.iter().any(|e| e.contains("goal")), "no runway, no waltz: {:?}", cramped.errs);
}

/// A flat walkway (row 9) with a spike pit `width` wide sunk into it (spikes in row 10 from
/// col 20): stains land level with the walkway.
#[allow(clippy::needless_range_loop)]
fn pit_level(width: usize, header: &str) -> Level {
    let w = (width + 45).max(60);
    let mut rows: Vec<Vec<char>> = vec![vec!['.'; w]; 14];
    for c in 0..w {
        rows[10][c] = if (20..20 + width).contains(&c) { '^' } else { '#' };
        for r in 11..14 {
            rows[r][c] = '#';
        }
    }
    rows[9][2] = 'P';
    rows[9][8] = 'C';
    rows[9][w - 3] = 'G';
    let rows: Vec<String> = rows.iter().map(|r| r.iter().collect()).collect();
    let mark = if width >= STAIN_PIT_MIN { format!("gate: stain 15,8 {},10\n", 25 + width) } else { String::new() };
    level_of(&format!("say@8,9: hey\n{mark}{header}"), &rows, 9)
}

/// Stain pits: too wide for any mode, crossed on your own splats.
#[test]
fn validator_knows_stain_pits() {
    // Narrow spikes are just jumped (and aren't a pit).
    let narrow = run(&pit_level(7, ""));
    assert!(narrow.errs.is_empty(), "{:?}", narrow.errs);
    assert!(gates(&narrow).is_empty());
    // 15 wide: beyond every mode (fired up included: the nugget line is right there), one
    // splat in the middle makes it two normal jumps.
    let one = run(&pit_level(15, "deaths: 1\n"));
    assert!(one.errs.is_empty(), "{:?}", one.errs);
    assert_eq!(one.gates.iter().map(|g| (g.0, g.3)).collect::<Vec<_>>(), [(Gate::StainPit, 1)]);
    assert!(one.gated_goal);
    assert_eq!(one.deaths, 1);
    // The level must say so.
    let undeclared = run(&pit_level(15, ""));
    assert!(undeclared.errs.iter().any(|e| e.contains("deaths")), "{:?}", undeclared.errs);
    // Wider: more splats.
    let two = run(&pit_level(22, "deaths: 2\n"));
    assert!(two.errs.is_empty(), "{:?}", two.errs);
    assert_eq!(two.gates.iter().map(|g| (g.0, g.3)).collect::<Vec<_>>(), [(Gate::StainPit, 2)]);
    // Too wide for 4 splats.
    let wide = run(&pit_level(50, "deaths: 5\n"));
    assert!(wide.errs.iter().any(|e| e.contains("can't be crossed")), "{:?}", wide.errs);
    // Teaching: a stain hint before the pit is the lesson; one after it is an error.
    let taught = run(&pit_level(15, "deaths: 1\nhint@14,9 stain: Splat in the middle!\n"));
    assert!(taught.errs.is_empty(), "{:?}", taught.errs);
    let lesson = taught.lessons.iter().find(|l| l.topic == Topic::Stain).expect("a stain lesson");
    assert!(lesson.taught(), "{lesson:?}");
    let late = run(&pit_level(15, "deaths: 1\nhint@40,9 stain: Splat in the middle!\n"));
    assert!(late.errs.iter().any(|e| e.contains("must come before")), "{:?}", late.errs);
}

/// A grease chute: drop from the ledge (row 7) onto grease (row 12) that slides into spikes;
/// the way on is a 3-tile jump up to the right, from the grease.
#[allow(clippy::needless_range_loop)]
fn chute_level(header: &str, spikes: bool) -> Level {
    let w = 60;
    let mut rows: Vec<Vec<char>> = vec![vec!['.'; w]; 14];
    for r in 8..14 {
        for c in 0..20 {
            rows[r][c] = '#';
        }
    }
    for c in 20..47 {
        rows[13][c] = '_';
    }
    if spikes {
        rows[12][46] = '^';
    }
    for r in 10..14 {
        for c in 47..w {
            rows[r][c] = '#';
        }
    }
    rows[7][2] = 'P';
    rows[7][8] = 'C';
    rows[9][57] = 'G';
    let rows: Vec<String> = rows.iter().map(|r| r.iter().collect()).collect();
    level_of(&format!("say@8,7: hey\ngate: grip 18,6 50,13\n{header}"), &rows, 7)
}

/// Grease chutes: impassable without grip in every mode, passable with it; three deaths.
#[test]
fn validator_knows_grease_chutes() {
    let chute = run(&chute_level("deaths: 3\nhint@18,7 grease grip: Greasy! Splat a few times!\n", true));
    assert!(chute.errs.is_empty(), "{:?}", chute.errs);
    assert_eq!(gates(&chute), [Gate::GreaseChute]);
    assert!(chute.gated_goal);
    assert_eq!(chute.deaths, 3);
    for t in [Topic::Grease, Topic::Grip] {
        let l = chute.lessons.iter().find(|l| l.topic == t).expect("lesson");
        assert!(l.taught(), "{l:?}");
    }
    // Grease that slides into a plain wall: nowhere to splat, the band never gets nervous.
    let no_spikes = run(&chute_level("deaths: 3\n", false));
    assert!(no_spikes.errs.iter().any(|e| e.contains("must end in spikes")), "{:?}", no_spikes.errs);
}

/// A grid `w` x 14: solid floor rows 10..14, `P` at col 2, checkpoint at col 8, goal at the
/// far right; `edit` carves the rest.
#[allow(clippy::needless_range_loop)]
fn floor_level(w: usize, header: &str, edit: impl Fn(&mut Vec<Vec<char>>)) -> Level {
    let mut rows: Vec<Vec<char>> = vec![vec!['.'; w]; 14];
    for r in 10..14 {
        for c in 0..w {
            rows[r][c] = '#';
        }
    }
    edit(&mut rows);
    rows[9][2] = 'P';
    rows[9][8] = 'C';
    rows[9][w - 3] = 'G';
    let rows: Vec<String> = rows.iter().map(|r| r.iter().collect()).collect();
    level_of(&format!("say@8,9: hey\n{header}"), &rows, 9)
}

/// A bottomless chasm `width` wide from col 30.
fn chasm_level(width: usize, header: &str) -> Level {
    floor_level(30 + width + 20, header, |rows| {
        for r in 10..14 {
            for c in 30..30 + width {
                rows[r][c] = '.';
            }
        }
    })
}

/// Chain-jump chasms: too wide for any mode and for one boost, crossed by a mid-air chain.
#[test]
fn validator_knows_chain_chasms() {
    let mark = |w: usize| format!("gate: chain 28,9 {},9\n", 33 + w);
    for w in [16, 18] {
        let r = run(&chasm_level(w, &mark(w)));
        assert!(r.errs.is_empty(), "{w}-wide chasm: {:?}", r.errs);
        assert_eq!(gates(&r), [Gate::ChainChasm], "{w}");
        assert!(r.gated_goal);
        assert_eq!(r.deaths, 0, "Han falling in isn't Nat's death");
        let lesson = r.lessons.iter().find(|l| l.topic == Topic::Chain).expect("a chain lesson");
        assert!(!lesson.taught(), "no hint yet");
    }
    let taught = run(&chasm_level(16, &format!("{}hint@24,9 chain: Jump, toot, land on my head!\n", mark(16))));
    assert!(taught.errs.is_empty(), "{:?}", taught.errs);
    assert!(taught.lessons.iter().any(|l| l.topic == Topic::Chain && l.taught()));
    // Too wide even for a chain (Han's three toots only keep him up so long).
    let wide = run(&chasm_level(22, &mark(22)));
    assert!(wide.errs.iter().any(|e| e.contains("goal")), "{:?}", wide.errs);
    // Unmarked: an error (Han's overuse limit would apply in it).
    let bare = run(&chasm_level(16, ""));
    assert!(bare.errs.iter().any(|e| e.contains("`gate: chain")), "{:?}", bare.errs);
    // Narrower than a chasm: one boost off Han at the edge does it (a buddy ledge, not a chain).
    let gap = run(&chasm_level(11, "gate: boost 20,9 50,9\n"));
    assert!(!gates(&gap).contains(&Gate::ChainChasm), "{:?}", gates(&gap));
}

/// Shield rows: a can tunnel too long for the waltz; Han goes ahead.
#[test]
fn validator_knows_shield_rows() {
    let level = |cans: usize, ceiling: bool| {
        floor_level(30 + cans + 20, "gate: shield 20,9 70,9\n", |rows| {
            let (c0, c1) = (30, 30 + cans - 1);
            for r in 0..=7 {
                for c in c0..=c1 {
                    rows[r][c] = if ceiling || r < 6 { '#' } else { '.' };
                }
            }
            for c in c0..=c1 {
                rows[10][c] = '=';
                rows[11][c] = 'S';
            }
        })
    };
    for n in [24, 26] {
        let r = run(&level(n, true));
        assert!(r.errs.is_empty(), "{n} cans: {:?}", r.errs);
        assert_eq!(gates(&r), [Gate::ShieldRow]);
        assert!(r.gated_goal);
    }
    let open = run(&level(26, false));
    assert!(open.errs.iter().any(|e| e.contains("low ceiling")), "{:?}", open.errs);
    // No mode dashes 24 cans, the waltz included; 20 is the waltz's.
    assert!(shield_row_timing(&WaltzRow { row: 11, c0: 30, c1: 53 }).is_empty());
    assert!(shield_row_timing(&WaltzRow { row: 11, c0: 30, c1: 49 }).iter().any(|e| e.contains("Waltz")));
}

/// Buddy raft pools: sewage under ceiling spikes, too wide for Nat's own rafts.
#[test]
fn validator_knows_buddy_raft_pools() {
    let level = |x: usize, width: usize, spikes_up: usize| {
        floor_level(x + width + 20, &format!("gate: buddyraft {},9 {},10\n", x - 2, x + width + 1), |rows| {
            for c in x..x + width {
                for r in 10..14 {
                    rows[r][c] = '~';
                }
                let s = 10 - spikes_up;
                rows[s][c] = 'v';
                for r in 0..s {
                    rows[r][c] = '#';
                }
            }
        })
    };
    // Far from the checkpoint: Nat's own 12 s rafts (3-4 of them, a 6 s round trip each) sink
    // before the last walk across. Han's 3-tile rafts bridge it.
    for (w, up) in [(14, 3), (12, 3)] {
        let r = run(&level(60, w, up));
        assert!(r.errs.is_empty(), "{w} wide: {:?}", r.errs);
        assert_eq!(gates(&r), [Gate::BuddyRaft]);
        assert!(r.gated_goal);
        assert_eq!(r.deaths, 0, "Han's splats aren't Nat's");
    }
    // Next to the checkpoint: Nat's own rafts do it (round trips of 2 s).
    let near = run(&level(20, 14, 3));
    assert!(near.errs.iter().any(|e| e.contains("Nat's own rafts bridge it")), "{:?}", near.errs);
    // Spikes higher up leave room for longer hops: fewer rafts, and fired up it's 2.
    let roomy = run(&level(60, 14, 4));
    assert!(roomy.errs.iter().any(|e| e.contains("Nat's own rafts bridge it (FiredUp)")), "{:?}", roomy.errs);
}

