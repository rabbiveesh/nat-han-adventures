//! Free play: every room template validates at bands 1, 5 and 10 over many seeds (start to end
//! reachable, gates, the deaths each needs), generation is deterministic per seed and fast
//! enough to do while playing, and a whole 8-room run plays through headless (Nat teleported
//! room to room) with the adaptive engine hearing about every room.
//!
//! `cargo test --release --test freeplay -- --nocapture` prints the timings.

use std::time::{Duration, Instant};

use bevy::{input::InputPlugin, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use nat_han_adventures::adapt::{AdaptEvent, AssistLevers, RoomRequest, Skill};
use nat_han_adventures::freeplay::course::COURSE_STAND as STAND;
use nat_han_adventures::freeplay::dice::{parse_seed, seed_text};
use nat_han_adventures::freeplay::generate::{Job, RoomPlan, draw, generate, physics, validate};
use nat_han_adventures::freeplay::run::begin;
use nat_han_adventures::freeplay::templates::{TEMPLATES, unlocked_skills};
use nat_han_adventures::freeplay::{FIXED_ROOMS, FreePlayRun, FreePlaySettings, StartFreePlay};
use nat_han_adventures::game::{ActiveLevel, AdaptiveProfile, AssistMode, LevelTile, Player, Pos, PrevPos, stand_pos};
use nat_han_adventures::level::ThingKind;
use nat_han_adventures::state::AppState;

const SEEDS: u32 = 50;
const BANDS: [u8; 3] = [1, 5, 10];

fn plan(template: usize, band: u8, seed: u32, hint: bool) -> RoomPlan {
    let request = RoomRequest { skill: TEMPLATES[template].skill, band, assists: AssistLevers::NONE };
    RoomPlan { template, hint, ..RoomPlan::new(seed, 3, request, 1 + (seed % 5) as u8, false, false, 10) }
}

#[test]
fn every_template_validates_at_every_band() {
    physics();
    let t0 = Instant::now();
    // (template, band) -> (first-try valid, attempts, fallbacks, worst µs, total µs, expected deaths seen)
    let results: Vec<(usize, u8, u32, u32, u32, u64, u64, Vec<u32>)> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..TEMPLATES.len())
            .flat_map(|t| BANDS.map(|b| (t, b)))
            .map(|(t, b)| {
                s.spawn(move || {
                    let (mut first, mut attempts, mut fallbacks, mut worst, mut total) = (0, 0, 0, 0u64, 0u64);
                    let mut deaths = Vec::new();
                    for seed in 0..SEEDS {
                        let room = Job::new(seed, plan(t, b, seed, seed % 2 == 0)).run();
                        first += (room.attempts == 1 && !room.fallback) as u32;
                        attempts += room.attempts;
                        fallbacks += room.fallback as u32;
                        worst = worst.max(room.micros / room.attempts as u64);
                        total += room.micros;
                        // Reachable start to goal, every checkpoint, the room's own checkpoint line.
                        let report = validate(&room.level);
                        assert!(report.errs.is_empty(), "{} band {b} seed {seed}: {:?}", TEMPLATES[t].name, report.errs);
                        if !room.fallback {
                            deaths.push(room.expected_deaths);
                        }
                    }
                    (t, b, first, attempts, fallbacks, worst, total, deaths)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    eprintln!("{} rooms in {:.1?} (threaded)", TEMPLATES.len() * BANDS.len() * SEEDS as usize, t0.elapsed());
    for (t, b, first, attempts, fallbacks, worst, total, deaths) in &results {
        let tpl = &TEMPLATES[*t];
        eprintln!(
            "{:18} band {b:2}: {first}/{SEEDS} first try, {attempts} attempts, {fallbacks} fallbacks, \
             {:.2} ms/attempt mean, {:.2} ms worst, deaths {:?}..={:?}",
            tpl.name,
            *total as f64 / *attempts as f64 / 1000.0,
            *worst as f64 / 1000.0,
            deaths.iter().min(),
            deaths.iter().max(),
        );
        assert_eq!(*fallbacks, 0, "{} band {b} needed the fallback room", tpl.name);
        assert!(*first * 10 >= SEEDS * 8, "{} band {b}: only {first}/{SEEDS} valid on the first try", tpl.name);
        // Expected deaths come from the template: stain pits need splats, chutes the nervous band.
        match tpl.skill {
            Skill::Stains => assert!(deaths.iter().all(|&d| d >= 1), "{}: {deaths:?}", tpl.name),
            Skill::Grease => assert!(deaths.iter().all(|&d| d == 3), "{}: {deaths:?}", tpl.name),
            _ => assert!(deaths.iter().all(|&d| d == 0), "{}: {deaths:?}", tpl.name),
        }
    }
}

/// The waltz row rooms: plain cans on the floor (no grating), deadly while they fire, and the
/// row is a waltz gate the validator finds and passes.
#[test]
fn waltz_rows_are_floor_cans_without_a_grating() {
    use nat_han_adventures::level::Tile;
    use nat_han_adventures::level::validate::Gate;
    let t = TEMPLATES.iter().position(|t| t.skill == Skill::Waltz).unwrap();
    assert_eq!(nat_han_adventures::freeplay::templates::waltz_cans(), [20, 21], "the dashable lengths");
    for b in BANDS {
        for seed in 0..10 {
            let room = Job::new(seed, plan(t, b, seed, false)).run();
            assert!(!room.fallback, "band {b} seed {seed}");
            let l = &room.level;
            let cans: Vec<_> = l.things.iter().filter(|t| t.kind == ThingKind::Spray).collect();
            assert!(cans.len() >= 20, "band {b} seed {seed}: {} cans", cans.len());
            for c in &cans {
                let (col, row) = (c.col as i32, c.row as i32);
                assert!(l.tile(col, row + 1).is_solid(), "can at {col},{row} stands on the floor");
                assert_ne!(l.tile(col, row - 1), Tile::OneWay, "no grating over the can at {col},{row}");
            }
            let report = validate(l);
            assert!(report.errs.is_empty(), "band {b} seed {seed}: {:?}", report.errs);
            assert!(report.gates.iter().any(|g| g.0 == Gate::WaltzRow), "band {b} seed {seed}: {:?}", report.gates);
        }
    }
}

#[test]
fn the_fallback_room_always_validates() {
    for seed in 0..SEEDS {
        let p = nat_han_adventures::freeplay::generate::fallback_plan(&plan(0, 7, seed, false));
        for attempt in 0..3 {
            let l = draw(seed, &p, attempt);
            assert!(validate(&l).errs.is_empty(), "fallback seed {seed} attempt {attempt}");
        }
    }
}

#[test]
fn generation_is_deterministic_per_seed() {
    for t in 0..TEMPLATES.len() {
        for seed in [0, 7, 123_456] {
            let a = generate(seed, plan(t, 6, seed, true));
            let b = generate(seed, plan(t, 6, seed, true));
            assert_eq!(a.level, b.level, "{}", TEMPLATES[t].name);
            assert_eq!(a.attempts, b.attempts);
        }
    }
    // Different seeds, different rooms.
    let a = generate(1, plan(0, 5, 1, false));
    let b = generate(2, plan(0, 5, 2, false));
    assert_ne!(a.level.tiles, b.level.tiles);
    // Whole runs: same seed and profile, same course.
    let mut p1 = nat_han_adventures::adapt::PlayerProfile::new();
    let mut p2 = nat_han_adventures::adapt::PlayerProfile::new();
    let (r1, l1) = begin(424_242, false, &mut p1, 10);
    let (r2, l2) = begin(424_242, false, &mut p2, 10);
    assert_eq!(l1, l2);
    assert_eq!((r1.world, r1.rooms[0].plan.clone()), (r2.world, r2.rooms[0].plan.clone()));
}

#[test]
fn rooms_generate_within_budget() {
    physics();
    let mut times = Vec::new();
    for t in 0..TEMPLATES.len() {
        for b in BANDS {
            for seed in 0..6 {
                let p = plan(t, b, seed, true);
                let s = Instant::now();
                let l = draw(seed, &p, 0);
                let drawn = s.elapsed();
                validate(&l);
                times.push((drawn, s.elapsed()));
            }
        }
    }
    times.sort_by_key(|t| t.1);
    let n = times.len();
    let mean = times.iter().map(|t| t.1).sum::<Duration>() / n as u32;
    let draw_mean = times.iter().map(|t| t.0).sum::<Duration>() / n as u32;
    eprintln!(
        "room generation + validation ({}): {n} rooms, mean {mean:.2?} (drawing {draw_mean:.2?}), median {:.2?}, p95 {:.2?}, max {:.2?}",
        if cfg!(debug_assertions) { "debug" } else { "release" },
        times[n / 2].1,
        times[n * 95 / 100].1,
        times[n - 1].1,
    );
    // One attempt runs off the main thread (natively) or between frames (web); keep it short.
    let budget = if cfg!(debug_assertions) { Duration::from_millis(400) } else { Duration::from_millis(60) };
    assert!(mean < budget, "mean {mean:?} over {budget:?}");
}

#[test]
fn seeds_round_trip() {
    for s in [0, 1, 42, 99_999, 999_999] {
        assert_eq!(parse_seed(&seed_text(s)), Some(s));
    }
    assert_eq!(seed_text(7), "000007");
}

#[test]
fn unlocks_follow_the_story() {
    assert_eq!(unlocked_skills(1), vec![Skill::Precision, Skill::HazardTiming, Skill::MovingPlatforms, Skill::GiantSteps]);
    assert_eq!(unlocked_skills(10).len(), Skill::COUNT);
    // Han's gates share a skill: each is served once its own story level is unlocked.
    assert!(!unlocked_skills(2).contains(&Skill::Buddy));
    assert!(unlocked_skills(3).contains(&Skill::Buddy));
    let request = RoomRequest { skill: Skill::Buddy, band: 5, assists: AssistLevers::NONE };
    let served = |levels: usize| -> Vec<&str> {
        let mut names: Vec<&str> =
            (0..200).map(|seed| RoomPlan::new(seed, 0, request, 1, false, false, levels).template().name).collect();
        names.sort();
        names.dedup();
        names
    };
    assert_eq!(served(3), ["buddy ledge"]);
    assert_eq!(served(5), ["buddy ledge", "buddy raft pool"]);
    assert_eq!(served(10), ["buddy ledge", "buddy raft pool", "chain chasm", "shield row"]);
}

/// A climb is taller than the other rooms: the course is as tall as it, every room sits on its
/// bottom, so the pipes line up and everything in a short room moves down with it, under a
/// ceiling.
#[test]
fn tall_rooms_stitch_on_the_course_floor() {
    use nat_han_adventures::freeplay::course::{COURSE_H, Course};
    let climb = TEMPLATES.iter().position(|t| t.name == "shelf climb").unwrap();
    let flat = draw(1, &plan(0, 1, 1, true), 0);
    let tall = draw(1, &plan(climb, 5, 1, true), 0);
    assert!(tall.height > flat.height);
    let (mut course, mut level) = Course::new(1, 3);
    assert_eq!(level.height, COURSE_H);
    let (a, _) = course.add(&mut level, &flat, false);
    let (b, _) = course.add(&mut level, &tall, false);
    let (c, _) = course.add(&mut level, &flat, true);
    for seam in [b.col0, c.col0] {
        for col in seam - 2..seam + 2 {
            assert!(!level.tile(col as i32, STAND as i32).is_solid(), "the pipes line up at column {col}");
            assert!(level.tile(col as i32, STAND as i32 + 1).is_solid(), "pipe floor at column {col}");
        }
    }
    let dr = COURSE_H - flat.height;
    // Over a short room the course is solid: its ceiling, as at the top of its own grid.
    for col in a.cols().chain(c.cols()) {
        assert!((0..dr).all(|r| level.tile(col as i32, r as i32).is_solid()), "ceiling over column {col}");
    }
    assert_eq!(level.start, (flat.start.0, flat.start.1 + dr));
    assert_eq!(level.goal.1, STAND);
    let say = &level.say_at[0];
    assert_eq!((say.col, say.row), (flat.say_at[0].col + a.col0, flat.say_at[0].row + dr));
    let hint = &level.hints[0];
    assert_eq!((hint.col, hint.row), (tall.hints[0].col + b.col0, tall.hints[0].row));
    let n = |l: &nat_han_adventures::level::Level, cols: std::ops::Range<usize>| {
        let mut v: Vec<_> = l.things.iter().filter(|t| cols.contains(&t.col)).map(|t| (t.col, t.row)).collect();
        v.sort();
        v
    };
    let mut shifted: Vec<_> = n(&flat, 0..flat.width).into_iter().map(|(x, y)| (x + c.col0, y + dr)).collect();
    shifted.sort();
    assert_eq!(n(&level, c.cols()), shifted);
    let mut climbed: Vec<_> = n(&tall, 0..tall.width).into_iter().map(|(x, y)| (x + b.col0, y)).collect();
    climbed.sort();
    assert_eq!(n(&level, b.cols()), climbed);
}

// ─── A headless run ──────────────────────────────────────────────────────────

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin, InputPlugin))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 60.0)))
        .add_plugins(nat_han_adventures::gameplay);
    app.finish();
    app.cleanup();
    app.update();
    // Everything unlocked: every template can come up.
    app.world_mut().resource_mut::<nat_han_adventures::save::Progress>().unlocked = 10;
    app
}

fn state(app: &App) -> AppState {
    *app.world().resource::<State<AppState>>().get()
}

fn teleport(app: &mut App, col: usize) {
    let at = {
        let level = &app.world().resource::<ActiveLevel>().level;
        stand_pos(level, col, STAND)
    };
    let mut q = app.world_mut().query_filtered::<(&mut Pos, &mut PrevPos), With<Player>>();
    for (mut p, mut pp) in q.iter_mut(app.world_mut()) {
        p.0 = at;
        pp.0 = at;
    }
}

fn run(app: &App) -> FreePlayRun {
    app.world().resource::<FreePlayRun>().clone()
}

#[test]
fn a_fixed_run_plays_through_to_the_goal() {
    let mut app = app();
    let seed = 31_337;
    app.world_mut().write_message(StartFreePlay { seed, endless: false });
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(state(&app), AppState::Playing);
    assert_eq!(*app.world().resource::<AssistMode>(), AssistMode::FreePlay);
    assert_eq!(app.world().resource::<FreePlaySettings>().last_seed, Some(seed));

    for k in 0..FIXED_ROOMS as usize {
        // Wait for room k to be generated and streamed in.
        let mut frames = 0;
        while run(&app).course.rooms.len() <= k {
            app.update();
            std::thread::sleep(Duration::from_millis(1));
            frames += 1;
            assert!(frames < 20_000, "room {k} never came");
        }
        for _ in 0..12 {
            app.update();
        }
        let r = run(&app);
        let col = r.course.rooms[k].start_col();
        teleport(&mut app, col);
        for _ in 0..3 {
            app.update();
        }
        let r = run(&app);
        assert_eq!(r.current, Some(k), "entered room {k}");
        assert!(r.room_label().starts_with(&format!("ROOM {}/8", k + 1)));
        // The room's tiles are in (streamed) and its checkpoint is where the course says.
        let level = &app.world().resource::<ActiveLevel>().level;
        let cp = level.checkpoints().nth(r.course.rooms[k].checkpoint).unwrap();
        assert_eq!(cp.col, col);
        let tiles = app.world_mut().query::<&LevelTile>().iter(app.world()).filter(|t| t.col == col).count();
        assert!(tiles > 0, "room {k} is drawn");
    }
    // Walk into the last room's goal.
    let goal = app.world().resource::<ActiveLevel>().level.goal;
    teleport(&mut app, goal.0);
    for _ in 0..5 {
        app.update();
    }
    assert_eq!(state(&app), AppState::LevelComplete);
    let r = run(&app);
    assert!(r.finished);
    assert_eq!(r.cleared, FIXED_ROOMS);
    let started = r.events.iter().filter(|e| matches!(e, AdaptEvent::RoomStarted { .. })).count();
    let finished = r.events.iter().filter(|e| matches!(e, AdaptEvent::RoomFinished(_))).count();
    assert_eq!((started, finished), (FIXED_ROOMS as usize, FIXED_ROOMS as usize));
    let profile = &app.world().resource::<AdaptiveProfile>().0;
    assert_eq!(profile.rooms_played, FIXED_ROOMS);
    assert!(!profile.calibrating(), "the first rooms placed the player");
    // Room checkpoints count up along the course; nuggets are all counted.
    let level = &app.world().resource::<ActiveLevel>().level;
    let cols: Vec<usize> = level.checkpoints().map(|c| c.col).collect();
    assert!(cols.windows(2).all(|w| w[0] < w[1]));
    let nuggets = level.things.iter().filter(|t| t.kind == ThingKind::Nugget).count() as u32;
    assert_eq!(app.world().resource::<nat_han_adventures::game::LevelRun>().nuggets_total, nuggets);

    // Back to the setup screen: story mode gets its level source and assists back.
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::FreePlaySetup);
    app.update();
    app.update();
    assert!(app.world().get_resource::<FreePlayRun>().is_none());
    assert_eq!(*app.world().resource::<AssistMode>(), AssistMode::Story);
}

#[test]
fn an_endless_run_unloads_rooms_behind() {
    let mut app = app();
    app.world_mut().write_message(StartFreePlay { seed: 5, endless: true });
    for _ in 0..3 {
        app.update();
    }
    for k in 0..6 {
        while run(&app).course.rooms.len() <= k {
            app.update();
            std::thread::sleep(Duration::from_millis(1));
        }
        for _ in 0..12 {
            app.update();
        }
        let col = run(&app).course.rooms[k].start_col();
        teleport(&mut app, col);
        for _ in 0..3 {
            app.update();
        }
    }
    let r = run(&app);
    assert_eq!(r.current, Some(5));
    assert_eq!(r.room_label(), "ROOM 6");
    // Rooms 0..=2 are gone (sealed off), 3.. are drawn.
    let first_kept = r.course.rooms[3].col0;
    let min_col = app.world_mut().query::<&LevelTile>().iter(app.world()).map(|t| t.col).min().unwrap();
    assert_eq!(min_col, first_kept);
    let level = &app.world().resource::<ActiveLevel>().level;
    assert!(level.tile(first_kept as i32, STAND as i32).is_solid(), "sealed");
}
