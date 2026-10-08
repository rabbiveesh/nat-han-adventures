//! The live music engine (`audio::live`): song files, the engine against the old renderer's
//! output, scheduling, inputs, clocks, the waltz and performance.

use nat_han_adventures::audio::{
    Music, chart,
    live::{library, song::SongFile, syntax},
    mml::{self, Channel},
};

// --- song files ---------------------------------------------------------------------------

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
            let known = probes.iter().any(|p| match mml::parse_checked(p, channel, 4.0) {
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

use kira::Frame;
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

/// What the old pre-rendering engine rendered (seed 7), every song through every filter it can
/// take: (music, harmony, laughing band, frames, RMS left, RMS right). The live engine was
/// checked bit-identical to it (from its second loop pass; one-shots up to the old fade-out)
/// before it was retired; these keep the music from drifting.
const OLD_RENDERS: [(Music, Harmony, bool, usize, f64, f64); 80] = [
    (Music::Title, Harmony::Original, false, 1335652, 0.185888, 0.173820),
    (Music::Title, Harmony::Original, true, 1335652, 0.185745, 0.173751),
    (Music::Title, Harmony::Coltrane, false, 1335652, 0.185404, 0.173036),
    (Music::Title, Harmony::Coltrane, true, 1335652, 0.185662, 0.173369),
    (Music::Title, Harmony::Quartal, false, 1335652, 0.180266, 0.168990),
    (Music::Title, Harmony::Quartal, true, 1335652, 0.179898, 0.168661),
    (Music::Title, Harmony::MelodicMinor, false, 1335652, 0.185620, 0.173285),
    (Music::Title, Harmony::MelodicMinor, true, 1335652, 0.185010, 0.172688),
    (Music::Title, Harmony::Waltz, false, 3072000, 0.133936, 0.119291),
    (Music::Title, Harmony::Waltz, true, 3072000, 0.133905, 0.119348),
    (Music::World(1), Harmony::Original, false, 1861818, 0.196146, 0.174048),
    (Music::World(1), Harmony::Original, true, 1861818, 0.196037, 0.173907),
    (Music::World(1), Harmony::Coltrane, false, 1861818, 0.198321, 0.179597),
    (Music::World(1), Harmony::Coltrane, true, 1861818, 0.198086, 0.179359),
    (Music::World(1), Harmony::Quartal, false, 1861818, 0.192867, 0.175132),
    (Music::World(1), Harmony::Quartal, true, 1861818, 0.192588, 0.174860),
    (Music::World(1), Harmony::MelodicMinor, false, 1861818, 0.197713, 0.178343),
    (Music::World(1), Harmony::MelodicMinor, true, 1861818, 0.197752, 0.178394),
    (Music::World(1), Harmony::Waltz, false, 3072000, 0.152487, 0.128558),
    (Music::World(1), Harmony::Waltz, true, 3072000, 0.152626, 0.128704),
    (Music::World(2), Harmony::Original, false, 1280000, 0.187497, 0.174533),
    (Music::World(2), Harmony::Original, true, 1280000, 0.187486, 0.174492),
    (Music::World(2), Harmony::Coltrane, false, 1280000, 0.188423, 0.176131),
    (Music::World(2), Harmony::Coltrane, true, 1280000, 0.187993, 0.175667),
    (Music::World(2), Harmony::Quartal, false, 1280000, 0.181028, 0.169010),
    (Music::World(2), Harmony::Quartal, true, 1280000, 0.181659, 0.169645),
    (Music::World(2), Harmony::MelodicMinor, false, 1280000, 0.187594, 0.174772),
    (Music::World(2), Harmony::MelodicMinor, true, 1280000, 0.187178, 0.174338),
    (Music::World(2), Harmony::Waltz, false, 3072000, 0.137759, 0.121169),
    (Music::World(2), Harmony::Waltz, true, 3072000, 0.137958, 0.121389),
    (Music::World(3), Harmony::Original, false, 1462857, 0.185373, 0.172563),
    (Music::World(3), Harmony::Original, true, 1462857, 0.186069, 0.173294),
    (Music::World(3), Harmony::Coltrane, false, 1462857, 0.185825, 0.173359),
    (Music::World(3), Harmony::Coltrane, true, 1462857, 0.186284, 0.173863),
    (Music::World(3), Harmony::Quartal, false, 1462857, 0.180188, 0.167976),
    (Music::World(3), Harmony::Quartal, true, 1462857, 0.180291, 0.168053),
    (Music::World(3), Harmony::MelodicMinor, false, 1462857, 0.185617, 0.172580),
    (Music::World(3), Harmony::MelodicMinor, true, 1462857, 0.185796, 0.172766),
    (Music::World(3), Harmony::Waltz, false, 3072000, 0.137806, 0.121323),
    (Music::World(3), Harmony::Waltz, true, 3072000, 0.137633, 0.121132),
    (Music::World(4), Harmony::Original, false, 2021053, 0.195732, 0.176348),
    (Music::World(4), Harmony::Original, true, 2021053, 0.195448, 0.176040),
    (Music::World(4), Harmony::Coltrane, false, 2021053, 0.196652, 0.178110),
    (Music::World(4), Harmony::Coltrane, true, 2021053, 0.196377, 0.177898),
    (Music::World(4), Harmony::Quartal, false, 2021053, 0.190951, 0.173030),
    (Music::World(4), Harmony::Quartal, true, 2021053, 0.190875, 0.172974),
    (Music::World(4), Harmony::MelodicMinor, false, 2021053, 0.196035, 0.177438),
    (Music::World(4), Harmony::MelodicMinor, true, 2021053, 0.196683, 0.178140),
    (Music::World(4), Harmony::Waltz, false, 3840000, 0.151193, 0.128335),
    (Music::World(4), Harmony::Waltz, true, 3840000, 0.150752, 0.127820),
    (Music::World(5), Harmony::Original, false, 1440000, 0.185700, 0.174370),
    (Music::World(5), Harmony::Original, true, 1440000, 0.185174, 0.173775),
    (Music::World(5), Harmony::Coltrane, false, 1440000, 0.185512, 0.174528),
    (Music::World(5), Harmony::Coltrane, true, 1440000, 0.185282, 0.174285),
    (Music::World(5), Harmony::Quartal, false, 1440000, 0.181077, 0.172621),
    (Music::World(5), Harmony::Quartal, true, 1440000, 0.180907, 0.172414),
    (Music::World(5), Harmony::MelodicMinor, false, 1440000, 0.184995, 0.173803),
    (Music::World(5), Harmony::MelodicMinor, true, 1440000, 0.185335, 0.174105),
    (Music::World(5), Harmony::Waltz, false, 3456000, 0.133628, 0.119191),
    (Music::World(5), Harmony::Waltz, true, 3456000, 0.133678, 0.119220),
    (Music::LevelClear, Harmony::Original, false, 109777, 0.182002, 0.172389),
    (Music::LevelClear, Harmony::Original, true, 109777, 0.181957, 0.172312),
    (Music::LevelClear, Harmony::Coltrane, false, 109777, 0.193728, 0.180419),
    (Music::LevelClear, Harmony::Coltrane, true, 109777, 0.192803, 0.179814),
    (Music::LevelClear, Harmony::Quartal, false, 109777, 0.187042, 0.177560),
    (Music::LevelClear, Harmony::Quartal, true, 109777, 0.187021, 0.177705),
    (Music::LevelClear, Harmony::MelodicMinor, false, 109777, 0.191642, 0.178359),
    (Music::LevelClear, Harmony::MelodicMinor, true, 109777, 0.192590, 0.179508),
    (Music::LevelClear, Harmony::Waltz, false, 192063, 0.140607, 0.122660),
    (Music::LevelClear, Harmony::Waltz, true, 192063, 0.140374, 0.122455),
    (Music::Victory, Harmony::Original, false, 1462857, 0.194583, 0.180031),
    (Music::Victory, Harmony::Original, true, 1462857, 0.194539, 0.179967),
    (Music::Victory, Harmony::Coltrane, false, 1462857, 0.193340, 0.177275),
    (Music::Victory, Harmony::Coltrane, true, 1462857, 0.193153, 0.177019),
    (Music::Victory, Harmony::Quartal, false, 1462857, 0.186634, 0.170973),
    (Music::Victory, Harmony::Quartal, true, 1462857, 0.187740, 0.172180),
    (Music::Victory, Harmony::MelodicMinor, false, 1462857, 0.193388, 0.177215),
    (Music::Victory, Harmony::MelodicMinor, true, 1462857, 0.193151, 0.176838),
    (Music::Victory, Harmony::Waltz, false, 3072000, 0.142007, 0.123380),
    (Music::Victory, Harmony::Waltz, true, 3072000, 0.142055, 0.123392),
];

/// The offline renders (the engine at freedom 0, run to the end) are still what the old engine
/// played: the same length, the same loudness on each side to six decimals.
#[test]
fn renders_match_the_old_engine() {
    let mut seen = 0;
    for (m, harmony, just_intonation, frames, l, r) in OLD_RENDERS {
        let (_, song) = library::song(m).unwrap();
        let f = Filters { harmony, just_intonation };
        let out = synth::render_song_with(song, f, SEED).unwrap();
        let n = out.frames.len() as f64;
        let rms = |x: fn(&Frame) -> f32| (out.frames.iter().map(|f| (x(f) as f64).powi(2)).sum::<f64>() / n).sqrt();
        assert_eq!(out.frames.len(), frames, "{m:?} {f:?}: length");
        let (gl, gr) = (rms(|f| f.left), rms(|f| f.right));
        assert!((gl - l).abs() < 2e-6 && (gr - r).abs() < 2e-6, "{m:?} {f:?}: rms {gl:.6} {gr:.6}, was {l:.6} {r:.6}");
        seen += 1;
    }
    assert_eq!(seen, 80);
}

/// A one-shot plays out, rings off and finishes; a fresh looping engine's first pass is its
/// steady loop except for bar 1 (no previous pass ringing over it).
#[test]
fn one_shots_finish_and_loops_settle() {
    let mut e = engine(Music::LevelClear, &[]);
    let len = e.shape().len as usize;
    let live = render(&mut e, len + 32_000, 512);
    let tail = live[len + 16_000..].iter().map(|f| f.left.abs().max(f.right.abs())).fold(0.0, f32::max);
    assert!(tail < 1e-3, "the end rings on ({tail})");
    assert!(e.finished());
    for f in [Filters::default(), Filters { harmony: Harmony::Waltz, just_intonation: true }] {
        let mut e = engine(Music::World(3), &[Input::SetFilters(f)]);
        let shape = if f.harmony == Harmony::Waltz { e.waltz_shape().unwrap().clone() } else { e.shape().clone() };
        let len = shape.len as usize;
        let out = render(&mut e, 2 * len, 512);
        let (r0, _) = bar_diffs(&out[..len], &out[len..], &shape.bar_starts);
        assert!(r0[1..].iter().all(|&r| r == 0.0), "{f:?}: first pass differs after bar 1: {r0:?}");
        assert!(r0[0] < 5e-3, "{f:?}: bar 1 differs by more than the drum tails: {}", r0[0]);
    }
}

/// The same output whatever the block size, with every dial up and inputs on the way.
#[test]
fn block_size_doesnt_matter() {
    let setup = [
        Input::SetFreedom { lead: 0.6, comp: 0.6, bass: 0.6, drums: 0.6, dynamics: 0.6 },
        Input::SetFilters(Filters { harmony: Harmony::Original, just_intonation: true }),
    ];
    let later = [Input::Toot, Input::Toot, Input::Death, Input::ForceHarmony(Some(Harmony::Waltz))];
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
    assert!(replanned[3].intent(3).short_fill, "the drums didn't cue a fill for the checkpoint");
    assert!(replanned[0].intent(3).wah, "the lead didn't cue its wah-wah after the death");
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
            Input::SetFreedom { lead: 1.0, comp: 1.0, bass: 1.0, drums: 1.0, dynamics: 0.7 },
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
            worst_song = (per_block, library::title(m));
        }
        // Debug builds (opt-level 1) are slow; release must stay 100x faster than real time.
        let budget = if cfg!(debug_assertions) { 0.25 } else { 0.01 };
        assert!(per_block < realtime * budget, "{m:?}: {:.0}us per block", per_block * 1e6);
        assert!(worst < realtime, "{m:?}: a block took {:.0}us, longer than it plays", worst * 1e6);
    }
    println!("slowest: {} at {:.1} us/block", worst_song.1, worst_song.0 * 1e6);
}

// --- the waltz ----------------------------------------------------------------------------

const WALTZ: Filters = Filters { harmony: Harmony::Waltz, just_intonation: false };

/// Into the waltz: posted mid-bar, it comes in at the next bar line, which is the first of a
/// pair of waltz bars at the same point of the tune (4/4 bar `k` is waltz bar `2k`): the
/// waltz's own events for that bar, the clock in 3/4 at the waltz's tempo, the bars counting on.
/// Out again: at the next waltz bar line that's also a 4/4 one (after the pair's second bar).
#[test]
fn the_waltz_switches_in_and_out_at_shared_bar_lines() {
    use nat_han_adventures::audio::waltz;
    for m in [Music::Title, Music::World(3), Music::World(5)] {
        let mut e = engine(m, &[]);
        let sh = e.shape().clone();
        let wz = e.waltz_shape().expect("a song with a chart waltzes").clone();
        assert_eq!((wz.bars, wz.bar_beats, wz.bpm), (2 * sh.bars, 3.0, waltz::WALTZ_BPM));
        render_to(&mut e, (sh.bar_starts[2] + sh.bar_starts[3]) / 2);
        e.post(Input::SetFilters(WALTZ));
        let line = sh.bar_starts[3];
        render_to(&mut e, line - 1);
        assert_eq!(e.state().harmony, Harmony::Original);
        assert_eq!(e.beat_clock().beats_per_bar, 4.0);
        // What's committed from the bar line on is the waltz's bar 6, placed from the bar line.
        let committed: Vec<_> =
            e.pending_events().filter(|ev| ev.start >= line).map(|ev| (ev.ch, ev.start - line, ev.end - line, ev.sound)).collect();
        let mut fresh = engine(m, &[Input::SetFilters(WALTZ)]);
        render_to(&mut fresh, wz.bar_starts[6] - 10);
        let w0 = wz.bar_starts[6];
        let want: Vec<_> = fresh
            .pending_events()
            .filter(|ev| ev.start >= w0 && ev.start < wz.bar_starts[7])
            .map(|ev| (ev.ch, ev.start - w0, ev.end - w0, ev.sound))
            .collect();
        assert!(!want.is_empty());
        let mut got = committed.clone();
        got.retain(|x| want.contains(x));
        assert_eq!(got.len(), want.len(), "{m:?}: the waltz's bar 6 ({committed:?} vs {want:?})");
        render_to(&mut e, line);
        let c = e.beat_clock();
        assert_eq!(e.state().harmony, Harmony::Waltz, "{m:?}");
        assert_eq!((c.beats_per_bar, c.bpm, c.position.song_bar, c.position.beat), (3.0, waltz::WALTZ_BPM, 6, 0.0), "{m:?}");
        assert_eq!(c.position.bar, 3, "{m:?}: bars count on");
        assert!((c.position.song_beat - waltz::warp(12.0)).abs() < 1e-9);
        // A beat and a half later: beat 2 of the bar.
        render_to(&mut e, line + (wz.samples_per_beat * 1.5) as u64);
        let c = e.beat_clock();
        assert_eq!((c.position.song_bar, c.beat_index), (6, 1));

        // Out, posted in the first bar of a pair: the pair finishes, 4/4 comes back at bar 4.
        e.post(Input::SetFilters(Filters::default()));
        let back = line + (wz.bar_starts[8] - wz.bar_starts[6]);
        render_to(&mut e, back - 1);
        assert_eq!(e.state().harmony, Harmony::Waltz, "{m:?}: the second bar of the pair is a waltz bar");
        assert_eq!(e.beat_clock().position.song_bar, 7);
        render_to(&mut e, back);
        let c = e.beat_clock();
        assert_eq!(e.state().harmony, Harmony::Original, "{m:?}");
        assert_eq!((c.beats_per_bar, c.position.song_bar, c.position.beat, c.position.bar), (4.0, 4, 0.0, 5), "{m:?}");
        // And it plays on through the loop, in 4/4, counting bars.
        let end = back + sh.len - sh.bar_starts[4] + sh.bar_starts[1];
        render_to(&mut e, end);
        let c = e.beat_clock().position;
        // (The pair was two bars: one more than the 4/4 bar it stood for.)
        assert_eq!((c.pass, c.song_bar, c.bar), (1, 1, sh.bars as u64 + 2), "{m:?}");
    }
}

/// Posted in the second bar of a pair, the way out waits for the next pair; the waltz loops
/// round on its own shape; nothing clicks at a switch.
#[test]
fn the_waltz_loops_and_switches_cleanly() {
    let m = Music::World(2);
    let mut e = engine(m, &[Input::SetFilters(WALTZ)]);
    let wz = e.waltz_shape().unwrap().clone();
    let sh = e.shape().clone();
    // Round the waltz's loop: pass 1, bar 1.
    let mut out = render_to(&mut e, wz.len + wz.bar_starts[1] + 100);
    let c = e.beat_clock();
    assert_eq!((c.position.pass, c.position.song_bar, c.position.bar), (1, 1, wz.bars as u64 + 1));
    assert_eq!(c.loop_beats, wz.beats);
    // In the second bar of a pair (bar 1): out at bar 2 = 4/4 bar 1.
    e.post(Input::SetFilters(Filters::default()));
    let back = wz.len + wz.bar_starts[2];
    out.extend(render_to(&mut e, back));
    assert_eq!(e.state().harmony, Harmony::Original);
    assert_eq!(e.beat_clock().position.song_bar, 1);
    // Back in at 4/4 bar 3.
    let at3 = back + sh.bar_starts[3] - sh.bar_starts[1];
    out.extend(render_to(&mut e, at3 - (sh.samples_per_beat * 2.0) as u64));
    e.post(Input::SetFilters(WALTZ));
    out.extend(render_to(&mut e, at3 + 20_000));
    assert_eq!(e.beat_clock().position.song_bar, 6);
    let step = |i: usize| (out[i].left - out[i - 1].left).abs().max((out[i].right - out[i - 1].right).abs());
    let max_inside = (1..out.len()).map(step).fold(0.0f32, f32::max);
    for s in [back as usize, at3 as usize] {
        let jump = (s - 2..s + 3).map(step).fold(0.0f32, f32::max);
        assert!(jump <= max_inside.max(0.05), "switch at {s}: jump {jump} (max step {max_inside})");
    }
    assert!(out.iter().all(|f| f.left.is_finite() && f.left.abs() <= 1.0));
}

/// The laughing band's medley changes tuning where the 4/4 song's phrases change, waltz or not.
#[test]
fn the_medley_follows_the_tune_through_the_waltz() {
    let laughing = Filters { harmony: Harmony::Original, just_intonation: true };
    let m = Music::World(4);
    let mut straight = engine(m, &[Input::SetFilters(laughing)]);
    let mut w = engine(m, &[Input::SetFilters(Filters { just_intonation: true, ..WALTZ })]);
    let (sh, wz) = (straight.shape().clone(), w.waltz_shape().unwrap().clone());
    let mut seen = std::collections::HashSet::new();
    for k in 0..sh.bars {
        render_to(&mut straight, sh.bar_starts[k] + 5);
        render_to(&mut w, wz.bar_starts[2 * k] + 5);
        let (a, b) = (straight.state().medley_phrase, w.state().medley_phrase);
        assert!(a.is_some());
        assert_eq!(a, b, "bar {k}");
        seen.insert(a);
    }
    assert!(seen.len() >= 2);
}

/// Without a game, the engine's own director hears a jump in threes (three evenly spaced ground
/// jumps) and waltzes at the next bar line, as the game's does; a waltz step from an editor
/// does the same.
#[test]
fn a_self_directed_engine_waltzes_on_a_jump_in_threes() {
    let file = library::load(library::stem(Music::World(1))).unwrap();
    let config = EngineConfig { seed: SEED, self_directed: true, ..EngineConfig::default() };
    let sr = synth::SAMPLE_RATE as u64;
    for steps in [&[Input::Jump { on_ground: true }; 3][..], &[Input::WaltzStep]] {
        let mut e = Engine::with_config(&file, synth::SAMPLE_RATE, config).unwrap();
        e.post(Input::LevelStart);
        render_to(&mut e, sr);
        for (k, i) in steps.iter().enumerate() {
            if k > 0 {
                let to = e.beat_clock().position.sample + sr * 6 / 10;
                render_to(&mut e, to);
            }
            e.post(*i);
        }
        e.fill(&mut []);
        assert_eq!(e.state().filters.harmony, Harmony::Waltz, "{steps:?}");
        let bar = e.shape().bar_starts[1];
        let to = e.beat_clock().position.sample + 2 * bar;
        render_to(&mut e, to);
        assert_eq!(e.state().harmony, Harmony::Waltz, "{steps:?}");
        assert_eq!(e.beat_clock().beats_per_bar, 3.0);
    }
    // Uneven jumps don't.
    let mut e = Engine::with_config(&file, synth::SAMPLE_RATE, config).unwrap();
    for gap in [0.5, 0.9, 0.3] {
        let to = e.beat_clock().position.sample + (gap * sr as f64) as u64;
        render_to(&mut e, to);
        e.post(Input::Jump { on_ground: true });
    }
    e.fill(&mut []);
    assert_eq!(e.state().filters.harmony, Harmony::Original);
}

/// `start_at` (the editor's hot swap and "play from here"): an engine started on a bar line
/// commits that bar exactly as one that played up to it; started mid-bar, it's silent until
/// the next bar line.
#[test]
fn an_engine_can_start_mid_song() {
    let m = Music::Title;
    let mut whole = engine(m, &[]);
    let shape = whole.shape().clone();
    let b = |k: usize| shape.bar_starts[k];
    render_to(&mut whole, b(4) - 10);
    let mut late = engine(m, &[]);
    late.start_at(b(4));
    assert_eq!((late.position().song_bar, late.position().bar), (4, 4));
    late.fill(&mut []);
    let bar4 = |e: &Engine| e.pending_events().filter(|ev| ev.bar == 4).map(|ev| (ev.ch, ev.start, ev.end, ev.sound)).collect::<Vec<_>>();
    assert!(!bar4(&late).is_empty());
    assert_eq!(bar4(&late), bar4(&whole));
    // Mid-bar: silence, then the band comes in on the bar line.
    let mut mid = engine(m, &[]);
    mid.start_at((b(4) + b(5)) / 2);
    let out = render_to(&mut mid, b(6));
    let quiet = (b(5) - (b(4) + b(5)) / 2) as usize;
    assert!(out[..quiet].iter().all(|f| *f == Frame::ZERO));
    assert!(out[quiet..].iter().any(|f| f.left.abs() > 1e-3));
    // Only before the first fill.
    let pos = mid.position();
    mid.start_at(b(1));
    assert_eq!(mid.position(), pos);
}

/// The mixer: a channel's level applies at once; all at 0 is silence.
#[test]
fn the_mixer_mutes_channels() {
    let m = Music::World(2);
    let full = render(&mut engine(m, &[]), 64_000, 512);
    let silent = render(&mut engine(m, &[Input::SetMix([0.0; 4])]), 64_000, 512);
    assert!(silent.iter().all(|f| *f == Frame::ZERO));
    let drums = render(&mut engine(m, &[Input::SetMix([0.0, 0.0, 0.0, 1.0])]), 64_000, 512);
    let energy = |v: &[Frame]| v.iter().map(|f| (f.left * f.left) as f64).sum::<f64>();
    assert!(energy(&drums) > 0.0 && energy(&drums) < energy(&full));
    assert_eq!(render(&mut engine(m, &[Input::SetMix([1.0; 4])]), 64_000, 512), full, "unity is as written");
}
