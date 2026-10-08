//! The top bar: name, song picker, transport, position, tempo/key/meter, view tabs, Save.

use bevy_egui::egui::{self, Align2, Rect, Sense, Shape, Stroke, pos2, vec2};
use nat_han_adventures::audio::chart::pc_name;

use crate::app::{Editor, View, short_title};
use crate::theme::*;
use crate::views::tracker::seg;

pub fn ui(ed: &mut Editor, ui: &mut egui::Ui) {
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        ui.label(egui::RichText::new("NAT HAN · MUSIC").font(pixel(10.0)).color(GOLD));
        ui.label(egui::RichText::new("song").color(TEXT_DIM));
        let cur = ed.doc.stem.clone();
        let mut pick = None;
        egui::ComboBox::from_id_salt("song").selected_text(cur).width(170.0).show_ui(ui, |ui| {
            for s in &ed.songs {
                if ui.selectable_label(s.stem == ed.doc.stem, format!("{} — {}", s.stem, short_title(&s.title))).clicked() {
                    pick = Some(s.stem.clone());
                }
            }
        });
        if let Some(s) = pick {
            ed.open(&s);
        }
        // Transport.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let playing = ed.player.playing;
            if icon_button(ui, Icon::Play, playing, GOLD).on_hover_text("play from the cursor (space)").clicked() {
                if playing {
                    let bar = ed.cursor_bar();
                    ed.play_from(bar);
                } else {
                    ed.toggle_play();
                }
            }
            if icon_button(ui, Icon::Stop, false, TEXT).on_hover_text("stop (space)").clicked() {
                ed.player.stop();
            }
            let tip = format!("loop bars {:02}–{:02} (the selection: set it in the chart strip below)", ed.selection.0 + 1, ed.selection.1);
            if icon_button(ui, Icon::Loop, ed.loop_on, GOLD).on_hover_text(tip).clicked() {
                ed.loop_on = !ed.loop_on;
                ed.reloop();
            }
        });
        // Position.
        let song = ed.song().cloned();
        let (bb, bpm) = song.as_ref().map_or((4.0, 120.0), |s| (s.bar_beats(), s.bpm));
        let beat = ed.playhead().unwrap_or(ed.cursor_bar() as f64 * bb);
        let bar = (beat / bb).floor();
        let in_bar = beat - bar * bb;
        let tick = ((in_bar - in_bar.floor()) * 4.0).floor() as usize + 1;
        let secs = beat * 60.0 / bpm as f64;
        let total = song.as_ref().map_or(0.0, |s| s.beats() * 60.0 / s.bpm as f64);
        let lp = if ed.loop_on { format!(" · loop {:02}–{:02}", ed.selection.0 + 1, ed.selection.1) } else { String::new() };
        let sub = format!("{} / {}{lp}", clock(secs), clock(total));
        let (rect, _) = ui.allocate_exact_size(vec2(140.0, 36.0), Sense::hover());
        let p = ui.painter_at(rect);
        p.text(pos2(rect.left(), rect.top() + 9.0), Align2::LEFT_CENTER, format!("BAR {:02} · {} · {tick}", bar as usize + 1, in_bar.floor() as usize + 1), egui::FontId::monospace(14.0), TEXT);
        p.text(pos2(rect.left(), rect.top() + 27.0), Align2::LEFT_CENTER, sub, mono(11.0), TEXT_FAINT);
        if let Some(s) = &song {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                kv(ui, "bpm", &format!("{}", s.bpm));
                kv(ui, "swing", &format!("{:.2}", s.swing));
                kv(ui, "key", pc_name(s.key));
                ui.label(egui::RichText::new(s.meter.to_string()).color(TEXT_DIM));
            });
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let save = egui::Button::new(egui::RichText::new(format!("Save {}", ed.doc.path())).color(if ed.doc.dirty() { GOLD } else { TEXT }))
                .fill(RAISED)
                .stroke(Stroke::new(1.0, if ed.doc.dirty() { GOLD } else { LINE }))
                .min_size(vec2(0.0, 32.0));
            if ui.add(save).on_hover_text("ctrl+S · checked with the real parser first").clicked() {
                ed.save();
            }
            ui.spacing_mut().item_spacing.x = 0.0;
            for (v, label) in [(View::Text, "Text"), (View::Piano, "Piano roll"), (View::Tracker, "Tracker")] {
                if seg(ui, ed.view == v, label).clicked() {
                    ed.set_view(v);
                }
            }
        });
    });
}

fn kv(ui: &mut egui::Ui, k: &str, v: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(egui::RichText::new(k).color(TEXT_DIM));
        ui.label(egui::RichText::new(v).strong().color(TEXT));
    });
}

fn clock(secs: f64) -> String {
    let s = secs.max(0.0);
    format!("{:02}:{:04.1}", (s / 60.0).floor() as u32, s % 60.0)
}

#[derive(Clone, Copy)]
enum Icon {
    Play,
    Stop,
    Loop,
}

fn icon_button(ui: &mut egui::Ui, icon: Icon, on: bool, accent: egui::Color32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::click());
    let p = ui.painter_at(rect);
    let (fill, fg, stroke) = match icon {
        Icon::Play => (if on { HILITE } else { GOLD }, if on { GOLD } else { BG }, if on { GOLD } else { GOLD }),
        Icon::Loop if on => (RAISED, accent, accent),
        _ => (RAISED, TEXT, LINE),
    };
    let fill = if resp.hovered() { fill.gamma_multiply(1.2) } else { fill };
    p.rect(rect.shrink(0.5), 4.0, fill, Stroke::new(1.0, stroke), egui::StrokeKind::Inside);
    let c = rect.center();
    match icon {
        Icon::Play => {
            p.add(Shape::convex_polygon(vec![pos2(c.x - 5.0, c.y - 6.0), pos2(c.x + 6.0, c.y), pos2(c.x - 5.0, c.y + 6.0)], fg, Stroke::NONE));
        }
        Icon::Stop => {
            p.rect_filled(Rect::from_center_size(c, vec2(10.0, 10.0)), 0.0, fg);
        }
        Icon::Loop => {
            let s = Stroke::new(1.6, fg);
            p.add(Shape::line(vec![pos2(c.x - 5.0, c.y + 2.0), pos2(c.x - 5.0, c.y - 2.0), pos2(c.x - 3.0, c.y - 4.0), pos2(c.x + 4.0, c.y - 4.0)], s));
            p.add(Shape::line(vec![pos2(c.x + 2.0, c.y - 6.0), pos2(c.x + 4.0, c.y - 4.0), pos2(c.x + 2.0, c.y - 2.0)], s));
            p.add(Shape::line(vec![pos2(c.x + 5.0, c.y - 2.0), pos2(c.x + 5.0, c.y + 2.0), pos2(c.x + 3.0, c.y + 4.0), pos2(c.x - 4.0, c.y + 4.0)], s));
            p.add(Shape::line(vec![pos2(c.x - 2.0, c.y + 2.0), pos2(c.x - 4.0, c.y + 4.0), pos2(c.x - 2.0, c.y + 6.0)], s));
        }
    }
    resp
}
