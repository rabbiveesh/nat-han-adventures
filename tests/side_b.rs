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
        sideb::{self, Fate, Phase, SetCall, SideB},
    },
    level::TILE,
};

const DT: f64 = 1.0 / 60.0;

#[derive(Resource, Default)]
struct Heard(Vec<String>);

fn app(level: usize) -> App {
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
    app.world_mut().run_system_once(move |mut c: Commands| sideb::start(&mut c, level)).unwrap();
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
        eprintln!("t={t:5.2} han=({:.2},{:.2}) target={:?} frontier={} charging={:?} landings={} cx={:.2} phase={:?} groove={g:?}", p.x / TILE, p.y / TILE, s.target, s.frontier, s.charging, s.charge_landings, s.charge_x / TILE, s.phase);
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

// The levels' solutions (and near misses) are `play` plans: (Han's column, set-list call).
// Level 1's set list: giant ×2, gap. Level 2: giant, melt, gap. Level 3: melt ×2, giant, gap.
// Level 4: giant ×3, gap ×2, melt ×2.
const L1: [(f32, usize); 3] = [(3.0, 0), (22.0, 1), (50.0, 0)];
const L2: [(f32, usize); 3] = [(30.0, 0), (55.0, 1), (80.0, 2)];
const L3: [(f32, usize); 4] = [(3.0, 0), (42.0, 1), (56.0, 2), (100.0, 0)];
const L4: [(f32, usize); 7] = [(3.0, 0), (22.0, 1), (78.0, 0), (92.0, 2), (125.0, 1), (160.0, 2), (190.0, 0)];
// The crushing ones, set lists with nothing to spare (found by `solve_a_level`). Rush Hour:
// giant ×2, gap, melt ×2. Double Trouble: giant ×3, gap ×2, melt ×3. The Royal Flush: giant ×6,
// gap ×3, melt ×4.
const L5: [(f32, usize); 5] = [(1.0, 0), (32.6, 1), (52.9, 2), (100.8, 0), (110.8, 2)];
const L6: [(f32, usize); 8] = [(1.0, 2), (2.0, 1), (23.8, 0), (27.8, 2), (86.6, 0), (87.9, 2), (107.6, 1), (141.6, 0)];
const L7: [(f32, usize); 13] = [
    (1.0, 0),
    (23.0, 1),
    (43.6, 0),
    (44.6, 2),
    (61.6, 1),
    (94.6, 0),
    (120.8, 2),
    (136.8, 0),
    (143.8, 2),
    (167.6, 1),
    (200.6, 0),
    (221.8, 2),
    (222.8, 0),
];

fn died(r: Result<(), (Option<Fate>, f32)>) -> (Fate, f32) {
    match r {
        Err((Some(f), c)) => (f, c),
        other => panic!("expected Han to come off worst, got {other:?}"),
    }
}

#[test]
fn level_one_parses_with_its_set_list() {
    let b = sideb::SideBLevel::load(0);
    assert_eq!(b.setlist, vec![(SetCall::Mode(Harmony::Coltrane), 2), (SetCall::Mode(Harmony::Quartal), 1)]);
}

#[test]
fn every_level_has_a_solution() {
    for (n, plan) in [&L1[..], &L2, &L3, &L4, &L5, &L6, &L7].into_iter().enumerate() {
        assert_eq!(play(n, plan), Ok(()), "level {}", n + 1);
    }
}

#[test]
fn with_no_music_han_charges_the_wall_and_knocks_himself_out() {
    let mut app = app(0);
    until(&mut app, 10.0, rewound).expect("Han charges the giant wall and comes off worst");
    assert_eq!(fate(&app), Some(Fate::Bonk));
    assert!(said(&app, "I can climb that. Probably!"));
    assert!(said(&app, Fate::Bonk.line()));
    until(&mut app, 3.0, |a| side(a).rewinds == 1).expect("the level rewinds");
    assert!(col(&mut app) < 5.0, "Han's back at the start");
}

#[test]
fn no_fired_up_for_the_gap_is_a_long_way_down() {
    let (f, c) = died(play(0, &L1[..1]));
    assert_eq!(f, Fate::Pit);
    assert!((38.0..50.0).contains(&c), "at the gap, not {c}");
}

#[test]
fn giant_steps_called_too_early_for_the_second_wall_wears_off() {
    // Called on top of the first wall it takes over from fired up: down the gap.
    assert_eq!(died(play(0, &[(3.0, 0), (22.0, 1), (28.0, 0)])).0, Fate::Pit);
}

#[test]
fn han_hops_spike_alley_then_bonks_the_wall() {
    let (f, c) = died(play(1, &[]));
    assert_eq!(f, Fate::Bonk, "through the spikes, then the wall");
    assert!((40.0..45.0).contains(&c));
}

#[test]
fn giant_steps_in_spike_alley_floats_han_into_the_ceiling() {
    let (f, c) = died(play(1, &[(3.0, 0)]));
    assert_eq!(f, Fate::Spikes);
    assert!(c < 33.0, "under the alley's ceiling, not {c}");
}

#[test]
fn spikes_kill_han_and_the_level_rewinds() {
    let mut app = app(1);
    until(&mut app, 0.3, |_| false);
    tap(&mut app, KeyCode::ArrowDown);
    assert_eq!(side(&app).calls_left(), 2);
    // Drop him on the first pit's spikes.
    let level = app.world().resource::<nat_han_adventures::game::ActiveLevel>().level.clone();
    let spot = level.tile_center(11, 10);
    let mut q = app.world_mut().query_filtered::<(&mut Pos, &mut nat_han_adventures::game::PrevPos, &mut nat_han_adventures::game::Body), With<Han>>();
    let (mut p, mut pp, mut body) = q.single_mut(app.world_mut()).unwrap();
    (p.0, pp.0, body.vel) = (spot, spot, Vec2::new(0.0, -200.0));
    until(&mut app, 0.5, rewound).expect("spikes are deadly on Side B");
    assert_eq!(fate(&app), Some(Fate::Spikes));
    until(&mut app, 3.0, |a| side(a).rewinds == 1).expect("the level rewinds");
    assert_eq!(side(&app).calls_left(), 3, "with the set list back");
}

#[test]
fn full_size_han_loses_his_cap_in_the_drip_pipe() {
    let mut app = app(2);
    until(&mut app, 10.0, rewound).unwrap();
    assert_eq!(fate(&app), Some(Fate::Spikes));
    assert!(said(&app, "Duck and run! Duck and run!"));
    assert!(col(&mut app) < 25.0);
}

#[test]
fn melting_too_late_is_too_late() {
    // The second drip pipe, melting called as he reaches it: the bar line's too far off.
    let (f, c) = died(play(2, &[(3.0, 0), (42.0, 1), (56.0, 2), (112.0, 0)]));
    assert_eq!(f, Fate::Spikes);
    assert!(c > 110.0, "at the second pipe, not {c}");
}

#[test]
fn giant_steps_still_on_in_the_second_alley_is_spikes() {
    // Giant Steps for the wall, nothing to take over before the alley.
    let (f, c) = died(play(2, &[(3.0, 0), (42.0, 1)]));
    assert_eq!(f, Fate::Spikes);
    assert!((64.0..83.0).contains(&c), "in the alley, not {c}");
}

#[test]
fn a_cleared_level_goes_on_to_the_next() {
    let mut app = app(0);
    for &(c, k) in &L1 {
        until(&mut app, 30.0, |a| col(a) >= c).unwrap();
        call(&mut app, k);
    }
    until(&mut app, 30.0, |a| matches!(side(a).phase, Phase::Cleared { .. })).expect("cleared");
    until(&mut app, 6.0, |a| side(a).level == 1).expect("on to Spike Alley");
    until(&mut app, 0.1, |_| false);
    assert_eq!(side(&app).phase, Phase::Playing);
    assert_eq!(side(&app).left, vec![1, 1, 1], "Spike Alley's set list");
    assert!(col(&mut app) < 5.0, "Han's at its start");
}

/// Pick set-list call `k` (Left/Right) and call it (Down).
fn call(app: &mut App, k: usize) {
    while side(app).selected < k {
        tap(app, KeyCode::ArrowRight);
    }
    while side(app).selected > k {
        tap(app, KeyCode::ArrowLeft);
    }
    tap(app, KeyCode::ArrowDown);
}

/// Play Side B level `level`, calling set-list call `k` once Han's past column `col`, for each
/// `(col, k)` in turn: cleared, or how Han met his end (and where).
fn play(level: usize, plan: &[(f32, usize)]) -> Result<(), (Option<Fate>, f32)> {
    let mut app = app(level);
    for &(c, k) in plan {
        until(&mut app, 30.0, |a| col(a) >= c || side(a).phase != Phase::Playing);
        if side(&app).phase != Phase::Playing {
            break;
        }
        if std::env::var("SIDE_B_TIMES").is_ok() {
            let t = app.world().resource::<nat_han_adventures::game::LevelRun>().time;
            eprintln!("call {k} at t={t:.2}");
        }
        call(&mut app, k);
    }
    until(&mut app, 40.0, |a| side(a).phase != Phase::Playing);
    match side(&app).phase {
        Phase::Cleared { .. } => Ok(()),
        _ => Err((fate(&app), col(&mut app))),
    }
}

/// `SIDE_B_PLAN="1: 3/0 22/1"` plays level 1 calling call 0 at column 3, call 1 at 22 (to find
/// a level's solution): `cargo test --test side_b try_a_plan -- --ignored --nocapture`.
#[test]
#[ignore]
fn try_a_plan() {
    let s = std::env::var("SIDE_B_PLAN").unwrap();
    let (lvl, rest) = s.split_once(':').unwrap();
    let plan: Vec<(f32, usize)> = rest
        .split_whitespace()
        .map(|p| {
            let (c, k) = p.split_once('/').unwrap();
            (c.parse().unwrap(), k.parse().unwrap())
        })
        .collect();
    eprintln!("{:?}", play(lvl.trim().parse::<usize>().unwrap() - 1, &plan));
}

/// Plan search for a level (to find a new level's solution): a beam of the plans that get Han
/// furthest, each grown by one call (any left in the set list, at columns back to 40 before
/// where he died). `SIDE_B_SOLVE=5 [SIDE_B_BEAM=4] cargo test --test side_b solve_a_level --
/// --ignored --nocapture`.
#[test]
#[ignore]
fn solve_a_level() {
    let n: usize = std::env::var("SIDE_B_SOLVE").unwrap().parse::<usize>().unwrap() - 1;
    let beam: usize = std::env::var("SIDE_B_BEAM").ok().and_then(|v| v.parse().ok()).unwrap_or(4);
    let setlist = sideb::SideBLevel::load(n).setlist;
    let score = |p: &[(f32, usize)]| match play(n, p) {
        Ok(()) => f32::INFINITY,
        Err((_, d)) => d,
    };
    // (`SIDE_B_PREFIX="3/0 22/1"`: start from these calls.)
    let prefix: Vec<(f32, usize)> = std::env::var("SIDE_B_PREFIX")
        .unwrap_or_default()
        .split_whitespace()
        .map(|p| {
            let (c, k) = p.split_once('/').unwrap();
            (c.parse().unwrap(), k.parse().unwrap())
        })
        .collect();
    let mut states: Vec<(Vec<(f32, usize)>, f32)> = vec![(prefix.clone(), score(&prefix))];
    loop {
        let mut children: Vec<(Vec<(f32, usize)>, f32)> = Vec::new();
        for (plan, at) in &states {
            let from = plan.last().map_or(1.0, |p| p.0 + 1.0).max(at - 40.0);
            for (k, &(_, count)) in setlist.iter().enumerate() {
                if plan.iter().filter(|p| p.1 == k).count() >= count as usize {
                    continue;
                }
                let mut c = from;
                while c < *at {
                    let mut p = plan.clone();
                    p.push((c, k));
                    let got = score(&p);
                    if got.is_infinite() {
                        return eprintln!("SOLVED: {p:?}");
                    }
                    if got > at + 0.5 {
                        children.push((p, got));
                    }
                    c += 3.0;
                }
            }
        }
        if children.is_empty() {
            return eprintln!("stuck: {states:?}");
        }
        children.sort_by(|a, b| b.1.total_cmp(&a.1));
        // Distinct outcomes first (many plans die in the same spot).
        let mut next: Vec<(Vec<(f32, usize)>, f32)> = Vec::new();
        for c in &children {
            if next.len() < beam && next.iter().all(|x| (x.1 - c.1).abs() > 0.3) {
                next.push(c.clone());
            }
        }
        eprintln!("best {:.1}: {:?}", next[0].1, next[0].0);
        states = next;
    }
}

/// How much slack each call of a level's solution has: the shifts (in columns) that still clear
/// it, with the others where they are. `SIDE_B_TIGHT=5 cargo test --test side_b how_tight --
/// --ignored --nocapture`.
#[test]
#[ignore]
fn how_tight() {
    let n: usize = std::env::var("SIDE_B_TIGHT").unwrap().parse().unwrap();
    let plan: &[(f32, usize)] = [&L1[..], &L2, &L3, &L4, &L5, &L6, &L7][n - 1];
    for i in 0..plan.len() {
        let ok: Vec<i32> = [-6, -4, -2, 2, 4, 6]
            .into_iter()
            .filter(|&d| {
                let mut p = plan.to_vec();
                p[i].0 += d as f32;
                (i == 0 || p[i].0 > p[i - 1].0) && play(n - 1, &p).is_ok()
            })
            .collect();
        eprintln!("call {i} {:?}: clears with shifts {ok:?}", plan[i]);
    }
}
