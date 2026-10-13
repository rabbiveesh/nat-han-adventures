//! The gadget tiles, headless: bricks (`%`), crumbling platforms (`-`), trampolines (`T`) and
//! crushers (`X` `<` `>`) in the real game (Nat, Han on Side B), and in the validator.

use std::time::Duration;

use bevy::{ecs::system::RunSystemOnce, input::InputPlugin, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use leafwing_input_manager::prelude::*;
use nat_han_adventures::{
    audio::AudioOutput,
    events::{DeathCause, PlayerDied},
    game::{
        ActiveLevel, Han, Player, Pos,
        crushers::CrusherBlock,
        sideb::{self, Phase, SideB, SideBLevel},
        tuning,
    },
    level::{CRUMBLE_BACK, CRUMBLE_DELAY, CrushDir, Crusher, Level, Levels, TILE, Tile, Topic, validate},
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
    app.world_mut().run_system_once(move |mut c: Commands| sideb::start_level(&mut c, level.clone(), 0)).unwrap();
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
    for t in [Topic::Brick, Topic::Crumble, Topic::Spring, Topic::Crusher] {
        assert!(topics.contains(&t), "{t:?} shows up in the playground: {topics:?}");
    }
}

