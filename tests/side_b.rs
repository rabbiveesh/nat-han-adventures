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
// set-list call `k` (0 giant, 1 gap, 2 melt) once Han's past column `col`; `col/1k` only picks
// it, for the nuggets ahead. The set lists start empty: every call is a nugget Han grabbed.
const ROUTES: [&[(&str, &str)]; SIDE_B_COUNT] = [
    // Down the Drain: over the deck (its nuggets) or through the tunnel (one).
    &[("over", "7/0 52/0 75/11 95/10 106/1 136/0"), ("under", "5/11 7/0 106/1 136/0")],
    // Spike Alley: over the alley's roof or through it.
    &[("over", "5/12 7/0 30/11 55/0 82/2 110/1"), ("under", "5/11 25/12 55/0 82/2 110/1")],
    // Drip Pipe: over and leap the deck's gap, over and drop through it, or under both drips.
    &[
        ("leap", "5/11 9/0 38/12 45/1 92/0 120/1 152/2"),
        ("drop", "5/12 7/11 9/0 38/12 50/2 92/0 120/1 152/2"),
        ("under", "3/12 7/11 9/2 50/2 52/10 92/0 120/1 152/2"),
    ],
    // The Big Flush: the alley and the deck, every route's nuggets exact.
    &[
        ("under, under", "5/12 7/11 9/0 80/12 112/2 142/1 174/2 215/10 221/2 223/11 260/0 286/1"),
        ("over, leap", "7/12 9/0 55/0 78/11 86/10 112/2 137/11 142/1 172/0 208/11 215/1 260/0 286/1"),
        ("over, drop", "7/12 9/0 55/0 78/11 86/10 112/2 137/12 142/1 172/0 208/11 220/2 260/0 286/1"),
    ],
    // Rush Hour.
    &[
        ("under, under", "4/12 6/11 8/0 83/0 98/2 101/1 140/12 147/2 186/2 189/10 230/1 252/0"),
        ("over, leap", "6/11 8/0 40/0 68/12 80/0 99/2 101/1 102/10 140/0 142/11 175/10 183/1 230/1 252/0"),
    ],
    // Double Trouble.
    &[
        ("under, under", "3/12 5/2 5/11 7/10 9/12 11/1 70/12 90/0 92/2 127/2 165/10 172/2 174/11 210/0 226/1"),
        ("over, leap", "3/12 5/2 5/11 7/10 11/1 45/0 66/12 76/10 92/0 94/2 120/11 127/0 158/11 167/1 210/0 226/1"),
    ],
    // The Royal Flush.
    &[
        (
            "under, under, under",
            "4.5/12 7.5/11 10.5/12 14/0 85.5/12 112/2 137.5/10 142/1 174/2 221/2 223.5/10 260/0 262/2 295.5/11 \
             333.5/10 356/0 372/1",
        ),
        (
            "over, leap, over",
            "7.5/12 10.5/11 14/0 55/0 88.5/11 112/2 137.5/10 142/1 172/0 203.5/12 208.5/10 217/1 260/0 262/2 \
             295.5/10 306/0 330.5/11 340.5/10 356/0 372/1",
        ),
    ],
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
        ("slide, drip, flume", "1/10 44/0 70/11 158/1 180/10 200/0 236/0"),
        ("drain, drain, ferries", ""),
        ("deck, drain, ferries", "1/10 40/0"),
        ("drain, drip, ferries", "1/11 158/1"),
        ("drain, drain, flume", "1/10 200/0"),
    ],
];

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
    // Nothing to start with: the calls are the nuggets Han grabs.
    assert_eq!(b.setlist, vec![(SetCall::Mode(Harmony::Coltrane), 0), (SetCall::Mode(Harmony::Quartal), 0)]);
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
fn nuggets_add_to_the_picked_call() {
    let mut app = app(8);
    pick(&mut app, 2);
    until(&mut app, 5.0, |a| col(a) >= 13.0).unwrap();
    assert_eq!(side(&app).refills, 2);
    assert_eq!(side(&app).left, vec![0, 0, 2], "both runway nuggets to melting");
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
        if k >= PICK { pick(&mut app, k - PICK) } else { call(&mut app, k) }
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
            play_in_app(&mut app, &plan(ROUTES[level][route].1)).unwrap();
            let name = ROUTES[level][route].0;
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
fn jump_calls_the_band_too() {
    let mut app = app(0);
    until(&mut app, 5.0, |a| col(a) >= 7.0).unwrap();
    assert_eq!(side(&app).left, vec![2, 0], "the runway's two nuggets");
    tap(&mut app, KeyCode::Space);
    assert_eq!(side(&app).calls, 1);
    assert_eq!(side(&app).left, vec![1, 0]);
}

#[test]
fn a_rewind_starts_the_tune_over() {
    let mut app = app(0);
    app.init_resource::<Started>()
        .add_systems(Last, |mut n: ResMut<Started>, mut s: MessageReader<nat_han_adventures::audio::MusicStarted>| n.0 += s.read().count());
    until(&mut app, 10.0, rewound).unwrap();
    let before = app.world().resource::<Started>().0;
    until(&mut app, 4.0, |a| side(a).rewinds == 1).unwrap();
    until(&mut app, 0.2, |_| false);
    assert_eq!(app.world().resource::<Started>().0, before + 1, "the tune starts over");
}

#[derive(Resource, Default)]
struct Started(usize);

#[test]
fn every_route_of_every_level() {
    for (n, routes) in ROUTES.iter().enumerate() {
        assert!(routes.len() >= 2, "level {} forks", n + 1);
        for (name, p) in routes.iter() {
            assert_eq!(play(n, &plan(p)), Ok(()), "level {}, {name}", n + 1);
        }
    }
}

#[test]
fn with_no_music_han_charges_the_wall_and_knocks_himself_out() {
    let mut app = app(0);
    until(&mut app, 10.0, rewound).expect("Han charges the giant wall and comes off worst");
    assert_eq!(fate(&app), Some(Fate::Bonk));
    assert!(said(&app, sideb::CLIMB_LINE));
    assert!(said(&app, Fate::Bonk.line()));
    until(&mut app, 3.0, |a| side(a).rewinds == 1).expect("the level rewinds");
    assert!(col(&mut app) < 5.0, "Han's back at the start");
}

#[test]
fn no_fired_up_for_the_gap_is_a_long_way_down() {
    let (f, c) = died(play(0, &plan("5/11 7/0 136/0")));
    assert_eq!(f, Fate::Pit);
    assert!((120.0..134.0).contains(&c), "at the gap, not {c}");
}

#[test]
fn han_hops_spike_alley_then_bonks_the_wall() {
    let (f, c) = died(play(1, &[]));
    assert_eq!(f, Fate::Bonk, "through the spikes, then the wall");
    assert!((64.0..73.0).contains(&c), "at the wall, not {c}");
}

#[test]
fn giant_steps_in_spike_alley_floats_han_into_the_ceiling() {
    // Called once he's under the alley (past the step up to its roof).
    let (f, c) = died(play(1, &plan("20/0")));
    assert_eq!(f, Fate::Spikes);
    assert!((17.0..48.0).contains(&c), "under the alley's ceiling, not {c}");
}

#[test]
fn spikes_kill_han_and_the_level_rewinds() {
    let mut app = app(1);
    until(&mut app, 5.0, |a| col(a) >= 8.0).unwrap();
    assert_eq!(side(&app).calls_left(), 2, "the runway's nuggets");
    // Drop him on the first pit's spikes.
    let level = app.world().resource::<nat_han_adventures::game::ActiveLevel>().level.clone();
    let spot = level.tile_center(23, 10);
    let mut q = app.world_mut().query_filtered::<(&mut Pos, &mut nat_han_adventures::game::PrevPos, &mut nat_han_adventures::game::Body), With<Han>>();
    let (mut p, mut pp, mut body) = q.single_mut(app.world_mut()).unwrap();
    (p.0, pp.0, body.vel) = (spot, spot, Vec2::new(0.0, -200.0));
    until(&mut app, 0.5, rewound).expect("spikes are deadly on Side B");
    assert_eq!(fate(&app), Some(Fate::Spikes));
    until(&mut app, 3.0, |a| side(a).rewinds == 1).expect("the level rewinds");
    assert_eq!(side(&app).calls_left(), 0, "the nuggets go back where they were");
    until(&mut app, 5.0, |a| col(a) >= 8.0).unwrap();
    assert_eq!(side(&app).calls_left(), 2, "to be grabbed again");
}

#[test]
fn full_size_han_loses_his_cap_in_the_drip_pipe() {
    let mut app = app(2);
    until(&mut app, 10.0, rewound).unwrap();
    assert_eq!(fate(&app), Some(Fate::Spikes));
    assert!((20.0..30.0).contains(&col(&mut app)), "at the first drip's mouth");
}

#[test]
fn melting_too_late_is_too_late() {
    // Drip Pipe's leap, melting for the last pipe called as he reaches it: the bar line's too
    // far off.
    let (f, c) = died(play(2, &plan("5/11 9/0 38/12 45/1 92/0 120/1 162/2")));
    assert_eq!(f, Fate::Spikes);
    assert!(c > 165.0, "at the last pipe, not {c}");
}

#[test]
fn a_cleared_level_goes_on_to_the_next() {
    let mut app = app(0);
    for (c, k) in plan(ROUTES[0][1].1) {
        until(&mut app, 30.0, |a| col(a) >= c).unwrap();
        if k >= PICK {
            pick(&mut app, k - PICK);
        } else {
            call(&mut app, k);
        }
    }
    until(&mut app, 30.0, |a| matches!(side(a).phase, Phase::Cleared { .. })).expect("cleared");
    until(&mut app, 6.0, |a| side(a).level == 1).expect("on to Spike Alley");
    until(&mut app, 0.1, |_| false);
    assert_eq!(side(&app).phase, Phase::Playing);
    assert_eq!(side(&app).left, vec![0, 0, 0], "Spike Alley's set list, empty");
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
    for (c, k) in plan(ROUTES[n][0].1) {
        until(&mut app, 60.0, |a| col(a) >= c).unwrap();
        if k >= PICK {
            pick(&mut app, k - PICK);
        } else {
            call(&mut app, k);
        }
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
    // ...and back down he goes, deeper.
    until(&mut app, 5.0, |a| phase(a) == EncorePhase::Continued).expect("to be continued");
    let bowl = app.world().resource::<Encore>().bowl();
    assert!(nat(&mut app).y < bowl.y && (nat(&mut app).x - bowl.x).abs() < 4.0, "down the bowl");
    assert!(said(&app, "...oops."));
    until(&mut app, 5.0, |a| a.world().get_resource::<SideB>().is_none()).expect("left Side B");
    assert_eq!(*app.world().resource::<State<AppState>>().get(), AppState::LevelSelect);
    assert_eq!(*app.world().resource::<Shelf>(), Shelf::SideB);
}

/// Pick set-list call `k` (Left/Right) and call it (Down).
fn call(app: &mut App, k: usize) {
    pick(app, k);
    tap(app, KeyCode::ArrowDown);
}

/// Pick set-list call `k` (Left/Right), not calling it: the call Han's next nugget adds to.
fn pick(app: &mut App, k: usize) {
    while side(app).selected < k {
        tap(app, KeyCode::ArrowRight);
    }
    while side(app).selected > k {
        tap(app, KeyCode::ArrowLeft);
    }
}

/// In a plan, `(col, PICK + k)` only picks call `k` ([`pick`]).
const PICK: usize = 10;

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
            eprintln!("call {k} at t={t:.2} col={:.1} left={:?}", col(app), side(app).left);
        }
        if k >= PICK {
            pick(app, k - PICK);
        } else {
            call(app, k);
        }
    }
    until(app, 90.0, |a| side(a).phase != Phase::Playing);
    match side(app).phase {
        Phase::Cleared { .. } => Ok(()),
        _ => Err((fate(app), col(app))),
    }
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
    let p = plan(&std::env::var("SIDE_B_PLAN").unwrap_or_default());
    let mut app = app_starting(move |c| sideb::start_level(c, slot, b));
    let r = play_in_app(&mut app, &p);
    let t = app.world().resource::<nat_han_adventures::game::LevelRun>().time;
    let bar = app.world().resource::<Groove>().clock.bar;
    eprintln!("{r:?} at {t:.1}s bar {bar}");
}

/// How much slack each call of a level's routes has: the shifts (in columns) that still clear
/// it, with the others where they are. `SIDE_B_TIGHT=5 cargo test --test side_b how_tight --
/// --ignored --nocapture`.
#[test]
#[ignore]
fn how_tight() {
    let n: usize = std::env::var("SIDE_B_TIGHT").unwrap().parse().unwrap();
    for (name, p) in ROUTES[n - 1] {
        let plan = plan(p);
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
    for (n, routes) in ROUTES.iter().enumerate() {
        if only.is_some_and(|o| o != n + 1) {
            continue;
        }
        for (name, p) in routes.iter() {
            let mut app = app(n);
            let mut calls = Vec::new();
            for &(c, k) in &plan(p) {
                until(&mut app, 30.0, |a| col(a) >= c || side(a).phase != Phase::Playing);
                if k >= PICK {
                    pick(&mut app, k - PICK);
                } else {
                    calls.push(format!("{}@{c} {}", ["giant", "gap", "melt"][k], clock(&app)));
                    call(&mut app, k);
                }
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
    (SideBLevel { level: c.finish_with(2, "Go!", 0), setlist: alone::setlist(cleared), turns: false }, drawn.solution)
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
    use nat_han_adventures::game::{Nugget, Player, PrevPos, stand_pos};

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
    fn nuggets_in(app: &mut App, cols: (usize, usize)) -> usize {
        let mut q = app.world_mut().query_filtered::<&Transform, With<Nugget>>();
        q.iter(app.world()).filter(|t| (cols.0 as f32 * TILE..cols.1 as f32 * TILE).contains(&t.translation.x)).count()
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
        assert_eq!(s.setlist, alone::setlist(3));
        assert!(s.left.iter().all(|&n| n == 0), "the set list starts empty");
        assert!(col(&mut app) < (placed.col0 + 4) as f32, "Han comes in through the pipe");
        let nuggets = nuggets_in(&mut app, cols);

        // No calls: Han's done for, and only the room rewinds.
        until(&mut app, 30.0, rewound);
        assert!(rewound(&mut app), "Han dies without the band");
        until(&mut app, 5.0, |a| side(a).phase == Phase::Playing);
        assert_eq!(side(&app).rewinds, 1);
        assert!(col(&mut app) < (placed.col0 + 4) as f32, "back at the room's start");
        assert_eq!(nuggets_in(&mut app, cols), nuggets, "the room's nuggets are back");
        assert_eq!(run(&app).current, Some(k), "the run's where it was");

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
            if k >= PICK {
                pick(&mut app, k - PICK);
            } else {
                call(&mut app, k);
            }
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
