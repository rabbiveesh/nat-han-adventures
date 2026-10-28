//! The showcase levels and the scripted runs their demo videos record:
//! - the sky things' playground (`levels/toybox/boom.txt`, `scripts/tours/boom.timeline`):
//!   both Booms and every diaper, without a death;
//! - the secret gadget gauntlet (`levels/secret/gauntlet.txt`, `scripts/tours/gauntlet.timeline`):
//!   every gadget tile in one hard level, beaten without a death;
//! - NAT ALONE, the gadget world (`game::solo`, `levels/solo/`): each of its ten levels has a
//!   run (`scripts/tours/<level>.timeline`) that clears it without a death (Breezy's fork has
//!   two more: `breezy-alt.timeline` fired up over the canal, `breezy-tunnel.timeline` with no
//!   call, through the tunnel and its secret; Bouncy Castle's too: `bouncy-alt.timeline` fired up
//!   over the moat, `bouncy-dungeon.timeline` with no call, through the dungeon and its secret; Up
//!   the Spout!'s: `spout-alt.timeline` fired up through the vault, `spout-stair.timeline` with
//!   no call, up the brick stair past its secret; Brush Hour's: `brush-alt.timeline` fired up
//!   along the express, `brush-underpass.timeline` with no call, through the underpass and its
//!   secret; Tilt!'s: `tilt-alt.timeline` fired up over the outlane, `tilt-outlane.timeline` with
//!   no call, through the outlane and its secret; Gone With the Wind's: `wind-alt.timeline` fired
//!   up down the low road's tailwind stair, `wind-sewer.timeline` with no call, through the sewer
//!   and its secret; Crusher Disco's: `disco-alt.timeline` fired up over the dance floor,
//!   `disco-cellar.timeline` with no call, through the cellar and its secret; Spin Cycle's:
//!   `spin-alt.timeline` fired up along the express, `spin-sump.timeline` with no call, through
//!   the sump, its secret and drum 2's low exit; Ping Pong Palace's: `pingpong-fired.timeline`
//!   fired up over the islands, `pingpong-cellar.timeline` with no call, through the cellar and
//!   its secret; The Grand Flush's: `flush-waltz.timeline` the waltz over the roof,
//!   `flush-fired.timeline` fired up along the express, `flush-sump.timeline` with no call,
//!   through the sump and the tenth secret).
//!
//! The runs play here the way the capture plays them (the keyboard-message path, 30 fps, the
//! music in lockstep driving the beat blocks and beat crushers).
//!
//! For writing a timeline: `SHOWCASE_TRACE=1` prints where Nat is every quarter second, the
//! bar lines, his toots, nuggets, band calls and secrets found, a wall cushion catching and slinging
//! him, the Boom catching him, its beans, a diaper dropping and splooshing, the helper's rope,
//! houses, blast buttons and stop signs, the harmony, and the crushers near him;
//! `SHOWCASE_LEVEL=<file>` / `SHOWCASE_TOUR=<file>` play another level / timeline in
//! `the_tour_beats_the_gauntlet_without_a_death` (`SHOWCASE_EVERY=n`: trace every n frames), and
//! `SHOWCASE_CHECK=<file>` prints the validator's report on a level file.
#![cfg(feature = "capture")]

use bevy::{input::InputPlugin, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use nat_han_adventures::{
    audio::{self, capture::AudioCapture},
    capture::{
        frame_duration,
        timeline::{Script, TimelinePlayer, play_timeline},
    },
    events::{BandFreedom, CallBand, Jumped, NuggetCollected, PlayerDied},
    game::{
        Body, Groove, Han, Player, Pos,
        boom::BoomEvent,
        crushers::CrusherBlock,
        diapers::{DiaperDropped, Splooshed},
        helper::RopeEvent,
        houses::HouseEvent,
        secret::SecretFound,
        toys::{Blasted, SignHit},
    },
    level::{Level, Levels, TILE, Topic, validate},
    state::AppState,
};

const GAUNTLET: &str = include_str!("../levels/secret/gauntlet.txt");
const GAUNTLET_TOUR: &str = include_str!("../scripts/tours/gauntlet.timeline");
const FPS: u32 = 30;

#[derive(Resource, Default)]
struct Deaths(Vec<(u64, Vec2)>);

#[derive(Resource, Default)]
struct Frame(u64);

/// What the trace reports between position lines: toots, nuggets, band calls, secrets.
#[derive(Resource, Default)]
struct Happened(Vec<&'static str>);

/// What a run did: its deaths (s, col, row), when it reached the goal (s), whether Han was in
/// the level, how many times it found the level's secret.
struct Run {
    deaths: Vec<(f32, f32, f32)>,
    goal: Option<f32>,
    end: f32,
    han: bool,
    secrets: usize,
    /// The freest the adaptive engine set the band's lead (a secret sets it loose).
    hottest: f32,
    /// When something happened (s): a key went down or up, a nugget, or a gadget kicked Nat (the
    /// density report's actions, see [`density`]).
    actions: Vec<f32>,
}

/// Actions for the density report: frames where a key went down or up, a nugget was taken, a
/// gadget kicked Nat (his speed jumped by more than [`KICK`] px/s in a frame he didn't jump in
/// and didn't land on: a duck, a spring, a cushion) or he blew into a fan's gust.
#[derive(Resource, Default)]
struct Actions(Vec<u64>, Option<Vec2>, (i32, i32));

const KICK: f32 = 120.0;

#[derive(Resource, Default)]
struct Secrets(usize);

#[derive(Resource, Default)]
struct Hottest(f32);

fn play(level: &str, tour: &str) -> Run {
    let level = Level::parse(level).unwrap();
    let rows = level.height as f32;
    let timeline = Script::parse(tour).unwrap().compile(FPS);
    let wav = std::env::temp_dir().join(format!("nathan-showcase-{}-{}.wav", std::process::id(), level.name.len()));
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin, InputPlugin, nat_han_adventures::gameplay))
        .insert_resource(TimeUpdateStrategy::ManualDuration(frame_duration(FPS).unwrap()))
        .insert_resource(Levels(vec![level]))
        .init_resource::<Deaths>()
        .init_resource::<Frame>()
        .add_systems(Last, |mut d: ResMut<Deaths>, f: Res<Frame>, mut r: MessageReader<PlayerDied>| {
            d.0.extend(r.read().map(|m| (f.0, m.pos)))
        })
        .init_resource::<Happened>()
        .init_resource::<Secrets>()
        .init_resource::<Hottest>()
        .init_resource::<Actions>()
        .add_systems(
            Last,
            |mut a: ResMut<Actions>,
             f: Res<Frame>,
             keys: Res<ButtonInput<KeyCode>>,
             mut n: MessageReader<NuggetCollected>,
             mut j: MessageReader<Jumped>,
             levels: Res<Levels>,
             nat: Query<(&Body, &Pos), With<Player>>| {
                let jumped = j.read().count() > 0;
                let mut act = keys.get_just_pressed().next().is_some() || keys.get_just_released().next().is_some();
                act |= n.read().count() > 0;
                if let Ok((b, p)) = nat.single() {
                    let level = &levels.0[0];
                    let cell = ((p.0.x / TILE).floor() as i32, (level.height as f32 - p.0.y / TILE).floor() as i32);
                    let wind = level.wind(cell.0, cell.1);
                    act |= wind != (0, 0) && wind != a.2;
                    a.2 = wind;
                    if let Some(was) = a.1 {
                        let up = b.vel.y - was.y;
                        let side = b.vel.x.abs() - was.x.abs();
                        act |= !jumped && !b.on_ground && (up > KICK || side > KICK);
                    }
                    a.1 = Some(b.vel);
                }
                if act {
                    a.0.push(f.0);
                }
            },
        )
        .add_systems(Last, |mut h: ResMut<Hottest>, mut r: MessageReader<BandFreedom>| {
            h.0 = r.read().fold(h.0, |hot, f| hot.max(f.lead))
        })
        .add_systems(
            Last,
            |mut h: ResMut<Happened>,
             mut secrets: ResMut<Secrets>,
             mut j: MessageReader<Jumped>,
             mut n: MessageReader<NuggetCollected>,
             mut c: MessageReader<CallBand>,
             mut s: MessageReader<SecretFound>,
             mut boom: MessageReader<BoomEvent>,
             mut dropped: MessageReader<DiaperDropped>,
             mut splooshed: MessageReader<Splooshed>,
             mut rope: MessageReader<RopeEvent>,
             mut house: MessageReader<HouseEvent>,
             mut blasted: MessageReader<Blasted>,
             mut sign: MessageReader<SignHit>| {
                h.0.extend(j.read().filter(|j| j.double).map(|_| "toot"));
                h.0.extend(boom.read().map(|e| match e {
                    BoomEvent::Caught(_) => "caught by the Boom",
                    BoomEvent::Bean(_) => "bean",
                    BoomEvent::Dry(_) => "the Boom ran dry",
                    BoomEvent::HoppedOff(_) => "hopped off the Boom",
                }));
                h.0.extend(rope.read().map(|e| match e {
                    RopeEvent::Grabbed(_) => "grabbed the rope",
                    RopeEvent::Snicker(_) => "the helper snickers",
                    RopeEvent::LetGo(_) => "he let go",
                    RopeEvent::JumpedOff(_) => "jumped off the rope",
                    RopeEvent::Hoisted(_) => "hoisted onto the ledge",
                }));
                h.0.extend(house.read().map(|e| match e {
                    HouseEvent::In(_) => "in the house",
                    HouseEvent::Sizzle(_) => "SIZZLE",
                    HouseEvent::Chimney(_) => "out of the chimney",
                }));
                h.0.extend(blasted.read().map(|_| "BLAST"));
                h.0.extend(sign.read().map(|_| "STOP"));
                h.0.extend(dropped.read().map(|_| "diaper drops"));
                h.0.extend(splooshed.read().map(|_| "sploosh"));
                h.0.extend(n.read().map(|_| "nugget"));
                h.0.extend(c.read().map(|_| "call"));
                let found = s.read().count();
                secrets.0 += found;
                h.0.extend((0..found).map(|_| "secret"));
            },
        );
    app.add_plugins(audio::plugin).insert_resource(AudioCapture::new(&wav, FPS).unwrap());
    app.finish();
    app.cleanup();
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::Playing);
    app.update();
    app.world_mut().resource_mut::<AudioCapture>().recording = true;
    app.insert_resource(TimelinePlayer::new(timeline.clone()))
        .add_systems(PreUpdate, play_timeline.before(bevy::input::InputSystems));
    let trace = std::env::var("SHOWCASE_TRACE").is_ok();
    let every = std::env::var("SHOWCASE_EVERY").ok().and_then(|e| e.parse().ok()).unwrap_or(FPS as u64 / 4);
    let secs = |f: u64| f as f32 / FPS as f32;
    let mut goal = None;
    let mut last_bar = None;
    let mut han = false;
    let mut caught = false;
    for frame in 0..timeline.frames {
        app.world_mut().resource_mut::<Frame>().0 = frame;
        app.update();
        han |= app.world_mut().query_filtered::<(), With<Han>>().iter(app.world()).next().is_some();
        let state = app.world().resource::<State<AppState>>().get().clone();
        if goal.is_none() && state == AppState::LevelComplete {
            goal = Some(secs(frame));
        }
        if !trace {
            continue;
        }
        let bar = app.world().resource::<Groove>().clock.bar;
        if last_bar != Some(bar) {
            eprintln!("t={:5.2} -- bar {bar}", secs(frame));
            last_bar = Some(bar);
        }
        let Some(p) = app.world_mut().query_filtered::<&Pos, With<Player>>().iter(app.world()).next().map(|p| p.0) else { continue };
        let mut happened = std::mem::take(&mut app.world_mut().resource_mut::<Happened>().0);
        let pad = app.world_mut().query_filtered::<&Body, With<Player>>().iter(app.world()).next().map_or(0.0, |b| b.pad);
        if (pad != 0.0) != caught {
            caught = pad != 0.0;
            happened.push(if caught { "caught" } else { "slung" });
        }
        if !happened.is_empty() {
            eprintln!("t={:5.2}   {}", secs(frame), happened.join(" "));
        }
        if frame % every == 0 {
            let harmony = app.world().resource::<Groove>().harmony;
            eprintln!("t={:5.2} col {:6.2} row {:6.2} {state:?} {harmony:?}", secs(frame), p.x / TILE, rows - p.y / TILE);
        }
        if frame % 3 == 0 {
            let near: Vec<_> = app
                .world_mut()
                .query::<(&CrusherBlock, &Pos)>()
                .iter(app.world())
                .filter(|(_, c)| (p.x - c.0.x).abs() < 4.0 * TILE)
                .map(|(b, c)| format!("crusher@{},{} {:+.2}", b.def.col, b.def.row, (c.0 - b.home).length() / TILE))
                .collect();
            if !near.is_empty() {
                eprintln!("t={:5.2}   nat {:6.2},{:6.2}  {}", secs(frame), p.x / TILE, rows - p.y / TILE, near.join("  "));
            }
        }
    }
    let _ = std::fs::remove_file(&wav);
    let deaths = app.world().resource::<Deaths>().0.iter().map(|(f, p)| (secs(*f), p.x / TILE, rows - p.y / TILE)).collect();
    let secrets = app.world().resource::<Secrets>().0;
    let hottest = app.world().resource::<Hottest>().0;
    let actions = app.world().resource::<Actions>().0.iter().map(|f| secs(*f)).collect();
    Run { deaths, goal, end: secs(timeline.frames), han, secrets, hottest, actions }
}

#[test]
fn the_gauntlet_is_a_valid_level_with_every_gadget() {
    let level = Level::parse(GAUNTLET).unwrap();
    let report = validate::check(&level, &validate::Options { dump: false });
    assert!(report.errs.is_empty(), "{:#?}", report.errs);
    let topics: Vec<_> = report.lessons.iter().map(|l| l.topic).collect();
    // (It came before the sideways and upside-down ones: NAT ALONE has those.)
    for t in Topic::GADGETS.into_iter().filter(|t| ![Topic::Wind, Topic::Fling].contains(t)) {
        assert!(topics.contains(&t), "no {t:?} in the gauntlet: {topics:?}");
    }
}

#[test]
fn the_tour_beats_the_gauntlet_without_a_death() {
    let file = |var: &str, default: &str| std::env::var(var).map(|p| std::fs::read_to_string(p).unwrap()).unwrap_or(default.to_string());
    let run = play(&file("SHOWCASE_LEVEL", GAUNTLET), &file("SHOWCASE_TOUR", GAUNTLET_TOUR));
    assert!(run.deaths.is_empty(), "deaths (s, col, row): {:?}", run.deaths);
    let goal = run.goal.expect("the tour reaches the goal");
    assert!(run.end - goal < 6.0, "the tour ends soon after the goal (goal at {goal}s)");
}

/// The sky things' playground (`levels/toybox/boom.txt`): its tour rides both Booms and
/// stops at every diaper's whistle, without a death.
#[test]
fn the_boom_tour_clears_its_playground_without_a_death() {
    let run = play(include_str!("../levels/toybox/boom.txt"), include_str!("../scripts/tours/boom.timeline"));
    assert!(run.deaths.is_empty(), "deaths (s, col, row): {:?}", run.deaths);
    assert!(run.goal.is_some(), "the tour reaches the goal");
}

/// A NAT ALONE level passes the full check, and its tour clears it without a death.
macro_rules! nat_alone {
    ($valid:ident, $clean:ident, $file:literal) => {
        #[test]
        fn $valid() {
            let level = Level::parse(include_str!(concat!("../levels/solo/", $file, ".txt"))).unwrap();
            assert!(level.solo);
            let report = validate::check(&level, &validate::Options { dump: false });
            assert!(report.errs.is_empty(), "{:#?}", report.errs);
        }

        #[test]
        fn $clean() {
            let level = include_str!(concat!("../levels/solo/", $file, ".txt"));
            let run = play(level, include_str!(concat!("../scripts/tours/", $file, ".timeline")));
            assert!(!run.han, "a solo level has no Han");
            // Only the deaths the level's design expects (`deaths:`: the splats that make the
            // band nervous for a grease chute).
            let expected = Level::parse(level).unwrap().deaths.unwrap_or(0) as usize;
            assert!(run.deaths.len() <= expected, "deaths (s, col, row): {:?}", run.deaths);
            let goal = run.goal.expect("the tour reaches the goal");
            assert!(run.end - goal < 6.0, "the tour ends soon after the goal (goal at {goal}s)");
        }
    };
}

nat_alone!(breezy_is_a_valid_level, the_tour_clears_breezy, "breezy");

/// Breezy's fork, the other way: the nugget lane fires the band up and the canal's long gaps
/// take Nat along under the rooftop (`breezy.timeline` calls Giant Steps and takes the rooftop).
#[test]
fn the_fired_up_tour_clears_breezy_over_the_canal() {
    let run = play(include_str!("../levels/solo/breezy.txt"), include_str!("../scripts/tours/breezy-alt.timeline"));
    assert!(run.deaths.is_empty(), "deaths (s, col, row): {:?}", run.deaths);
    let goal = run.goal.expect("the tour reaches the goal");
    assert!(run.end - goal < 6.0, "the tour ends soon after the goal (goal at {goal}s)");
}

/// Breezy's fork with no call: down into the tunnel, up the fan slot to the secret (Han behind a
/// grate, found once), the nerve fork's spiky low line, the plunger.
#[test]
fn the_no_call_tour_clears_breezy_through_the_tunnel_and_finds_the_secret() {
    let run = play(include_str!("../levels/solo/breezy.txt"), include_str!("../scripts/tours/breezy-tunnel.timeline"));
    assert!(run.deaths.is_empty(), "deaths (s, col, row): {:?}", run.deaths);
    assert_eq!(run.secrets, 1, "Nat finds the secret once");
    let loose = nat_han_adventures::game::adaptive::band_freedom(nat_han_adventures::adapt::BandMood { freedom: 1.0, intensity: 1.0 }).lead;
    assert!(run.hottest >= loose - 1e-4, "the secret sets the band loose ({} < {loose})", run.hottest);
    let goal = run.goal.expect("the tour reaches the goal");
    assert!(run.end - goal < 6.0, "the tour ends soon after the goal (goal at {goal}s)");
}

nat_alone!(bouncy_is_a_valid_level, the_tour_clears_bouncy, "bouncy");

/// Bouncy Castle's fork, the other way: the nugget lane fires the band up and the moat's belt
/// islands carry Nat across (`bouncy.timeline` calls the waltz and takes the keep).
#[test]
fn the_fired_up_tour_clears_bouncy_over_the_moat() {
    let run = play(include_str!("../levels/solo/bouncy.txt"), include_str!("../scripts/tours/bouncy-alt.timeline"));
    assert!(run.deaths.is_empty(), "deaths (s, col, row): {:?}", run.deaths);
    let goal = run.goal.expect("the tour reaches the goal");
    assert!(run.end - goal < 6.0, "the tour ends soon after the goal (goal at {goal}s)");
}

/// Bouncy Castle's fork with no call: down the moat into the dungeon, back to the secret (Han
/// behind a grate, found once), the loader and the belt, up the chimney, the stepping stones
/// under the roof (the long side of the backwards fork), the plunger.
#[test]
fn the_no_call_tour_clears_bouncy_through_the_dungeon_and_finds_the_secret() {
    let run = play(include_str!("../levels/solo/bouncy.txt"), include_str!("../scripts/tours/bouncy-dungeon.timeline"));
    assert!(run.deaths.is_empty(), "deaths (s, col, row): {:?}", run.deaths);
    assert_eq!(run.secrets, 1, "Nat finds the secret once");
    let goal = run.goal.expect("the tour reaches the goal");
    assert!(run.end - goal < 6.0, "the tour ends soon after the goal (goal at {goal}s)");
}
nat_alone!(brush_is_a_valid_level, the_tour_clears_brush, "brush");

/// Brush Hour's fork, the other way: the low lane's nuggets fire the band up, down the hole onto
/// the express's runway and across its islands, 12 apart (`brush.timeline` calls Giant Steps
/// and takes the gantry).
#[test]
fn the_fired_up_tour_clears_brush_along_the_express() {
    let run = play(include_str!("../levels/solo/brush.txt"), include_str!("../scripts/tours/brush-alt.timeline"));
    assert!(run.deaths.is_empty(), "deaths (s, col, row): {:?}", run.deaths);
    let goal = run.goal.expect("the tour reaches the goal");
    assert!(run.end - goal < 6.0, "the tour ends soon after the goal (goal at {goal}s)");
}

/// Brush Hour's fork with no call: off the runway's end into the underpass, back against the
/// traffic to the secret (Han behind a grate, found once), the crates and the drain, then the
/// braid's slow lane and its on-ramp up to the fast lane.
#[test]
fn the_no_call_tour_clears_brush_through_the_underpass_and_finds_the_secret() {
    let run = play(include_str!("../levels/solo/brush.txt"), include_str!("../scripts/tours/brush-underpass.timeline"));
    assert!(run.deaths.is_empty(), "deaths (s, col, row): {:?}", run.deaths);
    assert_eq!(run.secrets, 1, "Nat finds the secret once");
    let goal = run.goal.expect("the tour reaches the goal");
    assert!(run.end - goal < 6.0, "the tour ends soon after the goal (goal at {goal}s)");
}
nat_alone!(spout_is_a_valid_level, the_tour_clears_spout, "spout");

/// Up the Spout!'s fork, the other way: out of the chimney the nuggets fire the band up, the
/// long gap and the elevator, then the kicker over the merge's spikes into the brick vault
/// (`spout.timeline` calls Giant Steps and takes the upper shaft, then the safe elevator).
#[test]
fn the_fired_up_tour_clears_spout_through_the_vault() {
    let run = play(include_str!("../levels/solo/spout.txt"), include_str!("../scripts/tours/spout-alt.timeline"));
    assert!(run.deaths.is_empty(), "deaths (s, col, row): {:?}", run.deaths);
    let goal = run.goal.expect("the tour reaches the goal");
    assert!(run.end - goal < 6.0, "the tour ends soon after the goal (goal at {goal}s)");
}

/// Up the Spout!'s fork with no call: down into the basement, back to the secret (Han behind a
/// grate, found once), over the rubble and up the brick stair, storey by storey.
#[test]
fn the_no_call_tour_clears_spout_up_the_brick_stair_and_finds_the_secret() {
    let run = play(include_str!("../levels/solo/spout.txt"), include_str!("../scripts/tours/spout-stair.timeline"));
    assert!(run.deaths.is_empty(), "deaths (s, col, row): {:?}", run.deaths);
    assert_eq!(run.secrets, 1, "Nat finds the secret once");
    let goal = run.goal.expect("the tour reaches the goal");
    assert!(run.end - goal < 6.0, "the tour ends soon after the goal (goal at {goal}s)");
}
nat_alone!(tilt_is_a_valid_level, the_tour_clears_tilt, "tilt");

/// Tilt!'s fork, the other way: the nugget line under the beat stairs fires the band up and the
/// launch lane's belt throws Nat over the outlane, island to island, then the doors on the beat
/// (`tilt.timeline` calls the waltz, springs off the flipper on ONE and takes the gallery).
#[test]
fn the_fired_up_tour_clears_tilt_over_the_outlane() {
    let run = play(include_str!("../levels/solo/tilt.txt"), include_str!("../scripts/tours/tilt-alt.timeline"));
    assert!(run.deaths.is_empty(), "deaths (s, col, row): {:?}", run.deaths);
    let goal = run.goal.expect("the tour reaches the goal");
    assert!(run.end - goal < 6.0, "the tour ends soon after the goal (goal at {goal}s)");
}

/// Tilt!'s fork with no call: down into the outlane, through the beat's door to the secret (Han
/// behind a grate, found once), along the outlane's road and up the drain's duck shaft.
#[test]
fn the_no_call_tour_clears_tilt_through_the_outlane_and_finds_the_secret() {
    let run = play(include_str!("../levels/solo/tilt.txt"), include_str!("../scripts/tours/tilt-outlane.timeline"));
    assert!(run.deaths.is_empty(), "deaths (s, col, row): {:?}", run.deaths);
    assert_eq!(run.secrets, 1, "Nat finds the secret once");
    let goal = run.goal.expect("the tour reaches the goal");
    assert!(run.end - goal < 6.0, "the tour ends soon after the goal (goal at {goal}s)");
}
nat_alone!(wind_is_a_valid_level, the_tour_clears_wind, "wind");

/// Gone With the Wind's fork, the other way: the nugget line under the ledges fires the band up,
/// the leap off the edge, then the low road's stair down (each riser a wall fan: a tailwind over
/// the crumbling floor below it) and up through the lid (`wind.timeline` calls Giant Steps and
/// takes the high road).
#[test]
fn the_fired_up_tour_clears_wind_down_the_low_road() {
    let run = play(include_str!("../levels/solo/wind.txt"), include_str!("../scripts/tours/wind-alt.timeline"));
    assert!(run.deaths.is_empty(), "deaths (s, col, row): {:?}", run.deaths);
    let goal = run.goal.expect("the tour reaches the goal");
    assert!(run.end - goal < 6.0, "the tour ends soon after the goal (goal at {goal}s)");
}

/// Gone With the Wind's fork with no call: off the edge into the sewer, back to the secret (Han
/// behind a grate, found once), over the sewage pits and up through the lid.
#[test]
fn the_no_call_tour_clears_wind_through_the_sewer_and_finds_the_secret() {
    let run = play(include_str!("../levels/solo/wind.txt"), include_str!("../scripts/tours/wind-sewer.timeline"));
    assert!(run.deaths.is_empty(), "deaths (s, col, row): {:?}", run.deaths);
    assert_eq!(run.secrets, 1, "Nat finds the secret once");
    let goal = run.goal.expect("the tour reaches the goal");
    assert!(run.end - goal < 6.0, "the tour ends soon after the goal (goal at {goal}s)");
}
nat_alone!(disco_is_a_valid_level, the_tour_clears_disco, "disco");

/// Crusher Disco's fork, the other way: the nugget line under the riser's steps fires the band
/// up, the leap off the dance floor's edge onto its islands (under their crushers on the beat),
/// then the dodge row under the high lane (`disco.timeline` calls Giant Steps, climbs the drum
/// riser and rides the lift crusher up to the high lane).
#[test]
fn the_fired_up_tour_clears_disco_over_the_dance_floor() {
    let run = play(include_str!("../levels/solo/disco.txt"), include_str!("../scripts/tours/disco-alt.timeline"));
    assert!(run.deaths.is_empty(), "deaths (s, col, row): {:?}", run.deaths);
    let goal = run.goal.expect("the tour reaches the goal");
    assert!(run.end - goal < 6.0, "the tour ends soon after the goal (goal at {goal}s)");
}

/// Crusher Disco's fork with no call: off the dance floor's edge into the cellar, back to the
/// secret (Han behind a grate, found once), the cellar's beat stones and stairs, the lift's ride.
#[test]
fn the_no_call_tour_clears_disco_through_the_cellar_and_finds_the_secret() {
    let run = play(include_str!("../levels/solo/disco.txt"), include_str!("../scripts/tours/disco-cellar.timeline"));
    assert!(run.deaths.is_empty(), "deaths (s, col, row): {:?}", run.deaths);
    assert_eq!(run.secrets, 1, "Nat finds the secret once");
    let goal = run.goal.expect("the tour reaches the goal");
    assert!(run.end - goal < 6.0, "the tour ends soon after the goal (goal at {goal}s)");
}
nat_alone!(spin_is_a_valid_level, the_tour_clears_spin, "spin");

/// Spin Cycle's fork, the other way: four quick nuggets in the lint lane fire the band up, the
/// express's belt islands over the sump, drum 2 round at fired-up speed (`spin.timeline` calls
/// the waltz on the table and springs on ONE up onto the dryer).
#[test]
fn the_fired_up_tour_clears_spin_along_the_express() {
    let run = play(include_str!("../levels/solo/spin.txt"), include_str!("../scripts/tours/spin-alt.timeline"));
    assert!(run.deaths.is_empty(), "deaths (s, col, row): {:?}", run.deaths);
    let goal = run.goal.expect("the tour reaches the goal");
    assert!(run.end - goal < 6.0, "the tour ends soon after the goal (goal at {goal}s)");
}

/// Spin Cycle's fork with no call: the lint trap's secret (Han behind a grate, found once), the
/// sump against its belt, the one-way hatch, and drum 2's low exit through the spin-dry.
#[test]
fn the_no_call_tour_clears_spin_through_the_sump_and_finds_the_secret() {
    let run = play(include_str!("../levels/solo/spin.txt"), include_str!("../scripts/tours/spin-sump.timeline"));
    assert!(run.deaths.is_empty(), "deaths (s, col, row): {:?}", run.deaths);
    assert_eq!(run.secrets, 1, "Nat finds the secret once");
    let goal = run.goal.expect("the tour reaches the goal");
    assert!(run.end - goal < 6.0, "the tour ends soon after the goal (goal at {goal}s)");
}
nat_alone!(pingpong_is_a_valid_level, the_tour_clears_pingpong, "pingpong");

/// Ping Pong Palace's fork, the other way: four quick nuggets in the landing's trench fire the
/// band up, the islands ten apart, then the bank shot (`pingpong.timeline` calls Giant Steps up
/// the chimney and takes the gallery).
#[test]
fn the_fired_up_tour_clears_pingpong_over_the_islands() {
    let run = play(include_str!("../levels/solo/pingpong.txt"), include_str!("../scripts/tours/pingpong-fired.timeline"));
    assert!(run.deaths.is_empty(), "deaths (s, col, row): {:?}", run.deaths);
    let goal = run.goal.expect("the tour reaches the goal");
    assert!(run.end - goal < 6.0, "the tour ends soon after the goal (goal at {goal}s)");
}

/// Ping Pong Palace's fork with no call: the secret under the landing (Han behind a grate, found
/// once), the cellar's low hops under the cushions, the elevator's hatch, and the tunnel instead
/// of the bank shot.
#[test]
fn the_no_call_tour_clears_pingpong_through_the_cellar_and_finds_the_secret() {
    let run = play(include_str!("../levels/solo/pingpong.txt"), include_str!("../scripts/tours/pingpong-cellar.timeline"));
    assert!(run.deaths.is_empty(), "deaths (s, col, row): {:?}", run.deaths);
    assert_eq!(run.secrets, 1, "Nat finds the secret once");
    let goal = run.goal.expect("the tour reaches the goal");
    assert!(run.end - goal < 6.0, "the tour ends soon after the goal (goal at {goal}s)");
}
nat_alone!(flush_is_a_valid_level, the_tour_clears_flush, "flush");

/// The Grand Flush's fork, the waltz's way: three even hops over the table's baskets, caught on
/// the dryer's trampoline till ONE onto the roof, its pit trampolines, the nerve fork's low line
/// hopped in threes (`flush.timeline` calls Giant Steps and takes the balcony).
#[test]
fn the_waltz_tour_clears_flush_over_the_roof() {
    let run = play(include_str!("../levels/solo/flush.txt"), include_str!("../scripts/tours/flush-waltz.timeline"));
    assert!(run.deaths.is_empty(), "deaths (s, col, row): {:?}", run.deaths);
    let goal = run.goal.expect("the tour reaches the goal");
    assert!(run.end - goal < 6.0, "the tour ends soon after the goal (goal at {goal}s)");
}

/// The Grand Flush's fork fired up: the lint lane's four quick nuggets, the express, a toot off
/// its end and off each belt island over the sump.
#[test]
fn the_fired_up_tour_clears_flush_along_the_express() {
    let run = play(include_str!("../levels/solo/flush.txt"), include_str!("../scripts/tours/flush-fired.timeline"));
    assert!(run.deaths.is_empty(), "deaths (s, col, row): {:?}", run.deaths);
    let goal = run.goal.expect("the tour reaches the goal");
    assert!(run.end - goal < 6.0, "the tour ends soon after the goal (goal at {goal}s)");
}

/// The Grand Flush's fork with no call: off the express into the sump, back to the tenth secret
/// (Han behind a grate, found once), the sump's ducks and the hatch.
#[test]
fn the_no_call_tour_clears_flush_through_the_sump_and_finds_the_secret() {
    let run = play(include_str!("../levels/solo/flush.txt"), include_str!("../scripts/tours/flush-sump.timeline"));
    assert!(run.deaths.is_empty(), "deaths (s, col, row): {:?}", run.deaths);
    assert_eq!(run.secrets, 1, "Nat finds the secret once");
    let goal = run.goal.expect("the tour reaches the goal");
    assert!(run.end - goal < 6.0, "the tour ends soon after the goal (goal at {goal}s)");
}

#[test]
fn check_a_level_file() {
    let Ok(path) = std::env::var("SHOWCASE_CHECK") else { return };
    let level = Level::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    let report = validate::check(&level, &validate::Options { dump: std::env::var("SHOWCASE_DUMP").is_ok() });
    if let Some(d) = report.dump {
        eprintln!("{d}");
    }
    eprintln!("gates: {:?}\nsegments: {:?}\nlessons: {:?}", report.gates, report.segments, report.lessons);
    eprintln!("{:#?}", report.errs);
}

/// How busy a run is: its quiet stretches (more than 1 s between actions: a key down or up, a
/// nugget, a gadget's kick) up to the goal. `quiet` is the share of the run spent past the first
/// second of each quiet stretch (0: something every second); `worst` the longest stretch.
struct Density {
    quiet: f32,
    worst: (f32, f32),
    over2: usize,
}

fn density(run: &Run) -> Density {
    let goal = run.goal.unwrap_or(run.end);
    let mut t: Vec<f32> = run.actions.iter().copied().filter(|t| *t <= goal).collect();
    t.insert(0, 0.0);
    t.push(goal);
    let gaps: Vec<(f32, f32)> = t.windows(2).map(|w| (w[1] - w[0], w[0])).collect();
    let quiet = gaps.iter().map(|(g, _)| (g - 1.0).max(0.0)).sum::<f32>() / goal.max(1.0);
    let worst = gaps.iter().copied().fold((0.0, 0.0), |a, b| if b.0 > a.0 { b } else { a });
    Density { quiet, worst, over2: gaps.iter().filter(|(g, _)| *g > 2.0).count() }
}

/// The NAT ALONE tours, for the density report.
const TOURS: [(&str, &str, &str); 31] = [
    ("breezy", include_str!("../levels/solo/breezy.txt"), include_str!("../scripts/tours/breezy.timeline")),
    ("breezy-alt", include_str!("../levels/solo/breezy.txt"), include_str!("../scripts/tours/breezy-alt.timeline")),
    ("breezy-tunnel", include_str!("../levels/solo/breezy.txt"), include_str!("../scripts/tours/breezy-tunnel.timeline")),
    ("bouncy", include_str!("../levels/solo/bouncy.txt"), include_str!("../scripts/tours/bouncy.timeline")),
    ("bouncy-alt", include_str!("../levels/solo/bouncy.txt"), include_str!("../scripts/tours/bouncy-alt.timeline")),
    ("bouncy-dungeon", include_str!("../levels/solo/bouncy.txt"), include_str!("../scripts/tours/bouncy-dungeon.timeline")),
    ("spout", include_str!("../levels/solo/spout.txt"), include_str!("../scripts/tours/spout.timeline")),
    ("spout-alt", include_str!("../levels/solo/spout.txt"), include_str!("../scripts/tours/spout-alt.timeline")),
    ("spout-stair", include_str!("../levels/solo/spout.txt"), include_str!("../scripts/tours/spout-stair.timeline")),
    ("brush", include_str!("../levels/solo/brush.txt"), include_str!("../scripts/tours/brush.timeline")),
    ("brush-alt", include_str!("../levels/solo/brush.txt"), include_str!("../scripts/tours/brush-alt.timeline")),
    ("brush-underpass", include_str!("../levels/solo/brush.txt"), include_str!("../scripts/tours/brush-underpass.timeline")),
    ("tilt", include_str!("../levels/solo/tilt.txt"), include_str!("../scripts/tours/tilt.timeline")),
    ("tilt-alt", include_str!("../levels/solo/tilt.txt"), include_str!("../scripts/tours/tilt-alt.timeline")),
    ("tilt-outlane", include_str!("../levels/solo/tilt.txt"), include_str!("../scripts/tours/tilt-outlane.timeline")),
    ("wind", include_str!("../levels/solo/wind.txt"), include_str!("../scripts/tours/wind.timeline")),
    ("wind-alt", include_str!("../levels/solo/wind.txt"), include_str!("../scripts/tours/wind-alt.timeline")),
    ("wind-sewer", include_str!("../levels/solo/wind.txt"), include_str!("../scripts/tours/wind-sewer.timeline")),
    ("disco", include_str!("../levels/solo/disco.txt"), include_str!("../scripts/tours/disco.timeline")),
    ("disco-alt", include_str!("../levels/solo/disco.txt"), include_str!("../scripts/tours/disco-alt.timeline")),
    ("disco-cellar", include_str!("../levels/solo/disco.txt"), include_str!("../scripts/tours/disco-cellar.timeline")),
    ("spin", include_str!("../levels/solo/spin.txt"), include_str!("../scripts/tours/spin.timeline")),
    ("spin-alt", include_str!("../levels/solo/spin.txt"), include_str!("../scripts/tours/spin-alt.timeline")),
    ("spin-sump", include_str!("../levels/solo/spin.txt"), include_str!("../scripts/tours/spin-sump.timeline")),
    ("pingpong", include_str!("../levels/solo/pingpong.txt"), include_str!("../scripts/tours/pingpong.timeline")),
    ("pingpong-fired", include_str!("../levels/solo/pingpong.txt"), include_str!("../scripts/tours/pingpong-fired.timeline")),
    ("pingpong-cellar", include_str!("../levels/solo/pingpong.txt"), include_str!("../scripts/tours/pingpong-cellar.timeline")),
    ("flush", include_str!("../levels/solo/flush.txt"), include_str!("../scripts/tours/flush.timeline")),
    ("flush-waltz", include_str!("../levels/solo/flush.txt"), include_str!("../scripts/tours/flush-waltz.timeline")),
    ("flush-fired", include_str!("../levels/solo/flush.txt"), include_str!("../scripts/tours/flush-fired.timeline")),
    ("flush-sump", include_str!("../levels/solo/flush.txt"), include_str!("../scripts/tours/flush-sump.timeline")),
];

/// The density report (`scripts/density`): every NAT ALONE tour's quiet share, its longest quiet
/// stretch and how many run over 2 s (docs/level-design.md, section 4). `DENSITY_ONLY=<name>`
/// plays only the tours whose names start with it; `DENSITY_GAPS=1` lists each stretch over 1 s.
#[test]
#[ignore]
fn density_report() {
    let only = std::env::var("DENSITY_ONLY").unwrap_or_default();
    println!("| tour | goal (s) | quiet | longest (s, from) | over 2 s |\n|---|---|---|---|---|");
    for (name, level, tour) in TOURS.iter().filter(|(n, _, _)| n.starts_with(&only)) {
        let run = play(level, tour);
        let d = density(&run);
        let goal = run.goal.unwrap_or(run.end);
        println!("| {name} | {goal:.1} | {:.0}% | {:.1} (t={:.1}) | {} |", d.quiet * 100.0, d.worst.0, d.worst.1, d.over2);
        if std::env::var("DENSITY_GAPS").is_ok() {
            let mut t: Vec<f32> = run.actions.iter().copied().filter(|t| *t <= goal).collect();
            t.insert(0, 0.0);
            t.push(goal);
            let gaps: Vec<String> = t.windows(2).filter(|w| w[1] - w[0] > 1.0).map(|w| format!("{:.1}-{:.1}", w[0], w[1])).collect();
            println!("|  gaps | {} | | | |", gaps.join(" "));
        }
    }
}
