//! The tracker: rows of 16ths, a column per channel (note, volume, fx).

use bevy_egui::egui::{self, Align2, Color32, Key, Rect, Sense, Stroke, pos2, vec2};
use nat_han_adventures::audio::mml::{Arp, Drum};

use crate::app::Editor;
use crate::model::{EPS, HIGHEST, LOWEST, Note, Part, STEP, SongDoc, Sound, drum_letter};
use crate::theme::*;

/// Row height.
const ROW: f32 = 16.0;
const NUM_W: f32 = 44.0;

/// Tracker entry: a note at `row` of channel `ch`, `len` rows long.
pub fn enter(doc: &mut SongDoc, ch: usize, row: usize, sound: Sound, len: usize, volume: u8) -> Result<(), String> {
    let duty = doc.part(ch).and_then(|p| p.notes.iter().rev().find(|n| n.start <= row as f64 * STEP).map(|n| n.duty)).unwrap_or(2);
    doc.edit(ch, |p| {
        let inst = p.inst_at(row as f64 * STEP);
        p.insert(Note { start: row as f64 * STEP, dur: len.max(1) as f64 * STEP, sound, volume, duty, tie: false, inst });
    })
}

/// Delete the note starting at `row` (if any).
pub fn delete(doc: &mut SongDoc, ch: usize, row: usize) -> Result<(), String> {
    doc.edit(ch, |p| {
        let t = row as f64 * STEP;
        let first = p.starting_in(t, t + STEP).next();
        if let Some(i) = first {
            p.remove(i);
        }
    })
}

/// Note-off: end the note sounding at `row` there.
pub fn off(doc: &mut SongDoc, ch: usize, row: usize) -> Result<(), String> {
    doc.edit(ch, |p| p.cut_at(row as f64 * STEP))
}

pub fn note_name(n: u8) -> String {
    const N: [&str; 12] = ["C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-"];
    format!("{}{}", N[(n % 12) as usize], n as i32 / 12 - 1)
}

fn sound_name(s: &Sound) -> String {
    match s {
        Sound::Note(n) => note_name(*n),
        Sound::Arp(a) => note_name(a.notes()[0]),
        Sound::Drum(d) => format!("{}··", drum_letter(*d)),
    }
}

/// What a cell shows.
struct Cell {
    note: String,
    /// The instrument, where it changes (hex, `00` = default).
    ins: String,
    vol: String,
    fx: String,
    /// Note, note-off, sustain, empty.
    kind: u8,
}

fn cell(part: &Part, row: usize) -> Cell {
    let (a, z) = (row as f64 * STEP, (row + 1) as f64 * STEP);
    let mut starting = part.starting_in(a, z);
    if let Some(i) = starting.next() {
        let n = &part.notes[i];
        let more = starting.count();
        let prev_duty = (i > 0).then(|| part.notes[i - 1].duty);
        let fx = if more > 0 {
            format!("+{more}")
        } else if matches!(n.sound, Sound::Arp(_)) {
            "ARP".into()
        } else if n.tie {
            "SLR".into()
        } else if (n.start - a).abs() > EPS {
            "≈".into()
        } else if part.ch < 2 && prev_duty.is_some_and(|d| d != n.duty) {
            format!("@{}", n.duty)
        } else {
            "···".into()
        };
        let prev_inst = if i > 0 { part.notes[i - 1].inst } else { 0 };
        let ins = if n.inst != prev_inst || (i == 0 && n.inst != 0) { format!("{:02X}", n.inst) } else { "··".into() };
        return Cell { note: sound_name(&n.sound), ins, vol: format!("{:X}", n.volume), fx, kind: 0 };
    }
    let ended = part.notes.iter().any(|n| (n.end() - a).abs() < EPS) && !part.drums();
    if ended && part.at(a).is_none() {
        return Cell { note: "OFF".into(), ins: "··".into(), vol: "·".into(), fx: "···".into(), kind: 1 };
    }
    let kind = if part.at(a).is_some() && !part.drums() { 2 } else { 3 };
    Cell { note: "···".into(), ins: "··".into(), vol: "·".into(), fx: "···".into(), kind }
}

/// The tracker's piano keyboard: key → semitone above the cursor octave's C.
fn key_semitone(k: Key) -> Option<i32> {
    use Key::*;
    Some(match k {
        Z => 0,
        S => 1,
        X => 2,
        D => 3,
        C => 4,
        V => 5,
        G => 6,
        B => 7,
        H => 8,
        N => 9,
        J => 10,
        M => 11,
        Q => 12,
        Num2 => 13,
        W => 14,
        Num3 => 15,
        E => 16,
        R => 17,
        Num5 => 18,
        T => 19,
        Num6 => 20,
        Y => 21,
        Num7 => 22,
        U => 23,
        I => 24,
        Num9 => 25,
        O => 26,
        Num0 => 27,
        P => 28,
        _ => return None,
    })
}

fn key_drum(k: Key) -> Option<Drum> {
    match k {
        Key::Z => Some(Drum::Kick),
        Key::X => Some(Drum::Snare),
        Key::C => Some(Drum::ClosedHat),
        Key::V => Some(Drum::OpenHat),
        _ => None,
    }
}

/// Keyboard: the cursor always; notes when REC is on. True if a key was used.
pub fn keys(ed: &mut Editor, ctx: &egui::Context) {
    let rows = ed.bars() * ed.rows_per_bar();
    if rows == 0 {
        return;
    }
    let events = ctx.input(|i| i.events.clone());
    for ev in events {
        let egui::Event::Key { key, pressed: true, modifiers, .. } = ev else { continue };
        let t = &mut ed.tracker;
        let rpb = (ed.doc.good.as_ref().map_or(4.0, |s| s.bar_beats()) / STEP).round() as usize;
        match key {
            Key::ArrowUp => t.row = t.row.saturating_sub(1),
            Key::ArrowDown => t.row = (t.row + 1).min(rows - 1),
            Key::ArrowLeft => t.ch = t.ch.saturating_sub(1),
            Key::ArrowRight => t.ch = (t.ch + 1).min(3),
            Key::PageUp => t.row = t.row.saturating_sub(rpb),
            Key::PageDown => t.row = (t.row + rpb).min(rows - 1),
            Key::Home => t.row = 0,
            Key::End => t.row = rows - 1,
            _ if !t.rec || modifiers.ctrl || modifiers.command => {}
            Key::Delete | Key::Backspace => {
                let (ch, row) = (t.ch, t.row);
                let r = delete(&mut ed.doc, ch, row);
                ed.edit(r);
            }
            Key::A => {
                let (ch, row, step) = (t.ch, t.row, t.step);
                let r = off(&mut ed.doc, ch, row);
                ed.edit(r);
                ed.tracker.row = (row + step).min(rows - 1);
            }
            Key::Plus | Key::Equals => t.octave = (t.octave + 1).min(7),
            Key::Minus => t.octave = (t.octave - 1).max(0),
            Key::OpenBracket => t.len = (t.len.saturating_sub(1)).max(1),
            Key::CloseBracket => t.len = (t.len + 1).min(64),
            k => {
                let (ch, row, len, step, oct) = (t.ch, t.row, t.len, t.step, t.octave);
                let sound = if ch == 3 {
                    key_drum(k).map(Sound::Drum)
                } else {
                    key_semitone(k).map(|s| {
                        let n = (12 * (oct + 1) + s).clamp(LOWEST as i32, HIGHEST as i32) as u8;
                        // Typing on an arpeggio's row re-roots the chord.
                        let old = ed.doc.part(ch).and_then(|p| p.starting_in(row as f64 * STEP, (row + 1) as f64 * STEP).next().map(|i| p.notes[i].sound));
                        match old {
                            Some(Sound::Arp(a)) => {
                                let d = n as i32 - a.notes()[0] as i32;
                                Sound::Arp(Arp::new(&a.notes().iter().map(|&x| (x as i32 + d).clamp(LOWEST as i32, HIGHEST as i32) as u8).collect::<Vec<_>>()))
                            }
                            _ => Sound::Note(n),
                        }
                    })
                };
                if let Some(sound) = sound {
                    let vol = ed.doc.part(ch).and_then(|p| p.notes.iter().rev().find(|n| n.start <= row as f64 * STEP).map(|n| n.volume)).unwrap_or(12);
                    let r = enter(&mut ed.doc, ch, row, sound, len, vol);
                    ed.edit(r);
                    ed.tracker.row = (row + step).min(rows - 1);
                }
            }
        }
    }
}

pub fn ui(ed: &mut Editor, ui: &mut egui::Ui) {
    let bars = ed.bars();
    let rpb = ed.rows_per_bar().max(1);
    let rows = bars * rpb;
    let playhead_row = ed.playhead().map(|b| (b / STEP).floor() as usize);
    let shown = ed.shown_harmony();
    // Header.
    let played_label = format!("played ({})", harmony_name(shown));
    egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::symmetric(12, 0)).show(ui, |ui| {
        ui.set_height(36.0);
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 16.0;
            ui.label(egui::RichText::new("PATTERN").strong().color(TEXT));
            let bar = ed.tracker.row / rpb;
            ui.label(egui::RichText::new(format!("bar {:02}/{bars}", bar + 1)).color(TEXT_DIM));
            stepper(ui, "oct", &mut ed.tracker.octave, 0, 7);
            let mut len = ed.tracker.len as i32;
            stepper(ui, "len", &mut len, 1, 64);
            ed.tracker.len = len as usize;
            let mut step = ed.tracker.step as i32;
            stepper(ui, "step", &mut step, 0, 64);
            ed.tracker.step = step as usize;
            let rec = egui::RichText::new("● REC").strong().color(if ed.tracker.rec { RED } else { TEXT_FAINT });
            if ui.add(egui::Button::new(rec).frame(false)).on_hover_text("step entry from the keyboard (Z S X D C ... = notes, A = off, Del = delete, [ ] length, - + octave)").clicked() {
                ed.tracker.rec = !ed.tracker.rec;
            }
            let follow = if ed.tracker.follow { "follow on" } else { "follow off" };
            ui.spacing_mut().item_spacing.x = 12.0;
            if ui.add(egui::Button::new(egui::RichText::new(follow).color(TEXT_DIM)).frame(false)).clicked() {
                ed.tracker.follow = !ed.tracker.follow;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                if seg(ui, ed.tracker.show_played, &played_label).clicked() {
                    ed.tracker.show_played = true;
                }
                if seg(ui, !ed.tracker.show_played, "written").clicked() {
                    ed.tracker.show_played = false;
                }
                ui.add_space(8.0);
                ui.label(egui::RichText::new("show").color(TEXT_DIM));
            });
        });
    });
    line(ui);
    let parts: Vec<Part> = if ed.tracker.show_played {
        ed.played().parts.clone().map(|p| p.to_vec()).unwrap_or_default()
    } else {
        (0..4).filter_map(|ch| ed.doc.part(ch)).collect()
    };
    let width = ui.available_width();
    let col_w = ((width - NUM_W) / 4.0).max(60.0);
    // Channel headers.
    let (hdr, _) = ui.allocate_exact_size(vec2(width, 40.0), Sense::hover());
    let p = ui.painter_at(hdr);
    p.rect_filled(hdr, 0.0, RAISED);
    p.text(pos2(hdr.left() + 6.0, hdr.center().y), Align2::LEFT_CENTER, "ROW", mono(12.0), TEXT_FAINT);
    for ch in 0..4 {
        let x = hdr.left() + NUM_W + ch as f32 * col_w;
        p.line_segment([pos2(x, hdr.top()), pos2(x, hdr.bottom())], Stroke::new(1.0, LINE));
        p.rect_filled(Rect::from_min_size(pos2(x + 10.0, hdr.top() + 9.0), vec2(10.0, 10.0)), 2.0, CHANNEL[ch]);
        let name = p.text(pos2(x + 28.0, hdr.top() + 14.0), Align2::LEFT_CENTER, CHANNEL_NAME[ch], mono(12.0), TEXT);
        p.text(pos2(name.right() + 8.0, hdr.top() + 14.0), Align2::LEFT_CENTER, ROLE[ch], mono(12.0), TEXT_FAINT);
        for (k, l) in ["note", "ins", "vol", "fx"].iter().enumerate() {
            p.text(pos2(x + 10.0 + [0.0, 46.0, 72.0, 98.0][k], hdr.top() + 30.0), Align2::LEFT_CENTER, *l, mono(11.0), TEXT_FAINT);
        }
        if ch == ed.tracker.ch {
            p.line_segment([pos2(x + 1.0, hdr.bottom() - 1.0), pos2(x + col_w, hdr.bottom() - 1.0)], Stroke::new(2.0, CHANNEL[ch]));
        }
    }
    line(ui);
    if rows == 0 {
        ui.label("nothing parsed yet");
        return;
    }
    let mut area = egui::ScrollArea::vertical().id_salt("tracker").auto_shrink(false);
    let follow_row = playhead_row.filter(|_| ed.tracker.follow);
    let view_h = ui.available_height();
    if let Some(r) = follow_row {
        // Keep the playhead a third of the way down.
        area = area.vertical_scroll_offset((r as f32 * ROW - view_h / 3.0).max(0.0));
    } else if ed.tracker.scrolled != Some(ed.tracker.row) {
        // The cursor moved: bring it into view, a third of the way down.
        area = area.vertical_scroll_offset((ed.tracker.row as f32 * ROW - view_h / 3.0).max(0.0));
    }
    ed.tracker.scrolled = Some(ed.tracker.row);
    let mut clicked: Option<(usize, usize)> = None;
    area.show_rows(ui, ROW, rows, |ui, range| {
        for row in range {
            // A stable id per row (the rows shown change as it scrolls).
            let (rect, _) = ui.allocate_exact_size(vec2(width, ROW), Sense::hover());
            let resp = ui.interact(rect, egui::Id::new(("tracker-row", row)), Sense::click());
            let p = ui.painter_at(rect);
            let in_bar = row % rpb;
            let bar_row = in_bar == 0;
            let beat_row = in_bar.is_multiple_of(4);
            let playing = playhead_row == Some(row);
            let bg = if playing {
                PLAYHEAD_ROW
            } else if bar_row {
                BAR_ROW
            } else if beat_row {
                BEAT_ROW
            } else {
                Color32::TRANSPARENT
            };
            p.rect_filled(rect, 0.0, bg);
            if bar_row {
                p.line_segment([rect.left_top(), rect.right_top()], Stroke::new(1.0, BAR_LINE));
            }
            if playing {
                p.rect_filled(Rect::from_min_size(rect.min, vec2(3.0, ROW)), 0.0, GOLD);
            }
            let num_color = if bar_row { GOLD } else if beat_row { TEXT_DIM } else { ROW_NUM };
            let num = if bar_row { format!("{:02}", row / rpb + 1) } else { format!("{in_bar:02}") };
            p.text(pos2(rect.left() + 8.0, rect.center().y), Align2::LEFT_CENTER, num, mono(12.0), num_color);
            for (ch, part) in parts.iter().enumerate() {
                let x = rect.left() + NUM_W + ch as f32 * col_w;
                p.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(1.0, LINE_SOFT));
                let cursor = ch == ed.tracker.ch && row == ed.tracker.row;
                if cursor {
                    let c = Rect::from_min_size(pos2(x + 1.0, rect.top()), vec2(col_w - 1.0, ROW));
                    p.rect_filled(c, 2.0, if ed.tracker.rec { RED.gamma_multiply(0.25) } else { HILITE });
                    p.rect_stroke(c, 2.0, Stroke::new(1.0, if ed.tracker.rec { RED } else { GOLD }), egui::StrokeKind::Inside);
                }
                let c = cell(part, row);
                if c.kind == 2 {
                    p.rect_filled(Rect::from_min_size(pos2(x + 3.0, rect.top()), vec2(2.0, ROW)), 0.0, CHANNEL[ch].gamma_multiply(0.45));
                }
                let (nc, vc, fc) = match c.kind {
                    0 => (CHANNEL[ch], if playing { TEXT } else { TEXT_DIM }, if c.fx == "···" { TEXT_GHOST } else { GOLD }),
                    1 => (TEXT_FAINT, TEXT_GHOST, TEXT_GHOST),
                    _ => (TEXT_GHOST, TEXT_GHOST, TEXT_GHOST),
                };
                let ic = if c.ins == "··" { TEXT_GHOST } else { GOLD };
                let font = mono(12.0);
                p.text(pos2(x + 10.0, rect.center().y), Align2::LEFT_CENTER, &c.note, font.clone(), nc);
                p.text(pos2(x + 56.0, rect.center().y), Align2::LEFT_CENTER, &c.ins, font.clone(), ic);
                p.text(pos2(x + 84.0, rect.center().y), Align2::LEFT_CENTER, &c.vol, font.clone(), vc);
                p.text(pos2(x + 108.0, rect.center().y), Align2::LEFT_CENTER, &c.fx, font, fc);
            }
            if resp.clicked()
                && let Some(pos) = resp.interact_pointer_pos()
            {
                let ch = (((pos.x - rect.left() - NUM_W) / col_w).floor().max(0.0) as usize).min(3);
                clicked = Some((row, ch));
            }
            if resp.hovered() {
                let note = part_tooltip(&parts, row, ((ui.ctx().pointer_hover_pos().map_or(0.0, |p| p.x) - rect.left() - NUM_W) / col_w).floor() as i32);
                if let Some(t) = note {
                    resp.on_hover_text(t);
                }
            }
        }
    });
    if let Some((row, ch)) = clicked {
        ed.tracker.row = row;
        ed.tracker.ch = ch;
        // Clicked where it's seen: no need to scroll.
        ed.tracker.scrolled = Some(row);
    }
}

/// An arpeggio's notes, for the hover text.
fn part_tooltip(parts: &[Part], row: usize, ch: i32) -> Option<String> {
    let part = parts.get(usize::try_from(ch).ok()?)?;
    let i = part.starting_in(row as f64 * STEP, (row + 1) as f64 * STEP).next()?;
    let n = &part.notes[i];
    let what = match n.sound {
        Sound::Arp(a) => format!("arpeggio {{{}}}", a.notes().iter().map(|&x| note_name(x)).collect::<Vec<_>>().join(" ")),
        Sound::Note(x) => note_name(x),
        Sound::Drum(d) => format!("{d:?}"),
    };
    Some(format!("{what} · {} beats · v{} · @i {}{}", fmt_beats(n.dur), n.volume, part.inst_name(n.inst), if n.tie { " · slurred" } else { "" }))
}

fn fmt_beats(b: f64) -> String {
    let s = format!("{b:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub fn harmony_name(h: nat_han_adventures::audio::Harmony) -> &'static str {
    use nat_han_adventures::audio::Harmony::*;
    match h {
        Original => "original",
        Coltrane => "Coltrane",
        Quartal => "quartal",
        MelodicMinor => "mel. minor",
        Waltz => "waltz",
    }
}

/// A segmented-control button.
pub fn seg(ui: &mut egui::Ui, on: bool, label: &str) -> egui::Response {
    let text = egui::RichText::new(label).size(12.0).color(if on { BG } else { TEXT_DIM });
    let text = if on { text.strong() } else { text };
    ui.add(egui::Button::new(text).fill(if on { GOLD } else { RAISED }).stroke(Stroke::new(1.0, LINE)).corner_radius(0))
}

fn stepper(ui: &mut egui::Ui, label: &str, v: &mut i32, lo: i32, hi: i32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.label(egui::RichText::new(label).color(TEXT_DIM));
        if ui.small_button("−").clicked() {
            *v = (*v - 1).max(lo);
        }
        ui.label(egui::RichText::new(v.to_string()).color(TEXT));
        if ui.small_button("+").clicked() {
            *v = (*v + 1).min(hi);
        }
    });
}

pub fn line(ui: &mut egui::Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, LINE);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Editor, repo_root};
    use nat_han_adventures::audio::AudioOutput;

    fn press(ed: &mut Editor, keys: &[Key]) {
        let ctx = egui::Context::default();
        let events = keys
            .iter()
            .map(|&key| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE })
            .collect();
        let _ = ctx.run_ui(egui::RawInput { events, ..Default::default() }, |ui| keys_of(ed, ui.ctx()));
    }

    fn keys_of(ed: &mut Editor, ctx: &egui::Context) {
        keys(ed, ctx);
    }

    /// REC step entry: Z on row 0 of pulse 1 in octave 5 writes a C5 an 8th long and steps on;
    /// A is a note-off; Delete removes.
    #[test]
    fn rec_step_entry_writes_notes() {
        let mut ed = Editor::new(repo_root(), Some("sweet_georgia_brown"), AudioOutput::Headless);
        ed.tracker.rec = true;
        ed.tracker.octave = 5;
        press(&mut ed, &[Key::Z, Key::E]);
        let p = ed.doc.part(0).unwrap();
        assert_eq!(p.notes[0].sound, Sound::Note(72));
        assert_eq!((p.notes[0].start, p.notes[0].dur), (0.0, 0.5));
        assert_eq!(p.notes[1].sound, Sound::Note(88));
        assert_eq!(p.notes[1].start, 0.5);
        assert_eq!(ed.tracker.row, 4);
        assert!(ed.doc.error().is_none());
        // Drums on the noise channel.
        ed.tracker.ch = 3;
        ed.tracker.row = 0;
        press(&mut ed, &[Key::X]);
        assert_eq!(ed.doc.part(3).unwrap().notes[0].sound, Sound::Drum(Drum::Snare));
        // Off and delete.
        ed.tracker.ch = 0;
        ed.tracker.row = 1;
        press(&mut ed, &[Key::A]);
        assert_eq!(ed.doc.part(0).unwrap().notes[0].dur, 0.25);
        ed.tracker.row = 0;
        press(&mut ed, &[Key::Delete]);
        assert_ne!(ed.doc.part(0).unwrap().notes[0].start, 0.0);
        // Without REC the letters don't write.
        ed.tracker.rec = false;
        let before = ed.doc.text.clone();
        press(&mut ed, &[Key::Z, Key::ArrowDown]);
        assert_eq!(ed.doc.text, before);
        assert_eq!(ed.tracker.row, 1);
    }

    #[test]
    fn cells_show_notes_offs_and_arpeggios() {
        let doc = SongDoc::new("t", "[song]\ntitle = t\nbpm = 120\n[pulse2]\no4 {c e g}4 r4 c2 |\n");
        let p = doc.part(1).unwrap();
        let c = cell(&p, 0);
        assert_eq!((c.note.as_str(), c.fx.as_str()), ("C-4", "ARP"));
        assert_eq!(cell(&p, 4).note, "OFF");
        assert_eq!(cell(&p, 8).note, "C-4");
        assert_eq!(cell(&p, 9).kind, 2, "sustain");
    }

    /// The instrument column shows where the instrument changes.
    #[test]
    fn cells_show_instrument_changes() {
        let doc = SongDoc::new("t", "[song]\ntitle = t\nbpm = 120\n[instruments]\nb : vol 9\n[pulse1]\no4 c4 @i b d4 e4 @i default f4 |\n");
        let p = doc.part(0).unwrap();
        let ins: Vec<String> = [0, 4, 8, 12].iter().map(|&r| cell(&p, r).ins).collect();
        assert_eq!(ins, ["··", "01", "··", "00"]);
        // And it writes back.
        assert!(p.to_mml().contains("@i b") && p.to_mml().contains("@i default"), "{}", p.to_mml());
    }
}

