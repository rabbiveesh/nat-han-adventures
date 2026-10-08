//! The campaign's Han gates, played headless in the real levels with the real Han: each
//! buddy ledge, shield row, chain chasm and buddy raft pool (found by its `gate:` mark).

use std::time::Duration;

use bevy::{input::InputPlugin, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use leafwing_input_manager::prelude::*;
use nat_han_adventures::{
    events::PlayerDied,
    game::*,
    level::{LEVEL_SOURCES, Level, Levels, TILE, Topic},
    state::AppState,
};

const DT: f64 = 1.0 / 60.0;
const JUMP: KeyCode = KeyCode::KeyZ;
const RIGHT: KeyCode = KeyCode::ArrowRight;

#[derive(Resource, Default)]
struct Deaths(usize);

fn app(src: &str) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin, InputPlugin))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(DT)))
        .add_plugins(nat_han_adventures::gameplay)
        .insert_resource(Levels(vec![Level::parse(src).unwrap()]))
        .insert_resource(AssistMode::Manual)
        .init_resource::<Deaths>()
        .add_systems(Last, |mut d: ResMut<Deaths>, mut r: MessageReader<PlayerDied>| d.0 += r.read().count());
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
fn wait_for_han(app: &mut App, i: usize) {
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
    panic!("level {}: Han never came: {:?} at {at}", i + 1, app.world().get::<HanBrain>(h).unwrap().mode);
}

fn deaths(app: &App) -> usize {
    app.world().resource::<Deaths>().0
}

fn stand(level: &Level, c: i32, r: i32) -> Vec2 {
    nat_han_adventures::level::buddy::stand(level, (c, r))
}

/// Every Han gate mark in the campaign: (level index, mark).
fn marks(topic: Topic) -> Vec<(usize, nat_han_adventures::level::GateMark)> {
    LEVEL_SOURCES
        .iter()
        .enumerate()
        .flat_map(|(i, s)| Level::parse(s).unwrap().gates.into_iter().filter(move |g| g.topic == topic).map(move |g| (i, g)))
        .collect()
}

#[test]
fn buddy_ledges_in_the_campaign() {
    let ms = marks(Topic::Boost);
    assert!(ms.len() >= 3);
    for (i, m) in ms {
        let level = Level::parse(LEVEL_SOURCES[i]).unwrap();
        let mut app = app(LEVEL_SOURCES[i]);
        // The mesa: the first column in the mark solid at Nat's height.
        let r = m.r1;
        let face = (m.c0..=m.c1).find(|&c| level.tile(c, r).is_solid()).expect("a ledge face");
        put::<Player>(&mut app, stand(&level, face - 2, r));
        wait_for_han(&mut app, i);
        // Hop onto Han's head (he's in his slot behind), and boost, toot near the top.
        let h = pos::<Han>(&mut app);
        put::<Player>(&mut app, h + Vec2::new(0.0, 1.5 * TILE));
        step(&mut app, 0.4);
        assert_eq!(body::<Player>(&mut app).riding, Some(single::<Han>(&mut app)), "level {}: on Han", i + 1);
        RIGHT.press(app.world_mut());
        tap(&mut app);
        step(&mut app, 0.42);
        tap(&mut app);
        step(&mut app, 0.6);
        RIGHT.release(app.world_mut());
        step(&mut app, 1.0);
        let p = pos::<Player>(&mut app);
        let top = level.tile_center(face as usize, (r - 9) as usize).y - TILE / 2.0;
        assert!(p.y > top && body::<Player>(&mut app).on_ground, "level {}: on top of the ledge at {p} (top {top})", i + 1);
        assert_eq!(deaths(&app), 0);
    }
}

#[test]
fn shield_rows_in_the_campaign() {
    let ms = marks(Topic::Shield);
    assert!(ms.len() >= 2);
    for (i, m) in ms {
        let level = Level::parse(LEVEL_SOURCES[i]).unwrap();
        let mut app = app(LEVEL_SOURCES[i]);
        put::<Player>(&mut app, stand(&level, m.c0, m.r0));
        wait_for_han(&mut app, i);
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
        assert!(ahead, "level {}: Han went ahead", i + 1);
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
        assert_eq!(deaths(&app), 0, "level {}: sprayed at {p}", i + 1);
        assert!(p.x > end, "level {}: through at {p}", i + 1);
    }
}

#[test]
fn chain_chasms_in_the_campaign() {
    let ms = marks(Topic::Chain);
    assert!(ms.len() >= 2);
    for (i, m) in ms {
        let level = Level::parse(LEVEL_SOURCES[i]).unwrap();
        let mut app = app(LEVEL_SOURCES[i]);
        let r = m.r0;
        let edge = (m.c0..=m.c1).find(|&c| !level.tile(c, r + 1).is_solid()).expect("a chasm") - 1;
        put::<Player>(&mut app, stand(&level, edge - 4, r));
        wait_for_han(&mut app, i);
        RIGHT.press(app.world_mut());
        while pos::<Player>(&mut app).x < (edge as f32 + 1.0) * TILE - 6.0 {
            app.update();
        }
        tap(&mut app);
        step(&mut app, 0.32);
        tap(&mut app);
        let han = single::<Han>(&mut app);
        let mut boosted = false;
        for _ in 0..300 {
            app.update();
            if std::env::var("HAN_TRACE").is_ok() {
                let (n, h) = (pos::<Player>(&mut app), pos::<Han>(&mut app));
                let (nb, hb) = (body::<Player>(&mut app), body::<Han>(&mut app));
                let mode = app.world().get::<HanBrain>(han).unwrap().mode;
                eprintln!("nat {n} {} han {h} {} {mode:?}", nb.vel, hb.vel);
            }
            if !boosted && body::<Player>(&mut app).riding == Some(han) {
                tap(&mut app);
                boosted = true;
                step(&mut app, 0.4);
                tap(&mut app);
            }
            let b = body::<Player>(&mut app);
            if b.on_ground && b.riding.is_none() {
                break;
            }
        }
        let p = pos::<Player>(&mut app);
        assert!(boosted, "level {}: landed on Han mid-air", i + 1);
        assert_eq!(deaths(&app), 0, "level {}: fell at {p}", i + 1);
        assert!(p.x > (m.c1 as f32 - 5.0) * TILE, "level {}: across at {p}", i + 1);
    }
}

#[test]
fn buddy_raft_pools_in_the_campaign() {
    let ms = marks(Topic::BuddyRaft);
    assert!(ms.len() >= 2);
    for (i, m) in ms {
        let level = Level::parse(LEVEL_SOURCES[i]).unwrap();
        let mut app = app(LEVEL_SOURCES[i]);
        let r = m.r0;
        let shore = (m.c0..=m.c1).find(|&c| level.tile(c + 1, r + 1) == nat_han_adventures::level::Tile::Liquid).unwrap();
        let far = (shore + 1..).find(|&c| level.tile(c, r + 1) != nat_han_adventures::level::Tile::Liquid).unwrap();
        put::<Player>(&mut app, stand(&level, shore - 2, r));
        wait_for_han(&mut app, i);
        put::<Player>(&mut app, stand(&level, shore, r));
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
                eprintln!("L{} nat {p} han {h} {mode:?} deaths {}", i + 1, deaths(&app));
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
        assert!(rafts >= 2, "level {}: Han made {rafts} rafts", i + 1);
        assert_eq!(deaths(&app), 0, "level {}: sank at {p}", i + 1);
        assert!(p.x > (far as f32 + 0.5) * TILE, "level {}: across at {p} (far shore col {far})", i + 1);
    }
}
