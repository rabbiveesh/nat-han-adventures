//! Arranging (`audio::live::chorus`): each chorus has its parts (stop-time hits on ONE, breaks
//! with the band out, the comp out for strolling, the bass in two, the soli a third under the
//! tune, the shout's stabs with the lead, a key-up a half step up), the band arranges passes
//! itself above the game's calm end, a level's tune starts with an intro, and a song ends with
//! an ending (the jingle with a Basie ending).

use kira::Frame;
use nat_han_adventures::audio::{
    Music,
    live::{
        Engine, EngineConfig, Input,
        band::HitKind,
        chorus::{self, Call, Chorus, EndKind, EndStep, IntroKind},
        engine::CommittedBar,
        library,
        ornament::{Harm, Orn},
        song::SongFile,
        voice::{NoteEvent, Sound},
    },
    mml::Drum,
    synth,
};

struct Bar {
    c: CommittedBar,
    ev: Vec<NoteEvent>,
}

impl Bar {
    fn ch(&self, ch: u8) -> impl Iterator<Item = &NoteEvent> {
        self.ev.iter().filter(move |e| e.ch == ch)
    }

    /// An event's start, in beats from the bar line (4/4).
    fn rel(&self, e: &NoteEvent) -> f64 {
        e.beat - self.c.slot.song_bar as f64 * 4.0
    }

    fn notes(&self, ch: u8) -> Vec<u8> {
        self.ch(ch).flat_map(|e| e.sound.notes().iter().copied()).collect()
    }
}

fn freedom(f: f32) -> Input {
    Input::SetFreedom { lead: f, comp: f, bass: f, drums: f, dynamics: 0.0 }
}

fn force(chorus: Chorus) -> Input {
    Input::ForceChorus(Some(Call { chorus, key_up: false }))
}

/// Play `song` (with `config`) for up to `bars` bars, `at(k)` posted as bar `k` is about to be
/// decided; every bar as it's committed, until the song ends.
fn play_with(song: &SongFile, config: EngineConfig, setup: &[Input], at: impl Fn(u64) -> Vec<Input>, bars: u64) -> (Engine, Vec<Bar>) {
    let mut e = Engine::with_config(song, synth::SAMPLE_RATE, config).unwrap();
    for i in setup {
        e.post(*i);
    }
    let mut out = Vec::new();
    let mut buf = [Frame::ZERO; 512];
    let mut guard = 0;
    while (out.len() as u64) < bars && guard < 200_000 && !e.finished() {
        guard += 1;
        e.fill(&mut buf);
        let s = e.state();
        let have = out.len() as u64;
        for c in s.upcoming.iter().filter(|c| c.slot.index >= have) {
            let ev = e.pending_events().filter(|x| x.bar == c.slot.index).copied().collect();
            out.push(Bar { c: *c, ev });
            for i in at(c.slot.index + 1) {
                e.post(i);
            }
        }
    }
    (e, out)
}

fn play(song: &SongFile, seed: u64, setup: &[Input], bars: u64) -> Vec<Bar> {
    play_with(song, EngineConfig { seed, ..EngineConfig::default() }, setup, |_| vec![], bars).1
}

fn song(m: Music) -> &'static SongFile {
    library::song(m).unwrap().1
}

const LOOPING: [Music; 7] = [Music::Title, Music::World(1), Music::World(2), Music::World(3), Music::World(4), Music::World(5), Music::Victory];

/// Below the arranger's dial every pass is the head; above it the band arranges whole passes
/// itself, the first a head (in two or not), deterministically.
#[test]
fn the_band_arranges_whole_passes() {
    let mut kinds = std::collections::HashSet::new();
    for (k, m) in LOOPING.into_iter().enumerate() {
        let s = song(m);
        let bars = 4 * s.bars() as u64;
        assert!(play(s, k as u64, &[freedom(0.2)], bars).iter().all(|b| b.c.band.chorus == Chorus::Head && !b.c.band.arranged()));
        for f in [0.35, 0.7] {
            let a = play(s, k as u64, &[freedom(f)], bars);
            assert!(matches!(a[0].c.band.chorus, Chorus::Head | Chorus::TwoFeel));
            // One chorus per pass.
            for w in a.windows(2) {
                if w[0].c.slot.pass == w[1].c.slot.pass {
                    assert_eq!(w[0].c.band.chorus, w[1].c.band.chorus, "{m:?} at {f}: the chorus changed mid-pass");
                }
            }
            kinds.extend(a.iter().map(|b| b.c.band.chorus));
            let b = play(s, k as u64, &[freedom(f)], bars);
            assert!(a.iter().zip(&b).all(|(x, y)| x.c.band == y.c.band), "{m:?}: not deterministic");
        }
    }
    assert!(kinds.len() >= 6, "only {kinds:?}");
}

/// Stop-time: the band hits on ONE (and the "and" of 2 every other bar) in all but the last
/// two bars of a section, then lays out: no comp or bass after the hits, the drums only on the
/// hats on 2 and 4; the lead plays on. Even at freedom 0, forced.
#[test]
fn stop_time_hits_on_one_and_lays_out() {
    for f in [0.0, 0.5] {
        for m in [Music::World(1), Music::World(3)] {
            let bars = play(song(m), 3, &[freedom(f), force(Chorus::StopTime)], 32);
            let mut stops = 0;
            // (From the second bar: the first started before the harness saw it.)
            for b in &bars[1..] {
                let stop = chorus::stop_bar(b.c.slot.song_bar, song(m).bars());
                assert_eq!(b.c.band.hit_kind == HitKind::Stop, stop, "{m:?} bar {}", b.c.slot.song_bar);
                if !stop {
                    continue;
                }
                stops += 1;
                let last = b.c.band.hit_beats().last().unwrap();
                assert!(b.c.band.hits & 1 == 1, "a hit on ONE");
                for ch in [1, 2] {
                    assert!(b.ch(ch).all(|e| b.rel(e) <= last + 1e-6), "{m:?} at {f}: ch {ch} plays through the stop");
                    assert!(b.ch(ch).any(|e| b.rel(e).abs() < 1e-6), "{m:?}: ch {ch} misses the hit");
                }
                assert!(b.ch(3).filter(|e| b.rel(e) > last + 0.3).all(|e| e.sound == Sound::Drum(Drum::ClosedHat)));
                assert!(b.ch(3).any(|e| b.rel(e).abs() < 1e-6 && e.sound == Sound::Drum(Drum::Kick)));
            }
            assert!(stops >= 19, "{m:?}: {stops} stop bars");
            assert!(bars.iter().filter(|b| b.ch(0).next().is_some()).count() >= 24, "{m:?}: the lead stopped too");
        }
    }
}

/// Breaks: the band stops on the ONE of a section's second-last bar and is out for the last;
/// the lead plays both (and runs into the next section), and the band crashes back in.
#[test]
fn breaks_leave_the_lead_alone() {
    for m in [Music::World(2), Music::World(4)] {
        let bars = play(song(m), 5, &[freedom(0.5), force(Chorus::Breaks)], 34);
        let mut seen = 0;
        for w in bars.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            if !b.c.band.tacet {
                continue;
            }
            seen += 1;
            assert!(a.c.band.hit_kind == HitKind::Stop && a.c.band.solo_break, "{m:?}: no stop before the break");
            assert!(b.ch(1).next().is_none() && b.ch(2).next().is_none() && b.ch(3).next().is_none(), "{m:?}: the band played in the break");
            assert!(b.ch(0).count() >= 4 && b.c.orns[0].has(Orn::Break), "{m:?}: no break from the lead ({})", b.c.orns[0].names());
        }
        assert!(seen >= 3, "{m:?}: {seen} breaks");
        for w in bars.windows(2) {
            if w[0].c.band.tacet && w[1].c.slot.index == w[0].c.slot.index + 1 {
                assert!(w[1].c.band.crash, "{m:?}: no crash back in");
            }
        }
    }
}

/// Strolling: no comp; two-feel: the bass on 1 and 3.
#[test]
fn strolling_and_two_feel() {
    for m in [Music::World(1), Music::World(5)] {
        let bars = play(song(m), 2, &[freedom(0.5), force(Chorus::Strolling)], 16);
        assert!(bars.iter().all(|b| b.ch(1).next().is_none()), "{m:?}: the comp strolled in");
        assert!(bars.iter().all(|b| b.c.orns[0].has(Orn::Solo) || b.c.orns[0].has(Orn::WahWah)), "{m:?}: the lead should solo");
        let bars = play(song(m), 2, &[freedom(0.5), force(Chorus::TwoFeel)], 16);
        for b in &bars {
            assert!(b.c.orns[2].has(Orn::TwoFeel), "{m:?}: {}", b.c.orns[2].names());
            let fresh: Vec<f64> = b.ch(2).filter(|e| !e.tie).map(|e| b.rel(e)).filter(|&x| x < 3.9).collect();
            assert!(fresh.iter().all(|x| (x - 0.0).abs() < 1e-6 || (x - 2.0).abs() < 1e-6 || b.c.band.hits != 0), "{m:?}: the bass isn't in two: {fresh:?}");
        }
    }
}

/// Soli: the comp plays the lead's line, note for note, a third to a sixth below.
#[test]
fn soli_harmonizes_the_tune() {
    for m in [Music::World(1), Music::World(3), Music::World(4)] {
        let bars = play(song(m), 4, &[freedom(0.5), force(Chorus::Soli)], 16);
        let (mut paired, mut total) = (0, 0);
        for b in &bars {
            let lead: Vec<&NoteEvent> = b.ch(0).filter(|e| matches!(e.sound, Sound::Note(_))).collect();
            for l in &lead {
                let Sound::Note(n) = l.sound else { continue };
                total += 1;
                if let Some(c) = b.ch(1).find(|c| c.start == l.start) {
                    let Sound::Note(x) = c.sound else { panic!("{m:?}: a soli chord") };
                    let iv = (n as i32 - x as i32).rem_euclid(12);
                    assert!((3..=9).contains(&iv) || iv == 0, "{m:?}: interval {iv}");
                    paired += 1;
                }
            }
            assert!(b.c.orns[1].has(Orn::Soli) || b.c.band.trade != Default::default() || b.ch(0).next().is_none());
        }
        assert!(paired * 10 >= total * 8, "{m:?}: only {paired}/{total} lead notes harmonized");
    }
}

/// Shout: louder, the comp stabs with the lead's attacks; a key-up is the same chorus a half
/// step up (the drums as they were).
#[test]
fn shout_and_key_up() {
    for m in [Music::World(2), Music::World(5)] {
        let shout = play(song(m), 6, &[freedom(0.5), force(Chorus::Shout)], 12);
        let up = play(song(m), 6, &[freedom(0.5), Input::ForceChorus(Some(Call { chorus: Chorus::Shout, key_up: true }))], 12);
        let mut stabs = 0;
        for (a, b) in shout.iter().zip(&up) {
            assert!(a.c.band.swell > 0.0 && a.c.orns[1].has(Orn::ShoutStabs));
            stabs += a.ch(1).filter(|c| a.ch(0).any(|l| l.start == c.start)).count();
            assert_eq!(a.ev.len(), b.ev.len());
            for (x, y) in a.ev.iter().zip(&b.ev) {
                assert_eq!((x.ch, x.start, x.end), (y.ch, y.start, y.end));
                if x.ch == 3 {
                    assert_eq!(x.sound, y.sound);
                } else {
                    for (p, q) in x.sound.notes().iter().zip(y.sound.notes()) {
                        let d = *q as i32 - *p as i32;
                        assert!(d == 1 || d == -11, "{m:?}: key-up moved {p} to {q}");
                    }
                }
            }
        }
        assert!(stabs >= 12, "{m:?}: {stabs} stabs with the lead");
    }
}

/// Riffs: the comp plays single-note riffs behind the lead, every note a chord tone.
#[test]
fn riffs_are_chord_tones() {
    for m in [Music::World(1), Music::World(4)] {
        let chart = song(m).chart.as_ref().unwrap();
        let bars = play(song(m), 8, &[freedom(0.5), force(Chorus::Riffs)], 16);
        let mut notes = 0;
        for b in bars.iter().filter(|b| b.c.orns[1].has(Orn::Riff)) {
            for e in b.ch(1) {
                let Sound::Note(n) = e.sound else { panic!("{m:?}: a chord in a riff") };
                // (The 8ths swing: the chord where the 8th was written.)
                let h = Harm::new(chart.at(e.beat - 0.2));
                let h2 = Harm::new(chart.at(e.beat));
                assert!(h.is_chord_tone(n) || h2.is_chord_tone(n), "{m:?}: {n} over {:?}", h.chord);
                notes += 1;
            }
        }
        assert!(notes >= 16, "{m:?}: {notes} riff notes");
    }
}

/// A level's tune starts with a four-bar intro (from the song's last bars, the head still
/// pass 0): the lead out, then a pickup; a pedal intro's bass on the dominant. None at freedom
/// 0, none without the game's setting.
#[test]
fn levels_start_with_an_intro() {
    let config = |seed| EngineConfig { seed, intro: true, ..EngineConfig::default() };
    let mut kinds = std::collections::HashSet::new();
    for (k, m) in LOOPING.into_iter().enumerate() {
        let s = song(m);
        let n = s.bars();
        for seed in [k as u64, 40 + k as u64] {
            let (_, bars) = play_with(s, config(seed), &[freedom(0.35)], |_| vec![], 8);
            let intro: Vec<_> = bars[..4].iter().map(|b| b.c.band.intro.expect("an intro")).collect();
            kinds.insert(intro[0].kind);
            assert_eq!(bars[..4].iter().map(|b| b.c.slot.song_bar).collect::<Vec<_>>(), [n - 4, n - 3, n - 2, n - 1]);
            assert!(bars[..3].iter().all(|b| b.ch(0).next().is_none()), "{m:?}: the lead played in the intro");
            assert!(bars[3].ch(0).count() >= 3, "{m:?}: no pickup");
            assert!(bars[3].c.band.fill != Default::default(), "{m:?}: no fill into the head");
            assert_eq!((bars[4].c.slot.pass, bars[4].c.slot.song_bar), (0, 0));
            assert!(bars[4].c.band.intro.is_none() && bars[4].c.band.crash);
            if intro[0].kind == IntroKind::Pedal {
                let dom = (s.key + 7) % 12;
                // (The last bar may approach the head.)
                assert!(bars[..3].iter().all(|b| b.ch(2).all(|e| e.sound.notes()[0] % 12 == dom)), "{m:?}: the pedal moved");
                assert_eq!(bars[3].notes(2)[0] % 12, dom);
            }
        }
        let (_, plain) = play_with(s, config(1), &[freedom(0.0)], |_| vec![], 4);
        assert!(plain.iter().all(|b| b.c.band.intro.is_none()) && plain[0].c.slot.song_bar == 0);
        assert!(play(s, 1, &[freedom(0.35)], 4)[0].c.band.intro.is_none());
    }
    assert_eq!(kinds.len(), 2, "{kinds:?}");
}

/// End: the band plays an ending from the next bar on, its last chord the tonic in every
/// voice, and the engine finishes. Freedom 0: just the chord.
#[test]
fn songs_end_with_an_ending() {
    let mut kinds = std::collections::HashSet::new();
    for (k, m) in LOOPING.into_iter().enumerate() {
        let s = song(m);
        for (seed, f) in [(k as u64, 0.0), (k as u64, 0.5), (10 + k as u64, 0.5), (20 + k as u64, 0.5)] {
            let cfg = EngineConfig { seed, ..EngineConfig::default() };
            let (e, bars) = play_with(s, cfg, &[freedom(f)], |k| if k == 6 { vec![Input::End] } else { vec![] }, 40);
            assert!(e.finished(), "{m:?}: didn't finish");
            let ending: Vec<_> = bars.iter().filter_map(|b| b.c.band.ending).collect();
            assert_eq!(ending.len(), ending[0].n as usize, "{m:?}: a cut-short ending");
            assert!(bars.iter().position(|b| b.c.band.ending.is_some()).unwrap() >= 6);
            let kind = ending[0].kind;
            kinds.insert(kind);
            assert_eq!(kind == EndKind::Plain, f == 0.0);
            let last = bars.last().unwrap();
            assert_eq!(last.c.band.ending.unwrap().step(), EndStep::Final);
            let tonic = Harm::new(last.c.band.subs[0].unwrap().chord);
            for ch in [0, 1, 2] {
                let notes = last.notes(ch);
                assert!(!notes.is_empty() && notes.iter().all(|&x| tonic.is_chord_tone(x) || ch == 1), "{m:?} {kind:?}: ch {ch} {notes:?}");
            }
            assert_eq!(last.notes(2)[0] % 12, s.key % 12, "{m:?}: the bass isn't on the root");
            if kind == EndKind::Basie {
                let plinks = &bars[bars.len() - 2];
                assert_eq!(plinks.ch(1).count(), 3);
                let others: Vec<_> = plinks.ev.iter().filter(|e| e.ch != 1).map(|e| (e.ch, plinks.rel(e), e.sound)).collect();
                assert!(others.is_empty(), "{m:?} at {f}: {others:?} with the plinks ({:?})", plinks.c.orns.map(|o| o.names()));
            }
        }
    }
    assert_eq!(kinds.len(), 4, "{kinds:?}");
}

/// The level-clear jingle: a Basie ending after "two bits" (the plinks, the band's chord) when
/// the band's loose; at freedom 0 as written, nothing after.
#[test]
fn the_jingle_gets_a_basie_ending() {
    let s = song(Music::LevelClear);
    let (e, bars) = play_with(s, EngineConfig::default(), &[freedom(0.35)], |_| vec![], 20);
    assert!(e.finished());
    assert_eq!(bars.len(), s.bars() + 2);
    let steps: Vec<_> = bars.iter().filter_map(|b| b.c.band.ending.map(|x| (x.kind, x.step()))).collect();
    assert_eq!(steps, [(EndKind::Basie, EndStep::Plinks), (EndKind::Basie, EndStep::Final)]);
    let (e, bars) = play_with(s, EngineConfig::default(), &[freedom(0.0)], |_| vec![], 20);
    assert!(e.finished() && bars.len() == s.bars() && bars.iter().all(|b| !b.c.band.arranged()));
}
