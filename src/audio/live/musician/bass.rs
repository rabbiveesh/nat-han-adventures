//! The triangle: the bass line, and how it walks.
//!
//! From its written line the bass may, by the feel its plan rolled for the phrase, walk
//! quarters through the changes (mid; side-slipping a half step for two beats, high), play a
//! two-feel or a pedal point (mid), or an ostinato vamp (high). It plays the band's
//! reharmonization's roots (tritone subs), hits with the comp (a root on the hit, then rest),
//! ties over an anticipated downbeat, approaches the next bar chromatically (low) and fills
//! into the next phrase (high). In a feel ([`crate::audio::live::feel`]) it plays the feel's
//! line: bossa's root-fifth two-feel with the next root anticipated (and tied over), samba's
//! surdo, rock's pumping 8ths, funk's slap and pop locked to the kick.

use super::{Ctx, Musician, PhrasePlan, Player, Role, clear_span, cut_at, fold, musician_common, push, tidy, work};
use crate::audio::accomp::Rng;
use crate::audio::chart::Family;
use crate::audio::live::band::{HitKind, Trade};
use crate::audio::live::engine::Input;
use crate::audio::live::feel::{self, Extra, Feel};
use crate::audio::live::ornament::{Harm, Orn, Orns, nearest_pc};
use crate::audio::live::voice::{NoteEvent, Sound};

/// The bass's range (the triangle's lowest octaves).
pub const LO: i32 = 28;
pub const HI: i32 = 55;

/// Triangle: the bass line.
pub struct Bass {
    pub(super) p: Player,
    src: Vec<NoteEvent>,
    dst: Vec<NoteEvent>,
    last: Option<NoteEvent>,
    /// The last note committed.
    last_note: Option<u8>,
    /// A bossa bar ended anticipating this note (the next bar ties over it).
    anticipated: Option<(u64, u8)>,
}

impl Bass {
    pub(super) fn new(p: Player) -> Self {
        Bass { p, src: work(), dst: work(), last: None, last_note: None, anticipated: None }
    }
}

impl Musician for Bass {
    musician_common!();

    fn commit_next_bar(&mut self, ctx: &Ctx, out: &mut Vec<NoteEvent>) -> Orns {
        let mut fired = Orns::default();
        self.src.clear();
        ctx.written(2, &mut self.src);
        let feel = ctx.feel();
        if self.p.freedom <= 0.0 && feel == Feel::Swing {
            out.extend(self.src.iter().copied());
            self.remember();
            return fired;
        }
        let intent = self.p.intent(ctx.bar.index);
        let orns = intent.orns;
        let band = ctx.band;
        let bb = ctx.bb();
        let phrase_last = self.p.phrase_last(ctx.bar.index);
        let mut tmpl = ctx.template(2, self.src.first().copied().or(self.last));
        if let Some(k) = ctx.feel_inst(2, intent.switch) {
            tmpl.inst = k;
        }
        let anticipated = self.anticipated.take().filter(|a| a.0 == ctx.bar.index).map(|a| a.1);
        let mut r = ctx.rng(Role::Bass, 3);
        let prev = self.last_note.or_else(|| self.src.first().and_then(|e| e.sound.notes().first().copied())).unwrap_or(40) as i32;
        self.dst.clear();
        let has_harm = ctx.plain_harm_at(0.0).is_some();
        if band.trade == Trade::Drums && has_harm {
            // The drums' four: roots, one per chord, to keep the changes in the air.
            let mut b = 0.0;
            let mut p = prev;
            while b < bb - 1e-9 {
                let h = ctx.harm_at(b).expect("a chart");
                let mut z = b + 0.5;
                while z < bb - 1e-9 && ctx.harm_at(z).is_some_and(|x| x.chord == h.chord) {
                    z += 0.5;
                }
                let n = nearest_pc(h.chord.bass_pc(), p, LO, HI);
                push(&mut self.dst, ctx.make(&tmpl, b, (z - b) * 0.9, Sound::Note(n)));
                p = n as i32;
                b = z;
            }
            fired.add(Orn::LayOut);
        } else if has_harm && feel != Feel::Swing {
            fired.add(self.feel_line(ctx, &tmpl, prev, anticipated));
        } else if has_harm && orns.has(Orn::Ostinato) {
            self.ostinato(ctx, &tmpl, prev);
            fired.add(Orn::Ostinato);
        } else if has_harm && orns.has(Orn::Walking) {
            let slip = orns.has(Orn::SlipWalk) && bb >= 4.0;
            self.walk(ctx, &tmpl, prev, slip, &mut r);
            fired.add(Orn::Walking);
            if slip {
                fired.add(Orn::SlipWalk);
            }
        } else if has_harm && orns.has(Orn::TwoFeel) {
            let mut p = prev;
            let halves: &[f64] = if bb >= 4.0 { &[0.0, 2.0] } else { &[0.0] };
            for (k, &b) in halves.iter().enumerate() {
                let h = ctx.harm_at(b).expect("a chart");
                let same = k > 0 && ctx.harm_at(0.0).is_some_and(|x| x.chord == h.chord);
                let pc = if same { (h.chord.root + 7) % 12 } else { h.chord.bass_pc() };
                let d = if k + 1 < halves.len() { halves[k + 1] - b } else { bb - b };
                let n = nearest_pc(pc, p, LO, HI);
                push(&mut self.dst, ctx.make(&tmpl, b, d * 0.95, Sound::Note(n)));
                p = n as i32;
            }
            fired.add(Orn::TwoFeel);
        } else if has_harm && orns.has(Orn::Pedal) {
            let h = ctx.harm_at(0.0).expect("a chart");
            let key = ctx.shape.key;
            let pc = if h.chord.family() == Family::Dominant { (key + 7) % 12 } else { key };
            let n = nearest_pc(pc, prev, LO, HI);
            for k in 0..bb as usize {
                push(&mut self.dst, ctx.make(&tmpl, k as f64, 0.9, Sound::Note(n)));
            }
            fired.add(Orn::Pedal);
        } else {
            self.dst.extend(self.src.iter().copied());
        }
        // The band's reharmonization: its roots.
        for sub in band.subs.iter().flatten() {
            let h = Harm::new(sub.chord);
            let mut struck = false;
            for e in &mut self.dst {
                let b = ctx.rel(e);
                if b >= sub.from - 1e-9
                    && b < sub.to - 1e-9
                    && let Some(&n) = e.sound.notes().first()
                {
                    let at_start = (b - sub.from).abs() < 1e-6;
                    let to = if at_start { nearest_pc(sub.chord.root, n as i32, LO, HI) } else { h.nearest_chord_tone(n) };
                    e.sound = Sound::Note(to);
                    struck |= at_start;
                }
            }
            if !struck {
                cut_at(ctx, &mut self.dst, sub.from);
                let n = nearest_pc(sub.chord.root, prev, LO, HI);
                push(&mut self.dst, ctx.make(&tmpl, sub.from, (sub.to - sub.from).min(1.0) * 0.95, Sound::Note(n)));
            }
            fired.add(Orn::TritoneRoot);
        }
        tidy(&mut self.dst);
        // The band's hits: a root on each, then rest.
        let hit_list: [f64; 4] = {
            let mut a = [f64::NAN; 4];
            for (slot, b) in a.iter_mut().zip(band.hit_beats()) {
                *slot = b;
            }
            a
        };
        let nh = hit_list.iter().filter(|b| !b.is_nan()).count();
        for k in 0..nh {
            let hb = hit_list[k];
            let until = if k + 1 < nh { hit_list[k + 1] } else { bb };
            let h = if band.hit_kind == HitKind::Anticipation { ctx.plain_harm_at(bb) } else { ctx.harm_at(hb) };
            let Some(h) = h else { continue };
            clear_span(ctx, &mut self.dst, hb, until.max(hb + 0.25));
            let near = self.dst.iter().rev().find_map(|e| e.sound.notes().first().copied()).map_or(prev, |n| n as i32);
            let n = nearest_pc(h.chord.bass_pc(), near, LO, HI);
            let d = if band.hit_kind == HitKind::Anticipation { bb - hb } else { 0.5 };
            push(&mut self.dst, ctx.make(&tmpl, hb, d, Sound::Note(n)));
            fired.add(Orn::Hit);
        }
        tidy(&mut self.dst);
        // An anticipated downbeat ties over (no new attack).
        if band.anticipated
            && let Some(first) = self.dst.first_mut()
            && ctx.rel(first).abs() < 1e-6
            && self.last_note.is_some_and(|n| first.sound == Sound::Note(n))
        {
            first.tie = true;
        }
        // Into the next bar: a chromatic approach, or a fill at a phrase end.
        let next_root = ctx.plain_harm_at(bb).map(|h| h.chord.bass_pc());
        let subbed = band.subs.iter().flatten().any(|s| s.to > bb - 2.0);
        if band.hits == 0
            && !subbed
            && matches!(feel, Feel::Swing | Feel::Rock)
            && let Some(pc) = next_root
        {
            let last_i = self.dst.iter().rposition(|e| matches!(e.sound, Sound::Note(_)));
            let tail = self.dst.iter().map(|e| ctx.rel(e) + ctx.len(e)).fold(0.0, f64::max);
            if phrase_last && orns.has(Orn::BassFill) && tail <= bb + 1e-6 {
                // An 8th-note run up (or down) the scale into the next root.
                let from = bb - 2.0;
                clear_span(ctx, &mut self.dst, from, bb);
                let near = self.dst.iter().rev().find_map(|e| e.sound.notes().first().copied()).map_or(prev, |n| n as i32);
                let target = nearest_pc(pc, near, LO + 4, HI - 4);
                let scale = ctx.harm_at(from).map(|h| h.scale);
                let up = r.chance(0.6);
                for j in 0..4 {
                    let steps = 4 - j as i32;
                    let n = match scale {
                        Some(s) => s.step(target, if up { -steps } else { steps }),
                        None => (target as i32 + if up { -steps } else { steps }) as u8,
                    };
                    let b = ctx.swing8(from + j as f64 * 0.5);
                    let z = ctx.swing8(from + j as f64 * 0.5 + 0.5).min(bb);
                    push(&mut self.dst, ctx.make(&tmpl, b, (z - b) * 0.95, Sound::Note(fold(n as i32, LO, HI))));
                }
                fired.add(Orn::BassFill);
            } else if orns.has(Orn::Approach)
                && let Some(i) = last_i
            {
                let e = self.dst[i];
                let (b, d) = (ctx.rel(&e), ctx.len(&e));
                if b >= bb - 1.0 - 1e-6 && b + d <= bb + 1e-6 && d <= 1.0 + 1e-6 && ctx.harm_at(b).is_some_and(|h| h.chord.bass_pc() != pc || r.chance(0.5)) {
                    let Sound::Note(n) = e.sound else { unreachable!() };
                    let target = nearest_pc(pc, n as i32, LO + 1, HI - 1) as i32;
                    let from = if r.chance(0.5) { target + 1 } else { target - 1 };
                    if (from - n as i32).abs() <= 7 {
                        self.dst[i].sound = Sound::Note(from as u8);
                        fired.add(Orn::Approach);
                    }
                }
            }
        }
        if intent.switch
            && feel == Feel::Swing
            && !self.dst.is_empty()
            && let Some(alt) = ctx.alternate(2, self.p.plan.map_or(0, |p| p.start), tmpl.inst)
        {
            for e in &mut self.dst {
                e.inst = alt;
            }
            fired.add(Orn::Switch);
        }
        for e in &mut self.dst {
            if let Sound::Note(n) = e.sound {
                e.sound = Sound::Note(fold(n as i32, LO - 4, HI + 4));
            }
        }
        tidy(&mut self.dst);
        out.extend(self.dst.iter().copied());
        self.src.clear();
        self.src.extend(self.dst.iter().copied());
        self.remember();
        fired
    }

    fn on_input(&mut self, input: &Input, next_bar: u64) {
        self.p.on_input(input, next_bar);
    }
}

impl Bass {
    /// The feel's bass line for the bar (what it played).
    fn feel_line(&mut self, ctx: &Ctx, t: &NoteEvent, prev: i32, anticipated: Option<u8>) -> Orn {
        let bb = ctx.bb();
        let harm = |b: f64| ctx.harm_at(b).expect("a chart");
        let vol = |x: i32| (t.volume as i32 + x).clamp(1, 15) as u8;
        let note = |b: f64, d: f64, n: i32, v: u8| NoteEvent { volume: v, ..ctx.make(t, b, d, Sound::Note(n.clamp(LO - 4, HI + 4) as u8)) };
        match ctx.feel() {
            Feel::Bossa => {
                // Root on 1, the fifth on 3 (a new chord's root if it changed), the next bar's
                // root on the "and" of 4, tied over into it.
                let h0 = harm(0.0);
                let root = nearest_pc(h0.chord.bass_pc(), prev, LO, HI - 7) as i32;
                let mut first = note(0.0, 1.5, root, vol(-1));
                first.tie = anticipated == Some(root as u8) && ctx.band.sub_at(0.0).is_none();
                push(&mut self.dst, first);
                let mut p = root;
                if bb >= 4.0 {
                    let h2 = harm(2.0);
                    let n = if h2.chord == h0.chord { nearest_pc((h0.chord.root + 7) % 12, root, LO, HI) as i32 } else { nearest_pc(h2.chord.bass_pc(), root, LO, HI) as i32 };
                    push(&mut self.dst, note(2.0, 1.4, n, vol(-1)));
                    p = n;
                }
                if let Some(nh) = ctx.plain_harm_at(bb) {
                    let n = nearest_pc(nh.chord.bass_pc(), p, LO, HI - 7);
                    push(&mut self.dst, note(bb - 0.5, 0.5, n as i32, vol(-2)));
                    self.anticipated = Some((ctx.bar.index + 1, n));
                }
                Orn::BossaBass
            }
            Feel::Samba => {
                // The surdo, a 2/4 bar per half: a light root on its 1 with a 16th pickup (the
                // fifth below), the big hit on its 2.
                let mut b = 0.0;
                let mut p = prev;
                while b < bb - 1e-9 {
                    let h = harm(b);
                    let root = nearest_pc(h.chord.bass_pc(), p.min(43), LO + 5, HI - 5) as i32;
                    push(&mut self.dst, note(b, 0.7, root, vol(-3)));
                    if b + 1.0 < bb - 1e-9 {
                        let fifth = nearest_pc((h.chord.root + 7) % 12, root - 5, LO, HI) as i32;
                        push(&mut self.dst, note(b + 0.75, 0.25, fifth, vol(-4)));
                        let hb = harm(b + 1.0);
                        let big = nearest_pc(hb.chord.bass_pc(), root, LO, HI) as i32;
                        push(&mut self.dst, note(b + 1.0, 0.9, big, vol(2)));
                    }
                    p = root;
                    b += 2.0;
                }
                Orn::Surdo
            }
            Feel::Rock => {
                // Pumping 8th-note roots, the beats a little harder.
                let mut p = prev;
                for k in 0..(bb * 2.0).round() as usize {
                    let b = k as f64 * 0.5;
                    let n = nearest_pc(harm(b).chord.bass_pc(), p, LO, HI - 5) as i32;
                    push(&mut self.dst, note(b, 0.4, n, vol(if k % 2 == 0 { 1 } else { -1 })));
                    p = n;
                }
                Orn::Pumping
            }
            Feel::Funk => {
                // On the kick: the root low (slapped), octaves popped between; dead notes (a
                // muted blip) on the 16ths before some of them.
                let kicks = feel::FUNK_KICKS[feel::pattern(ctx.seed, ctx.bar.index, feel::FUNK_KICKS.len())];
                let dead = ctx.extra(Extra::Dead);
                let mut r = ctx.rng(Role::Bass, 41);
                for (j, &k) in kicks.iter().enumerate() {
                    let b = k as f64 * 0.25;
                    if b >= bb - 1e-9 {
                        continue;
                    }
                    let root = nearest_pc(harm(b).chord.bass_pc(), 36, LO, LO + 11) as i32;
                    if k > 0 && !kicks.contains(&(k - 1)) && r.chance(0.6) {
                        push(&mut self.dst, NoteEvent { inst: dead, ..note(b - 0.25, 0.08, root, vol(-2)) });
                    }
                    let (n, d) = match j {
                        0 => (root, 0.45),
                        j if j % 2 == 1 => (root + 12, 0.2),
                        _ => (root, 0.3),
                    };
                    push(&mut self.dst, note(b, d, n, vol(1)));
                }
                Orn::Slap
            }
            Feel::Swing => unreachable!("only in a feel"),
        }
    }

    fn remember(&mut self) {
        if let Some(e) = self.src.last() {
            self.last = Some(*e);
            self.last_note = e.sound.notes().first().copied();
        }
    }

    /// Walking quarters: the chord's root on its first beat, an approach on its last, scale
    /// and chord tones heading there between; the odd swung octave skip.
    fn walk(&mut self, ctx: &Ctx, t: &NoteEvent, prev: i32, slip: bool, r: &mut Rng) {
        let bb = ctx.bb();
        let beats = bb.round() as usize;
        let mut p = prev;
        for k in 0..beats {
            let b = k as f64;
            let h = ctx.harm_at(b).expect("a chart");
            let next = if k + 1 < beats { ctx.harm_at(b + 1.0) } else { ctx.plain_harm_at(bb) }.unwrap_or(h);
            let first = k == 0 || ctx.harm_at(b - 1.0).is_some_and(|x| x.chord != h.chord);
            let last = next.chord != h.chord || k + 1 == beats;
            let mut n = if first {
                nearest_pc(h.chord.bass_pc(), p, LO, HI) as i32
            } else if last {
                let target = nearest_pc(next.chord.bass_pc(), p, LO + 1, HI - 1) as i32;
                if r.chance(0.2) && target + 7 <= HI { target + 7 } else if r.chance(0.5) { target + 1 } else { target - 1 }
            } else {
                let target = nearest_pc(next.chord.bass_pc(), p, LO, HI) as i32;
                passing(p, target, &h, r)
            };
            if slip && (k == 1 || k == 2) {
                n += 1;
            }
            let n = n.clamp(LO, HI);
            if !first && !last && r.chance(0.12) {
                let skip = if n + 12 <= HI { n + 12 } else { n - 12 };
                push(&mut self.dst, ctx.make(t, b, 0.5, Sound::Note(n as u8)));
                let s = ctx.swing8(b + 0.5);
                push(&mut self.dst, ctx.make(t, s, b + 1.0 - s, Sound::Note(skip as u8)));
            } else {
                push(&mut self.dst, ctx.make(t, b, 1.0, Sound::Note(n as u8)));
            }
            p = n;
        }
    }

    /// An ostinato: root, fifth, octave, fifth in swung 8ths, chord by chord (a b7 for
    /// dominants now and then).
    fn ostinato(&mut self, ctx: &Ctx, t: &NoteEvent, prev: i32) {
        let bb = ctx.bb();
        let mut b = 0.0;
        while b < bb - 1e-9 {
            let h = ctx.harm_at(b).expect("a chart");
            let root = nearest_pc(h.chord.root, prev.min(40), LO, HI - 12) as i32;
            let top = if h.chord.family() == Family::Dominant { root + 10 } else { root + 12 };
            for (j, n) in [root, root + 7, top, root + 7].into_iter().enumerate() {
                let x = b + j as f64 * 0.5;
                if x >= bb - 1e-9 {
                    break;
                }
                let s = ctx.swing8(x);
                let z = ctx.swing8(x + 0.5).min(bb);
                push(&mut self.dst, ctx.make(t, s, (z - s) * 0.9, Sound::Note(n.clamp(LO, HI + 7) as u8)));
            }
            b += 2.0;
        }
    }
}

/// A passing note 1..=5 semitones from `p`, in the chord's scale, heading for `target`.
fn passing(p: i32, target: i32, h: &Harm, r: &mut Rng) -> i32 {
    let dir = (target - p).signum();
    let mut cands = [0i32; 10];
    let mut m = 0;
    for d in (-5..=5).filter(|d| *d != 0) {
        let n = p + d;
        let toward = dir == 0 || d.signum() == dir;
        if (LO..=HI).contains(&n) && h.scale.contains(n.rem_euclid(12) as u8) && toward {
            cands[m] = n;
            m += 1;
        }
    }
    if m == 0 {
        return nearest_pc(h.chord.root, p, LO, HI) as i32;
    }
    cands[r.below(m)]
}

