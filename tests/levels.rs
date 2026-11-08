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
//! wall (Giant Steps), `R` long gap (fired up), `Z` glide launch (waltz), `Y` grease chute (grip), `K`
//! stain pit (splats), and Han's: `M` mega wall (Giant Steps + boost), `N` chain chasm,
//! `U` buddy raft pool.

use std::time::Instant;

use nat_han_adventures::level::validate::*;
use nat_han_adventures::level::*;

/// Name, world and the gates the level must use.
const PLAN: [(&str, u8, &[Gate]); LEVEL_COUNT] = [
    ("Bathroom Floor", 1, &[Gate::GiantWall]),
    ("The Bowl", 1, &[]),
    ("U-Bend", 2, &[Gate::LongGap, Gate::StainPit, Gate::MegaWall]),
    ("Pipe Maze", 2, &[Gate::GiantWall]),
    ("Main Sewer", 3, &[Gate::LongGap, Gate::StainPit, Gate::BuddyRaft]),
    ("Rat Kingdom", 3, &[Gate::GiantWall, Gate::GreaseChute, Gate::MegaWall]),
    ("Septic Tank", 4, &[Gate::GlideLaunch]),
    ("Porta-Potty Festival", 4, &[Gate::GiantWall, Gate::GlideLaunch, Gate::StainPit, Gate::ChainChasm]),
    ("Treatment Plant", 5, &[Gate::LongGap, Gate::GlideLaunch, Gate::GreaseChute, Gate::MegaWall, Gate::ClingShaft, Gate::BuddyRaft]),
    ("The Golden Throne", 5, &[Gate::GiantWall, Gate::Trapeze, Gate::ChainChasm]),
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
            None if !Topic::GADGETS.contains(&topic) => failures.push(format!("no level has {} at all", topic.word())),
            _ => {}
        }
    }
    assert!(
        failures.is_empty(),
        "level problems (LEVEL_DUMP=1 LEVEL_ONLY=N cargo test --test levels -- --nocapture to see a map):\n{}",
        failures.join("\n")
    );
}

/// The laughing band's phrases that take reach away (`game::groove`: melting jumps, alien
/// gravity) never close a way through: every goal, checkpoint and nugget stays reachable with
/// each in force all along ([`Physics::laughing`]). (Gates may read differently: a jump that
/// can't be made for a phrase can look like one.)
#[test]
fn laughing_band_never_closes_a_level() {
    let levels: Vec<Level> = LEVEL_SOURCES.iter().map(|s| Level::parse(s).unwrap()).collect();
    let mut failures = Vec::new();
    for (nudge, phys) in Physics::laughing() {
        let reports: Vec<Report> = std::thread::scope(|scope| {
            let phys = &phys;
            let handles: Vec<_> =
                levels.iter().map(|l| scope.spawn(move || check_with(l, &Options::default(), phys))).collect();
            handles.into_iter().map(|h| h.join().expect("validator panicked")).collect()
        });
        for (i, (l, r)) in levels.iter().zip(&reports).enumerate() {
            for e in r.errs.iter().filter(|e| e.contains("unreachable")) {
                failures.push(format!("{nudge}: levels/{:02}.txt ({}): {e}", i + 1, l.name));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
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

/// `l` with `n` more rows of sky on top (everything moved down with the grid): headroom for
/// boosts and chains under the ceiling at the top of the screen.
fn with_sky(mut l: Level, n: usize) -> Level {
    let mut tiles = vec![Tile::Empty; n * l.width];
    tiles.extend(&l.tiles);
    l.tiles = tiles;
    l.height += n;
    l.start.1 += n;
    l.goal.1 += n;
    for t in &mut l.things {
        t.row += n;
    }
    for s in l.hints.iter_mut().chain(&mut l.say_at) {
        s.row += n;
    }
    for p in &mut l.platforms {
        p.row += n;
    }
    for g in &mut l.gates {
        g.r0 += n as i32;
        g.r1 += n as i32;
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
    // ...but a mega wall if he comes along: Giant Steps and his mega-boost (it must be
    // marked: unmarked, his plain boost climbs 9 from outside any band zone).
    let mega_mark = "gate: mega 20,1 41,13";
    let mega = check_with_nuggets(&mark(&wall(9), mega_mark), &[]);
    assert!(mega.errs.is_empty(), "the mega-boost climbs 9: {:?}", mega.errs);
    assert_eq!(gates(&mega), [Gate::MegaWall]);
    assert!(errs_of(&wall(9)).iter().any(|e| e.contains("`gate: mega")), "unmarked: {:?}", errs_of(&wall(9)));
    // 11 tiles: too tall for the plain boost, a mega wall.
    assert!(errs_of(&wall(11)).iter().any(|e| e.contains("goal")), "11 tiles is too tall for the plain boost");
    let tall = check_with_nuggets(&mark(&wall(11), mega_mark), &[]);
    assert!(tall.errs.is_empty(), "the mega-boost climbs 11: {:?}", tall.errs);
    assert_eq!(gates(&tall), [Gate::MegaWall]);
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
    // Han barks (1-3 words, the intro aside), and every hint names what its card teaches.
    let chatty = ok.replace("say@19,13: hey", "say@19,13: hey there you lot");
    assert!(errs_of(&chatty).iter().any(|e| e.contains("barks")), "{:?}", errs_of(&chatty));
    let mute = ok.replace("say@19,13: hey\n", "say@19,13: hey\nhint@5,13: Look!\n");
    assert!(errs_of(&mute).iter().any(|e| e.contains("names no topic")), "{:?}", errs_of(&mute));
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
    let taught = run(&pit_level(15, "deaths: 1\nhint@14,9 stain: Splat time!\n"));
    assert!(taught.errs.is_empty(), "{:?}", taught.errs);
    let lesson = taught.lessons.iter().find(|l| l.topic == Topic::Stain).expect("a stain lesson");
    assert!(lesson.taught(), "{lesson:?}");
    let late = run(&pit_level(15, "deaths: 1\nhint@40,9 stain: Splat time!\n"));
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
    let chute = run(&chute_level("deaths: 3\nhint@18,7 grease grip: Grease! Careful!\n", true));
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

/// A glide launch: a toilet roll (8 tiles in 4.5 s) sliding out over a bottomless pit from
/// col 20, `gap` tiles of pit past its path; `spiky`: spikes on the floor before it (no
/// runway to summon the waltz on).
fn glide_level(gap: usize, spiky: bool) -> Level {
    let land = 20 + 2 + 8 + gap;
    let header = format!("1: dx=8 dy=0 period=4.5\ngate: waltz 19,8 {},9\n", land + 1);
    floor_level(land + 14, &header, |rows| {
        for r in 10..14 {
            for c in 20..land {
                rows[r][c] = '.';
            }
        }
        rows[10][20] = '1';
        rows[10][21] = '1';
        if spiky {
            for c in 9..20 {
                rows[10][c] = '^';
            }
        }
    })
}

/// Glide launches: a gap past a platform's path too wide for any other mode (the platform at
/// its everyday speed, at most), crossed off the platform gliding on ONE in the waltz.
#[test]
fn validator_knows_glide_launches() {
    let good = run(&glide_level(17, false));
    assert!(good.errs.is_empty(), "{:?}", good.errs);
    assert_eq!(gates(&good), [Gate::GlideLaunch]);
    assert!(good.gated_goal, "the only way across is the launch");
    let narrow = run(&glide_level(14, false));
    assert!(narrow.errs.iter().any(|e| e.contains("passable with FiredUp")), "{:?}", narrow.errs);
    let wide = run(&glide_level(21, false));
    assert!(wide.errs.iter().any(|e| e.contains("goal")), "too far even gliding: {:?}", wide.errs);
    let cramped = run(&glide_level(17, true));
    assert!(cramped.errs.iter().any(|e| e.contains("goal")), "no runway, no waltz: {:?}", cramped.errs);
}

/// A platform's speed carries the jump off it: a gap a jump from the ledge can't clear is
/// crossed off the platform sliding out over it (a human keeps [`HUMAN_CARRY`] of its speed).
#[test]
fn validator_knows_platform_carry() {
    use nat_han_adventures::game::tuning::LAUNCH_MAX;
    let map_of = |l: &Level| (Map::new(l).carry((22, 9)), Map::new(l).carry((5, 9)));
    let l = glide_level(17, false);
    let (on, off) = map_of(&l);
    let top = std::f32::consts::PI * 8.0 * 16.0 / 4.5;
    assert!(on > 0.5 * top && on <= top + 0.01, "the platform's speed over its path: {on} of {top}");
    assert_eq!(off, 0.0, "none on the ground");
    assert!(top < LAUNCH_MAX);
}

/// A bottomless chasm `width` wide from col 30.
/// A fired-up chasm: `width` bottomless columns from col 30, a nugget line before it (to fire
/// the band up).
fn chasm_level(width: usize, header: &str) -> Level {
    floor_level(30 + width + 20, header, |rows| {
        for r in 10..14 {
            for c in 30..30 + width {
                rows[r][c] = '.';
            }
        }
        for c in [14, 17, 20, 23] {
            rows[9][c] = 'o';
        }
    })
}

/// Fired-up chasms: too wide for any mode and for a plunger boost, crossed by the fired-up
/// band's running throw off Han at the edge.
#[test]
fn validator_knows_fired_up_chasms() {
    const SKY: usize = 10;
    let mark = |w: usize| format!("gate: chain 25,9 {},9\n", 33 + w);
    for w in [15, 18] {
        let r = run(&with_sky(chasm_level(w, &mark(w)), SKY));
        assert!(r.errs.is_empty(), "{w}-wide chasm: {:?}", r.errs);
        assert_eq!(gates(&r), [Gate::ChainChasm], "{w}");
        assert!(r.gated_goal);
        assert_eq!(r.deaths, 0, "Han falling in isn't Nat's death");
        let lesson = r.lessons.iter().find(|l| l.topic == Topic::Chain).expect("a chasm lesson");
        assert!(!lesson.taught(), "no hint yet");
    }
    let taught = run(&with_sky(chasm_level(16, &format!("{}hint@24,9 chain: Hop on!\n", mark(16))), SKY));
    assert!(taught.errs.is_empty(), "{:?}", taught.errs);
    assert!(taught.lessons.iter().any(|l| l.topic == Topic::Chain && l.taught()));
    // Too wide even for a throw.
    let wide = run(&with_sky(chasm_level(21, &mark(21)), SKY));
    assert!(wide.errs.iter().any(|e| e.contains("goal")), "{:?}", wide.errs);
    // Unmarked: an error (Han's overuse limit would apply in it).
    let bare = run(&with_sky(chasm_level(16, ""), SKY));
    assert!(bare.errs.iter().any(|e| e.contains("`gate: chain")), "{:?}", bare.errs);
    // A checkpoint between the nugget line and the edge: a splat there loses the nuggets, so
    // the band may not be fired up at the edge, and nothing else crosses.
    let cp = floor_level(66, &format!("{}say@26,9: hi\n", mark(16)), |rows| {
        for r in 10..14 {
            for c in 30..46 {
                rows[r][c] = '.';
            }
        }
        for c in [14, 17, 20, 23] {
            rows[9][c] = 'o';
        }
        rows[9][26] = 'C';
    });
    let cp = with_sky(cp, SKY);
    let cp = run(&cp);
    assert!(cp.errs.iter().any(|e| e.contains("goal")), "{:?} {:?}", cp.errs, gates(&cp));
    // Narrower than a chasm: a long gap (the fired-up band alone).
    let gap = run(&with_sky(chasm_level(11, "gate: gap 20,9 50,9\n"), SKY));
    assert!(!gates(&gap).contains(&Gate::ChainChasm), "{:?}", gates(&gap));
}

/// Han's boost in a band zone, and chains of boosts: an 18-wide stain pit with a ledge 9 tiles
/// over it that no jump (Giant Steps included) reaches. At full strength one boost off Han at
/// the edge doesn't cross the pit, but it reaches the ledge, and a second one off Han standing
/// there crosses: the chain proof catches it. Han's real band-zone boost (the weak one) reaches
/// neither, so the level is fine.
#[test]
fn validator_proves_band_zone_boost_chains() {
    let mut l = pit_level(18, "deaths: 2\n");
    for c in 27..31 {
        l.tiles[l.width + c] = Tile::Solid;
    }
    // The mark reaches up over the ledge: the zone covers the air above it (Han may climb up
    // there, and catch a high-flying Nat jumping off it).
    l.gates[0].r0 = 0;
    l.gates[0].c1 = 47;
    // Headroom over the ledge for the second boost (the top of the screen is a ceiling).
    let l = with_sky(l, 6);
    let r = run(&l);
    assert!(r.errs.is_empty(), "the weak boost doesn't open the pit: {:?}", r.errs);
    assert_eq!(gates(&r), [Gate::StainPit]);
    let full = Physics::new().with_weak_boost(nat_han_adventures::game::BOOST_SPEED);
    let errs = check_with(&l, &Options::default(), &full).errs;
    assert!(
        errs.iter().any(|e| e.contains("stain pit") && e.contains("chain of boosts")),
        "a full boost to the ledge, then another: {errs:?}"
    );
    // (The ledge is what makes it a chain: without it, one full boost doesn't do it.)
    let mut bare = with_sky(pit_level(18, "deaths: 2\n"), 6);
    bare.things = l.things.clone();
    bare.gates = l.gates.clone();
    let one = check_with(&bare, &Options::default(), &full).errs;
    assert!(one.is_empty(), "{one:?}");
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


#[test]
fn validator_and_game_share_the_ceiling() {
    // The top of the screen is a ceiling for the validator as for the game (`Level::tile`), so
    // a wall reaching row 0 is a wall all the way up: nobody can walk on its roof, unseen.
    for src in LEVEL_SOURCES {
        let level = Level::parse(src).unwrap();
        let map = Map::new(&level);
        for c in 0..level.width as i32 {
            assert!(level.tile(c, -1).is_solid() && map.is_solid(c, -1), "{}: open sky over col {c}", level.name);
            assert!(!map.standable((c, -1)), "{}: a roof over col {c}", level.name);
        }
    }
}
