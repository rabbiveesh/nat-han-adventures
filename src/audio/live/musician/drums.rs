//! The noise channel: the kit, and how the drummer plays it.
//!
//! On the written groove the drummer adds ghost snares and an open hat on an "and" (low),
//! kicks with the band's hits, crashes after a fill or on a summon, and plays the band's fills
//! (short, full, press rolls into sections); it breaks the time up on a loose night and solos
//! for its four when the band trades. In a feel ([`crate::audio::live::feel`]) it plays the
//! feel's groove on the feel's kit instead of the written one: the bossa clave on the rim, the
//! samba's batucada (and a cuíca now and then), a rock backbeat, a funk 16th groove.

use super::{Ctx, Musician, PhrasePlan, Player, Role, clear_span, musician_common, push, work};
use crate::audio::accomp::Rng;
use crate::audio::live::band::{Fill, HitKind, Trade, mid};
use crate::audio::live::chorus::EndStep;
use crate::audio::live::engine::Input;
use crate::audio::live::feel::{self, Extra, Feel};
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
        let feel = ctx.feel();
        if self.p.freedom <= 0.0 && feel == Feel::Swing && !ctx.band.arranged() {
            out.extend(self.src.iter().copied());
            self.last = self.src.first().copied().or(self.last);
            return fired;
        }
        let f = self.p.freedom;
        let intent = self.p.intent(ctx.bar.index);
        let band = ctx.band;
        let orns = super::arranged(Role::Drums, band, intent.orns);
        let step = band.ending.map(|e| e.step());
        let bb = ctx.bb();
        let mut tmpl = ctx.template(3, self.src.first().copied().or(self.last));
        if let Some(kit) = ctx.feel_inst(3, false) {
            tmpl.inst = kit;
        }
        let vol = self.src.iter().map(|e| e.volume).max().unwrap_or(tmpl.volume).max(6);
        let mut r = ctx.rng(Role::Drums, 3);
        self.dst.clear();
        if band.tacet || step == Some(EndStep::Final) {
            // Out (the last chord's crash comes with the band's crash below).
            fired.add(Orn::LayOut);
        } else if band.trade == Trade::Drums {
            self.solo(ctx, &tmpl, vol, &mut r);
            fired.add(Orn::Trade);
        } else if feel != Feel::Swing {
            fired.add(self.feel_groove(ctx, &tmpl, vol, &mut r));
        } else if orns.has(Orn::BrokenTime) {
            self.broken(ctx, &tmpl, vol, &mut r);
            fired.add(Orn::BrokenTime);
        } else {
            self.dst.extend(self.src.iter().copied());
        }
        if band.trade != Trade::Drums && feel == Feel::Swing && !band.tacet && step != Some(EndStep::Final) {
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
        // Kicks with the band's hits (an ending figure crashes on its last and stops; a
        // stop-time hit is a kick and a snare, then only the hats on 2 and 4).
        if band.hit_kind == HitKind::Stop
            && let Some(lh) = band.hit_beats().last()
        {
            self.dst.retain(|e| ctx.rel(e) < lh + 0.25 - 1e-6);
            for hb in band.hit_beats() {
                self.dst.retain(|e| (ctx.rel(e) - hb).abs() > 1e-6);
                push(&mut self.dst, hit(ctx, &tmpl, hb, Drum::Kick, vol + 2));
                push(&mut self.dst, hit(ctx, &tmpl, hb, Drum::Snare, vol));
            }
            let mut b = 1.0;
            while b < bb - 1e-9 {
                if b > lh + 0.25 {
                    push(&mut self.dst, hit(ctx, &tmpl, b, Drum::ClosedHat, (vol / 2).max(2)));
                }
                b += 2.0;
            }
            fired.add(Orn::Stop);
        } else if mid(f) > 0.0 {
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
        if band.trade != Trade::Drums && !band.tacet && step != Some(EndStep::Final) {
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
            && feel == Feel::Swing
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
    /// A feel's groove for the bar (what it played).
    fn feel_groove(&mut self, ctx: &Ctx, t: &NoteEvent, vol: u8, r: &mut Rng) -> Orn {
        let bb = ctx.bb();
        let v = |x: i32| (vol as i32 + x).clamp(1, 15) as u8;
        let sixteenths = (bb * 4.0).round() as usize;
        let add = |dst: &mut Vec<NoteEvent>, b: f64, d: Drum, volume: u8| {
            if b < bb - 1e-9 {
                push(dst, hit(ctx, t, b, d, volume));
            }
        };
        match ctx.feel() {
            Feel::Bossa => {
                // Laid back and soft: the shaker on the 8ths (quarters, half-time, in a fast
                // tune), the clave on the rim, a light kick.
                let half = feel::fast(ctx.shape.bpm);
                let step = if half { 1.0 } else { 0.5 };
                for k in 0..(bb / step).round() as usize {
                    let b = k as f64 * step;
                    let up = if half { k % 2 == 1 } else { k % 2 == 1 };
                    add(&mut self.dst, b, Drum::ClosedHat, v(if up { -6 } else { -8 }));
                }
                let (clave, n) = feel::two_bar(feel::BOSSA_CLAVE, ctx.bar.index, ctx.band.feel_since, half);
                for &b in &clave[..n] {
                    add(&mut self.dst, b, Drum::Snare, v(-3));
                }
                let (kick, n) = feel::two_bar(feel::BOSSA_KICK, ctx.bar.index, ctx.band.feel_since, half);
                for (j, &b) in kick[..n].iter().enumerate() {
                    add(&mut self.dst, b, Drum::Kick, v(if j == 0 { -4 } else { -7 }));
                }
                Orn::Clave
            }
            Feel::Samba => {
                // The ganzá on the 16ths (the 8ths in a fast tune), soft but for the "a"; the
                // tamborim's teleco-teco (thinned out when fast), the kick on 2 and 4.
                let fast = feel::fast(ctx.shape.bpm);
                for k in 0..sixteenths {
                    if fast && k % 2 == 1 {
                        continue;
                    }
                    add(&mut self.dst, k as f64 * 0.25, Drum::ClosedHat, v(match (k % 4, fast) {
                        (3, _) | (2, true) => -4,
                        (0, _) => -6,
                        _ => -9,
                    }));
                }
                let tamborim: &[u8] = if fast { &feel::SAMBA_TAMBORIM_FAST } else { &feel::SAMBA_TAMBORIM };
                for &k in tamborim {
                    add(&mut self.dst, k as f64 * 0.25, Drum::Snare, v(if k % 4 == 0 { -2 } else { -4 }));
                }
                for k in 0..bb.round() as usize {
                    if k % 2 == 1 || !fast {
                        add(&mut self.dst, k as f64, Drum::Kick, v(if k % 2 == 1 { 0 } else { -5 }));
                    }
                }
                // The cuíca: "oo-EE", now and then (the second bar of a pair).
                if !feel::first_of_pair(ctx.bar.index, ctx.band.feel_since) && r.chance(0.3) {
                    let at = bb - 1.5;
                    for (j, x) in [Extra::CuicaLo, Extra::CuicaHi].into_iter().enumerate() {
                        let b = at + j as f64 * 0.25;
                        if b < bb - 1e-9 {
                            push(&mut self.dst, NoteEvent { inst: ctx.extra(x), ..hit(ctx, t, b, Drum::Snare, v(-3 + j as i32)) });
                        }
                    }
                }
                Orn::Batucada
            }
            Feel::Rock => {
                // 8th hats, kick on 1 and 3 (and the odd push), the backbeat hard, a crash at a
                // section's start.
                let crash = ctx.bar.song_bar.is_multiple_of(8);
                for k in 0..(bb * 2.0).round() as usize {
                    if k == 0 && crash {
                        add(&mut self.dst, 0.0, Drum::Crash, v(1));
                    } else {
                        add(&mut self.dst, k as f64 * 0.5, Drum::ClosedHat, v(if k % 2 == 0 { -2 } else { -4 }));
                    }
                }
                let push_kick = r.chance(0.4);
                for k in 0..bb.round() as usize {
                    let b = k as f64;
                    if k % 2 == 0 {
                        add(&mut self.dst, b, Drum::Kick, v(1));
                    } else {
                        add(&mut self.dst, b, Drum::Snare, v(3));
                    }
                }
                if push_kick {
                    add(&mut self.dst, 2.5, Drum::Kick, v(-1));
                }
                Orn::Backbeat
            }
            Feel::Funk => {
                // The vamp: 8th hats (the quarters up), the backbeat, one ghost, the kick's
                // riff (the one hard); space between.
                let (vamp, side) = feel::vamp(ctx.seed, ctx.bar.index, ctx.band.feel_since);
                for k in 0..(bb * 2.0).round() as usize {
                    let b = k as f64 * 0.5;
                    if side == 1 && k == 7 {
                        add(&mut self.dst, b, Drum::OpenHat, v(-4));
                    } else {
                        add(&mut self.dst, b, Drum::ClosedHat, v(if k % 2 == 0 { -3 } else { -6 }));
                    }
                }
                for &k in &feel::FUNK_BACKBEAT {
                    add(&mut self.dst, k as f64 * 0.25, Drum::Snare, v(2));
                }
                if side == 1 {
                    add(&mut self.dst, vamp.ghost as f64 * 0.25, Drum::Snare, (vol / 4).max(2));
                }
                for &k in vamp.kick[side] {
                    add(&mut self.dst, k as f64 * 0.25, Drum::Kick, v(if k == 0 { 2 } else { 0 }));
                }
                Orn::FunkGroove
            }
            Feel::Swing => unreachable!("only in a feel"),
        }
    }

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

