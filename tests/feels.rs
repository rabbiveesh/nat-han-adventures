//! The band's feels (`audio::live::feel`): chosen by the band itself at high freedom
//! (deterministically, more often the freer it is, never at the game's neutral 0.35 or below,
//! never in the waltz), announced with a fill and a crash at a bar line, each with its rhythm
//! (the bossa clave, the samba's surdo, rock's backbeat, funk's 16ths), the harmony as charted
//! (coloured, or simplified to power chords in rock), through every filter and tuning.

use kira::Frame;
use nat_han_adventures::audio::{
    Filters, Harmony, Music,
    live::{
        Engine, EngineConfig, Input,
        band::Fill,
        engine::CommittedBar,
        feel::{self, Feel},
        library,
        ornament::{Harm, Orn},
        song::SongFile,
        voice::{NoteEvent, Sound},
    },
    mml::Drum,
    synth,
    tuning::Tuning,
};

struct Bar {
    c: CommittedBar,
    ev: Vec<NoteEvent>,
}

impl Bar {
    fn ch(&self, ch: u8) -> impl Iterator<Item = &NoteEvent> {
        self.ev.iter().filter(move |e| e.ch == ch)
    }

    /// An event's beat from its bar line.
    fn rel(&self, e: &NoteEvent) -> f64 {
        e.beat - self.c.slot.song_bar as f64 * if self.c.harmony == Harmony::Waltz { 3.0 } else { 4.0 }
    }

    /// Drum hits of `d` (beats from the bar line).
    fn drums(&self, d: Drum) -> Vec<f64> {
        self.ch(3).filter(|e| e.sound == Sound::Drum(d)).map(|e| self.rel(e)).collect()
    }
}

fn freedom(f: f32) -> Input {
    Input::SetFreedom { lead: f, comp: f, bass: f, drums: f, dynamics: 0.0 }
}

fn play(song: &SongFile, seed: u64, setup: &[Input], bars: u64) -> (Engine, Vec<Bar>) {
    let mut e = Engine::with_config(song, synth::SAMPLE_RATE, EngineConfig { seed, ..EngineConfig::default() }).unwrap();
    for i in setup {
        e.post(*i);
    }
    let mut out = Vec::new();
    let mut buf = [Frame::ZERO; 512];
    while (out.len() as u64) < bars {
        e.fill(&mut buf);
        let s = e.state();
        let have = out.len() as u64;
        for c in s.upcoming.iter().filter(|c| c.slot.index >= have) {
            let ev = e.pending_events().filter(|x| x.bar == c.slot.index).copied().collect();
            out.push(Bar { c: *c, ev });
        }
    }
    (e, out)
}

fn song(m: Music) -> &'static SongFile {
    library::song(m).unwrap().1
}

const LOOPING: [Music; 7] = [Music::Title, Music::World(1), Music::World(2), Music::World(3), Music::World(4), Music::World(5), Music::Victory];

fn feels_of(bars: &[Bar]) -> Vec<Feel> {
    bars.iter().map(|b| b.c.band.feel).collect()
}

/// The same seed and dial play the same feels in the same bars; the freer the band, the more
/// bars in a feel; none at all up to 0.35 (the game's neutral), so the music there is as before.
#[test]
fn feels_are_deterministic_and_follow_the_dial() {
    let mut share = Vec::new();
    for f in [0.35, 0.6, 0.9] {
        let mut feel_bars = 0;
        let mut all = 0;
        for (k, m) in LOOPING.into_iter().enumerate() {
            let (_, a) = play(song(m), k as u64, &[freedom(f)], 100);
            let (_, b) = play(song(m), k as u64, &[freedom(f)], 100);
            assert_eq!(feels_of(&a), feels_of(&b), "{m:?} at {f}");
            // Never in the first section.
            assert!(a[..8].iter().all(|b| b.c.band.feel == Feel::Swing), "{m:?}");
            feel_bars += a.iter().filter(|b| b.c.band.feel != Feel::Swing).count();
            all += a.len();
            if f <= 0.35 {
                assert!(a.iter().all(|b| b.c.band.feel == Feel::Swing && !b.c.orns.iter().any(|o| o.has(Orn::Clave) || o.has(Orn::Backbeat))), "{m:?}");
            }
        }
        share.push(feel_bars as f64 / all as f64);
    }
    println!("share of bars in a feel at 0.35 / 0.6 / 0.9: {share:?}");
    assert_eq!(share[0], 0.0);
    // (The arranged choruses share the loose end of the dial: no feels in a stop-time, soli,
    // breaks, two-feel or shout chorus.)
    assert!(share[1] > 0.03 && share[1] < share[2] && share[2] > 0.1, "{share:?}");
}

/// A feel changes at a bar line, announced: a full fill in the bar before (into it and out of
/// it), a crash on the first bar of the new one; it lasts whole sections (8 or 16 bars).
#[test]
fn transitions_land_on_bar_lines_with_a_fill() {
    let mut seen = 0;
    for (k, m) in LOOPING.into_iter().enumerate() {
        let (_, bars) = play(song(m), 20 + k as u64, &[freedom(0.9)], 120);
        for w in bars.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            if a.c.band.feel == b.c.band.feel {
                continue;
            }
            seen += 1;
            assert!(matches!(a.c.band.fill, Fill::Full | Fill::PressRoll), "{m:?} bar {}: no fill into {:?}", b.c.slot.index, b.c.band.feel);
            assert!(a.c.orns[3].has(Orn::Fill) || a.c.orns[3].has(Orn::PressRoll) || a.c.orns[3].has(Orn::Trade), "{m:?}: the drums didn't fill");
            assert!(b.c.band.crash && b.ch(3).any(|e| e.sound == Sound::Drum(Drum::Crash) && e.start == b.c.slot.start), "{m:?} bar {}: no crash", b.c.slot.index);
            // At a section's bar line, from the first bar of the new feel's groove.
            assert_eq!(b.c.slot.song_bar % 8, 0, "{m:?}: a feel changed mid-section");
            if b.c.band.feel != Feel::Swing {
                assert_eq!(b.c.band.feel_since, b.c.slot.index);
                let groove = [Orn::Clave, Orn::Batucada, Orn::Backbeat, Orn::FunkGroove][b.c.band.feel as usize - 1];
                assert!(b.c.orns[3].has(groove), "{m:?}: {}", b.c.orns[3].names());
            }
        }
    }
    assert!(seen >= 10, "only {seen} transitions");
}

/// Forced feels: every bar in the feel (from the first committed), whatever the freedom.
fn forced(m: Music, f: Feel, setup: &[Input], bars: u64) -> Vec<Bar> {
    let mut s = vec![Input::ForceFeel(Some(f))];
    s.extend_from_slice(setup);
    let (_, b) = play(song(m), 3, &s, bars);
    assert!(b.iter().all(|b| b.c.band.feel == f), "{m:?}: not all {f:?}");
    b
}

/// The plain bars of a forced feel (no fill, crash, hits or trade in the way).
fn plain(bars: &[Bar]) -> impl Iterator<Item = &Bar> {
    bars.iter().skip(1).filter(|b| b.c.band.fill == Fill::None && !b.c.band.crash && b.c.band.hits == 0 && b.c.slot.song_bar % 8 != 0)
}

/// The bossa clave on the rim: the 3-2 two-bar pattern, or (a fast tune: relaxed, half-time)
/// spread over four bars, the shaker on quarters.
#[test]
fn bossa_plays_the_clave_on_the_rim() {
    for m in [Music::Title, Music::World(1), Music::World(3)] {
        let bars = forced(m, Feel::Bossa, &[freedom(0.5)], 24);
        let half = feel::fast(song(m).bpm);
        assert_eq!(half, m != Music::World(1), "{m:?}");
        let mut n = 0;
        for b in plain(&bars) {
            let (clave, k) = feel::two_bar(feel::BOSSA_CLAVE, b.c.slot.index, b.c.band.feel_since, half);
            assert_eq!(b.drums(Drum::Snare), clave[..k], "{m:?} bar {}", b.c.slot.index);
            let j = b.c.slot.index - b.c.band.feel_since;
            let want: &[f64] = match (half, j % 2, (j / 2) % 2) {
                (false, 0, _) => &[0.0, 1.5, 3.0],
                (false, _, _) => &[1.0, 2.5],
                (true, 0, 0) => &[0.0, 3.0],
                (true, _, 0) => &[2.0],
                (true, 0, _) => &[2.0],
                (true, _, _) => &[1.0],
            };
            assert_eq!(clave[..k], *want, "{m:?}");
            // Straight 8ths on the shaker (quarters, half-time).
            let step = if half { 1.0 } else { 0.5 };
            assert_eq!(b.drums(Drum::ClosedHat), (0..(4.0 / step) as usize).map(|k| k as f64 * step).collect::<Vec<_>>());
            assert!(b.c.orns[1].has(Orn::BossaComp) && b.c.orns[2].has(Orn::BossaBass));
            // The bass: root on 1 (or tied over from the anticipation).
            assert!(b.ch(2).any(|e| b.rel(e) == 0.0), "{m:?}");
            n += 1;
        }
        assert!(n >= 8);
    }
}

#[test]
fn samba_hits_the_surdo_on_two() {
    for m in [Music::Title, Music::World(1), Music::World(3)] {
        let bars = forced(m, Feel::Samba, &[freedom(0.5)], 24);
        for b in plain(&bars) {
            let at = |x: f64| b.ch(2).find(|e| (b.rel(e) - x).abs() < 1e-6).map(|e| e.volume);
            let (one, two, three, four) = (at(0.0).unwrap(), at(1.0).unwrap(), at(2.0).unwrap(), at(3.0).unwrap());
            assert!(two > one && four > three, "{m:?} bar {}: surdo {one} {two} {three} {four}", b.c.slot.index);
            // The ganzá: every 16th (every 8th in a fast tune).
            assert_eq!(b.drums(Drum::ClosedHat).len(), if feel::fast(song(m).bpm) { 8 } else { 16 });
            assert!(b.c.orns[1].has(Orn::PartidoAlto) && b.c.orns[2].has(Orn::Surdo));
        }
    }
}

#[test]
fn rock_has_a_backbeat() {
    for m in [Music::Title, Music::World(1), Music::World(3)] {
        let bars = forced(m, Feel::Rock, &[freedom(0.5)], 24);
        for b in plain(&bars) {
            assert_eq!(b.drums(Drum::Snare), [1.0, 3.0], "{m:?} bar {}", b.c.slot.index);
            let kicks = b.drums(Drum::Kick);
            assert!(kicks.contains(&0.0) && kicks.contains(&2.0), "{m:?}: kicks {kicks:?}");
            assert_eq!(b.drums(Drum::ClosedHat), (0..8).map(|k| k as f64 * 0.5).collect::<Vec<_>>());
            // The bass pumps 8ths.
            assert_eq!(b.ch(2).count(), 8, "{m:?}");
            let snare = b.ch(3).find(|e| e.sound == Sound::Drum(Drum::Snare)).unwrap();
            let hat = b.ch(3).find(|e| e.sound == Sound::Drum(Drum::ClosedHat)).unwrap();
            assert!(snare.volume > hat.volume);
        }
    }
}

/// Funk: a two-bar vamp on the 16th grid, the same all through the feel; the bass locked to
/// the kick, the one hard; one ghost note; space.
#[test]
fn funk_vamps_on_16ths_and_the_bass_locks_to_the_kick() {
    for m in [Music::Title, Music::World(1), Music::World(3)] {
        let bars = forced(m, Feel::Funk, &[freedom(0.5)], 24);
        let mut kicks = [None, None];
        for b in plain(&bars) {
            let hats: Vec<f64> = b.ch(3).filter(|e| matches!(e.sound, Sound::Drum(Drum::ClosedHat | Drum::OpenHat))).map(|e| b.rel(e)).collect();
            assert_eq!(hats, (0..8).map(|k| k as f64 * 0.5).collect::<Vec<_>>(), "{m:?} bar {}", b.c.slot.index);
            // A vamp: each side of the pair plays the same kick all along.
            let side = (b.c.slot.index - b.c.band.feel_since) as usize % 2;
            let k = b.drums(Drum::Kick);
            assert_eq!(*kicks[side].get_or_insert_with(|| k.clone()), k, "{m:?}: the vamp changed");
            assert!(k.contains(&0.0), "{m:?}: no one");
            // Every kick has a bass note on it; the one is the longest and loudest.
            for x in &k {
                assert!(b.ch(2).any(|e| (b.rel(e) - x).abs() < 1e-6), "{m:?} bar {}: kick at {x} alone", b.c.slot.index);
            }
            let one = b.ch(2).find(|e| b.rel(e) == 0.0).unwrap();
            assert!(b.ch(2).all(|e| e.end - e.start <= one.end - one.start && e.volume <= one.volume), "{m:?}: the one");
            // Space: the backbeat and at most one ghost.
            let snares: Vec<&NoteEvent> = b.ch(3).filter(|e| e.sound == Sound::Drum(Drum::Snare)).collect();
            assert!(snares.len() <= 3 && snares.iter().filter(|e| e.volume <= 4).count() <= 1, "{m:?}: snares");
            assert!(b.ch(1).count() <= 3, "{m:?}: a busy clav");
            // The rhythm section on the 16th grid (straight).
            for e in b.ev.iter().filter(|e| e.ch > 0) {
                let x = b.rel(e) * 4.0;
                assert!((x - x.round()).abs() < 1e-6, "{m:?}: off the grid at {}", b.rel(e));
            }
            assert!(b.c.orns[1].has(Orn::Clav) && b.c.orns[2].has(Orn::Slap));
        }
    }
}

/// The harmony stays the chart's (or the filter's: Coltrane's changes), coloured for bossa,
/// samba and funk, simplified to the roots' power chords in rock: the comp sounds nothing
/// outside the chord's allowed notes, the bass lands on the chord's bass note on the one.
#[test]
fn feels_keep_the_harmony() {
    for f in Feel::OTHERS {
        for (m, h) in [(Music::Title, Harmony::Original), (Music::World(3), Harmony::Coltrane), (Music::World(4), Harmony::MelodicMinor)] {
            let (e, bars) = play(song(m), 5, &[Input::ForceFeel(Some(f)), freedom(0.7), Input::SetFilters(Filters { harmony: h, just_intonation: false })], 32);
            let arr = e.arrangement(h).unwrap();
            for b in bars.iter().skip(1).filter(|b| b.c.harmony == h) {
                let line = b.c.slot.song_bar as f64 * 4.0;
                for c in b.ch(1) {
                    let at = b.rel(c);
                    let harms: Vec<Harm> = [Some(at), (at >= 3.5 - 1e-9).then_some(4.0)]
                        .into_iter()
                        .flatten()
                        .filter_map(|x| b.c.band.sub_at(x).map(|s| Harm::new(s.chord)).or_else(|| arr.harm_at(line + x)))
                        .collect();
                    let ok = harms.iter().any(|hm| {
                        let allowed = feel::allowed_pcs(&hm.chord, f);
                        let chord: Vec<u8> = hm.chord.pitch_classes().collect();
                        c.sound.notes().iter().all(|n| allowed.contains(&(n % 12)) || (f != Feel::Rock && chord.contains(&(n % 12))))
                    });
                    assert!(ok || b.c.band.hits != 0, "{f:?} {m:?} {h:?} bar {}: comp {:?} at {at} over {:?}", b.c.slot.index, c.sound.notes(), harms.iter().map(|x| x.chord.to_string()).collect::<Vec<_>>());
                }
                if b.c.band.hits != 0 || b.c.band.anticipated {
                    continue;
                }
                let Some(first) = b.ch(2).find(|e| b.rel(e).abs() < 1e-6) else { panic!("{f:?} {m:?}: no bass on the one") };
                let want = b.c.band.sub_at(0.0).map(|s| s.chord.root).unwrap_or_else(|| arr.harm_at(line).unwrap().chord.bass_pc());
                assert_eq!(first.sound.notes()[0] % 12, want, "{f:?} {m:?} {h:?} bar {}: the bass's one", b.c.slot.index);
            }
        }
    }
}

/// No feels in the waltz: a forced feel stops at the waltz's bar line and the waltz plays its
/// own (and the feel comes back after it).
#[test]
fn the_waltz_has_no_feels() {
    let waltz = Filters { harmony: Harmony::Waltz, just_intonation: false };
    let (_, bars) = play(song(Music::Title), 1, &[Input::ForceFeel(Some(Feel::Rock)), freedom(0.9), Input::SetFilters(waltz)], 20);
    assert!(bars.iter().filter(|b| b.c.harmony == Harmony::Waltz).count() >= 10);
    for b in bars.iter().filter(|b| b.c.harmony == Harmony::Waltz) {
        assert_eq!(b.c.band.feel, Feel::Swing);
        assert!(b.c.orns.iter().all(|o| !o.has(Orn::Backbeat) && !o.has(Orn::PowerChords)));
    }
    // And the band's own choice never feels in the waltz either.
    let (_, bars) = play(song(Music::World(2)), 4, &[freedom(1.0), Input::SetFilters(waltz)], 120);
    assert!(bars.iter().filter(|b| b.c.harmony == Harmony::Waltz).all(|b| b.c.band.feel == Feel::Swing));
}

/// Every feel through every filter and tuning: the ornaments straight (no swung 8ths), every
/// channel in range, monophonic, on the feel's instruments; the render clean.
#[test]
fn feels_play_through_every_filter_and_tuning() {
    for f in Feel::OTHERS {
        for (k, h) in Harmony::ALL.into_iter().filter(|h| *h != Harmony::Waltz).enumerate() {
            let m = LOOPING[k % LOOPING.len()];
            let tuning = [Tuning::Equal, Tuning::Medley, Tuning::Just, Tuning::Tet7][k % 4];
            let setup = [Input::ForceFeel(Some(f)), freedom(1.0), Input::SetFilters(Filters { harmony: h, just_intonation: false }), Input::ForceTuning(Some(tuning))];
            let (e, bars) = play(song(m), 9, &setup, 24);
            let insts = e.instruments();
            for b in bars.iter().skip(1) {
                for ev in &b.ev {
                    let (lo, hi) = [(45, 96), (40, 88), (21, 60), (0, 127)][ev.ch as usize];
                    assert!(ev.sound.notes().iter().all(|n| (lo..=hi).contains(n)), "{f:?} {h:?}: ch {} {:?}", ev.ch, ev.sound);
                    // Straight: nothing on a swung off-beat.
                    let x = b.rel(ev);
                    let swung = 0.5 + song(m).swing as f64 * 0.5;
                    assert!((x - x.floor() - swung).abs() > 1e-6, "{f:?} {h:?} {m:?}: ch {} at {x} (swung)", ev.ch);
                    let pal = insts.feel_palette(f, ev.ch as usize);
                    assert!(pal.contains(&ev.inst) || insts.feel_extras.contains(&ev.inst), "{f:?}: ch {} on {}", ev.ch, insts.name(ev.inst));
                }
                for ch in 0..3 {
                    let v: Vec<&NoteEvent> = b.ch(ch).collect();
                    assert!(v.windows(2).all(|w| w[0].end <= w[1].start), "{f:?} {h:?} ch {ch}: overlap");
                }
            }
        }
    }
}

/// The feels render far faster than real time (release: 100x), every player at full freedom
/// on Coltrane's changes with the medley.
#[test]
fn feels_render_far_under_real_time() {
    let block = 512;
    for f in Feel::OTHERS {
        for m in [Music::Title, Music::World(4), Music::World(5)] {
            let mut e = Engine::with_config(song(m), synth::SAMPLE_RATE, EngineConfig { seed: 7, ..EngineConfig::default() }).unwrap();
            e.post(Input::ForceFeel(Some(f)));
            e.post(Input::SetFilters(Filters { harmony: Harmony::Coltrane, just_intonation: true }));
            e.post(Input::SetFreedom { lead: 1.0, comp: 1.0, bass: 1.0, drums: 1.0, dynamics: 0.7 });
            let mut buf = vec![Frame::ZERO; block];
            let blocks = 20 * synth::SAMPLE_RATE as usize / block;
            let t = std::time::Instant::now();
            for _ in 0..blocks {
                e.fill(&mut buf);
            }
            let per_block = t.elapsed().as_secs_f64() / blocks as f64;
            let realtime = block as f64 / synth::SAMPLE_RATE as f64;
            println!("{f:?} {m:?}: {:.1} us/block ({:.0}x real time)", per_block * 1e6, realtime / per_block);
            let budget = if cfg!(debug_assertions) { 0.25 } else { 0.01 };
            assert!(per_block < realtime * budget, "{f:?} {m:?}: {:.0}us per block", per_block * 1e6);
        }
    }
}
