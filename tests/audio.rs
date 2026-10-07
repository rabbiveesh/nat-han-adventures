//! Renders the whole soundtrack and every sound effect and checks them numerically
//! (we can't listen in CI): audible, finite, never clipping, seamless loops, fast enough.

use std::time::Instant;

use durhay::audio::{Music, Sfx, Song, sfx, songs, synth};

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
    for v in 0..sfx::GUS_VARIANTS {
        check(&format!("gus blip {v}"), &sfx::gus_blip(v));
    }
    let secs = t.elapsed().as_secs_f64();
    assert!(secs < budget_secs(), "rendering everything took {secs:.2}s");
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
    };
    let t = Instant::now();
    let r = synth::render_song(&song).unwrap();
    let secs = t.elapsed().as_secs_f64();
    assert!((r.duration_secs() - 90.0).abs() < 0.01, "{}", r.duration_secs());
    check("stress", &r);
    println!("90s busy song: {:.0}ms", secs * 1000.0);
    assert!(secs < budget_secs() / 4.0, "90s busy song took {secs:.2}s");
}
