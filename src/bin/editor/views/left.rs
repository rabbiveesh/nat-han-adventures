//! The left panel: songs, channels (mute, solo, level), parse status.

use bevy_egui::egui::{self, Rect, Sense, Stroke, pos2, vec2};

use crate::app::{Editor, short_title};
use crate::theme::*;

pub fn ui(ed: &mut Editor, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing.y = 0.0;
    ui.add_space(10.0);
    pad(ui, |ui| caption(ui, "SONGS · music/"));
    ui.add_space(6.0);
    let mut open = None;
    for s in &ed.songs {
        let active = s.stem == ed.doc.stem;
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 31.0), Sense::click());
        let p = ui.painter_at(rect);
        if active {
            p.rect_filled(rect, 0.0, HILITE);
            p.rect_filled(Rect::from_min_size(rect.min, vec2(3.0, rect.height())), 0.0, GOLD);
        } else if resp.hovered() {
            p.rect_filled(rect, 0.0, RAISED);
        }
        let dirty = if active && ed.doc.dirty() { " •" } else { "" };
        p.text(pos2(rect.left() + 12.0, rect.top() + 9.0), egui::Align2::LEFT_CENTER, format!("{}.song{dirty}", s.stem), mono(12.0), TEXT);
        p.text(pos2(rect.left() + 12.0, rect.top() + 22.0), egui::Align2::LEFT_CENTER, short_title(&s.title), mono(11.0), TEXT_FAINT);
        if resp.clicked() && !active {
            open = Some(s.stem.clone());
        }
    }
    if let Some(s) = open {
        ed.open(&s);
    }
    ui.add_space(6.0);
    sep(ui);
    ui.add_space(10.0);
    pad(ui, |ui| caption(ui, "CHANNELS"));
    ui.add_space(4.0);
    let mut remix = false;
    for ch in 0..4 {
        pad(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 2.0);
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                let (chip, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                ui.painter().rect_filled(chip, 2.0, CHANNEL[ch]);
                let name = ["Pulse 1", "Pulse 2", "Triangle", "Noise"][ch];
                ui.label(egui::RichText::new(format!("{name} · {}", ROLE[ch])).strong().color(TEXT));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if small_toggle(ui, ed.solo[ch], "S", GOLD).on_hover_text("solo").clicked() {
                        ed.solo[ch] = !ed.solo[ch];
                        remix = true;
                    }
                    if small_toggle(ui, ed.mute[ch], "M", RED).on_hover_text("mute").clicked() {
                        ed.mute[ch] = !ed.mute[ch];
                        remix = true;
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("vol").color(TEXT_FAINT).size(11.0));
                let w = ui.available_width() - 30.0;
                let (rect, resp) = ui.allocate_exact_size(vec2(w, 12.0), Sense::click_and_drag());
                let track = Rect::from_center_size(rect.center(), vec2(rect.width(), 4.0));
                ui.painter().rect_filled(track, 2.0, LINE);
                let frac = ed.level[ch] as f32 / 15.0;
                ui.painter().rect_filled(Rect::from_min_size(track.min, vec2(track.width() * frac, 4.0)), 2.0, CHANNEL[ch]);
                if let Some(pos) = resp.interact_pointer_pos()
                    && (resp.dragged() || resp.clicked())
                {
                    let v = (((pos.x - rect.left()) / rect.width()) * 15.0).round().clamp(0.0, 15.0) as u8;
                    if v != ed.level[ch] {
                        ed.level[ch] = v;
                        remix = true;
                    }
                }
                ui.label(egui::RichText::new(ed.level[ch].to_string()).color(TEXT_DIM).size(11.0));
            });
        });
    }
    if remix {
        ed.remix();
    }
    // Status at the bottom.
    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
        ui.add_space(10.0);
        pad(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            if let Some((msg, bad)) = &ed.status {
                let short: String = msg.lines().next().unwrap_or("").chars().take(120).collect();
                ui.add(egui::Label::new(egui::RichText::new(short).size(11.0).color(if *bad { ERR_TEXT } else { TEXT_DIM })).wrap_mode(egui::TextWrapMode::Wrap))
                    .on_hover_text(msg);
            }
            let out = if !ed.player.has_output() {
                ("no audio device".to_string(), RED)
            } else if ed.player.playing {
                (format!("engine playing{}", if ed.player.headless() { " (headless)" } else { "" }), GREEN)
            } else {
                ("engine stopped".to_string(), TEXT_FAINT)
            };
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(out.0).size(11.0).color(TEXT_FAINT));
                ui.label(egui::RichText::new("●").size(11.0).color(out.1));
            });
            let mut sfx = ed.player.sfx_on;
            if ui.checkbox(&mut sfx, egui::RichText::new("game sfx on the buttons").size(11.0).color(TEXT_FAINT)).changed() {
                ed.player.sfx_on = sfx;
            }
            let mut job = egui::text::LayoutJob::default();
            let fmt = |c| egui::TextFormat { font_id: mono(11.0), color: c, ..Default::default() };
            job.append("parse: ", 0.0, fmt(TEXT_FAINT));
            match ed.doc.error() {
                Some(e) => job.append(&format!("line {}", e.line), 0.0, fmt(RED)),
                None => job.append("ok", 0.0, fmt(GREEN)),
            }
            let chart = if ed.song().is_some_and(|s| s.chart.is_some()) { "chart ok" } else { "no chart" };
            job.append(&format!(" · {} bars · {chart}", ed.bars()), 0.0, fmt(TEXT_FAINT));
            ui.add(egui::Label::new(job).wrap());
        });
        ui.add_space(10.0);
        sep(ui);
    });
}

fn pad(ui: &mut egui::Ui, f: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 0)).show(ui, f);
}

fn sep(ui: &mut egui::Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, LINE);
}

fn small_toggle(ui: &mut egui::Ui, on: bool, label: &str, color: egui::Color32) -> egui::Response {
    let text = egui::RichText::new(label).size(11.0).strong().color(if on { BG } else { TEXT_DIM });
    ui.add(egui::Button::new(text).fill(if on { color } else { RAISED }).stroke(Stroke::new(1.0, if on { color } else { LINE })).min_size(vec2(22.0, 22.0)).corner_radius(3))
}
