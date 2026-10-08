//! The live engine's kira sound, without a manager. (The Bevy plugin's tests, headless, are in
//! `tests/audio.rs` and `tests/groove.rs`.)

use kira::{Frame, info::MockInfoBuilder, sound::Sound};
use nat_han_adventures::audio::{
    Harmony,
    live::{
        Engine, Input, library,
        playback::{ENGINE_RATE, LiveSoundData},
    },
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
