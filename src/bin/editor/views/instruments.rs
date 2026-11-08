//! The instruments tab: the song's `[instruments]`, one at a time. A tone instrument's macros
//! are bar graphs (one column per 60 Hz frame: drag to draw; the loop and release points as
//! markers), its vibrato and fade as sliders; a kit's drums as sliders; the definition as
//! text either way. Every change rewrites that line of the song (the comment kept) and plays
//! on the next parse; ▶ plays a few notes on it alone, "in the song" plays the song with a
//! channel switched to it (a try-out: the song isn't changed). Each channel's palette (what
//! its musician may switch to) is a row of toggles.

use bevy_egui::egui::{self, Align2, Rect, Sense, Stroke, pos2, vec2};
use nat_han_adventures::audio::live::instrument::{Def, Instruments, Kit, SEQ_MAX, Seq, Tone, Vibrato, Wave};
use nat_han_adventures::audio::live::song::CHANNELS;

use crate::app::Editor;
use crate::model::{instrument_lines, set_instrument, set_palette};
use crate::theme::*;
use crate::views::tracker::{line, seg};

/// A macro graph's column width.
const COL: f32 = 14.0;

pub fn ui(ed: &mut Editor, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing = vec2(8.0, 4.0);
    let Some(song) = ed.song().cloned() else {
        ui.label("nothing parsed yet: fix the text first");
        return;
    };
    let insts = &song.instruments;
    // Header: new instruments, the preview's channel and octave.
    egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::symmetric(12, 0)).show(ui, |ui| {
        ui.set_height(36.0);
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            ui.label(egui::RichText::new("INSTRUMENTS").strong().color(TEXT));
            if ui.button("+ instrument").on_hover_text("a new tone instrument").clicked() {
                let name = fresh(insts, "inst");
                let t = set_instrument(&ed.doc.text, &name, "vol 15 12 10 8 | 7 | duty 2");
                ed.set_text(t);
                ed.inst.selected = Some(name);
            }
            if ui.button("+ kit").on_hover_text("a new drum kit").clicked() {
                let name = fresh(insts, "kit");
                let t = set_instrument(&ed.doc.text, &name, "kick decay=4 | snare decay=3 | hat decay=1");
                ed.set_text(t);
                ed.inst.selected = Some(name);
            }
            ui.label(egui::RichText::new("preview on").color(TEXT_DIM));
            ui.spacing_mut().item_spacing.x = 0.0;
            for ch in 0..3 {
                if seg(ui, ed.inst.ch == ch, CHANNELS[ch].0).clicked() {
                    ed.inst.ch = ch;
                }
            }
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.add_space(8.0);
            ui.label(egui::RichText::new("oct").color(TEXT_DIM));
            ui.add(egui::DragValue::new(&mut ed.inst.octave).range(1..=7));
            if let Some((ch, name)) = ed.audition.clone() {
                ui.label(egui::RichText::new(format!("auditioning {name} on {}", CHANNELS[ch].0)).color(GOLD));
            }
        });
    });
    line(ui);
    if ed.inst.selected.as_ref().is_none_or(|n| insts.index(n).is_none_or(|k| k == 0)) {
        ed.inst.selected = insts.names.first().cloned();
    }
    ui.horizontal_top(|ui| {
        // The list.
        ui.allocate_ui_with_layout(vec2(214.0, ui.available_height()), egui::Layout::top_down(egui::Align::Min), |ui| {
            egui::ScrollArea::vertical().id_salt("inst-list").show(ui, |ui| {
                ui.set_width(200.0);
                ui.add_space(6.0);
                if insts.names.is_empty() {
                    ui.label(egui::RichText::new("no [instruments] yet: every channel plays its built-in (`default`)").color(TEXT_FAINT));
                }
                for (k, name) in insts.names.iter().enumerate() {
                    let num = k as u8 + 1;
                    let on = ed.inst.selected.as_deref() == Some(name);
                    let kind = if insts.is_kit(num) { "kit" } else { "tone" };
                    let chans: Vec<&str> = (0..4).filter(|&c| insts.palette(c).contains(&num)).map(|c| CHANNELS[c].0).collect();
                    let text = format!("{num:02X} {name:<10} {kind}");
                    let r = ui.add(egui::Button::new(egui::RichText::new(text).font(mono(12.0)).color(if on { BG } else { TEXT })).fill(if on { GOLD } else { RAISED }).min_size(vec2(196.0, 22.0)));
                    if r.clicked() {
                        ed.inst.selected = Some(name.clone());
                        ed.inst.draft = None;
                    }
                    if !chans.is_empty() {
                        ui.label(egui::RichText::new(format!("   palette: {}", chans.join(" "))).size(10.0).color(TEXT_FAINT));
                    }
                }
            });
        });
        let Some(name) = ed.inst.selected.clone() else { return };
        let Some(num) = insts.index(&name) else { return };
        let def = insts.defs[num as usize - 1];
        ui.allocate_ui_with_layout(ui.available_size(), egui::Layout::top_down(egui::Align::Min), |ui| {
            egui::ScrollArea::vertical().id_salt("inst-edit").auto_shrink(false).show(ui, |ui| {
                ui.add_space(6.0);
                editor(ed, ui, insts, &name, num, def);
            });
        });
    });
}

/// A name not taken yet.
fn fresh(insts: &Instruments, base: &str) -> String {
    (1..).map(|k| format!("{base}{k}")).find(|n| insts.index(n).is_none()).expect("a free name")
}

fn editor(ed: &mut Editor, ui: &mut egui::Ui, insts: &Instruments, name: &str, num: u8, def: Def) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("{num:02X}  {name}")).font(pixel(10.0)).color(GOLD));
        if ui.button("▶ notes").on_hover_text("play a few notes on it, alone").clicked() {
            ed.preview(name);
        }
        let ch = if insts.is_kit(num) { 3 } else { ed.inst.ch };
        let on = ed.audition.as_ref().is_some_and(|(c, n)| *c == ch && n == name);
        let label = if on { "■ stop the try-out".to_string() } else { format!("▶ in the song ({})", CHANNELS[ch].0) };
        if ui.button(label).on_hover_text("play the song with this channel on it (nothing's written)").clicked() {
            ed.audition(ch, name);
        }
    });
    // The definition as text.
    let current = instrument_lines(&ed.doc.text).into_iter().find(|l| l.name == name && !l.palette).map(|l| l.def).unwrap_or_default();
    let mut draft = match &ed.inst.draft {
        Some((n, d)) if n == name => d.clone(),
        _ => current.clone(),
    };
    ui.add_space(4.0);
    let resp = ui.add(egui::TextEdit::singleline(&mut draft).font(mono(12.0)).desired_width(f32::INFINITY));
    if resp.changed() {
        match Instruments::parse(&format!("{name} : {draft}")) {
            Ok(_) => {
                let t = set_instrument(&ed.doc.text, name, &draft);
                ed.set_text(t);
                ed.inst.draft = None;
            }
            Err((_, e)) => {
                ed.inst.draft = Some((name.to_string(), draft));
                ed.complain(e);
            }
        }
    }
    ui.add_space(8.0);
    let changed = match def {
        Def::Tone(mut t) => tone_editor(ui, &mut t).then_some(Def::Tone(t)),
        Def::Kit(mut k) => kit_editor(ui, &mut k).then_some(Def::Kit(k)),
    };
    if let Some(d) = changed {
        let t = set_instrument(&ed.doc.text, name, &Instruments::def_text(&d));
        ed.set_text(t);
        ed.inst.draft = None;
    }
    // Palettes.
    ui.add_space(10.0);
    ui.label(egui::RichText::new("PALETTES · which musicians may switch to it").color(TEXT_FAINT));
    let kit = insts.is_kit(num);
    for ch in 0..4 {
        if kit != (ch == 3) {
            continue;
        }
        let mut pal: Vec<u8> = insts.palette(ch).to_vec();
        let mut on = pal.contains(&num);
        if ui.checkbox(&mut on, format!("{} ({})", CHANNELS[ch].0, ROLE[ch])).changed() {
            if on {
                if pal.is_empty() {
                    pal.push(0);
                }
                pal.push(num);
            } else {
                pal.retain(|&x| x != num);
            }
            let names: Vec<String> = pal.iter().map(|&i| insts.name(i).to_string()).collect();
            let names = if names.len() == 1 && names[0] == "default" { Vec::new() } else { names };
            let t = set_palette(&ed.doc.text, CHANNELS[ch].0, &names);
            ed.set_text(t);
        }
    }
}

/// The tone editor; true if anything changed.
fn tone_editor(ui: &mut egui::Ui, t: &mut Tone) -> bool {
    let mut changed = false;
    let mut tri = t.wave == Some(Wave::Triangle);
    if ui.checkbox(&mut tri, "tri: the triangle's 4-bit wave").changed() {
        t.wave = tri.then_some(Wave::Triangle);
        changed = true;
    }
    for (label, seq, lo, hi, scale, default) in [
        ("vol", &mut t.vol, 0, 15, 1, 15),
        ("duty", &mut t.duty, 0, 3, 1, 2),
        ("pitch (semitones)", &mut t.pitch, -12, 12, 100, 0),
    ] {
        changed |= seq_editor(ui, label, seq, lo, hi, scale, default);
    }
    ui.add_space(6.0);
    let mut vib = t.vib.is_some();
    if ui.checkbox(&mut vib, "vibrato").changed() {
        t.vib = vib.then(|| Vibrato::from_units(10.0, 12.0, 5.5, 15.0));
        changed = true;
    }
    if let Some(v) = &mut t.vib {
        let (mut delay, mut depth, mut speed, mut ramp) = (v.delay * 60.0, 1200.0 * (1.0 + v.depth).log2(), v.rate, v.ramp * 60.0);
        let mut c = false;
        ui.horizontal(|ui| {
            c |= ui.add(egui::Slider::new(&mut delay, 0.0..=60.0).text("delay (frames)")).changed();
            c |= ui.add(egui::Slider::new(&mut depth, 0.0..=100.0).text("depth (cents)")).changed();
        });
        ui.horizontal(|ui| {
            c |= ui.add(egui::Slider::new(&mut speed, 0.5..=12.0).text("speed (Hz)")).changed();
            c |= ui.add(egui::Slider::new(&mut ramp, 0.0..=60.0).text("ramp (frames)")).changed();
        });
        if c {
            *v = Vibrato::from_units(delay.round(), depth.round(), (speed * 10.0).round() / 10.0, ramp.round().max(1.0));
            changed = true;
        }
    }
    let mut fade = t.fade.is_some();
    ui.horizontal(|ui| {
        if ui.checkbox(&mut fade, "fade").changed() {
            t.fade = fade.then_some(0.8);
            changed = true;
        }
        if let Some(f) = &mut t.fade
            && ui.add(egui::Slider::new(f, 0.05..=4.0).text("seconds to 65%")).changed()
        {
            *f = (*f * 100.0).round() / 100.0;
            changed = true;
        }
    });
    changed
}

/// One macro as a bar graph: drag to draw, buttons for length and the loop / release points.
fn seq_editor(ui: &mut egui::Ui, label: &str, seq: &mut Option<Seq>, lo: i16, hi: i16, scale: i16, default: i16) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let mut on = seq.is_some();
        if ui.checkbox(&mut on, egui::RichText::new(label).strong()).changed() {
            *seq = on.then(|| Seq::new(&[default * scale], None, None));
            changed = true;
        }
        if let Some(q) = seq {
            let mut steps = q.steps().to_vec();
            let (mut lp, mut rel) = (q.loop_at(), q.release());
            let mut c = false;
            if ui.small_button("+ frame").clicked() && steps.len() < SEQ_MAX {
                steps.push(*steps.last().unwrap_or(&0));
                c = true;
            }
            if ui.small_button("− frame").clicked() && steps.len() > 1 {
                steps.pop();
                c = true;
            }
            let n = steps.len();
            let mut l = lp.map_or(-1, |x| x as i32);
            if ui.add(egui::DragValue::new(&mut l).range(-1..=n as i32 - 1).prefix("loop | ")).on_hover_text("loop from this frame (-1: hold the last)").changed() {
                lp = (l >= 0).then_some(l as usize);
                c = true;
            }
            let mut r = rel.map_or(-1, |x| x as i32);
            if ui.add(egui::DragValue::new(&mut r).range(-1..=n as i32 - 1).prefix("release / ")).on_hover_text("the release starts at this frame (-1: none)").changed() {
                rel = (r >= 1).then_some(r as usize);
                c = true;
            }
            if c {
                let lp = lp.filter(|&l| l < n && rel.is_none_or(|r| l < r));
                *q = Seq::new(&steps, lp, rel.filter(|&r| r < n));
                changed = true;
            }
        }
    });
    let Some(q) = seq else { return changed };
    let n = q.steps().len();
    let h = 70.0;
    let (rect, resp) = ui.allocate_exact_size(vec2((SEQ_MAX as f32 * COL).min(ui.available_width()), h + 14.0), Sense::click_and_drag());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0, RAISED);
    let graph = Rect::from_min_size(rect.min, vec2(rect.width(), h));
    let y_of = |v: i16| graph.bottom() - (v - lo * scale) as f32 / ((hi - lo) * scale) as f32 * (h - 4.0) - 2.0;
    let zero = y_of(0.max(lo) * scale);
    for (k, &v) in q.steps().iter().enumerate() {
        let x = graph.left() + k as f32 * COL;
        let in_release = q.release().is_some_and(|r| k >= r);
        let looped = q.loop_at().is_some_and(|l| k >= l) && !in_release;
        let c = if in_release { RED } else if looped { GOLD } else { CHANNEL[0] };
        let (a, b) = (zero.min(y_of(v)), zero.max(y_of(v)));
        p.rect_filled(Rect::from_min_max(pos2(x + 1.0, a), pos2(x + COL - 1.0, b.max(a + 2.0))), 1.0, c);
        p.text(pos2(x + COL / 2.0, graph.bottom() + 7.0), Align2::CENTER_CENTER, format!("{}", v / scale), mono(8.0), TEXT_FAINT);
    }
    for (at, mark, c) in [(q.loop_at(), "|", GOLD), (q.release(), "/", RED)] {
        if let Some(k) = at {
            let x = graph.left() + k as f32 * COL;
            p.line_segment([pos2(x, graph.top()), pos2(x, graph.bottom())], Stroke::new(1.5, c));
            p.text(pos2(x + 2.0, graph.top() + 6.0), Align2::LEFT_CENTER, mark, mono(10.0), c);
        }
    }
    if (resp.dragged() || resp.clicked())
        && let Some(pos) = resp.interact_pointer_pos()
    {
        let k = ((pos.x - graph.left()) / COL).floor().max(0.0) as usize;
        let frac = ((graph.bottom() - 2.0 - pos.y) / (h - 4.0)).clamp(0.0, 1.0);
        let v = (lo as f32 + frac * (hi - lo) as f32).round() as i16 * scale;
        let mut steps = q.steps().to_vec();
        if k < SEQ_MAX {
            while steps.len() <= k {
                steps.push(*steps.last().unwrap_or(&0));
            }
            if steps[k] != v {
                steps[k] = v;
                *q = Seq::new(&steps, q.loop_at(), q.release());
                changed = true;
            }
        }
    }
    let _ = n;
    changed
}

/// The kit editor: each drum's main parameters.
fn kit_editor(ui: &mut egui::Ui, k: &mut Kit) -> bool {
    let d = Kit::DEFAULT;
    let mut changed = false;
    let frames = |x: f32| x * 60.0;
    ui.label(egui::RichText::new("kick").strong());
    ui.horizontal(|ui| {
        let mut semis = 12.0 * (1.0 + k.kick.sweep_hz / k.kick.base_hz).log2();
        if ui.add(egui::Slider::new(&mut semis, 0.0..=48.0).text("sweep (semitones)")).changed() {
            k.kick.sweep_hz = k.kick.base_hz * (2f32.powf(semis.round() / 12.0) - 1.0);
            changed = true;
        }
        let mut dec = frames(k.kick.amp_tau);
        if ui.add(egui::Slider::new(&mut dec, 1.0..=30.0).text("decay (frames)")).changed() {
            k.kick.amp_tau = dec.round() / 60.0;
            k.kick.len = (d.kick.len * k.kick.amp_tau / d.kick.amp_tau).min(1.2);
            changed = true;
        }
    });
    ui.label(egui::RichText::new("snare").strong());
    ui.horizontal(|ui| {
        changed |= ui.checkbox(&mut k.snare.short, "short noise").changed();
        let mut dec = frames(k.snare.noise_tau);
        if ui.add(egui::Slider::new(&mut dec, 1.0..=30.0).text("decay (frames)")).changed() {
            k.snare.noise_tau = dec.round() / 60.0;
            k.snare.len = (d.snare.len * k.snare.noise_tau / d.snare.noise_tau).min(1.2);
            changed = true;
        }
        if ui.add(egui::Slider::new(&mut k.snare.tone_hz, 60.0..=600.0).text("tone (Hz)")).changed() {
            k.snare.tone_hz = k.snare.tone_hz.round();
            changed = true;
        }
    });
    for (label, m, dm) in [("hat", &mut k.hat, d.hat), ("open hat", &mut k.ohat, d.ohat), ("crash", &mut k.crash, d.crash)] {
        ui.label(egui::RichText::new(label).strong());
        ui.horizontal(|ui| {
            changed |= ui.checkbox(&mut m.short, "short noise").changed();
            let mut dec = frames(m.tau);
            if ui.add(egui::Slider::new(&mut dec, 1.0..=40.0).text("decay (frames)")).changed() {
                m.tau = dec.round() / 60.0;
                m.len = (dm.len * m.tau / dm.tau).min(1.2);
                changed = true;
            }
        });
    }
    changed
}
