//! The noise channel: the kit, and how the drummer plays it.
//!
//! On the written groove the drummer adds ghost snares and an open hat on an "and" (low),
//! kicks with the band's hits, crashes after a fill or on a summon, and plays the band's fills
//! (short, full, press rolls into sections); it breaks the time up on a loose night and solos
//! for its four when the band trades.

use super::{Ctx, Musician, PhrasePlan, Player, Role, clear_span, musician_common, push, work};
use crate::audio::accomp::Rng;
use crate::audio::live::band::{Fill, HitKind, Trade, mid};
use crate::audio::live::engine::Input;
use crate::audio::live::ornament::{Orn, Orns};
use crate::audio::live::voice::{NoteEvent, Sound};
use crate::audio::mml::Drum;

/// Noise: the kit.
pub struct Drums {
    pub(super) p: Player,
    src: Vec<NoteEvent>,
    dst: Vec<NoteEvent>,
    last: Option<NoteEvent>,
}

impl Drums {
    pub(super) fn new(p: Player) -> Self {
        Drums { p, src: work(), dst: work(), last: None }
    }
}

/// A hit of drum `d` at beat `b` of the bar.
fn hit(ctx: &Ctx, t: &NoteEvent, b: f64, d: Drum, volume: u8) -> NoteEvent {
    NoteEvent { volume: volume.clamp(1, 15), ..ctx.make(t, b, 0.25, Sound::Drum(d)) }
}

/// Is something struck within a 16th of beat `b`?
fn struck(ctx: &Ctx, v: &[NoteEvent], b: f64) -> bool {
    v.iter().any(|e| (ctx.rel(e) - b).abs() < 0.2)
}

impl Musician for Drums {
    musician_common!();

    fn commit_next_bar(&mut self, ctx: &Ctx, out: &mut Vec<NoteEvent>) -> Orns {
        let mut fired = Orns::default();
        self.src.clear();
        ctx.written(3, &mut self.src);
        // Dynamics: accent the downbeat.
        if ctx.dynamics > 0.0 {
            let accent = 1.0 + ctx.dynamics * ctx.intensity * 0.3;
            for e in self.src.iter_mut().filter(|e| e.start == ctx.bar.start) {
                e.gain *= accent;
            }
        }
        if self.p.freedom <= 0.0 {
            out.extend(self.src.iter().copied());
            self.last = self.src.first().copied().or(self.last);
            return fired;
        }
        let f = self.p.freedom;
        let intent = self.p.intent(ctx.bar.index);
        let orns = intent.orns;
        let band = ctx.band;
        let bb = ctx.bb();
        let tmpl = ctx.template(3, self.src.first().copied().or(self.last));
        let vol = self.src.iter().map(|e| e.volume).max().unwrap_or(tmpl.volume).max(6);
        let mut r = ctx.rng(Role::Drums, 3);
        self.dst.clear();
        if band.trade == Trade::Drums {
            self.solo(ctx, &tmpl, vol, &mut r);
            fired.add(Orn::Trade);
        } else if orns.has(Orn::BrokenTime) {
            self.broken(ctx, &tmpl, vol, &mut r);
            fired.add(Orn::BrokenTime);
        } else {
            self.dst.extend(self.src.iter().copied());
        }
        if band.trade != Trade::Drums {
            // Ghost snares before the beats.
            if orns.has(Orn::Ghost) {
                let sixteenths = (bb * 4.0) as usize;
                let first = r.below(sixteenths);
                let mut n = 0;
                for j in 0..sixteenths {
                    let k = (first + j) % sixteenths;
                    let b = k as f64 * 0.25;
                    if k % 4 == 3 && !struck(ctx, &self.dst, b) && n < 2 {
                        push(&mut self.dst, hit(ctx, &tmpl, b, Drum::Snare, (vol / 3).max(2)));
                        n += 1;
                    }
                }
                if n > 0 {
                    fired.add(Orn::Ghost);
                }
            }
            // An open hat on an "and".
            if orns.has(Orn::OpenHat) {
                let k0 = r.below(8);
                let ands = self.dst.iter().enumerate().filter(|(_, e)| e.sound == Sound::Drum(Drum::ClosedHat) && ((ctx.rel(e) * 2.0).round() as i64) % 2 == 1).count();
                if ands > 0 {
                    let pick = k0 % ands;
                    if let Some(e) = self.dst.iter_mut().filter(|e| e.sound == Sound::Drum(Drum::ClosedHat) && ((ctx.rel(e) * 2.0).round() as i64) % 2 == 1).nth(pick) {
                        e.sound = Sound::Drum(Drum::OpenHat);
                        fired.add(Orn::OpenHat);
                    }
                }
            }
        }
        // Kicks with the band's hits (an ending figure crashes on its last and stops).
        if mid(f) > 0.0 {
            let last = band.hit_beats().last();
            for hb in band.hit_beats() {
                if !self.dst.iter().any(|e| (ctx.rel(e) - hb).abs() < 1e-6 && e.sound == Sound::Drum(Drum::Kick)) {
                    push(&mut self.dst, hit(ctx, &tmpl, hb, Drum::Kick, vol + 2));
                }
                fired.add(Orn::Hit);
            }
            if band.hit_kind == HitKind::Ending
                && let Some(lh) = last
            {
                self.dst.retain(|e| ctx.rel(e) <= lh + 1e-6 || e.sound == Sound::Drum(Drum::ClosedHat));
                push(&mut self.dst, hit(ctx, &tmpl, lh, Drum::Crash, vol + 1));
            }
        }
        // A crash on the one: after a fill, a summon, a checkpoint's cue.
        if band.crash || intent.accent {
            self.dst.retain(|e| ctx.rel(e).abs() > 1e-6 || e.sound == Sound::Drum(Drum::Kick));
            push(&mut self.dst, hit(ctx, &tmpl, 0.0, Drum::Crash, vol + 2));
            if !self.dst.iter().any(|e| ctx.rel(e).abs() < 1e-6 && e.sound == Sound::Drum(Drum::Kick)) {
                push(&mut self.dst, hit(ctx, &tmpl, 0.0, Drum::Kick, vol + 2));
            }
            fired.add(Orn::Crash);
        }
        // The fill.
        let fill = if intent.short_fill && band.fill == Fill::None { Fill::Short } else { band.fill };
        if band.trade != Trade::Drums {
            match fill {
                Fill::None => {}
                Fill::Short => {
                    let from = bb - 1.0;
                    clear_span(ctx, &mut self.dst, from, bb);
                    let pat: [Drum; 4] = if r.chance(0.5) { [Drum::Snare, Drum::Snare, Drum::Kick, Drum::Snare] } else { [Drum::Snare, Drum::Kick, Drum::Snare, Drum::Snare] };
                    for (j, d) in pat.into_iter().enumerate() {
                        push(&mut self.dst, hit(ctx, &tmpl, from + j as f64 * 0.25, d, vol - 1 + j as u8));
                    }
                    fired.add(Orn::ShortFill);
                }
                Fill::Full => {
                    self.fill(ctx, &tmpl, vol, &mut r);
                    fired.add(Orn::Fill);
                }
                Fill::PressRoll => {
                    let from = bb - 2.0;
                    clear_span(ctx, &mut self.dst, from, bb);
                    for j in 0..16 {
                        let v = (vol as i32 - 6 + j * 9 / 15) as u8;
                        let d = if j == 15 { Drum::Kick } else { Drum::Snare };
                        push(&mut self.dst, NoteEvent { volume: v.clamp(1, 15), ..ctx.make(&tmpl, from + j as f64 * 0.125, 0.125, Sound::Drum(d)) });
                    }
                    fired.add(Orn::PressRoll);
                }
            }
        }
        if intent.switch
            && (fired.has(Orn::Fill) || fired.has(Orn::Trade))
            && let Some(alt) = ctx.alternate(3, self.p.plan.map_or(0, |p| p.start), tmpl.inst)
        {
            for e in &mut self.dst {
                e.inst = alt;
            }
            fired.add(Orn::Switch);
        }
        sort(&mut self.dst);
        out.extend(self.dst.iter().copied());
        self.last = self.dst.first().copied().or(self.last);
        fired
    }

    fn on_input(&mut self, input: &Input, next_bar: u64) {
        self.p.on_input(input, next_bar);
        if matches!(input, Input::Checkpoint) {
            // A short fill now (the band crashes after it).
            self.p.cue(next_bar, super::BarIntent { short_fill: true, ..Default::default() });
        }
    }
}

impl Drums {
    /// A full fill over the last two beats: a snare roll, snare and kick 16ths, or triplets.
    fn fill(&mut self, ctx: &Ctx, t: &NoteEvent, vol: u8, r: &mut Rng) {
        let bb = ctx.bb();
        let from = bb - 2.0;
        clear_span(ctx, &mut self.dst, from, bb);
        match r.below(3) {
            0 => {
                for j in 0..8 {
                    let d = if j == 7 { Drum::Kick } else { Drum::Snare };
                    push(&mut self.dst, hit(ctx, t, from + j as f64 * 0.25, d, vol - 3 + (j * 3 / 4) as u8));
                }
            }
            1 => {
                let pat = [Drum::Snare, Drum::Kick, Drum::Snare, Drum::Kick, Drum::Snare, Drum::Snare, Drum::Kick, Drum::Snare];
                for (j, d) in pat.into_iter().enumerate() {
                    push(&mut self.dst, hit(ctx, t, from + j as f64 * 0.25, d, vol - 2 + (j / 2) as u8));
                }
            }
            _ => {
                let pat = [Drum::Snare, Drum::Snare, Drum::Snare, Drum::Kick, Drum::Snare, Drum::Snare];
                for (j, d) in pat.into_iter().enumerate() {
                    push(&mut self.dst, NoteEvent { volume: (vol - 2 + (j / 2) as u8).min(15), ..ctx.make(t, from + j as f64 / 3.0, 1.0 / 3.0, Sound::Drum(d)) });
                }
            }
        }
    }

    /// The drums' four: a bar of solo, 16ths of snare and kick with the hats in the gaps.
    fn solo(&mut self, ctx: &Ctx, t: &NoteEvent, vol: u8, r: &mut Rng) {
        let sixteenths = (ctx.bb() * 4.0) as usize;
        for k in 0..sixteenths {
            if !r.chance(0.72) && k % 4 != 0 {
                continue;
            }
            let x = r.f();
            let d = if k % 4 == 0 && x < 0.4 {
                Drum::Kick
            } else if x < 0.65 {
                Drum::Snare
            } else if x < 0.85 {
                Drum::Kick
            } else {
                Drum::ClosedHat
            };
            let accent = if k % 4 == 0 { 2 } else if k % 2 == 0 { 0 } else { -2 };
            push(&mut self.dst, hit(ctx, t, k as f64 * 0.25, d, (vol as i32 + accent).clamp(2, 15) as u8));
        }
    }

    /// Broken time: a ride-ish hat line with gaps, snare and kick "bombs" off the beat.
    fn broken(&mut self, ctx: &Ctx, t: &NoteEvent, vol: u8, r: &mut Rng) {
        let bb = ctx.bb();
        // Keep the written kicks on the beats.
        for e in &self.src {
            if e.sound == Sound::Drum(Drum::Kick) && (ctx.rel(e) - ctx.rel(e).round()).abs() < 1e-6 {
                push(&mut self.dst, *e);
            }
        }
        let eighths = (bb * 2.0) as usize;
        for k in 0..eighths {
            let b = ctx.swing8(k as f64 * 0.5);
            if k % 2 == 0 || r.chance(0.55) {
                push(&mut self.dst, hit(ctx, t, b, Drum::ClosedHat, if k % 4 == 2 { vol } else { vol - 2 }));
            }
        }
        for _ in 0..2 {
            let k = 2 * r.below(eighths / 2) + 1;
            let b = ctx.swing8(k as f64 * 0.5);
            let d = if r.chance(0.6) { Drum::Snare } else { Drum::Kick };
            push(&mut self.dst, hit(ctx, t, b, d, vol + 1));
        }
    }
}

/// Sort by start (stable insertion sort: no allocation).
fn sort(v: &mut [NoteEvent]) {
    for i in 1..v.len() {
        let mut j = i;
        while j > 0 && v[j - 1].start > v[j].start {
            v.swap(j - 1, j);
            j -= 1;
        }
    }
}

