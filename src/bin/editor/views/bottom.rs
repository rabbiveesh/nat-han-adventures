//! The bottom strip: the chord chart as written and as played (changes in gold), each
//! musician's phrase plan (solid = committed, dashed = planned), the playhead, and the loop
//! selection (drag across the bar numbers).

use bevy_egui::egui::{self, Align2, Color32, Rect, Sense, Shape, Stroke, pos2, vec2};
use nat_han_adventures::audio::live::musician::{Role, Target};

use crate::app::Editor;
use crate::theme::*;

const LABEL_W: f32 = 120.0;
const SHOWN: usize = 16;

pub fn ui(ed: &mut Editor, ui: &mut egui::Ui) {
    let bars = ed.bars();
    let bb = ed.bar_beats();
    let playhead = ed.playhead();
    let focus_bar = playhead.map_or(ed.cursor_bar(), |b| (b / bb).floor() as usize);
    let first = (focus_bar / SHOWN) * SHOWN;
    let n = SHOWN.min(bars.saturating_sub(first)).max(1);
    let (written, played) = {
        let p = ed.played();
        (p.written_chart.clone(), p.played_chart.clone())
    };
    let rect = ui.available_rect_before_wrap().shrink2(vec2(12.0, 8.0));
    let resp = ui.allocate_rect(ui.available_rect_before_wrap(), Sense::hover());
    let p = ui.painter_at(resp.rect);
    let col = (rect.width() - LABEL_W) / SHOWN as f32;
    let x_of = |bar: f64| rect.left() + LABEL_W + (bar - first as f64) as f32 * col;
    let font = mono(11.0);
    // Bar numbers (drag to select the loop).
    let num_row = Rect::from_min_size(rect.min, vec2(rect.width(), 16.0));
    p.text(pos2(rect.left(), num_row.center().y), Align2::LEFT_CENTER, "BAR", font.clone(), TEXT_FAINT);
    let sel = ed.selection;
    for k in 0..n {
        let b = first + k;
        let x = x_of(b as f64);
        let in_sel = b >= sel.0 && b < sel.1;
        if in_sel {
            let r = Rect::from_min_max(pos2(x, num_row.top()), pos2(x + col, num_row.bottom()));
            if ed.loop_on {
                p.rect_filled(r, 0.0, GOLD.gamma_multiply(0.25));
            } else {
                p.line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(2.0, LINE));
            }
        }
        p.line_segment([pos2(x, num_row.top()), pos2(x, num_row.bottom())], Stroke::new(1.0, LINE));
        let c = if Some(b) == playhead.map(|h| (h / bb).floor() as usize) { GOLD } else { TEXT_FAINT };
        p.text(pos2(x + 4.0, num_row.center().y), Align2::LEFT_CENTER, format!("{:02}", b + 1), font.clone(), c);
    }
    let sel_resp = ui.interact(Rect::from_min_max(pos2(x_of(first as f64), num_row.top()), num_row.max), ui.id().with("sel"), Sense::click_and_drag());
    let bar_at = |x: f32| (first as f64 + ((x - rect.left() - LABEL_W) / col).floor() as f64).clamp(0.0, bars.saturating_sub(1) as f64) as usize;
    if (sel_resp.drag_started() || sel_resp.clicked())
        && let Some(pos) = sel_resp.interact_pointer_pos()
    {
        let b = bar_at(pos.x);
        ed.selection = (b, b + 1);
        ui.memory_mut(|m| m.data.insert_temp(ui.id().with("anchor"), b));
    }
    if sel_resp.dragged()
        && let Some(pos) = sel_resp.interact_pointer_pos()
    {
        let a: usize = ui.memory(|m| m.data.get_temp(ui.id().with("anchor"))).unwrap_or(ed.selection.0);
        let b = bar_at(pos.x);
        ed.selection = (a.min(b), a.max(b) + 1);
    }
    if (sel_resp.drag_stopped() || sel_resp.clicked()) && ed.loop_on {
        ed.reloop();
    }
    sel_resp.on_hover_text("drag to select bars to loop (the loop button plays them)");
    // Chart rows.
    let mut y = num_row.bottom() + 4.0;
    for (label, row, gold) in [("chart · written", &written, false), ("as played", &played, true)] {
        let r = Rect::from_min_size(pos2(rect.left(), y), vec2(rect.width(), 20.0));
        p.text(pos2(r.left(), r.center().y), Align2::LEFT_CENTER, label, font.clone(), if gold { GOLD } else { TEXT_DIM });
        for k in 0..n {
            let b = first + k;
            let x = x_of(b as f64);
            let text = row.get(b).cloned().unwrap_or_default();
            let changed = gold && written.get(b) != Some(&text);
            let cell = Rect::from_min_max(pos2(x, r.top()), pos2(x + col, r.bottom()));
            if changed {
                p.rect_filled(cell, 0.0, HILITE);
            }
            p.line_segment([cell.left_top(), cell.left_bottom()], Stroke::new(1.0, LINE));
            let c = if changed { GOLD } else { TEXT };
            ui.painter_at(cell.shrink2(vec2(2.0, 0.0))).text(pos2(x + 4.0, r.center().y), Align2::LEFT_CENTER, text, if changed { mono(11.0) } else { font.clone() }, c);
        }
        y = r.bottom() + 2.0;
    }
    if written.is_empty() {
        p.text(pos2(rect.left() + LABEL_W + 4.0, y - 22.0), Align2::LEFT_CENTER, "(no [chords]: the band can only play it as written)", font.clone(), TEXT_FAINT);
    }
    // Phrase plans.
    y += 6.0;
    let cap = p.text(pos2(rect.left(), y + 6.0), Align2::LEFT_CENTER, "PHRASE PLANS", font.clone(), TEXT_FAINT);
    p.text(pos2(cap.right() + 6.0, y + 6.0), Align2::LEFT_CENTER, "· solid = committed · dashed = planned, can still change", font.clone(), TEXT_FAINT);
    y += 16.0;
    let st = &ed.player.published.state;
    let pos = st.position;
    let playing = ed.player.playing;
    // Absolute engine bars → bars of the edited song (the waltz has two bars per written bar).
    let waltz = (ed.player.published.clock.beats_per_bar - ed.player.bar_beats).abs() > 1e-6;
    let to_song = |abs: u64| -> f64 {
        let rel = abs as f64 - pos.bar as f64 + pos.song_bar as f64;
        let rel = if waltz { rel / 2.0 } else { rel };
        rel + ed.player.offset as f64
    };
    let committed_to = st.upcoming.last().map(|b| b.slot.index + 1).unwrap_or(pos.bar);
    for (role, m) in Role::ALL.iter().zip(st.musicians.iter()) {
        let ch = role.channel();
        let r = Rect::from_min_size(pos2(rect.left(), y), vec2(rect.width(), 20.0));
        p.text(pos2(r.left(), r.center().y), Align2::LEFT_CENTER, ROLE[ch], font.clone(), CHANNEL[ch]);
        if playing && let Some(plan) = m.plan {
            let (a, z) = (plan.start, plan.start + plan.bars as u64);
            let split = committed_to.clamp(a, z);
            let label = plan_label(*role, &plan);
            for (from, to, solid) in [(a, split, true), (split, z, false)] {
                if from >= to {
                    continue;
                }
                let (xa, xb) = (x_of(to_song(from)).max(rect.left() + LABEL_W), x_of(to_song(to)).min(rect.right()));
                if xb <= xa + 2.0 {
                    continue;
                }
                let block = Rect::from_min_max(pos2(xa + 1.0, r.top() + 1.0), pos2(xb - 1.0, r.bottom() - 1.0));
                let text_c = if solid {
                    p.rect_filled(block, 3.0, CHANNEL[ch]);
                    BG
                } else {
                    dashed(&p, block, Stroke::new(1.0, CHANNEL[ch]));
                    CHANNEL[ch]
                };
                let text = if solid { label.clone() } else if split > a { "planned".into() } else { label.clone() };
                ui.painter_at(block.shrink2(vec2(4.0, 0.0))).text(pos2(block.left() + 6.0, block.center().y + if solid { 0.0 } else { -4.0 }), Align2::LEFT_CENTER, text, font.clone(), text_c);
            }
            // Bar by bar: what was played (committed), what's planned (ahead).
            let mut tip = Vec::new();
            for bar in a..z {
                let played = st.upcoming.iter().find(|c| c.slot.index == bar).map(|c| c.orns[ch]);
                let orns = played.unwrap_or(plan.intent(bar).orns);
                let names: Vec<&str> = orns.iter().map(|o| o.name()).collect();
                if !names.is_empty() {
                    tip.push(format!("bar {}: {}{}", to_song(bar).floor() as usize + 1, if played.is_some() { "" } else { "(planned) " }, names.join(", ")));
                }
                let (xa, xb) = (x_of(to_song(bar)), x_of(to_song(bar + 1)));
                if xa < rect.left() + LABEL_W || xb > rect.right() || played.is_some() || names.is_empty() {
                    continue;
                }
                let short: String = names[0].chars().take(((xb - xa) / 7.0).max(3.0) as usize).collect();
                ui.painter_at(Rect::from_min_max(pos2(xa + 2.0, r.top()), pos2(xb - 2.0, r.bottom())))
                    .text(pos2(xa + 4.0, r.center().y + 5.0), Align2::LEFT_CENTER, short, mono(9.0), CHANNEL[ch].gamma_multiply(0.8));
            }
            let hover = ui.interact(Rect::from_min_max(pos2(rect.left() + LABEL_W, r.top()), r.max), ui.id().with(("lane", ch)), Sense::hover());
            if !tip.is_empty() {
                hover.on_hover_text(tip.join("\n"));
            }
        } else if !playing && ch == 0 {
            p.text(pos2(rect.left() + LABEL_W + 4.0, r.center().y), Align2::LEFT_CENTER, "(the band's plans show while it plays)", font.clone(), TEXT_FAINT);
        }
        y = r.bottom() + 2.0;
    }
    // The playhead.
    if let Some(h) = playhead {
        let bar = h / bb;
        if bar >= first as f64 && bar < (first + SHOWN) as f64 {
            let x = x_of(bar);
            p.rect_filled(Rect::from_min_max(pos2(x - 3.0, rect.top()), pos2(x + 3.0, rect.bottom())), 0.0, GOLD.gamma_multiply(0.12));
            p.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(2.0, GOLD));
        }
    }
}

fn plan_label(role: Role, plan: &nat_han_adventures::audio::live::musician::PhrasePlan) -> String {
    let target = match plan.target {
        Target::Cadence => "→ cadence",
        Target::SectionEnd => "→ section end",
        Target::LoopEnd => "→ loop end",
    };
    let intents = &plan.intents[..plan.bars as usize];
    let mut extra = Vec::new();
    if intents.iter().any(|i| i.answer) {
        extra.push("answers the toot".to_string());
    }
    if intents.iter().any(|i| i.wah) {
        extra.push("wah-wah".to_string());
    }
    if intents.iter().any(|i| i.short_fill || i.accent) {
        extra.push("checkpoint fill".to_string());
    }
    if intents.iter().any(|i| i.switch) {
        extra.push("switches instrument".to_string());
    }
    if role == Role::Drums && intents.iter().any(|i| i.fill) {
        extra.push("fill".to_string());
    }
    let n = plan.orns().iter().count();
    if n > 0 {
        extra.push(format!("{n} ornaments planned"));
    }
    let mut s = format!("{} bars {target}", plan.bars);
    for e in extra {
        s += " · ";
        s += &e;
    }
    s
}

fn dashed(p: &egui::Painter, r: Rect, stroke: Stroke) {
    let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    p.extend(Shape::dashed_line(&pts, stroke, 4.0, 3.0));
    let _ = Color32::TRANSPARENT;
}
