//! Pulse 2: the comping, and how it moves.
//!
//! The comp starts from its written part (chords as arpeggios, or a counter-line) and, by its
//! plan: swaps the bar's rhythm for a Charleston or "Freddie Green" quarters (mid) or rising
//! McCoy Tyner fourths (high); voices the band's reharmonization ([`BandPlan::subs`]); turns
//! chords into upper-structure polychords (high); moves the voicing on a held chord (mid);
//! slides into the next phrase by side-slipping or planing (high); plays the band's hits (an
//! anticipation of the next bar, an ending figure) and doesn't re-attack an anticipated
//! downbeat; adds the odd extra stab (low); and lays out under a big drum fill.
//!
//! [`BandPlan::subs`]: crate::audio::live::band::BandPlan::subs

use super::{Ctx, Musician, PhrasePlan, Player, Role, clear_from, clear_span, cut_at, fold_sound, musician_common, push, tidy, work};
use crate::audio::accomp::{planing_mode, planing_run};
use crate::audio::chart::Family;
use crate::audio::live::band::{Fill, HitKind, Trade};
use crate::audio::live::engine::Input;
use crate::audio::live::ornament::{self, Harm, Orn, Orns};
use crate::audio::live::voice::{NoteEvent, Sound};
use crate::audio::mml::Arp;

/// The comp's range.
pub const LO: i32 = 43;
pub const HI: i32 = 86;

/// Pulse 2: chords.
pub struct Comp {
    pub(super) p: Player,
    src: Vec<NoteEvent>,
    dst: Vec<NoteEvent>,
    last: Option<NoteEvent>,
    /// The middle of the written part's register (where voicings sit).
    center: u8,
}

impl Comp {
    pub(super) fn new(p: Player) -> Self {
        Comp { p, src: work(), dst: work(), last: None, center: 64 }
    }
}

/// `arp` moved by `semis`.
fn shift(a: Arp, semis: i32) -> Arp {
    let mut n = [0u8; Arp::MAX];
    let src = a.notes();
    for (d, &x) in n.iter_mut().zip(src) {
        *d = (x as i32 + semis).clamp(0, 127) as u8;
    }
    Arp::new(&n[..src.len()])
}

impl Musician for Comp {
    musician_common!();

    fn commit_next_bar(&mut self, ctx: &Ctx, out: &mut Vec<NoteEvent>) -> Orns {
        let mut fired = Orns::default();
        self.src.clear();
        ctx.written(1, &mut self.src);
        // Where the written part sits.
        let (sum, count) = self.src.iter().flat_map(|e| e.sound.notes()).fold((0u32, 0u32), |(s, c), &n| (s + n as u32, c + 1));
        if count > 0 {
            self.center = ((sum / count) as u8).clamp(57, 72);
        }
        if self.p.freedom <= 0.0 {
            out.extend(self.src.iter().copied());
            self.last = self.src.last().copied().or(self.last);
            return fired;
        }
        let intent = self.p.intent(ctx.bar.index);
        let orns = intent.orns;
        let band = ctx.band;
        let bb = ctx.bb();
        let center = self.center;
        let phrase_last = self.p.phrase_last(ctx.bar.index);
        let tmpl = ctx.template(1, self.src.first().copied().or(self.last));
        let mut r = ctx.rng(Role::Comp, 3);
        self.dst.clear();
        let stab = |b: f64, d: f64, a: Arp| ctx.make(&tmpl, b, d, Sound::Arp(a));
        if band.trade == Trade::Drums {
            fired.add(Orn::LayOut);
        } else {
            let h0 = ctx.harm_at(0.0);
            let whole = h0.is_some_and(|h| (1..(bb * 2.0) as usize).all(|k| ctx.harm_at(k as f64 * 0.5).is_some_and(|x| x.chord == h.chord)));
            // The bar's rhythm.
            if h0.is_some() && (orns.has(Orn::Charleston) || orns.has(Orn::FreddieGreen)) {
                let charleston = orns.has(Orn::Charleston);
                let hits: &[(f64, f64)] = if charleston { &[(0.0, 0.6), (1.5, 0.45)] } else { &[(0.0, 0.4), (1.0, 0.4), (2.0, 0.4), (3.0, 0.4)] };
                let mut inv = 0;
                let mut prev: Option<Harm> = None;
                for &(b, d) in hits.iter().filter(|(b, _)| *b < bb - 1e-9) {
                    let Some(h) = ctx.harm_at(b) else { continue };
                    if orns.has(Orn::MovingVoicing) && prev.is_some_and(|p| p.chord == h.chord) {
                        inv += 1;
                        fired.add(Orn::MovingVoicing);
                    } else {
                        inv = 0;
                    }
                    let mut e = stab(ctx.swing8(b), d, ornament::voice(&h.chord, center, inv));
                    if !charleston && (b as usize) % 2 == 1 {
                        e.volume = (e.volume + 1).min(15);
                    }
                    push(&mut self.dst, e);
                    prev = Some(h);
                }
                fired.add(if charleston { Orn::Charleston } else { Orn::FreddieGreen });
            } else if let Some(h) = h0.filter(|_| orns.has(Orn::Fourths) && whole) {
                // McCoy Tyner: stacks of fourths climbing the mode on the off-beats.
                let offbeats = bb.floor() as usize;
                match planing_mode(&h.chord) {
                    Some(mode) => {
                        for (k, shape) in planing_run(&h.chord, mode, offbeats).iter().enumerate() {
                            let mut e = stab(ctx.swing8(k as f64 + 0.5), 0.45, Arp::new(shape));
                            e.volume = (e.volume + 1).min(15);
                            e.duty = 1;
                            push(&mut self.dst, e);
                        }
                    }
                    None => {
                        let mut n = h.scale.snap(center.saturating_sub(5));
                        for k in 0..offbeats {
                            push(&mut self.dst, stab(ctx.swing8(k as f64 + 0.5), 0.45, ornament::fourths(n)));
                            n = h.scale.step(n, 1);
                        }
                    }
                }
                fired.add(Orn::Fourths);
            } else {
                self.dst.extend(self.src.iter().copied());
                // Moving voicings: the same chord struck again, re-voiced.
                if orns.has(Orn::MovingVoicing) {
                    let mut inv = 0;
                    for k in 1..self.dst.len() {
                        let (a, b) = (self.dst[k - 1], self.dst[k]);
                        if let (Sound::Arp(x), Sound::Arp(y)) = (a.sound, b.sound)
                            && x == y
                            && let Some(h) = ctx.harm_at(ctx.rel(&b))
                        {
                            inv += 1;
                            self.dst[k].sound = Sound::Arp(ornament::voice(&h.chord, center, inv));
                            fired.add(Orn::MovingVoicing);
                        }
                    }
                }
            }
            // The band's reharmonization.
            for sub in band.subs.iter().flatten() {
                let h = Harm::new(sub.chord);
                let mut struck = false;
                for e in &mut self.dst {
                    let b = ctx.rel(e);
                    if b >= sub.from - 1e-9 && b < sub.to - 1e-9 {
                        e.sound = match e.sound {
                            Sound::Arp(_) => Sound::Arp(ornament::voice(&sub.chord, center, 0)),
                            Sound::Note(n) => Sound::Note(h.nearest_chord_tone(n)),
                            s => s,
                        };
                        struck |= (b - sub.from).abs() < 1e-6;
                    }
                }
                if !struck {
                    cut_at(ctx, &mut self.dst, sub.from);
                    push(&mut self.dst, stab(sub.from, 0.6, ornament::voice(&sub.chord, center, 0)));
                }
                fired.add(Orn::Reharm);
            }
            // Polychords: an upper-structure triad over the chord's guide tones.
            if orns.has(Orn::Polychord) {
                for e in &mut self.dst {
                    if let Sound::Arp(_) = e.sound
                        && let Some(h) = ctx.harm_at(ctx.rel(e))
                        && matches!(h.chord.family(), Family::Dominant | Family::Major)
                    {
                        e.sound = Sound::Arp(ornament::upper_structure(&h.chord, center + 2));
                        fired.add(Orn::Polychord);
                    }
                }
            }
            // Into the next phrase: chromatic planing up to its chord, or a side-slip above it.
            let next = ctx.plain_harm_at(bb);
            if phrase_last
                && band.hits == 0
                && band.sub_at(bb - 0.5).is_none()
                && let Some(nh) = next
            {
                let target = ornament::voice(&nh.chord, center, 0);
                if orns.has(Orn::PlaneChords) {
                    clear_from(ctx, &mut self.dst, bb - 1.5);
                    for (k, semis) in [-3, -2, -1].into_iter().enumerate() {
                        push(&mut self.dst, stab(bb - 1.5 + k as f64 * 0.5, 0.4, shift(target, semis)));
                    }
                    fired.add(Orn::PlaneChords);
                } else if orns.has(Orn::SlipVoicing) {
                    clear_from(ctx, &mut self.dst, bb - 0.5);
                    push(&mut self.dst, stab(ctx.swing8(bb - 0.5), 0.45, shift(target, if r.chance(0.5) { 1 } else { -1 })));
                    fired.add(Orn::SlipVoicing);
                }
            }
            // An anticipated downbeat isn't struck again.
            if band.anticipated
                && let Some(i) = self.dst.iter().position(|e| ctx.rel(e).abs() < 1e-6 && matches!(e.sound, Sound::Arp(_)))
            {
                self.dst.remove(i);
            }
            // The band's hits.
            let last_hit = band.hit_beats().last();
            for hb in band.hit_beats() {
                let h = if band.hit_kind == HitKind::Anticipation { ctx.plain_harm_at(bb) } else { ctx.harm_at(hb) };
                let Some(h) = h else { continue };
                let d = if band.hit_kind == HitKind::Anticipation { bb - hb } else { 0.5 };
                clear_span(ctx, &mut self.dst, hb, hb + d);
                let mut e = stab(hb, d, ornament::voice(&h.chord, center, 0));
                e.volume = (e.volume + 1).min(15);
                push(&mut self.dst, e);
                fired.add(Orn::Hit);
                if band.hit_kind == HitKind::Anticipation {
                    fired.add(Orn::Anticipation);
                }
            }
            if band.hit_kind == HitKind::Ending
                && let Some(lh) = last_hit
            {
                clear_from(ctx, &mut self.dst, lh + 0.5);
            }
            // The odd extra stab in a gap.
            if orns.has(Orn::ExtraStab) && band.hits == 0 {
                let spots = [1.5, 0.5, 2.5];
                let k0 = r.below(3);
                for j in 0..3 {
                    let p = spots[(k0 + j) % 3];
                    if p + 0.5 > bb {
                        continue;
                    }
                    let busy = self.dst.iter().any(|e| ctx.rel(e) < p + 0.5 - 1e-6 && ctx.rel(e) + ctx.len(e) > p + 1e-6);
                    if !busy && let Some(h) = ctx.harm_at(p) {
                        let mut e = stab(ctx.swing8(p), 0.4, ornament::voice(&h.chord, center, r.below(3)));
                        e.volume = e.volume.saturating_sub(1).max(3);
                        push(&mut self.dst, e);
                        fired.add(Orn::ExtraStab);
                        break;
                    }
                }
            }
            // Room for the drums' big fill at a phrase end.
            if phrase_last && band.hits == 0 && matches!(band.fill, Fill::Full | Fill::PressRoll) && !fired.has(Orn::PlaneChords) {
                clear_from(ctx, &mut self.dst, bb - 2.0);
                fired.add(Orn::LayOut);
            }
        }
        if intent.switch
            && !self.dst.is_empty()
            && let Some(alt) = ctx.alternate(1, self.p.plan.map_or(0, |p| p.start), tmpl.inst)
        {
            for e in &mut self.dst {
                e.inst = alt;
            }
            fired.add(Orn::Switch);
        }
        for e in &mut self.dst {
            e.sound = fold_sound(e.sound, LO, HI);
        }
        tidy(&mut self.dst);
        out.extend(self.dst.iter().copied());
        self.last = self.dst.last().copied().or(self.last);
        fired
    }

    fn on_input(&mut self, input: &Input, next_bar: u64) {
        self.p.on_input(input, next_bar);
    }
}
