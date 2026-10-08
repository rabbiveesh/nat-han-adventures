//! The text view: the `.song` source with line numbers, the real parser's errors (line and
//! column) in a red bar, and the cheat sheet docked on the right (F1), made from
//! [`syntax::CHEAT_SHEET`] so it can't drift from the grammar. Click an entry to insert it.

use std::sync::Arc;

use bevy_egui::egui::{self, Color32, FontId, Galley, Sense, Stroke, TextBuffer, text::LayoutJob};
use nat_han_adventures::audio::live::syntax;

use crate::app::Editor;
use crate::theme::*;
use crate::views::tracker::line;

/// The cheat sheet as the panel shows it: (section, [(syntax, meaning, text to insert)]).
pub fn cheat_items() -> Vec<(&'static str, Vec<(&'static str, &'static str, String)>)> {
    syntax::CHEAT_SHEET
        .iter()
        .map(|(section, rows)| (*section, rows.iter().map(|(s, m)| (*s, *m, insertion(s))).collect()))
        .collect()
}

/// What clicking an entry inserts: section headers on a line of their own, the rest as typed.
fn insertion(syntax: &str) -> String {
    if syntax.starts_with('[') && syntax.ends_with(']') && !syntax.contains(' ') && syntax.len() > 3 {
        format!("\n{syntax}\n")
    } else if syntax.contains(" = ") {
        format!("{syntax}\n")
    } else {
        format!("{syntax} ")
    }
}

const TEXT_ID: &str = "song-text";

pub fn text_id() -> egui::Id {
    egui::Id::new(TEXT_ID)
}

pub fn ui(ed: &mut Editor, ui: &mut egui::Ui) {
    let err = ed.doc.error().cloned();
    // Header.
    egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::symmetric(12, 0)).show(ui, |ui| {
        ui.set_height(36.0);
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 16.0;
            ui.label(egui::RichText::new(ed.doc.path()).strong().color(TEXT));
            let state = if ed.doc.dirty() { "edited · unsaved" } else { "saved" };
            ui.label(egui::RichText::new(state).color(TEXT_DIM));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let b = egui::Button::new(egui::RichText::new("cheat sheet ?").size(11.0).color(GOLD)).fill(HILITE).stroke(Stroke::new(1.0, GOLD));
                if ui.add(b).on_hover_text("F1").clicked() {
                    ed.text.cheat = !ed.text.cheat;
                }
                if let Some(beat) = ed.text_cursor_beat() {
                    let bb = ed.bar_beats();
                    let bar = (beat / bb).floor();
                    let txt = format!("cursor bar {:02} · beat {} → ctrl+⏎ plays from here", bar as usize + 1, ((beat - bar * bb).floor() as usize) + 1);
                    ui.label(egui::RichText::new(txt).color(TEXT_DIM));
                }
            });
        });
    });
    line(ui);
    let bar_h = 34.0;
    let body_h = ui.available_height() - bar_h;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
        let cheat_w = if ed.text.cheat { 300.0 } else { 0.0 };
        let edit_w = ui.available_width() - cheat_w;
        let down = egui::Layout::top_down(egui::Align::Min);
        ui.allocate_ui_with_layout(egui::vec2(edit_w, body_h), down, |ui| {
            ui.set_min_size(egui::vec2(edit_w, body_h));
            editor(ed, ui, err.as_ref().map(|e| e.line));
        });
        if ed.text.cheat {
            ui.allocate_ui_with_layout(egui::vec2(cheat_w, body_h), down, |ui| {
                ui.set_min_size(egui::vec2(cheat_w, body_h));
                cheat_sheet(ed, ui);
            });
        }
    });
    // The parse status bar.
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), bar_h), Sense::hover());
    let p = ui.painter_at(rect);
    match &err {
        Some(e) => {
            p.rect_filled(rect, 0.0, ERR_BG);
            p.line_segment([rect.left_top(), rect.right_top()], Stroke::new(1.0, RED));
            let section = section_of(&ed.doc.text, e.line).unwrap_or_default();
            let at = if e.col > 0 { format!("{section}line {}, col {}", e.line, e.col) } else { format!("{section}line {}", e.line) };
            let r = p.text(egui::pos2(rect.left() + 12.0, rect.center().y), egui::Align2::LEFT_CENTER, at, mono(12.0), RED);
            p.text(egui::pos2(r.right() + 12.0, rect.center().y), egui::Align2::LEFT_CENTER, &e.msg, mono(12.0), ERR_TEXT);
        }
        None => {
            p.rect_filled(rect, 0.0, PANEL);
            p.line_segment([rect.left_top(), rect.right_top()], Stroke::new(1.0, LINE));
            let bars = ed.bars();
            let msg = format!("parses · {bars} bars{}", if ed.song().is_some_and(|s| s.chart.is_some()) { " · chart ok" } else { " · no chart" });
            p.text(egui::pos2(rect.left() + 12.0, rect.center().y), egui::Align2::LEFT_CENTER, msg, mono(12.0), GREEN);
        }
    }
}

/// "pulse2 · " for an error on a line of `[pulse2]`.
fn section_of(text: &str, line: usize) -> Option<String> {
    let secs = crate::model::sections(text);
    let s = secs.iter().rfind(|s| s.header < line)?;
    Some(format!("{} · ", s.name))
}

fn editor(ed: &mut Editor, ui: &mut egui::Ui, err_line: Option<usize>) {
    let mut text = ed.doc.text.clone();
    let font = FontId::monospace(12.5);
    let lines = text.lines().count().max(1) + usize::from(text.ends_with('\n'));
    let mut layouter = |ui: &egui::Ui, buf: &dyn TextBuffer, _wrap: f32| -> Arc<Galley> {
        let job = highlight(buf.as_str(), err_line, &FontId::monospace(12.5));
        ui.fonts_mut(|f| f.layout_job(job))
    };
    egui::ScrollArea::both().id_salt("text-scroll").auto_shrink(false).show(ui, |ui| {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            // Line numbers (painted beside the laid-out rows below).
            let (nums, _) = ui.allocate_exact_size(egui::vec2(48.0, 10.0), Sense::hover());
            let out = egui::TextEdit::multiline(&mut text)
                .id(text_id())
                .font(font.clone())
                .code_editor()
                .frame(egui::Frame::NONE)
                .margin(egui::Margin::symmetric(0, 10))
                .desired_width(f32::INFINITY)
                .desired_rows(lines)
                .lock_focus(true)
                .layouter(&mut layouter)
                .show(ui);
            let p = ui.painter();
            let mut line = 1;
            let mut new_line = true;
            for row in &out.galley.rows {
                if new_line {
                    let y = out.galley_pos.y + row.rect().center().y;
                    let c = if err_line == Some(line) { RED } else { ROW_NUM };
                    p.text(egui::pos2(nums.right() - 12.0, y), egui::Align2::RIGHT_CENTER, line.to_string(), font.clone(), c);
                    line += 1;
                }
                new_line = row.ends_with_newline;
            }
            if let Some(r) = out.cursor_range {
                ed.text.cursor = r.primary.index;
            }
            ed.text.focus = out.response.has_focus();
        });
    });
    // A cheat-sheet click: insert at the cursor.
    if let Some(ins) = ed.text.insert.take() {
        let at = ed.text.cursor.min(text.chars().count());
        let byte = text.char_indices().nth(at).map_or(text.len(), |(i, _)| i);
        text.insert_str(byte, &ins);
        let new = at + ins.chars().count();
        ed.text.cursor = new;
        if let Some(mut st) = egui::TextEdit::load_state(ui.ctx(), text_id()) {
            st.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(new))));
            st.store(ui.ctx(), text_id());
        }
        ui.ctx().memory_mut(|m| m.request_focus(text_id()));
    }
    if text != ed.doc.text {
        ed.set_text(text);
    }
}

/// Colours: section headers gold, each channel in its colour, comments faint, the error line
/// on red.
fn highlight(text: &str, err_line: Option<usize>, font: &FontId) -> LayoutJob {
    let mut job = LayoutJob::default();
    let mut section = String::new();
    for (i, line) in text.split_inclusive('\n').enumerate() {
        let (code, comment) = match line.find(';') {
            Some(k) => line.split_at(k),
            None => (line, ""),
        };
        let trimmed = code.trim();
        let header = trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() > 4 && trimmed[1..trimmed.len() - 1].chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if header {
            section = trimmed[1..trimmed.len() - 1].to_string();
        }
        let color = if header {
            GOLD
        } else {
            match section.as_str() {
                "song" => TEXT_DIM,
                "chords" => TEXT,
                "pulse1" => CHANNEL[0],
                "pulse2" => CHANNEL[1],
                "triangle" => CHANNEL[2],
                "noise" => CHANNEL[3],
                _ => TEXT,
            }
        };
        let bad = err_line == Some(i + 1);
        let bg = if bad { ERR_LINE_BG } else { Color32::TRANSPARENT };
        let fmt = |c: Color32| egui::TextFormat { font_id: font.clone(), color: if bad { ERR_TEXT } else { c }, background: bg, ..Default::default() };
        job.append(code, 0.0, fmt(color));
        if !comment.is_empty() {
            job.append(comment, 0.0, fmt(TEXT_FAINT));
        }
    }
    job.wrap.max_width = f32::INFINITY;
    job
}

fn cheat_sheet(ed: &mut Editor, ui: &mut egui::Ui) {
    let rect = ui.max_rect();
    ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(0x1a, 0x14, 0x10));
    ui.painter().line_segment([rect.left_top(), rect.left_bottom()], Stroke::new(1.0, LINE));
    egui::ScrollArea::vertical().id_salt("cheat").auto_shrink(false).show(ui, |ui| {
        egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 10)).show(ui, |ui| {
            ui.set_width(276.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("CHEAT SHEET").strong().color(GOLD).size(11.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("F1 · click to insert").color(TEXT_FAINT).size(11.0));
                });
            });
            ui.add_space(6.0);
            for (section, rows) in cheat_items() {
                ui.label(egui::RichText::new(section.to_uppercase()).color(TEXT_FAINT).size(11.0));
                for (syntax, meaning, ins) in rows {
                    let code = egui::RichText::new(syntax).strong().color(TEXT).size(11.0);
                    let what = egui::RichText::new(meaning).color(TEXT_DIM).size(11.0);
                    let r = if syntax.chars().count() <= 14 {
                        // A grid: the syntax in a column, the meaning beside it.
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            let s = ui
                                .allocate_ui_with_layout(egui::vec2(100.0, 14.0), egui::Layout::left_to_right(egui::Align::Min), |ui| {
                                    ui.set_min_width(100.0);
                                    ui.add(egui::Label::new(code).sense(Sense::click()))
                                })
                                .inner;
                            s.union(ui.add(egui::Label::new(what).wrap().sense(Sense::click())))
                        })
                        .inner
                    } else {
                        // Long syntax: on its own line, the meaning under it.
                        let s = ui.add(egui::Label::new(code).wrap().sense(Sense::click()));
                        let m = ui.horizontal_top(|ui| {
                            ui.add_space(108.0);
                            ui.add(egui::Label::new(what).wrap().sense(Sense::click()))
                        });
                        s.union(m.inner)
                    };
                    let r = r.on_hover_text(format!("insert `{}`", ins.trim()));
                    if r.clicked() {
                        ed.text.insert = Some(ins);
                    }
                    if r.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }
                ui.add_space(8.0);
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every entry of the grammar's cheat sheet is in the panel, word for word, in order.
    #[test]
    fn the_panel_covers_the_cheat_sheet() {
        let items = cheat_items();
        assert_eq!(items.len(), syntax::CHEAT_SHEET.len());
        for ((section, rows), (s2, items)) in syntax::CHEAT_SHEET.iter().zip(&items) {
            assert_eq!(section, s2);
            assert_eq!(rows.len(), items.len(), "{section}");
            for ((syntax, meaning), (a, b, ins)) in rows.iter().zip(items) {
                assert_eq!((syntax, meaning), (a, b));
                assert!(ins.contains(syntax), "{ins:?}");
            }
        }
        // Every section header and `[song]` key is there to insert.
        for s in syntax::SECTIONS {
            assert!(items.iter().flat_map(|(_, r)| r).any(|(_, _, ins)| ins.trim() == format!("[{s}]")), "[{s}]");
        }
        for k in syntax::SONG_KEYS {
            assert!(items.iter().flat_map(|(_, r)| r).any(|(_, _, ins)| ins.starts_with(&format!("{k} = "))), "{k}");
        }
    }
}
