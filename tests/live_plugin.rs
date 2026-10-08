//! The live engine's kira sound and Bevy plugin, headless.

use std::time::Duration;

use bevy::{prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use kira::{Frame, info::MockInfoBuilder, sound::Sound};
use nat_han_adventures::{
    audio::{
        Harmony, MusicChanged, NowPlaying, director,
        live::{
            Engine, Input, library,
            playback::{ENGINE_RATE, LiveSoundData},
            plugin::{LiveClock, LiveOutput},
        },
    },
    events::Jumped,
    game::Groove,
    state::{AppState, CurrentLevel},
};

fn engine(stem: &str) -> Engine {
    Engine::new(&library::load(stem).unwrap(), ENGINE_RATE).unwrap()
}

/// At the engine's own rate the sound passes the engine through untouched (three frames late,
/// for the interpolator); at 48 kHz it's resampled; songs crossfade; stop fades out.
#[test]
fn the_sound_plays_engines() {
    let (mut sound, mut handle) = LiveSoundData::new().split();
    let info = MockInfoBuilder::new().build();
    handle.play(engine("tiger_rag"), 0.0, 0.0);
    let n = 3 * ENGINE_RATE as usize;
    let mut out = vec![Frame::ZERO; n];
    for chunk in out.chunks_mut(128) {
        sound.process(chunk, 1.0 / ENGINE_RATE as f64, &info);
    }
    let mut direct = engine("tiger_rag");
    let mut want = vec![Frame::ZERO; n];
    direct.fill(&mut want);
    assert!(out[3..].iter().zip(&want).all(|(a, b)| a.left == b.left && a.right == b.right), "not a passthrough");
    let p = handle.published();
    assert!(p.playing);
    assert!((p.clock.position.sample as i64 - n as i64).abs() <= 130, "{:?}", p.clock.position);
    assert!(p.ahead_secs > 0.0 && p.ahead_secs < 0.01);

    // 48 kHz: the same music (same loudness), resampled.
    let mut hi = vec![Frame::ZERO; 48_000];
    for chunk in hi.chunks_mut(128) {
        sound.process(chunk, 1.0 / 48_000.0, &info);
    }
    let rms = |x: &[Frame]| (x.iter().map(|f| (f.left as f64).powi(2)).sum::<f64>() / x.len() as f64).sqrt();
    assert!(hi.iter().all(|f| f.left.is_finite() && f.left.abs() <= 1.05));
    assert!(rms(&hi) > 0.3 * rms(&want) && rms(&hi) < 3.0 * rms(&want));

    // Crossfade to another song, then stop.
    handle.play(engine("when_the_saints"), 0.25, 0.45);
    handle.post(Input::ForceHarmony(Some(Harmony::Quartal)));
    for chunk in hi.chunks_mut(128) {
        sound.process(chunk, 1.0 / 48_000.0, &info);
    }
    let p = handle.published();
    assert!(p.state.upcoming.iter().any(|b| b.harmony == Harmony::Quartal), "input reached the new engine");
    handle.stop(0.1);
    for chunk in hi.chunks_mut(128) {
        sound.process(chunk, 1.0 / 48_000.0, &info);
    }
    assert!(hi[10_000..].iter().all(|f| f.left == 0.0 && f.right == 0.0), "stopped");
    assert!(!handle.published().playing);
    handle.collect_garbage();
    drop(handle);
    assert!(sound.finished());
}

/// Everything the music announced.
#[derive(Resource, Default)]
struct Heard(Vec<(f64, MusicChanged, LiveClock)>);

fn listen(time: Res<Time<Real>>, clock: Res<LiveClock>, mut heard: ResMut<Heard>, mut c: MessageReader<MusicChanged>) {
    let now = time.elapsed_secs_f64();
    heard.0.extend(c.read().map(|m| (now, m.clone(), *clock)));
}

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
    .insert_resource(LiveOutput::Headless)
    .add_plugins(nat_han_adventures::audio::live::plugin)
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 60.0)))
    .init_resource::<Heard>()
    .add_systems(Last, listen);
    app.update();
    app
}

/// In the game: five toots summon Giant Steps, which the band plays from a bar line; the
/// physics follow the moment it sounds; the old plugin's music is silent.
#[test]
fn toots_summon_giant_steps_on_a_bar_line() {
    let mut app = app();
    app.world_mut().resource_mut::<CurrentLevel>().0 = 0;
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::Playing);
    for _ in 0..30 {
        app.update();
    }
    assert!(!app.world().resource::<Groove>().giant_steps());
    let clock = app.world().resource::<LiveClock>().clock;
    assert!(clock.bpm > 100.0 && clock.position.sample > 0, "{clock:?}");
    for _ in 0..director::GIANT_STEPS_TOOTS {
        app.world_mut().write_message(Jumped { pos: Vec2::ZERO, double: true });
    }
    let mut frames = 0;
    while app.world().resource::<Heard>().0.is_empty() {
        app.update();
        frames += 1;
        assert!(frames < 60 * 5, "no switch after 5s");
    }
    let (_, change, at) = app.world().resource::<Heard>().0[0].clone();
    assert_eq!(change.now.filters.harmony, Harmony::Coltrane);
    assert_eq!(change.now.toast(), "GIANT STEPS! - COLTRANE CHANGES");
    // Heard right at a bar line (within a frame and the output latency, either side).
    let beat = at.clock.position.beat;
    let off = beat.min(at.clock.beats_per_bar - beat);
    assert!(off < 0.02 * at.clock.bpm as f64 / 60.0, "switched {beat} beats into a bar");
    // Within two bars of the toots (the next bar line not committed yet).
    let bar_secs = at.clock.beats_per_bar * 60.0 / at.clock.bpm as f64;
    assert!((frames as f64 / 60.0) < 2.0 * bar_secs + 0.1, "{frames} frames");
    assert!(app.world().resource::<Groove>().giant_steps());
    assert_eq!(app.world().resource::<NowPlaying>().filters.harmony, Harmony::Coltrane);
    // The old music is a silent song now.
    let song = (app.world().resource::<nat_han_adventures::audio::SongSource>().0)(nat_han_adventures::audio::Music::Title);
    assert_eq!((song.pulse1, song.noise), ("r1", ""));
}
