//! Side B (Han alone, the player calls the band from a set list), headless: the real gameplay
//! and the band (headless audio), Han walking Side B's levels on his own.

use std::time::Duration;

use bevy::{ecs::system::RunSystemOnce, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use leafwing_input_manager::prelude::*;
use nat_han_adventures::{
    audio::{AudioOutput, Harmony},
    events::HanSays,
    game::{
        Groove, Han, Pos,
        sideb::{self, Fate, Phase, SIDE_B_COUNT, SetCall, SideB, SideBLevel},
    },
    level::TILE,
    save::Progress,
    state::{AppState, Shelf},
};

const DT: f64 = 1.0 / 60.0;

#[derive(Resource, Default)]
struct Heard(Vec<String>);

fn app(level: usize) -> App {
    app_starting(move |c| sideb::start(c, level))
}

/// An app that starts Side B with `start`.
fn app_starting(start: impl FnOnce(&mut Commands) + Send + Sync + 'static) -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        StatesPlugin,
        bevy::input::InputPlugin,
        nat_han_adventures::gameplay,
        nat_han_adventures::audio::plugin,
    ))
    .insert_resource(AudioOutput::Headless)
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(DT)))
    .init_resource::<Heard>()
    .add_systems(Last, |mut h: ResMut<Heard>, mut s: MessageReader<HanSays>| h.0.extend(s.read().map(|m| m.text.clone())));
    app.update();
    let mut start = Some(start);
    app.world_mut().run_system_once(move |mut c: Commands| (start.take().unwrap())(&mut c)).unwrap();
    app.update();
    app.update();
    app
}

fn side(app: &App) -> &SideB {
    app.world().resource::<SideB>()
}

fn han(app: &mut App) -> Vec2 {
    app.world_mut().query_filtered::<&Pos, With<Han>>().single(app.world()).unwrap().0
}

fn col(app: &mut App) -> f32 {
    han(app).x / TILE
}

fn trace(app: &mut App, t: f32) {
    if std::env::var("SIDE_B_TRACE").is_ok() {
        let (p, s, g) = (han(app), side(app).clone(), app.world().resource::<Groove>().harmony);
        let gr = app.world().resource::<Groove>();
        let (b, ph) = (gr.bounce, gr.phrase);
        let melt = app.world().resource::<nat_han_adventures::audio::plugin::Director>().melt_until;
        let half = app.world_mut().query_filtered::<&nat_han_adventures::game::Body, With<Han>>().single(app.world()).unwrap().half;
        eprintln!("   bounce={b} phrase={ph:?} melt_until={melt:?} half={half:?}");
        eprintln!("t={t:5.2} han=({:.2},{:.2}) target={:?} frontier={} charging={:?} landings={} cx={:.2} phase={:?} dir={:?} groove={g:?}", p.x / TILE, p.y / TILE, s.target, s.frontier, s.charging, s.charge_landings, s.charge_x / TILE, s.phase, s.dir);
    }
}

/// Step until `done` (or `secs` pass); the seconds it took.
fn until(app: &mut App, secs: f32, mut done: impl FnMut(&mut App) -> bool) -> Option<f32> {
    let n = (secs as f64 / DT).round() as usize;
    for i in 0..n {
        app.update();
        if i % std::env::var("SIDE_B_EVERY").ok().and_then(|v| v.parse().ok()).unwrap_or(15) == 0 {
            trace(app, i as f32 * DT as f32);
        }
        if done(app) {
            return Some(i as f32 * DT as f32);
        }
    }
    None
}

fn tap(app: &mut App, k: KeyCode) {
    // `SIDE_B_TIMELINE=1`: the taps as a `scripts/record-run --capture` timeline.
    if std::env::var("SIDE_B_TIMELINE").is_ok() {
        let t = app.world().resource::<Time>().elapsed_secs();
        let key = format!("{k:?}").trim_start_matches("Arrow").to_string();
        eprintln!("t={t:.4} tap {key} 1f");
    }
    k.press(app.world_mut());
    app.update();
    k.release(app.world_mut());
    app.update();
}

fn rewound(app: &mut App) -> bool {
    matches!(side(app).phase, Phase::Rewind { .. })
}

fn fate(app: &App) -> Option<Fate> {
    side(app).fate
}

fn said(app: &App, line: &str) -> bool {
    app.world().resource::<Heard>().0.iter().any(|s| s == line)
}

// Every level's routes are `play` plans, written as `try_a_plan` takes them: `col/k` calls
// set-list call `k` (0 giant, 1 gap, 2 melt) once Han's past column `col`; `col/20` is a hit,
// `col/30` and `col/31` lean the band left and right. (`col/1k`, the old pick for the nuggets
// ahead, does nothing now: calls are free.)
const ROUTES: [&[(&str, &str)]; SIDE_B_COUNT] = [
    // B1-B3 and B5-B7 are built from the building blocks (`examples/side_b_levels.rs`), their
    // routes with them: `levels/side-b/routes.txt` ([`routes`]). Their plans are the game's
    // (`sideb::parse_plan`: `col/k` as Han crosses a column, `+secs/k` after the step before;
    // `/20` the hit, `/30` `/31` a step of the record back and forward: a hold is `/30 ... /31`).
    &[],
    &[],
    &[],
    // The Big Flush (the waltz in place of melting), from high up: doors to hit over the pits,
    // then the shelf (Giant Steps over its walls, or the waltz under on trapezes), the climb, and
    // the trapdoor (a hit: down the tunnel of doors and bridges) or the shelf again (Giant Steps,
    // or fired up under it) and the trapezes down to the sewer.
    &[
        ("up, up", "10/1 27/20 44/20 77/0 79/20 166/0 168/20 206/20 209/0 300/2"),
        ("under, under", "10/1 27/20 44/20 77/2 79/20 166/0 168/20 206/20 209/1 300/2"),
        ("up, tunnel", "10/1 27/20 44/20 77/0 79/20 166/0 168/20 206/20 222/20 233/20 246/20 267/20 280/20 301/20 314/20 335/20 348/20 369/20"),
    ],
    &[],
    &[],
    &[],
    // Up the Pipe, a climb (Han turns at walls): a lift, a chimney of up-through floors, then
    // the giant wall (Giant Steps, over to the high gutter) or back over the trapdoor into the
    // cistern (its ferry and lift); then the leap (fired up, to the rim) or short of it, onto
    // the low road; the chimney up to the attic, back left: its hole (fired up, he leaps it;
    // otherwise down to the crawlway, its ferry and Nat behind a grate); the last chimney to the
    // roof and its wall (Giant Steps climbs it; otherwise he waits for the lift).
    &[
        ("cistern, low road, crawlway, lift", ""),
        ("giant wall, low road, crawlway, lift", "1/10 45/0"),
        ("giant wall, rim, crawlway, lift", "1/10 45/0 100/11 112/1"),
        ("giant wall, rim, leap, lift", "1/10 45/0 100/11 112/1 t=29.5/1"),
        ("giant wall, rim, leap, roof wall", "1/10 45/0 100/11 112/1 t=29.5/1 t=36/10 t=38/0"),
    ],
    // The U-Bend, a maze: the drip door (melted, he's through; otherwise he turns, drops through
    // the trapdoor behind him and a duck sends him round the detour), the hole (fired up, he
    // leaps it; otherwise down to the ferry, and Nat behind a grate), the giant wall (Giant
    // Steps, over it to the high road; otherwise back through a trapdoor to the sewer's ferry
    // and lift), then the switchback (Giant Steps over the last wall; otherwise back over a
    // trapdoor to the lane under the high road and round), and the last run.
    &[
        ("the long way", ""),
        ("drip door", "1/12 16/2"),
        ("drip door, leap", "1/12 16/2 40/11 66/1"),
        ("drip door, leap, giant wall", "1/12 16/2 40/11 66/1 100/10 120/0"),
        ("drip door, leap, giant wall, last wall", "1/12 16/2 40/11 66/1 100/10 120/0 230/0"),
        ("leap", "1/11 60/1"),
        ("giant wall", "100/10 124/0"),
        ("last wall", "1/10 230/0"),
    ],
    // The Final Flush, over the sewage lake: a ferry, a lift, then the grease slide (fired up, he
    // flies to the deck and its crushers; otherwise down to the drain's ferries and lift), then
    // the drip tunnel (melted, he's through; otherwise back over the trapdoor to the drain's
    // loop, and Nat behind a grate), then the flume's two canals (fired up, he jumps them;
    // otherwise he waits for their ferries) to the throne.
    &[
        ("slide, drip, flume", "1/11 44/1 70/12 158/2 180/11 200/1 236/1"),
        ("drain, drain, ferries", ""),
        ("deck, drain, ferries", "1/11 40/1"),
        ("drain, drip, ferries", "1/12 158/2"),
        ("drain, drain, flume", "1/11 200/1"),
    ],
];

/// The routes the building blocks wrote with the levels (`b1 shelf: <plan>` lines).
const ROUTE_FILE: &str = include_str!("../levels/side-b/routes.txt");

/// Level `n`'s routes: from [`ROUTE_FILE`], or [`ROUTES`] for a level built by hand.
fn routes(n: usize) -> Vec<(String, String)> {
    let key = format!("b{} ", n + 1);
    let built: Vec<(String, String)> = ROUTE_FILE
        .lines()
        .filter_map(|l| l.strip_prefix(key.as_str())?.split_once(": "))
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
    if !built.is_empty() {
        return built;
    }
    ROUTES[n].iter().map(|&(a, b)| (a.to_string(), b.to_string())).collect()
}

/// The levels whose routes are the game's plans (`play_steps`); the rest are `plan`s.
const STEPPED: usize = 7;

/// A plan from its text (`5/12 7/0`: pick melting at column 5, call Giant Steps at 7).
/// A plan: `col/call` (the call when Han reaches that column) or `t=secs/call` (at that play
/// time: in a maze or a climb he comes by a column more than once).
fn plan(s: &str) -> Vec<(f32, usize)> {
    s.split_whitespace()
        .map(|p| {
            let (c, k) = p.split_once('/').unwrap();
            let k: usize = k.parse().unwrap();
            match c.strip_prefix("t=") {
                Some(t) => (t.parse().unwrap(), k | AT_TIME),
                None => (c.parse().unwrap(), k),
            }
        })
        .collect()
}

/// A plan step's call made at a time, not a column.
const AT_TIME: usize = 1 << 16;

fn died(r: Result<(), (Option<Fate>, f32)>) -> (Fate, f32) {
    match r {
        Err((Some(f), c)) => (f, c),
        other => panic!("expected Han to come off worst, got {other:?}"),
    }
}

#[test]
fn level_one_parses_with_its_set_list() {
    let b = sideb::SideBLevel::load(0);
    // Nothing to start with: the calls are the nuggets Han grabs. All three, as on every level.
    assert_eq!(b.setlist, vec![(SetCall::Mode(Harmony::Coltrane), 0), (SetCall::Mode(Harmony::Quartal), 0), (SetCall::Melt, 0)]);
}

#[test]
fn han_only_barks_on_side_b_too() {
    let fates = [Fate::Spikes, Fate::Pit, Fate::Sewage, Fate::Bonk, Fate::Pooped].map(Fate::line);
    let lines = [sideb::DUCK_LINE, sideb::CLIMB_LINE, sideb::LEAP_LINE, sideb::GO_LINE, sideb::CLEARED_LINE];
    for l in fates.into_iter().chain(lines) {
        assert!(nat_han_adventures::level::is_bark(l), "{l:?}");
    }
    for n in 0..sideb::SIDE_B_SOURCES.len() {
        assert_eq!(sideb::SideBLevel::load(n).level.intro, "", "level {} has no intro speech", n + 1);
    }
}

#[test]
fn set_lists_come_in_one_order() {
    let b = sideb::SideBLevel::parse("name: x\nworld: 2\nsetlist: melt=1 gap=2 giant=3\n---\nP..G\n####\n").unwrap();
    let calls: Vec<SetCall> = b.setlist.iter().map(|c| c.0).collect();
    assert_eq!(calls, [SetCall::Mode(Harmony::Coltrane), SetCall::Mode(Harmony::Quartal), SetCall::Melt]);
}

#[test]
fn nuggets_are_the_score_and_calls_are_free() {
    let mut app = app(8);
    until(&mut app, 5.0, |a| col(a) >= 13.0).unwrap();
    assert_eq!(side(&app).nuggets, 2, "both runway nuggets");
    for _ in 0..3 {
        tap(&mut app, KeyCode::Digit3);
    }
    assert_eq!(side(&app).calls, 3, "as many calls as you like");
}

#[test]
fn in_the_maze_drips_turn_han_round_unless_hes_melted() {
    // The U-Bend's drip door (columns 38-47): full size, he stops short and turns back (and drops
    // through the trapdoor he crossed); melted, he's through.
    let mut full = app(8);
    until(&mut full, 10.0, |a| side(a).dir == Some(-1.0)).expect("turned at the drips");
    assert_eq!(side(&full).phase, Phase::Playing, "alive");
    let at = col(&mut full);
    assert!((30.0..38.0).contains(&at), "at the door, not {at}");
    let mut app = app(8);
    for (c, k) in plan("1/12 16/2") {
        until(&mut app, 10.0, |a| col(a) >= c).unwrap();
        act(&mut app, k);
    }
    until(&mut app, 10.0, |a| col(a) >= 49.0).expect("through the door");
    assert_eq!(side(&app).dir, Some(1.0));
    assert!(han(&mut app).y > 12.0 * TILE, "on the main lane, not the detour");
}

#[test]
fn han_waits_for_a_ferry_and_rides_a_lift_to_the_top() {
    // The Final Flush's first canal (columns 20-27) and the lift up to the high deck (46-48).
    let mut app = app(9);
    until(&mut app, 10.0, |a| col(a) >= 19.0).unwrap();
    let at = col(&mut app);
    until(&mut app, 0.5, |_| false);
    assert!((col(&mut app) - at).abs() < 1.5, "waiting at the canal");
    until(&mut app, 10.0, |a| col(a) >= 49.0).expect("over the canal and up the lift");
    assert_eq!(side(&app).phase, Phase::Playing);
    assert!(han(&mut app).y > 20.0 * TILE, "up on the high deck");
}

#[test]
fn the_long_way_finds_nat_behind_a_grate() {
    // B8-B10 hide Nat on the route with no calls (the crawlway, the ferry pit, the drain's loop):
    // Han finds him there, and the save remembers. The quick route goes by.
    for (level, long, quick) in [(7, 0, 4), (8, 0, 4), (9, 1, 0)] {
        for (route, found) in [(long, true), (quick, false)] {
            let mut app = app(level);
            let r = routes(level);
            play_in_app(&mut app, &plan(&r[route].1)).unwrap();
            let name = &r[route].0;
            assert_eq!(app.world().resource::<Progress>().side_b_secrets[level], found, "B{} {name}", level + 1);
        }
    }
}

#[test]
fn hans_closing_stretch_starts_the_shout() {
    // Side B's `shout:` goes by Han (Nat's off screen): B9's switchback, from column 250.
    #[derive(Resource, Default)]
    struct Shouts(Vec<bool>);
    let mut app = app(8);
    app.init_resource::<Shouts>().add_systems(Last, |mut n: ResMut<Shouts>, mut s: MessageReader<nat_han_adventures::game::secret::ShoutChorus>| {
        n.0.extend(s.read().map(|s| s.0))
    });
    until(&mut app, 60.0, |a| col(a) >= 245.0).unwrap();
    assert!(app.world().resource::<Shouts>().0.is_empty(), "not yet");
    until(&mut app, 10.0, |a| !a.world().resource::<Shouts>().0.is_empty()).expect("the shout");
    assert!(col(&mut app) >= 250.0);
}

#[test]
fn jump_is_the_hit_on_the_next_beat() {
    let mut app = app(0);
    until(&mut app, 5.0, |a| col(a) >= 7.0).unwrap();
    tap(&mut app, KeyCode::Space);
    assert_eq!(side(&app).calls, 0, "not a call");
    assert_eq!(side(&app).hits, 0, "not till the next beat");
    let beat = app.world().resource::<Groove>().clock.beat_secs;
    until(&mut app, beat + 0.1, |a| side(a).hits > 0).expect("the hit lands");
    assert_eq!(side(&app).hits, 1);
}

#[test]
fn a_number_calls_at_once_as_often_as_you_like() {
    // (On a phone, a tap on a third of the screen: index.html presses 1, 2 or 3.)
    let mut app = app(0);
    until(&mut app, 2.0, |a| col(a) >= 3.0).unwrap();
    tap(&mut app, KeyCode::Digit2);
    assert_eq!((side(&app).selected, side(&app).calls), (1, 1));
    tap(&mut app, KeyCode::Digit2);
    assert_eq!((side(&app).selected, side(&app).calls), (1, 2));
    // (Down is a hand on the record, not a call.)
    tap(&mut app, KeyCode::ArrowDown);
    assert_eq!(side(&app).calls, 2);
}

#[test]
fn a_hand_on_the_record_holds_han_and_pulls_him_back() {
    let mut app = app(0);
    assert!(side(&app).on_record(), "the record is the steer");
    until(&mut app, 2.0, |a| col(a) >= 4.0).unwrap();
    // Down held: the record drags to a stop, and Han waits.
    KeyCode::ArrowDown.press(app.world_mut());
    until(&mut app, 0.5, |_| false);
    let at = col(&mut app);
    until(&mut app, 1.0, |_| false);
    assert!((col(&mut app) - at).abs() < 0.2, "held at {at}, now {}", col(&mut app));
    KeyCode::ArrowDown.release(app.world_mut());
    // Left held: scratched back, he moonwalks (still facing the goal).
    KeyCode::ArrowLeft.press(app.world_mut());
    until(&mut app, 1.0, |_| false);
    assert!(col(&mut app) < at - 1.0, "back he goes");
    let left = app.world_mut().query_filtered::<&nat_han_adventures::game::HanAnim, With<Han>>().single(app.world()).unwrap().facing_left;
    assert!(!left, "a moonwalk: still facing the goal");
    KeyCode::ArrowLeft.release(app.world_mut());
    // Let go: on he runs.
    let back = col(&mut app);
    until(&mut app, 1.0, |_| false);
    assert!(col(&mut app) > back + 1.0, "and on again");
}

#[test]
fn the_balance_steers_han() {
    let mut app = app(0);
    app.world_mut().resource_mut::<SideB>().steer_sound = nat_han_adventures::audio::live::desk::Steer::Balance;
    until(&mut app, 2.0, |a| col(a) >= 4.0).unwrap();
    // To the middle: he stops.
    tap(&mut app, KeyCode::ArrowLeft);
    assert_eq!(side(&app).pan_to, 0.0);
    until(&mut app, 0.6, |_| false);
    let at = col(&mut app);
    until(&mut app, 1.0, |_| false);
    assert!((col(&mut app) - at).abs() < 0.2, "stopped at {at}, now {}", col(&mut app));
    // Over to the left: back he goes.
    tap(&mut app, KeyCode::ArrowLeft);
    until(&mut app, 1.0, |_| false);
    assert!(col(&mut app) < at - 1.0, "heading back");
    tap(&mut app, KeyCode::ArrowRight);
    tap(&mut app, KeyCode::ArrowRight);
    let back = col(&mut app);
    until(&mut app, 1.0, |_| false);
    assert!(col(&mut app) > back + 1.0, "and on again");
}

#[test]
fn a_death_keeps_the_tune_playing_and_r_starts_it_over() {
    let mut app = app(0);
    app.init_resource::<Started>()
        .add_systems(Last, |mut n: ResMut<Started>, mut s: MessageReader<nat_han_adventures::audio::MusicStarted>| n.0 += s.read().count());
    let (f, _) = died(play_in_app(&mut app, &[]));
    assert_eq!(f, Fate::Pit);
    let before = app.world().resource::<Started>().0;
    until(&mut app, 4.0, |a| side(a).phase == Phase::Playing).unwrap();
    until(&mut app, 0.2, |_| false);
    assert_eq!(side(&app).rewinds, 1);
    assert_eq!(app.world().resource::<Started>().0, before, "the tune plays on");
    tap(&mut app, KeyCode::KeyR);
    until(&mut app, 0.5, |_| false);
    assert_eq!(app.world().resource::<Started>().0, before + 1, "R: from the top");
}

#[derive(Resource, Default)]
struct Started(usize);

#[test]
fn every_route_of_every_level() {
    for n in 0..SIDE_B_COUNT {
        let routes = routes(n);
        assert!(routes.len() >= 2, "level {} forks", n + 1);
        for (name, p) in routes.iter() {
            if n < STEPPED {
                let mut app = app(n);
                assert!(play_steps(&mut app, p).is_ok(), "level {}, {name}", n + 1);
                assert_eq!(side(&app).rewinds, 0, "level {}, {name}: no deaths", n + 1);
            } else {
                assert_eq!(play(n, &plan(p)), Ok(()), "level {}, {name}", n + 1);
            }
        }
    }
}

#[test]
fn with_no_music_han_keeps_at_the_wall() {
    let mut app = app(0);
    // (Fired up for the first pit, then nothing for the wall.)
    until(&mut app, 2.0, |a| col(a) >= 3.0).unwrap();
    act(&mut app, 1);
    until(&mut app, 15.0, |a| said(a, sideb::CLIMB_LINE)).expect("Han charges the giant wall");
    let at = col(&mut app);
    until(&mut app, 8.0, |_| false);
    assert_eq!(side(&app).phase, Phase::Playing, "no knocking himself out: call the band or turn him round");
    assert!((col(&mut app) - at).abs() < 3.0, "still at it");
}

#[test]
fn no_fired_up_for_the_gap_is_a_long_way_down() {
    let (f, c) = died(play(0, &[]));
    assert_eq!(f, Fate::Pit);
    assert!((11.0..22.0).contains(&c), "at the first pit, not {c}");
}

#[test]
fn han_hops_the_floor_spikes_but_not_the_alleys_drips() {
    // Spike Alley, fired up for the pit and Giant Steps for the wall: over the floor's spikes
    // on his own, but with nothing playing at the fork he takes the alley, full size.
    let mut app = app(1);
    let r = play_in_app(&mut app, &plan("3/1 22/0"));
    let (f, c) = died(r);
    assert_eq!(f, Fate::Spikes);
    assert!((78.0..116.0).contains(&c), "in the alley's drips, not {c}");
}

#[test]
fn full_size_han_loses_his_cap_in_the_drips() {
    // Down the Drain's first drips (columns 59-70), with no melting.
    let (f, c) = died(play(0, &plan("3/1 24/0")));
    assert_eq!(f, Fate::Spikes);
    assert!((57.0..71.0).contains(&c), "at the drips, not {c}");
}

#[test]
fn spikes_kill_han_and_the_level_rewinds() {
    let mut app = app(0);
    until(&mut app, 5.0, |a| col(a) >= 8.0).unwrap();
    // Put him under the first drips.
    let level = app.world().resource::<nat_han_adventures::game::ActiveLevel>().level.clone();
    let i = level.tiles.iter().position(|&t| t == nat_han_adventures::level::Tile::SpikesDown).unwrap();
    let spot = level.tile_center(i % level.width, i / level.width);
    let mut q = app.world_mut().query_filtered::<(&mut Pos, &mut nat_han_adventures::game::PrevPos, &mut nat_han_adventures::game::Body), With<Han>>();
    let (mut p, mut pp, mut body) = q.single_mut(app.world_mut()).unwrap();
    (p.0, pp.0, body.vel) = (spot, spot, Vec2::ZERO);
    until(&mut app, 0.5, rewound).expect("spikes are deadly on Side B");
    assert_eq!(fate(&app), Some(Fate::Spikes));
    until(&mut app, 3.0, |a| side(a).rewinds == 1).expect("back he comes");
    assert!(col(&mut app) < 5.0, "at the start: no checkpoint passed");
}

#[test]
fn melting_too_late_is_too_late() {
    // Down the Drain's first drips, melting called as he reaches them: the melting phrase is
    // too far off.
    let (f, c) = died(play(0, &plan("3/1 24/0 57/2")));
    assert_eq!(f, Fate::Spikes);
    assert!(c > 57.0, "at the drips, not {c}");
}

#[test]
fn a_cleared_level_goes_on_to_the_next() {
    let mut app = app(0);
    play_steps(&mut app, &routes(0)[1].1).expect("cleared");
    until(&mut app, 6.0, |a| side(a).level == 1).expect("on to Spike Alley");
    until(&mut app, 0.1, |_| false);
    assert_eq!(side(&app).phase, Phase::Playing);
    assert!(col(&mut app) < 5.0, "Han's at its start");
    let progress = app.world().resource::<Progress>();
    assert_eq!((progress.side_b_unlocked, progress.side_b_rewinds[0]), (2, Some(0)), "saved: B2 open");
}

#[test]
fn the_last_level_ends_in_hans_encore_gone_wrong() {
    use nat_han_adventures::game::{
        Player,
        encore::{Encore, EncorePhase, Gag},
    };
    let n = SIDE_B_COUNT - 1;
    let mut app = app(n);
    for (c, k) in plan(&routes(n)[0].1) {
        until(&mut app, 60.0, |a| col(a) >= c).unwrap();
        act(&mut app, k);
    }
    until(&mut app, 30.0, |a| a.world().get_resource::<Encore>().is_some()).expect("an encore at the goal");
    let e = app.world().resource::<Encore>().clone();
    assert_eq!((e.gag, e.phase), (Gag::Plunge, EncorePhase::Lap));
    assert_eq!(app.world().resource::<Progress>().side_b_rewinds[n], Some(0), "saved at the goal");
    assert!(said(&app, sideb::CLEARED_LINE));
    // Calls are free in the lap, Down or Jump, and Han dances on the stage.
    let calls = side(&app).calls;
    tap(&mut app, KeyCode::ArrowDown);
    tap(&mut app, KeyCode::Space);
    assert_eq!(app.world().resource::<Encore>().calls, 2);
    assert_eq!(side(&app).calls, calls, "not the set list's");
    until(&mut app, 2.0, |_| false);
    let x = han(&mut app).x;
    assert!((e.stage.0..=e.stage.1).contains(&x), "Han on the stage");
    // The last chord: up on the throne, plunge, and up comes Nat on the plunger...
    let phase = |a: &App| a.world().resource::<Encore>().phase;
    until(&mut app, 30.0, |a| phase(a) == EncorePhase::Plunging).expect("plunging");
    let seat = app.world().resource::<Encore>().seat();
    assert!(han(&mut app).distance(seat) < 16.0, "on the throne");
    until(&mut app, 3.0, |a| phase(a) == EncorePhase::Yanked).expect("yanked");
    until(&mut app, 0.2, |_| false);
    assert!(app.world().resource::<Encore>().nat_shown());
    assert!(said(&app, "NAT!!"));
    let nat = |a: &mut App| a.world_mut().query_filtered::<&Pos, With<Player>>().single(a.world()).unwrap().0;
    assert!(nat(&mut app).y > seat.y + 16.0, "Nat up on the cup");
    // ...and the throne blows him up through the ceiling, upstairs.
    until(&mut app, 5.0, |a| phase(a) == EncorePhase::Continued).expect("to be continued");
    let bowl = app.world().resource::<Encore>().bowl();
    assert!(nat(&mut app).y > bowl.y + 200.0 && (nat(&mut app).x - bowl.x).abs() < 4.0, "up and away");
    assert!(said(&app, "...oops."));
    until(&mut app, 5.0, |a| a.world().get_resource::<SideB>().is_none()).expect("left Side B");
    assert_eq!(*app.world().resource::<State<AppState>>().get(), AppState::LevelSelect);
    assert_eq!(*app.world().resource::<Shelf>(), Shelf::SideB);
}

/// Call set-list call `k` (1, 2, 3).
fn call(app: &mut App, k: usize) {
    tap(app, [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3][k]);
}

/// In a plan, `(col, PICK + k)` was a pick (nothing now: the calls are free).
const PICK: usize = 10;
/// In a plan, `(col, HIT)` (`col/20`) plays the hit (Jump); `col/30`, `col/31` lean the band.
const HIT: usize = 20;
const LEFT: usize = 30;
const RIGHT: usize = 31;

/// Do plan step `k`.
fn act(app: &mut App, k: usize) {
    match k {
        HIT => tap(app, KeyCode::Space),
        // (A step of where the steer rests, as the game's own plans do: on the record a key is
        // a hand held on it.)
        LEFT => app.world_mut().resource_mut::<SideB>().lean(-1.0),
        RIGHT => app.world_mut().resource_mut::<SideB>().lean(1.0),
        PICK.. => {}
        k => call(app, k),
    }
}

/// Play Side B level `level`, calling set-list call `k` once Han's past column `col`, for each
/// `(col, k)` in turn: cleared, or how Han met his end (and where).
fn play(level: usize, plan: &[(f32, usize)]) -> Result<(), (Option<Fate>, f32)> {
    play_in(app(level), plan)
}

fn play_in(mut app: App, plan: &[(f32, usize)]) -> Result<(), (Option<Fate>, f32)> {
    play_in_app(&mut app, plan)
}

fn play_in_app(app: &mut App, plan: &[(f32, usize)]) -> Result<(), (Option<Fate>, f32)> {
    for &(c, k) in plan {
        let timed = k & AT_TIME != 0;
        let k = k & !AT_TIME;
        let now = |a: &App| a.world().resource::<nat_han_adventures::game::LevelRun>().time;
        until(app, 60.0, |a| (if timed { now(a) >= c } else { col(a) >= c }) || side(a).phase != Phase::Playing);
        if side(app).phase != Phase::Playing {
            break;
        }
        if std::env::var("SIDE_B_TIMES").is_ok() {
            let t = app.world().resource::<nat_han_adventures::game::LevelRun>().time;
            eprintln!("call {k} at t={t:.2} col={:.1}", col(app));
        }
        act(app, k);
    }
    until(app, 90.0, |a| side(a).phase != Phase::Playing);
    match side(app).phase {
        Phase::Cleared { .. } => Ok(()),
        _ => Err((fate(app), col(app))),
    }
}

#[test]
fn a_death_rewinds_to_the_last_checkpoint() {
    // B4: over the pits fired up, then no waltz for the trapezes under the shelf: Han falls in,
    // and comes back at the checkpoint at the shelf's mouth, not at the top.
    let mut app = app(3);
    let r = play_in_app(&mut app, &plan("10/1 27/20 44/20"));
    assert_eq!(r.map_err(|e| e.0), Err(Some(Fate::Pit)));
    let mouth = side(&app).mouth.clone().expect("passed a checkpoint");
    until(&mut app, 5.0, |a| side(a).phase == Phase::Playing);
    assert_eq!(side(&app).rewinds, 1);
    assert!((col(&mut app) - mouth.at.x / TILE).abs() < 1.5, "back at {} for the checkpoint at {}", col(&mut app), mouth.at.x / TILE);
    // And on from there: the trapezes with the waltz, as the route goes.
    assert!(play_steps(&mut app, "77/2 79/20 166/0 168/20 206/20 209/1 300/2").is_ok());
    assert_eq!(side(&app).rewinds, 1);
}

/// `SIDE_B_PLAN="1: 3/0 22/1"` plays level 1 calling call 0 at column 3, call 1 at 22 (to find
/// a level's solution): `cargo test --test side_b try_a_plan -- --ignored --nocapture`.
#[test]
#[ignore]
fn try_a_plan() {
    let s = std::env::var("SIDE_B_PLAN").unwrap();
    let (lvl, rest) = s.split_once(':').unwrap();
    eprintln!("{:?}", play(lvl.trim().parse::<usize>().unwrap() - 1, &plan(rest)));
}

/// A level file that isn't in the game yet, played with a plan (to build one):
/// `SIDE_B_FILE=path SIDE_B_PLAN="3/0 22/1" cargo test --test side_b try_a_file -- --ignored
/// --nocapture` (`SIDE_B_SLOT=n`: as level n, for its tune; `SIDE_B_TRACE=1` traces Han).
#[test]
#[ignore]
fn try_a_file() {
    let src = std::fs::read_to_string(std::env::var("SIDE_B_FILE").unwrap()).unwrap();
    let b = SideBLevel::parse(&src).unwrap();
    let slot: usize = std::env::var("SIDE_B_SLOT").ok().and_then(|v| v.parse().ok()).unwrap_or(1) - 1;
    let p = std::env::var("SIDE_B_PLAN").unwrap_or_default();
    let mut app = app_starting(move |c| sideb::start_level(c, slot, b));
    let r = play_steps(&mut app, &p);
    let t = app.world().resource::<nat_han_adventures::game::LevelRun>().time;
    let bar = app.world().resource::<Groove>().clock.bar;
    eprintln!("{r:?} at {t:.1}s bar {bar}, {} deaths ({:?})", side(&app).rewinds, side(&app).fate);
}

/// Play a route as the game's `NATHAN_SIDE_B_PLAN` does ([`sideb::parse_plan`]: steps at a
/// column Han crosses, either way, or `+secs` after the last; a death doesn't stop it): cleared
/// (and how long it took), or where Han got to.
fn play_steps(app: &mut App, text: &str) -> Result<f32, f32> {
    let steps = sideb::parse_plan(text);
    let (mut i, mut prev, mut secs, mut deaths) = (0, col(app), 0.0f32, 0);
    let n = (120.0 / DT) as usize;
    for f in 0..n {
        app.update();
        let x = col(app);
        if side(app).phase == Phase::Playing && i < steps.len() {
            secs += DT as f32;
            if sideb::plan_due(steps[i], prev, x, secs) {
                if let Some(k) = steps[i].act() {
                    if std::env::var("SIDE_B_TIMES").is_ok() {
                        eprintln!("step {i} ({k}) at t={:.2} col={x:.1} hits={}", f as f64 * DT, side(app).hits);
                    }
                    act(app, k);
                }
                i += 1;
                secs = 0.0;
            }
        }
        if side(app).rewinds != deaths {
            deaths = side(app).rewinds;
            if std::env::var("SIDE_B_TIMES").is_ok() {
                eprintln!("death {deaths} ({:?}) at t={:.2} col={prev:.1}", side(app).fate, f as f64 * DT);
            }
        }
        prev = x;
        if f % 15 == 0 {
            trace(app, f as f32 * DT as f32);
        }
        if matches!(side(app).phase, Phase::Cleared { .. }) {
            return Ok(f as f32 * DT as f32);
        }
    }
    Err(col(app))
}

/// How much slack each call of a level's routes has: the shifts (in columns) that still clear
/// it, with the others where they are. `SIDE_B_TIGHT=5 cargo test --test side_b how_tight --
/// --ignored --nocapture`.
#[test]
#[ignore]
fn how_tight() {
    let n: usize = std::env::var("SIDE_B_TIGHT").unwrap().parse().unwrap();
    for (name, p) in routes(n - 1) {
        let plan = plan(&p);
        for i in 0..plan.len() {
            let ok: Vec<i32> = [-6, -4, -2, 2, 4, 6]
                .into_iter()
                .filter(|&d| {
                    let mut p = plan.clone();
                    p[i].0 += d as f32;
                    (i == 0 || p[i].0 > p[i - 1].0) && play(n - 1, &p).is_ok()
                })
                .collect();
            eprintln!("{name}: {i} {:?}: clears with shifts {ok:?}", plan[i]);
        }
    }
}

/// Every route of every level against the tune's form: when each call goes in (play time, and
/// the bar it lands on: the clock's bar, 0-based, beat) and when Han reaches the goal. The tunes
/// are 32 bars of 8-bar sections at 192 bpm (a bar is 1.25 s), after a 4-bar intro when the band
/// plays one. `cargo test --test side_b route_report -- --ignored --nocapture`.
#[test]
#[ignore]
fn route_report() {
    let clock = |a: &App| {
        let c = a.world().resource::<Groove>().clock;
        let t = a.world().resource::<nat_han_adventures::game::LevelRun>().time;
        format!("{t:.1}s bar {}.{}", c.bar, c.beat + 1)
    };
    let only: Option<usize> = std::env::var("SIDE_B_LEVEL").ok().and_then(|v| v.parse().ok());
    for n in 0..SIDE_B_COUNT {
        let routes = routes(n);
        if only.is_some_and(|o| o != n + 1) {
            continue;
        }
        for (name, p) in routes.iter() {
            let mut app = app(n);
            let mut calls = Vec::new();
            for &(c, k) in &plan(p) {
                until(&mut app, 30.0, |a| col(a) >= c || side(a).phase != Phase::Playing);
                if k < PICK {
                    calls.push(format!("{}@{c} {}", ["giant", "gap", "melt"][k], clock(&app)));
                }
                act(&mut app, k);
            }
            until(&mut app, 40.0, |a| side(a).phase != Phase::Playing);
            let end = clock(&app);
            eprintln!("B{} {name}: goal {end} | {}", n + 1, calls.join(" | "));
        }
    }
}

/// Free play's Side B room `seed` (band, Side B levels cleared) on its own, as a Side B level,
/// with the plan it was drawn with.
fn alone_room(seed: u64, band: u8, cleared: usize) -> (SideBLevel, Vec<(f32, usize)>) {
    use nat_han_adventures::freeplay::{alone, canvas::Canvas, dice::Dice};
    let mut c = Canvas::with_height(alone::ALONE_H);
    let drawn = alone::draw(&mut c, &mut Dice::new(seed), band, cleared);
    (SideBLevel { level: c.finish_with(2, "Go!", 0), setlist: alone::setlist(), turns: false }, drawn.solution)
}

/// Every Side B room free play draws clears with the plan it was drawn with: every band, with
/// each set of Side B pieces (B1's, B2's, B3's). `SIDE_B_ROOMS=n` plays n seeds of each.
#[test]
fn free_play_side_b_rooms_clear_with_their_plans() {
    let n: u64 = std::env::var("SIDE_B_ROOMS").ok().and_then(|v| v.parse().ok()).unwrap_or(2);
    let mut failed = Vec::new();
    for cleared in [1, 2, 3] {
        for band in [1, 4, 7, 10] {
            for seed in 0..n {
                let seed = seed * 7919 + band as u64 * 31 + cleared as u64;
                let (b, plan) = alone_room(seed, band, cleared);
                let text: Vec<String> = plan.iter().map(|(c, k)| format!("{c}/{k}")).collect();
                if let Err(e) = play_in(app_starting(move |c| sideb::start_level(c, 0, b)), &plan) {
                    failed.push(format!("seed {seed} band {band} cleared {cleared}: {e:?}, plan {}", text.join(" ")));
                }
            }
        }
    }
    assert!(failed.is_empty(), "{} rooms failed:\n{}", failed.len(), failed.join("\n"));
}

/// `SIDE_B_ROOM="seed band cleared"` prints a free play Side B room and plays its plan.
#[test]
#[ignore]
fn try_a_room() {
    let s = std::env::var("SIDE_B_ROOM").unwrap();
    let v: Vec<u64> = s.split_whitespace().map(|w| w.parse().unwrap()).collect();
    let (b, plan) = alone_room(v[0], v[1] as u8, v[2] as usize);
    for r in 0..b.level.height {
        use nat_han_adventures::level::Tile;
        let row: String = (0..b.level.width)
            .map(|c| match b.level.tile(c as i32, r as i32) {
                Tile::Solid => '#',
                Tile::SpikesUp => '^',
                Tile::SpikesDown => 'v',
                Tile::Empty => '.',
                _ => '?',
            })
            .collect();
        eprintln!("{row}");
    }
    eprintln!("{plan:?}");
    eprintln!("{:?}", play_in(app_starting(move |c| sideb::start_level(c, 0, b)), &plan));
}

/// Free play, once Side B's been played: now and then the run hands a room to Han. Nat waits
/// in the pipe (hidden), the set list comes up, a death rewinds only the room (Han back at its
/// start, its nuggets back), and Han reaching its exit hands the run back to Nat there.
#[test]
fn free_play_mixes_in_a_side_b_room() {
    use nat_han_adventures::freeplay::{FreePlayRun, StartFreePlay, alone, canvas::Canvas, course::COURSE_STAND, dice, generate::ROOM_STREAM};
    use nat_han_adventures::game::{Player, PrevPos, stand_pos};

    fn run(app: &App) -> FreePlayRun {
        app.world().resource::<FreePlayRun>().clone()
    }
    fn teleport(app: &mut App, col: usize) {
        let at = stand_pos(&app.world().resource::<nat_han_adventures::game::ActiveLevel>().level, col, COURSE_STAND);
        let mut q = app.world_mut().query_filtered::<(&mut Pos, &mut PrevPos), With<Player>>();
        for (mut p, mut pp) in q.iter_mut(app.world_mut()) {
            p.0 = at;
            pp.0 = at;
        }
    }
    // Seeds until a run serves a Side B room (they come once the first rooms have placed the
    // player).
    for seed in 1..40u32 {
        let mut app = app_starting(move |c| {
            c.queue(move |w: &mut World| {
                let mut p = w.resource_mut::<Progress>();
                p.unlocked = 10;
                for r in p.side_b_rewinds.iter_mut().take(3) {
                    *r = Some(0);
                }
                w.write_message(StartFreePlay { seed, endless: false });
            })
        });
        let mut k = 0;
        let found = loop {
            let mut frames = 0;
            while run(&app).course.rooms.len() <= k {
                app.update();
                std::thread::sleep(Duration::from_millis(1));
                frames += 1;
                assert!(frames < 20_000, "room {k} never came");
            }
            // (Streamed in a few columns a frame.)
            for _ in 0..40 {
                app.update();
            }
            if run(&app).rooms[k].plan.template().alone() {
                break true;
            }
            if k + 1 >= nat_han_adventures::freeplay::FIXED_ROOMS as usize {
                break false;
            }
            let start = run(&app).course.rooms[k].start_col();
            teleport(&mut app, start);
            for _ in 0..3 {
                app.update();
            }
            k += 1;
        };
        if !found {
            continue;
        }
        // In through the pipe: Han's turn.
        let r = run(&app);
        let (room, placed) = (r.rooms[k].clone(), r.course.rooms[k]);
        teleport(&mut app, placed.start_col());
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(run(&app).current, Some(k));
        let s = side(&app).clone();
        let cols = s.room.expect("a free play Side B room").cols;
        assert_eq!(cols, (placed.col0, placed.col0 + placed.width));
        assert_eq!(s.setlist, alone::setlist());
        assert!(col(&mut app) < (placed.col0 + 4) as f32, "Han comes in through the pipe");

        // No calls: Han's stuck (or done for, and back at the room's start); never through.
        until(&mut app, 30.0, rewound);
        if rewound(&mut app) {
            until(&mut app, 5.0, |a| side(a).phase == Phase::Playing);
            assert_eq!(side(&app).rewinds, 1);
            assert!(col(&mut app) < (placed.col0 + 4) as f32, "back at the room's start");
        } else {
            assert_eq!(side(&app).phase, Phase::Playing, "not through without the band");
        }
        assert_eq!(run(&app).current, Some(k), "the run's where it was");
        // (Back to the room's start, as a death would.)
        app.world_mut().resource_mut::<SideB>().phase = Phase::Rewind { t: 0.01 };
        until(&mut app, 1.0, |a| side(a).phase == Phase::Playing);

        // The plan it was drawn with clears it.
        let mut c = Canvas::with_height(alone::ALONE_H);
        let mut d = dice::Dice::new(dice::stream(seed, ROOM_STREAM, room.plan.index as u64 * 1024 + room.attempts as u64 - 1));
        let plan = alone::draw(&mut c, &mut d, room.plan.request.band, room.plan.side_b as usize).solution;
        // (Han starts back at the pipe each try: the plan's columns are the room's.)
        let at = |c: f32| c + placed.col0 as f32;
        for &(c, k) in &plan {
            until(&mut app, 30.0, |a| col(a) >= at(c) || side(a).phase != Phase::Playing);
            let now = col(&mut app);
            assert_eq!(side(&app).phase, Phase::Playing, "Han died at {now:.1} ({:?}), plan {plan:?}", fate(&app));
            act(&mut app, k);
        }
        until(&mut app, 40.0, |a| a.world().get_resource::<SideB>().is_none());
        assert!(app.world().get_resource::<SideB>().is_none(), "Han clears the room");
        assert!(said(&app, sideb::ROOM_CLEARED_LINE));

        // Nat's back, out of the room's exit, and the run carries on.
        let nat = app.world_mut().query_filtered::<(&Pos, Option<&Visibility>), With<Player>>().single(app.world()).map(|(p, v)| (p.0, v.copied())).unwrap();
        assert_ne!(nat.1, Some(Visibility::Hidden));
        assert!(nat.0.x > (cols.1 - 8) as f32 * TILE && nat.0.x < cols.1 as f32 * TILE, "Nat at the exit: {}", nat.0.x / TILE);
        KeyCode::ArrowRight.press(app.world_mut());
        let before = nat.0.x;
        for _ in 0..30 {
            app.update();
        }
        KeyCode::ArrowRight.release(app.world_mut());
        let after = app.world_mut().query_filtered::<&Pos, With<Player>>().single(app.world()).unwrap().0.x;
        assert!(after > before + TILE, "Nat runs again");
        let mut frames = 0;
        while run(&app).course.rooms.len() <= k + 1 {
            app.update();
            std::thread::sleep(Duration::from_millis(1));
            frames += 1;
            assert!(frames < 20_000, "the next room never came");
        }
        let next = run(&app).course.rooms[k + 1].start_col();
        teleport(&mut app, next);
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(run(&app).current, Some(k + 1), "on to the next room");
        assert!(run(&app).stats.deaths == 0 && run(&app).cleared as usize == k + 1);
        return;
    }
    panic!("no run served a Side B room");
}

/// A Side B room expects the rewinds learning it takes, as a grease chute expects its splats:
/// one a piece, two a fork; and its par time allows for them.
#[test]
fn free_play_side_b_rooms_expect_rewinds() {
    use nat_han_adventures::adapt::{AssistLevers, RoomRequest, Skill};
    use nat_han_adventures::freeplay::generate::{Job, RoomPlan};
    use nat_han_adventures::freeplay::templates::Unlocks;
    for (band, cleared) in [(1, 1), (10, 3)] {
        let request = RoomRequest { skill: Skill::Buddy, band, assists: AssistLevers::NONE };
        let room = Job::new(7, RoomPlan::alone(7, 2, request, 1, Unlocks { levels: 10, side_b: cleared, solo: 0 })).run();
        assert!(room.plan.template().alone());
        assert!((2..=12).contains(&room.expected_deaths), "band {band}: {}", room.expected_deaths);
        let walk = room.level.width as f32 * TILE / 70.0;
        assert!(room.par_secs > walk * (1.0 + room.expected_deaths as f32 / 2.0), "par {} for {} rewinds", room.par_secs, room.expected_deaths);
    }
}
