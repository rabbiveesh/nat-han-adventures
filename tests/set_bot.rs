//! A bot that plays a level file the way the capture plays a timeline (keyboard messages, 30
//! fps, the music in lockstep), and writes the timeline of what it pressed: a first draft of a
//! tour for a generated level (free play's sets: `examples/room_check.rs` writes one as a
//! level file). `BOT_LEVEL=<file> BOT_OUT=<timeline> [BOT_CALL=<col>] [BOT_TRACE=1]
//! cargo test --features capture --test set_bot -- --ignored --nocapture`. Holds Right; jumps
//! at pit edges and walls (or walks off when only that lands); toots as late as still lands
//! (the later the farther), predicting the fall tile by tile; presses Down (the call) at column
//! `BOT_CALL`. Check the result with `tests/showcase.rs` (`SHOWCASE_LEVEL`/`SHOWCASE_TOUR`).
#![cfg(feature = "capture")]

use bevy::{input::InputPlugin, input::keyboard::KeyboardInput, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use nat_han_adventures::{
    audio::{self, capture::AudioCapture},
    capture::{frame_duration, timeline::keyboard_input},
    events::PlayerDied,
    game::{Body, Player, Pos},
    level::{Level, Levels, TILE, Tile},
    state::AppState,
};

const FPS: u32 = 30;

#[derive(Resource)]
struct Bot {
    level: Level,
    frame: u64,
    keys: Vec<(KeyCode, bool)>,
    log: Vec<(u64, KeyCode, bool)>,
    space_since: Option<u64>,
    tooted: bool,
    call: Option<f32>,
    called: bool,
    deaths: Vec<(u64, f32, f32)>,
    trace: bool,
    takeoff: i32,
    gravity: f32,
}

impl Bot {
    fn set(&mut self, key: KeyCode, down: bool, out: &mut MessageWriter<KeyboardInput>) {
        let held = self.keys.iter().any(|&(k, d)| k == key && d);
        if held == down {
            return;
        }
        self.keys.retain(|&(k, _)| k != key);
        self.keys.push((key, down));
        self.log.push((self.frame, key, down));
        out.write(keyboard_input(key, down, Entity::PLACEHOLDER));
    }

    fn standable(&self, col: i32, row: i32) -> bool {
        matches!(
            self.level.tile(col, row),
            Tile::Solid | Tile::OneWay | Tile::Crumble | Tile::Trampoline | Tile::BeltLeft | Tile::BeltRight | Tile::Grease
        )
    }

    /// Ground under column `col` within `depth` rows below standing row `row`.
    fn ground_below(&self, col: i32, row: i32, depth: i32) -> bool {
        for r in row + 1..=row + depth {
            let t = self.level.tile(col, r);
            if t.is_deadly() {
                return false;
            }
            if self.standable(col, r) {
                return true;
            }
        }
        false
    }
}

/// Where a fall from `pos` at `vel` (no input but holding Right) ends: `true` on something to
/// stand on no lower than row `floor`, `false` in something deadly, off the bottom, against a
/// wall, or down a shaft.
fn lands_safe(b: &Bot, pos: Vec2, vel: Vec2, half: Vec2, floor: i32) -> bool {
    lands_safe_tooting(b, pos, vel, half, floor, None)
}

/// [`lands_safe`], tooting after `toot` seconds.
fn lands_safe_tooting(b: &Bot, pos: Vec2, vel: Vec2, half: Vec2, floor: i32, toot: Option<f32>) -> bool {
    let rows = b.level.height as f32;
    let (mut p, mut v) = (pos, vel);
    let dt = 1.0 / 120.0;
    for step in 0..360 {
        if toot.is_some_and(|t| (step as f32 * dt - t).abs() < dt / 2.0) {
            v.y = 330.0;
        }
        v.y = (v.y - b.gravity * dt).max(-420.0);
        let feet_before = rows - (p.y - half.y) / TILE;
        p += v * dt;
        let feet = rows - (p.y - half.y) / TILE;
        if feet >= rows {
            return false;
        }
        let (l, r) = (((p.x - half.x) / TILE + 0.02).floor() as i32, ((p.x + half.x) / TILE - 0.02).floor() as i32);
        let (top, bottom) = ((rows - (p.y + half.y) / TILE).floor() as i32, (feet - 0.02).floor() as i32);
        for col in l..=r {
            for row in top..=bottom {
                let t = b.level.tile(col, row);
                if t.is_deadly() {
                    return false;
                }
                if t.is_solid() && row < bottom {
                    return false;
                }
            }
        }
        // Crossing a tile's top on the way down.
        if v.y < 0.0 && feet.floor() > feet_before.floor() {
            let row = feet.floor() as i32;
            for col in l..=r {
                let t = b.level.tile(col, row);
                if t.is_deadly() {
                    return false;
                }
                if b.standable(col, row) {
                    return row <= floor;
                }
            }
        }
    }
    false
}

fn drive(mut bot: ResMut<Bot>, nat: Query<(&Pos, &Body), With<Player>>, mut out: MessageWriter<KeyboardInput>) {
    let b = &mut *bot;
    // (A beat to start, as the tours do: the game takes a moment to take keys.)
    let Ok((pos, body)) = nat.single().map_err(drop).and_then(|n| if b.frame < 9 { Err(()) } else { Ok(n) }) else {
        b.frame += 1;
        return;
    };
    let rows = b.level.height as f32;
    let x = pos.0.x / TILE;
    // The row Nat stands in (his feet's).
    let feet = rows - (pos.0.y - body.half.y) / TILE;
    let stand = (feet - 0.5).floor() as i32;
    let front = (x + body.half.x / TILE + 0.05).floor() as i32;
    b.set(KeyCode::ArrowRight, true, &mut out);
    if let Some(c) = b.call
        && !b.called
        && x >= c
    {
        b.called = true;
        b.set(KeyCode::ArrowDown, true, &mut out);
    } else {
        b.set(KeyCode::ArrowDown, false, &mut out);
    }
    if b.trace && b.frame % 6 == 0 {
        eprintln!(
            "t={:5.2} col {:6.2} row {:6.2} ground {} vel {:4.0},{:4.0}",
            b.frame as f32 / FPS as f32,
            x,
            feet,
            body.on_ground,
            body.vel.x,
            body.vel.y
        );
    }
    if body.on_ground {
        b.tooted = false;
        b.takeoff = stand;
        if let Some(s) = b.space_since {
            // Let go of a jump that hasn't left the ground yet (and don't press again this
            // frame: a press the frame after reads as a buffered toot).
            if b.frame - s >= 2 {
                b.space_since = None;
                b.set(KeyCode::Space, false, &mut out);
            }
            b.frame += 1;
            return;
        }
        // A pit or a wall (or something deadly) right ahead: jump.
        let pit = !b.ground_below(front + 1, stand, 3);
        let wall = b.level.tile(front + 1, stand).is_solid() || b.level.tile(front + 1, stand - 1).is_solid();
        let deadly = (stand..=stand + 1).any(|r| b.level.tile(front + 1, r).is_deadly());
        // (Over a pit: walk off instead when only that lands somewhere, e.g. off an awning
        // onto the roof below it.)
        let off = Vec2::new((front + 1) as f32 * TILE + body.half.x + 0.5, pos.0.y);
        let walk_instead = pit
            && !wall
            && !deadly
            && !lands_safe(b, pos.0, Vec2::new(150.0, 380.0), body.half, stand + 4)
            && lands_safe(b, off, Vec2::new(150.0, 0.0), body.half, stand + 4);
        if (pit || wall || deadly) && !walk_instead {
            b.space_since = Some(b.frame);
            b.set(KeyCode::Space, true, &mut out);
        }
    } else {
        // In the air: hold the jump to its top; then toot when the fall ends badly.
        if b.space_since.is_some() && body.vel.y <= 0.0 {
            b.space_since = None;
            b.set(KeyCode::Space, false, &mut out);
        } else if body.vel.y <= 0.0 && !b.tooted && b.space_since.is_none() {
            let vel = Vec2::new(body.vel.x.max(150.0), body.vel.y);
            let floor = b.takeoff + 4;
            let safe = lands_safe(b, pos.0, vel, body.half, floor);
            // Toot as late as still works (the later, the farther), or at the takeoff's height
            // when nothing works.
            let now = lands_safe_tooting(b, pos.0, vel, body.half, floor, Some(0.0));
            let later = lands_safe_tooting(b, pos.0, vel, body.half, floor, Some(2.0 / FPS as f32));
            let low = feet >= b.takeoff as f32 + 1.0;
            if b.trace {
                eprintln!("  falling col {x:.2} feet {feet:.2} safe {safe} toot now {now} later {later}");
            }
            if !safe && ((now && !later) || (!now && !later && low)) {
                b.tooted = true;
                b.space_since = Some(b.frame);
                b.set(KeyCode::Space, true, &mut out);
            }
        }
    }
    b.frame += 1;
}

#[test]
#[ignore]
fn bot_plays_a_level() {
    let Ok(path) = std::env::var("BOT_LEVEL") else { return };
    let level = Level::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let secs: f32 = std::env::var("BOT_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(70.0);
    let wav = std::env::temp_dir().join(format!("nathan-bot-{}.wav", std::process::id()));
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin, InputPlugin, nat_han_adventures::gameplay))
        .insert_resource(TimeUpdateStrategy::ManualDuration(frame_duration(FPS).unwrap()))
        .insert_resource(Levels(vec![level.clone()]));
    app.add_plugins(audio::plugin).insert_resource(AudioCapture::new(&wav, FPS).unwrap());
    app.finish();
    app.cleanup();
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::Playing);
    app.update();
    app.world_mut().resource_mut::<AudioCapture>().recording = true;
    let call = std::env::var("BOT_CALL").ok().and_then(|c| c.parse().ok());
    app.insert_resource(Bot {
        level: level.clone(),
        frame: 0,
        keys: Vec::new(),
        log: Vec::new(),
        space_since: None,
        tooted: false,
        call,
        called: false,
        deaths: Vec::new(),
        trace: std::env::var("BOT_TRACE").is_ok(),
        takeoff: 0,
        gravity: nat_han_adventures::game::tuning::GRAVITY,
    })
    .add_systems(PreUpdate, drive.before(bevy::input::InputSystems))
    .add_systems(Last, |mut b: ResMut<Bot>, mut r: MessageReader<PlayerDied>| {
        let rows = b.level.height as f32;
        let f = b.frame;
        let ds: Vec<_> = r.read().map(|m| (f, m.pos.x / TILE, rows - m.pos.y / TILE)).collect();
        b.deaths.extend(ds);
    });
    let mut goal = None;
    for frame in 0..(secs * FPS as f32) as u64 {
        app.update();
        let state = app.world().resource::<State<AppState>>().get().clone();
        if state == AppState::LevelComplete {
            goal = Some(frame);
            break;
        }
    }
    let _ = std::fs::remove_file(&wav);
    let bot = app.world().resource::<Bot>();
    let mut out = String::from("# Written by tests/set_bot.rs.\n");
    for &(f, k, d) in &bot.log {
        let name = match k {
            KeyCode::ArrowRight => "Right",
            KeyCode::ArrowDown => "Down",
            _ => "Space",
        };
        out += &format!("at {f}f\n{} {name}\n", if d { "press" } else { "release" });
    }
    let end = goal.map_or(bot.frame, |g| g + FPS as u64 * 2);
    out += &format!("at {end}f\nrelease Right\n");
    if let Ok(p) = std::env::var("BOT_OUT") {
        std::fs::write(p, &out).unwrap();
    }
    eprintln!(
        "goal: {:?} s, deaths (s, col, row): {:?}",
        goal.map(|g| g as f32 / FPS as f32),
        bot.deaths.iter().map(|&(f, c, r)| (f as f32 / FPS as f32, c, r)).collect::<Vec<_>>()
    );
}
