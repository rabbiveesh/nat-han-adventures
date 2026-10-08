//! The live music engine (`audio::live`): song files, the engine against the offline renderer,
//! scheduling, inputs, clocks and performance.

use nat_han_adventures::audio::{
    Music, chart,
    live::{convert, library, mml as live_mml, song::SongFile, syntax},
    mml::{self, Channel},
    songs,
};

// --- song files ---------------------------------------------------------------------------

/// Until the swap, songs.rs stays the old engine's source: the files must not drift from it.
#[test]
fn every_song_file_parses_to_the_same_events_as_songs_rs() {
    for m in Music::ALL {
        let old = songs::song(m);
        let stem = library::stem(m);
        let new = library::load(stem).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(new.title, old.title, "{stem}");
        assert_eq!((new.bpm, new.swing, new.key, new.looping), (old.bpm, old.swing, old.key, old.looping), "{stem}");
        let srcs = [old.pulse1, old.pulse2, old.triangle, old.noise];
        for (ch, src) in srcs.iter().enumerate() {
            let channel = if ch == 3 { Channel::Drums } else { Channel::Melodic };
            assert_eq!(new.tracks[ch], mml::parse(src, channel).unwrap(), "{stem} channel {ch}");
        }
        let old_chart = (!old.chords.trim().is_empty()).then(|| chart::parse(old.chords).unwrap());
        assert_eq!(new.chart, old_chart, "{stem}: chart");
        // And the committed file is exactly what the converter makes today.
        assert_eq!(library::text(stem).unwrap(), convert::to_song_file(&old).unwrap(), "{stem}: run `cargo run --example convert_songs`");
    }
}

#[test]
fn song_files_round_trip() {
    for (stem, text) in library::FILES {
        let a = SongFile::parse(text).unwrap();
        let b = SongFile::parse(&a.to_text()).unwrap_or_else(|e| panic!("{stem}: {e}"));
        assert_eq!(a, b, "{stem}");
    }
}

#[test]
fn song_file_bar_lines_are_checked() {
    let text = library::text("shave_and_a_haircut").unwrap();
    // Every bar of every channel is checked: shorten any one note and the file is rejected.
    let broken = text.replacen("o5 c4 o4 g8 g8", "o5 c4 o4 g8 g16", 1);
    assert_ne!(broken, text);
    let e = SongFile::parse(&broken).unwrap_err();
    assert!(e.msg.contains("[pulse1]") && e.msg.contains("short"), "{e}");
    assert_eq!(broken.lines().nth(e.line - 1).unwrap().trim_start().get(..6), Some("v13 @1"), "{e}");
    // A bar line in the middle of a bar.
    let e = SongFile::parse(&text.replacen("g8 g8 a4", "g8 | g8 a4", 1)).unwrap_err();
    assert!(e.msg.contains("bar 1 is 1.5 beats long"), "{e}");
    // The converted files have (nearly) every bar line in place.
    for (stem, text) in library::FILES {
        let s = SongFile::parse(text).unwrap();
        let bars = s.bars();
        let marks = s.sources.iter().map(|src| src.matches('|').count()).sum::<usize>();
        assert!(marks >= bars, "{stem}: only {marks} bar lines for {bars} bars x 4 channels");
    }
}

/// Every character the MML parser treats as a token (not "unexpected"), every chord quality,
/// every section and every `[song]` key appears in the cheat sheet.
#[test]
fn the_cheat_sheet_covers_every_token() {
    let syntaxes: Vec<&str> = syntax::CHEAT_SHEET.iter().flat_map(|(_, rows)| rows.iter().map(|(s, _)| *s)).collect();
    let listed = |tok: &str| syntaxes.iter().any(|s| s.contains(tok));
    for c in (0x21u8..0x7f).map(char::from) {
        for channel in [Channel::Melodic, Channel::Drums] {
            // A token on its own, or with a number (commands), or after a note (accidentals, dots).
            let n = if channel == Channel::Melodic { "c4" } else { "k4" };
            let probes = [format!("{c}"), format!("{c}4"), format!("{n}{c}"), format!("{c}{n}")];
            let known = probes.iter().any(|p| match live_mml::parse(p, channel, 4.0) {
                Ok(_) => true,
                Err(e) => !e.msg.starts_with("unexpected"),
            });
            // Digits only ever follow something; they're covered by the lengths and commands.
            if known && !c.is_ascii_digit() {
                assert!(listed(&c.to_string()), "`{c}` ({channel:?}) is a token but not in the cheat sheet");
            }
        }
    }
    for (_, q) in chart::Quality::ALL {
        if !q.is_empty() {
            assert!(syntaxes.iter().any(|s| s.split_whitespace().any(|w| w == q)), "chord quality `{q}`");
        }
    }
    for s in syntax::SECTIONS {
        assert!(listed(&format!("[{s}]")), "section [{s}]");
    }
    for k in syntax::SONG_KEYS {
        assert!(listed(&format!("{k} = ")), "key {k}");
    }
    assert!(syntax::cheat_sheet_text().lines().count() > 30);
}

// --- streaming vs offline -----------------------------------------------------------------

use bevy_kira_audio::prelude::Frame;
use nat_han_adventures::audio::{
    Filters, Harmony,
    live::{Engine, EngineConfig, Input},
    synth,
};

const SEED: u64 = 7;

fn engine(m: Music, inputs: &[Input]) -> Engine {
    let file = library::load(library::stem(m)).unwrap();
    let mut e = Engine::with_config(&file, synth::SAMPLE_RATE, EngineConfig { seed: SEED, ..EngineConfig::default() }).unwrap();
    for i in inputs {
        e.post(*i);
    }
    e
}

fn render(e: &mut Engine, frames: usize, block: usize) -> Vec<Frame> {
    let mut out = vec![Frame::ZERO; frames];
    for chunk in out.chunks_mut(block) {
        e.fill(chunk);
    }
    out
}

/// Per-bar RMS of the difference (both channels), and the max abs difference.
fn bar_diffs(a: &[Frame], b: &[Frame], bar_starts: &[u64]) -> (Vec<f64>, f32) {
    let mut max = 0.0f32;
    let rms = bar_starts
        .windows(2)
        .map(|w| {
            let (s, e) = (w[0] as usize, (w[1] as usize).min(a.len()).min(b.len()));
            let mut sum = 0.0f64;
            for i in s..e {
                let (dl, dr) = (a[i].left - b[i].left, a[i].right - b[i].right);
                max = max.max(dl.abs()).max(dr.abs());
                sum += (dl as f64).powi(2) + (dr as f64).powi(2);
            }
            (sum / (2 * (e.saturating_sub(s)).max(1)) as f64).sqrt()
        })
        .collect();
    (rms, max)
}

/// The engine at freedom 0 against the offline renderer (same song, filters and seed), every
/// song through every filter it can take: the second loop pass is bit-identical; the first
/// differs only where the offline loop has wrapped the end's drum tails onto its start (a fresh
/// engine has no previous pass ringing), so only within bar 1; a one-shot only in the offline
/// render's final fade.
#[test]
fn streaming_matches_offline_at_freedom_0() {
    let mut report = Vec::new();
    for m in Music::ALL {
        let song = songs::song(m);
        for harmony in Harmony::ALL {
            if harmony != Harmony::Original && song.chords.trim().is_empty() {
                continue;
            }
            for just_intonation in [false, true] {
                let f = Filters { harmony, just_intonation };
                let name = if f == Filters::default() { "plain".to_string() } else { f.label() };
                let off = synth::render_song_with(&song, f, SEED).unwrap();
                let mut e = engine(m, &[Input::SetFilters(f)]);
                let len = e.shape().len as usize;
                let starts = e.shape().bar_starts.clone();
                if !song.looping {
                    // One-shot: identical up to the offline render's trim-and-fade at the end.
                    let live = render(&mut e, len + 32_000, 512);
                    let n = off.frames.len() - (0.005 * 32_000.0) as usize;
                    let (rms, max) = bar_diffs(&live[..n], &off.frames[..n], &[0, n as u64]);
                    assert_eq!(max, 0.0, "{m:?} {f:?}: one-shot differs (rms {rms:?})");
                    // After the offline render's end, silence.
                    let tail = live[off.frames.len()..].iter().map(|f| f.left.abs().max(f.right.abs())).fold(0.0, f32::max);
                    assert!(tail < 1e-3, "{m:?}: the end rings on ({tail})");
                    assert!(e.finished(), "{m:?}: a one-shot finishes");
                    report.push(format!("{m:?} {name}: bit-exact up to the offline fade-out"));
                    continue;
                }
                let live = render(&mut e, 2 * len, 512);
                let (r0, _) = bar_diffs(&live[..len], &off.frames, &starts);
                let (r1, max1) = bar_diffs(&live[len..], &off.frames, &starts);
                assert_eq!(max1, 0.0, "{m:?} {f:?}: second pass differs: per-bar rms {r1:?}");
                assert!(r0[1..].iter().all(|&r| r == 0.0), "{m:?} {f:?}: first pass differs after bar 1: {r0:?}");
                assert!(r0[0] < 5e-3, "{m:?} {f:?}: bar 1 differs by more than the wrapped drum tails: {}", r0[0]);
                report.push(format!("{m:?} {name}: pass 2 bit-exact; pass 1 bit-exact except bar 1, rms {:.1e} (wrapped drum tails)", r0[0]));
            }
        }
    }
    println!("{}", report.join("\n"));
}

/// The same output whatever the block size, with every dial up and inputs on the way.
#[test]
fn block_size_doesnt_matter() {
    let setup = [
        Input::SetFreedom { lead: 0.6, comp: 0.6, bass: 0.6, drums: 0.6, dynamics: 0.6 },
        Input::SetFilters(Filters { harmony: Harmony::Original, just_intonation: true }),
    ];
    let later = [Input::Toot, Input::Toot, Input::Death, Input::ForceHarmony(Some(Harmony::Coltrane))];
    let at = 4096 * 40;
    let run = |block: usize| {
        let mut e = engine(Music::World(3), &setup);
        let mut out = render(&mut e, at, block);
        for i in later {
            e.post(i);
        }
        let rest = 3 * e.shape().len as usize - at;
        out.extend(render(&mut e, rest, block));
        out
    };
    let reference = run(4096);
    for block in [64, 512, 1000] {
        let out = run(block);
        let same = out.iter().zip(&reference).all(|(a, b)| a.left.to_bits() == b.left.to_bits() && a.right.to_bits() == b.right.to_bits());
        assert!(same && out.len() == reference.len(), "block {block}");
    }
}

/// Render up to absolute sample `to`.
fn render_to(e: &mut Engine, to: u64) -> Vec<Frame> {
    let n = (to - e.beat_clock().position.sample) as usize;
    render(e, n, 512)
}

fn first_difference(a: &[Frame], b: &[Frame]) -> Option<usize> {
    a.iter().zip(b).position(|(x, y)| x.left.to_bits() != y.left.to_bits() || x.right.to_bits() != y.right.to_bits())
}

/// A filter change lands on the first bar line not committed yet: posted mid-bar, at the next
/// one; posted in the last beat (after that bar was committed), at the one after.
#[test]
fn a_filter_change_lands_on_the_next_bar_line() {
    let m = Music::Title;
    let coltrane = Filters { harmony: Harmony::Coltrane, just_intonation: false };
    let mut plain = engine(m, &[]);
    let shape = plain.shape().clone();
    let bar = |k: usize| shape.bar_starts[k];
    let plain_out = render(&mut plain, bar(6) as usize, 512);

    for (post_at, lands) in [((bar(2) + bar(3)) / 2, 3), (bar(3) - (shape.samples_per_beat * 0.5) as u64, 4)] {
        let mut e = engine(m, &[]);
        let mut out = render_to(&mut e, post_at);
        e.post(Input::SetFilters(coltrane));
        // Just before the bar line: still as written; on it, Coltrane.
        out.extend(render_to(&mut e, bar(lands) - 1));
        assert_eq!(e.state().harmony, Harmony::Original);
        out.extend(render_to(&mut e, bar(lands)));
        assert_eq!(e.state().harmony, Harmony::Coltrane, "lands at bar {lands}");
        let upcoming: Vec<_> = e.state().upcoming.iter().map(|b| (b.slot.song_bar, b.harmony)).collect();
        assert_eq!(upcoming[0], (lands, Harmony::Coltrane));
        out.extend(render_to(&mut e, bar(6)));
        // The audio is the plain version's, sample for sample, right up to the bar line.
        let d = first_difference(&out, &plain_out).expect("Coltrane changes the music");
        assert!(d >= bar(lands) as usize, "changed at sample {d}, before bar line {}", bar(lands));
        assert!(d < bar(lands + 1) as usize, "the change is heard in its first bar");
    }
    // And what lands is the Coltrane arrangement itself, from that bar on.
    let mut e = engine(m, &[]);
    render_to(&mut e, (bar(2) + bar(3)) / 2);
    e.post(Input::SetFilters(coltrane));
    render_to(&mut e, bar(3) - 10);
    let mut all = engine(m, &[Input::SetFilters(coltrane)]);
    render_to(&mut all, bar(3) - 10);
    let bar3 = |e: &Engine| e.pending_events().filter(|ev| ev.bar == 3).map(|ev| (ev.ch, ev.start, ev.end, ev.sound)).collect::<Vec<_>>();
    assert_eq!(bar3(&e), bar3(&all));
    assert!(!bar3(&e).is_empty());
}

/// Inputs and dial changes replan the bars ahead, never the ones committed.
#[test]
fn replanning_never_alters_committed_events() {
    let free = Input::SetFreedom { lead: 0.8, comp: 0.8, bass: 0.8, drums: 0.8, dynamics: 0.8 };
    let m = Music::World(5);
    let mut e = engine(m, &[free]);
    let shape = e.shape().clone();
    // In the last beat of bar 2 (0-based): bars 0-2 are committed, the phrase plan runs on.
    let when = shape.bar_starts[2] - (shape.samples_per_beat * 0.5) as u64;
    render_to(&mut e, when);
    let committed: Vec<_> = e.pending_events().copied().collect();
    assert!(committed.iter().any(|ev| ev.bar == 2) && committed.iter().all(|ev| ev.bar <= 2));
    let before = e.state().musicians.map(|m| m.plan.unwrap());
    assert!(before.iter().all(|p| p.last_bar() > 2), "{before:?}");
    for i in [
        Input::Toot,
        Input::Toot,
        Input::Death,
        Input::Checkpoint,
        Input::Land { speed: 400.0 },
        Input::SetFreedom { lead: 0.1, comp: 1.0, bass: 0.0, drums: 1.0, dynamics: 0.0 },
        Input::ForceHarmony(Some(Harmony::Quartal)),
        Input::ForceTuning(Some(nat_han_adventures::audio::tuning::Tuning::Tet7)),
    ] {
        e.post(i);
    }
    e.fill(&mut []);
    let after: Vec<_> = e.pending_events().copied().collect();
    assert_eq!(after, committed, "committed events changed");
    // The plans changed for the bars ahead, and only for those.
    let replanned = e.state().musicians.map(|m| m.plan.unwrap());
    for (b, a) in before.iter().zip(&replanned) {
        for bar in b.start..=b.last_bar() {
            if bar <= 2 {
                assert_eq!(a.intent(bar), b.intent(bar), "committed bar {bar} replanned");
            }
        }
    }
    assert_ne!(replanned[0].intent(3), before[0].intent(3), "the lead didn't replan");
    assert!(replanned[3].intent(3).accent, "the drums didn't cue a crash after the death");
    assert!(replanned[0].intent(3).answer, "the lead didn't cue an answer to the toot");
    // What's heard up to the next bar line is what was committed: the same as without the inputs.
    let mut quiet = engine(m, &[free]);
    let a = render_to(&mut quiet, shape.bar_starts[4]);
    let mut e2 = engine(m, &[free]);
    let mut b = render_to(&mut e2, when);
    e2.post(Input::ForceHarmony(Some(Harmony::Quartal)));
    e2.post(Input::SetFreedom { lead: 0.0, comp: 0.0, bass: 0.0, drums: 0.0, dynamics: 0.0 });
    b.extend(render_to(&mut e2, shape.bar_starts[4]));
    let d = first_difference(&a, &b).expect("the inputs change what comes after");
    assert!(d >= shape.bar_starts[3] as usize - 400, "committed bar 2 sounded different from sample {d}");
}

/// Inputs apply in the order they were posted, all before the next commit.
#[test]
fn inputs_apply_in_order() {
    let m = Music::World(1);
    let coltrane = Filters { harmony: Harmony::Coltrane, just_intonation: false };
    let mut e = engine(m, &[]);
    let shape = e.shape().clone();
    let harmony_of = |e: &Engine, bar: usize| e.state().upcoming.iter().find(|b| b.slot.song_bar == bar).map(|b| b.harmony);
    let steps: [(&[Input], Harmony); 5] = [
        (&[Input::ForceHarmony(Some(Harmony::Quartal)), Input::ForceHarmony(Some(Harmony::MelodicMinor))], Harmony::MelodicMinor),
        (&[Input::SetFilters(coltrane)], Harmony::MelodicMinor),
        (&[Input::ForceHarmony(None)], Harmony::Coltrane),
        (&[Input::ForceHarmony(Some(Harmony::Quartal)), Input::ForceHarmony(None), Input::SetFilters(Filters::default())], Harmony::Original),
        (&[Input::SetFilters(coltrane), Input::SetFilters(Filters::default()), Input::SetFilters(coltrane)], Harmony::Coltrane),
    ];
    for (k, (inputs, want)) in steps.iter().enumerate() {
        let bar = k + 1;
        render_to(&mut e, shape.bar_starts[bar] - shape.samples_per_beat as u64 * 2);
        for i in *inputs {
            e.post(*i);
        }
        render_to(&mut e, shape.bar_starts[bar] + 1);
        assert_eq!(harmony_of(&e, bar), Some(*want), "step {k}");
    }
    // The freedom dials too: the last one posted wins.
    e.post(Input::SetFreedom { lead: 1.0, comp: 1.0, bass: 1.0, drums: 1.0, dynamics: 1.0 });
    e.post(Input::SetFreedom { lead: 0.25, comp: 0.5, bass: 0.75, drums: 0.0, dynamics: 0.1 });
    e.fill(&mut []);
    let s = e.state();
    assert_eq!(s.musicians.map(|m| m.freedom), [0.25, 0.5, 0.75, 0.0]);
    assert_eq!(s.freedom.dynamics, 0.1);
    // A flood of inputs never blocks or allocates without bound: the oldest are dropped.
    for _ in 0..10_000 {
        e.post(Input::Nugget);
    }
    e.fill(&mut [Frame::ZERO; 64]);
}

/// The beat clock is the song position the offline renderer would be at, sample for sample.
#[test]
fn the_beat_clock_is_accurate() {
    for m in [Music::Title, Music::World(4), Music::LevelClear] {
        let mut e = engine(m, &[]);
        let sh = e.shape().clone();
        let passes = if sh.looping { 2 } else { 1 };
        let mut t = 0u64;
        // Bar lines, and points inside bars, over two passes.
        for pass in 0..passes {
            for k in 0..sh.bars {
                for frac in [0.0, 0.3, 0.77] {
                    let s = pass * sh.len + sh.bar_starts[k] + ((sh.bar_starts[k + 1] - sh.bar_starts[k]) as f64 * frac) as u64;
                    if s < t {
                        continue;
                    }
                    render_to(&mut e, s);
                    t = s;
                    let c = e.beat_clock();
                    let p = c.position;
                    assert_eq!((p.sample, p.pass, p.song_bar, p.bar), (s, pass, k, pass * sh.bars as u64 + k as u64), "{m:?}");
                    let want = (s - pass * sh.len - sh.bar_starts[k]) as f64 / sh.samples_per_beat;
                    assert!((p.beat - want).abs() < 1e-9, "{m:?} bar {k}: beat {} vs {want}", p.beat);
                    if frac == 0.0 {
                        assert_eq!((p.beat, c.beat_index, c.phase), (0.0, 0, 0.0));
                    }
                    // Song beats match the sample within the bar-line rounding (half a sample).
                    let in_loop = (s - pass * sh.len) as f64;
                    assert!((p.song_beat * sh.samples_per_beat - in_loop).abs() <= 0.5 + 1e-6, "{m:?}");
                    assert_eq!(c.beat_index, p.beat.floor() as u32);
                    assert!((0.0..1.0).contains(&c.phase));
                    // Extrapolating a quarter second matches rendering it (within the rounding).
                    if sh.looping && k % 4 == 1 {
                        let ahead = c.advanced(0.25);
                        let mut probe = engine(m, &[]);
                        render_to(&mut probe, s + (0.25 * sh.sample_rate as f64) as u64);
                        let real = probe.beat_clock().position;
                        assert_eq!(ahead.position.bar, real.bar, "{m:?}");
                        assert!((ahead.position.song_beat - real.song_beat).abs() < 1e-3, "{m:?}: {ahead:?} vs {real:?}");
                    }
                }
            }
        }
    }
}

/// Across the loop seam the signal is as continuous as anywhere inside the song.
#[test]
fn the_loop_seam_doesnt_click() {
    for m in Music::ALL.into_iter().filter(|&m| m != Music::LevelClear) {
        for f in [Filters::default(), Filters { harmony: Harmony::Quartal, just_intonation: true }] {
            let mut e = engine(m, &[Input::SetFilters(f)]);
            let len = e.shape().len as usize;
            let out = render(&mut e, 2 * len + 4096, 512);
            let step = |i: usize| (out[i].left - out[i - 1].left).abs().max((out[i].right - out[i - 1].right).abs());
            let max_inside = (len + 1..2 * len).map(step).fold(0.0f32, f32::max);
            for seam in [len, 2 * len] {
                let jump = (seam - 2..seam + 3).map(step).fold(0.0f32, f32::max);
                assert!(jump <= max_inside.max(0.05), "{m:?} {f:?}: seam jump {jump} (max step inside {max_inside})");
            }
        }
    }
}

/// Far under real time: microseconds per 512-frame block, with everything on.
#[test]
fn rendering_is_far_under_real_time() {
    let block = 512;
    let mut worst_song = (0.0f64, "");
    for m in Music::ALL {
        let mut e = engine(m, &[
            Input::SetFilters(Filters { harmony: Harmony::Coltrane, just_intonation: true }),
            Input::SetFreedom { lead: 0.7, comp: 0.7, bass: 0.7, drums: 0.7, dynamics: 0.7 },
        ]);
        let mut buf = vec![Frame::ZERO; block];
        let blocks = 20 * synth::SAMPLE_RATE as usize / block;
        let mut worst = 0.0f64;
        let t = std::time::Instant::now();
        for _ in 0..blocks {
            let b = std::time::Instant::now();
            e.fill(&mut buf);
            worst = worst.max(b.elapsed().as_secs_f64());
        }
        let per_block = t.elapsed().as_secs_f64() / blocks as f64;
        let realtime = block as f64 / synth::SAMPLE_RATE as f64;
        println!("{m:?}: {:.1} us/block mean, {:.1} us worst ({:.0}x real time)", per_block * 1e6, worst * 1e6, realtime / per_block);
        if per_block > worst_song.0 {
            worst_song = (per_block, songs::song(m).title);
        }
        // Debug builds (opt-level 1) are slow; release has a far tighter budget.
        let budget = if cfg!(debug_assertions) { 0.25 } else { 0.02 };
        assert!(per_block < realtime * budget, "{m:?}: {:.0}us per block", per_block * 1e6);
        assert!(worst < realtime, "{m:?}: a block took {:.0}us, longer than it plays", worst * 1e6);
    }
    println!("slowest: {} at {:.1} us/block", worst_song.1, worst_song.0 * 1e6);
}

/// Above freedom 0 the placeholders play (grace notes, fills); at 0 nothing changes.
#[test]
fn freedom_brings_the_placeholders_in() {
    use nat_han_adventures::audio::live::voice::Sound;
    use nat_han_adventures::audio::mml::Drum;
    let m = Music::World(2);
    let count = |inputs: &[Input]| {
        let mut e = engine(m, inputs);
        let shape = e.shape().clone();
        let (mut events, mut graces, mut snares) = (0, 0, 0);
        for bar in 1..shape.bars {
            render_to(&mut e, shape.bar_starts[bar] - 10);
            for ev in e.pending_events().filter(|ev| ev.bar == bar as u64) {
                events += 1;
                graces += (ev.ch == 0 && ((ev.end - ev.start) as f64) < shape.samples_per_beat * 0.15) as u32;
                snares += (ev.sound == Sound::Drum(Drum::Snare)) as u32;
            }
        }
        (events, graces, snares)
    };
    let plain = count(&[]);
    let free = count(&[Input::SetFreedom { lead: 1.0, comp: 1.0, bass: 1.0, drums: 1.0, dynamics: 1.0 }]);
    assert!(free.1 > plain.1 + 5, "grace notes: {plain:?} vs {free:?}");
    assert!(free.2 > plain.2 + 5, "fills: {plain:?} vs {free:?}");
}
