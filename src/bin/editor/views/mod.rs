//! Drawing: the panels of the approved layout, and the three views of the song.

pub mod bottom;
pub mod feed;
pub mod left;
pub mod piano;
pub mod text;
pub mod top;
pub mod tracker;

use bevy_egui::egui::{self, Key, Stroke};

use crate::app::{Editor, View};
use crate::feed::Button;
use crate::theme::*;

/// The whole window, one frame.
pub fn frame(ed: &mut Editor, ctx: &egui::Context) {
    keys(ed, ctx);
    let mut root = egui::Ui::new(ctx.clone(), "root".into(), egui::UiBuilder::new().layer_id(egui::LayerId::background()).max_rect(ctx.viewport_rect()));
    root.painter().rect_filled(ctx.viewport_rect(), 0.0, BG);
    let panel = |fill| egui::Frame::new().fill(fill).stroke(Stroke::NONE);
    egui::Panel::top("top")
        .exact_size(56.0)
        .resizable(false)
        .frame(panel(PANEL).inner_margin(egui::Margin::symmetric(14, 0)))
        .show_separator_line(true)
        .show_inside(&mut root, |ui| top::ui(ed, ui));
    egui::Panel::bottom("bottom").exact_size(204.0).resizable(false).frame(panel(PANEL)).show_inside(&mut root, |ui| bottom::ui(ed, ui));
    egui::Panel::left("left").exact_size(220.0).resizable(false).frame(panel(PANEL)).show_inside(&mut root, |ui| left::ui(ed, ui));
    egui::Panel::right("feed").exact_size(320.0).resizable(false).frame(panel(PANEL)).show_inside(&mut root, |ui| feed::ui(ed, ui));
    egui::CentralPanel::default().frame(panel(BG)).show_inside(&mut root, |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
        match ed.view {
            View::Tracker => tracker::ui(ed, ui),
            View::Piano => piano::ui(ed, ui),
            View::Text => text::ui(ed, ui),
        }
    });
}

/// Shortcuts: F1 cheat sheet, space play/stop, ctrl+S save, ctrl+enter play from the text
/// cursor, the feed's letters (unless typing or recording), the tracker's keys.
fn keys(ed: &mut Editor, ctx: &egui::Context) {
    let typing = ctx.egui_wants_keyboard_input();
    let (f1, space, save, run) = ctx.input(|i| {
        (
            i.key_pressed(Key::F1),
            i.key_pressed(Key::Space),
            i.modifiers.command && i.key_pressed(Key::S),
            i.modifiers.command && i.key_pressed(Key::Enter),
        )
    });
    if f1 {
        ed.text.cheat = !ed.text.cheat;
        if ed.view != View::Text {
            ed.view = View::Text;
            ed.text.cheat = true;
        }
    }
    if save {
        ed.save();
    }
    if run {
        let bar = ed.cursor_bar();
        ed.play_from(bar);
    }
    if typing {
        return;
    }
    if space {
        ed.toggle_play();
    }
    let recording = ed.view == View::Tracker && ed.tracker.rec;
    if !recording && !ctx.input(|i| i.modifiers.command) {
        let pressed: Vec<Button> = ctx.input(|i| Button::ALL.into_iter().filter(|b| key_of(b.key()).is_some_and(|k| i.key_pressed(k))).collect());
        for b in pressed {
            ed.press(b);
        }
    }
    if ed.view == View::Tracker {
        tracker::keys(ed, ctx);
    }
}

fn key_of(c: char) -> Option<Key> {
    Key::from_name(&c.to_string())
}
