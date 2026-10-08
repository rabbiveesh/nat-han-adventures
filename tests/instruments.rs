//! Instruments (`[instruments]`, `@i`): the format, the macros' timing frame by frame, the
//! kits, and the built-in defaults playing exactly what every song played before instruments.

use kira::Frame;
use nat_han_adventures::audio::{
    Filters, Harmony, Music,
    live::{Engine, EngineConfig, Input, instrument, library, song::SongFile, syntax},
    synth,
};

const SR: usize = synth::SAMPLE_RATE as usize;

/// FNV-1a over every sample's bits.
fn fingerprint(frames: &[Frame]) -> u64 {
    let mut x: u64 = 0xcbf2_9ce4_8422_2325;
    for f in frames {
        for v in [f.left.to_bits(), f.right.to_bits()] {
            x ^= v as u64;
            x = x.wrapping_mul(0x100_0000_01b3);
        }
    }
    x
}

/// Every song through every filter (seed 7) as rendered before instruments existed, bit for bit.
/// The songs' starter instrument sets keep `default` as each channel's base, so at freedom 0
/// (what the offline renderer plays) they still sound exactly like this.
const BEFORE_INSTRUMENTS: [(Music, Harmony, bool, u64); 80] = [
    (Music::Title, Harmony::Original, false, 0x661560b32f72b73a),
    (Music::Title, Harmony::Original, true, 0x9e4e5552343292f8),
    (Music::Title, Harmony::Coltrane, false, 0x27fdbc64de3c0bc3),
    (Music::Title, Harmony::Coltrane, true, 0x25d61883a5cf68f9),
    (Music::Title, Harmony::Quartal, false, 0x4025c08c7d52c590),
    (Music::Title, Harmony::Quartal, true, 0xf14da2e2405f32d1),
    (Music::Title, Harmony::MelodicMinor, false, 0x6f895803af346b89),
    (Music::Title, Harmony::MelodicMinor, true, 0xf84bf4eaac7f4d53),
    (Music::Title, Harmony::Waltz, false, 0x3b4e8287b1217856),
    (Music::Title, Harmony::Waltz, true, 0x706aeaffe1c7b3d6),
    (Music::World(1), Harmony::Original, false, 0x05875fe72e1cc035),
    (Music::World(1), Harmony::Original, true, 0xb2c78e7b3e14b717),
    (Music::World(1), Harmony::Coltrane, false, 0x312f62b52b7094b7),
    (Music::World(1), Harmony::Coltrane, true, 0x70a7756fc7060487),
    (Music::World(1), Harmony::Quartal, false, 0xe455637aa98a79fb),
    (Music::World(1), Harmony::Quartal, true, 0xa4c432ede59185a1),
    (Music::World(1), Harmony::MelodicMinor, false, 0x156e29455b845ad7),
    (Music::World(1), Harmony::MelodicMinor, true, 0xc789d8e75e37d5b4),
    (Music::World(1), Harmony::Waltz, false, 0x7b1c78747891e4d8),
    (Music::World(1), Harmony::Waltz, true, 0x4757e958662807ba),
    (Music::World(2), Harmony::Original, false, 0xf8651fc190060f94),
    (Music::World(2), Harmony::Original, true, 0x0fe6f2072ae56b5e),
    (Music::World(2), Harmony::Coltrane, false, 0xeed775813c787c94),
    (Music::World(2), Harmony::Coltrane, true, 0x230f8e2808f12590),
    (Music::World(2), Harmony::Quartal, false, 0x2c1a3a3acf5c9520),
    (Music::World(2), Harmony::Quartal, true, 0xbf733bfb4b4cd103),
    (Music::World(2), Harmony::MelodicMinor, false, 0x8eefeebe56aa94f2),
    (Music::World(2), Harmony::MelodicMinor, true, 0xc48a8f670fbd04b6),
    (Music::World(2), Harmony::Waltz, false, 0x867897bf4df2d788),
    (Music::World(2), Harmony::Waltz, true, 0xf7d7cf8a46287793),
    (Music::World(3), Harmony::Original, false, 0x8bcabd60688a4746),
    (Music::World(3), Harmony::Original, true, 0xfd8ab91a329f3acf),
    (Music::World(3), Harmony::Coltrane, false, 0xb6a80700930a4611),
    (Music::World(3), Harmony::Coltrane, true, 0x1205c243e7075cff),
    (Music::World(3), Harmony::Quartal, false, 0x3818dddc691507e1),
    (Music::World(3), Harmony::Quartal, true, 0xaed383c68c57ec98),
    (Music::World(3), Harmony::MelodicMinor, false, 0xbd6d40b2aa5c6a71),
    (Music::World(3), Harmony::MelodicMinor, true, 0x3601f535803a6cfd),
    (Music::World(3), Harmony::Waltz, false, 0x68939ae28ae37b5b),
    (Music::World(3), Harmony::Waltz, true, 0x86b50b799ee881dc),
    (Music::World(4), Harmony::Original, false, 0x0174acfc3d379f92),
    (Music::World(4), Harmony::Original, true, 0xa64ce7d0ca4a0ac0),
    (Music::World(4), Harmony::Coltrane, false, 0x18a14716e8376a75),
    (Music::World(4), Harmony::Coltrane, true, 0xd38d67e7e86996d9),
    (Music::World(4), Harmony::Quartal, false, 0x40c3fb0dd7db67f4),
    (Music::World(4), Harmony::Quartal, true, 0x41c88414395717b8),
    (Music::World(4), Harmony::MelodicMinor, false, 0x921996e4df7a3433),
    (Music::World(4), Harmony::MelodicMinor, true, 0x4ade2c2f122c55f6),
    (Music::World(4), Harmony::Waltz, false, 0x0d7f4f933507cafd),
    (Music::World(4), Harmony::Waltz, true, 0x0fbb3485ddbeff17),
    (Music::World(5), Harmony::Original, false, 0xc55b90f2fcb91766),
    (Music::World(5), Harmony::Original, true, 0x2293889c3879d7f7),
    (Music::World(5), Harmony::Coltrane, false, 0x3b9f646ebe5734f0),
    (Music::World(5), Harmony::Coltrane, true, 0x68fc2e74a2fbf5a0),
    (Music::World(5), Harmony::Quartal, false, 0xe82dfd06fe760088),
    (Music::World(5), Harmony::Quartal, true, 0x68a97678a56b0515),
    (Music::World(5), Harmony::MelodicMinor, false, 0x20c421c14c87744a),
    (Music::World(5), Harmony::MelodicMinor, true, 0x27123e38d49586f0),
    (Music::World(5), Harmony::Waltz, false, 0x70556c676debdece),
    (Music::World(5), Harmony::Waltz, true, 0xe265578848eb6aeb),
    (Music::LevelClear, Harmony::Original, false, 0x4b6381d0395b35c5),
    (Music::LevelClear, Harmony::Original, true, 0xf6de3a9c7df0e62d),
    (Music::LevelClear, Harmony::Coltrane, false, 0x106c5779569e73e2),
    (Music::LevelClear, Harmony::Coltrane, true, 0x426585f11749d1ca),
    (Music::LevelClear, Harmony::Quartal, false, 0x8a63b6735a32b1af),
    (Music::LevelClear, Harmony::Quartal, true, 0x552652a6ca527417),
    (Music::LevelClear, Harmony::MelodicMinor, false, 0xf8ecf5624a07d315),
    (Music::LevelClear, Harmony::MelodicMinor, true, 0xfc3a52c12b3e7032),
    (Music::LevelClear, Harmony::Waltz, false, 0xd007b6408295b16b),
    (Music::LevelClear, Harmony::Waltz, true, 0x52c46d273b221f5c),
    (Music::Victory, Harmony::Original, false, 0xd90c1e6109ca0fe9),
    (Music::Victory, Harmony::Original, true, 0xc97490552a7264fb),
    (Music::Victory, Harmony::Coltrane, false, 0xc334708bea69e4dd),
    (Music::Victory, Harmony::Coltrane, true, 0x3f01b93b11f48e47),
    (Music::Victory, Harmony::Quartal, false, 0x2f6d78a15eea4ba0),
    (Music::Victory, Harmony::Quartal, true, 0x708c9283222ebeb0),
    (Music::Victory, Harmony::MelodicMinor, false, 0xcb1f2f7233d0f9c9),
    (Music::Victory, Harmony::MelodicMinor, true, 0x8f09bbd834d2cd47),
    (Music::Victory, Harmony::Waltz, false, 0xca452dab433f56c1),
    (Music::Victory, Harmony::Waltz, true, 0xdc6788a481148ce7),
];

#[test]
fn the_default_instruments_play_exactly_what_songs_played_before() {
    for (m, harmony, just_intonation, want) in BEFORE_INSTRUMENTS {
        let (_, song) = library::song(m).unwrap();
        let f = Filters { harmony, just_intonation };
        let got = fingerprint(&synth::render_song_with(song, f, 7).unwrap().frames);
        assert_eq!(got, want, "{m:?} {f:?}: 0x{got:016x}");
    }
}

/// The same with the `[instruments]` section gone, or with every channel saying `@i default`.
#[test]
fn default_is_the_built_in_instrument() {
    for stem in ["sweet_georgia_brown", "shave_and_a_haircut"] {
        let text = library::text(stem).unwrap();
        let song = SongFile::parse(text).unwrap();
        let want = fingerprint(&synth::render_song(&song).unwrap().frames);
        let mut bare = song.clone();
        bare.instruments = Default::default();
        bare.instruments_src.clear();
        assert_eq!(fingerprint(&synth::render_song(&bare).unwrap().frames), want, "{stem}: without [instruments]");
        let mut explicit = song.clone();
        for t in &mut explicit.sources {
            if !t.trim().is_empty() {
                *t = format!("@i default {t}");
            }
        }
        let explicit = SongFile::parse(&explicit.to_text()).unwrap();
        assert_eq!(fingerprint(&synth::render_song(&explicit).unwrap().frames), want, "{stem}: @i default");
    }
}

fn song(instruments: &str, p1: &str, p2: &str, noise: &str) -> SongFile {
    let text = format!(
        "[song]\ntitle = t\nbpm = 60\nloop = yes\n[instruments]\n{instruments}\n[pulse1]\n{p1}\n[pulse2]\n{p2}\n[noise]\n{noise}\n"
    );
    SongFile::parse(&text).unwrap_or_else(|e| panic!("{e}\n{text}"))
}

fn render(s: &SongFile, frames: usize, block: usize) -> Vec<Frame> {
    let mut e = Engine::with_config(s, synth::SAMPLE_RATE, EngineConfig::default()).unwrap();
    let mut out = vec![Frame::ZERO; frames];
    for c in out.chunks_mut(block) {
        e.fill(c);
    }
    out
}

/// Sample where 60 Hz frame `k` starts (from a note at sample 0).
fn frame_start(k: usize) -> usize {
    (k * SR).div_ceil(60)
}

/// The middle of frame `k`, trimmed.
fn frame(x: &[f32], k: usize) -> &[f32] {
    let (a, b) = (frame_start(k), frame_start(k + 1));
    &x[a + 40..b - 40]
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0f32, |m, v| m.max(v.abs()))
}

/// Upward zero crossings per second.
fn freq(x: &[f32]) -> f32 {
    let ups: Vec<usize> = (1..x.len()).filter(|&i| x[i - 1] < 0.0 && x[i] >= 0.0).collect();
    if ups.len() < 2 {
        return 0.0;
    }
    (ups.len() - 1) as f32 * SR as f32 / (ups[ups.len() - 1] - ups[0]) as f32
}

/// The fraction of a frame the pulse is high.
fn duty(x: &[f32]) -> f32 {
    let mean = x.iter().sum::<f32>() / x.len() as f32;
    x.iter().filter(|v| **v > mean).count() as f32 / x.len() as f32
}

fn left(f: &[Frame]) -> Vec<f32> {
    f.iter().map(|f| f.left).collect()
}

#[test]
fn volume_macros_step_once_per_frame_and_loop() {
    let s = song("tick : vol 15 5 | 10 15 | duty 2", "@i tick v15 o5 a1", "", "");
    let x = left(&render(&s, SR, 512));
    let p: Vec<f32> = (0..8).map(|k| peak(frame(&x, k))).collect();
    let unit = p[0] / 15.0;
    for (k, want) in [15.0, 5.0, 10.0, 15.0, 10.0, 15.0, 10.0, 15.0].iter().enumerate() {
        assert!((p[k] / unit - want).abs() < 0.3, "frame {k}: {:?}", p.iter().map(|v| v / unit).collect::<Vec<_>>());
    }
}

#[test]
fn duty_and_pitch_macros_step_once_per_frame() {
    let s = song("d : duty 0 1 2 3", "@i d v15 o6 a1", "", "");
    let x = left(&render(&s, SR, 333));
    let duties: Vec<f32> = (0..4).map(|k| duty(frame(&x, k))).collect();
    for (k, want) in [0.125, 0.25, 0.5, 0.75].iter().enumerate() {
        assert!((duties[k] - want).abs() < 0.06, "duty per frame {duties:?}");
    }
    let s = song("p : pitch 0 12 0 -12 0", "@i p v15 o4 a1", "", "");
    let x = left(&render(&s, SR, 333));
    let f: Vec<f32> = (0..6).map(|k| freq(frame(&x, k))).collect();
    for (k, want) in [440.0, 880.0, 440.0, 220.0, 440.0, 440.0].iter().enumerate() {
        assert!((f[k] / want - 1.0).abs() < 0.08, "frame {k}: {f:?}");
    }
}

#[test]
fn vibrato_waits_its_delay_then_wobbles() {
    let s = song("v : vib delay=20 depth=60 speed=5 ramp=1", "", "@i v v15 o4 a1", "");
    let x = left(&render(&s, 2 * SR, 512));
    // Frame-sized windows: steady before the delay, wobbling by tens of cents after.
    let cents = |k: usize| 1200.0 * (freq(frame(&x, k)) / 440.0).log2();
    let before: Vec<f32> = (2..19).map(cents).collect();
    assert!(before.iter().all(|c| c.abs() < 8.0), "{before:?}");
    let after: Vec<f32> = (24..60).map(cents).collect();
    let (lo, hi) = after.iter().fold((0.0f32, 0.0f32), |(a, b), c| (a.min(*c), b.max(*c)));
    assert!(hi > 30.0 && lo < -30.0, "{lo} .. {hi}");
}

#[test]
fn the_release_plays_after_the_note_unless_the_next_note_cuts_it() {
    let s = song("r : vol 15 / 8 8 8 0 | duty 2", "@i r v15 o5 a4 r2.", "", "");
    let x = left(&render(&s, 2 * SR, 512));
    let q = SR; // the note ends here (a quarter at 60 bpm)
    let p = |k: usize| peak(&x[q + frame_start(k) + 40..q + frame_start(k + 1) - 40]);
    let unit = peak(&x[2000..q - 1000]) / 15.0;
    assert!((p(0) / unit - 8.0).abs() < 0.5 && (p(2) / unit - 8.0).abs() < 0.5, "{} {}", p(0) / unit, p(2) / unit);
    assert!(peak(&x[q + frame_start(4) + 400..q + frame_start(8)]) < 1e-4, "it ends after the release");
    // Straight into the next note: it starts on time, at full level (the tail stops there).
    let a = left(&render(&song("r : vol 15 / 8 8 8 0", "@i r v15 o5 a4 b4 r2", "", ""), 2 * SR, 512));
    let b = left(&render(&song("r : vol 15 / 8 8 8 0", "v15 r4 @i r b4 r2", "", ""), 2 * SR, 512));
    for w in [q + 100..q + 600, q + 1500..q + 4000] {
        let (pa, pb) = (peak(&a[w.clone()]), peak(&b[w.clone()]));
        assert!((pa / pb - 1.0).abs() < 0.03, "{w:?}: {pa} vs {pb}");
    }
}

#[test]
fn a_tri_instrument_plays_the_triangle_on_a_pulse() {
    let s = song("flute : tri", "@i flute v15 o4 a1", "", "");
    let x = left(&render(&s, SR / 2, 512));
    let w = &x[4000..12000];
    // The 4-bit triangle: few distinct levels, and no pulse plateaus.
    let mut levels: Vec<i32> = w.iter().map(|v| (v * 1e4).round() as i32).collect();
    levels.sort_unstable();
    levels.dedup();
    assert!(levels.len() <= 40, "{} levels", levels.len());
    assert!((freq(w) - 440.0).abs() < 3.0);
}

#[test]
fn kits_change_the_drums() {
    let kit = "boom : kick pitch=-36 decay=12 | snare noise=short decay=12 | hat decay=4 | crash decay=40";
    let len = |x: &[f32]| x.iter().rposition(|v| v.abs() > 1e-3).unwrap_or(0);
    for (drum, longer) in [("k", true), ("s", true), ("h", true)] {
        let plain = left(&render(&song(kit, "", "", &format!("v15 {drum}1")), SR, 512));
        let boom = left(&render(&song(kit, "", "", &format!("@i boom v15 {drum}1")), SR, 512));
        assert_ne!(plain, boom, "{drum}");
        assert_eq!(len(&boom) > len(&plain), longer, "{drum}: {} vs {}", len(&boom), len(&plain));
    }
    // The crash rings far longer than an open hat.
    let crash = left(&render(&song("", "", "", "v15 x1"), SR, 512));
    let ohat = left(&render(&song("", "", "", "v15 H1"), SR, 512));
    assert!(len(&crash) > 2 * len(&ohat), "{} vs {}", len(&crash), len(&ohat));
}

#[test]
fn instruments_can_change_mid_line_and_survive_ties() {
    let s = song("a : vol 15\nb : vol 4", "@i a v15 o4 a4 @i b a4 @i a a4& a4", "", "");
    let ev = &s.tracks[0].events;
    assert_eq!(ev.iter().map(|e| e.inst).collect::<Vec<_>>(), [1, 2, 1]);
    assert_eq!(ev[2].dur, 2.0, "a tie on one instrument merges");
    let x = left(&render(&s, 3 * SR, 512));
    assert!(peak(&x[SR + 4000..SR + 12000]) < 0.4 * peak(&x[4000..12000]));
}

#[test]
fn the_format_rejects_mistakes_with_their_line() {
    let base = "[song]\ntitle = t\nbpm = 60\n[instruments]\nlead : vol 15 12\nkit : kick decay=4\n[pulse1]\nc1 |\n[noise]\nk1 |\n";
    assert!(SongFile::parse(base).is_ok());
    let e = |from: &str, to: &str| SongFile::parse(&base.replacen(from, to, 1)).unwrap_err();
    let x = e("vol 15 12", "vol 15 99");
    assert_eq!(x.line, 5, "{x}");
    assert!(x.msg.contains("[instruments]") && x.msg.contains("lead"), "{x}");
    let x = e("c1 |", "@i brass c1 |");
    assert_eq!(x.line, 8, "{x}");
    assert!(x.msg.contains("unknown instrument `brass`") && x.msg.contains("lead"), "{x}");
    assert!(e("c1 |", "@i kit c1 |").msg.contains("kit"));
    assert!(e("k1 |", "@i lead k1 |").msg.contains("kits"));
    assert!(e("c1 |", "@i c1 |").msg.contains("unknown instrument `c1`"));
    assert!(e("c1 |", "c1 @i").msg.contains("needs an instrument name"));
    assert!(e("kit : kick decay=4", "kit : kick decay=4\npulse1 = kit").msg.contains("noise"));
    // A palette round-trips with the text.
    let s = SongFile::parse(&base.replacen("[pulse1]", "pulse1 = default lead\n[pulse1]", 1)).unwrap();
    assert_eq!(s.instruments.palette(0), [0, 1]);
    assert_eq!(SongFile::parse(&s.to_text()).unwrap(), s);
}

/// Every instrument keyword, kit drum and parameter is in the cheat sheet.
#[test]
fn the_cheat_sheet_covers_instruments() {
    let rows: Vec<&str> = syntax::CHEAT_SHEET.iter().flat_map(|(_, r)| r.iter().map(|(s, _)| *s)).collect();
    let words = |w: &str| rows.iter().any(|s| s.split([' ', '|', ':']).any(|x| x == w || x.starts_with(w)));
    for k in instrument::TONE_KEYS.iter().chain(&instrument::KIT_DRUMS).chain(&instrument::PARAMS) {
        assert!(words(k), "`{k}` isn't in the cheat sheet");
    }
    for k in ["@i", "default", "|", "/", "pulse1 =", "[instruments]", "x"] {
        assert!(rows.iter().any(|s| s.contains(k)), "`{k}` isn't in the cheat sheet");
    }
}

/// Instruments and effects render the same whatever the block size (macros are counted in
/// samples from the note-on).
#[test]
fn macros_dont_depend_on_the_block_size() {
    let s = song(
        "a : vol 15 12 | 9 6 9 / 4 2 0 | duty 0 1 2 | pitch +0.5 0 | vib delay=3 depth=30 speed=7\nk : kick pitch=-30 | snare noise=short",
        "@i a v15 o4 [a8 b8 > c4 < r4 {c e g}4 |]4",
        "@i a v9 o3 [c2 d2 |]4",
        "@i k v12 [k8 s8 h8 H8 x4 s4 |]4",
    );
    let reference = render(&s, 4 * 4 * SR, 4096);
    for block in [1, 61, 512, 1000] {
        assert_eq!(render(&s, 4 * 4 * SR, block), reference, "block {block}");
    }
}

/// A free band switching instruments on the fly still renders the same whatever the block.
#[test]
fn songs_with_instruments_dont_depend_on_the_block_size() {
    let file = library::load("tiger_rag").unwrap();
    let run = |block: usize| {
        let mut e = Engine::with_config(&file, synth::SAMPLE_RATE, EngineConfig { seed: 3, ..EngineConfig::default() }).unwrap();
        e.post(Input::SetFreedom { lead: 0.8, comp: 0.8, bass: 0.8, drums: 0.8, dynamics: 0.5 });
        let mut out = vec![Frame::ZERO; 40 * SR];
        for c in out.chunks_mut(block) {
            e.fill(c);
        }
        out
    };
    let reference = run(4096);
    for block in [37, 512] {
        assert!(run(block) == reference, "block {block}");
    }
}
