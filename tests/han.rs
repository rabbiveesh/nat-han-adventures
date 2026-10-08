//! Han the AI buddy, headless: the real `gameplay` plugins on `MinimalPlugins` at 60 Hz (one
//! step per update), each test with its own small level.

use std::time::Duration;

use bevy::{input::InputPlugin, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use leafwing_input_manager::prelude::*;
use nat_han_adventures::{
    audio::{Filters, Harmony},
    events::*,
    game::*,
    level::{Level, Levels, TILE, Topic},
    state::AppState,
};

const DT: f64 = 1.0 / 60.0;
const JUMP: KeyCode = KeyCode::KeyZ;
const RIGHT: KeyCode = KeyCode::ArrowRight;
const LEFT: KeyCode = KeyCode::ArrowLeft;

#[derive(Resource, Default)]
struct Heard {
    says: Vec<String>,
    jumped: usize,
    boosted: usize,
    died: usize,
}

fn app(level: &str) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin, InputPlugin))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(DT)))
        .add_plugins(nat_han_adventures::gameplay)
        .insert_resource(Levels(vec![Level::parse(level).expect("test level parses")]))
        .init_resource::<Heard>()
        // Tests set Han's eagerness themselves: the adaptive engine stays out.
        .insert_resource(AssistMode::Manual)
        .add_systems(
            Last,
            |mut h: ResMut<Heard>,
             mut s: MessageReader<HanSays>,
             mut j: MessageReader<Jumped>,
             mut b: MessageReader<HanBoosted>,
             mut d: MessageReader<PlayerDied>| {
                h.says.extend(s.read().map(|m| m.text.clone()));
                h.jumped += j.read().count();
                h.boosted += b.read().count();
                h.died += d.read().count();
            },
        );
    app.finish();
    app.cleanup();
    app.update();
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::Playing);
    app.update();
    app.update();
    app
}

fn heard(app: &App) -> &Heard {
    app.world().resource::<Heard>()
}

fn said(app: &App, line: &str) -> usize {
    heard(app).says.iter().filter(|s| *s == line).count()
}

fn step(app: &mut App, secs: f32) {
    for _ in 0..(secs as f64 / DT).round() as usize {
        app.update();
        trace(app);
    }
}

/// `HAN_TRACE=1`: print Han's state every step.
fn trace(app: &mut App) {
    if std::env::var("HAN_TRACE").is_err() || app.world_mut().query::<&Han>().iter(app.world()).next().is_none() {
        return;
    }
    let (n, nb, h, hb, b) = (nat(app), nat_body(app), han(app), han_body(app), brain(app));
    eprintln!(
        "t {:6.2} nat {:5.0},{:4.0} v{:4.0},{:4.0} g{} r{} | han {:5.0},{:4.0} v{:4.0},{:4.0} g{} {:?} goal {:?} path {} exec {:?} lost {} toots {}",
        app.world().resource::<SimClock>().time,
        n.x, n.y, nb.vel.x, nb.vel.y, nb.on_ground as u8, nb.riding.is_some() as u8,
        h.x, h.y, hb.vel.x, hb.vel.y, hb.on_ground as u8, b.mode, b.goal, b.path.len(),
        b.exec.map(|e| (e.mv, e.to, e.flying, e.t)), b.lost, b.ctl.toots_left
    );
}

fn press(app: &mut App, k: KeyCode) {
    k.press(app.world_mut());
}

fn release(app: &mut App, k: KeyCode) {
    k.release(app.world_mut());
}

/// Tap jump: a fresh press (release first so it registers as just pressed).
fn tap(app: &mut App) {
    release(app, JUMP);
    app.update();
    trace(app);
    press(app, JUMP);
}

fn single<C: Component>(app: &mut App) -> Entity {
    app.world_mut().query_filtered::<Entity, With<C>>().single(app.world()).unwrap()
}

fn nat(app: &mut App) -> Vec2 {
    let e = single::<Player>(app);
    app.world().get::<Pos>(e).unwrap().0
}

fn nat_body(app: &mut App) -> Body {
    let e = single::<Player>(app);
    app.world().get::<Body>(e).unwrap().clone()
}

fn han(app: &mut App) -> Vec2 {
    let e = single::<Han>(app);
    app.world().get::<Pos>(e).unwrap().0
}

fn han_body(app: &mut App) -> Body {
    let e = single::<Han>(app);
    app.world().get::<Body>(e).unwrap().clone()
}

fn brain(app: &mut App) -> HanBrain {
    let e = single::<Han>(app);
    app.world().get::<HanBrain>(e).unwrap().clone()
}

fn anim(app: &mut App) -> HanAnim {
    let e = single::<Han>(app);
    *app.world().get::<HanAnim>(e).unwrap()
}

fn head(app: &mut App) -> HanHead {
    let e = single::<Han>(app);
    *app.world().get::<HanHead>(e).unwrap()
}

fn put<C: Component>(app: &mut App, p: Vec2) {
    let e = single::<C>(app);
    app.world_mut().get_mut::<Pos>(e).unwrap().0 = p;
    app.world_mut().get_mut::<PrevPos>(e).unwrap().0 = p;
}

fn set_groove(app: &mut App, harmony: Harmony, laughing: bool) {
    app.world_mut().insert_resource(Groove::new(Filters { harmony, just_intonation: laughing }));
}

fn eagerness(app: &mut App, e: f32) {
    app.world_mut().resource_mut::<Assists>().han_eagerness = e;
}

/// Box center standing on a floor whose top is at `floor_top`.
fn standing(floor_top: f32) -> f32 {
    floor_top + tuning::PLAYER_SIZE.1 / 2.0
}

/// A level from rows (padded with sky to 14 rows above a floor).
fn level(header: &str, rows: &[&str]) -> String {
    format!("name: T\n{header}---\n{}\n", rows.join("\n"))
}

const FLAT: &str = "name: Flat
---
............................................................
............................................................
............................................................
............................................................
............................................................
............................................................
............................................................
..P........................................................G
############################################################
";

// --- Following ------------------------------------------------------------------------------

/// Steps up (1, 2 and 4 tiles): Han gets up them on his own routes (his jump moves and toots),
/// not by replaying Nat.
const STEPS: &str = "name: Steps
---
............................................................
............................................................
............................................................
............................................................
..........................................######............
..........................................######............
...............................#####......######............
.P..................####.......#####......######...........G
############################################################
";

#[test]
fn han_follows_on_his_own_routes_and_lands_properly() {
    let mut app = app(STEPS);
    press(&mut app, RIGHT);
    let mut used_moves = false;
    let mut max_gap: f32 = 0.0;
    let mut toot_at = None;
    for _ in 0..900 {
        // Nat jumps when he bumps into something, and toots on the way up.
        let nb = nat_body(&mut app);
        if nb.on_ground && nb.vel.x.abs() < 10.0 && toot_at.is_none() {
            tap(&mut app);
            toot_at = Some(18);
        }
        if let Some(k) = toot_at.as_mut() {
            *k -= 1;
            if *k == 0 {
                tap(&mut app);
                toot_at = None;
            }
        }
        app.update();
        trace(&mut app);
        let b = brain(&mut app);
        used_moves |= b.exec.is_some_and(|e| e.flying);
        max_gap = max_gap.max(han(&mut app).distance(nat(&mut app)));
        if nat(&mut app).x > 54.0 * TILE {
            break;
        }
    }
    release(&mut app, RIGHT);
    release(&mut app, JUMP);
    step(&mut app, 3.0);
    let (p, h) = (nat(&mut app), han(&mut app));
    assert_eq!(heard(&app).died, 0);
    assert!(used_moves, "Han jumped on his own route");
    assert_eq!(brain(&mut app).drops, 0, "no parachute needed");
    assert!(max_gap < 12.0 * TILE, "kept up: {max_gap}");
    assert!(han_body(&mut app).on_ground, "standing at {h}");
    assert!((h.y - p.y).abs() < 1.0 && (p.x - h.x) > 0.5 * TILE && (p.x - h.x) < 3.0 * TILE, "in his slot: {h} vs {p}");
}

/// Walking and stopping, back and forth: Han never stands inside Nat.
#[test]
fn han_never_overlaps_nat_awkwardly() {
    let mut app = app(FLAT);
    for (key, secs) in [(RIGHT, 1.2), (LEFT, 0.6), (RIGHT, 2.0), (LEFT, 1.5)] {
        press(&mut app, key);
        step(&mut app, secs);
        release(&mut app, key);
        step(&mut app, 1.5);
        let (p, h) = (nat(&mut app), han(&mut app));
        assert!((p.x - h.x).abs() >= 12.0, "han at {h}, nat at {p}: overlapping");
        assert!((p.x - h.x).abs() < 3.0 * TILE, "han at {h}, nat at {p}: lagging");
    }
}

const PLATFORM: &str = "name: Ferry
1: dx=6 dy=0 period=4 kind=tp
---
............................................................
............................................................
............................................................
............................................................
............................................................
............................................................
............................................................
..P..............111.......................................G
################.........###################################
";

/// A ferry across a gap: Han waits for the platform, rides it, gets off.
#[test]
fn han_rides_moving_platforms() {
    let mut app = app(PLATFORM);
    let mut rode = false;
    // Nat rides across himself (positions put him on the far side), Han follows.
    put::<Player>(&mut app, Vec2::new(30.0 * TILE, standing(TILE)));
    for _ in 0..900 {
        app.update();
        trace(&mut app);
        rode |= han_body(&mut app).riding.is_some();
    }
    let h = han(&mut app);
    assert!(rode, "Han rode the platform");
    assert_eq!(brain(&mut app).drops, 0, "no parachute: {:?}", brain(&mut app).mode);
    assert!(h.x > 25.0 * TILE && han_body(&mut app).on_ground, "across: {h}");
}

// --- The plunger boost ------------------------------------------------------------------------

/// Drop Nat on Han's head, press jump: launched ~7 tiles above Han's head, toot ready.
#[test]
fn boost_launches_and_refreshes_the_toot() {
    let mut app = app(FLAT);
    step(&mut app, 1.0);
    let h = han(&mut app);
    put::<Player>(&mut app, h + Vec2::new(0.0, 2.0 * TILE));
    step(&mut app, 0.5);
    let p = single::<Player>(&mut app);
    assert_eq!(nat_body(&mut app).riding, Some(single::<Han>(&mut app)), "standing on Han's head");
    assert_eq!(anim(&mut app).pose, HanPose::Braced, "braced");
    let jumps = heard(&app).jumped;
    // Use the toot first so the boost visibly refreshes it.
    app.world_mut().get_mut::<PlayerControl>(p).unwrap().has_toot = false;
    let feet0 = nat(&mut app).y - 7.0;
    tap(&mut app);
    let mut top: f32 = 0.0;
    for i in 0..60 {
        app.update();
        trace(&mut app);
        top = top.max(nat(&mut app).y - 7.0);
        if i == 2 {
            assert!(app.world().get::<PlayerControl>(p).unwrap().has_toot, "the boost refreshes the toot");
        }
    }
    assert_eq!(heard(&app).boosted, 1);
    assert_eq!(heard(&app).jumped, jumps, "a boost isn't a Jumped: the band doesn't count it");
    let rise = (top - feet0) / TILE;
    assert!((6.5..7.5).contains(&rise), "launched {rise} tiles");
}

/// Boost + toot clears a 9-tile buddy ledge; a Giant Steps double jump can't (validator), and
/// the game agrees.
#[test]
fn boost_and_toot_climb_a_buddy_ledge() {
    let lvl = level(
        "",
        &[
            "....................................",
            "....................................",
            "....................................",
            "....................................",
            "....................................",
            ".........................###########",
            ".........................###########",
            ".........................###########",
            ".........................###########",
            ".........................###########",
            ".........................###########",
            ".........................###########",
            ".........................###########",
            ".P.......................##########G",
            "####################################",
        ],
    );
    let mut app = app(&lvl);
    // Han right at the foot of the ledge, Nat on him.
    put::<Han>(&mut app, Vec2::new(24.0 * TILE + 8.0, standing(TILE)));
    step(&mut app, 0.2);
    let h = han(&mut app);
    put::<Player>(&mut app, h + Vec2::new(0.0, 2.0 * TILE));
    step(&mut app, 0.5);
    press(&mut app, RIGHT);
    tap(&mut app);
    step(&mut app, 0.42);
    tap(&mut app); // the toot near the top
    step(&mut app, 1.5);
    let p = nat(&mut app);
    assert!(p.y > standing(9.0 * TILE) - 1.0 && p.x > 25.0 * TILE, "on the ledge: {p}");
}

// --- Intercept: a mid-air chain -----------------------------------------------------------------

/// A 16-tile chasm (marked: no overuse limit): run, jump at the edge, toot; Han dives under
/// Nat; jump off his head (the boost), toot; land on the far side.
#[test]
fn mid_air_intercept_chain_crosses_a_chasm() {
    let mut rows = vec![".".repeat(70); 14];
    rows.push(format!("{}{}{}", "#".repeat(30), ".".repeat(16), "#".repeat(24)));
    rows[13].replace_range(2..3, "P");
    rows[13].replace_range(66..67, "G");
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let lvl = level("gate: chain 28,13 49,13\n", &rows);
    let mut app = app(&lvl);
    step(&mut app, 0.5);
    press(&mut app, RIGHT);
    // Run to the edge.
    while nat(&mut app).x < 30.0 * TILE - 6.0 {
        app.update();
        trace(&mut app);
    }
    tap(&mut app);
    step(&mut app, 0.32);
    tap(&mut app); // toot
    let han_e = single::<Han>(&mut app);
    let mut boosted = false;
    let mut chased = false;
    for _ in 0..240 {
        app.update();
        trace(&mut app);
        chased |= brain(&mut app).mode == HanMode::Intercept;
        if !boosted && nat_body(&mut app).riding == Some(han_e) {
            tap(&mut app); // boost
            boosted = true;
            step(&mut app, 0.4);
            tap(&mut app); // toot
        }
        if nat_body(&mut app).on_ground && nat_body(&mut app).riding.is_none() {
            break;
        }
    }
    let p = nat(&mut app);
    assert!(chased, "Han went for the intercept");
    assert!(boosted, "Nat landed on Han in mid-air");
    assert_eq!(heard(&app).died, 0);
    assert!(p.x > 46.0 * TILE, "across the chasm: {p}");
}

// --- Lemme check that ----------------------------------------------------------------------------

/// A shield row: 24 adjacent cans under a grating, low ceiling.
fn shield_level() -> String {
    shield_level_on(true)
}

/// A shield row of 24 adjacent cans with a low ceiling: under a grating (`grated`), or on
/// the floor Nat walks on (the cans themselves deadly while they fire).
fn shield_level_on(grated: bool) -> String {
    let w = 80;
    let mut rows: Vec<Vec<char>> = vec![vec!['.'; w]; 14];
    for r in 10..14 {
        for c in 0..w {
            rows[r][c] = '#';
        }
    }
    for c in 30..54 {
        for r in 0..=7 {
            rows[r][c] = '#';
        }
        if grated {
            rows[10][c] = '=';
            rows[11][c] = 'S';
        } else {
            rows[9][c] = 'S';
        }
    }
    rows[9][2] = 'P';
    rows[9][w - 3] = 'G';
    let rows: Vec<String> = rows.iter().map(|r| r.iter().collect()).collect();
    format!("name: Shield\ngate: shield 29,9 54,9\n---\n{}\n", rows.join("\n"))
}

#[test]
fn han_goes_ahead_and_shields_nat_from_sprays() {
    shields_nat(&shield_level());
}

/// Han plugs the cans themselves too: a shield row on the floor, walked behind him.
#[test]
fn han_shields_nat_from_floor_cans() {
    shields_nat(&shield_level_on(false));
}

fn shields_nat(level: &str) {
    let mut app = app(level);
    // Walk up to the row and stand facing it.
    press(&mut app, RIGHT);
    while nat(&mut app).x < 28.0 * TILE {
        app.update();
        trace(&mut app);
    }
    release(&mut app, RIGHT);
    let t0 = app.world().resource::<SimClock>().time;
    while !matches!(brain(&mut app).mode, HanMode::Ahead { .. }) {
        app.update();
        trace(&mut app);
        assert!(app.world().resource::<SimClock>().time - t0 < 3.0, "Han never went ahead");
    }
    let waited = app.world().resource::<SimClock>().time - t0;
    assert!((go_ahead_delay(0.5) - 0.2..go_ahead_delay(0.5) + 0.6).contains(&waited), "went after {waited}s");
    assert_eq!(said(&app, LEMME_LINE), 1);
    assert_eq!(anim(&mut app).pose, HanPose::March);
    // Walk right behind him (he's solid from behind while he marches).
    step(&mut app, 0.4);
    press(&mut app, RIGHT);
    let mut plugged = 0;
    while nat(&mut app).x < 56.0 * TILE {
        app.update();
        trace(&mut app);
        plugged = plugged.max(app.world_mut().query::<&SprayPlug>().iter(app.world()).filter(|p| p.linger > 0.0).count());
        let at = nat(&mut app);
        assert_eq!(heard(&app).died, 0, "Nat got sprayed at {at}");
        assert!(app.world().resource::<SimClock>().time - t0 < 20.0, "stuck at {}", nat(&mut app));
    }
    assert!(plugged > 0, "jets were plugged");
    assert!(nat(&mut app).x - han(&mut app).x < 2.0 * TILE, "Nat stayed behind Han");
}

#[test]
fn without_han_the_shield_row_sprays_nat() {
    sprays_nat_alone(&shield_level());
}

#[test]
fn without_han_floor_cans_spray_nat() {
    sprays_nat_alone(&shield_level_on(false));
}

fn sprays_nat_alone(level: &str) {
    let mut app = app(level);
    // Han out of the way (far behind, and lazy).
    eagerness(&mut app, 0.0);
    press(&mut app, RIGHT);
    for _ in 0..600 {
        app.update();
        trace(&mut app);
        if heard(&app).died > 0 {
            break;
        }
    }
    assert!(heard(&app).died > 0, "running through alone gets you sprayed");
}

const FLIES: &str = "name: Flies
---
............................................................
............................................................
............................................................
............................................................
............................................................
............................................................
.............F..............................................
..P........................................................G
############################################################
";

#[test]
fn flies_bounce_off_han() {
    let mut app = app(FLIES);
    // Stand facing the swarm; Han goes to check it out.
    step(&mut app, 0.5);
    press(&mut app, RIGHT);
    while nat(&mut app).x < 10.0 * TILE {
        app.update();
    }
    release(&mut app, RIGHT);
    step(&mut app, 4.0);
    let spins: Vec<FlySpin> = app.world_mut().query::<&FlySpin>().iter(app.world()).copied().collect();
    assert!(spins.iter().any(|s| s.dir < 0.0 || s.offset != 0.0), "a fly bounced: {spins:?}");
    assert!(said(&app, FLY_LINE) >= 1);
    assert_eq!(heard(&app).died, 0);
}

const SEWER: &str = "name: Sewer
---
............................................................
............................................................
............................................................
............................................................
............................................................
............................................................
............................................................
..P........................................................G
##########~~~~~~~~~~~~~~#####################################
##########~~~~~~~~~~~~~~#####################################
";

/// Stand facing a pool: Han wades in, splats, leaves a big raft, parachutes back. Not a death.
#[test]
fn han_makes_a_big_raft_in_sewage() {
    let mut app = app(SEWER);
    press(&mut app, RIGHT);
    while nat(&mut app).x < 9.0 * TILE {
        app.update();
        trace(&mut app);
    }
    release(&mut app, RIGHT);
    let mut sank = false;
    for _ in 0..240 {
        app.update();
        trace(&mut app);
        sank |= matches!(brain(&mut app).mode, HanMode::Sinking { .. });
        if sank {
            break;
        }
    }
    assert!(sank, "Han waded in: {:?} at {}", brain(&mut app).mode, han(&mut app));
    step(&mut app, 0.2);
    let rafts: Vec<MovingPlatform> =
        app.world_mut().query_filtered::<&MovingPlatform, With<HanRaft>>().iter(app.world()).cloned().collect();
    assert_eq!(rafts.len(), 1, "one big raft");
    assert_eq!(rafts[0].width, HAN_RAFT_WIDTH);
    assert!(rafts[0].base.x > 10.0 * TILE && rafts[0].base.x < 14.0 * TILE, "raft at {}", rafts[0].base);
    assert_eq!(app.world().resource::<LevelRun>().deaths, 0, "not Nat's death");
    assert_eq!(heard(&app).died, 0);
    // Back a few seconds later, on his parachute.
    let mut chute = false;
    for _ in 0..600 {
        app.update();
        trace(&mut app);
        chute |= matches!(brain(&mut app).mode, HanMode::Parachute { .. });
        if chute && brain(&mut app).mode == HanMode::Follow {
            break;
        }
    }
    assert!(chute, "parachuted back");
    assert_eq!(said(&app, PRO_LINE), 1);
    // The raft holds Nat, and outlives a stain raft.
    step(&mut app, RAFT_LIFE_FLOOR + 1.0);
    let n = app.world_mut().query_filtered::<(), With<HanRaft>>().iter(app.world()).count();
    assert!(n >= 1, "Han's raft floats longer than 12 s");
    assert!(han_raft_life(&Assists::default()) >= HAN_RAFT_LIFE_FLOOR);
    let mut a = Assists::default();
    a.raft_life_mult = 2.0;
    assert_eq!(han_raft_life(&a), 2.0 * HAN_RAFT_LIFE_FLOOR);
}

// --- The band zone: Han's boost is feeble near the band's gates --------------------------------

/// A level with a giant wall (marked) at col 50: 7 tiles tall, the goal on top.
fn giant_wall_level() -> String {
    let mut rows = vec![".".repeat(90); 14];
    for r in 7..14 {
        rows[r].replace_range(50..90, &"#".repeat(40));
    }
    rows.push("#".repeat(90));
    rows[13].replace_range(2..3, "P");
    rows[6].replace_range(86..87, "G");
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    level("gate: giant 41,6 52,13\n", &rows)
}

/// Han follows Nat right up to a giant wall: no hanging back, no parachuting in afterwards.
#[test]
fn han_follows_normally_next_to_a_giant_wall() {
    let mut app = app(&giant_wall_level());
    press(&mut app, RIGHT);
    let mut max_gap: f32 = 0.0;
    for _ in 0..420 {
        app.update();
        trace(&mut app);
        max_gap = max_gap.max((nat(&mut app).x - han(&mut app).x).abs());
    }
    release(&mut app, RIGHT);
    step(&mut app, 2.0);
    let (p, h) = (nat(&mut app), han(&mut app));
    assert!(p.x > 48.0 * TILE, "Nat at the wall: {p}");
    assert!(max_gap < 4.0 * TILE, "kept up: {max_gap}");
    assert!((p.x - h.x).abs() < 3.0 * TILE && (p.x - h.x).abs() >= 12.0, "in his slot by the wall: {h} vs {p}");
    assert_eq!(brain(&mut app).drops, 0, "no parachute");
    let cell = (h.x / TILE) as i32;
    assert!(cell > 41 - nat_han_adventures::level::HAN_BERTH, "in the band zone: col {cell}");
    assert!(head(&mut app).weak, "his boost is the weak one there");
}

/// Drop Nat on Han (standing at `col`), jump: how high Nat's feet rise above where they were (tiles).
fn boost_rise(app: &mut App, col: f32) -> f32 {
    put::<Han>(app, Vec2::new(col * TILE + 8.0, standing(TILE)));
    step(app, 0.3);
    let h = han(app);
    put::<Player>(app, h + Vec2::new(0.0, 1.5 * TILE));
    step(app, 0.5);
    assert_eq!(nat_body(app).riding, Some(single::<Han>(app)), "on Han's head");
    let feet0 = nat(app).y - 7.0;
    tap(app);
    let mut top: f32 = 0.0;
    for _ in 0..60 {
        app.update();
        trace(app);
        top = top.max(nat(app).y - 7.0);
    }
    release(app, JUMP);
    step(app, 0.5);
    (top - feet0) / TILE
}

/// Near each kind of band (or death) gate the boost is weak and Han grumbles (a line for the
/// gate's kind first); away from them it's the full one, silently.
#[test]
fn boost_is_weak_near_band_gates_and_han_grumbles() {
    for topic in [Topic::Giant, Topic::Gap, Topic::Waltz, Topic::Grip, Topic::Stain] {
        let lvl = FLAT.replace("name: Flat\n", &format!("name: Flat\ngate: {} 40,6 45,7\n", topic.word()));
        let mut app = app(&lvl);
        step(&mut app, 0.5);
        // In the zone (14 columns from the mark): a hop about a normal jump's height.
        let weak = boost_rise(&mut app, 30.0);
        assert!(head(&mut app).weak, "{topic:?}");
        let expect = WEAK_BOOST_SPEED * WEAK_BOOST_SPEED / (2.0 * tuning::GRAVITY) / TILE;
        assert!((weak - expect).abs() < 0.3, "{topic:?}: weak boost {weak} tiles, expected ~{expect}");
        assert!(weak < 3.2, "{topic:?}: no more than a normal jump");
        assert_eq!(heard(&app).boosted, 1, "{topic:?}");
        let line = grumble_lines(topic)[0];
        assert!(line.len() <= nat_han_adventures::level::MAX_LINE, "{line}");
        assert_eq!(said(&app, line), 1, "{topic:?}: {:?}", heard(&app).says);
        assert_eq!(brain(&mut app).weak_boosts, 1);
        assert_eq!(brain(&mut app).streak, 0, "his back doesn't mind those");
        // Away from it: the full boost, no grumbling.
        let says = heard(&app).says.len();
        let full = boost_rise(&mut app, 8.0);
        assert!(!head(&mut app).weak);
        assert!((6.5..7.5).contains(&full), "{topic:?}: full boost {full} tiles");
        assert_eq!(heard(&app).says.len(), says, "{:?}", heard(&app).says);
        // In mid-air in the zone his head holds nobody (no chains of weak boosts).
        put::<Han>(&mut app, Vec2::new(30.0 * TILE + 8.0, standing(TILE) + 3.0 * TILE));
        app.update();
        assert!(!head(&mut app).solid, "{topic:?}: mid-air in the zone");
    }
}

/// The grumbles go round: the gate's own lines and the general ones, all short enough.
#[test]
fn grumbles_rotate() {
    for topic in [Topic::Giant, Topic::Gap, Topic::Waltz, Topic::Grip, Topic::Stain] {
        let n = grumble_lines(topic).len() + GRUMBLE_LINES.len();
        let lines: std::collections::HashSet<&str> = (0..n as u32).map(|k| grumble_line(Some(topic), k)).collect();
        assert_eq!(lines.len(), n, "{topic:?}: every line in turn");
        assert!(lines.iter().all(|l| l.len() <= nat_han_adventures::level::MAX_LINE), "{lines:?}");
    }
    assert!(GRUMBLE_LINES.contains(&grumble_line(None, 1)));
}

/// The weak boost and its toot, however timed, don't get Nat up a 6-tile giant wall from Han's
/// head at its foot (the band's Giant Steps still does that job).
#[test]
fn weak_boost_cannot_climb_a_giant_wall() {
    let rows = [
        "....................................",
        "....................................",
        "....................................",
        "....................................",
        "....................................",
        "....................................",
        "....................................",
        "....................................",
        ".........................###########",
        ".........................###########",
        ".........................###########",
        ".........................###########",
        ".........................###########",
        ".P.......................##########G",
        "####################################",
    ];
    let lvl = level("gate: giant 10,7 30,13\n", &rows);
    for toot_at in [0.1, 0.15, 0.2, 0.25, 0.3, 0.35] {
        let mut app = app(&lvl);
        put::<Han>(&mut app, Vec2::new(24.0 * TILE + 8.0, standing(TILE)));
        step(&mut app, 0.3);
        let h = han(&mut app);
        put::<Player>(&mut app, h + Vec2::new(0.0, 1.5 * TILE));
        step(&mut app, 0.5);
        press(&mut app, RIGHT);
        tap(&mut app);
        step(&mut app, toot_at);
        tap(&mut app);
        step(&mut app, 1.5);
        let p = nat(&mut app);
        assert!(p.y < standing(7.0 * TILE) - 8.0, "toot at {toot_at}: up the wall at {p}");
        assert_eq!(heard(&app).boosted, 1);
    }
}

/// An 11-tile long gap (marked): the weak boost off Han at the edge, running, with the toot,
/// falls short (the fired-up band still does that job).
#[test]
fn weak_boost_cannot_cross_a_long_gap() {
    let mut rows = vec![".".repeat(60); 14];
    rows.push(format!("{}{}{}", "#".repeat(25), ".".repeat(11), "#".repeat(24)));
    rows[13].replace_range(2..3, "P");
    rows[13].replace_range(56..57, "G");
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let lvl = level("gate: gap 23,12 37,13\n", &rows);
    for toot_at in [0.2, 0.3, 0.4, 0.5] {
        let mut app = app(&lvl);
        // Han hanging off the edge, Nat on the front of his head.
        put::<Han>(&mut app, Vec2::new(25.0 * TILE + 4.0, standing(TILE)));
        step(&mut app, 0.3);
        let h = han(&mut app);
        put::<Player>(&mut app, h + Vec2::new(4.0, 1.5 * TILE));
        step(&mut app, 0.5);
        press(&mut app, RIGHT);
        tap(&mut app);
        step(&mut app, toot_at);
        tap(&mut app);
        step(&mut app, 2.0);
        assert!(heard(&app).died >= 1 || nat(&mut app).x < 36.0 * TILE, "toot at {toot_at}: across at {}", nat(&mut app));
    }
}

// --- Overuse -----------------------------------------------------------------------------------

fn boost_once(app: &mut App) -> bool {
    let b0 = heard(app).boosted;
    let h = han(app);
    put::<Player>(app, h + Vec2::new(0.0, 1.5 * TILE));
    step(app, 0.3);
    tap(app);
    step(app, 1.2);
    heard(app).boosted > b0
}

#[test]
fn overuse_makes_han_take_a_breather() {
    let mut app = app(FLAT);
    eagerness(&mut app, 0.5);
    step(&mut app, 1.0);
    let limit = overuse_limit(0.5);
    for k in 0..limit {
        assert!(boost_once(&mut app), "boost {k}");
    }
    assert_eq!(said(&app, UNION_LINE), 1, "{:?}", heard(&app).says);
    assert_eq!(said(&app, BACK_WARN_LINE), 1);
    assert!(!head(&mut app).boost, "winded");
    assert_eq!(anim(&mut app).pose, HanPose::Winded);
    assert!(!boost_once(&mut app), "no boost while winded");
    step(&mut app, breather(0.5));
    assert!(boost_once(&mut app), "rested");
}

#[test]
fn eager_han_allows_more_boosts_and_no_limit_in_chasms() {
    assert!(overuse_limit(1.0) > overuse_limit(0.5) && overuse_limit(0.5) > overuse_limit(0.0));
    assert!(breather(1.0) < breather(0.0));
    // In a chain chasm mark: no limit at all.
    let lvl = FLAT.replace("name: Flat\n", "name: Flat\ngate: chain 0,0 59,9\n");
    let mut app = app(&lvl);
    step(&mut app, 1.0);
    for k in 0..overuse_limit(0.0) + 3 {
        assert!(boost_once(&mut app), "boost {k}");
    }
    assert_eq!(said(&app, UNION_LINE), 0);
}

// --- Eagerness ---------------------------------------------------------------------------------

#[test]
fn eagerness_scales_han() {
    assert!((go_ahead_delay(0.5) - 1.5).abs() < 1e-5);
    assert!((go_ahead_delay(1.0) - 0.7).abs() < 1e-5);
    assert!(go_ahead_delay(0.0) > go_ahead_delay(0.5));
    assert!(brace_range(1.0) > brace_range(0.0));
    assert!(intercept_range(1.0) > intercept_range(0.0));
    // In the game: an eager Han goes ahead about twice as soon.
    let wait = |e: f32| {
        let mut app = self::app(SEWER);
        eagerness(&mut app, e);
        press(&mut app, RIGHT);
        while nat(&mut app).x < 9.0 * TILE {
            app.update();
            trace(&mut app);
        }
        release(&mut app, RIGHT);
        let t0 = app.world().resource::<SimClock>().time;
        while !matches!(brain(&mut app).mode, HanMode::Ahead { .. } | HanMode::Sinking { .. }) {
            app.update();
            trace(&mut app);
            assert!(app.world().resource::<SimClock>().time - t0 < 5.0);
        }
        app.world().resource::<SimClock>().time - t0
    };
    let (eager, neutral) = (wait(1.0), wait(0.5));
    assert!(eager < neutral - 0.5, "eager {eager}s vs neutral {neutral}s");
}

// --- Per-mode physics --------------------------------------------------------------------------

#[test]
fn han_per_mode_physics() {
    // Giant Steps: floatier than Nat.
    let gs = HanPhys::of(&Groove::of(Harmony::Coltrane));
    assert!(gs.fall.gravity < Fall::of(&Groove::of(Harmony::Coltrane)).gravity);
    // Fired up: Nat runs faster, Han doesn't.
    let q = HanPhys::of(&Groove::of(Harmony::Quartal));
    assert!(q.speed < tuning::RUN_SPEED * FIRED_UP_SPEED);
    let mut app = app(FLAT);
    set_groove(&mut app, Harmony::Quartal, false);
    press(&mut app, RIGHT);
    step(&mut app, 2.5);
    let behind = nat(&mut app).x - han(&mut app).x;
    assert!(behind > 3.0 * TILE, "Han can't keep up: {behind}");
    // Waltz: steps only on the beat.
    let mut app = self::app(FLAT);
    let secs = 60.0 / nat_han_adventures::audio::waltz::WALTZ_BPM as f64;
    press(&mut app, RIGHT);
    step(&mut app, 0.6);
    release(&mut app, RIGHT);
    let mut moved_off_beat = 0.0f32;
    for k in 0..90 {
        let beats = k as f64 / 60.0 / secs;
        *app.world_mut().resource_mut::<Groove>() = Groove::of(Harmony::Waltz).at(BeatClock::at(beats, secs, 3));
        let x0 = han(&mut app).x;
        app.update();
        trace(&mut app);
        if app.world().resource::<Groove>().clock.phase >= 0.45 && han_body(&mut app).on_ground {
            moved_off_beat = moved_off_beat.max((han(&mut app).x - x0).abs() - 0.0);
        }
    }
    assert!(moved_off_beat < 3.0, "Han slid {moved_off_beat}px off the beat");
    // Nervous: clings closer, trembles.
    let mut app = self::app(FLAT);
    set_groove(&mut app, Harmony::MelodicMinor, false);
    press(&mut app, RIGHT);
    step(&mut app, 1.0);
    release(&mut app, RIGHT);
    step(&mut app, 2.0);
    let gap = nat(&mut app).x - han(&mut app).x;
    assert!(gap < FOLLOW_GAP && gap >= 12.0, "clings: {gap}");
    assert!(anim(&mut app).tremble);
    // Laughing band: he bounces and rolls.
    let mut app = self::app(FLAT);
    set_groove(&mut app, Harmony::Original, true);
    step(&mut app, 0.5);
    let h = han(&mut app);
    put::<Han>(&mut app, h + Vec2::new(0.0, 3.0 * TILE));
    let mut bounced = false;
    let mut was_falling = false;
    for _ in 0..90 {
        app.update();
        trace(&mut app);
        let v = han_body(&mut app).vel.y;
        bounced |= was_falling && v > 50.0;
        was_falling = v < -150.0;
    }
    assert!(bounced, "Han bounces with the laughing band");
}

// --- Numbers the validator relies on ---------------------------------------------------------

#[test]
fn buddy_raft_and_shield_timing() {
    use nat_han_adventures::game::{HAN_PLUG_LINGER, HAN_SEWAGE_RESPAWN, HAN_SINK_TIME, PARACHUTE_FALL};
    // One raft cycle (sink, gone, parachute from ~8.5 tiles, the lazy go-ahead, the walk in) is
    // far shorter than Han's raft floats: Nat on raft i waits for raft i+1 in safety.
    let cycle = HAN_SINK_TIME + HAN_SEWAGE_RESPAWN + 8.5 * TILE / PARACHUTE_FALL + go_ahead_delay(0.0) + 1.0;
    assert!(cycle < HAN_RAFT_LIFE_FLOOR - 5.0, "cycle {cycle}s");
    // Nat right behind Han (blocked by his back: 12 px, at march speed) clears each jet well
    // inside its linger.
    let lag = (tuning::PLAYER_SIZE.0 + 8.0) / HAN_MARCH_SPEED;
    assert!(lag < HAN_PLUG_LINGER, "lag {lag}s");
}

/// Going ahead is an escort: Nat may dawdle, Han waits for him and keeps the jets plugged.
#[test]
fn han_escorts_a_dawdling_nat_through_the_shield_row() {
    let mut app = app(&shield_level());
    press(&mut app, RIGHT);
    while nat(&mut app).x < 28.0 * TILE {
        app.update();
    }
    release(&mut app, RIGHT);
    while !matches!(brain(&mut app).mode, HanMode::Ahead { .. }) {
        app.update();
    }
    step(&mut app, 2.0);
    let lead = han(&mut app).x - nat(&mut app).x;
    assert!(lead < 2.5 * TILE, "Han waited for Nat: {lead}");
    press(&mut app, RIGHT);
    for _ in 0..1200 {
        app.update();
        if nat(&mut app).x > 56.0 * TILE {
            break;
        }
    }
    let at = nat(&mut app);
    assert_eq!(heard(&app).died, 0, "sprayed at {at}");
    assert!(at.x > 56.0 * TILE);
}

