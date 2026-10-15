//! The gadget tiles, headless: bricks (`%`), crumbling platforms (`-`), trampolines (`T`),
//! crushers (`X` `<` `>`, beat crushers too), conveyor belts (`{` `}`), fans (`*`), bumpers
//! (`(` `)`) and beat blocks (`[` `]`) in the real game (Nat, Han on Side B), and in the
//! validator.

use std::time::Duration;

use bevy::{ecs::system::RunSystemOnce, input::InputPlugin, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use leafwing_input_manager::prelude::*;
use nat_han_adventures::{
    audio::AudioOutput,
    events::{DeathCause, PlayerDied},
    game::{
        ActiveLevel, BeatClock, Groove, Han, Player, Pos,
        crushers::CrusherBlock,
        sideb::{self, Phase, SideB, SideBLevel},
        tuning,
    },
    level::{
        BELT_SPEED, CRUMBLE_BACK, CRUMBLE_DELAY, CRUSH_BEAT_REST, CrushDir, CrushTime, Crusher, FAN_REACH, Level, Levels, TILE, Tile,
        Topic, validate,
    },
    state::AppState,
};

const DT: f64 = 1.0 / 60.0;
const JUMP: KeyCode = KeyCode::KeyZ;
const RIGHT: KeyCode = KeyCode::ArrowRight;

#[derive(Resource, Default)]
struct Deaths(Vec<DeathCause>);

fn app(level: &str) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin, InputPlugin))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(DT)))
        .add_plugins(nat_han_adventures::gameplay)
        .insert_resource(Levels(vec![Level::parse(level).expect("test level parses")]))
        .init_resource::<Deaths>()
        .add_systems(Last, |mut d: ResMut<Deaths>, mut r: MessageReader<PlayerDied>| d.0.extend(r.read().map(|m| m.cause)));
    app.finish();
    app.cleanup();
    app.update();
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::Playing);
    app.update();
    app.update();
    app
}

fn step(app: &mut App, secs: f32) {
    for _ in 0..(secs as f64 / DT).round() as usize {
        app.update();
    }
}

fn nat(app: &mut App) -> Vec2 {
    app.world_mut().query_filtered::<&Pos, With<Player>>().single(app.world()).unwrap().0
}

fn deaths(app: &App) -> Vec<DeathCause> {
    app.world().resource::<Deaths>().0.clone()
}

fn tile(app: &App, c: i32, r: i32) -> Tile {
    app.world().resource::<ActiveLevel>().level.tile(c, r)
}

/// Feet height (px above the level's bottom) of Nat standing on a floor whose top is `floor`.
fn standing(floor: f32) -> f32 {
    floor + tuning::PLAYER_SIZE.1 / 2.0
}

const SPRING: &str = "name: Spring
---
........................
........................
........................
........................
........................
........................
........................
........................
........................
........................
..P....................G
#####T##################
";

#[test]
fn a_trampoline_springs_nat_way_up_and_gives_the_toot_back() {
    let mut app = app(SPRING);
    step(&mut app, 0.3);
    let floor = standing(TILE);
    assert_eq!(nat(&mut app).y, floor);
    RIGHT.press(app.world_mut());
    let mut top = 0.0f32;
    for _ in 0..30 {
        step(&mut app, 1.0 / 60.0);
        top = top.max(nat(&mut app).y);
        if top > floor + 2.0 * TILE {
            break;
        }
    }
    RIGHT.release(app.world_mut());
    for _ in 0..60 {
        step(&mut app, 1.0 / 60.0);
        top = top.max(nat(&mut app).y);
    }
    let rise = (top - floor) / TILE;
    assert!(rise > 5.8, "a trampoline springs ~6.5 tiles, got {rise:.2}");
    assert!(deaths(&app).is_empty());
}

const BRICK: &str = "name: Brick
---
........................
........................
........................
....%...................
........................
........................
....P..................G
########################
";

#[test]
fn a_jump_smashes_a_brick_and_carries_on_up() {
    let mut app = app(BRICK);
    step(&mut app, 0.3);
    assert_eq!(tile(&app, 4, 3), Tile::Brick);
    JUMP.press(app.world_mut());
    let mut top = 0.0f32;
    for _ in 0..40 {
        step(&mut app, 1.0 / 60.0);
        top = top.max(nat(&mut app).y);
    }
    JUMP.release(app.world_mut());
    assert_eq!(tile(&app, 4, 3), Tile::Empty, "smashed");
    // The brick's bottom is 4 tiles up: Nat's head went on into where it was.
    let head = top + tuning::PLAYER_SIZE.1 / 2.0;
    assert!(head > 4.5 * TILE, "carried on up through it (head at {:.2} tiles)", head / TILE);
    step(&mut app, 1.0);
    assert!(deaths(&app).is_empty());
}

const CRUMBLE: &str = "name: Crumble
---
........................
........................
....P...................
...---.................G
........................
........................
########################
";

#[test]
fn a_crumbling_platform_falls_under_nat_and_comes_back() {
    let mut app = app(CRUMBLE);
    step(&mut app, 0.1);
    assert_eq!(tile(&app, 4, 3), Tile::Crumble);
    step(&mut app, CRUMBLE_DELAY + 0.1);
    assert_eq!(tile(&app, 4, 3), Tile::Empty, "fell under him");
    step(&mut app, 0.6);
    let floor = standing(TILE);
    assert_eq!(nat(&mut app).y, floor, "down on the floor");
    // It comes back once nobody's in its way.
    step(&mut app, CRUMBLE_BACK);
    assert_eq!(tile(&app, 4, 3), Tile::Crumble);
    assert_eq!(tile(&app, 3, 3), Tile::Crumble, "never stood on");
}

const UNDER_CRUSHER: &str = "name: Squash
---
........................
........................
....X...................
........................
........................
....P..................G
########################
";

#[test]
fn a_crusher_squashes_nat_standing_under_it() {
    let mut app = app(UNDER_CRUSHER);
    let def = app.world().resource::<ActiveLevel>().level.crushers[0];
    assert_eq!((def.dir, def.travel), (CrushDir::Down, 3));
    step(&mut app, Crusher::period(def.travel));
    assert_eq!(deaths(&app).first(), Some(&DeathCause::Crusher));
}

const ON_CRUSHER: &str = "name: Ride
---
........................
....P...................
....X...................
........................
........................
.......................G
########################
";

#[test]
fn nat_rides_a_crusher_down_and_back_up() {
    let mut app = app(ON_CRUSHER);
    let def = app.world().resource::<ActiveLevel>().level.crushers[0];
    let mut low = f32::INFINITY;
    for _ in 0..(Crusher::period(def.travel) * 60.0) as usize {
        step(&mut app, 1.0 / 60.0);
        let p = nat(&mut app);
        let block = app.world_mut().query::<(&CrusherBlock, &Pos)>().single(app.world()).unwrap().1 .0;
        let top = block.y + TILE / 2.0;
        assert!((p.y - standing(top)).abs() < 1.0, "riding on top ({} vs {})", p.y, standing(top));
        low = low.min(p.y);
    }
    assert!(low < standing(4.0 * TILE), "went down with it");
    assert!(deaths(&app).is_empty());
}

/// A squeeze too low to stand under: a sideways crusher pins Nat to the wall.
const SIDEWAYS: &str = "name: Sideways
---
........................
........................
........................
........................
..>>.......P..#........G
########################
";

#[test]
fn a_sideways_crusher_squashes_nat_against_a_wall() {
    let mut app = app(SIDEWAYS);
    let def = app.world().resource::<ActiveLevel>().level.crushers[0];
    assert_eq!((def.dir, def.travel, def.w), (CrushDir::Right, 10, 2));
    step(&mut app, Crusher::period(def.travel));
    assert_eq!(deaths(&app).first(), Some(&DeathCause::Crusher));
}

#[test]
fn crushers_parse_as_rectangles() {
    let l = Level::parse("name: T\ncrusher@6,1: phase=0.5\n---\n.P....XX..<\n......XX...\n...........\n#.........G\n###########\n").unwrap();
    assert_eq!(l.crushers.len(), 2);
    let x = l.crushers.iter().find(|c| c.dir == CrushDir::Down).unwrap();
    assert_eq!((x.col, x.row, x.w, x.h, x.travel, x.phase), (6, 0, 2, 2, 2, 0.5));
    let left = l.crushers.iter().find(|c| c.dir == CrushDir::Left).unwrap();
    assert_eq!((left.col, left.travel), (10, 2), "stops at the other crusher");
    assert!(Level::parse("name: T\n---\nPXX..G\n.X....\n######\n").is_err(), "not a rectangle");
    assert!(Level::parse("name: T\n---\nP....G\n.X....\n######\n").is_err(), "no room to slam");
}

// --- The validator.

fn reaches_goal(src: &str) -> bool {
    let level = Level::parse(src).expect("parses");
    validate::reachable(&level, (level.goal.0 as i32, level.goal.1 as i32))
}

/// A lone trampoline in a pit, a ledge high up and far off: out of reach of anything else (Han
/// can't stand in the pit to boost Nat).
const SPRING_PILLAR: &str = "name: Spring pillar
---
....................
....................
..............G.....
.............####...
....................
....................
....................
....................
....................
....................
.P..................
####....T...........
";

#[test]
fn the_validator_springs_off_trampolines() {
    assert!(reaches_goal(SPRING_PILLAR), "up to the ledge off the trampoline");
    assert!(!reaches_goal(&SPRING_PILLAR.replace('T', ".")), "no way up without it");
}

const BRICK_CEILING: &str = "name: Brick ceiling
---
................
................
............G...
########%%######
................
................
.P..............
################
";

#[test]
fn the_validator_smashes_bricks_from_below() {
    assert!(reaches_goal(BRICK_CEILING), "up through the brick");
    assert!(!reaches_goal(&BRICK_CEILING.replace('%', "#")), "no brick, no way up");
}

const CRUSHER_HALL: &str = "name: Crusher hall
---
################
.......X........
................
................
.P............G.
################
";

#[test]
fn the_validator_times_small_crushers_and_not_big_ones() {
    assert!(reaches_goal(CRUSHER_HALL), "under a one-tile crusher, timed");
    assert!(!reaches_goal(&CRUSHER_HALL.replace(".X..", "XXXX")), "a four-tile one is too much to time");
}

#[test]
fn the_validator_does_not_linger_on_crumbles() {
    // A runway of crumbling cardboard is no place to toot five times.
    let level = Level::parse("name: T\n---\n........\n.P......G\n--------#\n").unwrap();
    let map = validate::Map::new(&level);
    assert!(map.standable((3, 1)));
    assert!(!map.calm((3, 1)));
}

// --- Han on Side B.

fn side_b(src: &str) -> App {
    let level = SideBLevel::parse(src).expect("parses");
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        StatesPlugin,
        InputPlugin,
        nat_han_adventures::gameplay,
        nat_han_adventures::audio::plugin,
    ))
    .insert_resource(AudioOutput::Headless)
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(DT)));
    app.update();
    app.world_mut().run_system_once(move |mut c: Commands| sideb::start_level(&mut c, 0, level.clone())).unwrap();
    app.update();
    app.update();
    app
}

fn han(app: &mut App) -> Vec2 {
    app.world_mut().query_filtered::<&Pos, With<Han>>().single(app.world()).unwrap().0
}

/// Han's way: under three crushers out of step, up a wall off a trampoline, up through the
/// bricks over the run-up to a high ledge (a plain ceiling there would bonk him short of it).
const HAN_GADGETS: &str = "name: Han's gadgets
crusher@12,8: phase=0.3
crusher@18,8: phase=0.6
---
........................................
........................................
.....................................G..
..................................######
......................#######%%%%%######
..................................######
..........................##############
..........................##############
......X.....X.....X.......##############
..........................##############
..........................##############
.P........................##############
#########################T##############
";

/// Run a Side B level for up to `secs`: when Han cleared it (or how he died), and the app.
fn han_clears(src: &str, secs: f64) -> (Result<f64, String>, App) {
    let mut app = side_b(src);
    for i in 0..(secs / DT) as usize {
        app.update();
        let phase = app.world().resource::<SideB>().phase;
        if std::env::var("GADGETS_TRACE").is_ok() && i % 15 == 0 {
            let p = han(&mut app);
            eprintln!("t={:.2} han=({:.2},{:.2}) {phase:?}", i as f64 * DT, p.x / TILE, p.y / TILE);
        }
        match phase {
            Phase::Cleared { .. } => return (Ok(i as f64 * DT), app),
            Phase::Rewind { .. } => {
                let fate = format!("died: {:?}", app.world().resource::<SideB>().fate);
                return (Err(fate), app);
            }
            Phase::Playing => {}
        }
    }
    (Err("ran out of time".into()), app)
}

#[test]
fn han_times_crushers_springs_and_smashes_his_way_to_the_goal() {
    let (cleared, mut app) = han_clears(HAN_GADGETS, 40.0);
    assert!(cleared.is_ok(), "Han never got there: {cleared:?} at {:?}", han(&mut app) / TILE);
    let bricks = app.world().resource::<ActiveLevel>().level.tiles.iter().filter(|&&t| t == Tile::Brick).count();
    assert!(bricks < 5, "he smashed his way up");
    // Plain tiles instead of the bricks: he bonks short of the ledge.
    let (cleared, _) = han_clears(&HAN_GADGETS.replace("%%%%%", "#####"), 20.0);
    assert!(cleared.is_err(), "a plain ceiling stops him");
}

/// `levels/playground/gadgets.txt`: every gadget in a row, for trying them out.
#[test]
fn the_gadget_playground_is_beatable() {
    let src = include_str!("../levels/playground/gadgets.txt");
    let level = Level::parse(src).unwrap();
    let report = validate::check(&level, &validate::Options { dump: false });
    // A full check: only the hints a campaign level owes are missing.
    let errs: Vec<_> = report.errs.iter().filter(|e| !e.contains("hint")).collect();
    assert!(errs.is_empty(), "{errs:#?}");
    let topics: Vec<_> = report.lessons.iter().map(|l| l.topic).collect();
    for t in Topic::GADGETS {
        assert!(topics.contains(&t), "{t:?} shows up in the playground: {topics:?}");
    }
}


// ─── Conveyor belts, fans, bumpers, beat blocks, beat crushers ──────────────

const BELT: &str = "name: Belt
---
................................
................................
................................
................................
..P.............................
#}}}}}}}}}}}}}}}}}}}}}}}}}}}}}G#
";

#[test]
fn a_belt_carries_nat_and_a_jump_off_it_keeps_its_speed() {
    let mut app = app(BELT);
    step(&mut app, 0.2);
    let x0 = nat(&mut app).x;
    step(&mut app, 1.0);
    let moved = nat(&mut app).x - x0;
    assert!((moved - BELT_SPEED).abs() < 3.0, "standing still, carried {moved:.1} px in 1 s");
    // A jump straight up drifts on with the belt.
    let x1 = nat(&mut app).x;
    JUMP.press(app.world_mut());
    step(&mut app, 0.2);
    JUMP.release(app.world_mut());
    let air = nat(&mut app).x - x1;
    assert!(air > BELT_SPEED * 0.2 - 2.0, "kept the belt's speed in the air ({air:.1} px in 0.2 s)");
}

const FAN: &str = "name: Fan
---
..............
..............
..............
..............
..............
..............
..............
..............
..............
....P........G
....*.........
##############
";

#[test]
fn a_fan_floats_nat_up_its_column_and_holds_him_there() {
    let mut app = app(FAN);
    let floor = 2.0 * TILE;
    let mut top = 0.0f32;
    step(&mut app, 0.5);
    for _ in 0..180 {
        step(&mut app, 1.0 / 60.0);
        top = top.max(nat(&mut app).y);
    }
    let rise = (top - floor) / TILE;
    assert!(rise > FAN_REACH as f32 - 1.5 && rise < FAN_REACH as f32 + 1.5, "floated up ~{FAN_REACH} tiles, got {rise:.2}");
    let y = nat(&mut app).y;
    assert!(y > floor + 3.0 * TILE, "still up there after 3.5 s (at {:.2} tiles)", (y - floor) / TILE);
    assert!(deaths(&app).is_empty());
}

const BUMPER: &str = "name: Bumper
---
....................
....................
....................
....................
....................
..G.................
..........P..(......
####################
";

#[test]
fn a_bumper_bats_nat_back_and_up() {
    let mut app = app(BUMPER);
    step(&mut app, 0.2);
    let start = nat(&mut app);
    RIGHT.press(app.world_mut());
    let mut top = 0.0f32;
    let mut left_most = f32::MAX;
    for _ in 0..30 {
        step(&mut app, 1.0 / 60.0);
        top = top.max(nat(&mut app).y);
    }
    RIGHT.release(app.world_mut());
    for _ in 0..40 {
        step(&mut app, 1.0 / 60.0);
        top = top.max(nat(&mut app).y);
        left_most = left_most.min(nat(&mut app).x);
    }
    assert!(top > start.y + TILE, "batted up ({:.2} tiles)", (top - start.y) / TILE);
    assert!(left_most < start.x - TILE, "batted back left (to {:.2}, from {:.2})", left_most / TILE, start.x / TILE);
}

const BEAT: &str = "name: Beat
---
..............
..............
..P...........
.[[]].......G.
..............
##############
";

fn bar(app: &mut App, bar: u32) {
    app.world_mut().resource_mut::<Groove>().clock = BeatClock::at(bar as f64 * 4.0 + 0.5, 0.5, 4);
}

#[test]
fn beat_blocks_swap_on_the_bar_and_wait_for_nat_to_get_clear() {
    let mut app = app(BEAT);
    bar(&mut app, 0);
    step(&mut app, 0.1);
    assert_eq!((tile(&app, 1, 3), tile(&app, 3, 3)), (Tile::BeatRed, Tile::BeatBlueOff), "bar 0: red's there, blue's gone");
    // Nat stands on a red one.
    step(&mut app, 0.4);
    assert_eq!(nat(&mut app).y, standing(3.0 * TILE));
    bar(&mut app, 1);
    step(&mut app, 0.05);
    assert_eq!((tile(&app, 1, 3), tile(&app, 3, 3)), (Tile::BeatRedOff, Tile::BeatBlue), "bar 1: swapped");
    // He falls through where the red one was, down to the floor...
    step(&mut app, 0.6);
    assert_eq!(nat(&mut app).y, standing(TILE));
    // ...and a red one coming back waits until he's out of it: he stands in row 4 now, so
    // put him in a red block's cell and swap again.
    bar(&mut app, 0);
    step(&mut app, 0.05);
    assert_eq!(tile(&app, 2, 3), Tile::BeatRed);
}

#[test]
fn a_beat_block_never_closes_on_nat() {
    let mut app = app(BEAT);
    bar(&mut app, 0);
    step(&mut app, 0.1);
    // Teleport Nat into the blue block's cell (gone on bar 0), then go to bar 1.
    let at = app.world().resource::<ActiveLevel>().level.tile_center(3, 3);
    app.world_mut().query_filtered::<&mut Pos, With<Player>>().single_mut(app.world_mut()).unwrap().0 = at;
    bar(&mut app, 1);
    app.update();
    assert_eq!(tile(&app, 3, 3), Tile::BeatBlueOff, "waits while he's in it");
}

#[test]
fn a_beat_crusher_slams_on_the_one_and_rests_long_enough_however_fast_the_band() {
    let level = Level::parse("name: B\ncrusher@4,1: beat=2\n---\n.......\n.P..X.G\n.......\n.......\n.......\n#######\n").unwrap();
    let def = level.crushers[0];
    assert_eq!(def.beat, 2);
    for bar_secs in [1.0f32, 2.0, 3.0] {
        let at = |bars: f32| CrushTime { sim: 0.0, bars, bar_secs };
        assert!(Crusher::offset(&def, at(0.01)).1, "slamming just after the ONE");
        assert!(Crusher::offset(&def, at(2.01)).1, "and again two bars later");
        assert!(!Crusher::offset(&def, at(1.0)).1);
        let rest = Crusher::rest_left(&def, at(2.0 - CRUSH_BEAT_REST * 2.0 + 0.01));
        assert!(rest.is_some(), "home with {CRUSH_BEAT_REST} of the cycle to spare at {bar_secs} s a bar");
        assert_eq!(Crusher::offset(&def, at(1.99)).0, 0.0);
    }
    // The sim-clock crushers didn't change.
    let plain = Level::parse(UNDER_CRUSHER).unwrap().crushers[0];
    assert_eq!(Crusher::offset(&plain, CrushTime::sim(0.0)), (0.0, false));
    assert!(Crusher::offset(&plain, CrushTime::sim(1.65)).1);
}

/// The widest gap a level reaches across, with `run` as the floor of the run-up.
fn widest_gap(run: char) -> usize {
    (2..16)
        .take_while(|&w| {
            let src = format!(
                "name: Gap\n---\n{}\n{}\n.P{}{}..G.\n{}{}{}####\n",
                ".".repeat(12 + w),
                ".".repeat(12 + w),
                ".".repeat(6),
                ".".repeat(w),
                "#",
                run.to_string().repeat(7),
                ".".repeat(w),
            );
            let level = Level::parse(&src).unwrap();
            validate::reachable(&level, (level.goal.0 as i32, level.goal.1 as i32))
        })
        .last()
        .unwrap_or(0)
}

#[test]
fn the_validator_jumps_further_off_a_belt_going_your_way() {
    let (plain, with, against) = (widest_gap('#'), widest_gap('}'), widest_gap('{'));
    assert!(with > plain, "a belt going right carries a jump further: {with} vs {plain}");
    assert!(against <= plain, "a belt going left doesn't: {against} vs {plain}");
}

#[test]
fn the_validator_rides_a_fan_up_a_wall() {
    let reaches = |src: &str| {
        let level = Level::parse(src).unwrap();
        validate::reachable_plain(&level, (level.goal.0 as i32, level.goal.1 as i32))
    };
    // A wall 8 tiles tall, the goal on its top, a fan in the floor at its foot.
    let up = "name: Fan wall
---
................
................
................
.........G......
.......#########
.......#########
.......#########
.......#########
.......#########
.......#########
.......#########
.P.....#########
######*#########
";
    assert!(reaches(up), "a fan floats Nat up the wall");
    assert!(!reaches(&up.replace('*', "#")), "no fan, no way up");
}

/// The highest one-way shelf left of a bumper wall a level reaches, with the wall's face `face`.
fn highest_shelf(face: char) -> usize {
    (2..12)
        .take_while(|&h| {
            let mut rows: Vec<String> = (0..14).map(|_| ".".repeat(16)).collect();
            rows[13] = "#".repeat(16);
            rows[12].replace_range(1..2, "P");
            for r in 4..13 {
                rows[r].replace_range(10..11, &face.to_string());
                rows[r].replace_range(11..16, "#####");
            }
            let shelf = 12 - h;
            rows[shelf].replace_range(5..8, "===");
            rows[shelf - 1].replace_range(6..7, "G");
            let src = format!("name: Bump\n---\n{}\n", rows.join("\n"));
            let level = Level::parse(&src).unwrap();
            validate::reachable_plain(&level, (level.goal.0 as i32, level.goal.1 as i32))
        })
        .last()
        .unwrap_or(0)
}

#[test]
fn the_validator_gets_batted_up_by_a_bumper() {
    let (plain, bumped) = (highest_shelf('#'), highest_shelf('('));
    assert!(bumped > plain, "a bumper's bat reaches higher shelves: {bumped} vs {plain}");
}

/// Han, alone on Side B: a fan at the foot of a wall floats him up it.
const HAN_FAN: &str = "name: Han's fan
---
..............................
..............................
..............................
.........................G....
.....................#########
.....................#########
.....................#########
.....................#########
.....................#########
.....................#########
.P...................#########
####################*#########
##############################
";

#[test]
fn han_floats_up_a_fan_on_side_b() {
    let (cleared, mut app) = han_clears(HAN_FAN, 30.0);
    assert!(cleared.is_ok(), "Han never got there: {cleared:?} at {:?}", han(&mut app) / TILE);
    let (cleared, _) = han_clears(&HAN_FAN.replace('*', "#"), 20.0);
    assert!(cleared.is_err(), "no fan, no way up");
}
