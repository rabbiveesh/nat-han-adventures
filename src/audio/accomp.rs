//! Generated accompaniment for the reharmonizing [`Harmony`] filters: a comping track for
//! pulse 2 (chords as fast arpeggios, in jazz comping rhythms) and a bass track for the
//! triangle, built from a chord chart. Deterministic for a given seed.
//!
//! - Coltrane / melodic minor: comping in Charleston / anticipation / stab rhythms, walking
//!   quarter-note bass (chord tone on each chord's first beat, an approach tone into the next
//!   chord — chromatic for Coltrane, from the scale for melodic minor — with the odd swung
//!   8th-note octave skip). Melodic minor draws every comp and bass note from the chord's
//!   melodic-minor scale.
//! - Quartal: big Charleston stabs of fourths voicings, sustained pads and the odd pentatonic
//!   run of fourths; the left hand pounds open root–fifth on 1 and the "and of 2" (anticipating
//!   beat 3), with the occasional low tonic pedal.
//!
//! Comping is voiced in o3–o5 (MIDI 48..=84) with smooth voice leading; bass stays in o1–o3.
//! Both tracks are exactly `chart.bars * 4` beats long and contiguous (gaps are rests), so
//! [`super::synth::apply_swing`] treats them like hand-written MML.

use super::Harmony;
use super::chart::{Chart, Slot};
use super::mml::{Arp, Event, EventKind, Track};
use super::theory::{self, MmChord};

/// Comping volume / duty (the hand-written pulse 2 parts sit around v7–v9).
const COMP_VOL: u8 = 7;
const PAD_VOL: u8 = 6;
const COMP_DUTY: u8 = 2;
/// Triangle volume (hand-written bass lines use the default, 12).
const BASS_VOL: u8 = 12;
/// Comping range (MIDI).
const COMP_LO: i32 = 48;
const COMP_HI: i32 = 84;
/// Walking bass range: E1..E3.
const BASS_LO: i32 = 28;
const BASS_HI: i32 = 52;

/// SplitMix64: tiny, seedable, and the same everywhere (no platform RNG in the generator).
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed ^ 0x5DEE_CE66_D1CE_4E5B)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub fn f(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.f() < p
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.f() * n as f64) as usize % n.max(1)
    }
}

/// Generated (pulse 2, triangle) for a chart. `harmony` must not be [`Harmony::Original`]
/// (that keeps the written parts); `key` is the home tonic (for the quartal pedal).
pub fn generate(chart: &Chart, harmony: Harmony, key: u8, seed: u64) -> (Track, Track) {
    let mut rng = Rng::new(seed);
    let chart = match harmony {
        Harmony::Coltrane => theory::coltrane(chart),
        _ => chart.clone(),
    };
    let slots = chart.merged();
    let voices = voice(&slots, harmony);
    let beats = chart.beats();
    let comp = match harmony {
        Harmony::Quartal => quartal_comp(&slots, &voices, beats, &mut rng),
        _ => swing_comp(&slots, &voices, beats, &mut rng),
    };
    let bass = match harmony {
        Harmony::Quartal => tyner_left_hand(&slots, key, beats, &mut rng),
        _ => walking_bass(&slots, &voices, harmony, beats, &mut rng),
    };
    (comp, bass)
}

/// What each chord slot gives the generators.
#[derive(Debug, Clone)]
struct Voice {
    /// Comping voicing (MIDI notes, ascending), placed in range with voice leading.
    comp: Vec<u8>,
    /// Pitch classes the walking bass may use on in-between beats.
    pool: Vec<u8>,
    /// Melodic-minor scale, when every note must come from it.
    scale: Option<[u8; 7]>,
}

fn voice(slots: &[Slot], harmony: Harmony) -> Vec<Voice> {
    let mut prev_center = 64.0;
    (0..slots.len())
        .map(|i| {
            let chord = slots[i].chord;
            let root = chord.root as i32;
            let (shape, pool, scale): (Vec<i32>, Vec<u8>, Option<[u8; 7]>) = match harmony {
                Harmony::MelodicMinor => {
                    let mm: MmChord = theory::melodic_minor(slots, i);
                    let shape = mm.sonority().iter().map(|&x| x as i32).collect();
                    (shape, mm.scale().to_vec(), Some(mm.scale()))
                }
                Harmony::Quartal => {
                    let shape = theory::quartal(&chord).iter().map(|&x| x as i32).collect();
                    (shape, chord.pitch_classes().collect(), None)
                }
                _ => {
                    let mut shape: Vec<i32> = chord.quality.intervals().iter().map(|&x| x as i32).collect();
                    shape.truncate(5);
                    (shape, chord.pitch_classes().collect(), None)
                }
            };
            // Pick the octave that keeps the voicing in range and closest to the last one.
            let mut best: Option<(f64, Vec<u8>)> = None;
            for oct in 2..=7 {
                let notes: Vec<i32> = shape.iter().map(|s| 12 * oct + root + s).collect();
                let (lo, hi) = (*notes.iter().min().unwrap(), *notes.iter().max().unwrap());
                if lo < COMP_LO || hi > COMP_HI {
                    continue;
                }
                let center = notes.iter().sum::<i32>() as f64 / notes.len() as f64;
                let cost = (center - prev_center).abs() + (center - 66.0).abs() * 0.5;
                if best.as_ref().is_none_or(|(c, _)| cost < *c) {
                    let mut v: Vec<u8> = notes.iter().map(|&n| n as u8).collect();
                    v.sort_unstable();
                    best = Some((cost, v));
                }
            }
            let comp = best.map(|(_, v)| v).unwrap_or_else(|| vec![(60 + root % 12) as u8]);
            prev_center = comp.iter().map(|&n| n as f64).sum::<f64>() / comp.len() as f64;
            Voice { comp, pool, scale }
        })
        .collect()
}

/// Builds a contiguous track: gaps become rests, overlaps are trimmed.
struct Builder {
    events: Vec<Event>,
    time: f64,
}

impl Builder {
    fn new() -> Self {
        Builder { events: Vec::new(), time: 0.0 }
    }

    fn push(&mut self, start: f64, dur: f64, kind: EventKind, volume: u8, duty: u8) {
        const EPS: f64 = 1e-9;
        if start + EPS < self.time {
            // Overlap: shorten the previous event (or drop this one if it starts inside it).
            match self.events.last_mut() {
                Some(prev) if prev.start + EPS < start => {
                    prev.dur = start - prev.start;
                    self.time = start;
                }
                _ => return,
            }
        }
        if start > self.time + EPS {
            self.events.push(Event { start: self.time, dur: start - self.time, kind: EventKind::Rest, volume, duty, tie: false });
        }
        if dur > EPS {
            self.events.push(Event { start, dur, kind, volume, duty, tie: false });
            self.time = start + dur;
        }
    }

    fn finish(mut self, length: f64) -> Track {
        if let Some(last) = self.events.last_mut()
            && last.start + last.dur > length
        {
            last.dur = length - last.start;
        }
        if self.time < length - 1e-9 {
            let t = self.time;
            self.events.push(Event { start: t, dur: length - t, kind: EventKind::Rest, volume: 0, duty: 0, tie: false });
        }
        self.events.retain(|e| e.dur > 1e-9);
        Track { events: self.events, length }
    }
}

/// Index of the slot sounding at beat `t` (wrapping).
fn slot_at(slots: &[Slot], beats: f64, t: f64) -> usize {
    let t = t.rem_euclid(beats);
    slots.partition_point(|s| s.start <= t + 1e-9).saturating_sub(1)
}

/// One comping hit: start, length, and whether it anticipates the chord half a beat later.
type Hit = (f64, f64, bool);

/// Lay the hits of one bar against the chart: every chord gets at least one hit, hits use the
/// chord sounding when they start (anticipations: the next one), and stop at chord changes.
fn place_hits(slots: &[Slot], beats: f64, bar: f64, pattern: &[Hit]) -> Vec<(f64, f64, usize)> {
    let mut hits: Vec<(f64, f64, usize)> = Vec::new();
    for &(s, d, anticip) in pattern {
        let t = bar + s;
        let idx = slot_at(slots, beats, if anticip { t + 0.5 } else { t });
        let end = if anticip { t + d } else { (t + d).min(slots[idx].end()) };
        hits.push((t, end - t, idx));
    }
    // Chords starting inside this bar with no hit of their own get one on their first beat.
    for (i, s) in slots.iter().enumerate() {
        if s.start >= bar - 1e-9 && s.start < bar + 4.0 - 1e-9 && !hits.iter().any(|h| h.2 == i) {
            hits.push((s.start, s.dur.min(1.0), i));
        }
    }
    hits.sort_by(|a, b| a.0.total_cmp(&b.0));
    hits
}

fn arp(notes: &[u8]) -> EventKind {
    EventKind::Arp(Arp::new(notes))
}

/// Coltrane / melodic-minor comping.
fn swing_comp(slots: &[Slot], voices: &[Voice], beats: f64, rng: &mut Rng) -> Track {
    const PATTERNS: [&[Hit]; 4] = [
        // Charleston: dotted quarter on 1, 8th on the "and" of 2.
        &[(0.0, 1.5, false), (1.5, 0.5, false)],
        // Downbeat + anticipation of the next bar on the "and" of 4.
        &[(0.0, 1.0, false), (3.5, 0.5, true)],
        // Stabs on 2 and the "and" of 3.
        &[(1.0, 0.5, false), (2.5, 1.0, false)],
        // Reverse Charleston.
        &[(1.5, 0.5, false), (3.0, 1.0, false)],
    ];
    let mut b = Builder::new();
    let mut bar = 0.0;
    while bar < beats - 1e-9 {
        let pattern = PATTERNS[rng.below(PATTERNS.len())];
        for (t, d, i) in place_hits(slots, beats, bar, pattern) {
            if t < beats - 1e-9 {
                b.push(t, d, arp(&voices[i].comp), COMP_VOL, COMP_DUTY);
            }
        }
        bar += 4.0;
    }
    b.finish(beats)
}

/// McCoy Tyner comping: big stabs of fourths, pads, and pentatonic runs of fourths.
fn quartal_comp(slots: &[Slot], voices: &[Voice], beats: f64, rng: &mut Rng) -> Track {
    let mut b = Builder::new();
    let mut bar = 0.0;
    while bar < beats - 1e-9 {
        let i = slot_at(slots, beats, bar);
        let whole_bar = slots[i].end() >= bar + 4.0 - 1e-9;
        let roll = rng.f();
        if whole_bar && roll < 0.2 {
            // Sustained pad.
            b.push(bar, 4.0, arp(&voices[i].comp), PAD_VOL, 1);
        } else if whole_bar && roll < 0.35 {
            // Stab, then a run down (or up) through the fourths, one 8th each.
            b.push(bar, 1.0, arp(&voices[i].comp), COMP_VOL + 1, COMP_DUTY);
            let mut run: Vec<u8> = voices[i].comp.clone();
            if run.last().is_some_and(|&n| (n as i32 + 12) <= COMP_HI) {
                run.extend(voices[i].comp.iter().map(|n| n + 12).filter(|&n| n as i32 <= COMP_HI));
            }
            if rng.chance(0.5) {
                run.reverse();
            }
            for (k, &n) in run.iter().take(6).enumerate() {
                b.push(bar + 1.0 + 0.5 * k as f64, 0.5, EventKind::Note(n), COMP_VOL, 1);
            }
        } else {
            for (t, d, i) in place_hits(slots, beats, bar, &[(0.0, 1.5, false), (1.5, 0.5, false)]) {
                if t < beats - 1e-9 {
                    b.push(t, d, arp(&voices[i].comp), COMP_VOL + 1, COMP_DUTY);
                }
            }
        }
        bar += 4.0;
    }
    b.finish(beats)
}

/// The note with pitch class `pc` closest to `near`, within `lo..=hi`.
fn nearest(pc: u8, near: i32, lo: i32, hi: i32) -> i32 {
    let mut best = lo + (pc as i32 - lo).rem_euclid(12);
    let mut n = best;
    while n <= hi {
        if (n - near).abs() < (best - near).abs() {
            best = n;
        }
        n += 12;
    }
    best
}

/// Quarter-note walking bass (Coltrane, melodic minor).
fn walking_bass(slots: &[Slot], voices: &[Voice], harmony: Harmony, beats: f64, rng: &mut Rng) -> Track {
    let mut b = Builder::new();
    let total = beats.round() as usize;
    let mut prev = 36; // C2
    let mut beat = 0usize;
    while beat < total {
        let t = beat as f64;
        let i = slot_at(slots, beats, t);
        let slot = slots[i];
        let next = (i + 1) % slots.len();
        let next_root = slots[next].chord.bass_pc();
        let first = (t - slot.start).abs() < 1e-9;
        let last = (slot.end() - (t + 1.0)).abs() < 1e-9;
        let note = if first {
            nearest(slot.chord.bass_pc(), prev, BASS_LO, BASS_HI)
        } else if last {
            let target = nearest(next_root, prev, BASS_LO + 1, BASS_HI - 1);
            approach(target, &voices[i], harmony, rng)
        } else {
            let target = nearest(next_root, prev, BASS_LO, BASS_HI);
            passing(prev, target, &voices[i], rng)
        };
        let note = note.clamp(BASS_LO, BASS_HI);
        // The odd swung 8th-note octave skip on an in-between beat.
        if !first && !last && rng.chance(0.12) {
            let skip = if note + 12 <= BASS_HI { note + 12 } else { note - 12 };
            b.push(t, 0.5, EventKind::Note(note as u8), BASS_VOL, 0);
            b.push(t + 0.5, 0.5, EventKind::Note(skip as u8), BASS_VOL, 0);
        } else {
            b.push(t, 1.0, EventKind::Note(note as u8), BASS_VOL, 0);
        }
        prev = note;
        beat += 1;
    }
    b.finish(beats)
}

/// An approach tone into `target`: chromatic (Coltrane) or the nearest scale tone (melodic
/// minor), from above or below; sometimes the fifth above (a dominant approach).
fn approach(target: i32, v: &Voice, harmony: Harmony, rng: &mut Rng) -> i32 {
    let below = rng.chance(0.5);
    match (harmony, v.scale) {
        (Harmony::MelodicMinor, Some(scale)) => {
            let in_scale = |n: i32| scale.contains(&((n.rem_euclid(12)) as u8));
            let steps: [i32; 2] = if below { [-1, 1] } else { [1, -1] };
            for d in 1..=3 {
                for s in steps {
                    if in_scale(target + s * d) {
                        return target + s * d;
                    }
                }
            }
            target
        }
        _ => {
            if rng.chance(0.2) && target + 7 <= BASS_HI {
                target + 7
            } else if below {
                target - 1
            } else {
                target + 1
            }
        }
    }
}

/// An in-between beat: a pool note 1..=5 semitones from `prev`, heading for `target`.
fn passing(prev: i32, target: i32, v: &Voice, rng: &mut Rng) -> i32 {
    let dir = (target - prev).signum();
    let mut cands: Vec<i32> = (-5..=5)
        .filter(|&d| d != 0)
        .map(|d| prev + d)
        .filter(|&n| (BASS_LO..=BASS_HI).contains(&n) && v.pool.contains(&(n.rem_euclid(12) as u8)))
        .collect();
    let toward: Vec<i32> = cands.iter().copied().filter(|&n| dir == 0 || (n - prev).signum() == dir).collect();
    if !toward.is_empty() && rng.chance(0.8) {
        cands = toward;
    }
    if cands.is_empty() {
        // Nothing near: jump to the nearest chord tone (pool[0] is the root).
        return nearest(v.pool[0], prev, BASS_LO, BASS_HI);
    }
    cands[rng.below(cands.len())]
}

/// McCoy Tyner's left hand: open root–fifth on 1 and the "and" of 2 (anticipating beat 3),
/// and now and then a bar of low tonic pedal.
fn tyner_left_hand(slots: &[Slot], key: u8, beats: f64, rng: &mut Rng) -> Track {
    let mut b = Builder::new();
    let fifth = |pc: u8| {
        let r = nearest(pc, 34, BASS_LO, BASS_LO + 11);
        arp(&[r as u8, (r + 7) as u8])
    };
    let mut bar = 0.0;
    let mut n = 0;
    while bar < beats - 1e-9 {
        if n % 8 == 7 && rng.chance(0.5) || rng.chance(0.08) {
            let pedal = nearest(key, 28, 24, 35);
            b.push(bar, 4.0, EventKind::Note(pedal as u8), BASS_VOL, 0);
        } else {
            let a = slots[slot_at(slots, beats, bar)].chord.bass_pc();
            let c = slots[slot_at(slots, beats, bar + 2.0)].chord.bass_pc();
            b.push(bar, 1.25, fifth(a), BASS_VOL, 0);
            b.push(bar + 1.5, 2.0, fifth(c), BASS_VOL, 0);
        }
        bar += 4.0;
        n += 1;
    }
    b.finish(beats)
}

#[cfg(test)]
mod tests {
    use super::super::chart;
    use super::*;

    const CHART: &str = "| Cmaj7 | Am7 | Dm7 | G7 | Em7 A7 | Dm7 G7 | Cmaj7 | Dm7 G7 | \
                         C7 | F6 | Bm7b5 E7 | Am6 | D7 | % | G7 | % |";

    fn notes_of(e: &Event) -> Vec<u8> {
        match e.kind {
            EventKind::Note(n) => vec![n],
            EventKind::Arp(a) => a.notes().to_vec(),
            _ => vec![],
        }
    }

    #[test]
    fn tracks_are_loop_aligned_contiguous_and_in_range() {
        let c = chart::parse(CHART).unwrap();
        for h in [Harmony::Coltrane, Harmony::Quartal, Harmony::MelodicMinor] {
            for seed in 0..20 {
                let (comp, bass) = generate(&c, h, 0, seed);
                for (name, t, lo, hi) in [("comp", &comp, 48, 84), ("bass", &bass, 24, 59)] {
                    assert_eq!(t.length, c.beats(), "{h:?} {name}");
                    let mut time = 0.0;
                    for e in &t.events {
                        assert!((e.start - time).abs() < 1e-9, "{h:?} {name}: gap at {time}");
                        assert!(e.dur > 0.0);
                        time = e.start + e.dur;
                        for n in notes_of(e) {
                            assert!((lo..=hi).contains(&(n as i32)), "{h:?} {name}: note {n}");
                        }
                    }
                    assert!((time - c.beats()).abs() < 1e-9, "{h:?} {name}: ends at {time}");
                }
                // Deterministic.
                assert_eq!(generate(&c, h, 0, seed), (comp, bass));
            }
        }
    }

    #[test]
    fn walking_bass_lands_on_roots_and_melodic_minor_stays_in_scale() {
        let c = chart::parse(CHART).unwrap();
        let slots = c.merged();
        for seed in 0..10 {
            let (_, bass) = generate(&c, Harmony::Coltrane, 0, seed);
            let ct = theory::coltrane(&c).merged();
            for s in &ct {
                let e = bass.events.iter().find(|e| (e.start - s.start).abs() < 1e-9).expect("a note on each chord");
                assert_eq!(notes_of(e)[0] % 12, s.chord.root, "seed {seed} at {}", s.start);
            }
            let (comp, bass) = generate(&c, Harmony::MelodicMinor, 0, seed);
            for e in comp.events.iter().chain(&bass.events) {
                let i = slot_at(&slots, c.beats(), e.start);
                let mm = theory::melodic_minor(&slots, i);
                let ok = notes_of(e).iter().all(|&n| mm.in_scale(n));
                // Anticipations may belong to the next chord's scale.
                let j = slot_at(&slots, c.beats(), e.start + 0.5);
                let ok_next = notes_of(e).iter().all(|&n| theory::melodic_minor(&slots, j).in_scale(n));
                assert!(ok || ok_next, "seed {seed}: {e:?} not in {mm:?}");
            }
        }
    }
}
