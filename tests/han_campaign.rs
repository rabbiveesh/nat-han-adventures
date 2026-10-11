//! The campaign's Han gates, played headless in the real levels with the real Han: each
//! mega wall, shield row, fired-up chasm and buddy raft pool (found by its `gate:` mark). The
//! same for free play's Han rooms (`free_play_*`), at a few bands and seeds.

use std::time::Duration;

use bevy::{input::InputPlugin, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use leafwing_input_manager::prelude::*;
use nat_han_adventures::{
    events::PlayerDied,
    game::*,
    level::{GateMark, LEVEL_SOURCES, Level, Levels, TILE, Topic},
    state::AppState,
};

const DT: f64 = 1.0 / 60.0;
const JUMP: KeyCode = KeyCode::KeyZ;
const RIGHT: KeyCode = KeyCode::ArrowRight;
const LEFT: KeyCode = KeyCode::ArrowLeft;

#[derive(Resource, Default)]
struct Deaths(usize);

fn app(level: &Level) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin, InputPlugin))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(DT)))
        .add_plugins(nat_han_adventures::gameplay)
        .insert_resource(Levels(vec![level.clone()]))
        .insert_resource(AssistMode::Manual)
        .init_resource::<Deaths>()
        .add_systems(Last, |mut d: ResMut<Deaths>, mut r: MessageReader<PlayerDied>| {
            for e in r.read() {
                if std::env::var("HAN_TRACE").is_ok() {
                    eprintln!("died: {e:?}");
                }
                d.0 += 1;
            }
        });
    app.finish();
    app.cleanup();
    app.update();
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::Playing);
    app.update();
    app.update();
    app
}

fn single<C: Component>(app: &mut App) -> Entity {
    app.world_mut().query_filtered::<Entity, With<C>>().single(app.world()).unwrap()
}

fn pos<C: Component>(app: &mut App) -> Vec2 {
    let e = single::<C>(app);
    app.world().get::<Pos>(e).unwrap().0
}

fn body<C: Component>(app: &mut App) -> Body {
    let e = single::<C>(app);
    app.world().get::<Body>(e).unwrap().clone()
}

fn put<C: Component>(app: &mut App, p: Vec2) {
    let e = single::<C>(app);
    app.world_mut().get_mut::<Pos>(e).unwrap().0 = p;
    app.world_mut().get_mut::<PrevPos>(e).unwrap().0 = p;
    if let Some(mut b) = app.world_mut().get_mut::<Body>(e) {
        b.vel = Vec2::ZERO;
    }
}

fn step(app: &mut App, secs: f32) {
    for _ in 0..(secs as f64 / DT).round() as usize {
        app.update();
    }
}

fn tap(app: &mut App) {
    JUMP.release(app.world_mut());
    app.update();
    JUMP.press(app.world_mut());
}

/// Nat was put somewhere new: wait for Han to arrive (routing, or parachuting in) and settle.
fn wait_for_han(app: &mut App, what: &str) {
    for _ in 0..(20.0 / DT) as usize {
        app.update();
        let (n, h) = (pos::<Player>(app), pos::<Han>(app));
        if (n - h).length() < 3.0 * TILE && body::<Han>(app).on_ground && body::<Han>(app).vel.x.abs() < 1.0 {
            step(app, 0.3);
            return;
        }
    }
    let h = single::<Han>(app);
    let at = pos::<Han>(app);
    panic!("{what}: Han never came: {:?} at {at}", app.world().get::<HanBrain>(h).unwrap().mode);
}

fn deaths(app: &App) -> usize {
    app.world().resource::<Deaths>().0
}

fn stand(level: &Level, c: i32, r: i32) -> Vec2 {
    nat_han_adventures::level::buddy::stand(level, (c, r))
}

/// Every Han gate mark of `topic` in the campaign: (what, level, mark).
fn marks(topic: Topic) -> Vec<(String, Level, GateMark)> {
    LEVEL_SOURCES
        .iter()
        .enumerate()
        .flat_map(|(i, s)| {
            let level = Level::parse(s).unwrap();
            level.gates.iter().filter(|g| g.topic == topic).map(|&g| (format!("level {}", i + 1), level.clone(), g)).collect::<Vec<_>>()
        })
        .collect()
}

/// Free play's rooms from `template` at bands 1, 5 and 10 (two seeds each), with the marks of
/// `topic` the validator derived: (what, room, mark).
fn room_marks(template: &str, topic: Topic) -> Vec<(String, Level, GateMark)> {
    use nat_han_adventures::adapt::{AssistLevers, RoomRequest};
    use nat_han_adventures::freeplay::generate::{RoomPlan, generate};
    use nat_han_adventures::freeplay::templates::TEMPLATES;
    let ti = TEMPLATES.iter().position(|t| t.name == template).expect("a template");
    let mut out = Vec::new();
    for band in [1, 5, 10] {
        for seed in [11, 12] {
            let request = RoomRequest { skill: TEMPLATES[ti].skill, band, assists: AssistLevers::NONE };
            let plan = RoomPlan { template: ti, hint: false, ..RoomPlan::new(seed, 1, request, 2, true, false, 10) };
            let room = generate(seed, plan);
            assert!(!room.fallback, "{template} band {band} seed {seed}: fell back");
            let ms: Vec<GateMark> = room.level.gates.iter().copied().filter(|g| g.topic == topic).collect();
            assert!(!ms.is_empty(), "{template} band {band} seed {seed}: no {topic:?} mark");
            out.extend(ms.into_iter().map(|m| (format!("{template} band {band} seed {seed}"), room.level.clone(), m)));
        }
    }
    out
}

#[test]
fn mega_walls_in_the_campaign() {
    let ms = marks(Topic::Mega);
    assert!(ms.len() >= 3);
    for (what, level, m) in &ms {
        assert!(play_mega(what, level, *m, true), "{what}: not on top");
    }
    // Without Giant Steps it's his weak boost there: no way up.
    let (what, level, m) = &ms[0];
    assert!(!play_mega(what, level, *m, false), "{what}: on top without Giant Steps");
}

#[test]
fn free_play_mega_walls() {
    for (what, level, m) in room_marks("mega wall", Topic::Mega) {
        assert!(play_mega(&what, &level, m, true), "{what}: not on top");
    }
}

/// Boost off Han by mega wall `m` (the band playing Giant Steps or not): on top of it?
fn play_mega(what: &str, level: &Level, m: GateMark, giant_steps: bool) -> bool {
    let mut app = app(level);
    if giant_steps {
        app.world_mut().insert_resource(Groove::of(nat_han_adventures::audio::Harmony::Coltrane));
    }
    // The mesa: the first column in the mark solid at Nat's height.
    let r = m.r1;
    let face = (m.c0..=m.c1).find(|&c| level.tile(c, r).is_solid()).expect("a wall face");
    let height = (0..).take_while(|k| level.tile(face, r - k).is_solid()).count() as i32;
    assert!(height >= 11, "{what}: a mega wall is 11+ tall, not {height}");
    put::<Player>(&mut app, stand(level, face - 2, r));
    wait_for_han(&mut app, what);
    // Hop onto Han's head (he's in his slot behind), and mega-boost, toot near the top.
    let h = pos::<Han>(&mut app);
    put::<Player>(&mut app, h + Vec2::new(0.0, 1.5 * TILE));
    step(&mut app, 0.4);
    assert_eq!(body::<Player>(&mut app).riding, Some(single::<Han>(&mut app)), "{what}: on Han");
    RIGHT.press(app.world_mut());
    tap(&mut app);
    step(&mut app, 0.6);
    tap(&mut app);
    // Let go once over the mesa (a narrow top is easy to overshoot).
    let over = level.tile_center(face as usize, 0).x + TILE / 2.0;
    for _ in 0..(1.0 / DT) as usize {
        app.update();
        if pos::<Player>(&mut app).x > over {
            break;
        }
    }
    RIGHT.release(app.world_mut());
    step(&mut app, 1.5);
    let p = pos::<Player>(&mut app);
    let top = level.tile_center(face as usize, (r - height) as usize).y - TILE / 2.0;
    assert_eq!(deaths(&app), 0, "{what}");
    p.y > top && body::<Player>(&mut app).on_ground
}

#[test]
fn cling_shafts_in_the_campaign() {
    let ms = marks(Topic::Shaft);
    assert!(!ms.is_empty());
    for (what, level, m) in &ms {
        assert!(play_shaft(what, level, *m, true), "{what}: not up the shaft");
    }
    // The band not nervous, he doesn't cling: no way up.
    let (what, level, m) = &ms[0];
    assert!(!play_shaft(what, level, *m, false), "{what}: up the shaft without grip");
}

#[test]
fn free_play_cling_shafts() {
    for (what, level, m) in room_marks("cling shaft", Topic::Shaft) {
        assert!(play_shaft(&what, &level, m, true), "{what}: not up the shaft");
    }
}

/// Wait in cling shaft `m` (the band nervous or not) for Han to climb a wall, hop on his head
/// and kick off it, toot at the top: up out of the shaft?
fn play_shaft(what: &str, level: &Level, m: GateMark, grip: bool) -> bool {
    let mut app = app(level);
    app.world_mut().insert_resource(Groove { nervous: grip, ..Groove::default() });
    // The shaft's floor: the first open column of the mark at its bottom row.
    let r = m.r1;
    let c = (m.c0..=m.c1).find(|&c| !level.tile(c, r).is_solid() && level.tile(c, r + 1).is_solid()).expect("a shaft floor");
    put::<Player>(&mut app, stand(level, c + 1, r));
    let han = single::<Han>(&mut app);
    let mut kick = 0.0;
    // Han comes (or parachutes in) and, nervous, climbs a wall: his head kicks.
    for _ in 0..(20.0 / DT) as usize {
        app.update();
        let head = *app.world().get::<HanHead>(han).unwrap();
        if head.boost && head.kick != 0.0 {
            kick = head.kick;
            break;
        }
    }
    if kick == 0.0 {
        assert!(!grip, "{what}: Han never clung: {:?}", app.world().get::<HanBrain>(han).unwrap().mode);
        return false;
    }
    let floor = pos::<Player>(&mut app).y;
    let h = pos::<Han>(&mut app);
    put::<Player>(&mut app, h + Vec2::new(0.0, 1.5 * TILE));
    step(&mut app, 0.4);
    assert_eq!(body::<Player>(&mut app).riding, Some(han), "{what}: on Han");
    let dir = if kick > 0.0 { RIGHT } else { LEFT };
    dir.press(app.world_mut());
    tap(&mut app);
    step(&mut app, 0.33); // the toot near the top
    let mode = app.world().get::<HanBrain>(han).unwrap().mode;
    assert!(!matches!(mode, HanMode::Cling { .. }), "{what}: kicked off, he lets go: {mode:?}");
    tap(&mut app);
    // Let go once landed up top (a narrow top is easy to overshoot).
    for _ in 0..(1.5 / DT) as usize {
        app.update();
        if body::<Player>(&mut app).on_ground && pos::<Player>(&mut app).y > floor + 6.0 * TILE {
            break;
        }
    }
    dir.release(app.world_mut());
    step(&mut app, 1.0);
    let p = pos::<Player>(&mut app);
    assert_eq!(deaths(&app), 0, "{what}");
    p.y > floor + 6.0 * TILE && body::<Player>(&mut app).on_ground
}

#[test]
fn shield_rows_in_the_campaign() {
    let ms = marks(Topic::Shield);
    assert!(ms.len() >= 2);
    for (what, level, m) in ms {
        play_shield(&what, &level, m);
    }
}

#[test]
fn free_play_shield_rows() {
    for (what, level, m) in room_marks("shield row", Topic::Shield) {
        play_shield(&what, &level, m);
    }
}

fn play_shield(what: &str, level: &Level, m: GateMark) {
    let mut app = app(level);
    put::<Player>(&mut app, stand(level, m.c0, m.r1));
    wait_for_han(&mut app, what);
    // Face the row, stand still: Han goes ahead.
    RIGHT.press(app.world_mut());
    step(&mut app, 0.05);
    RIGHT.release(app.world_mut());
    let mut ahead = false;
    for _ in 0..300 {
        app.update();
        let h = single::<Han>(&mut app);
        if matches!(app.world().get::<HanBrain>(h).unwrap().mode, HanMode::Ahead { .. }) {
            ahead = true;
            break;
        }
    }
    assert!(ahead, "{what}: Han went ahead");
    step(&mut app, 0.4);
    RIGHT.press(app.world_mut());
    let end = (m.c1 as f32 + 1.0) * TILE;
    for _ in 0..1200 {
        app.update();
        if pos::<Player>(&mut app).x > end {
            break;
        }
    }
    let p = pos::<Player>(&mut app);
    assert_eq!(deaths(&app), 0, "{what}: sprayed at {p}");
    assert!(p.x > end, "{what}: through at {p}");
}

#[test]
fn fired_up_chasms_in_the_campaign() {
    let ms = marks(Topic::Chain);
    assert!(ms.len() >= 2);
    for (what, level, m) in ms {
        play_chasm(&what, &level, m);
    }
}

#[test]
fn free_play_fired_up_chasms() {
    for (what, level, m) in room_marks("fired-up chasm", Topic::Chain) {
        play_chasm(&what, &level, m);
    }
}

/// The band fired up, Nat walks to the edge and waits for Han, hops back onto his head, and
/// jumps off it (the throw) running right, then toots: across. A few toot timings (a player
/// tries again too); every failure if none gets across.
fn play_chasm(what: &str, level: &Level, m: GateMark) {
    let mut fails = Vec::new();
    for toot in [0.35, 0.25, 0.45, 0.55] {
        match throw_try(what, level, m, toot) {
            Ok(()) => return,
            Err(e) => fails.push(format!("toot at {toot}s: {e}")),
        }
    }
    panic!("{what}: no throw across:\n  {}", fails.join("\n  "));
}

fn throw_try(what: &str, level: &Level, m: GateMark, toot: f32) -> Result<(), String> {
    let mut app = app(level);
    app.world_mut().insert_resource(Groove::of(nat_han_adventures::audio::Harmony::Quartal));
    // The walk row: the first row of the mark with floor under it.
    let r = (m.r0..=m.r1).find(|&r| (m.c0..=m.c1).any(|c| level.tile(c, r + 1).is_solid())).expect("a floor");
    let edge = (m.c0..=m.c1).find(|&c| !level.tile(c, r + 1).is_solid()).expect("a chasm") - 1;
    put::<Player>(&mut app, stand(level, edge - 3, r));
    wait_for_han(&mut app, what);
    RIGHT.press(app.world_mut());
    while pos::<Player>(&mut app).x < (edge as f32 + 0.5) * TILE {
        app.update();
    }
    RIGHT.release(app.world_mut());
    wait_for_han(&mut app, what);
    // Hop back onto Han's head.
    let han = single::<Han>(&mut app);
    let mut on = false;
    for k in 0..4 {
        // Straight up (Han runs under), else a little hop back towards him.
        let back = if k == 0 { 0 } else { 2 * k };
        if back > 0 {
            LEFT.press(app.world_mut());
        }
        tap(&mut app);
        for i in 0..60 {
            app.update();
            if i >= back {
                LEFT.release(app.world_mut());
            }
            if std::env::var("HAN_TRACE").is_ok() {
                let mode = app.world().get::<HanBrain>(han).unwrap().mode;
                eprintln!("k={k} i={i} nat {} {} han {} {} {mode:?}", pos::<Player>(&mut app), body::<Player>(&mut app).vel, pos::<Han>(&mut app), body::<Han>(&mut app).vel);
            }
            let b = body::<Player>(&mut app);
            if b.riding == Some(han) {
                on = true;
                break;
            }
            if b.on_ground {
                break;
            }
        }
        LEFT.release(app.world_mut());
        if on {
            break;
        }
        step(&mut app, 0.3);
    }
    if !on {
        return Err(format!("never got onto Han (Nat at {}, Han at {})", pos::<Player>(&mut app), pos::<Han>(&mut app)));
    }
    step(&mut app, 0.2);
    RIGHT.press(app.world_mut());
    tap(&mut app);
    step(&mut app, toot);
    tap(&mut app);
    for _ in 0..300 {
        app.update();
        let b = body::<Player>(&mut app);
        if (b.on_ground && b.riding.is_none()) || deaths(&app) > 0 {
            break;
        }
    }
    let p = pos::<Player>(&mut app);
    if deaths(&app) > 0 {
        return Err(format!("fell at {p}"));
    }
    if p.x <= (m.c1 as f32 - 5.0) * TILE {
        return Err(format!("not across at {p}"));
    }
    Ok(())
}

#[test]
fn buddy_raft_pools_in_the_campaign() {
    let ms = marks(Topic::BuddyRaft);
    assert!(ms.len() >= 2);
    for (what, level, m) in ms {
        play_raft_pool(&what, &level, m);
    }
}

#[test]
fn free_play_buddy_raft_pools() {
    for (what, level, m) in room_marks("buddy raft pool", Topic::BuddyRaft) {
        play_raft_pool(&what, &level, m);
    }
}

fn play_raft_pool(what: &str, level: &Level, m: GateMark) {
    let mut app = app(level);
    // The walk row: the first row of the mark with sewage under it.
    let liquid = |c: i32, r: i32| level.tile(c, r) == nat_han_adventures::level::Tile::Liquid;
    let r = (m.r0..=m.r1).find(|&r| (m.c0..=m.c1).any(|c| liquid(c, r + 1) && !liquid(c, r))).expect("a pool");
    let shore = (m.c0..=m.c1).find(|&c| level.tile(c + 1, r + 1) == nat_han_adventures::level::Tile::Liquid).unwrap();
    let far = (shore + 1..).find(|&c| level.tile(c, r + 1) != nat_han_adventures::level::Tile::Liquid).unwrap();
    put::<Player>(&mut app, stand(level, shore - 2, r));
    wait_for_han(&mut app, what);
    put::<Player>(&mut app, stand(level, shore, r));
    step(&mut app, 0.1);
    RIGHT.press(app.world_mut());
    step(&mut app, 0.05);
    RIGHT.release(app.world_mut());
    let mut rafts = 0;
    // Each time Han's made a raft, walk to its end and wait for the next one.
    for _ in 0..(60.0 / DT) as usize {
        app.update();
        let p = pos::<Player>(&mut app);
        if std::env::var("HAN_TRACE").is_ok() {
            let h = pos::<Han>(&mut app);
            let e = single::<Han>(&mut app);
            let mode = app.world().get::<HanBrain>(e).unwrap().mode;
            eprintln!("{what}: nat {p} han {h} {mode:?} deaths {}", deaths(&app));
        }
        if p.x > (far as f32 + 0.5) * TILE {
            break;
        }
        // How far right is safe: the shore, then the rafts end to end.
        let mut safe = ((shore as f32 + 1.0) * TILE - 6.0).max(p.x);
        let mut platforms: Vec<(f32, f32)> = app
            .world_mut()
            .query_filtered::<&MovingPlatform, With<HanRaft>>()
            .iter(app.world())
            .map(|mp| (mp.base.x - mp.width as f32 * TILE / 2.0, mp.base.x + mp.width as f32 * TILE / 2.0))
            .collect();
        rafts = rafts.max(platforms.len());
        platforms.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (a, b) in platforms {
            if a <= safe + 7.0 {
                safe = safe.max(b - 6.0);
            }
        }
        if safe >= far as f32 * TILE - 6.0 {
            safe = (far as f32 + 2.0) * TILE;
        }
        if p.x < safe - 4.0 {
            RIGHT.press(app.world_mut());
        } else {
            RIGHT.release(app.world_mut());
        }
    }
    let p = pos::<Player>(&mut app);
    assert!(rafts >= 2, "{what}: Han made {rafts} rafts");
    assert_eq!(deaths(&app), 0, "{what}: sank at {p}");
    assert!(p.x > (far as f32 + 0.5) * TILE, "{what}: across at {p} (far shore col {far})");
}
