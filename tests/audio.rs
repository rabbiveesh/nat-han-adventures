//! Renders the whole soundtrack (the live engine run offline) and every sound effect and
//! checks them numerically (we can't listen in CI): audible, finite, never clipping, seamless
//! loops, fast enough. Then the Bevy plugin, headless.

use std::time::Instant;

use nat_han_adventures::audio::{
    Filters, Harmony, Music, Sfx, demo,
    live::{SongFile, library},
    sfx, synth, waltz,
};

type Song = SongFile;

/// Every song of the game, and the demo.
fn all_songs() -> Vec<(String, Song)> {
    let mut all: Vec<(String, Song)> = Music::ALL.iter().map(|m| (format!("{m:?}"), song(*m))).collect();
    all.push(("demo".into(), demo::demo_song()));
    all
}

fn song(m: Music) -> Song {
    library::song(m).unwrap().1.clone()
}

/// Frames a looping version renders to: the plain length, or the waltz's (1.5x the beats at
/// the waltz tempo).
fn version_frames(song: &Song, f: Filters, plain: usize) -> usize {
    if f.harmony == Harmony::Waltz {
        let secs = waltz::warp(song.beats()) * 60.0 / waltz::WALTZ_BPM as f64;
        (secs * synth::SAMPLE_RATE as f64).round() as usize
    } else {
        plain
    }
}

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
        let song = song(m);
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

/// Every song with a chord chart, through every harmony with and without the laughing band's
/// medley tuning:
/// clean, loop-length, and about as fast to render as the original.
#[test]
fn every_song_renders_through_every_filter() {
    let all = all_songs();
    // Render time per second of audio (the waltz is longer than the rest).
    let (mut plain_secs, mut plain_audio) = (0.0, 0.0);
    let (mut filtered_secs, mut filtered_audio) = (0.0, 0.0);
    for (name, song) in &all {
        let t = Instant::now();
        let plain = synth::render_song(song).unwrap();
        plain_secs += t.elapsed().as_secs_f64();
        plain_audio += plain.duration_secs() as f64;
        for harmony in Harmony::ALL {
            if harmony != Harmony::Original && song.chart.is_none() {
                continue;
            }
            for just_intonation in [false, true] {
                let f = Filters { harmony, just_intonation };
                let t = Instant::now();
                let r = synth::render_song_with(song, f, 42).unwrap_or_else(|e| panic!("{name} {f:?}: {e}"));
                filtered_secs += t.elapsed().as_secs_f64();
                filtered_audio += r.duration_secs() as f64;
                if song.looping || harmony != Harmony::Waltz {
                    assert_eq!(r.frames.len(), version_frames(song, f, plain.frames.len()), "{name} {f:?}: length changed");
                }
                check(&format!("{name} {f:?}"), &r);
            }
        }
    }
    let per_plain = plain_secs / plain_audio;
    let per_filtered = filtered_secs / filtered_audio;
    println!("plain {:.2}ms/s of audio, filtered {:.2}ms/s", per_plain * 1000.0, per_filtered * 1000.0);
    assert!(per_filtered < 2.0 * per_plain + 0.0002, "filtered renders too slow: {per_filtered:.5}s vs {per_plain:.5}s per second");
}

/// The jazz waltz, alone and with the laughing band: every song with a chart, clean (including
/// the loop seam), exactly 1.5x the beats at the waltz tempo, and within the render budget.
#[test]
fn every_song_waltzes_cleanly_and_within_budget() {
    let all = all_songs();
    let t = std::time::Instant::now();
    let mut waltzed = 0;
    for (name, song) in &all {
        if song.chart.is_none() {
            continue;
        }
        for just_intonation in [false, true] {
            let f = Filters { harmony: Harmony::Waltz, just_intonation };
            let r = synth::render_song_with(song, f, 7).unwrap_or_else(|e| panic!("{name} {f:?}: {e}"));
            let want = waltz::warp(song.beats()) * 60.0 / waltz::WALTZ_BPM as f64;
            // (A one-shot rings out a little past its last beat.)
            let tail = if song.looping { 1e-3 } else { 0.5 };
            let d = r.duration_secs() as f64;
            assert!(d > want - 1e-3 && d < want + tail, "{name}: {d} s, want {want}");
            assert_eq!(r.looping, song.looping);
            check(&format!("{name} {f:?}"), &r);
            // Not just the melody: the bass, comping and drums all play.
            let quiet = r.frames.iter().filter(|x| x.left.abs().max(x.right.abs()) < 1e-3).count();
            assert!(quiet * 10 < r.frames.len(), "{name} {f:?}: {quiet} silent frames");
            waltzed += 1;
        }
    }
    assert!(waltzed >= 12, "{waltzed}");
    let secs = t.elapsed().as_secs_f64();
    println!("{waltzed} waltz renders: {:.0}ms", secs * 1000.0);
    assert!(secs < 2.0 * budget_secs(), "waltz renders took {secs:.2}s");
}

/// The waltz's melody is the written one, rhythm warped (ONE-two-three), pitches untouched.
#[test]
fn the_waltz_melody_is_the_tune_in_three() {
    for m in Music::ALL {
        let song = song(m);
        if song.chart.is_none() {
            continue;
        }
        let mel = &song.tracks[0];
        let w = waltz::warp_track(mel);
        assert_eq!(w.length, 1.5 * mel.length, "{m:?}");
        assert_eq!(w.events.len(), mel.events.len(), "{m:?}");
        for (a, b) in w.events.iter().zip(&mel.events) {
            assert_eq!((a.kind, a.volume, a.duty, a.tie), (b.kind, b.volume, b.duty, b.tie), "{m:?}");
            assert!((a.start - waltz::warp(b.start)).abs() < 1e-9);
        }
        // Every 4/4 downbeat note is on a waltz ONE.
        for (a, b) in w.events.iter().zip(&mel.events) {
            if b.start.rem_euclid(2.0) < 1e-9 {
                assert!(a.start.rem_euclid(3.0) < 1e-9, "{m:?}: {} -> {}", b.start, a.start);
            }
        }
    }
}

/// The laughing band's medley tuning (a different tuning every phrase, a bit drunk), alone:
/// every song clean (including the loop seam), loop-length, and within the render budget.
#[test]
fn every_song_renders_cleanly_in_the_medley() {
    let laughing = Filters { just_intonation: true, ..Filters::default() };
    let all = all_songs();
    let t = Instant::now();
    for (name, song) in &all {
        let r = synth::render_song_with(song, laughing, 0).unwrap_or_else(|e| panic!("{name}: {e}"));
        let plain = synth::render_song(song).unwrap();
        assert_eq!(r.frames.len(), plain.frames.len(), "{name}: length changed");
        check(&format!("{name} medley"), &r);
    }
    let secs = t.elapsed().as_secs_f64();
    assert!(secs < 2.0 * budget_secs(), "medley + plain renders took {secs:.2}s");
}

/// Every real song's melody through every reharmonizing filter: same rhythm, in range, close
/// to the written line, same contour; melodic minor in the scale, quartal free of strong avoid
/// notes, Coltrane untouched where the chart is.
#[test]
fn melodies_follow_the_reharmonization() {
    use nat_han_adventures::audio::{
        melody,
        mml::{self, EventKind},
        theory,
    };
    let all = all_songs();
    let pitch = |e: &mml::Event| match e.kind {
        EventKind::Note(n) => Some(n as i32),
        _ => None,
    };
    for (name, song) in &all {
        if song.chart.is_none() {
            continue;
        }
        let c = song.chart.clone().unwrap();
        let slots = c.merged();
        let beats = c.beats();
        let mel = song.tracks[0].clone();
        for h in [Harmony::Coltrane, Harmony::Quartal, Harmony::MelodicMinor] {
            let t = melody::reharmonize(&mel, &c, h);
            assert_eq!(t, melody::reharmonize(&mel, &c, h), "{name} {h:?}: deterministic");
            // Rhythm, dynamics and slurs untouched.
            assert_eq!(t.events.len(), mel.events.len(), "{name} {h:?}");
            assert_eq!(t.length, mel.length);
            let mut notes = Vec::new();
            for (a, b) in t.events.iter().zip(&mel.events) {
                assert_eq!(
                    (a.start, a.dur, a.tie, a.volume, a.duty),
                    (b.start, b.dur, b.tie, b.volume, b.duty),
                    "{name} {h:?}"
                );
                assert_eq!(pitch(a).is_some(), pitch(b).is_some(), "{name} {h:?}");
                if let (Some(x), Some(y)) = (pitch(a), pitch(b)) {
                    assert!((48..=95).contains(&x), "{name} {h:?}: {x} out of o3-o6");
                    assert!((x - y).abs() <= melody::MAX_SHIFT, "{name} {h:?}: moved {y} -> {x}");
                    notes.push((a, x, y));
                }
            }
            // Contour: direction kept for (almost) every step; no interval changes wildly.
            let mut kept = 0;
            let mut worst = 0;
            for w in notes.windows(2) {
                let (d_new, d_old) = (w[1].1 - w[0].1, w[1].2 - w[0].2);
                kept += (d_new.signum() == d_old.signum()) as usize;
                worst = worst.max((d_new - d_old).abs());
            }
            let share = kept as f64 / (notes.len() - 1) as f64;
            println!("{name} {h:?}: contour kept {:.1}%, worst interval change {worst}", share * 100.0);
            assert!(share >= 0.9, "{name} {h:?}: contour kept only {:.0}%", share * 100.0);
            assert!(worst <= 12, "{name} {h:?}: an interval changed by {worst}");
            let seg = |e: &mml::Event| melody::sounding(&slots, beats, e.start, e.dur);
            let new = match h {
                Harmony::Coltrane => theory::coltrane(&c).merged(),
                _ => slots.clone(),
            };
            let new_seg = |e: &mml::Event| melody::sounding(&new, beats, e.start, e.dur);
            for w in notes.windows(2) {
                // Repeated notes over one chord (old and new) stay repeated.
                if w[0].2 == w[1].2 && seg(w[0].0) == seg(w[1].0) && new_seg(w[0].0) == new_seg(w[1].0) {
                    assert_eq!(w[0].1, w[1].1, "{name} {h:?}: repeated note split at {}", w[1].0.start);
                }
            }
            for (k, &(e, x, y)) in notes.iter().enumerate() {
                let i = seg(e);
                match h {
                    Harmony::MelodicMinor => {
                        let mm = theory::melodic_minor(&slots, i);
                        if !mm.in_scale(x as u8) {
                            // A written chromatic approach: a semitone straight into the next note.
                            let next = notes.get(k + 1).expect("an approach has a target");
                            assert!(
                                (next.0.start - (e.start + e.dur)).abs() < 1e-6 && (next.1 - x).abs() == 1,
                                "{name}: {x} at {} not in {mm:?} and not an approach",
                                e.start
                            );
                        }
                    }
                    Harmony::Quartal => {
                        let chord = slots[i].chord;
                        let semi = (x - chord.root as i32).rem_euclid(12) as u8;
                        if melody::strong_beat(e.start) {
                            assert!(
                                melody::quartal_avoid(&chord).iter().all(|(a, _)| *a != semi),
                                "{name}: avoid note {x} on {chord} at beat {}",
                                e.start
                            );
                        } else if e.dur < 1.5 {
                            assert_eq!(x, y, "{name}: short weak-beat notes stay as written");
                        }
                    }
                    _ => {
                        // Coltrane: untouched where the chart is (bar chromatic approaches).
                        let j = new_seg(e);
                        let chromatic = !melody::chord_scale(&slots[i].chord).contains(y as u8);
                        if slots[i].chord == new[j].chord && !chromatic {
                            assert_eq!(x, y, "{name}: changed at {} where the chart wasn't", e.start);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn every_chart_parses_and_covers_its_song() {
    for m in Music::ALL {
        let song = song(m);
        let Some(c) = &song.chart else { continue };
        assert_eq!(c.beats(), song.tracks[0].length, "{m:?}: chart is {} bars", c.bars);
        assert_eq!(c.beats(), song.beats(), "{m:?}");
    }
    // Every level's song has one (or its gates can't open).
    for w in 1..=5 {
        assert!(song(Music::World(w)).chart.is_some(), "world {w}");
    }
}

#[test]
fn filter_labels_and_override_syntax() {
    let f = |harmony, just_intonation| Filters { harmony, just_intonation };
    assert_eq!(Filters::default().label(), "");
    assert_eq!(f(Harmony::Coltrane, false).label(), "COLTRANE CHANGES");
    assert_eq!(f(Harmony::Quartal, false).label(), "QUARTAL");
    assert_eq!(f(Harmony::MelodicMinor, false).label(), "MELODIC MINOR");
    assert_eq!(f(Harmony::Original, true).label(), "TUNING? WHAT TUNING");
    assert_eq!(f(Harmony::Coltrane, true).label(), "COLTRANE CHANGES + TUNING? WHAT TUNING");
    assert_eq!(Filters::parse("coltrane"), Some(f(Harmony::Coltrane, false)));
    assert_eq!(Filters::parse("Quartal+JI"), Some(f(Harmony::Quartal, true)));
    assert_eq!(Filters::parse("melodic"), Some(f(Harmony::MelodicMinor, false)));
    assert_eq!(Filters::parse("original+ji"), Some(f(Harmony::Original, true)));
    assert_eq!(Filters::parse("ji"), Some(f(Harmony::Original, true)));
    assert_eq!(Filters::parse("bebop"), None);
    assert_eq!(f(Harmony::Waltz, false).label(), "JAZZ WALTZ");
    assert_eq!(Filters::parse("waltz+ji"), Some(f(Harmony::Waltz, true)));
}

/// A busy, 90-second loop on every channel: an upper bound on what a real song costs.
#[test]
fn a_long_busy_song_renders_fast_and_loops_cleanly() {
    // 60 bars of 4/4 at 160bpm = 90s.
    let song = Song::from_mml("stress", 160.0, 0.3, true, 0, "", [
        "[ o5 l16 v13 @1 c e g >c< b g e d c+ e a >c+< b a e c+ ]60",
        "[ o4 l8 v9 @2 e g e g f a f a ]60",
        "[ o2 l8 c c g g a a e& e ]60",
        "[ k8 h8 s8 h16 h16 k8 k8 s8 H8 ]60",
    ])
    .unwrap();
    let t = Instant::now();
    let r = synth::render_song(&song).unwrap();
    let secs = t.elapsed().as_secs_f64();
    assert!((r.duration_secs() - 90.0).abs() < 0.01, "{}", r.duration_secs());
    check("stress", &r);
    println!("90s busy song: {:.0}ms", secs * 1000.0);
    // (Offline, the engine plays two passes and keeps the second.)
    assert!(secs < budget_secs() / 2.0, "90s busy song took {secs:.2}s");
}

mod plugin {
    use std::time::Duration;

    use bevy::{prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
    use nat_han_adventures::{
        audio::{
            AudioOutput, Filters, Harmony, LiveClock, LivePlayer, Music, MusicChanged, MusicOverride, MusicStarted, NowPlaying, SfxCount,
            director, live::library, tuning::Tuning,
        },
        events::{CheckpointReached, Jumped},
        game::{Groove, LevelRun},
        level::Levels,
        save::Progress,
        state::{AppState, CurrentLevel},
    };

    /// Everything the music plugin announced.
    #[derive(Resource, Default)]
    struct Heard {
        started: Vec<MusicStarted>,
        changed: Vec<(f64, MusicChanged, LiveClock)>,
    }

    fn listen(
        time: Res<Time<Real>>,
        clock: Res<LiveClock>,
        mut heard: ResMut<Heard>,
        mut s: MessageReader<MusicStarted>,
        mut c: MessageReader<MusicChanged>,
    ) {
        heard.started.extend(s.read().cloned());
        let now = time.elapsed_secs_f64();
        heard.changed.extend(c.read().map(|m| (now, m.clone(), *clock)));
    }

    /// The plugin headless (no sound card: the music renders as real time passes), on a fixed
    /// 60 fps clock.
    fn app_with(overrides: Option<Filters>) -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            StatesPlugin,
            bevy::input::InputPlugin,
            nat_han_adventures::gameplay,
            nat_han_adventures::audio::plugin,
        ))
        .insert_resource(AudioOutput::Headless)
        .insert_resource(MusicOverride(overrides))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 60.0)))
        .init_resource::<Heard>()
        .add_systems(Last, listen);
        app.update();
        app
    }

    fn app() -> App {
        app_with(None)
    }

    fn go(app: &mut App, s: AppState) {
        app.world_mut().resource_mut::<NextState<AppState>>().set(s);
        app.update();
        app.update();
    }

    fn playing(app: &App) -> Option<Music> {
        app.world().resource::<LivePlayer>().now_playing()
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
        assert_eq!((now.music, now.title), (Music::Title, library::title(Music::Title)));
        assert!(now.title.starts_with("Sweet Georgia Brown"), "{}", now.title);
        assert_eq!(now.toast(), "");
        assert_eq!(now.tuning_now, None);
        // The engine is playing it.
        for _ in 0..10 {
            app.update();
        }
        let clock = app.world().resource::<LiveClock>().clock;
        assert!(clock.bpm > 100.0 && clock.position.sample > 0, "{clock:?}");
        assert_eq!(app.world().resource::<LivePlayer>().filters(), Some(Filters::default()));
    }

    /// The player's AUDIO DELAY steps the beat back by exactly that much, all the way through
    /// the bar: no freezing at the bar line while the music before it is still being heard.
    #[test]
    fn the_audio_delay_steps_the_heard_beat_back() {
        let mut plain = app();
        let mut delayed = app();
        delayed.world_mut().resource_mut::<Progress>().audio_delay_ms = 250;
        let mut checked = 0;
        for frame in 0..60 * 6 {
            plain.update();
            delayed.update();
            if frame < 60 {
                continue;
            }
            let (a, b) = (plain.world().resource::<LiveClock>().clock, delayed.world().resource::<LiveClock>().clock);
            assert!(a.bpm > 0.0 && a.bpm == b.bpm);
            let want = 0.25 * a.bpm as f64 / 60.0;
            let mut lag = a.position.song_beat - b.position.song_beat;
            if a.loop_beats > 0.0 {
                lag = lag.rem_euclid(a.loop_beats);
            }
            assert!((lag - want).abs() < 1e-6, "frame {frame}: heard {lag} beats behind, want {want} ({a:?} vs {b:?})");
            checked += 1;
        }
        assert!(checked > 200);
    }

    /// 5 toots then a checkpoint: Coltrane changes, played from a bar line without restarting
    /// the song. 20s of play later the periodic check finds no toots in the window and goes
    /// back to the original.
    #[test]
    fn toots_then_checkpoint_switch_to_coltrane_on_a_bar_line() {
        let mut app = app();
        app.world_mut().resource_mut::<CurrentLevel>().0 = 0;
        go(&mut app, AppState::Playing);
        let music = playing(&app).unwrap();
        let (_, song) = library::song(music).unwrap();
        assert!(song.chart.is_some(), "the test needs a song with a chart");
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
        let player = app.world().resource::<LivePlayer>();
        assert_eq!(player.pending_filters().map(|f| f.harmony), Some(Harmony::Coltrane));
        assert_eq!(player.filters(), Some(Filters::default()), "still as written until the bar line");

        let mut frames = 0;
        while app.world().resource::<Heard>().changed.is_empty() {
            app.update();
            frames += 1;
            assert!(frames < 60 * 10, "no switch after 10s");
        }
        let (switched_at, change, clock) = app.world().resource::<Heard>().changed[0].clone();
        assert_eq!(change.now.filters.harmony, Harmony::Coltrane);
        assert_eq!(change.now.reason, director::REASON_GIANT_STEPS);
        assert_eq!(change.now.toast(), "GIANT STEPS! - COLTRANE CHANGES");
        let bars = change.at_secs / bar;
        assert!((bars - bars.round()).abs() < 1e-6, "switched mid-bar: {} s = {bars} bars", change.at_secs);
        // Heard right at the bar line (within a frame and the output latency).
        let beat = clock.clock.position.beat;
        assert!(beat < 0.02 * clock.clock.bpm as f64 / 60.0 + 1e-9, "switched {beat} beats into a bar");
        // Within two bars of the decision (the next bar line not committed yet).
        assert!(switched_at >= decided_at && switched_at - decided_at < 2.0 * bar + 0.1, "{}", switched_at - decided_at);
        assert_eq!(app.world().resource::<NowPlaying>().filters.harmony, Harmony::Coltrane);
        assert_eq!(app.world().resource::<LivePlayer>().filters().unwrap().harmony, Harmony::Coltrane);
        assert_eq!(app.world().resource::<LivePlayer>().pending_filters(), None);
        assert!(app.world().resource::<Groove>().giant_steps(), "the physics follow");
        assert_eq!(app.world().resource::<Heard>().started.len(), started, "the song didn't restart");

        // The periodic check, MUSIC_CHECK_SECS of play after the checkpoint.
        let mut frames = 0;
        while app.world().resource::<Heard>().changed.len() < 2 {
            app.update();
            frames += 1;
            assert!(frames < 60 * 40, "no periodic switch after 40s");
        }
        let (_, back, _) = app.world().resource::<Heard>().changed[1].clone();
        assert_eq!(back.now.filters, Filters::default());
        let after = run_time(&app) - t0;
        assert!(after >= director::MUSIC_CHECK_SECS, "switched back too early ({after}s)");
        let bars = back.at_secs / bar;
        assert!((bars - bars.round()).abs() < 1e-6);
        assert!(!app.world().resource::<Groove>().giant_steps());
    }

    /// `NATHAN_MUSIC` forcing: every looping song starts in the forced filters (not the
    /// fanfare), physics and all; the laughing band's phrase tuning is readable every frame.
    #[test]
    fn nathan_music_forces_the_filters() {
        let forced = Filters { harmony: Harmony::Waltz, just_intonation: true };
        let mut app = app_with(Some(forced));
        app.world_mut().resource_mut::<CurrentLevel>().0 = 0;
        go(&mut app, AppState::Playing);
        let started = app.world().resource::<Heard>().started.last().unwrap().clone();
        assert_eq!((started.0.filters, started.0.reason), (forced, "NATHAN_MUSIC"));
        for _ in 0..30 {
            app.update();
        }
        let groove = *app.world().resource::<Groove>();
        assert!(groove.waltz() && groove.bounce);
        assert_eq!((groove.clock.beats_per_bar, groove.clock.beat_secs), (3, 0.5), "the world dances in 3");
        let now = app.world().resource::<NowPlaying>().clone();
        assert_eq!(now.filters, forced);
        let t = now.tuning_now.expect("the medley's tuning");
        assert_ne!(t, Tuning::Medley);
        assert_eq!(Some(t), app.world().resource::<LiveClock>().clock.position.song_beat.is_finite().then_some(t));
        // The director's decisions are overridden too (a death decides).
        app.world_mut().write_message(nat_han_adventures::events::PlayerDied { pos: Vec2::ZERO });
        for _ in 0..200 {
            app.update();
        }
        assert!(app.world().resource::<Heard>().changed.is_empty(), "still forced");
        // The fanfare plays as written.
        go(&mut app, AppState::LevelComplete);
        let started = app.world().resource::<Heard>().started.last().unwrap().clone();
        assert_eq!((started.0.music, started.0.filters), (Music::LevelClear, Filters::default()));
    }

    /// Sound effects go out on the same manager (headless: counted).
    #[test]
    fn gameplay_plays_sound_effects() {
        let mut app = app();
        let before = app.world().resource::<SfxCount>().0;
        app.world_mut().write_message(Jumped { pos: Vec2::ZERO, double: false });
        app.world_mut().write_message(nat_han_adventures::events::PlaySfx(nat_han_adventures::audio::Sfx::MenuMove));
        app.update();
        assert_eq!(app.world().resource::<SfxCount>().0, before + 2);
        // Han's babble: a few blips over the next half second.
        app.world_mut().write_message(nat_han_adventures::events::HanSays { text: "ONE-two-three!".into() });
        for _ in 0..40 {
            app.update();
        }
        assert!(app.world().resource::<SfxCount>().0 >= before + 4);
    }

    /// The adaptive engine's band freedom reaches the engine: at level start and on decisions
    /// (here a death beyond the level's expectation: calmer, sparser).
    #[test]
    fn the_band_freedom_is_posted_on_decisions() {
        let mut app = app();
        assert_eq!(app.world().resource::<LivePlayer>().freedom(), None);
        app.world_mut().resource_mut::<CurrentLevel>().0 = 0;
        go(&mut app, AppState::Playing);
        app.update();
        let start = app.world().resource::<LivePlayer>().freedom().expect("posted at level start");
        for _ in 0..4 {
            app.world_mut().write_message(nat_han_adventures::events::PlayerDied { pos: Vec2::ZERO });
            app.update();
        }
        let after = app.world().resource::<LivePlayer>().freedom().unwrap();
        assert!(after.lead < start.lead && after.dynamics <= start.dynamics, "{start:?} → {after:?}");
    }
}
