//! The piano roll: one channel's notes as blocks, the others as ghosts, the band's version as a
//! dashed gold overlay, the chord lane on top and the velocities underneath. Snap 1/16.
//!
//! Mouse: click or drag on empty space draws a note (drag for its length); drag a note to move
//! it, its right edge to resize it; right-click deletes; drag a velocity bar to set the volume.

use bevy_egui::egui::{self, Align2, Color32, CursorIcon, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, pos2, vec2};

use crate::app::{Drag, Editor};
use crate::model::{DRUMS, EPS, HIGHEST, LOWEST, Note, Part, STEP, SongDoc, Sound, drum_letter};
use crate::theme::*;
use crate::views::tracker::{harmony_name, line, note_name, seg};

const ROW: f32 = 18.0;
const DRUM_ROW: f32 = 36.0;
const KEYS_W: f32 = 56.0;
const VEL_H: f32 = 64.0;

/// Draw a note at `step`, `len` steps long.
pub fn draw(doc: &mut SongDoc, ch: usize, step: usize, sound: Sound, len: usize, volume: u8) -> Result<(), String> {
    doc.edit(ch, |p| {
        p.insert(Note { start: step as f64 * STEP, dur: len.max(1) as f64 * STEP, sound, volume, duty: duty_at(p, step), tie: false });
    })
}

fn duty_at(p: &Part, step: usize) -> u8 {
    p.notes.iter().rev().find(|n| n.start <= step as f64 * STEP).or(p.notes.first()).map_or(2, |n| n.duty)
}

/// Move note `i` to `step`, `semis` higher (drums: rows).
pub fn drag(doc: &mut SongDoc, ch: usize, i: usize, step: usize, semis: i32) -> Result<(), String> {
    doc.edit(ch, |p| {
        if let Some(n) = p.notes.get(i).copied() {
            p.move_note(i, step as f64 * STEP, n.sound.shifted(semis));
        }
    })
}

/// Make note `i` `len` steps long.
pub fn resize(doc: &mut SongDoc, ch: usize, i: usize, len: usize) -> Result<(), String> {
    doc.edit(ch, |p| {
        p.resize(i, len.max(1) as f64 * STEP);
    })
}

pub fn erase(doc: &mut SongDoc, ch: usize, i: usize) -> Result<(), String> {
    doc.edit(ch, |p| p.remove(i))
}

pub fn velocity(doc: &mut SongDoc, ch: usize, i: usize, v: u8) -> Result<(), String> {
    doc.edit(ch, |p| p.set_volume(i, v))
}

/// Vertical layout: the rows (top to bottom) as (pitch, label, black key).
fn rows(ch: usize) -> Vec<(u8, String, bool)> {
    if ch == 3 {
        return DRUMS.iter().enumerate().rev().map(|(i, d)| (i as u8, drum_name(*d).to_string(), false)).collect();
    }
    (LOWEST..=HIGHEST)
        .rev()
        .map(|n| {
            let black = matches!(n % 12, 1 | 3 | 6 | 8 | 10);
            let label = if n % 12 == 0 || !black { note_name(n).replace('-', "") } else { String::new() };
            (n, label, black)
        })
        .collect()
}

fn drum_name(d: nat_han_adventures::audio::mml::Drum) -> &'static str {
    use nat_han_adventures::audio::mml::Drum::*;
    match d {
        Kick => "kick",
        Snare => "snare",
        ClosedHat => "hat",
        OpenHat => "open hat",
    }
}

pub fn ui(ed: &mut Editor, ui: &mut egui::Ui) {
    let bars = ed.bars();
    let bb = ed.bar_beats();
    let ch = ed.piano.ch;
    let shown = ed.shown_harmony();
    let playhead = ed.playhead();
    if ed.piano.follow
        && let Some(b) = playhead
    {
        let bar = (b / bb).floor() as usize;
        if bar < ed.piano.first_bar || bar >= ed.piano.first_bar + ed.piano.bars {
            ed.piano.first_bar = bar - bar % ed.piano.bars.max(1);
        }
    }
    ed.piano.first_bar = ed.piano.first_bar.min(bars.saturating_sub(1));
    // Header.
    egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::symmetric(12, 0)).show(ui, |ui| {
        ui.set_height(36.0);
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for (k, role) in ROLE.iter().enumerate() {
                let on = k == ch;
                let text = egui::RichText::new(*role).size(12.0).color(if on { BG } else { TEXT_DIM });
                let b = egui::Button::new(if on { text.strong() } else { text }).fill(if on { CHANNEL[k] } else { RAISED }).stroke(Stroke::new(1.0, LINE)).corner_radius(0);
                if ui.add(b).clicked() && !on {
                    ed.piano.ch = k;
                    ed.piano.selected = None;
                    ed.piano.recenter = true;
                }
            }
            ui.spacing_mut().item_spacing.x = 12.0;
            ui.add_space(12.0);
            ui.label(egui::RichText::new("snap 1/16").color(TEXT_DIM));
            ui.spacing_mut().item_spacing.x = 10.0;
            let lens = [(1, "1/16"), (2, "1/8"), (4, "1/4"), (8, "1/2"), (16, "1")];
            let cur = lens.iter().find(|l| l.0 == ed.piano.len).map_or("?", |l| l.1);
            egui::ComboBox::from_id_salt("newlen").selected_text(format!("new {cur}")).width(70.0).show_ui(ui, |ui| {
                for (n, l) in lens {
                    ui.selectable_value(&mut ed.piano.len, n, l);
                }
            });
            toggle(ui, &mut ed.piano.ghosts, "ghosts", TEXT_DIM);
            toggle(ui, &mut ed.piano.overlay, &format!("played ({})", harmony_name(shown)), GOLD);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if ui.small_button("▶").clicked() {
                    ed.piano.first_bar = (ed.piano.first_bar + ed.piano.bars).min(bars.saturating_sub(1));
                    ed.piano.follow = false;
                }
                let last = (ed.piano.first_bar + ed.piano.bars).min(bars);
                ui.label(egui::RichText::new(format!("{:02}–{last:02}", ed.piano.first_bar + 1)).color(TEXT_DIM));
                if ui.small_button("◀").clicked() {
                    ed.piano.first_bar = ed.piano.first_bar.saturating_sub(ed.piano.bars);
                    ed.piano.follow = false;
                }
                ui.add_space(6.0);
                ui.spacing_mut().item_spacing.x = 0.0;
                for z in [8, 4, 2, 1] {
                    if seg(ui, ed.piano.bars == z, &format!("{z}")).clicked() {
                        ed.piano.bars = z;
                        ed.piano.first_bar -= ed.piano.first_bar % z;
                    }
                }
                ui.spacing_mut().item_spacing.x = 6.0;
                toggle(ui, &mut ed.piano.follow, "follow", TEXT_DIM);
            });
        });
    });
    line(ui);
    let Some(part) = ed.doc.part(ch) else {
        ui.label("nothing parsed yet");
        return;
    };
    let others: Vec<Part> = (0..4).filter(|&k| k != ch && k != 3 && ch != 3).filter_map(|k| ed.doc.part(k)).collect();
    let played = ed.played().parts.as_ref().map(|p| p[ch].clone());
    let (written_chart, played_chart) = {
        let p = ed.played();
        (p.written_chart.clone(), p.played_chart.clone())
    };
    let (b0, nb) = (ed.piano.first_bar, ed.piano.bars.min(bars.saturating_sub(ed.piano.first_bar)).max(1));
    let (t0, t1) = (b0 as f64 * bb, (b0 + nb) as f64 * bb);
    let width = ui.available_width();
    let grid_w = width - KEYS_W;
    let x_of = |left: f32, t: f64| left + KEYS_W + ((t - t0) / (t1 - t0)) as f32 * grid_w;

    // The chord lane.
    let (lane, _) = ui.allocate_exact_size(vec2(width, 26.0), Sense::hover());
    {
        let p = ui.painter_at(lane);
        p.rect_filled(lane, 0.0, PANEL);
        p.text(pos2(lane.left() + 6.0, lane.center().y), Align2::LEFT_CENTER, "chords", mono(12.0), TEXT_FAINT);
        for b in b0..b0 + nb {
            let x = x_of(lane.left(), b as f64 * bb);
            p.line_segment([pos2(x, lane.top()), pos2(x, lane.bottom())], Stroke::new(1.0, LINE));
            // `%` spelled out: the chord it repeats.
            let resolve = |chart: &[String]| chart[..=b.min(chart.len().saturating_sub(1))].iter().rev().find(|s| *s != "%").cloned().unwrap_or_default();
            let (w, pl) = if b < written_chart.len() { (resolve(&written_chart), resolve(&played_chart)) } else { (String::new(), String::new()) };
            let (w, pl) = (w.as_str(), pl.as_str());
            let (text, color) = if !pl.is_empty() && pl != w { (format!("{w} → {pl}"), GOLD) } else { (w.to_string(), TEXT) };
            let font = mono(12.0);
            let clip = Rect::from_min_max(pos2(x, lane.top()), pos2(x_of(lane.left(), (b + 1) as f64 * bb), lane.bottom()));
            ui.painter_at(clip).text(pos2(x + 8.0, lane.center().y), Align2::LEFT_CENTER, text, font, color);
        }
    }
    line(ui);

    let rows = rows(ch);
    let row_h = if ch == 3 { DRUM_ROW } else { ROW };
    let y_index = |pitch: u8| rows.iter().position(|r| r.0 == pitch).unwrap_or(0);
    let avail = ui.available_height() - VEL_H - 1.0;
    let mut area = egui::ScrollArea::vertical().id_salt(("piano", ch)).max_height(avail).auto_shrink(false);
    if ed.piano.recenter {
        let mid = if part.notes.is_empty() {
            72
        } else {
            let lo = part.notes.iter().map(|n| n.sound.pitch()).min().unwrap_or(60);
            let hi = part.notes.iter().map(|n| n.sound.pitch()).max().unwrap_or(72);
            ((lo as u32 + hi as u32) / 2) as u8
        };
        area = area.vertical_scroll_offset((y_index(mid) as f32 * row_h - avail / 2.0).max(0.0));
        ed.piano.recenter = false;
    }
    let mut action: Option<Action> = None;
    let selected = ed.piano.selected;
    let new_len = ed.piano.len;
    let mut drag_state = ed.piano.drag;
    area.show(ui, |ui| {
        let (rect, resp) = ui.allocate_exact_size(vec2(width, rows.len() as f32 * row_h), Sense::click_and_drag());
        let p = ui.painter_at(rect);
        let grid = Rect::from_min_max(pos2(rect.left() + KEYS_W, rect.top()), rect.max);
        // Keys and lanes.
        for (i, (_, label, black)) in rows.iter().enumerate() {
            let y = rect.top() + i as f32 * row_h;
            let key = Rect::from_min_size(pos2(rect.left(), y), vec2(KEYS_W, row_h - 1.0));
            p.rect_filled(key, 0.0, if *black { KEY_BLACK } else { KEY_WHITE });
            p.text(pos2(key.left() + 6.0, key.center().y), Align2::LEFT_CENTER, label, mono(10.0), if *black { TEXT_FAINT } else { BG });
            let lane = Rect::from_min_size(pos2(grid.left(), y), vec2(grid.width(), row_h));
            p.rect_filled(lane, 0.0, if *black { LANE_BLACK } else { LANE_WHITE });
            p.line_segment([pos2(grid.left(), y + row_h - 0.5), pos2(grid.right(), y + row_h - 0.5)], Stroke::new(1.0, Color32::from_rgb(0x24, 0x1d, 0x16)));
        }
        // Grid lines.
        let steps = ((t1 - t0) / STEP).round() as usize;
        let spb = (bb / STEP).round() as usize;
        for s in 0..=steps {
            let x = x_of(rect.left(), t0 + s as f64 * STEP);
            let c = if s % spb == 0 { Color32::from_rgb(0x6b, 0x56, 0x40) } else if s % 4 == 0 { LINE } else { Color32::from_rgb(0x26, 0x1f, 0x18) };
            p.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(1.0, c));
        }
        let note_rect = |n: &Note| -> Option<Rect> {
            if n.end() <= t0 + EPS || n.start >= t1 - EPS {
                return None;
            }
            let i = rows.iter().position(|r| r.0 == n.sound.pitch())?;
            let y = rect.top() + i as f32 * row_h;
            let (xa, xb) = (x_of(rect.left(), n.start.max(t0)), x_of(rect.left(), n.end().min(t1)));
            Some(Rect::from_min_max(pos2(xa + 1.0, y + 1.0), pos2((xb - 1.0).max(xa + 3.0), y + row_h - 2.0)))
        };
        // Ghosts.
        if ed.piano.ghosts {
            for o in &others {
                for n in &o.notes {
                    for (k, r) in arp_rects(n, &rows, rect, row_h, &x_of, t0, t1) {
                        let _ = k;
                        p.rect_filled(r, 3.0, CHANNEL[o.ch].gamma_multiply(0.28));
                    }
                }
            }
        }
        // The written notes.
        for (i, n) in part.notes.iter().enumerate() {
            let Some(r) = note_rect(n) else { continue };
            // An arpeggio's other notes, lighter.
            for (k, ar) in arp_rects(n, &rows, rect, row_h, &x_of, t0, t1) {
                if k > 0 {
                    p.rect_filled(ar, 3.0, CHANNEL[ch].gamma_multiply(0.55));
                }
            }
            let sel = selected == Some(i);
            p.rect_filled(r, 3.0, CHANNEL[ch]);
            if sel {
                p.rect_stroke(r, 3.0, Stroke::new(2.0, TEXT), StrokeKind::Outside);
            }
            if n.tie {
                p.line_segment([pos2(r.left() - 2.0, r.center().y), pos2(r.left() + 3.0, r.center().y)], Stroke::new(2.0, TEXT));
            }
            let label = match n.sound {
                Sound::Note(x) => note_name(x).replace('-', ""),
                Sound::Arp(a) => format!("{{{}}}", a.notes().iter().map(|&x| note_name(x).replace('-', "")).collect::<Vec<_>>().join(" ")),
                Sound::Drum(d) => drum_letter(d).to_string(),
            };
            if r.width() > 22.0 {
                ui.painter_at(r).text(pos2(r.left() + 4.0, r.center().y), Align2::LEFT_CENTER, label, mono(10.0), BG);
            }
        }
        // As played.
        if ed.piano.overlay
            && let Some(pl) = &played
        {
            for n in &pl.notes {
                let same = part.notes.iter().any(|w| (w.start - n.start).abs() < EPS && w.sound == n.sound && (w.dur - n.dur).abs() < EPS);
                if same {
                    continue;
                }
                for (_, r) in arp_rects(n, &rows, rect, row_h, &x_of, t0, t1) {
                    dashed_rect(&p, r, Stroke::new(1.5, GOLD));
                }
            }
        }
        // The drag in progress.
        let pointer = resp.interact_pointer_pos().or(resp.hover_pos());
        let at = |pos: Pos2| -> (usize, usize) {
            let s = (t0 + ((pos.x - grid.left()) / grid.width()) as f64 * (t1 - t0)) / STEP;
            let row = (((pos.y - rect.top()) / row_h).floor().max(0.0) as usize).min(rows.len() - 1);
            (s.floor().max(0.0) as usize, row)
        };
        let hit = |pos: Pos2| -> Option<(usize, bool)> {
            part.notes.iter().enumerate().find_map(|(i, n)| {
                let r = note_rect(n)?;
                r.expand(1.0).contains(pos).then(|| (i, pos.x > r.right() - 6.0))
            })
        };
        if let Some(pos) = resp.hover_pos()
            && pos.x > grid.left()
        {
            let icon = match hit(pos) {
                Some((_, true)) => CursorIcon::ResizeHorizontal,
                Some(_) => CursorIcon::Grab,
                None => CursorIcon::Crosshair,
            };
            ui.ctx().set_cursor_icon(icon);
        }
        if resp.drag_started()
            && let Some(pos) = resp.interact_pointer_pos()
            && pos.x > grid.left()
        {
            let (s, row) = at(pos);
            drag_state = Some(match hit(pos) {
                Some((i, true)) => Drag::Resize { index: i },
                Some((i, false)) => {
                    let n = part.notes[i];
                    Drag::Move { index: i, grab: s as f64 * STEP - n.start, pitch: rows[row].0 as i32 }
                }
                None => Drag::Draw { step: s, sound_pitch: rows[row].0 as i32 },
            });
        }
        if let (Some(d), Some(pos)) = (drag_state, pointer) {
            let (s, row) = at(pos);
            let preview = match d {
                Drag::Draw { step, sound_pitch } => {
                    let len = if s > step { s + 1 - step } else { new_len };
                    Some((Note { start: step as f64 * STEP, dur: len as f64 * STEP, sound: sound_of(ch, sound_pitch as u8), volume: 12, duty: 2, tie: false }, Action::Draw(step, sound_pitch as u8, len)))
                }
                Drag::Move { index, grab, pitch } => part.notes.get(index).map(|n| {
                    let start = ((s as f64 * STEP - grab) / STEP).round().max(0.0) * STEP;
                    // (Drum rows: the drum's index, so the same sum.)
                    let semis = rows[row].0 as i32 - pitch;
                    (Note { start, sound: n.sound.shifted(semis), ..*n }, Action::Move(index, (start / STEP).round() as usize, semis))
                }),
                Drag::Resize { index } => part.notes.get(index).map(|n| {
                    let end_step = (s + 1).max((n.start / STEP).round() as usize + 1);
                    let len = end_step - (n.start / STEP).round() as usize;
                    (Note { dur: len as f64 * STEP, ..*n }, Action::Resize(index, len))
                }),
                Drag::Velocity { .. } => None,
            };
            if let Some((n, act)) = preview {
                if let Some(r) = note_rect(&n) {
                    p.rect_stroke(r, 3.0, Stroke::new(2.0, TEXT), StrokeKind::Outside);
                    p.rect_filled(r, 3.0, CHANNEL[ch].gamma_multiply(0.6));
                }
                if resp.drag_stopped() {
                    action = Some(act);
                    drag_state = None;
                }
            }
        }
        if resp.clicked()
            && let Some(pos) = resp.interact_pointer_pos()
            && pos.x > grid.left()
        {
            match hit(pos) {
                Some((i, _)) => action = Some(Action::Select(i)),
                None => {
                    let (s, row) = at(pos);
                    action = Some(Action::Draw(s, rows[row].0, new_len));
                }
            }
        }
        if resp.secondary_clicked()
            && let Some(pos) = resp.interact_pointer_pos()
            && let Some((i, _)) = hit(pos)
        {
            action = Some(Action::Erase(i));
        }
        // The playhead.
        if let Some(b) = playhead
            && b >= t0
            && b < t1
        {
            let x = x_of(rect.left(), b);
            p.rect_filled(Rect::from_min_max(pos2(x - 3.0, rect.top()), pos2(x + 3.0, rect.bottom())), 0.0, GOLD.gamma_multiply(0.15));
            p.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(2.0, GOLD));
        }
    });
    line(ui);
    // Velocities.
    let (vel, vresp) = ui.allocate_exact_size(vec2(width, VEL_H), Sense::click_and_drag());
    {
        let p = ui.painter_at(vel);
        p.rect_filled(vel, 0.0, PANEL);
        p.text(pos2(vel.left() + 6.0, vel.top() + 12.0), Align2::LEFT_CENTER, "vel", mono(12.0), TEXT_FAINT);
        let bar_h = VEL_H - 14.0;
        let mut bars_x = Vec::new();
        for (i, n) in part.notes.iter().enumerate() {
            if n.start < t0 - EPS || n.start >= t1 - EPS {
                continue;
            }
            let x = x_of(vel.left(), n.start) + 2.0;
            let mut v = n.volume;
            if let (Some(Drag::Velocity { index }), Some(pos)) = (drag_state, vresp.interact_pointer_pos())
                && index == i
            {
                v = (((vel.bottom() - 6.0 - pos.y) / bar_h * 15.0).round().clamp(0.0, 15.0)) as u8;
            }
            let h = v as f32 / 15.0 * bar_h;
            p.rect_filled(Rect::from_min_max(pos2(x, vel.bottom() - 6.0 - h), pos2(x + 6.0, vel.bottom() - 6.0)), 2.0, CHANNEL[ch]);
            bars_x.push((i, x));
        }
        if vresp.drag_started()
            && let Some(pos) = vresp.interact_pointer_pos()
            && let Some((i, _)) = bars_x.iter().min_by(|a, b| (a.1 - pos.x).abs().total_cmp(&(b.1 - pos.x).abs())).filter(|(_, x)| (x - pos.x).abs() < 12.0)
        {
            drag_state = Some(Drag::Velocity { index: *i });
        }
        if vresp.drag_stopped()
            && let Some(Drag::Velocity { index }) = drag_state
            && let Some(pos) = vresp.interact_pointer_pos()
        {
            let v = (((vel.bottom() - 6.0 - pos.y) / bar_h * 15.0).round().clamp(0.0, 15.0)) as u8;
            action = Some(Action::Velocity(index, v));
            drag_state = None;
        }
    }
    ed.piano.drag = drag_state;
    if let Some(a) = action {
        let r = match a {
            Action::Draw(step, pitch, len) => {
                let vol = part.notes.iter().rev().find(|n| n.start <= step as f64 * STEP).map_or(12, |n| n.volume);
                let r = draw(&mut ed.doc, ch, step, sound_of(ch, pitch), len, vol);
                ed.piano.selected = ed.doc.part(ch).and_then(|p| p.starting_in(step as f64 * STEP, (step + 1) as f64 * STEP).next());
                r
            }
            Action::Move(i, step, semis) => {
                let r = drag(&mut ed.doc, ch, i, step, semis);
                ed.piano.selected = ed.doc.part(ch).and_then(|p| p.starting_in(step as f64 * STEP, (step + 1) as f64 * STEP).next());
                r
            }
            Action::Resize(i, len) => resize(&mut ed.doc, ch, i, len),
            Action::Erase(i) => {
                ed.piano.selected = None;
                erase(&mut ed.doc, ch, i)
            }
            Action::Velocity(i, v) => velocity(&mut ed.doc, ch, i, v),
            Action::Select(i) => {
                ed.piano.selected = Some(i);
                Ok(())
            }
        };
        ed.edit(r);
    }
    // Delete the selected note.
    if !ui.ctx().egui_wants_keyboard_input()
        && ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
        && let Some(i) = ed.piano.selected.take()
    {
        let r = erase(&mut ed.doc, ch, i);
        ed.edit(r);
    }
}

#[derive(Debug, Clone, Copy)]
enum Action {
    Draw(usize, u8, usize),
    Move(usize, usize, i32),
    Resize(usize, usize),
    Erase(usize),
    Velocity(usize, u8),
    Select(usize),
}

fn sound_of(ch: usize, pitch: u8) -> Sound {
    if ch == 3 { Sound::Drum(DRUMS[(pitch as usize).min(3)]) } else { Sound::Note(pitch) }
}

/// A note's rectangles (an arpeggio: one per chord note, the written lowest first).
fn arp_rects(n: &Note, rows: &[(u8, String, bool)], rect: Rect, row_h: f32, x_of: &dyn Fn(f32, f64) -> f32, t0: f64, t1: f64) -> Vec<(usize, Rect)> {
    if n.end() <= t0 + EPS || n.start >= t1 - EPS {
        return Vec::new();
    }
    let pitches: Vec<u8> = match n.sound {
        Sound::Arp(a) => a.notes().to_vec(),
        s => vec![s.pitch()],
    };
    let (xa, xb) = (x_of(rect.left(), n.start.max(t0)), x_of(rect.left(), n.end().min(t1)));
    pitches
        .iter()
        .enumerate()
        .filter_map(|(k, p)| {
            let i = rows.iter().position(|r| r.0 == *p)?;
            let y = rect.top() + i as f32 * row_h;
            Some((k, Rect::from_min_max(pos2(xa + 1.0, y + 1.0), pos2((xb - 1.0).max(xa + 3.0), y + row_h - 2.0))))
        })
        .collect()
}

fn dashed_rect(p: &egui::Painter, r: Rect, stroke: Stroke) {
    let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    p.extend(Shape::dashed_line(&pts, stroke, 4.0, 3.0));
}

fn toggle(ui: &mut egui::Ui, v: &mut bool, label: &str, color: Color32) {
    let text = format!("{label} {}", if *v { "on" } else { "off" });
    if ui.add(egui::Button::new(egui::RichText::new(text).color(if *v { color } else { TEXT_FAINT })).frame(false)).clicked() {
        *v = !*v;
    }
}
