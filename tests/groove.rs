//! The music bends the physics: the band director summons a mode the moment the player earns
//! it, the new version switches in at the next bar line, and `Groove` follows exactly then.
//! The playback plugin runs headless (no audio device) on a fixed 60 fps clock.

use std::time::Duration;

use bevy::{prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use nat_han_adventures::{
    audio::{Filters, Harmony, Music, MusicChanged, MusicPlayer, director, songs, waltz},
    events::{Jumped, NuggetCollected, PlayerDied},
    game::{Groove, LevelRun},
    state::{AppState, CurrentLevel},
};

/// Every switch heard, with the real time and the groove in force right after it.
#[derive(Resource, Default)]
struct Heard(Vec<(f64, MusicChanged, Groove)>);

fn listen(time: Res<Time<Real>>, groove: Res<Groove>, mut heard: ResMut<Heard>, mut c: MessageReader<MusicChanged>) {
    let now = time.elapsed_secs_f64();
    heard.0.extend(c.read().map(|m| (now, m.clone(), *groove)));
}

fn app(level: usize) -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        StatesPlugin,
        bevy::input::InputPlugin,
        AssetPlugin::default(),
        nat_han_adventures::gameplay,
        nat_han_adventures::audio::plugin,
    ))
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 60.0)))
    .init_resource::<Heard>()
    .add_systems(Last, listen);
    app.update();
    app.world_mut().resource_mut::<CurrentLevel>().0 = level;
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::Playing);
    app.update();
    app.update();
    // Let the level-start decision settle (it's plain, like what's playing: no switch).
    for _ in 0..30 {
        app.update();
    }
    app
}

fn bar_secs(app: &App) -> f64 {
    let music = app.world().resource::<MusicPlayer>().now_playing().unwrap();
    assert!(matches!(music, Music::World(_)));
    let song = songs::song(music);
    assert!(!song.chords.trim().is_empty(), "every level song needs a chart, or its gates can't open");
    4.0 * 60.0 / song.bpm as f64
}

fn now(app: &App) -> f64 {
    app.world().resource::<Time<Real>>().elapsed_secs_f64()
}

/// Run until the first switch; returns (seconds after `from`, the switch, the groove after it).
fn until_switch(app: &mut App, from: f64) -> (f64, MusicChanged, Groove) {
    let mut frames = 0;
    while app.world().resource::<Heard>().0.is_empty() {
        assert_eq!(*app.world().resource::<Groove>(), Groove::default(), "groove changed before the music");
        app.update();
        frames += 1;
        assert!(frames < 60 * 10, "no switch after 10s");
    }
    let (t, change, groove) = app.world().resource::<Heard>().0[0].clone();
    (t - from, change, groove)
}

fn toot(app: &mut App) {
    app.world_mut().write_message(Jumped { pos: Vec2::ZERO, double: true });
    app.update();
}

/// The 5th toot summons Giant Steps right away (no waiting for the 20s check), it comes in at
/// the next bar line or so, and the physics switch with it.
#[test]
fn five_toots_summon_giant_steps_at_the_next_bar() {
    let mut app = app(0);
    let bar = bar_secs(&app);
    for _ in 0..director::GIANT_STEPS_TOOTS - 1 {
        toot(&mut app);
        app.update();
    }
    assert_eq!(app.world().resource::<MusicPlayer>().pending_filters(), None, "4 toots: nothing yet");
    toot(&mut app);
    let player = app.world().resource::<MusicPlayer>();
    assert_eq!(player.pending_filters().map(|f| f.harmony), Some(Harmony::Coltrane), "decided at once");
    let decided = now(&app);
    let (after, change, groove) = until_switch(&mut app, decided);
    println!("Giant Steps came in {after:.2}s after the 5th toot (a bar is {bar:.2}s)");
    assert!(after <= 2.0 * bar + 0.5, "took {after:.2}s");
    assert_eq!(change.now.filters.harmony, Harmony::Coltrane);
    assert_eq!(change.now.reason, director::REASON_GIANT_STEPS);
    assert_eq!(groove, Groove::of(Harmony::Coltrane), "physics follow at the switch");
    assert!(groove.giant_steps() && groove.gravity_scale < 1.0);

    // Toots while it plays keep it going through the periodic check...
    let t0 = app.world().resource::<LevelRun>().time;
    while app.world().resource::<LevelRun>().time - t0 < director::MUSIC_CHECK_SECS + 2.0 {
        for _ in 0..30 {
            app.update();
        }
        toot(&mut app);
    }
    assert_eq!(app.world().resource::<Heard>().0.len(), 1, "still Giant Steps");
    assert!(app.world().resource::<Groove>().giant_steps());
    // ...and without toots it ends at a later check, physics back to normal.
    let mut frames = 0;
    while app.world().resource::<Heard>().0.len() < 2 {
        app.update();
        frames += 1;
        assert!(frames < 60 * 40, "Giant Steps never ended");
    }
    let (_, back, groove) = app.world().resource::<Heard>().0[1].clone();
    assert_eq!(back.now.filters.harmony, Harmony::Original);
    assert_eq!(groove, Groove::default());
}

/// A nervous band (3+ deaths) still gives way to Giant Steps: a player stuck at a giant wall
/// can always summon it.
#[test]
fn giant_steps_beats_a_nervous_band() {
    let mut app = app(0);
    for _ in 0..director::NERVOUS_DEATHS {
        app.world_mut().write_message(PlayerDied { pos: Vec2::ZERO });
        app.update();
    }
    let player = app.world().resource::<MusicPlayer>();
    assert_eq!(player.pending_filters().map(|f| f.harmony), Some(Harmony::MelodicMinor));
    for _ in 0..director::GIANT_STEPS_TOOTS {
        toot(&mut app);
    }
    let player = app.world().resource::<MusicPlayer>();
    assert_eq!(player.pending_filters().map(|f| f.harmony), Some(Harmony::Coltrane));
    let (_, change, groove) = until_switch(&mut app, 0.0);
    assert_eq!(change.now.filters.harmony, Harmony::Coltrane);
    assert!(groove.giant_steps());
}

/// 4 quick nuggets fire the band up (quartal) right away; the run speeds up at the switch.
#[test]
fn quick_nuggets_fire_up_the_band_at_once() {
    let mut app = app(2);
    let bar = bar_secs(&app);
    for k in 0..director::FIRED_UP_NUGGETS {
        if k == director::FIRED_UP_NUGGETS - 1 {
            assert_eq!(app.world().resource::<MusicPlayer>().pending_filters(), None);
        }
        app.world_mut().write_message(NuggetCollected { pos: Vec2::ZERO });
        app.update();
        for _ in 0..20 {
            app.update();
        }
    }
    let player = app.world().resource::<MusicPlayer>();
    assert_eq!(player.pending_filters().map(|f| f.harmony), Some(Harmony::Quartal));
    let decided = now(&app) - 20.0 / 60.0;
    let (after, change, groove) = until_switch(&mut app, decided);
    assert!(after <= 2.0 * bar + 0.8, "took {after:.2}s");
    assert_eq!(change.now.filters.harmony, Harmony::Quartal);
    assert_eq!(change.now.reason, director::REASON_FIRED_UP);
    assert!(groove.speed_scale > 1.0);
}

/// NATHAN_MUSIC-style override: a track that starts reharmonized starts with its physics.
#[test]
fn groove_follows_every_filter() {
    for harmony in Harmony::ALL {
        for just_intonation in [false, true] {
            let g = Groove::new(Filters { harmony, just_intonation });
            assert_eq!(g.harmony, harmony);
            assert_eq!(g.bounce, just_intonation);
            assert_eq!(g.giant_steps(), harmony == Harmony::Coltrane);
        }
    }
    assert_eq!(Groove::default(), Groove::of(Harmony::Original));
    let plain = Groove::default();
    assert_eq!((plain.gravity_scale, plain.speed_scale, plain.time_scale, plain.bounce), (1.0, 1.0, 1.0, false));
}

fn ground_jump(app: &mut App) {
    app.world_mut().write_message(Jumped { pos: Vec2::ZERO, double: false });
    app.update();
}

/// Three evenly spaced ground jumps summon the waltz at once; it comes in at the next 4/4 bar
/// line (the first of a pair of waltz bars), the song position mapped through the warp, and the
/// physics and the world's beat clock follow. Waltzing on keeps it; it ends at a later check.
#[test]
fn jumping_in_threes_waltzes_at_the_next_bar() {
    let mut app = app(7);
    let bar = bar_secs(&app);
    // 0.6s apart (36 frames).
    for k in 0..3 {
        assert_eq!(app.world().resource::<MusicPlayer>().pending_filters(), None, "{k} jumps: nothing yet");
        ground_jump(&mut app);
        if k < 2 {
            for _ in 0..35 {
                app.update();
            }
        }
    }
    let player = app.world().resource::<MusicPlayer>();
    assert_eq!(player.pending_filters().map(|f| f.harmony), Some(Harmony::Waltz), "decided at the 3rd jump");
    let decided = now(&app);
    let (after, change, groove) = until_switch(&mut app, decided);
    println!("the waltz came in {after:.2}s after the 3rd jump (a 4/4 bar is {bar:.2}s)");
    assert!(after <= 2.0 * bar + 0.5, "took {after:.2}s");
    assert_eq!(change.now.filters.harmony, Harmony::Waltz);
    assert_eq!(change.now.reason, director::REASON_WALTZ);
    assert_eq!(change.now.toast(), "THE BAND WALTZES - JAZZ WALTZ");
    assert_eq!(groove, Groove::of(Harmony::Waltz), "physics follow at the switch");
    assert!(groove.waltz());
    // On a waltz bar line: the first of a pair (a 4/4 bar line, warped).
    let pair = 2.0 * 3.0 * 60.0 / waltz::WALTZ_BPM as f64;
    let pairs = change.at_secs / pair;
    assert!((pairs - pairs.round()).abs() < 1e-6, "switched mid-bar: {}s = {pairs} waltz bar pairs", change.at_secs);
    // The world's clock is the waltz's: 3 beats of 0.5s, on the downbeat of the switch.
    assert_eq!((groove.clock.beats_per_bar, groove.clock.beat_secs), (3, 0.5));
    assert_eq!(groove.clock.bar % 2, 0);
    assert_eq!(groove.clock.beat, 0);
    // The clock keeps following the waltz. (How far it runs here depends on whether kira found
    // an audio device: its position runs on the wall clock, this test's frames don't.)
    for _ in 0..45 {
        app.update();
    }
    assert_eq!(app.world().resource::<Groove>().clock.beats_per_bar, 3);

    // Waltzing on (every 0.6s) keeps it going through the periodic check...
    let t0 = app.world().resource::<LevelRun>().time;
    while app.world().resource::<LevelRun>().time - t0 < director::MUSIC_CHECK_SECS + 2.0 {
        ground_jump(&mut app);
        for _ in 0..35 {
            app.update();
        }
    }
    assert_eq!(app.world().resource::<Heard>().0.len(), 1, "still waltzing");
    assert!(app.world().resource::<Groove>().waltz());
    // ...and without it the band goes back to 4/4 at a later check, on a shared bar line.
    let mut frames = 0;
    while app.world().resource::<Heard>().0.len() < 2 {
        app.update();
        frames += 1;
        assert!(frames < 60 * 40, "the waltz never ended");
    }
    let (_, back, groove) = app.world().resource::<Heard>().0[1].clone();
    assert_eq!(back.now.filters.harmony, Harmony::Original);
    assert_eq!(groove, Groove::default());
    let bars = back.at_secs / bar;
    assert!((bars - bars.round()).abs() < 1e-6, "back on a 4/4 bar line: {}s", back.at_secs);
    assert_eq!(groove.clock.beats_per_bar, 4);
}

/// Uneven jumps don't waltz.
#[test]
fn uneven_jumps_dont_waltz() {
    let mut app = app(7);
    for gap in [30, 50, 20, 70, 15] {
        ground_jump(&mut app);
        for _ in 0..gap {
            app.update();
        }
    }
    assert_eq!(app.world().resource::<MusicPlayer>().pending_filters(), None);
}
