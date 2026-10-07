//! Renders the whole soundtrack and every sound effect and checks them numerically
//! (we can't listen in CI): audible, finite, never clipping, seamless loops, fast enough.

use std::time::Instant;

use nat_han_adventures::audio::{Filters, Harmony, Music, Sfx, Song, demo, sfx, songs, synth};

/// Render budget for everything. Generous in debug builds (this crate is built at opt-level 1).
fn budget_secs() -> f64 {
    if cfg!(debug_assertions) { 10.0 } else { 1.0 }
}

fn check(name: &str, r: &synth::Rendered) {
    assert!(!r.frames.is_empty(), "{name}: empty");
    let mut peak = 0.0f32;
    for (i, f) in r.frames.iter().enumerate() {
        assert!(f.left.is_finite() && f.right.is_finite(), "{name}: NaN/inf at frame {i}");
        peak = peak.max(f.left.abs()).max(f.right.abs());
    }
    assert!(peak <= 1.0, "{name}: peak {peak} > 1");
    assert!(peak > 0.05, "{name}: nearly silent (peak {peak})");

    let (first, last) = (r.frames[0], *r.frames.last().unwrap());
    if r.looping {
        // Across the loop seam the signal must be as continuous as anywhere inside the song.
        let max_step = r
            .frames
            .windows(2)
            .map(|w| (w[1].left - w[0].left).abs().max((w[1].right - w[0].right).abs()))
            .fold(0.0f32, f32::max);
        let seam = (first.left - last.left).abs().max((first.right - last.right).abs());
        assert!(seam <= max_step.max(0.05), "{name}: loop seam jump {seam} (max step inside {max_step})");
    } else {
        // One-shots start and end (near) silent: no pops.
        assert!(last.left.abs() < 1e-3 && last.right.abs() < 1e-3, "{name}: ends at {last:?}");
        assert!(first.left.abs() < 0.1 && first.right.abs() < 0.1, "{name}: starts at {first:?}");
    }
}

#[test]
fn every_song_and_sfx_renders_cleanly_and_quickly() {
    let t = Instant::now();
    for m in Music::ALL {
        let song = songs::song(m);
        let r = synth::render_song(&song).unwrap_or_else(|e| panic!("{m:?}: {e}"));
        assert_eq!(r.looping, song.looping);
        check(&format!("{m:?}"), &r);
    }
    for s in Sfx::ALL {
        check(&format!("{s:?}"), &sfx::render(s));
    }
    for v in 0..sfx::HAN_VARIANTS {
        check(&format!("han blip {v}"), &sfx::han_blip(v));
    }
    let secs = t.elapsed().as_secs_f64();
    assert!(secs < budget_secs(), "rendering everything took {secs:.2}s");
}

/// Every song with a chord chart, through every harmony with and without just intonation:
/// clean, loop-length, and about as fast to render as the original.
#[test]
fn every_song_renders_through_every_filter() {
    let mut all: Vec<(String, Song)> = Music::ALL.iter().map(|m| (format!("{m:?}"), songs::song(*m))).collect();
    all.push(("demo".into(), demo::demo_song()));
    let mut plain_secs = 0.0;
    let mut filtered_secs = 0.0;
    let mut filtered = 0;
    for (name, song) in &all {
        let t = Instant::now();
        let plain = synth::render_song(song).unwrap();
        plain_secs += t.elapsed().as_secs_f64();
        for harmony in Harmony::ALL {
            if harmony != Harmony::Original && song.chords.trim().is_empty() {
                continue;
            }
            for just_intonation in [false, true] {
                let f = Filters { harmony, just_intonation };
                let t = Instant::now();
                let r = synth::render_song_with(song, f, 42).unwrap_or_else(|e| panic!("{name} {f:?}: {e}"));
                filtered_secs += t.elapsed().as_secs_f64();
                filtered += 1;
                assert_eq!(r.frames.len(), plain.frames.len(), "{name} {f:?}: length changed");
                check(&format!("{name} {f:?}"), &r);
            }
        }
    }
    let per_plain = plain_secs / all.len() as f64;
    let per_filtered = filtered_secs / filtered as f64;
    println!("plain {:.1}ms/song, filtered {:.1}ms/song", per_plain * 1000.0, per_filtered * 1000.0);
    assert!(per_filtered < 2.0 * per_plain + 0.005, "filtered renders too slow: {per_filtered:.3}s vs {per_plain:.3}s");
}

#[test]
fn every_chart_parses_and_covers_its_song() {
    use nat_han_adventures::audio::{chart, mml};
    for m in Music::ALL {
        let song = songs::song(m);
        if song.chords.trim().is_empty() {
            continue;
        }
        let c = chart::parse(song.chords).unwrap_or_else(|e| panic!("{m:?}: {e}"));
        let beats = mml::parse(song.pulse1, mml::Channel::Melodic).unwrap().length;
        assert_eq!(c.beats(), beats, "{m:?}: chart is {} bars", c.bars);
    }
}

#[test]
fn filter_labels_and_override_syntax() {
    let f = |harmony, just_intonation| Filters { harmony, just_intonation };
    assert_eq!(Filters::default().label(), "");
    assert_eq!(f(Harmony::Coltrane, false).label(), "COLTRANE CHANGES");
    assert_eq!(f(Harmony::Quartal, false).label(), "QUARTAL");
    assert_eq!(f(Harmony::MelodicMinor, false).label(), "MELODIC MINOR");
    assert_eq!(f(Harmony::Original, true).label(), "JUST INTONATION");
    assert_eq!(f(Harmony::Coltrane, true).label(), "COLTRANE CHANGES + JUST INTONATION");
    assert_eq!(Filters::parse("coltrane"), Some(f(Harmony::Coltrane, false)));
    assert_eq!(Filters::parse("Quartal+JI"), Some(f(Harmony::Quartal, true)));
    assert_eq!(Filters::parse("melodic"), Some(f(Harmony::MelodicMinor, false)));
    assert_eq!(Filters::parse("original+ji"), Some(f(Harmony::Original, true)));
    assert_eq!(Filters::parse("ji"), Some(f(Harmony::Original, true)));
    assert_eq!(Filters::parse("bebop"), None);
}

/// A busy, 90-second loop on every channel: an upper bound on what a real song costs.
#[test]
fn a_long_busy_song_renders_fast_and_loops_cleanly() {
    let song = Song {
        title: "stress",
        bpm: 160.0,
        swing: 0.3,
        looping: true,
        // 60 bars of 4/4 at 160bpm = 90s.
        pulse1: "[ o5 l16 v13 @1 c e g >c< b g e d c+ e a >c+< b a e c+ ]60",
        pulse2: "[ o4 l8 v9 @2 e g e g f a f a ]60",
        triangle: "[ o2 l8 c c g g a a e& e ]60",
        noise: "[ k8 h8 s8 h16 h16 k8 k8 s8 H8 ]60",
        key: 0,
        chords: "",
    };
    let t = Instant::now();
    let r = synth::render_song(&song).unwrap();
    let secs = t.elapsed().as_secs_f64();
    assert!((r.duration_secs() - 90.0).abs() < 0.01, "{}", r.duration_secs());
    check("stress", &r);
    println!("90s busy song: {:.0}ms", secs * 1000.0);
    assert!(secs < budget_secs() / 4.0, "90s busy song took {secs:.2}s");
}

mod plugin {
    use std::time::Duration;

    use bevy::{prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
    use nat_han_adventures::{
        audio::{Filters, Harmony, Music, MusicChanged, MusicPlayer, MusicStarted, NowPlaying, director, songs},
        events::{CheckpointReached, Jumped},
        game::LevelRun,
        level::Levels,
        state::{AppState, CurrentLevel},
    };

    /// Everything the music plugin announced.
    #[derive(Resource, Default)]
    struct Heard {
        started: Vec<MusicStarted>,
        changed: Vec<(f64, MusicChanged)>,
    }

    fn listen(
        time: Res<Time<Real>>,
        mut heard: ResMut<Heard>,
        mut s: MessageReader<MusicStarted>,
        mut c: MessageReader<MusicChanged>,
    ) {
        heard.started.extend(s.read().cloned());
        let now = time.elapsed_secs_f64();
        heard.changed.extend(c.read().map(|m| (now, m.clone())));
    }

    /// The playback plugin headless (no audio device: kira's manager fails to start, which
    /// bevy_kira_audio tolerates), on a fixed 60 fps clock.
    fn app() -> App {
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
        app
    }

    fn go(app: &mut App, s: AppState) {
        app.world_mut().resource_mut::<NextState<AppState>>().set(s);
        app.update();
        app.update();
    }

    fn playing(app: &App) -> Option<Music> {
        app.world().resource::<MusicPlayer>().now_playing()
    }

    #[test]
    fn music_follows_state() {
        let mut app = app();
        assert_eq!(playing(&app), Some(Music::Title));
        let world_of = |app: &App, i: usize| app.world().resource::<Levels>().0[i].world;
        go(&mut app, AppState::LevelSelect);
        assert_eq!(playing(&app), Some(Music::Title));
        app.world_mut().resource_mut::<CurrentLevel>().0 = 0;
        go(&mut app, AppState::Playing);
        assert_eq!(playing(&app), Some(Music::World(world_of(&app, 0))));
        go(&mut app, AppState::LevelComplete);
        assert_eq!(playing(&app), Some(Music::LevelClear));
        go(&mut app, AppState::Victory);
        assert_eq!(playing(&app), Some(Music::Victory));
        go(&mut app, AppState::Title);
        assert_eq!(playing(&app), Some(Music::Title));

        // Each (re)start was announced once, plain, and NowPlaying follows.
        let w = Music::World(world_of(&app, 0));
        let heard: Vec<Music> = app.world().resource::<Heard>().started.iter().map(|m| m.0.music).collect();
        assert_eq!(heard, [Music::Title, w, Music::LevelClear, Music::Victory, Music::Title]);
        assert!(app.world().resource::<Heard>().started.iter().all(|m| m.0.filters == Filters::default()));
        let now = app.world().resource::<NowPlaying>();
        assert_eq!((now.music, now.title), (Music::Title, songs::song(Music::Title).title));
        assert_eq!(now.toast(), "");
    }

    /// 5 toots then a checkpoint: Coltrane changes, switched in on a bar line without
    /// restarting the song. 20s of play later the periodic check finds no toots in the window
    /// and goes back to the original.
    #[test]
    fn toots_then_checkpoint_switch_to_coltrane_on_a_bar_line() {
        let mut app = app();
        app.world_mut().resource_mut::<CurrentLevel>().0 = 0;
        go(&mut app, AppState::Playing);
        let music = playing(&app).unwrap();
        let song = songs::song(music);
        assert!(!song.chords.is_empty(), "the test needs a song with a chart");
        let bar = 4.0 * 60.0 / song.bpm as f64;
        let started = app.world().resource::<Heard>().started.len();

        for _ in 0..director::GIANT_STEPS_TOOTS {
            app.world_mut().write_message(Jumped { pos: Vec2::ZERO, double: true });
        }
        app.world_mut().write_message(CheckpointReached { index: 0, pos: Vec2::ZERO });
        app.update();
        let run_time = |app: &App| app.world().resource::<LevelRun>().time;
        let t0 = run_time(&app);
        let decided_at = app.world().resource::<Time<Real>>().elapsed_secs_f64();
        let player = app.world().resource::<MusicPlayer>();
        assert_eq!(player.pending_filters().map(|f| f.harmony), Some(Harmony::Coltrane));
        assert_eq!(player.filters(), Some(Filters::default()), "still the old version while rendering");

        // Renders over several frames, then switches at the next bar line.
        let mut frames = 0;
        while app.world().resource::<Heard>().changed.is_empty() {
            app.update();
            frames += 1;
            assert!(frames < 60 * 10, "no switch after 10s");
        }
        let (switched_at, change) = app.world().resource::<Heard>().changed[0].clone();
        assert!(frames > 1, "rendered in one frame?");
        assert_eq!(change.now.filters.harmony, Harmony::Coltrane);
        assert_eq!(change.now.reason, director::REASON_GIANT_STEPS);
        assert_eq!(change.now.toast(), "GIANT STEPS! - COLTRANE CHANGES");
        let bars = change.at_secs / bar;
        assert!((bars - bars.round()).abs() < 1e-6, "switched mid-bar: {} s = {bars} bars", change.at_secs);
        // The bar line is the one after the decision (song clock started with the level).
        assert!(switched_at >= decided_at && switched_at - decided_at < 10.0);
        assert_eq!(app.world().resource::<NowPlaying>().filters.harmony, Harmony::Coltrane);
        assert_eq!(app.world().resource::<MusicPlayer>().filters().unwrap().harmony, Harmony::Coltrane);
        assert_eq!(app.world().resource::<Heard>().started.len(), started, "the song didn't restart");

        // The periodic check, MUSIC_CHECK_SECS of play after the checkpoint.
        let mut frames = 0;
        while app.world().resource::<Heard>().changed.len() < 2 {
            app.update();
            frames += 1;
            assert!(frames < 60 * 40, "no periodic switch after 40s");
        }
        let (_, back) = app.world().resource::<Heard>().changed[1].clone();
        assert_eq!(back.now.filters, Filters::default());
        let after = run_time(&app) - t0;
        assert!(after >= director::MUSIC_CHECK_SECS, "switched back too early ({after}s)");
        let bars = back.at_secs / bar;
        assert!((bars - bars.round()).abs() < 1e-6);
    }
}
