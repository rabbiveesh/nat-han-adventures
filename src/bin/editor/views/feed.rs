//! The right panel: the gameplay feed (buttons, auto-play Nat), the director's stats, decision
//! and physics, the force chips and the musicians' freedom.

use bevy_egui::egui::{self, Rect, Sense, Stroke, pos2, vec2};
use nat_han_adventures::audio::live::chorus::{Call as ChorusCall, Chorus};
use nat_han_adventures::audio::live::feel::Feel;
use nat_han_adventures::audio::tuning::Tuning;
use nat_han_adventures::audio::{Filters, Harmony};
use nat_han_adventures::game::Groove;

use crate::app::Editor;
use crate::feed::{Button, stat_bars};
use crate::theme::*;

pub fn ui(ed: &mut Editor, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().id_salt("feed").auto_shrink(false).show(ui, |ui| {
        egui::Frame::new().inner_margin(egui::Margin::same(12)).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("GAMEPLAY FEED").strong().color(TEXT));
                ui.label(egui::RichText::new("same inputs the game sends").color(TEXT_FAINT).size(11.0));
            });
            buttons(ed, ui);
            auto(ed, ui);
            stats(ed, ui);
            director(ed, ui);
            forces(ed, ui);
            musicians(ed, ui);
        });
    });
}

fn buttons(ed: &mut Editor, ui: &mut egui::Ui) {
    let w = (ui.available_width() - 18.0) / 4.0;
    egui::Grid::new("feed-buttons").spacing(vec2(6.0, 6.0)).show(ui, |ui| {
        for (i, b) in Button::ALL.into_iter().enumerate() {
            let (rect, resp) = ui.allocate_exact_size(vec2(w, 34.0), Sense::click());
            let flash = ed.game.log.back().is_some_and(|(t, l)| *l == b.label() && ed.game.now - t < 0.15);
            let fill = if resp.is_pointer_button_down_on() || flash { HILITE } else if resp.hovered() { HILITE.gamma_multiply(0.9) } else { RAISED };
            let p = ui.painter_at(rect);
            p.rect(rect.shrink(0.5), 4.0, fill, Stroke::new(1.0, if flash { GOLD } else { LINE }), egui::StrokeKind::Inside);
            p.text(rect.center() - vec2(0.0, 6.0), egui::Align2::CENTER_CENTER, b.label(), mono(11.0), TEXT);
            p.text(rect.center() + vec2(0.0, 8.0), egui::Align2::CENTER_CENTER, b.key().to_string(), mono(10.0), TEXT_FAINT);
            if resp.on_hover_text(format!("{} ({})", b.label(), b.key())).clicked() {
                ed.press(b);
            }
            if i % 4 == 3 {
                ui.end_row();
            }
        }
    });
}

fn auto(ed: &mut Editor, ui: &mut egui::Ui) {
    egui::Frame::new().fill(RAISED).stroke(Stroke::new(1.0, LINE)).corner_radius(4).inner_margin(egui::Margin::symmetric(8, 4)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.checkbox(&mut ed.auto.on, "auto-play Nat");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new("chaos").color(TEXT_FAINT));
                ui.spacing_mut().slider_width = 80.0;
                ui.add(egui::Slider::new(&mut ed.auto.chaos, 0.0..=1.0).show_value(false));
                ui.label(egui::RichText::new("calm").color(TEXT_FAINT));
            });
        });
    });
}

fn stats(ed: &mut Editor, ui: &mut egui::Ui) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 3.0;
        ui.spacing_mut().interact_size.y = 14.0;
        caption(ui, "LAST 20s OF PLAY");
        for (label, v, threshold) in stat_bars(&ed.game.band.stats) {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(112.0, 13.0), Sense::hover());
                ui.painter().text(r.left_center(), egui::Align2::LEFT_CENTER, label, mono(12.0), TEXT_DIM);
                let max = (threshold * 2).max(4) as f32;
                let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width() - 62.0, 13.0), Sense::hover());
                let track = Rect::from_center_size(rect.center(), vec2(rect.width(), 6.0));
                let hit = v >= threshold;
                ui.painter().rect_filled(track, 3.0, LINE);
                ui.painter().rect_filled(Rect::from_min_size(track.min, vec2(track.width() * (v as f32 / max).min(1.0), 6.0)), 3.0, if hit { GOLD } else { TEXT_DIM });
                let x = track.left() + track.width() * threshold as f32 / max;
                ui.painter().rect_filled(Rect::from_min_max(pos2(x - 1.0, track.top() - 3.0), pos2(x + 1.0, track.bottom() + 3.0)), 0.0, TEXT);
                let t = egui::RichText::new(format!("{v} / {threshold}"));
                ui.label(if hit { t.color(GOLD).strong() } else { t.color(TEXT) });
            });
        }
    });
}

fn harmony_label(h: Harmony) -> &'static str {
    match h {
        Harmony::Original => "as written",
        Harmony::Coltrane => "Coltrane changes",
        Harmony::Quartal => "quartal (McCoy Tyner)",
        Harmony::MelodicMinor => "melodic minor",
        Harmony::Waltz => "jazz waltz (3/4)",
    }
}

fn director(ed: &mut Editor, ui: &mut egui::Ui) {
    let st = &ed.player.published.state;
    let (filters, reason) = ed.game.decided.unwrap_or((Filters::default(), ""));
    // What the engine has been told (forces win), and where it comes in.
    let harmony = ed.dials.force_harmony.unwrap_or(filters.harmony);
    let laughing = ed.dials.force_tuning.map_or(filters.just_intonation, |t| t == Tuning::Medley);
    let landing = st.upcoming.last().map(|b| b.slot.index + 1).map(|abs| {
        let p = st.position;
        // The next uncommitted bar, in the edited song's bars.
        let song_bar = abs as i64 - p.bar as i64 + p.song_bar as i64;
        let bars = ed.player.bars.max(1) as i64;
        (song_bar.rem_euclid(bars) as usize) + ed.player.offset
    });
    egui::Frame::new().fill(HILITE).stroke(Stroke::new(1.0, BAR_LINE)).corner_radius(4).inner_margin(8).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 3.0;
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 14.0), Sense::hover());
        ui.painter().text(r.left_center(), egui::Align2::LEFT_CENTER, "DIRECTOR", mono(11.0), TEXT_FAINT);
        ui.painter().text(r.right_center(), egui::Align2::RIGHT_CENTER, if ed.player.playing { format!("next check {:.1}s", ed.game.next_check_in()) } else { "play time stopped".into() }, mono(11.0), TEXT_FAINT);
        let title = match ed.dials.force_harmony {
            Some(h) if h != filters.harmony => format!("FORCED: {}", if h == Harmony::Original { "AS WRITTEN" } else { h.label() }),
            _ if !reason.is_empty() => reason.to_string(),
            _ if filters == Filters::default() => "PLAYING IT STRAIGHT".into(),
            _ => filters.label(),
        };
        ui.label(egui::RichText::new(title).strong().color(GOLD).size(14.0));
        let sounding = st.harmony;
        let at = match (ed.player.playing, landing) {
            (true, Some(b)) if sounding != harmony => format!(", from bar {:02}", b + 1),
            (true, _) => ", sounding".into(),
            _ => String::new(),
        };
        let forced = if ed.dials.force_harmony.is_some() {
            " (forced)".to_string()
        } else {
            ed.game.holding().map(|(_, s)| format!(" (held {s:.0}s)")).unwrap_or_default()
        };
        ui.label(format!("→ {}{forced}{at}", harmony_label(harmony)));
        let g = Groove::new(Filters { harmony, just_intonation: laughing });
        let mut phys = format!("physics: gravity ×{:.2} · run ×{:.2}", g.gravity_scale, g.speed_scale);
        if g.time_scale != 1.0 {
            phys += &format!(" · time ×{:.2}", g.time_scale);
        }
        if g.bounce {
            phys += " · bouncy";
        }
        ui.label(egui::RichText::new(phys).color(TEXT_DIM));
        let note = if ed.player.playing {
            st.medley_phrase.map(|t| format!("tuning: the medley, this phrase {}", t.slug())).unwrap_or_else(|| format!("tuning: {}", st.tuning.slug()))
        } else {
            "press play: the band plays what the game would".into()
        };
        ui.label(egui::RichText::new(note).color(TEXT_FAINT).size(11.0));
    });
}

fn forces(ed: &mut Editor, ui: &mut egui::Ui) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        caption(ui, "FORCE HARMONY");
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            ui.spacing_mut().button_padding = vec2(7.0, 2.0);
            let chips: [(Option<Harmony>, &str); 6] = [
                (None, "auto"),
                (Some(Harmony::Original), "original"),
                (Some(Harmony::Coltrane), "coltrane"),
                (Some(Harmony::Quartal), "quartal"),
                (Some(Harmony::MelodicMinor), "mel. minor"),
                (Some(Harmony::Waltz), "waltz"),
            ];
            for (h, label) in chips {
                let can = h.is_none_or(|h| ed.song().is_some_and(|s| s.chart.is_some() || h == Harmony::Original) && (h != Harmony::Waltz || ed.song().is_some_and(|s| s.meter.beats == 4)));
                if chip(ui, ed.dials.force_harmony == h, can, label).clicked() {
                    ed.set_force_harmony(h);
                }
            }
        });
        ui.add_space(4.0);
        caption(ui, "FORCE FEEL");
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            ui.spacing_mut().button_padding = vec2(7.0, 2.0);
            let mut chips: Vec<(Option<Feel>, &str)> = vec![(None, "auto")];
            chips.extend(Feel::ALL.map(|f| (Some(f), f.slug())));
            for (f, label) in chips {
                if chip(ui, ed.dials.force_feel == f, true, label).clicked() {
                    ed.set_force_feel(f);
                }
            }
        });
        let st = &ed.player.published.state;
        let note = match (ed.dials.force_feel, st.feel) {
            (None, Feel::Swing) => "the band picks (from 0.4 freedom: a section now and then)".to_string(),
            (_, f) if ed.player.playing && f != Feel::Swing => format!("sounding: {}", f.label().to_lowercase()),
            (Some(f), _) => format!("forced: {} from the next bar", f.slug()),
            (None, _) => String::new(),
        };
        ui.label(egui::RichText::new(note).color(TEXT_FAINT).size(11.0));
        ui.add_space(4.0);
        caption(ui, "FORCE CHORUS");
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            ui.spacing_mut().button_padding = vec2(7.0, 2.0);
            let mut chips: Vec<(Option<ChorusCall>, &str)> = vec![(None, "auto")];
            chips.extend(Chorus::ALL.map(|chorus| (Some(ChorusCall { chorus, key_up: false }), chorus.slug())));
            chips.push((Some(ChorusCall { chorus: Chorus::Shout, key_up: true }), "shout +1/2"));
            for (c, label) in chips {
                if chip(ui, ed.dials.force_chorus == c, true, label).clicked() {
                    ed.set_force_chorus(c);
                }
            }
            if chip(ui, false, ed.player.playing, "END").on_hover_text("the band plays an ending from the next bar").clicked() {
                ed.end_song();
            }
        });
        let st = &ed.player.published.state;
        let note = match (ed.dials.force_chorus, st.label) {
            (_, l) if ed.player.playing && !l.is_empty() => {
                format!("sounding: {}", l.to_lowercase())
            }
            (None, _) => "the band arranges (from 0.25 freedom: stop-time, breaks, ...)".to_string(),
            (Some(c), _) => format!("forced: {} from the next bar", c.chorus.slug()),
        };
        ui.label(egui::RichText::new(note).color(TEXT_FAINT).size(11.0));
        ui.add_space(4.0);
        caption(ui, "FORCE TUNING");
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            ui.spacing_mut().button_padding = vec2(7.0, 2.0);
            if chip(ui, ed.dials.force_tuning.is_none(), true, "auto").clicked() {
                ed.set_force_tuning(None);
            }
            for t in Tuning::ALL {
                let label = match t {
                    Tuning::Equal => "equal",
                    Tuning::Just => "just",
                    Tuning::CarlosAlpha => "alpha",
                    Tuning::BohlenPierce => "B-P",
                    Tuning::Tet7 => "7-tet",
                    Tuning::Harmonic => "harm.",
                    Tuning::Drunk => "drunk",
                    Tuning::Medley => "medley",
                };
                if chip(ui, ed.dials.force_tuning == Some(t), true, label).clicked() {
                    ed.set_force_tuning(Some(t));
                }
            }
        });
    });
}

fn musicians(ed: &mut Editor, ui: &mut egui::Ui) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.spacing_mut().interact_size.y = 16.0;
        caption(ui, "MUSICIANS · FREEDOM");
        let mut f = ed.dials.freedom;
        let mut changed = false;
        for (k, label) in ["lead", "comp", "bass", "drums", "dynamics"].iter().enumerate() {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(76.0, 16.0), Sense::hover());
                ui.painter().text(r.left_center(), egui::Align2::LEFT_CENTER, *label, mono(12.0), TEXT_DIM);
                ui.spacing_mut().slider_width = ui.available_width() - 48.0;
                changed |= ui.add(egui::Slider::new(&mut f[k], 0.0..=1.0).show_value(false)).changed();
                ui.label(format!("{:.2}", f[k]));
            });
        }
        if changed {
            ed.set_freedom(f);
        }
    });
}
