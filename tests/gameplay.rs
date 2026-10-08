//! Headless gameplay tests: the real `gameplay` plugins on `MinimalPlugins`, with a fixed
//! 60 Hz clock (exactly one simulation step per `app.update()`), so runs are deterministic.
//! Each test injects its own small level.

use std::time::Duration;

use bevy::{input::InputPlugin, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use nat_han_adventures::{
    events::*,
    audio::{Filters, Harmony},
    game::{
        Body, Checkpoint, Dead, FIRED_UP_SPEED, GIANT_STEPS_SPEED, Groove, HAN_DELAY_STEPS, Han, LevelRun,
        MovingPlatform, NERVOUS_TIME, Nugget, Player, Pos, SimClock, tuning,
    },
    level::{Level, Levels, TILE},
    state::{AppState, PlayState},
};
use leafwing_input_manager::prelude::*;

const DT: f64 = 1.0 / 60.0;
const JUMP: KeyCode = KeyCode::KeyZ;
const RIGHT: KeyCode = KeyCode::ArrowRight;
const LEFT: KeyCode = KeyCode::ArrowLeft;

fn app(level: &str) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin, InputPlugin))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(DT)))
        .add_plugins(nat_han_adventures::gameplay)
        .insert_resource(Levels(vec![Level::parse(level).expect("test level parses")]));
    count::<Jumped>(&mut app);
    count::<Landed>(&mut app);
    count::<PlayerDied>(&mut app);
    count::<PlayerRespawned>(&mut app);
    count::<NuggetCollected>(&mut app);
    count::<CheckpointReached>(&mut app);
    count::<LevelCompleted>(&mut app);
    count::<HanSays>(&mut app);
    app.finish();
    app.cleanup();
    app.update(); // Startup
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::Playing);
    app.update(); // OnEnter(Playing): spawn the level
    app.update();
    app
}

/// Counts messages of type `M` into `Count<M>`.
#[derive(Resource)]
struct Count<M>(usize, std::marker::PhantomData<M>);

fn count<M: Message>(app: &mut App) {
    app.insert_resource(Count::<M>(0, default()))
        .add_systems(Last, |mut r: MessageReader<M>, mut c: ResMut<Count<M>>| c.0 += r.read().count());
}

fn counted<M: Message>(app: &App) -> usize {
    app.world().resource::<Count<M>>().0
}

fn step(app: &mut App, secs: f32) {
    for _ in 0..(secs as f64 / DT).round() as usize {
        app.update();
    }
}

fn hold(app: &mut App, key: KeyCode) {
    key.press(app.world_mut());
}

fn release(app: &mut App, key: KeyCode) {
    key.release(app.world_mut());
}

fn single<C: Component>(app: &mut App) -> Entity {
    app.world_mut().query_filtered::<Entity, With<C>>().single(app.world()).unwrap()
}

fn player_pos(app: &mut App) -> Vec2 {
    let e = single::<Player>(app);
    app.world().get::<Pos>(e).unwrap().0
}

fn han_pos(app: &mut App) -> Vec2 {
    let e = single::<Han>(app);
    app.world().get::<Pos>(e).unwrap().0
}

fn body(app: &mut App) -> Body {
    let e = single::<Player>(app);
    app.world().get::<Body>(e).unwrap().clone()
}

fn run(app: &App) -> LevelRun {
    app.world().resource::<LevelRun>().clone()
}

fn app_state(app: &App) -> AppState {
    *app.world().resource::<State<AppState>>().get()
}

/// Player standing height (box center) on a floor whose top is at `floor_top`.
fn standing(floor_top: f32) -> f32 {
    floor_top + tuning::PLAYER_SIZE.1 / 2.0
}

/// A flat 40-wide floor, goal far away on the right.
const FLAT: &str = "name: Flat
intro: Hello Nat!
---
........................................
........................................
........................................
........................................
........................................
........................................
........................................
..P....................................G
########################################
";

#[test]
fn one_fixed_step_per_update() {
    let mut app = app(FLAT);
    let before = app.world().resource::<SimClock>().steps;
    step(&mut app, 1.0);
    assert_eq!(app.world().resource::<SimClock>().steps - before, 60);
}

#[test]
fn level_starts_with_intro_and_player_on_the_ground() {
    let mut app = app(FLAT);
    assert_eq!(counted::<HanSays>(&app), 1);
    step(&mut app, 0.2);
    let b = body(&mut app);
    assert!(b.on_ground);
    assert_eq!(player_pos(&mut app).y, standing(TILE));
}

#[test]
fn falls_and_lands_on_ground() {
    let mut app = app(FLAT);
    let p = single::<Player>(&mut app);
    app.world_mut().get_mut::<Pos>(p).unwrap().0.y += 4.0 * TILE;
    step(&mut app, 1.0);
    assert!(body(&mut app).on_ground);
    assert_eq!(player_pos(&mut app).y, standing(TILE));
    assert_eq!(counted::<Landed>(&app), 1, "a 4-tile fall is a noticeable landing");
}

fn max_height_while(app: &mut App, secs: f32, mut each: impl FnMut(&mut App, usize)) -> f32 {
    let mut max = f32::MIN;
    for i in 0..(secs as f64 / DT).round() as usize {
        each(app, i);
        app.update();
        max = max.max(player_pos(app).y);
    }
    max - standing(TILE)
}

#[test]
fn single_jump_reaches_about_three_tiles() {
    let mut app = app(FLAT);
    hold(&mut app, JUMP);
    let h = max_height_while(&mut app, 1.0, |_, _| {});
    assert!((3.0 * TILE..3.6 * TILE).contains(&h), "jump height {h}");
    assert_eq!(counted::<Jumped>(&app), 1, "holding jump doesn't re-jump");
    assert!(body(&mut app).on_ground);
}

#[test]
fn short_hop_when_jump_released_early() {
    let mut app = app(FLAT);
    let h = max_height_while(&mut app, 1.0, |app, i| match i {
        0 => hold(app, JUMP),
        5 => release(app, JUMP),
        _ => {}
    });
    assert!(h < 2.0 * TILE, "tap jump height {h}");
}

#[test]
fn double_jump_reaches_about_five_tiles_and_no_triple() {
    let mut app = app(FLAT);
    let h = max_height_while(&mut app, 1.5, |app, i| match i {
        0 => hold(app, JUMP),
        17 => release(app, JUMP),
        18 => hold(app, JUMP), // toot
        40 => release(app, JUMP),
        41 => hold(app, JUMP), // nothing left
        _ => {}
    });
    assert!(h >= 5.0 * TILE, "double jump height {h}");
    assert!(h < 6.5 * TILE, "double jump height {h}");
    assert_eq!(counted::<Jumped>(&app), 2);
}

#[test]
fn toot_comes_back_after_landing() {
    let mut app = app(FLAT);
    for _ in 0..2 {
        hold(&mut app, JUMP);
        step(&mut app, 0.25);
        release(&mut app, JUMP);
        app.update();
        hold(&mut app, JUMP);
        step(&mut app, 1.0);
        release(&mut app, JUMP);
        step(&mut app, 0.1);
    }
    assert_eq!(counted::<Jumped>(&app), 4);
}

const LEDGE: &str = "name: Ledge
---
..............................
..............................
..............................
..............................
.P............................
#####.........................
#####.........................
#####.......................G.
##############################
";

#[test]
fn coyote_time_allows_a_late_ground_jump() {
    let mut app = app(LEDGE);
    hold(&mut app, RIGHT);
    // Walk until we've just left the ledge.
    for _ in 0..120 {
        app.update();
        if !body(&mut app).on_ground {
            break;
        }
    }
    assert!(!body(&mut app).on_ground);
    step(&mut app, 0.05); // within COYOTE_TIME
    hold(&mut app, JUMP);
    app.update();
    assert_eq!(counted::<Jumped>(&app), 1);
    let p = single::<Player>(&mut app);
    let ctl = app.world().get::<nat_han_adventures::game::PlayerControl>(p).unwrap();
    assert!(ctl.has_toot, "got the ground jump, not the toot");
    // And the toot is still there.
    release(&mut app, JUMP);
    step(&mut app, 0.2);
    hold(&mut app, JUMP);
    app.update();
    assert_eq!(counted::<Jumped>(&app), 2);
}

#[test]
fn late_jump_after_coyote_is_a_toot() {
    let mut app = app(LEDGE);
    hold(&mut app, RIGHT);
    for _ in 0..120 {
        app.update();
        if !body(&mut app).on_ground {
            break;
        }
    }
    step(&mut app, 0.2);
    hold(&mut app, JUMP);
    app.update();
    assert_eq!(counted::<Jumped>(&app), 1);
    assert!(body(&mut app).vel.y <= tuning::DOUBLE_JUMP_SPEED);
}

#[test]
fn jump_buffer_fires_on_landing() {
    let mut app = app(FLAT);
    let p = single::<Player>(&mut app);
    app.world_mut().get_mut::<Pos>(p).unwrap().0.y += 2.0 * TILE;
    // Burn the toot first, then press just before landing.
    app.world_mut().get_mut::<nat_han_adventures::game::PlayerControl>(p).unwrap().has_toot = false;
    app.update();
    for _ in 0..60 {
        if player_pos(&mut app).y < standing(TILE) + 4.0 {
            break;
        }
        app.update();
    }
    hold(&mut app, JUMP);
    step(&mut app, 0.15);
    assert_eq!(counted::<Jumped>(&app), 1);
    assert!(player_pos(&mut app).y > standing(TILE) + 8.0);
}

const ONE_WAY: &str = "name: OneWay
---
..............................
..............................
..............................
..............................
..............................
..............................
..====........................
..P.........................G.
##############################
";

#[test]
fn one_way_platform_from_below() {
    let mut app = app(ONE_WAY);
    hold(&mut app, JUMP);
    step(&mut app, 1.0);
    let top = 3.0 * TILE; // the `=` row is 2 tiles above the floor top
    assert!(body(&mut app).on_ground);
    assert_eq!(player_pos(&mut app).y, standing(top), "jumped up through it and stands on it");
    release(&mut app, JUMP);
    // Walk off: falls back to the floor.
    hold(&mut app, RIGHT);
    step(&mut app, 1.0);
    assert_eq!(player_pos(&mut app).y, standing(TILE));
}

const PLATFORM: &str = "name: Platform
1: dx=4 dy=0 period=4
2: dx=0 dy=3 period=3
---
..............................
..............................
..............................
..............................
..P.............P.............
..11...........22.............
..............................
..............................
..............................
.............................G
";

#[test]
fn carried_by_a_moving_platform() {
    // Two `P`s isn't allowed: use the left one.
    let level = PLATFORM.replacen("..P.............P", "..P..............", 1);
    let mut app = app(&level);
    let plat = {
        let mut q = app.world_mut().query::<(Entity, &MovingPlatform)>();
        q.iter(app.world()).find(|(_, p)| p.travel.y == 0.0).unwrap().0
    };
    let p0 = player_pos(&mut app);
    let plat0 = app.world().get::<Pos>(plat).unwrap().0;
    step(&mut app, 1.5);
    let moved = app.world().get::<Pos>(plat).unwrap().0 - plat0;
    assert!(moved.x > 2.0 * TILE, "platform moved {moved}");
    assert!(body(&mut app).on_ground);
    let p = player_pos(&mut app);
    assert!((p.x - p0.x - moved.x).abs() < 0.01, "carried along: {p} vs {p0}+{moved}");
    assert_eq!(counted::<PlayerDied>(&app), 0);
}

#[test]
fn rides_a_vertical_platform() {
    let level = PLATFORM.replacen("..P.............P", "................P", 1);
    let mut app = app(&level);
    let mut max = f32::MIN;
    for _ in 0..90 {
        app.update();
        max = max.max(player_pos(&mut app).y);
        assert!(body(&mut app).on_ground || app.world().resource::<SimClock>().steps < 3);
    }
    assert!(max > standing(5.0 * TILE) + 3.0 * TILE - 1.0, "rode up: {max}");
    assert_eq!(counted::<PlayerDied>(&app), 0);
}

const SPIKES: &str = "name: Spikes
say: First checkpoint line
---
..............................
..............................
.P...C.......^^.............G.
##############################
";

#[test]
fn spikes_kill_and_respawn_at_checkpoint() {
    let mut app = app(SPIKES);
    let says0 = counted::<HanSays>(&app);
    hold(&mut app, RIGHT);
    for _ in 0..240 {
        app.update();
        if counted::<PlayerDied>(&app) > 0 {
            break;
        }
    }
    release(&mut app, RIGHT);
    assert_eq!(counted::<CheckpointReached>(&app), 1);
    assert_eq!(counted::<PlayerDied>(&app), 1);
    assert_eq!(run(&app).checkpoint, Some(0));
    assert_eq!(run(&app).deaths, 1);
    // Checkpoint line + the first-death line.
    assert_eq!(counted::<HanSays>(&app) - says0, 2);
    let cp = single::<Checkpoint>(&mut app);
    assert!(app.world().get::<Checkpoint>(cp).unwrap().active);

    let p = single::<Player>(&mut app);
    assert!(app.world().get::<Dead>(p).is_some());
    let frozen = player_pos(&mut app);
    step(&mut app, tuning::RESPAWN_DELAY / 2.0);
    assert_eq!(player_pos(&mut app), frozen, "frozen while dead");
    step(&mut app, tuning::RESPAWN_DELAY / 2.0 + 0.05);
    assert_eq!(counted::<PlayerRespawned>(&app), 1);
    assert!(app.world().get::<Dead>(p).is_none());
    let pos = player_pos(&mut app);
    assert_eq!(pos.x, 5.0 * TILE + TILE / 2.0);
    assert!((pos.y - standing(TILE)).abs() < 1.0);
    // Han pops back in behind.
    assert!(han_pos(&mut app).distance(pos) < 2.0 * TILE);
}

const PIT: &str = "name: Pit
---
..............................
.P............................
####......................G...
##########...........#########
";

#[test]
fn falling_into_the_pit_kills() {
    let mut app = app(PIT);
    hold(&mut app, RIGHT);
    step(&mut app, 3.0);
    assert!(counted::<PlayerDied>(&app) >= 1);
    assert!(run(&app).deaths >= 1);
}

const HAZARDS: &str = "name: Hazards
---
..............................
..............................
..............................
.P..........F..............G..
##############################
";

#[test]
fn flies_kill() {
    let mut app = app(HAZARDS);
    hold(&mut app, RIGHT);
    step(&mut app, 2.0);
    assert!(counted::<PlayerDied>(&app) >= 1);
}

const SPRAY: &str = "name: Spray
---
..............................
..............................
..............................
..............................
.P...........S.............G..
##############################
";

#[test]
fn spray_kills_only_while_on() {
    use nat_han_adventures::game::Spray;
    let mut app = app(SPRAY);
    let p = single::<Player>(&mut app);
    let s = single::<Spray>(&mut app);
    // Hover right in the jet column above the can (all cans fire from t=1.5s).
    let hover = Vec2::new(13.5 * TILE, standing(TILE) + TILE);
    for i in 0..120 {
        app.world_mut().get_mut::<Pos>(p).unwrap().0 = hover;
        app.world_mut().get_mut::<Body>(p).unwrap().vel = Vec2::ZERO;
        app.update();
        let on = app.world().get::<Spray>(s).unwrap().on;
        let died = counted::<PlayerDied>(&app) > 0;
        assert_eq!(on, died, "step {i}: dies exactly when the jet turns on");
        if died {
            assert!(i > 60, "off for the first second or so");
            return;
        }
    }
    panic!("spray never fired");
}

const NUGGETS: &str = "name: Nuggets
---
..............................
.P..o.o.o.................G...
##############################
";

#[test]
fn nuggets_are_collected() {
    let mut app = app(NUGGETS);
    assert_eq!(run(&app).nuggets_total, 3);
    hold(&mut app, RIGHT);
    step(&mut app, 1.0);
    assert_eq!(run(&app).nuggets, 3);
    assert_eq!(counted::<NuggetCollected>(&app), 3);
    let left = app.world_mut().query::<&Nugget>().iter(app.world()).count();
    assert_eq!(left, 0);
}

#[test]
fn restart_resets_nuggets_and_position() {
    let mut app = app(NUGGETS);
    let start = player_pos(&mut app);
    hold(&mut app, RIGHT);
    step(&mut app, 0.6);
    release(&mut app, RIGHT);
    assert!(run(&app).nuggets >= 1);
    hold(&mut app, KeyCode::KeyR);
    app.update();
    release(&mut app, KeyCode::KeyR);
    app.update();
    assert_eq!(run(&app).nuggets, 0);
    assert_eq!(run(&app).nuggets_total, 3);
    assert_eq!(app.world_mut().query::<&Nugget>().iter(app.world()).count(), 3);
    assert_eq!(app.world_mut().query::<&Player>().iter(app.world()).count(), 1);
    assert_eq!(app.world_mut().query::<&Han>().iter(app.world()).count(), 1);
    assert_eq!(player_pos(&mut app), start);
}

#[test]
fn reaching_the_goal_completes_the_level() {
    let mut app = app(NUGGETS);
    hold(&mut app, RIGHT);
    step(&mut app, 4.0);
    assert_eq!(counted::<LevelCompleted>(&app), 1);
    assert_eq!(app_state(&app), AppState::LevelComplete);
    // Level entities stay (frozen) for the results screen...
    assert_eq!(app.world_mut().query::<&Player>().iter(app.world()).count(), 1);
    let t = run(&app).time;
    step(&mut app, 0.5);
    assert_eq!(run(&app).time, t, "timer stopped");
    // ...and go away on the way out.
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::LevelSelect);
    app.update();
    app.update();
    assert_eq!(app.world_mut().query::<&Player>().iter(app.world()).count(), 0);
}

#[test]
fn pause_freezes_everything() {
    let mut app = app(FLAT);
    hold(&mut app, KeyCode::Escape);
    app.update();
    release(&mut app, KeyCode::Escape);
    app.update();
    assert_eq!(*app.world().resource::<State<PlayState>>().get(), PlayState::Paused);
    let p = player_pos(&mut app);
    let t = run(&app).time;
    hold(&mut app, RIGHT);
    step(&mut app, 0.5);
    assert_eq!(player_pos(&mut app), p);
    assert_eq!(run(&app).time, t);
    app.world_mut().resource_mut::<NextState<PlayState>>().set(PlayState::Running);
    step(&mut app, 0.5);
    assert!(player_pos(&mut app).x > p.x);
    assert!(run(&app).time > t);
}

const LONG: &str = "name: Long
---
............................................................
............................................................
............................................................
..........................................#.................
.P...........................#####.......###...............G
############################################################
";

#[test]
fn han_follows_the_same_path() {
    let mut app = app(LONG);
    hold(&mut app, RIGHT);
    let mut trail = Vec::new();
    for i in 0..240 {
        // Hop over the bumps.
        if i % 40 == 0 {
            hold(&mut app, JUMP);
        } else if i % 40 == 30 {
            release(&mut app, JUMP);
        }
        app.update();
        let (p, g) = (player_pos(&mut app), han_pos(&mut app));
        trail.push(p);
        assert!(g.distance(p) <= 12.0 * TILE, "Han stays in range");
        if trail.len() > HAN_DELAY_STEPS + 30 {
            // Replaying the path ~HAN_DELAY_STEPS steps late.
            let past = trail[trail.len() - 1 - HAN_DELAY_STEPS];
            assert!(g.distance(past) < 1.0, "step {i}: han {g} vs player's past {past}");
        }
    }
    release(&mut app, RIGHT);
    release(&mut app, JUMP);
    step(&mut app, 1.5);
    let (p, g) = (player_pos(&mut app), han_pos(&mut app));
    assert!((g.y - p.y).abs() < 1.0, "landed, not frozen mid-jump: {g} vs {p}");
    assert!(p.x - g.x > 0.0 && p.x - g.x < 5.0 * TILE, "waits a little behind: {g} vs {p}");
}

#[test]
fn han_pops_back_when_far() {
    let mut app = app(LONG);
    let p = single::<Player>(&mut app);
    app.world_mut().get_mut::<Pos>(p).unwrap().0.x += 30.0 * TILE;
    app.update();
    let (pp, g) = (player_pos(&mut app), han_pos(&mut app));
    assert!(g.distance(pp) < 2.0 * TILE, "popped next to the player");
}

#[test]
fn walls_and_level_edges_block() {
    let mut app = app(LONG);
    hold(&mut app, LEFT);
    step(&mut app, 1.0);
    assert_eq!(player_pos(&mut app).x, tuning::PLAYER_SIZE.0 / 2.0, "left edge is a wall");
    release(&mut app, LEFT);
    // The first bump (cols 29..34, one tile high) blocks walking.
    hold(&mut app, RIGHT);
    step(&mut app, 5.0);
    assert_eq!(player_pos(&mut app).x, 29.0 * TILE - tuning::PLAYER_SIZE.0 / 2.0);
}

/// Every real level loads into the simulation, and standing still at the start for 5s is safe
/// (no hazard reaches the spawn, nothing falls through the floor).
#[test]
fn real_levels_start_safe() {
    for (i, src) in nat_han_adventures::level::LEVEL_SOURCES.iter().enumerate() {
        let mut app = app(src);
        let start = player_pos(&mut app);
        step(&mut app, 5.0);
        assert_eq!(counted::<PlayerDied>(&app), 0, "level {}: died standing at the start", i + 1);
        assert!(body(&mut app).on_ground, "level {}: not on the ground after 5s", i + 1);
        assert!(player_pos(&mut app).distance(start) < TILE * 2.0, "level {}: drifted", i + 1);
        assert_eq!(app_state(&app), AppState::Playing);
    }
}

/// When the player stops, Han waits a little behind rather than standing inside them.
#[test]
fn han_stops_behind_a_standing_player() {
    let mut app = app(FLAT);
    hold(&mut app, RIGHT);
    step(&mut app, 1.0);
    release(&mut app, RIGHT);
    step(&mut app, 2.0);
    let (p, g) = (player_pos(&mut app), han_pos(&mut app));
    assert!(p.x - g.x > TILE, "han at {g}, player at {p}: should be visibly behind");
    assert!(p.x - g.x < 5.0 * TILE, "han at {g}, player at {p}: shouldn't lag far behind");
}

fn set_groove(app: &mut App, harmony: Harmony, just_intonation: bool) {
    app.world_mut().insert_resource(Groove::new(Filters { harmony, just_intonation }));
}

/// Feet height reached by a quick double-tap: 0.1s press, 0.1s gap (releasing cuts the first
/// jump short), then hold the toot.
fn quick_double_tap(app: &mut App) -> f32 {
    let floor = standing(TILE);
    hold(app, JUMP);
    step(app, 0.1);
    release(app, JUMP);
    step(app, 0.1);
    hold(app, JUMP);
    let mut top = 0.0f32;
    for _ in 0..120 {
        app.update();
        top = top.max(player_pos(app).y - floor);
    }
    assert_eq!(counted::<Jumped>(app), 2, "ground jump + toot");
    top
}

/// Highest a double jump gets with the best toot timing (hold the first jump, toot at frame k).
fn best_double_jump(groove: Harmony) -> (f32, usize) {
    (8..50)
        .map(|k| {
            let mut app = app(FLAT);
            set_groove(&mut app, groove, false);
            let h = max_height_while(&mut app, 2.0, |app, i| match i {
                0 => hold(app, JUMP),
                i if i == k => release(app, JUMP),
                i if i == k + 1 => hold(app, JUMP),
                _ => {}
            });
            (h, k)
        })
        .fold((0.0, 0), |a, b| if b.0 > a.0 { b } else { a })
}

/// Normal physics: even a perfectly timed double jump falls short of a 6-tile "giant wall"
/// (it needs Giant Steps); a 5-tile wall is just possible.
#[test]
fn no_double_jump_clears_six_tiles_normally() {
    let (h, k) = best_double_jump(Harmony::Original);
    println!("best normal double jump: {h:.1}px (toot at frame {k})");
    assert!(h > 5.0 * TILE, "best double jump {h}px can't even do 5 tiles");
    assert!(h < 6.0 * TILE - 4.0, "best double jump {h}px (toot at frame {k}) clears a giant wall");
    let mut app = app(FLAT);
    let top = quick_double_tap(&mut app);
    println!("normal quick double-tap: {top:.1}px");
    assert!(top < 5.0 * TILE, "quick double-tap {top}px");
}

/// Giant Steps (the band plays Coltrane changes): weaker gravity, higher jumps. A single
/// jump reaches ~4.8 tiles and even a quick double-tap gets the feet over a 6-tile wall.
#[test]
fn giant_steps_jumps_higher() {
    let mut app = app(FLAT);
    set_groove(&mut app, Harmony::Coltrane, false);
    hold(&mut app, JUMP);
    let h = max_height_while(&mut app, 1.5, |_, _| {});
    println!("giant steps single jump: {h:.1}px = {:.2} tiles", h / TILE);
    assert!((4.4 * TILE..5.0 * TILE).contains(&h), "giant steps jump height {h}");

    let mut app = self::app(FLAT);
    set_groove(&mut app, Harmony::Coltrane, false);
    let top = quick_double_tap(&mut app);
    println!("giant steps quick double-tap: {top:.1}px");
    assert!(top > 6.0 * TILE + 2.0, "feet rose only {top}px");
    let (best, _) = best_double_jump(Harmony::Coltrane);
    println!("giant steps best double jump: {best:.1}px");
    assert!(best > 7.0 * TILE);
}

/// Top running speed after a second of holding Right.
fn top_speed(harmony: Harmony) -> f32 {
    let mut app = app(FLAT);
    set_groove(&mut app, harmony, false);
    hold(&mut app, RIGHT);
    step(&mut app, 1.0);
    body(&mut app).vel.x
}

/// Fired up (quartal): faster running, same jump height. Giant Steps runs slower so its long
/// air time doesn't also clear long gaps.
#[test]
fn run_speed_per_mode() {
    assert_eq!(top_speed(Harmony::Original), tuning::RUN_SPEED);
    assert_eq!(top_speed(Harmony::Quartal), tuning::RUN_SPEED * FIRED_UP_SPEED);
    assert_eq!(top_speed(Harmony::Coltrane), tuning::RUN_SPEED * GIANT_STEPS_SPEED);
    assert_eq!(top_speed(Harmony::MelodicMinor), tuning::RUN_SPEED);

    let mut app = app(FLAT);
    set_groove(&mut app, Harmony::Quartal, false);
    hold(&mut app, JUMP);
    let h = max_height_while(&mut app, 1.0, |_, _| {});
    assert!((3.0 * TILE..3.6 * TILE).contains(&h), "quartal jump height {h}");
}

/// The nervous band (melodic minor): the game runs in slow motion (the music doesn't).
#[test]
fn nervous_band_slows_time() {
    let mut app = app(FLAT);
    set_groove(&mut app, Harmony::MelodicMinor, false);
    app.update();
    let before = app.world().resource::<SimClock>().steps;
    step(&mut app, 1.0);
    let steps = app.world().resource::<SimClock>().steps - before;
    let want = (60.0 * NERVOUS_TIME).round() as u64;
    assert!(steps.abs_diff(want) <= 1, "{steps} steps in 1s real time");
    // Back to normal speed with the normal groove.
    set_groove(&mut app, Harmony::Original, false);
    app.update();
    let before = app.world().resource::<SimClock>().steps;
    step(&mut app, 1.0);
    assert!((app.world().resource::<SimClock>().steps - before).abs_diff(60) <= 1);
}

/// The laughing band (just intonation): landings spring Nat back up ~1 tile, lower each time,
/// without using up a jump; jumping out of a bounce is a full ground jump.
#[test]
fn laughing_band_bounces() {
    let mut app = app(FLAT);
    set_groove(&mut app, Harmony::Original, true);
    hold(&mut app, JUMP);
    step(&mut app, 0.1);
    release(&mut app, JUMP);
    // Wait for the landing.
    let mut frames = 0;
    while counted::<Landed>(&app) == 0 {
        app.update();
        frames += 1;
        assert!(frames < 120);
    }
    // Bounces back up.
    let h = max_height_while(&mut app, 0.6, |_, _| {});
    println!("bounce height {h:.1}px");
    assert!((0.5 * TILE..1.3 * TILE).contains(&h), "bounce height {h}");
    assert_eq!(counted::<Jumped>(&app), 1, "the bounce isn't a jump");
    // Settles down eventually.
    step(&mut app, 2.0);
    assert!(body(&mut app).on_ground);
    let landings = counted::<Landed>(&app);
    assert!(landings >= 2, "landings: {landings}");

    // Jumping out of a bounce: a ground jump, the toot still in hand.
    hold(&mut app, JUMP);
    step(&mut app, 0.6);
    release(&mut app, JUMP);
    while counted::<Landed>(&app) == landings {
        app.update();
    }
    app.update();
    assert!(!body(&mut app).on_ground, "bouncing");
    hold(&mut app, JUMP);
    app.update();
    assert!(body(&mut app).vel.y > tuning::JUMP_SPEED * 0.9, "a full ground jump");
    release(&mut app, JUMP);
    app.update();
    hold(&mut app, JUMP);
    app.update();
    assert_eq!(counted::<Jumped>(&app), 4, "jump out of the bounce + a toot");
    assert!(body(&mut app).vel.y > tuning::DOUBLE_JUMP_SPEED * 0.9);

    // Normal groove: no bounce.
    let mut app = self::app(FLAT);
    hold(&mut app, JUMP);
    step(&mut app, 0.1);
    release(&mut app, JUMP);
    step(&mut app, 1.0);
    assert!(body(&mut app).on_ground);
    assert_eq!(counted::<Landed>(&app), 1);
}

/// The groove is the music's business, but a fresh level (or a restart) always starts normal.
#[test]
fn groove_resets_on_restart_and_level_start() {
    let mut app = app(FLAT);
    assert_eq!(*app.world().resource::<Groove>(), Groove::default());
    set_groove(&mut app, Harmony::Coltrane, true);
    hold(&mut app, KeyCode::KeyR);
    app.update();
    release(&mut app, KeyCode::KeyR);
    app.update();
    assert_eq!(*app.world().resource::<Groove>(), Groove::default());

    set_groove(&mut app, Harmony::Quartal, false);
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::Title);
    app.update();
    assert_eq!(*app.world().resource::<Groove>(), Groove::default(), "leaving the level");
    set_groove(&mut app, Harmony::MelodicMinor, false);
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::Playing);
    app.update();
    assert_eq!(*app.world().resource::<Groove>(), Groove::default(), "entering the level");
}

const NUGGETS_THEN_SPIKES: &str = "name: Nugget run
---
..............................
.P..o.o.C.o.o..^..........G...
##############################
";

/// Nuggets picked up since the last checkpoint come back after a splat (so a nugget line that
/// fires up the band for a long gap is there for the next try); earlier ones stay collected.
#[test]
fn nuggets_since_the_checkpoint_come_back_after_a_splat() {
    let mut app = app(NUGGETS_THEN_SPIKES);
    hold(&mut app, RIGHT);
    for _ in 0..240 {
        app.update();
        if counted::<PlayerDied>(&app) > 0 {
            break;
        }
    }
    release(&mut app, RIGHT);
    assert_eq!(run(&app).nuggets, 4);
    step(&mut app, tuning::RESPAWN_DELAY + 0.1);
    assert_eq!(counted::<PlayerRespawned>(&app), 1);
    assert_eq!(run(&app).nuggets, 2);
    assert_eq!(app.world_mut().query::<&Nugget>().iter(app.world()).count(), 2);
    hold(&mut app, RIGHT);
    step(&mut app, 0.5);
    assert_eq!(run(&app).nuggets, 4);
}

/// Hopping in place (no sideways movement) never pulls Han on top of the player.
#[test]
fn han_stays_beside_a_player_hopping_in_place() {
    let mut app = app(FLAT);
    for _ in 0..6 {
        hold(&mut app, JUMP);
        step(&mut app, 0.1);
        release(&mut app, JUMP);
        step(&mut app, 0.6);
        let (p, h) = (player_pos(&mut app), han_pos(&mut app));
        assert!((p.x - h.x).abs() >= 13.0, "han at {h}, player at {p}: overlapping");
    }
}
