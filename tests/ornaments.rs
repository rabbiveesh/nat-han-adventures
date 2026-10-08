//! The band's ornaments (`audio::live::musician`, `band`, `ornament`): freedom 0 plays the
//! written parts exactly; above it the ornaments stay in range, follow the harmony, resolve
//! at phrase ends, line up across the band, and are deterministic per seed.

use kira::Frame;
use nat_han_adventures::audio::{
    Filters, Harmony, Music,
    chart::Family,
    live::{
        Engine, EngineConfig, Input,
        band::{Fill, Flourish, HitKind},
        feel::Feel,
        engine::CommittedBar,
        library,
        musician::{PhrasePlan, Role},
        ornament::{Harm, Orn, Orns},
        song::SongFile,
        voice::{NoteEvent, Sound},
    },
    mml::Drum,
    synth,
};

/// One committed bar: its record, its events, the plans in force.
struct Bar {
    c: CommittedBar,
    ev: Vec<NoteEvent>,
    plans: [Option<PhrasePlan>; 4],
}

impl Bar {
    fn ch(&self, ch: u8) -> impl Iterator<Item = &NoteEvent> {
        self.ev.iter().filter(move |e| e.ch == ch)
    }
}

fn freedom(f: f32) -> Input {
    Input::SetFreedom { lead: f, comp: f, bass: f, drums: f, dynamics: 0.0 }
}

/// Play `song` for `bars` bars (`inputs(k)` posted as bar `k` is about to be decided), and
/// collect every bar as it's committed.
fn play(song: &SongFile, seed: u64, setup: &[Input], inputs: impl Fn(u64) -> Vec<Input>, bars: u64) -> (Engine, Vec<Bar>) {
    let mut e = Engine::with_config(song, synth::SAMPLE_RATE, EngineConfig { seed, ..EngineConfig::default() }).unwrap();
    for i in setup {
        e.post(*i);
    }
    let mut out = Vec::new();
    let mut buf = [Frame::ZERO; 256];
    let mut guard = 0;
    while (out.len() as u64) < bars && guard < 1_000_000 {
        guard += 1;
        e.fill(&mut buf);
        if e.finished() {
            break;
        }
        let s = e.state();
        let have = out.len() as u64;
        for c in s.upcoming.iter().filter(|c| c.slot.index >= have) {
            let ev = e.pending_events().filter(|x| x.bar == c.slot.index).copied().collect();
            let plans = s.musicians.map(|m| m.plan);
            out.push(Bar { c: *c, ev, plans });
            for i in inputs(c.slot.index + 1) {
                e.post(i);
            }
        }
    }
    (e, out)
}

fn song(m: Music) -> &'static SongFile {
    library::song(m).unwrap().1
}

const LOOPING: [Music; 7] = [Music::Title, Music::World(1), Music::World(2), Music::World(3), Music::World(4), Music::World(5), Music::Victory];

/// Freedom 0 is the written part, whatever the game does (the cues need freedom to play).
#[test]
fn freedom_zero_is_exactly_as_written() {
    for m in [Music::World(2), Music::World(4)] {
        let render = |inputs: &dyn Fn(u64) -> Vec<Input>| {
            let mut e = Engine::with_config(song(m), synth::SAMPLE_RATE, EngineConfig::default()).unwrap();
            e.post(freedom(0.0));
            let mut out = vec![Frame::ZERO; 40 * 32_000];
            for (k, c) in out.chunks_mut(32_000).enumerate() {
                for i in inputs(k as u64) {
                    e.post(i);
                }
                e.fill(c);
            }
            out
        };
        let quiet = render(&|_| vec![]);
        let busy = render(&|k| match k {
            3 => vec![Input::Toot, Input::Toot],
            6 => vec![Input::Death],
            9 => vec![Input::Checkpoint],
            _ => vec![],
        });
        assert!(quiet == busy, "{m:?}: the cues changed the written parts at freedom 0");
        // And the written parts are what the offline renderer plays (the engine at 0).
        let (_, bars) = play(song(m), 1, &[freedom(0.0)], |_| vec![], 8);
        assert!(bars.iter().all(|b| b.c.orns.iter().all(|o| o.is_empty()) && b.c.band.hits == 0 && b.c.band.subs == [None, None]));
    }
}

/// The same seed plays the same notes; another seed, others.
#[test]
fn ornaments_are_deterministic_per_seed() {
    let s = song(Music::World(3));
    let notes = |seed: u64| {
        let (_, bars) = play(s, seed, &[freedom(0.8)], |k| if k == 5 { vec![Input::Toot, Input::Death] } else { vec![] }, 24);
        bars.iter().flat_map(|b| b.ev.iter().map(|e| (e.ch, e.start, e.end, e.sound, e.volume, e.inst, e.fx))).collect::<Vec<_>>()
    };
    assert_eq!(notes(4), notes(4));
    assert_ne!(notes(4), notes(5));
}

fn notes_of(e: &NoteEvent) -> &[u8] {
    e.sound.notes()
}

/// Every channel stays in its range, every filter, any freedom.
#[test]
fn ornaments_stay_in_range() {
    for (k, m) in LOOPING.into_iter().enumerate() {
        let h = [Harmony::Original, Harmony::Coltrane, Harmony::Waltz, Harmony::MelodicMinor, Harmony::Quartal][k % 5];
        for f in [0.5, 1.0] {
            let (_, bars) = play(song(m), k as u64, &[freedom(f), Input::SetFilters(Filters { harmony: h, just_intonation: false })], |_| vec![], 36);
            for b in &bars {
                for e in &b.ev {
                    let (lo, hi) = match e.ch {
                        0 => (45, 96),
                        1 => (40, 88),
                        2 => (21, 60),
                        _ => (0, 127),
                    };
                    for &n in notes_of(e) {
                        assert!((lo..=hi).contains(&n), "{m:?} {h:?} {f}: ch {} note {n} in bar {} ({})", e.ch, b.c.slot.index, b.c.orns[e.ch as usize].names());
                    }
                    assert!(e.end > e.start, "{m:?}: an empty event");
                }
                // Monophonic voices: no overlaps within a channel.
                for ch in 0..3 {
                    let v: Vec<&NoteEvent> = b.ch(ch).collect();
                    for w in v.windows(2) {
                        assert!(w[0].end <= w[1].start, "{m:?} {h:?} ch {ch} bar {}: overlap", b.c.slot.index);
                    }
                }
            }
        }
    }
}

/// The written lead's notes in each bar, from an engine at freedom 0 (same seed and filters).
fn written(m: Music, setup: &[Input], bars: u64) -> Vec<Bar> {
    let mut s = setup.to_vec();
    s.push(freedom(0.0));
    play(song(m), 1, &s, |_| vec![], bars).1
}

/// At a phrase end the lead lands: on the written note, a chord tone of the harmony in force,
/// or a step from the next phrase's first note.
#[test]
fn the_lead_resolves_phrase_ends() {
    for m in LOOPING {
        for (h, f) in [(Harmony::Original, 0.5), (Harmony::Coltrane, 0.8), (Harmony::Original, 1.0)] {
            let setup = [Input::SetFilters(Filters { harmony: h, just_intonation: false })];
            let plain = written(m, &setup, 34);
            let mut s = setup.to_vec();
            s.push(freedom(f));
            let (e, bars) = play(song(m), 3, &s, |_| vec![], 34);
            let arr = e.arrangement(h).unwrap();
            let shape = e.shape();
            for (k, b) in bars.iter().enumerate().take(33) {
                let Some(p) = b.plans[0] else { continue };
                if p.last_bar() != b.c.slot.index || b.c.orns[0].is_empty() || b.c.orns[0].has(Orn::LayOut) {
                    continue;
                }
                let Some(last) = b.ch(0).filter(|e| matches!(e.sound, Sound::Note(_))).last() else { continue };
                let Sound::Note(n) = last.sound else { unreachable!() };
                let harm: Harm = match arr.harm_at(last.beat) {
                    Some(x) => x,
                    None => continue,
                };
                let sub = b.c.band.sub_at(last.beat - b.c.slot.song_bar as f64 * shape.bar_beats);
                let chord_tone = harm.is_chord_tone(n) || sub.is_some_and(|s| Harm::new(s.chord).is_chord_tone(n));
                let as_written = plain[k].ch(0).any(|w| w.start == last.start && w.sound == last.sound);
                let next = plain.get(k + 1).and_then(|nb| nb.ch(0).find_map(|w| w.sound.notes().first().copied()));
                let steps_on = next.is_some_and(|t| (t as i32 - n as i32).abs() <= 2);
                assert!(
                    chord_tone || as_written || steps_on,
                    "{m:?} {h:?} {f}: phrase ends bar {} on {n} over {} ({})",
                    b.c.slot.index,
                    harm.chord,
                    b.c.orns[0].names()
                );
            }
        }
    }
}

/// Up to 0.5 the tune is the tune: most written notes still sound where they were written
/// (the same pitch class, under an ornament or not).
#[test]
fn the_head_stays_recognizable_up_to_half_freedom() {
    for m in LOOPING {
        let plain = written(m, &[], 32);
        let (_, bars) = play(song(m), 9, &[freedom(0.5)], |_| vec![], 32);
        let (mut kept, mut total) = (0, 0);
        for (p, b) in plain.iter().zip(&bars) {
            for w in p.ch(0) {
                let Sound::Note(n) = w.sound else { continue };
                total += 1;
                // The written pitch class sounds early in the written note (under a grace,
                // a turn or a mordent, an octave away: it's still the tune).
                let window = w.start..w.start + ((w.end - w.start) / 2).max(1);
                kept += b.ch(0).any(|e| e.start < window.end && e.end > window.start && e.sound.notes().iter().any(|x| x % 12 == n % 12)) as u32;
            }
        }
        let frac = kept as f64 / total.max(1) as f64;
        assert!(frac >= 0.8, "{m:?}: only {:.0}% of the tune left at 0.5", frac * 100.0);
    }
}

/// At the game's neutral 0.35 the band is clearly doing things: most bars carry an ornament,
/// and a good dozen different ones turn up in a song.
#[test]
fn a_neutral_band_is_audibly_alive() {
    for m in LOOPING {
        let (_, bars) = play(song(m), 7, &[freedom(0.35)], |_| vec![], 32);
        let busy = bars.iter().filter(|b| b.c.orns[0].0 | b.c.orns[1].0 | b.c.orns[2].0 != 0).count();
        let all = Orns(bars.iter().flat_map(|b| b.c.orns).fold(0, |a, o| a | o.0));
        assert!(busy * 10 >= bars.len() * 7, "{m:?}: only {busy}/{} bars ornamented", bars.len());
        assert!(all.iter().count() >= 9, "{m:?}: only {}", all.names());
        // The lead decorates in most bars.
        let lead = bars.iter().filter(|b| !b.c.orns[0].is_empty()).count();
        assert!(lead * 2 >= bars.len(), "{m:?}: the lead ornaments only {lead} bars");
    }
}

/// Hits line up: where the band plans one, the comp stabs, the bass hits a root and the
/// drums kick, on the same sample.
#[test]
fn hits_line_up_across_comp_bass_and_drums() {
    let mut seen = 0;
    for m in LOOPING {
        let (_, bars) = play(song(m), 11, &[freedom(0.7)], |_| vec![], 40);
        for b in bars.iter().filter(|b| b.c.band.hits != 0) {
            let line = b.c.slot.song_bar as f64 * if b.c.harmony == Harmony::Waltz { 3.0 } else { 4.0 };
            for hb in b.c.band.hit_beats() {
                let at = |ev: &NoteEvent| ev.beat - line;
                let near = |ev: &&NoteEvent| (at(ev) - hb).abs() < 1e-6;
                let comp = b.ch(1).find(near);
                let bass = b.ch(2).find(near);
                let kick = b.ch(3).find(|e| near(e) && e.sound == Sound::Drum(Drum::Kick));
                assert!(comp.is_some() && bass.is_some() && kick.is_some(), "{m:?} bar {}: hit at {hb}: comp {comp:?} bass {bass:?} kick {kick:?}", b.c.slot.index);
                let (c, ba, k) = (comp.unwrap(), bass.unwrap(), kick.unwrap());
                assert!(c.start == ba.start && ba.start == k.start, "{m:?}: hit samples differ");
                seen += 1;
                // The bass rests after an anticipation (the root rings to the bar line).
                if b.c.band.hit_kind == HitKind::Anticipation {
                    assert!(b.ch(2).all(|e| e.start <= ba.start), "{m:?}: the bass played on after the hit");
                }
            }
        }
    }
    assert!(seen >= 10, "only {seen} hits");
}

/// The band's reharmonization: the bass plays the substitute's root where it starts, and the
/// comp's notes under it are the substitute's chord tones.
#[test]
fn the_reharm_keeps_bass_and_comp_together() {
    let mut seen = 0;
    for m in LOOPING {
        for seed in [1, 2] {
            let (_, bars) = play(song(m), seed, &[freedom(0.95)], |_| vec![], 40);
            // (Bar 0's first notes have started before it's collected.)
            for b in bars.iter().skip(1) {
                let line = b.c.slot.song_bar as f64 * 4.0;
                for s in b.c.band.subs.iter().flatten() {
                    let h = Harm::new(s.chord);
                    let in_sub = |e: &&NoteEvent| e.beat - line >= s.from - 1e-6 && e.beat - line < s.to - 1e-6;
                    let bass = b.ch(2).find(|e| (e.beat - line - s.from).abs() < 1e-6);
                    if b.c.band.hits == 0 {
                        let bass = bass.unwrap_or_else(|| panic!("{m:?} bar {}: no bass note at the sub", b.c.slot.index));
                        assert_eq!(bass.sound.notes()[0] % 12, s.chord.root, "{m:?} bar {}: the bass missed the sub's root", b.c.slot.index);
                    }
                    for e in b.ch(1).filter(in_sub) {
                        if b.c.band.hits != 0 {
                            continue;
                        }
                        assert!(e.sound.notes().iter().all(|&n| h.is_chord_tone(n) || b.c.orns[1].has(Orn::Polychord)), "{m:?} bar {}: comp {:?} over {}", b.c.slot.index, e.sound, s.chord);
                    }
                    seen += 1;
                }
            }
        }
    }
    assert!(seen >= 5, "only {seen} reharmonizations");
}

/// Side-slips move half a bar of the tune by exactly a semitone (and snap back); planing keeps
/// the tune on top of parallel fourths, triads or clusters; both only over a dominant. The
/// bass slips into the next phrase a semitone off its root.
#[test]
fn side_slips_and_planing_have_their_intervals() {
    let (mut slips, mut planes, mut bass_slips) = (0, 0, 0);
    for m in LOOPING {
        let plain = written(m, &[], 40);
        for f in [0.55, 0.8] {
        let (e, bars) = play(song(m), 21, &[freedom(f)], |_| vec![], 40);
        let arr = e.arrangement(Harmony::Original).unwrap();
        let bar_beats = e.shape().bar_beats;
        // The harmony at song beat `at` of bar `b`, as the band played it.
        let harm = |b: &Bar, at: f64| b.c.band.sub_at(at - b.c.slot.song_bar as f64 * bar_beats).map(|s| Harm::new(s.chord)).or_else(|| arr.harm_at(at)).expect("a chart");
        for (p, b) in plain.iter().zip(&bars) {
            // (A feel plays the line straight: not the written grid.)
            if b.c.band.feel != Feel::Swing {
                continue;
            }
            let o = b.c.orns[0];
            if o.has(Orn::SideSlip) {
                let mut moved = 0;
                for (w, e) in p.ch(0).zip(b.ch(0)) {
                    let (Sound::Note(a), Sound::Note(z)) = (w.sound, e.sound) else { continue };
                    assert_eq!(w.start, e.start, "{m:?} bar {}: a side-slip moved a note in time", b.c.slot.index);
                    let d = z as i32 - a as i32;
                    assert!(d.abs() <= 1, "{m:?} bar {}: slipped by {d}", b.c.slot.index);
                    if d != 0 {
                        let h = harm(b, e.beat);
                        assert_eq!(h.chord.family(), Family::Dominant, "{m:?} bar {}: a side-slip over {}", b.c.slot.index, h.chord);
                    }
                    moved += (d != 0) as u32;
                }
                assert!(moved > 0);
                slips += 1;
            }
            if o.has(Orn::Planing) {
                for (w, e) in p.ch(0).zip(b.ch(0)) {
                    let (Sound::Note(a), Sound::Arp(arp)) = (w.sound, e.sound) else { continue };
                    let n = arp.notes();
                    assert_eq!(*n.last().unwrap(), a, "{m:?}: the tune isn't on top");
                    let iv: Vec<i32> = n.windows(2).map(|x| x[1] as i32 - x[0] as i32).collect();
                    assert!(iv.iter().all(|&i| i == 5) || iv.iter().all(|&i| (1..=4).contains(&i)), "{m:?}: planed {n:?}");
                    let h = harm(b, e.beat);
                    assert_eq!(h.chord.family(), Family::Dominant, "{m:?} bar {}: planing over {}", b.c.slot.index, h.chord);
                }
                planes += 1;
            }
            // The bass's slip into the next phrase: its root a semitone off, on the last 8th.
            if b.c.orns[2].has(Orn::SlipBass)
                && let Some(next) = arr.harm_at((b.c.slot.song_bar + 1) as f64 * bar_beats)
                && let Some(last) = b.ch(2).last()
                && let Sound::Note(n) = last.sound
            {
                let off = (n as i32 - next.chord.bass_pc() as i32).rem_euclid(12);
                assert!(off == 1 || off == 11, "{m:?} bar {}: slipped {n} into {}", b.c.slot.index, next.chord);
                bass_slips += 1;
            }
        }
        }
    }
    assert!(slips > 0 && planes > 0 && bass_slips > 0, "slips {slips}, planes {planes}, bass slips {bass_slips}");
}

/// The game's big moments: a summon crashes and fills; a checkpoint fills short; a death gets
/// the lead's wah-wah.
#[test]
fn big_moments_get_a_flourish() {
    let coltrane = Filters { harmony: Harmony::Coltrane, just_intonation: false };
    let (_, bars) = play(
        song(Music::World(1)),
        2,
        &[freedom(0.35)],
        |k| match k {
            4 => vec![Input::Checkpoint],
            8 => vec![Input::Death],
            12 => vec![Input::SetFilters(coltrane)],
            _ => vec![],
        },
        16,
    );
    let b = |k: usize| &bars[k];
    assert_eq!(b(4).c.band.flourish, Flourish::Checkpoint);
    assert!(b(4).c.orns[3].has(Orn::ShortFill) || b(4).c.orns[3].has(Orn::Fill), "{}", b(4).c.orns[3].names());
    assert!(b(5).c.band.crash);
    assert_eq!(b(8).c.band.flourish, Flourish::Death);
    assert!(b(8).c.orns[0].has(Orn::WahWah));
    let wah: Vec<u8> = b(8).ch(0).map(|e| e.sound.notes()[0]).collect();
    assert_eq!(wah.len(), 4);
    assert!(wah.windows(2).all(|w| w[1] + 1 == w[0]), "wah-wah {wah:?}");
    assert!(b(8).ch(0).all(|e| e.fx.wah));
    let switch = bars.iter().position(|x| x.c.harmony == Harmony::Coltrane).unwrap();
    let s = b(switch);
    assert_eq!(s.c.band.flourish, Flourish::Summon);
    assert!(s.c.band.crash && s.c.band.fill == Fill::Full);
    assert!(s.c.orns[3].has(Orn::Crash) && s.c.orns[3].has(Orn::Fill), "{}", s.c.orns[3].names());
    assert!(s.ch(3).any(|e| e.sound == Sound::Drum(Drum::Crash) && e.start == s.c.slot.start));
}

/// Musicians switch to an alternate from their palette for a phrase, and back (and play a
/// feel on the feel's instruments).
#[test]
fn musicians_switch_instruments_and_back() {
    let mut switched = 0;
    for m in LOOPING {
        let s = song(m);
        let (e, bars) = play(s, 5, &[freedom(0.8)], |_| vec![], 40);
        let insts = e.instruments();
        for b in &bars {
            for r in [Role::Lead, Role::Comp, Role::Bass] {
                let ch = r as u8;
                let pal = s.instruments.palette(ch as usize);
                let feel = insts.feel_palette(b.c.band.feel, ch as usize);
                for e in b.ch(ch) {
                    // Only the written instrument, a palette entry, or the feel's.
                    let extra = insts.feel_extras.contains(&e.inst) && !feel.is_empty();
                    assert!(
                        e.inst == 0 || pal.contains(&e.inst) || feel.contains(&e.inst) || extra || s.tracks[ch as usize].events.iter().any(|w| w.inst == e.inst),
                        "{m:?}: instrument {}",
                        e.inst
                    );
                    if !feel.is_empty() {
                        assert!(feel.contains(&e.inst) || extra, "{m:?} bar {}: a {:?} bar on {}", b.c.slot.index, b.c.band.feel, insts.name(e.inst));
                    }
                }
                if b.c.orns[ch as usize].has(Orn::Switch) {
                    assert!(b.ch(ch).all(|e| e.inst != 0 && pal.contains(&e.inst)));
                    switched += 1;
                }
            }
        }
        // And back: some bars after a switch are on the base again.
        let lead_insts: Vec<u8> = bars.iter().filter(|b| b.c.band.feel == Feel::Swing).flat_map(|b| b.ch(0).map(|e| e.inst)).collect();
        if lead_insts.iter().any(|&i| i != 0) {
            assert!(lead_insts.contains(&0), "{m:?}: the lead never came back");
        }
    }
    assert!(switched >= 5, "only {switched} switches");
}
