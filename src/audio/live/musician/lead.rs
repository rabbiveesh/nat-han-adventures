//! Pulse 1: the tune, and how the lead ornaments it.
//!
//! Each bar the lead takes its written notes (in the current harmony) and, by the ornaments
//! its plan rolled for the bar:
//! - replaces the bar outright when the band or the game says so: lays out for the drums'
//!   four, plays the wah-wah after a death, or solos (trading fours, a solo chorus);
//! - or transforms the whole bar (high freedom): planing, a digital pattern, pentatonic
//!   superimposition over a dominant, a hemiola, an 8th of displacement; or (mid) side-slips
//!   half a bar a semitone off (planing and side-slips over a long dominant only);
//! - or decorates note by note: an octave displacement, a turn, a mordent or a grace note on a
//!   long note, an enclosure into a note after a rest, an arpeggio flourish; vibrato, a slide
//!   or a duty sweep on the note itself; an echo into the rest after it;
//! - then, at a phrase end, fills the space before the next phrase (a scale run, triplet
//!   arpeggios, a swung pickup) or falls off the last note, and makes sure the phrase lands on
//!   a chord tone (or leads by step into the next phrase).
//!
//! In a feel ([`crate::audio::live::feel`]) the written line is played straight (un-swung,
//! and every ornament with it) on the feel's instrument: a notch softer in a bossa, with
//! bluesy bends in rock, punchy short notes and horn stabs at its phrase ends in funk.

use super::{Ctx, Musician, PhrasePlan, Player, Role, fold_sound, musician_common, push, tidy, work};
use crate::audio::chart::{Chord, Quality};
use crate::audio::live::band::{Flourish, Trade};
use crate::audio::live::chorus::{Chorus, EndKind, EndStep};
use crate::audio::live::engine::Input;
use crate::audio::live::feel::Feel;
use crate::audio::live::ornament::{self, Harm, Orn, Orns, Plane, Scale};
use crate::audio::live::voice::{NoteEvent, Sound};
use crate::audio::mml::Arp;

/// The lead's range.
pub const LO: i32 = 48;
pub const HI: i32 = 96;

/// Pulse 1: the tune.
pub struct Lead {
    pub(super) p: Player,
    src: Vec<NoteEvent>,
    dst: Vec<NoteEvent>,
    /// The last event committed (a template for a bar with nothing written).
    last: Option<NoteEvent>,
    /// Where the last committed note ended (absolute sample).
    end: u64,
    /// Where an improvised line left off.
    solo_note: Option<u8>,
}

impl Lead {
    pub(super) fn new(p: Player) -> Self {
        Lead { p, src: work(), dst: work(), last: None, end: 0, solo_note: None }
    }
}

/// Is `note` (at beat `b` of the bar) one of the written notes there?
fn as_written(src: &[NoteEvent], ctx: &Ctx, b: f64, note: u8) -> bool {
    src.iter().any(|e| (ctx.rel(e) - b).abs() < 1e-6 && e.sound == Sound::Note(note))
}

impl Musician for Lead {
    musician_common!();

    fn commit_next_bar(&mut self, ctx: &Ctx, out: &mut Vec<NoteEvent>) -> Orns {
        let mut fired = Orns::default();
        self.src.clear();
        ctx.written(0, &mut self.src);
        let feel = ctx.feel();
        if self.p.freedom <= 0.0 && feel == Feel::Swing && !ctx.band.arranged() {
            out.extend(self.src.iter().copied());
            self.remember(ctx);
            return fired;
        }
        if feel != Feel::Swing {
            for e in &mut self.src {
                ctx.straighten(e);
            }
        }
        let mut intent = self.p.intent(ctx.bar.index);
        let band = ctx.band;
        intent.orns = super::arranged(Role::Lead, band, intent.orns);
        let phrase_last = self.p.phrase_last(ctx.bar.index);
        let tmpl = ctx.template(0, self.src.first().copied().or(self.last));
        let mut r = ctx.rng(Role::Lead, 3);
        self.dst.clear();
        let mut replaced = true;
        let f = self.p.freedom;
        let step = band.ending.map(|e| e.step());
        let solos =
            band.trade == Trade::Lead || intent.orns.has(Orn::Solo) || band.chorus.lead_solos(f) || band.solo_break || (step == Some(EndStep::Time) && f > 0.0);
        if band.trade == Trade::Drums || step == Some(EndStep::Plinks) || band.intro.is_some_and(|i| !i.last()) || (step == Some(EndStep::Time) && f <= 0.0) {
            fired.add(Orn::LayOut);
        } else if band.intro.is_some() {
            // The intro's last bar: a pickup into the head.
            self.pickup(ctx, &tmpl);
            fired.add(Orn::Pickup);
        } else if step == Some(EndStep::Final) {
            self.final_note(ctx, &tmpl);
            fired.add(Orn::LastChord);
        } else if band.flourish == Flourish::Death || intent.wah {
            wah_wah(ctx, &tmpl, &mut self.dst);
            fired.add(Orn::WahWah);
        } else if solos && ctx.plain_harm_at(0.0).is_some() {
            self.solo(ctx, &tmpl, &mut r, phrase_last && !band.solo_break);
            fired.add(Orn::Solo);
            if band.solo_break {
                if band.tacet {
                    self.break_run(ctx, &tmpl);
                }
                fired.add(Orn::Break);
            }
        } else if let Some(o) = self.whole_bar(ctx, intent.orns, &mut r) {
            fired.add(o);
            replaced = false;
        } else {
            self.note_by_note(ctx, intent.orns, &mut r, &mut fired);
            replaced = false;
        }
        tidy(&mut self.dst);
        if !replaced && feel != Feel::Swing {
            self.feel_line(ctx, &mut r, &mut fired);
        }
        if !replaced && band.chorus == Chorus::Shout {
            self.shout_line(ctx);
            fired.add(Orn::ShoutLine);
        }
        if !replaced && phrase_last {
            if feel == Feel::Funk {
                self.horn_stabs(ctx, &mut fired);
            } else {
                self.phrase_end(ctx, intent.orns, &tmpl, &mut r, &mut fired);
            }
        }
        if intent.answer && !replaced {
            self.answer(ctx, &tmpl, &mut fired);
        }
        if phrase_last && (!replaced || fired.has(Orn::Solo)) {
            self.resolve(ctx);
        }
        // An instrument for the phrase (or a solo / a fill on a loose night).
        let fill_bar = fired.has(Orn::Solo) || fired.has(Orn::RunFill);
        if let Some(k) = ctx.feel_inst(0, intent.switch) {
            for e in &mut self.dst {
                e.inst = k;
            }
        } else if (intent.switch || fill_bar && ornament_switch(&mut r, self.p.freedom))
            && let Some(alt) = ctx.alternate(0, self.p.plan.map_or(0, |p| p.start), tmpl.inst)
            && !self.dst.is_empty()
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
        self.src.clear();
        self.src.extend(self.dst.iter().copied());
        self.remember(ctx);
        fired
    }

    fn on_input(&mut self, input: &Input, next_bar: u64) {
        self.p.on_input(input, next_bar);
        match input {
            // Answer the toot in the next bar.
            Input::Toot => self.p.cue(next_bar, super::BarIntent { answer: true, ..Default::default() }),
            // The comic wah-wah.
            Input::Death => self.p.cue(next_bar, super::BarIntent { wah: true, ..Default::default() }),
            Input::LevelStart | Input::Restart => self.solo_note = None,
            _ => {}
        }
    }
}

/// A fill bar may switch instrument too (high freedom).
fn ornament_switch(r: &mut crate::audio::accomp::Rng, f: f32) -> bool {
    r.chance(0.5 * crate::audio::live::band::high(f))
}

impl Lead {
    /// Keep what the next bar needs: the last event, where it ends, the last note.
    fn remember(&mut self, ctx: &Ctx) {
        if let Some(e) = self.src.last() {
            self.last = Some(*e);
            self.end = self.src.iter().map(|e| e.end).max().unwrap_or(0);
            if let Some(&n) = e.sound.notes().last() {
                self.solo_note = Some(n);
            }
        } else if self.end < ctx.bar.start {
            self.end = 0;
        }
    }

    /// Beats of this bar the previous bar's last note still covers.
    fn carry(&self, ctx: &Ctx) -> f64 {
        if self.end > ctx.bar.start { ctx.beat_of(self.end) } else { 0.0 }
    }

    /// A whole-bar transformation, if one was planned and fits the bar.
    fn whole_bar(&mut self, ctx: &Ctx, orns: Orns, r: &mut crate::audio::accomp::Rng) -> Option<Orn> {
        let bb = ctx.bb();
        let notes = self.src.iter().filter(|e| matches!(e.sound, Sound::Note(_))).count();
        if notes == 0 {
            return None;
        }
        let first = *self.src.iter().find(|e| matches!(e.sound, Sound::Note(_)))?;
        let center = first.sound.notes()[0];
        let h0 = ctx.harm_at(0.0);
        let fits_bar = self.src.iter().all(|e| ctx.rel(e) + ctx.len(e) <= bb + 1e-6) && ctx.rel(&first) >= -1e-9;
        // (Planing rubs against the chord: over a dominant the whole bar long only.)
        if orns.has(Orn::Planing) && ctx.long_dominant(0.0, bb) {
            let kind = [Plane::Fourths, Plane::Triad, Plane::Cluster][r.below(3)];
            for k in 0..self.src.len() {
                let mut e = self.src[k];
                if let Sound::Note(n) = e.sound {
                    let scale = scale_at(ctx, ctx.rel(&e));
                    e.sound = Sound::Arp(ornament::plane(n, kind, &scale));
                }
                push(&mut self.dst, e);
            }
            return Some(Orn::Planing);
        }
        let sparse = notes <= 4 && fits_bar;
        if orns.has(Orn::Digital) && sparse && h0.is_some() {
            let floor = center.saturating_sub(5).max(LO as u8);
            let halves = (bb / 2.0).floor() as usize;
            let mut down = r.chance(0.5);
            let mut k = 0;
            for half in 0..halves.max(1) {
                let b0 = half as f64 * 2.0;
                let Some(h) = ctx.harm_at(b0) else { break };
                let pat = ornament::digital(&h, floor, down);
                for (j, &n) in pat.iter().enumerate() {
                    let b = b0 + j as f64 * 0.5;
                    self.eighth(ctx, &first, b, bb, Sound::Note(n));
                    k += 1;
                }
                down = !down;
            }
            // An odd beat left (3/4): the chord tone nearest, held.
            let tail = halves as f64 * 2.0;
            if tail < bb - 1e-9
                && let Some(h) = ctx.harm_at(tail)
            {
                let n = h.nearest_chord_tone(center);
                push(&mut self.dst, ctx.make(&first, tail, bb - tail, Sound::Note(n)));
            }
            let _ = k;
            return Some(Orn::Digital);
        }
        if orns.has(Orn::Pentatonic)
            && sparse
            && let Some(h) = h0
            && h.chord.family() == crate::audio::chart::Family::Dominant
        {
            // The dominant's span in the bar (at least two beats).
            let span = (1..(bb * 2.0) as usize).map(|k| k as f64 * 0.5).find(|&b| ctx.harm_at(b).is_some_and(|x| x.chord != h.chord)).unwrap_or(bb);
            if span >= 2.0 - 1e-9 {
                let pcs = ornament::pentatonic_up_half(h.chord.root);
                let mut n = ornament::next_in(&pcs, center + 4, -1);
                let mut dir = -1;
                let count = (span * 2.0).round() as usize;
                for j in 0..count {
                    let b = j as f64 * 0.5;
                    self.eighth(ctx, &first, b, span, Sound::Note(n));
                    let next = ornament::next_in(&pcs, n, dir);
                    if (next as i32) < LO + 10 || (next as i32) > HI - 10 {
                        dir = -dir;
                    }
                    n = ornament::next_in(&pcs, n, dir);
                }
                // Resolve: the next chord's nearest chord tone, held to the bar's end.
                if span < bb - 1e-9
                    && let Some(hn) = ctx.harm_at(span)
                {
                    let t = hn.nearest_chord_tone(n);
                    push(&mut self.dst, ctx.make(&first, span, bb - span, Sound::Note(t)));
                }
                return Some(Orn::Pentatonic);
            }
        }
        if orns.has(Orn::Hemiola) && fits_bar {
            // A three-note cell in 8ths across the bar's grouping of two.
            let mut cell = [center; 3];
            let mut m = 0;
            for e in &self.src {
                if let Sound::Note(n) = e.sound
                    && m < 3
                    && (m == 0 || cell[m - 1] != n)
                {
                    cell[m] = n;
                    m += 1;
                }
            }
            if m < 3
                && let Some(h) = h0
            {
                while m < 3 {
                    cell[m] = h.chord_tone_above(cell[m - 1] + 1);
                    m += 1;
                }
            }
            let count = (bb * 2.0).round() as usize;
            for j in 0..count {
                let b = j as f64 * 0.5;
                let start = self.dst.len();
                self.eighth(ctx, &first, b, bb, Sound::Note(cell[j % 3]));
                if j % 3 == 0
                    && let Some(e) = self.dst.get_mut(start)
                {
                    e.volume = (e.volume + 2).min(15);
                }
            }
            return Some(Orn::Hemiola);
        }
        if orns.has(Orn::Displace) && fits_bar && ctx.rel(&first) < 0.5 {
            for k in 0..self.src.len() {
                let e = self.src[k];
                let b = ctx.rel(&e) + 0.5;
                if b >= bb - 0.2 {
                    continue;
                }
                let d = ctx.len(&e).min(bb - b);
                push(&mut self.dst, NoteEvent { tie: e.tie && k > 0, ..ctx.make(&e, b, d, e.sound) });
            }
            return Some(Orn::Displace);
        }
        // Half a bar a semitone off, snapping back at the half: only over a long dominant (one
        // chord, a dominant, all through a half of at least two beats), where the tension is
        // the chord's own.
        let half = (bb / 2.0).floor();
        let has_notes = |from: f64, to: f64| self.src.iter().any(|e| (from - 1e-9..to - 1e-9).contains(&ctx.rel(e)) && matches!(e.sound, Sound::Note(_)));
        if orns.has(Orn::SideSlip)
            && let Some((from, to)) = [(0.0, half), (half, bb)].into_iter().find(|&(from, to)| ctx.long_dominant(from, to) && has_notes(from, to))
        {
            let dir: i32 = if r.chance(0.6) { 1 } else { -1 };
            for k in 0..self.src.len() {
                let mut e = self.src[k];
                let b = ctx.rel(&e);
                if b >= from - 1e-9
                    && b < to - 1e-9
                    && let Sound::Note(n) = e.sound
                {
                    e.sound = Sound::Note((n as i32 + dir).clamp(0, 127) as u8);
                }
                push(&mut self.dst, e);
            }
            return Some(Orn::SideSlip);
        }
        None
    }

    /// An 8th note at beat `b` (on the 8th grid, swung like the song), cut at `until`.
    fn eighth(&mut self, ctx: &Ctx, t: &NoteEvent, b: f64, until: f64, sound: Sound) {
        let s = ctx.swing8(b);
        let e = ctx.swing8(b + 0.5).min(until);
        if e > s + 1e-6 {
            push(&mut self.dst, ctx.make(t, s, e - s, sound));
        }
    }

    /// Ornaments note by note (low and mid freedom: the tune stays the tune).
    fn note_by_note(&mut self, ctx: &Ctx, orns: Orns, r: &mut crate::audio::accomp::Rng, fired: &mut Orns) {
        let bb = ctx.bb();
        let n = self.src.len();
        let carry = self.carry(ctx);
        let mut octave_done = false;
        let mut prev_note: Option<u8> = self.last.and_then(|e| e.sound.notes().last().copied());
        for k in 0..n {
            let e = self.src[k];
            let (b, d) = (ctx.rel(&e), ctx.len(&e));
            let prev_end = if k == 0 { carry } else { ctx.rel(&self.src[k - 1]) + ctx.len(&self.src[k - 1]) };
            let gap_before = b - prev_end;
            let next_start = if k + 1 < n { ctx.rel(&self.src[k + 1]) } else { bb.max(b + d) };
            let gap_after = next_start - (b + d);
            let Sound::Note(note) = e.sound else {
                push(&mut self.dst, e);
                prev_note = e.sound.notes().last().copied();
                continue;
            };
            let h = ctx.harm_at(b);
            let scale = scale_at(ctx, b);
            let on_grid = ((b * 2.0).round() - b * 2.0).abs() < 1e-6;
            let mut main = e;
            // Effects on the note itself.
            if orns.has(Orn::Vibrato) && d >= 1.25 {
                main.fx.vib = 24;
                fired.add(Orn::Vibrato);
            }
            let leap = prev_note.is_some_and(|p| (p as i32 - note as i32).abs() >= 5);
            if orns.has(Orn::Slide) && !e.tie && (gap_before >= 0.5 || leap) && r.chance(0.6) {
                main.fx.slide = if r.chance(0.75) { -2 } else { 1 };
                main.fx.slide_frames = 4;
                fired.add(Orn::Slide);
            }
            if orns.has(Orn::DutySweep) && d >= 2.0 && r.chance(0.7) {
                main.fx.sweep = 3;
                fired.add(Orn::DutySweep);
            }
            // At most one ornament on the note.
            let mut done = false;
            if !e.tie {
                if orns.has(Orn::Octave) && !octave_done && k > 0 && k + 1 < n && d >= 0.5 && r.chance(0.5) {
                    let up = (note as i32) + 12 <= HI - 4;
                    main.sound = Sound::Note(if up { note + 12 } else { note.saturating_sub(12) });
                    octave_done = true;
                    fired.add(Orn::Octave);
                } else if orns.has(Orn::Turn) && d >= 1.0 && on_grid && r.chance(0.6) {
                    let t = ornament::turn(note, &scale);
                    for (j, &x) in t[..3].iter().enumerate() {
                        push(&mut self.dst, ctx.make(&e, b + j as f64 / 6.0, 1.0 / 6.0, Sound::Note(x)));
                    }
                    push(&mut self.dst, NoteEvent { tie: true, ..ctx.make(&main, b + 0.5, d - 0.5, main.sound) });
                    fired.add(Orn::Turn);
                    done = true;
                } else if orns.has(Orn::Mordent) && d >= 0.75 && r.chance(0.6) {
                    let m = ornament::mordent(note, &scale, r.chance(0.5));
                    push(&mut self.dst, ctx.make(&e, b, 0.125, Sound::Note(m[0])));
                    push(&mut self.dst, NoteEvent { tie: true, ..ctx.make(&e, b + 0.125, 0.125, Sound::Note(m[1])) });
                    push(&mut self.dst, NoteEvent { tie: true, ..ctx.make(&main, b + 0.25, d - 0.25, main.sound) });
                    fired.add(Orn::Mordent);
                    done = true;
                } else if orns.has(Orn::Grace) && d >= 0.75 && on_grid && r.chance(0.7) {
                    let g = if r.chance(0.7) { note.saturating_sub(1) } else { scale.step(note, 1) };
                    push(&mut self.dst, ctx.make(&e, b, 0.125, Sound::Note(g)));
                    push(&mut self.dst, NoteEvent { tie: true, ..ctx.make(&main, b + 0.125, d - 0.125, main.sound) });
                    fired.add(Orn::Grace);
                    done = true;
                } else if orns.has(Orn::ArpFlourish)
                    && d >= 1.5
                    && let Some(h) = h
                    && r.chance(0.6)
                {
                    let a = h.chord_tone_above(note + 1);
                    let c = h.chord_tone_above(a + 1);
                    main.sound = Sound::Arp(Arp::new(&[note, a, c]));
                    fired.add(Orn::ArpFlourish);
                }
            }
            // An enclosure into the note, in the rest before it.
            if !done && orns.has(Orn::Enclosure) && !e.tie && gap_before >= 0.5 - 1e-9 && on_grid && b >= 0.5 && d >= 0.5 && r.chance(0.7) {
                let enc = ornament::enclosure(note, &scale);
                push(&mut self.dst, ctx.make(&e, b - 0.5, 0.25, Sound::Note(enc[0])));
                push(&mut self.dst, ctx.make(&e, b - 0.25, 0.25, Sound::Note(enc[1])));
                fired.add(Orn::Enclosure);
            }
            if !done {
                push(&mut self.dst, main);
            }
            // The NES echo: the note again, quietly, an 8th later (into the rest).
            if orns.has(Orn::Echo) && gap_after >= 0.5 - 1e-9 && r.chance(0.8) {
                let at = if d <= 0.5 + 1e-9 { b + 0.5 } else { b + d };
                let len = (next_start - at).min(0.5);
                if len >= 0.2 && at < bb {
                    let mut echo = ctx.make(&e, at, len * 0.9, Sound::Note(note));
                    echo.volume = (e.volume / 3).max(2);
                    push(&mut self.dst, echo);
                    fired.add(Orn::Echo);
                }
            }
            prev_note = Some(note);
        }
    }

    /// The intro's last bar: four swung 8ths up (by scale steps) into the head's first note.
    fn pickup(&mut self, ctx: &Ctx, t: &NoteEvent) {
        let bb = ctx.bb();
        let target = ctx.next_first_note(0).unwrap_or(72);
        let from = bb - 2.0;
        let scale = scale_at(ctx, from);
        for j in 0..4 {
            let n = scale.step(target, -(4 - j as i32));
            self.eighth(ctx, t, from + j as f64 * 0.5, bb, Sound::Note(n));
        }
    }

    /// The last chord: a chord tone near where the line was, held (short for a Basie ending's
    /// "bwah"), with vibrato.
    fn final_note(&mut self, ctx: &Ctx, t: &NoteEvent) {
        let bb = ctx.bb();
        let Some(h) = ctx.harm_at(0.0) else { return };
        let near = self.solo_note.unwrap_or(72).clamp(64, 84);
        let n = h.nearest_chord_tone(near);
        let basie = ctx.band.ending.is_some_and(|e| e.kind == EndKind::Basie);
        let mut e = ctx.make(t, 0.0, if basie { 1.5 } else { bb * 0.95 }, Sound::Note(n));
        e.volume = (e.volume + 1).min(15);
        e.fx.vib = 30;
        push(&mut self.dst, e);
    }

    /// A break's second bar (the band out): the solo's first half, then a 16th-note scale run
    /// into the next phrase's first note.
    fn break_run(&mut self, ctx: &Ctx, t: &NoteEvent) {
        let bb = ctx.bb();
        let from = (bb - 2.0).max(1.0);
        super::clear_from(ctx, &mut self.dst, from);
        let target = ctx.next_first_note(0).or(self.solo_note).unwrap_or(72);
        let scale = scale_at(ctx, from);
        let count = ((bb - from) / 0.25).round() as i32;
        let dir = if self.solo_note.is_some_and(|n| n > target) { -1 } else { 1 };
        let mut n = scale.step(target, -dir * count);
        for j in 0..count {
            push(&mut self.dst, ctx.make(t, from + j as f64 * 0.25, 0.25, Sound::Note(n)));
            n = scale.step(n, dir);
        }
        self.solo_note = Some(n);
    }

    /// The shout chorus: the tune up an octave where it fits, short, punchy and accented.
    fn shout_line(&mut self, ctx: &Ctx) {
        let up = self.dst.iter().filter_map(|e| e.sound.notes().iter().max().copied()).max().is_some_and(|top| top as i32 + 12 <= HI - 2);
        let n = self.dst.len();
        for k in 0..n {
            let next_ties = self.dst.get(k + 1).is_some_and(|x| x.tie);
            let e = &mut self.dst[k];
            if up {
                e.sound = match e.sound {
                    Sound::Note(x) => Sound::Note(x + 12),
                    s => s,
                };
            }
            if matches!(e.sound, Sound::Note(_)) && !next_ties {
                let d = ctx.len(e);
                if d > 0.75 {
                    let keep = (d * 0.7).max(0.5);
                    e.end = e.start + (keep * ctx.shape.samples_per_beat) as u64;
                }
                e.volume = (e.volume + 1).min(15);
            }
        }
    }

    /// The feel's articulation of the bar: softer in a bossa, bluesy bends in rock (a whole
    /// step up into a long note, the blue third bent up to the major), punchy in funk.
    fn feel_line(&mut self, ctx: &Ctx, r: &mut crate::audio::accomp::Rng, fired: &mut Orns) {
        let p_bend = 0.25 + 0.35 * crate::audio::live::band::mid(self.p.freedom);
        let n = self.dst.len();
        for k in 0..n {
            let next_ties = self.dst.get(k + 1).is_some_and(|x| x.tie);
            let e = &mut self.dst[k];
            match ctx.feel() {
                Feel::Bossa => e.volume = e.volume.saturating_sub(1).max(1),
                Feel::Rock => {
                    let Sound::Note(note) = e.sound else { continue };
                    let d = ctx.len(e);
                    if e.tie || e.fx.slide != 0 || d < 0.75 {
                        continue;
                    }
                    let third = ctx.harm_at(ctx.rel(e)).is_some_and(|h| {
                        matches!(h.chord.family(), crate::audio::chart::Family::Major | crate::audio::chart::Family::Dominant) && (note + 12 - h.chord.root) % 12 == 4
                    });
                    if third && r.chance(0.7) {
                        (e.fx.slide, e.fx.slide_frames) = (-1, 9);
                        fired.add(Orn::Bend);
                    } else if r.chance(p_bend) {
                        (e.fx.slide, e.fx.slide_frames) = (-2, 7);
                        fired.add(Orn::Bend);
                    }
                }
                Feel::Funk => {
                    // Short and punchy (a slur keeps its length).
                    if matches!(e.sound, Sound::Note(_)) && !next_ties {
                        let d = ctx.len(e);
                        if d > 0.5 {
                            let keep = (d * 0.6).max(0.4);
                            e.end = e.start + (keep * ctx.shape.samples_per_beat) as u64;
                        }
                    }
                }
                Feel::Samba | Feel::Swing => {}
            }
        }
    }

    /// Funk: the horn section's stabs in the space after the phrase's last note (on the "e" and
    /// the "and" of the last beat), the chord voiced up high.
    fn horn_stabs(&mut self, ctx: &Ctx, fired: &mut Orns) {
        let bb = ctx.bb();
        let tail = self.dst.iter().map(|e| ctx.rel(e) + ctx.len(e)).fold(0.0, f64::max);
        if tail > bb - 1.0 + 1e-6 {
            return;
        }
        let Some(last) = self.dst.iter().rev().find(|e| matches!(e.sound, Sound::Note(_))).copied() else { return };
        let top = last.sound.notes()[0].clamp(68, 84);
        for (b, d) in [(bb - 0.75, 0.2), (bb - 0.25, 0.25)] {
            let Some(h) = ctx.harm_at(b) else { continue };
            let mut e = ctx.make(&ctx.template(0, Some(last)), b, d, Sound::Arp(ornament::voice(&h.chord, top, 0)));
            e.volume = (e.volume + 1).min(15);
            push(&mut self.dst, e);
        }
        fired.add(Orn::HornStab);
    }

    /// The phrase's last bar: fill the space before the next phrase, or fall off the last note.
    fn phrase_end(&mut self, ctx: &Ctx, orns: Orns, tmpl: &NoteEvent, r: &mut crate::audio::accomp::Rng, fired: &mut Orns) {
        let bb = ctx.bb();
        let Some(li) = self.dst.iter().rposition(|e| matches!(e.sound, Sound::Note(_) | Sound::Arp(_))) else { return };
        let last = self.dst[li];
        let (lb, ld) = (ctx.rel(&last), ctx.len(&last));
        if lb + ld > bb + 1e-6 {
            // Tied over into the next phrase: leave it.
            return;
        }
        let space = bb - (lb + ld);
        let last_note = *last.sound.notes().last().unwrap_or(&72);
        let target = ctx.next_first_note(0).unwrap_or(last_note);
        let t = ctx.template(0, Some(last));
        let _ = tmpl;
        if orns.has(Orn::RunFill) && (space >= 1.5 - 1e-9 || ld >= 2.0) {
            let from = if space >= 1.5 - 1e-9 { ((lb + ld) * 2.0).ceil() / 2.0 } else { lb + 1.0 }.max(bb - 2.0);
            if from > lb + 0.2 {
                self.dst[li].end = self.dst[li].end.min(ctx.at(ctx.line() + from));
                let count = ((bb - from) / 0.25).floor() as i32;
                let scale = scale_at(ctx, from);
                let dir = if target >= last_note { 1 } else { -1 };
                let mut n = scale.step(target, -dir * count);
                for j in 0..count {
                    push(&mut self.dst, ctx.make(&t, from + j as f64 * 0.25, 0.25, Sound::Note(n)));
                    n = scale.step(n, dir);
                }
                fired.add(Orn::RunFill);
                return;
            }
        }
        if orns.has(Orn::TripletRun)
            && space >= 1.0 - 1e-9
            && let Some(h) = ctx.harm_at(bb - 1.0)
        {
            let from = ((lb + ld) * 3.0).ceil() / 3.0;
            let from = from.max(bb - 2.0);
            let count = ((bb - from) * 3.0).round() as usize;
            let mut n = h.chord_tone_above(last_note.saturating_sub(5));
            for j in 0..count {
                push(&mut self.dst, ctx.make(&t, from + j as f64 / 3.0, 1.0 / 3.0, Sound::Note(n)));
                n = h.chord_tone_above(n + 1);
            }
            fired.add(Orn::TripletRun);
            return;
        }
        if orns.has(Orn::Pickup) && space >= 1.0 - 1e-9 {
            let count = if space >= 2.0 - 1e-9 { 4 } else { 2 };
            let from = bb - 0.5 * count as f64;
            let scale = scale_at(ctx, from);
            for j in 0..count {
                let n = scale.step(target, -(count as i32 - j as i32));
                self.eighth(ctx, &t, from + j as f64 * 0.5, bb, Sound::Note(n));
            }
            fired.add(Orn::Pickup);
            return;
        }
        if orns.has(Orn::FallOff) && ld >= 0.75 && matches!(last.sound, Sound::Note(_)) {
            let frames = ((last.end - last.start) as f64 * 60.0 / ctx.shape.sample_rate as f64) as u32;
            let e = &mut self.dst[li];
            e.fx.fall = -(3 + r.below(3) as i8);
            e.fx.fall_frames = (frames / 2).clamp(4, 14) as u8;
            fired.add(Orn::FallOff);
        }
    }

    /// Answer a toot: a slide into the bar's first real note, and an echo of it.
    fn answer(&mut self, ctx: &Ctx, tmpl: &NoteEvent, fired: &mut Orns) {
        if let Some(e) = self.dst.iter_mut().find(|e| matches!(e.sound, Sound::Note(_)) && (e.end - e.start) as f64 >= ctx.shape.samples_per_beat * 0.45) {
            e.fx.slide = -3;
            e.fx.slide_frames = 5;
            fired.add(Orn::Slide);
            return;
        }
        // Nothing to answer with: a quick arpeggio in the last beat, if it's free.
        let bb = ctx.bb();
        let free = !self.dst.iter().any(|e| ctx.rel(e) + ctx.len(e) > bb - 1.0 + 1e-6);
        if free && let Some(h) = ctx.harm_at(bb - 1.0) {
            let mut n = h.chord_tone_above(67);
            for j in 0..3 {
                push(&mut self.dst, ctx.make(tmpl, bb - 1.0 + j as f64 / 3.0, 1.0 / 3.0, Sound::Note(n)));
                n = h.chord_tone_above(n + 1);
            }
            fired.add(Orn::TripletRun);
        }
    }

    /// Land the phrase: its last note is the written one, a chord tone, or a step into the
    /// next phrase's first note; otherwise it moves to the nearest chord tone.
    fn resolve(&mut self, ctx: &Ctx) {
        let target = ctx.next_first_note(0);
        let Some(li) = self.dst.iter().rposition(|e| matches!(e.sound, Sound::Note(_))) else { return };
        let e = self.dst[li];
        let Sound::Note(n) = e.sound else { return };
        let b = ctx.rel(&e);
        let Some(h) = ctx.harm_at(b) else { return };
        let leads_on = target.is_some_and(|t| (t as i32 - n as i32).abs() <= 2);
        if as_written(&self.src, ctx, b, n) || h.is_chord_tone(n) || leads_on {
            return;
        }
        self.dst[li].sound = Sound::Note(h.nearest_chord_tone(n));
    }

    /// An improvised bar (a solo chorus, the lead's four): an 8th-note line on the changes,
    /// chord tones on the beats, scale steps and skips between.
    fn solo(&mut self, ctx: &Ctx, t: &NoteEvent, r: &mut crate::audio::accomp::Rng, phrase_last: bool) {
        let bb = ctx.bb();
        let mut cur = self.solo_note.unwrap_or(72).clamp(62, 86) as i32;
        let mut dir = if cur > 76 { -1 } else { 1 };
        // Rhythm: (start, length) on the 8th grid.
        let mut slots = [(0.0f64, 0.5f64); 16];
        let mut m = 0;
        let mut add = |s: f64, d: f64| {
            if m < 16 && s < bb - 1e-9 {
                slots[m] = (s, d.min(bb - s));
                m += 1;
            }
        };
        match r.below(4) {
            0 => (0..(bb * 2.0) as usize - 1).for_each(|k| add(k as f64 * 0.5, 0.5)),
            1 => {
                let mut b = 0.0;
                while b < bb - 1e-9 {
                    add(b, 1.0);
                    add(b + 1.0, 0.5);
                    add(b + 1.5, 0.5);
                    b += 2.0;
                }
            }
            2 => {
                (0..6).for_each(|k| add(k as f64 / 3.0, 1.0 / 3.0));
                let mut b = 2.0;
                while b < bb - 1e-9 {
                    add(b, 1.0);
                    b += 1.0;
                }
            }
            _ => (1..(bb * 2.0) as usize).for_each(|k| add(k as f64 * 0.5, 0.5)),
        }
        if phrase_last && m > 0 {
            // Land: the last note held to the bar line.
            let (s, _) = slots[m - 1];
            slots[m - 1] = (s, bb - s);
        }
        for &(s, d) in &slots[..m] {
            let Some(h) = ctx.harm_at(s) else { continue };
            let strong = (s - s.round()).abs() < 1e-6;
            let mut next = if r.chance(0.7) { h.scale.step(cur as u8, dir) as i32 } else { h.nearest_chord_tone((cur + 3 * dir).clamp(0, 127) as u8) as i32 };
            if strong && !h.is_chord_tone(next as u8) {
                next = h.nearest_chord_tone(next as u8) as i32;
            }
            if next > 86 || next < 62 {
                dir = -dir;
                next = h.scale.step(cur as u8, dir) as i32;
            }
            if r.chance(0.18) {
                dir = -dir;
            }
            let eighth = ((s * 2.0).round() - s * 2.0).abs() < 1e-6;
            let (s0, d0) = if eighth && (d - 0.5).abs() < 1e-6 {
                let a = ctx.swing8(s);
                (a, (ctx.swing8(s + 0.5).min(bb) - a).max(0.05))
            } else {
                (s, d)
            };
            push(&mut self.dst, ctx.make(t, s0, d0 * 0.95, Sound::Note(next.clamp(LO, HI) as u8)));
            cur = next;
        }
        self.solo_note = Some(cur as u8);
    }
}

/// The scale at beat `b` of the bar (the key's major scale without a chart).
fn scale_at(ctx: &Ctx, b: f64) -> Scale {
    ctx.harm_at(b).map(|h| h.scale).unwrap_or_else(|| Scale::of(&Chord::new(ctx.shape.key, Quality::Major), false))
}

/// The sad trombone: three wahs down by semitones and a long falling one, with the plunger.
fn wah_wah(ctx: &Ctx, t: &NoteEvent, dst: &mut Vec<NoteEvent>) {
    let bb = ctx.bb();
    let step = if bb >= 4.0 { 0.75 } else { 0.5 };
    let start = ctx.harm_at(0.0).map_or(67, |h: Harm| ornament::nearest_pc((h.chord.root + 7) % 12, 67, 60, 74));
    let vol = t.volume.max(10);
    for j in 0..4u8 {
        let b = j as f64 * step;
        let d = if j == 3 { bb - b } else { step * 0.92 };
        let mut e = ctx.make(t, b, d, Sound::Note(start.saturating_sub(j)));
        e.volume = vol;
        e.fx.wah = true;
        if j == 3 {
            e.fx.vib = 45;
            e.fx.fall = -4;
            e.fx.fall_frames = 24;
        }
        push(dst, e);
    }
}
