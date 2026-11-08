//! The deterministic video capture (`src/capture`, the `capture` feature): the input timeline
//! drives play exactly as real key presses do, and the lockstep audio is exactly frames/fps
//! long and repeats sample for sample. The full capture (window, screenshots) runs under Xvfb
//! in an ignored test.
#![cfg(feature = "capture")]

use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::{input::InputPlugin, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use leafwing_input_manager::prelude::*;
use nat_han_adventures::{
    audio::{
        self,
        capture::{AudioCapture, CAPTURE_RATE, read_wav},
    },
    capture::{
        frame_duration,
        timeline::{Script, Timeline, TimelinePlayer, play_timeline},
    },
    events::{Jumped, PlayerDied},
    game::Player,
    state::AppState,
};

const FLOURISH: &str = include_str!("../scripts/tours/flourish.timeline");

/// What happened, by frame.
#[derive(Resource, Default, Debug, Clone, PartialEq)]
struct Log {
    frame: u64,
    jumps: Vec<(u64, bool)>,
    deaths: Vec<u64>,
}

fn log(mut l: ResMut<Log>, mut j: MessageReader<Jumped>, mut d: MessageReader<PlayerDied>) {
    let f = l.frame;
    let jumps: Vec<_> = j.read().map(|j| (f, j.double)).collect();
    l.jumps.extend(jumps);
    let deaths = d.read().count();
    l.deaths.extend(std::iter::repeat_n(f, deaths));
    l.frame += 1;
}

/// The game's simulation (+ the audio plugin if `audio`), level 1, stepped a frame of `fps`
/// per update, the warm-up update done.
fn app(fps: u32, audio: Option<&Path>) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin, InputPlugin, nat_han_adventures::gameplay))
        .insert_resource(TimeUpdateStrategy::ManualDuration(frame_duration(fps).unwrap()))
        .init_resource::<Log>()
        .add_systems(Last, log);
    if let Some(wav) = audio {
        app.add_plugins(audio::plugin).insert_resource(AudioCapture::new(wav, fps).unwrap());
    }
    app.finish();
    app.cleanup();
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::Playing);
    app.update(); // warm-up: Startup, the level spawns, time zero
    app.world_mut().resource_mut::<Log>().frame = 0;
    if audio.is_some() {
        app.world_mut().resource_mut::<AudioCapture>().recording = true;
    }
    app
}

/// Play `timeline` through the keyboard-message path (as the capture does).
fn play(fps: u32, timeline: &Timeline, audio: Option<&Path>) -> App {
    let mut app = app(fps, audio);
    app.insert_resource(TimelinePlayer::new(timeline.clone()))
        .add_systems(PreUpdate, play_timeline.before(bevy::input::InputSystems));
    for _ in 0..timeline.frames {
        app.update();
    }
    app
}

/// Press the same keys on the same frames the way the gameplay tests (and BRP) do: leafwing's
/// simulated key presses, sent between updates.
fn press_directly(fps: u32, timeline: &Timeline) -> App {
    let mut app = app(fps, None);
    for frame in 0..timeline.frames {
        for e in timeline.events.iter().filter(|e| e.frame == frame) {
            if e.pressed {
                e.key.press(app.world_mut());
            } else {
                e.key.release(app.world_mut());
            }
        }
        app.update();
    }
    app
}

fn flourish(fps: u32) -> Timeline {
    Script::parse(FLOURISH).unwrap().compile(fps)
}

/// The flourish tour, at 30 and 60 fps: the timeline's jumps, toots and deaths are the ones the
/// same key presses make, frame for frame, and the tour does what it says (5+ toots, hops,
/// splats).
#[test]
fn the_timeline_drives_the_same_jumps_as_real_keys() {
    for fps in [30, 60] {
        let t = flourish(fps);
        assert!((t.duration(fps).as_secs_f64() - 59.3).abs() < 0.05, "{:?}", t.duration(fps));
        let via_timeline = play(fps, &t, None).world().resource::<Log>().clone();
        let direct = press_directly(fps, &t).world().resource::<Log>().clone();
        assert_eq!(via_timeline, direct, "fps {fps}");
        let toots = via_timeline.jumps.iter().filter(|j| j.1).count();
        let jumps = via_timeline.jumps.len() - toots;
        assert!(toots >= 5 && jumps >= 30, "fps {fps}: {jumps} jumps, {toots} toots: {via_timeline:?}");
        assert!(via_timeline.deaths.len() >= 3, "fps {fps}: into the pit a few times: {:?}", via_timeline.deaths);
    }
}

/// Two runs of the same timeline in lockstep: identical frames of play.
#[test]
fn the_tour_repeats_exactly() {
    let t = flourish(30);
    let pos = |app: &mut App| {
        let e = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        app.world().get::<Transform>(e).unwrap().translation
    };
    let (mut a, mut b) = (play(30, &t, None), play(30, &t, None));
    assert_eq!(a.world().resource::<Log>(), b.world().resource::<Log>());
    assert_eq!(pos(&mut a), pos(&mut b));
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("nathan-capture-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The lockstep audio: exactly `48000 / fps` samples a frame, music + sfx, the same every take.
#[test]
fn capture_audio_is_exactly_frames_long_and_repeats() {
    let script = "wait 0.5\nrepeat 3\n  hold Space 140ms\n  wait 120ms\n  hold Space 140ms\n  wait 0.4\nend\nwait 0.5";
    let dir = scratch("audio");
    let mut takes = Vec::new();
    for (take, fps) in [(0, 30), (1, 30), (2, 60)] {
        let t = Script::parse(script).unwrap().compile(fps);
        let wav = dir.join(format!("take{take}.wav"));
        let mut app = play(fps, &t, Some(&wav));
        let log = app.world().resource::<Log>().clone();
        assert_eq!(log.jumps.iter().filter(|j| j.1).count(), 3, "three toots");
        let mut cap = app.world_mut().resource_mut::<AudioCapture>();
        assert_eq!(cap.blocks, t.frames);
        cap.finish().unwrap();
        let samples = read_wav(&wav).unwrap();
        assert_eq!(samples.len() as u64, t.frames * (CAPTURE_RATE / fps) as u64, "fps {fps}");
        assert_eq!(Duration::from_secs_f64(samples.len() as f64 / CAPTURE_RATE as f64), t.duration(fps));
        let peak = samples.iter().flatten().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak > 0.05 && peak <= 1.0, "audible, unclipped: {peak}");
        takes.push(samples);
    }
    assert!(takes[0] == takes[1], "two takes of the same timeline sound the same");
    let _ = std::fs::remove_dir_all(dir);
}

/// The real thing: the game binary in capture mode under Xvfb (software Vulkan), one second.
/// `cargo nextest run --run-ignored all capture_smoke`.
#[test]
#[ignore = "needs xvfb-run, lavapipe and a built game binary"]
fn capture_smoke_under_xvfb() {
    let dir = scratch("smoke");
    let tl = dir.join("t.timeline");
    std::fs::write(&tl, "wait 0.3\nhold Space 140ms\nwait 0.56").unwrap();
    let status = std::process::Command::new("xvfb-run")
        .args(["-a", "-s", "-screen 0 1280x720x24", env!("CARGO_BIN_EXE_nat-han-adventures")])
        .env("NATHAN_CAPTURE", &dir)
        .env("NATHAN_CAPTURE_TIMELINE", &tl)
        .env("NATHAN_LEVEL", "1")
        .env("NATHAN_SAVE", dir.join("progress.txt"))
        .env("VK_ICD_FILENAMES", "/usr/share/vulkan/icd.d/lvp_icd.x86_64.json")
        .env("ALSA_CONFIG_PATH", concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/alsa-null.conf"))
        .status()
        .expect("xvfb-run");
    assert!(status.success());
    let summary = std::fs::read_to_string(dir.join("capture.txt")).unwrap();
    assert!(summary.contains("frames 30\n") && summary.contains("ok true"), "{summary}");
    let pngs = std::fs::read_dir(&dir).unwrap().filter(|e| e.as_ref().unwrap().path().extension().is_some_and(|x| x == "png")).count();
    assert_eq!(pngs, 30);
    assert_eq!(read_wav(&dir.join("audio.wav")).unwrap().len(), 30 * 1600);
    let _ = std::fs::remove_dir_all(dir);
}
