//! Drawing: the toolbar, the palette and tools (left), the canvas (the level drawn with the
//! game's own sprites, its marks, tweaks and the validator's reach map), and the level's
//! settings, the cell inspector and the validator's findings (right).

use std::collections::HashMap;

use bevy_egui::egui::{self, Color32, Key, Pos2, Rect, Sense, Stroke, TextureId, pos2, vec2};
use nat_han_adventures::art::SpriteId;
use nat_han_adventures::level::doc::{LevelDoc, key_of, value_of};
use nat_han_adventures::level::tweak::{HanZone, KEYS};
use nat_han_adventures::level::{GLYPHS, Glyph, GlyphGroup, Topic, Tweak, glyph_info};

use crate::app::{Editor, HanProof, Tool, slug};
use crate::theme::*;

/// The sprites the canvas draws, as egui textures.
pub type Icons = HashMap<SpriteId, TextureId>;

/// The sprites to have as textures for world `w`.
pub fn wanted(w: u8) -> Vec<SpriteId> {
    use SpriteId::*;
    vec![
        GroundTop(w),
        GroundFill(w),
        OneWay(w),
        Grease(w),
        LiquidTop(w),
        LiquidFill(w),
        SpikesUp,
        SpikesDown,
        Fly,
        SprayCan,
        Nugget,
        CheckpointOff,
        GoalFlag,
        PooIdle,
        HanIdle,
        PlatformTp,
        PlatformDuck,
        PlatformPlunger,
    ]
}

/// Ready-made tweaks for the "add" menu.
const PRESETS: [(&str, &str); 12] = [
    ("ice", "friction=0"),
    ("sticky", "friction=3 jump=0.7"),
    ("trampoline", "bounce=1"),
    ("springy floor", "jump=1.5"),
    ("belt →", "belt=3"),
    ("low gravity", "gravity=0.4"),
    ("updraft", "gravity=-0.6"),
    ("wind ←", "wind=-3"),
    ("Han: avoid", "han=avoid"),
    ("Han: weak boost", "han=weak"),
    ("Han: full boost", "han=boost"),
    ("Han: wait here", "han=wait"),
];

pub fn frame(ed: &mut Editor, ctx: &egui::Context, icons: &Icons) {
    keys(ed, ctx);
    let mut root = egui::Ui::new(ctx.clone(), "root".into(), egui::UiBuilder::new().layer_id(egui::LayerId::background()).max_rect(ctx.viewport_rect()));
    root.painter().rect_filled(ctx.viewport_rect(), 0.0, BG);
    let panel = |fill| egui::Frame::new().fill(fill).stroke(Stroke::NONE).inner_margin(egui::Margin::same(10));
    egui::Panel::top("top").exact_size(44.0).resizable(false).frame(panel(PANEL)).show_inside(&mut root, |ui| top(ed, ui));
    egui::Panel::bottom("bottom").exact_size(26.0).resizable(false).frame(panel(PANEL).inner_margin(egui::Margin::symmetric(10, 4))).show_inside(&mut root, |ui| bottom(ed, ui));
    egui::Panel::left("left").exact_size(236.0).resizable(false).frame(panel(PANEL)).show_inside(&mut root, |ui| {
        egui::ScrollArea::vertical().id_salt("left").auto_shrink(false).show(ui, |ui| left(ed, ui, icons))
    });
    egui::Panel::right("right").exact_size(340.0).resizable(false).frame(panel(PANEL)).show_inside(&mut root, |ui| {
        egui::ScrollArea::vertical().id_salt("right").auto_shrink(false).show(ui, |ui| right(ed, ui))
    });
    egui::CentralPanel::default().frame(egui::Frame::new().fill(BG)).show_inside(&mut root, |ui| canvas(ed, ui, icons));
}

/// Shortcuts: ctrl+S save, ctrl+Z / ctrl+shift+Z (ctrl+Y) undo/redo, F5 play (shift: from
/// the inspected cell), F7 validate, Tab cycles tools; over the canvas a tile's char picks it.
fn keys(ed: &mut Editor, ctx: &egui::Context) {
    if ctx.egui_wants_keyboard_input() {
        return;
    }
    let (save, undo, redo, play, play_here, validate, copy, cut, paste, del, tab) = ctx.input(|i| {
        let c = i.modifiers.command;
        (
            c && i.key_pressed(Key::S),
            c && !i.modifiers.shift && i.key_pressed(Key::Z),
            c && (i.key_pressed(Key::Y) || (i.modifiers.shift && i.key_pressed(Key::Z))),
            i.key_pressed(Key::F5) && !i.modifiers.shift,
            i.key_pressed(Key::F5) && i.modifiers.shift,
            i.key_pressed(Key::F7),
            i.events.iter().any(|e| matches!(e, egui::Event::Copy)),
            i.events.iter().any(|e| matches!(e, egui::Event::Cut)),
            i.events.iter().any(|e| matches!(e, egui::Event::Paste(_))) || (c && i.key_pressed(Key::V)),
            i.key_pressed(Key::Delete),
            i.key_pressed(Key::Tab),
        )
    });
    if save {
        ed.save();
    }
    if undo {
        ed.undo();
    }
    if redo {
        ed.redo();
    }
    if play {
        ed.play(None);
    }
    if play_here {
        ed.play(ed.selected);
    }
    if validate && !ed.validating() {
        ed.validate();
    }
    if tab {
        ed.tool = match ed.tool {
            Tool::Tiles => Tool::Tweaks,
            Tool::Tweaks => Tool::Gates,
            Tool::Gates => Tool::Spots,
            Tool::Spots => Tool::Select,
            Tool::Select => Tool::Tiles,
        };
    }
    if (copy || cut) && let Some((a, b)) = ed.selection {
        let rows = (a.1..=b.1).map(|r| (a.0..=b.0).map(|c| ed.doc.get(c, r)).collect()).collect();
        let layer = (a.1..=b.1).map(|r| (a.0..=b.0).map(|c| ed.doc.tweak(c, r)).collect()).collect();
        ed.clipboard = Some((rows, layer));
        ed.status = format!("copied {}x{}", b.0 - a.0 + 1, b.1 - a.1 + 1);
        if cut {
            clear(ed, a, b);
        }
    }
    if del && let Some((a, b)) = ed.selection {
        clear(ed, a, b);
    }
    let _ = paste;
}

fn clear(ed: &mut Editor, a: (usize, usize), b: (usize, usize)) {
    ed.edit(|d| {
        for r in a.1..=b.1 {
            for c in a.0..=b.0 {
                d.set(c, r, '.');
                d.set_tweak(c, r, '.');
            }
        }
    });
}

fn top(ed: &mut Editor, ui: &mut egui::Ui) {
    ui.horizontal_centered(|ui| {
        ui.label(egui::RichText::new("LEVEL EDITOR").font(pixel(12.0)).color(GOLD));
        ui.add_space(12.0);
        let label = ed.path.as_ref().map_or("(unsaved)".to_string(), |p| p.display().to_string());
        egui::ComboBox::from_id_salt("open").selected_text(format!("{label}{}", if ed.dirty() { " •" } else { "" })).width(260.0).show_ui(ui, |ui| {
            for e in ed.files.clone() {
                if ui.selectable_label(ed.path.as_ref() == Some(&e.path), &e.label).clicked() {
                    ed.open(&e.path);
                }
            }
        });
        if ui.button("New A").on_hover_text("a new Side A level (Nat and Han)").clicked() {
            ed.new_level(false);
        }
        if ui.button("New B").on_hover_text("a new Side B level (Han alone, a set list)").clicked() {
            ed.new_level(true);
        }
        if ui.add_enabled(ed.parsed.level.is_ok(), egui::Button::new("Save")).on_hover_text("ctrl+S").clicked() {
            ed.save();
        }
        ui.add(egui::TextEdit::singleline(&mut ed.save_as).hint_text("save as name").desired_width(120.0));
        if ui.add_enabled(!ed.save_as.trim().is_empty() && ed.parsed.level.is_ok(), egui::Button::new("Save as")).on_hover_text("levels/custom/<name>.txt").clicked() {
            let p = std::path::PathBuf::from(format!("levels/custom/{}.txt", slug(&ed.save_as)));
            ed.save_as.clear();
            ed.save_to(p);
        }
        ui.separator();
        if ui.add_enabled(ed.can_undo(), egui::Button::new("Undo")).clicked() {
            ed.undo();
        }
        if ui.add_enabled(ed.can_redo(), egui::Button::new("Redo")).clicked() {
            ed.redo();
        }
        ui.separator();
        let vlabel = if ed.validating() { "Validating…" } else { "Validate (F7)" };
        if ui.add_enabled(!ed.validating() && ed.parsed.level.is_ok(), egui::Button::new(vlabel)).clicked() {
            ed.validate();
        }
        let play = if ed.playing() { "Restart in game (F5)" } else { "Play (F5)" };
        if ui.add_enabled(ed.parsed.level.is_ok(), egui::Button::new(egui::RichText::new(play).color(GOLD))).clicked() {
            ed.play(None);
        }
        let here = ed.selected.filter(|_| !ed.parsed.side_b);
        if ui.add_enabled(here.is_some() && ed.parsed.level.is_ok(), egui::Button::new("Play from cell")).on_hover_text("shift+F5: Nat starts at the inspected cell").clicked() {
            ed.play(here);
        }
        ui.separator();
        let blocker = ed.share_blocker();
        let share = ui.add_enabled(blocker.is_none(), egui::Button::new("Share"));
        let share = match blocker {
            Some(why) => share.on_disabled_hover_text(format!("can't share yet: {why}")),
            None => share.on_hover_text("copy a link that plays this level in the web game"),
        };
        if share.clicked() {
            let link = ed.share_link();
            ed.status = format!("share link copied ({} characters)", link.len());
            ui.ctx().copy_text(link);
        }
        let paste = ui.add(egui::TextEdit::singleline(&mut ed.pasted).hint_text("paste a share link").desired_width(110.0));
        let enter = paste.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if ui.add_enabled(!ed.pasted.trim().is_empty(), egui::Button::new("Open link")).clicked() || (enter && !ed.pasted.trim().is_empty()) {
            let pasted = std::mem::take(&mut ed.pasted);
            ed.open_shared(&pasted);
        }
        if ed.playing() {
            ui.checkbox(&mut ed.live, "live").on_hover_text("send every edit to the game as you make it");
            if ui.button("Stop").clicked() {
                ed.stop();
            }
        }
    });
}

fn bottom(ed: &mut Editor, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        match &ed.parsed.level {
            Ok(l) => ui.label(egui::RichText::new(format!("{}  {}x{}  {}", if ed.parsed.side_b { "SIDE B" } else { "SIDE A" }, l.width, l.height, l.name)).color(TEXT_DIM)),
            Err(e) => ui.label(egui::RichText::new(format!("doesn't parse: {e}")).color(ERR_TEXT)),
        };
        ui.separator();
        ui.label(egui::RichText::new(&ed.status).color(TEXT_FAINT));
    });
}

// ─── Left: tools, palette, view, grid ────────────────────────────────────────

fn left(ed: &mut Editor, ui: &mut egui::Ui, icons: &Icons) {
    caption(ui, "TOOL (Tab)");
    ui.horizontal_wrapped(|ui| {
        for (t, name, tip) in [
            (Tool::Tiles, "tiles", "paint the brush; shift-drag a rectangle; right-drag erases; alt-click picks"),
            (Tool::Tweaks, "tweaks", "paint the tweak layer (per-tile physics)"),
            (Tool::Gates, "gates", "drag out a gate mark"),
            (Tool::Spots, "lines", "click: a hint spot (a checkpoint's say@ line on a C)"),
            (Tool::Select, "select", "click to inspect, drag to select; ctrl+C/X/V, Delete"),
        ] {
            if chip(ui, ed.tool == t, true, name).on_hover_text(tip).clicked() {
                ed.tool = t;
            }
        }
    });
    ui.add_space(8.0);
    match ed.tool {
        Tool::Tweaks => tweak_palette(ed, ui),
        Tool::Gates => {
            caption(ui, "GATE KIND");
            ui.horizontal_wrapped(|ui| {
                for t in Topic::ALL.into_iter().filter(|t| t.is_gate()) {
                    if chip(ui, ed.gate_brush == t, true, t.word()).clicked() {
                        ed.gate_brush = t;
                    }
                }
            });
        }
        _ => tile_palette(ed, ui, icons),
    }
    ui.add_space(10.0);
    caption(ui, "VIEW");
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(&mut ed.show_grid, "grid");
        ui.checkbox(&mut ed.show_tweaks, "tweaks");
        ui.checkbox(&mut ed.show_marks, "marks");
        ui.checkbox(&mut ed.show_reach, "reach");
    });
    ui.horizontal(|ui| {
        ui.label("zoom");
        ui.add(egui::Slider::new(&mut ed.zoom, 0.5..=4.0).step_by(0.25));
    });
    ui.add_space(10.0);
    caption(ui, "GRID (at the inspected cell)");
    let at = ed.selected;
    ui.horizontal_wrapped(|ui| {
        let en = at.is_some();
        if ui.add_enabled(en, egui::Button::new("+col")).on_hover_text("insert a column left of it").clicked() {
            ed.edit(|d| d.insert_col(at.unwrap().0));
        }
        if ui.add_enabled(en, egui::Button::new("−col")).clicked() {
            ed.edit(|d| d.delete_col(at.unwrap().0));
        }
        if ui.add_enabled(en, egui::Button::new("+row")).on_hover_text("insert a row above it").clicked() {
            ed.edit(|d| d.insert_row(at.unwrap().1));
        }
        if ui.add_enabled(en, egui::Button::new("−row")).clicked() {
            ed.edit(|d| d.delete_row(at.unwrap().1));
        }
    });
    let (mut w, mut h) = (ed.doc.width(), ed.doc.height());
    ui.horizontal(|ui| {
        ui.label("size");
        ui.add(egui::DragValue::new(&mut w).range(8..=2000));
        ui.label("x");
        ui.add(egui::DragValue::new(&mut h).range(4..=200));
    });
    if (w, h) != (ed.doc.width(), ed.doc.height()) {
        ed.field_edit("size", |d| d.resize(w, h));
    }
    ui.add_space(10.0);
    caption(ui, "KEYS");
    ui.label(egui::RichText::new("wheel pans, ctrl+wheel zooms, middle-drag pans\ntype a tile's char to pick it\nF5 play · shift+F5 from cell · F7 validate\nctrl+Z / ctrl+shift+Z").color(TEXT_FAINT).size(10.5));
}

fn tile_palette(ed: &mut Editor, ui: &mut egui::Ui, icons: &Icons) {
    let world = ed.parsed.level.as_ref().map_or(1, |l| l.world);
    for (group, name) in [(GlyphGroup::Terrain, "TERRAIN"), (GlyphGroup::Hazard, "HAZARDS"), (GlyphGroup::Thing, "THINGS"), (GlyphGroup::Platform, "PLATFORMS")] {
        caption(ui, name);
        ui.horizontal_wrapped(|ui| {
            for g in GLYPHS.iter().filter(|g| g.group == group) {
                let (rect, resp) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::click());
                let on = ed.brush == g.ch;
                ui.painter().rect_filled(rect, 4.0, if on { HILITE } else { RAISED });
                ui.painter().rect_stroke(rect, 4.0, Stroke::new(1.0, if on { GOLD } else { LINE }), egui::StrokeKind::Inside);
                draw_glyph(ui.painter(), icons, g.ch, rect.shrink(5.0), world, None, true);
                if resp.on_hover_text(format!("{}  `{}`\n{}", g.name, g.ch, g.about)).clicked() {
                    ed.brush = g.ch;
                    ed.tool = Tool::Tiles;
                }
            }
            if group == GlyphGroup::Terrain {
                let (rect, resp) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::click());
                let on = ed.brush == '.';
                ui.painter().rect_filled(rect, 4.0, if on { HILITE } else { RAISED });
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "·", mono(14.0), TEXT_DIM);
                if resp.on_hover_text("empty `.`").clicked() {
                    ed.brush = '.';
                }
            }
        });
    }
    if let Some(g) = glyph_info(ed.brush) {
        ui.label(egui::RichText::new(format!("{} — {}", g.name, g.about)).color(TEXT_FAINT).size(10.5));
    }
}

fn tweak_palette(ed: &mut Editor, ui: &mut egui::Ui) {
    caption(ui, "TWEAK BRUSH");
    ui.horizontal_wrapped(|ui| {
        for (ch, t, _) in ed.tweaks() {
            let (rect, resp) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::click());
            let on = ed.tweak_brush == ch;
            ui.painter().rect_filled(rect, 4.0, tweak_color(ch).gamma_multiply(0.7));
            ui.painter().rect_stroke(rect, 4.0, Stroke::new(if on { 2.0 } else { 1.0 }, if on { GOLD } else { LINE }), egui::StrokeKind::Inside);
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, ch, mono(14.0), TEXT);
            if resp.on_hover_text(t.to_value()).clicked() {
                ed.tweak_brush = ch;
            }
        }
        let (rect, resp) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::click());
        ui.painter().rect_filled(rect, 4.0, if ed.tweak_brush == '.' { HILITE } else { RAISED });
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "·", mono(14.0), TEXT_DIM);
        if resp.on_hover_text("erase tweaks").clicked() {
            ed.tweak_brush = '.';
        }
    });
    ui.label(egui::RichText::new("surface keys work on the tile you stand on (paint ground); volume keys and Han's zones on the air cells (paint where he'd stand)").color(TEXT_FAINT).size(10.5));
    ui.add_space(4.0);
    egui::ComboBox::from_id_salt("preset").selected_text("add a tweak…").width(200.0).show_ui(ui, |ui| {
        for (name, value) in PRESETS {
            if ui.selectable_label(false, format!("{name}  ({value})")).clicked() {
                ed.add_tweak(Tweak::parse(value).unwrap());
            }
        }
        if ui.selectable_label(false, "blank").clicked() {
            ed.add_tweak(Tweak::default());
        }
    });
}

// ─── Right: level, inspector, tweaks, validator, playtest ────────────────────

fn field(ed: &mut Editor, ui: &mut egui::Ui, key: &'static str, label: &str, wide: bool) {
    let mut v = ed.doc.get_field(key).unwrap_or("").to_string();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).color(TEXT_DIM));
        let edit = if wide { egui::TextEdit::multiline(&mut v).desired_rows(2) } else { egui::TextEdit::singleline(&mut v) };
        if ui.add(edit.desired_width(f32::INFINITY)).changed() {
            let v = v.replace('\n', " ");
            ed.field_edit(key, |d| d.set_field(key, if v.is_empty() && key == "deaths" { None } else { Some(&v) }));
        }
    });
}

fn right(ed: &mut Editor, ui: &mut egui::Ui) {
    caption(ui, "LEVEL");
    ui.horizontal(|ui| {
        let mut b = ed.parsed.side_b;
        if chip(ui, !b, true, "SIDE A").on_hover_text("Nat and Han; the story's rules").clicked() {
            b = false;
        }
        if chip(ui, b, true, "SIDE B").on_hover_text("Han alone; the player calls the band from a set list").clicked() {
            b = true;
        }
        ed.set_side_b(b);
    });
    field(ed, ui, "name", "name ", false);
    let mut world: u8 = ed.doc.get_field("world").and_then(|w| w.parse().ok()).unwrap_or(1);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("world").color(TEXT_DIM));
        if ui.add(egui::Slider::new(&mut world, 1..=5)).changed() {
            ed.field_edit("world", |d| d.set_field("world", Some(&world.to_string())));
        }
    });
    field(ed, ui, "intro", "intro", true);
    if ed.parsed.side_b {
        setlist(ed, ui);
    } else {
        field(ed, ui, "deaths", "deaths", false);
    }
    ui.add_space(10.0);
    inspector(ed, ui);
    ui.add_space(10.0);
    tweaks(ed, ui);
    ui.add_space(10.0);
    gates(ed, ui);
    ui.add_space(10.0);
    validation(ed, ui);
    if !ed.play_log.is_empty() {
        ui.add_space(10.0);
        caption(ui, "PLAYTEST");
        for l in ed.play_log.iter().rev().take(8) {
            ui.label(egui::RichText::new(l).color(GREEN).size(11.0));
        }
    }
}

fn setlist(ed: &mut Editor, ui: &mut egui::Ui) {
    caption(ui, "SET LIST (Left/Right picks, Down calls; in order)");
    let mut list = ed.setlist();
    for word in ["giant", "gap", "melt"] {
        if !list.iter().any(|(w, _)| w == word) {
            list.push((word.to_string(), 0));
        }
    }
    let before = list.clone();
    ui.horizontal(|ui| {
        for (w, n) in &mut list {
            ui.label(w.as_str());
            ui.add(egui::DragValue::new(n).range(0..=20));
        }
    });
    if list != before {
        let keep: Vec<(String, u32)> = list.into_iter().filter(|(_, n)| *n > 0).collect();
        ed.set_setlist(&keep);
    }
}

/// The inspected cell: what's in it, its lines, its platform, the marks over it.
fn inspector(ed: &mut Editor, ui: &mut egui::Ui) {
    let Some((c, r)) = ed.selected else {
        caption(ui, "CELL (click one with the select or lines tool)");
        return;
    };
    let ch = ed.doc.get(c, r);
    caption(ui, &format!("CELL {c},{r}"));
    let what = glyph_info(ch).map_or("empty".to_string(), |g| format!("{} `{}`", g.name, g.ch));
    let tw = ed.doc.tweak(c, r);
    ui.label(format!("{what}{}", if tw != '.' { format!(", tweak {tw}") } else { String::new() }));
    // Lines at this cell.
    let at = format!("{c},{r}");
    for i in ed.doc.lines_where(|k| k.starts_with(&format!("hint@{at}")) || k == format!("say@{at}")) {
        let line = ed.doc.header[i].clone();
        let key = key_of(&line).unwrap_or("").to_string();
        let mut text = value_of(&line).to_string();
        let is_hint = key.starts_with("hint@");
        let mut topics: Vec<String> = key.split_whitespace().skip(1).map(str::to_string).collect();
        ui.group(|ui| {
            ui.label(egui::RichText::new(if is_hint { "hint (Han barks ≤3 words; cards teach the topics)" } else { "checkpoint line" }).color(TEXT_FAINT).size(10.5));
            let mut changed = ui.add(egui::TextEdit::singleline(&mut text).desired_width(f32::INFINITY)).changed();
            if is_hint {
                ui.horizontal_wrapped(|ui| {
                    for t in Topic::ALL {
                        let on = topics.iter().any(|w| w == t.word());
                        if chip(ui, on, true, t.word()).clicked() {
                            if on {
                                topics.retain(|w| w != t.word());
                            } else {
                                topics.push(t.word().into());
                            }
                            changed = true;
                        }
                    }
                });
            }
            if changed {
                let head = std::iter::once(key.split_whitespace().next().unwrap_or("").to_string()).chain(topics).collect::<Vec<_>>().join(" ");
                ed.field_edit("line", |d| d.header[i] = format!("{head}: {text}"));
            }
            if ui.small_button("remove").clicked() {
                ed.edit(|d| {
                    d.header.remove(i);
                });
            }
        });
    }
    // A platform's line.
    if ch.is_ascii_digit() && ch != '0' {
        platform(ed, ui, ch);
    }
}

fn platform(ed: &mut Editor, ui: &mut egui::Ui, digit: char) {
    let key = digit.to_string();
    let Some(i) = ed.doc.line_of(&key) else {
        if ui.button(format!("add the `{digit}:` line")).clicked() {
            ed.edit(|d| d.push_line(format!("{digit}: dx=4 dy=0 period=4")));
        }
        return;
    };
    let v = value_of(&ed.doc.header[i]).to_string();
    let mut kv: Vec<(String, String)> = v.split_whitespace().filter_map(|p| p.split_once('=')).map(|(a, b)| (a.into(), b.into())).collect();
    let get = |kv: &Vec<(String, String)>, k: &str, d: f32| kv.iter().find(|(a, _)| a == k).and_then(|(_, b)| b.parse().ok()).unwrap_or(d);
    let (mut dx, mut dy, mut period, mut phase) = (get(&kv, "dx", 0.0), get(&kv, "dy", 0.0), get(&kv, "period", 4.0), get(&kv, "phase", 0.0));
    let mut kind = kv.iter().find(|(a, _)| a == "kind").map_or("tp".to_string(), |(_, b)| b.clone());
    let before = (dx, dy, period, phase, kind.clone());
    ui.group(|ui| {
        caption(ui, &format!("PLATFORM {digit} (every run of `{digit}` shares it)"));
        ui.horizontal(|ui| {
            ui.label("dx");
            ui.add(egui::DragValue::new(&mut dx).speed(0.1).range(-40.0..=40.0));
            ui.label("dy↑");
            ui.add(egui::DragValue::new(&mut dy).speed(0.1).range(-40.0..=40.0));
        });
        ui.horizontal(|ui| {
            ui.label("period s");
            ui.add(egui::DragValue::new(&mut period).speed(0.05).range(0.2..=30.0));
            ui.label("phase");
            ui.add(egui::DragValue::new(&mut phase).speed(0.01).range(0.0..=1.0));
        });
        ui.horizontal(|ui| {
            for k in ["tp", "duck", "plunger"] {
                if chip(ui, kind == k, true, k).clicked() {
                    kind = k.into();
                }
            }
        });
    });
    if (dx, dy, period, phase, kind.clone()) != before {
        kv.retain(|(a, _)| !["dx", "dy", "period", "phase", "kind"].contains(&a.as_str()));
        let mut parts = vec![format!("dx={dx}"), format!("dy={dy}"), format!("period={period}")];
        if phase != 0.0 {
            parts.push(format!("phase={phase}"));
        }
        if kind != "tp" {
            parts.push(format!("kind={kind}"));
        }
        let line = format!("{digit}: {}", parts.join(" "));
        ed.field_edit("platform", |d| d.header[i] = line);
    }
}

fn tweaks(ed: &mut Editor, ui: &mut egui::Ui) {
    caption(ui, "TWEAKS (per-tile physics)");
    let list = ed.tweaks();
    if list.is_empty() {
        ui.label(egui::RichText::new("none: add one with the tweaks tool").color(TEXT_FAINT).size(11.0));
    }
    for (ch, mut t, i) in list {
        let before = t;
        let mut remove = false;
        egui::CollapsingHeader::new(egui::RichText::new(format!("{ch}  {}", t.to_value())).color(tweak_color(ch))).id_salt(("tweak", ch)).show(ui, |ui| {
            for k in KEYS.iter() {
                let slot = t.slot(k.word).unwrap();
                let mut on = slot.is_some();
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut on, k.word).on_hover_text(format!("{} ({})", k.about, if k.surface { "surface" } else { "volume" })).changed() {
                        *slot = on.then_some(1.0);
                    }
                    if let Some(v) = slot {
                        ui.add(egui::DragValue::new(v).speed(0.02).range(k.range.clone()).max_decimals(2));
                    }
                });
            }
            ui.horizontal_wrapped(|ui| {
                ui.label("Han");
                if chip(ui, t.han.is_none(), true, "—").clicked() {
                    t.han = None;
                }
                for z in HanZone::ALL {
                    if chip(ui, t.han == Some(z), true, z.word()).on_hover_text(z.about()).clicked() {
                        t.han = Some(z);
                    }
                }
            });
            if ui.small_button("remove (and its cells)").clicked() {
                remove = true;
            }
        });
        if remove {
            ed.edit(|d| {
                d.header.remove(i);
                for r in 0..d.height() {
                    for c in 0..d.width() {
                        if d.tweak(c, r) == ch {
                            d.set_tweak(c, r, '.');
                        }
                    }
                }
            });
        } else if t != before {
            let line = format!("tweak {ch}: {}", t.to_value());
            ed.field_edit("tweak", |d| d.header[i] = line);
        }
    }
}

fn gates(ed: &mut Editor, ui: &mut egui::Ui) {
    let idx = ed.doc.lines_where(|k| k == "gate");
    caption(ui, "GATE MARKS");
    if idx.is_empty() {
        ui.label(egui::RichText::new("none: drag one out with the gates tool").color(TEXT_FAINT).size(11.0));
    }
    for i in idx {
        let v = value_of(&ed.doc.header[i]).to_string();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&v).size(11.0));
            if ui.small_button("×").clicked() {
                ed.edit(|d| {
                    d.header.remove(i);
                });
            }
        });
    }
}

fn validation(ed: &mut Editor, ui: &mut egui::Ui) {
    caption(ui, "VALIDATOR (F7)");
    let fresh = ed.validation_fresh();
    let mut goto = None;
    let proving = ed.proving();
    let mut han_act = None;
    match &ed.validation {
        None => {
            ui.label(egui::RichText::new("not run yet").color(TEXT_FAINT).size(11.0));
        }
        Some(v) => {
            if !fresh {
                ui.label(egui::RichText::new("(stale: the level changed since)").color(TEXT_FAINT).size(11.0));
            }
            if v.errs.is_empty() && !ed.parsed.side_b {
                ui.label(egui::RichText::new("OK: beatable, gates hold").color(GREEN));
            }
            if let Some(h) = &v.han {
                han_act = han_proof(ui, h, proving);
            }
            let side_b = ed.parsed.side_b;
            let mut problems = |ui: &mut egui::Ui| {
                if !v.rules.is_empty() {
                    ui.label(egui::RichText::new("story house rules:").color(TEXT_FAINT).size(11.0));
                }
                for e in v.rules.iter().chain(&v.errs) {
                    let cell = find_cell(e);
                    let r = ui.add(egui::Label::new(egui::RichText::new(format!("• {e}")).color(ERR_TEXT).size(11.0)).sense(if cell.is_some() { Sense::click() } else { Sense::hover() }));
                    if let Some(c) = cell
                        && r.on_hover_text("click: go there").clicked()
                    {
                        goto = Some(c);
                    }
                }
            };
            if side_b && !v.errs.is_empty() {
                // Han's proof is Side B's test; Nat's rules are only a hint there.
                egui::CollapsingHeader::new(format!("Nat's rules ({}), for reference", v.errs.len())).id_salt("natrules").show(ui, problems);
            } else {
                problems(ui);
            }
            egui::CollapsingHeader::new("details").id_salt("vdetails").show(ui, |ui| {
                for s in &v.summary {
                    ui.label(egui::RichText::new(s).color(TEXT_DIM).size(11.0));
                }
                ui.label(egui::RichText::new("reach map: green dots reached, faint passed through, red unreachable, letters a gate's take-off").color(TEXT_FAINT).size(10.5));
            });
        }
    }
    if let Some(c) = goto {
        ed.selected = Some(c);
        ed.center_on = Some(c);
    }
    match han_act {
        Some(HanAct::Keep(plan)) => ed.keep_solution(&plan),
        Some(HanAct::Stop) => ed.stop_han(),
        None => {}
    }
}

/// A cell named in a validator message (`col 12 row 3`, `at 12,3`, `hint@12,3`).
fn find_cell(msg: &str) -> Option<(usize, usize)> {
    let words: Vec<&str> = msg.split(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == ':').collect();
    for w in words.windows(4) {
        if w[0] == "col" && w[2] == "row" {
            return Some((w[1].trim_matches(',').parse().ok()?, w[3].trim_matches(',').parse().ok()?));
        }
    }
    for w in &words {
        let w = w.rsplit('@').next().unwrap_or(w);
        if let Some((a, b)) = w.split_once(',')
            && let (Ok(a), Ok(b)) = (a.parse(), b.trim_matches(',').parse())
        {
            return Some((a, b));
        }
    }
    None
}

// ─── The canvas ──────────────────────────────────────────────────────────────

pub fn tweak_color(ch: char) -> Color32 {
    const C: [Color32; 8] = [
        Color32::from_rgb(0x5b, 0xc8, 0xff),
        Color32::from_rgb(0xff, 0x7a, 0xd9),
        Color32::from_rgb(0x8c, 0xf0, 0x6a),
        Color32::from_rgb(0xff, 0xa8, 0x3d),
        Color32::from_rgb(0xb6, 0x8c, 0xff),
        Color32::from_rgb(0x4f, 0xe0, 0xc4),
        Color32::from_rgb(0xff, 0x5c, 0x5c),
        Color32::from_rgb(0xe6, 0xe6, 0x5a),
    ];
    C[(ch as usize) % C.len()]
}

fn gate_color(t: Topic) -> Color32 {
    match t {
        Topic::Giant | Topic::Mega => Color32::from_rgb(0xf0, 0xb4, 0x29),
        Topic::Gap | Topic::Chain => Color32::from_rgb(0xff, 0x6b, 0x3d),
        Topic::Waltz | Topic::Trapeze => Color32::from_rgb(0xc9, 0xa2, 0xff),
        Topic::Grip | Topic::Shaft => Color32::from_rgb(0x7a, 0xb8, 0xff),
        _ => Color32::from_rgb(0x9f, 0xd3, 0x9a),
    }
}

/// The sprite for grid char `ch` (`ground_top`: a `#` with no `#` above it).
fn sprite_of(ch: char, world: u8, ground_top: bool, platform_kind: Option<&str>) -> Option<SpriteId> {
    use SpriteId::*;
    Some(match ch {
        '#' if ground_top => GroundTop(world),
        '#' => GroundFill(world),
        '=' => OneWay(world),
        '_' => Grease(world),
        '^' => SpikesUp,
        'v' => SpikesDown,
        '~' if ground_top => LiquidTop(world),
        '~' => LiquidFill(world),
        'F' => Fly,
        'S' => SprayCan,
        'o' => Nugget,
        'C' => CheckpointOff,
        'G' => GoalFlag,
        'P' => PooIdle,
        '1'..='9' => match platform_kind {
            Some("duck") => PlatformDuck,
            Some("plunger") => PlatformPlunger,
            _ => PlatformTp,
        },
        _ => return None,
    })
}

/// Draw grid char `ch` in `rect` (one tile): its sprite, or a labelled swatch for a char with no
/// sprite here yet (a new tile).
fn draw_glyph(p: &egui::Painter, icons: &Icons, ch: char, rect: Rect, world: u8, kind: Option<&str>, top: bool) {
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    match sprite_of(ch, world, top, kind).and_then(|s| icons.get(&s).map(|t| (s, *t))) {
        Some((SpriteId::GoalFlag, tex)) => {
            // Two tiles tall, standing on the cell's floor.
            let r = Rect::from_min_max(pos2(rect.min.x, rect.min.y - rect.height()), rect.max);
            p.image(tex, r, uv, Color32::WHITE);
        }
        Some((SpriteId::Fly, tex)) => {
            p.image(tex, Rect::from_center_size(rect.center(), rect.size() * 0.5), uv, Color32::WHITE);
        }
        Some((_, tex)) => {
            p.image(tex, rect, uv, Color32::WHITE);
        }
        None => {
            p.rect_filled(rect.shrink(1.0), 2.0, Color32::from_rgb(0x55, 0x40, 0x60));
            p.text(rect.center(), egui::Align2::CENTER_CENTER, ch, mono(rect.height() * 0.7), TEXT);
        }
    }
}


fn canvas(ed: &mut Editor, ui: &mut egui::Ui, icons: &Icons) {
    let rect = ui.available_rect_before_wrap();
    let resp = ui.allocate_rect(rect, Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    let s = 16.0 * ed.zoom;
    if let Some(c) = ed.center_on.take() {
        ed.pan = vec2(rect.width() / 2.0 - (c.0 as f32 + 0.5) * s, rect.height() / 2.0 - (c.1 as f32 + 0.5) * s);
    }
    let origin = rect.min + ed.pan + vec2(16.0, 16.0);
    let (w, h) = (ed.doc.width(), ed.doc.height());
    let cell_rect = |c: usize, r: usize| Rect::from_min_size(origin + vec2(c as f32 * s, r as f32 * s), vec2(s, s));
    let cell_at = |p: Pos2| -> Option<(usize, usize)> {
        let v = (p - origin) / s;
        (v.x >= 0.0 && v.y >= 0.0).then(|| (v.x as usize, v.y as usize))
    };

    // --- Input: pan and zoom.
    if resp.hovered() {
        let (scroll, zoom, ctrl) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.modifiers.command));
        if zoom != 1.0 || (ctrl && scroll.y != 0.0) {
            let f = if zoom != 1.0 { zoom } else { (scroll.y / 200.0).exp() };
            let new = (ed.zoom * f).clamp(0.5, 4.0);
            if let Some(m) = resp.hover_pos() {
                let k = new / ed.zoom;
                ed.pan = (m - rect.min - vec2(16.0, 16.0)) * (1.0 - k) + ed.pan * k;
            }
            ed.zoom = new;
        } else {
            ed.pan += scroll;
        }
    }
    if resp.dragged_by(egui::PointerButton::Middle) {
        ed.pan += resp.drag_delta();
    }
    let hover = resp.hover_pos().and_then(cell_at);

    // --- Pick a brush by typing its char.
    if resp.hovered() && !ui.ctx().egui_wants_keyboard_input() {
        let typed: Vec<String> = ui.input(|i| i.events.iter().filter_map(|e| if let egui::Event::Text(t) = e { Some(t.clone()) } else { None }).collect());
        for t in typed {
            for ch in t.chars() {
                if ed.tool == Tool::Tweaks && (ch.is_ascii_lowercase() || ch == '.') {
                    ed.tweak_brush = ch;
                } else if glyph_info(ch).is_some() || ch == '.' {
                    ed.brush = ch;
                    ed.tool = Tool::Tiles;
                }
            }
        }
    }

    // --- Input: the tools.
    let mut drag = std::mem::take(&mut ed.drag);
    let (pressed, down, released, sec_down, shift, alt) = ui.input(|i| {
        (
            i.pointer.primary_pressed() || i.pointer.secondary_pressed(),
            i.pointer.primary_down(),
            i.pointer.primary_released() || i.pointer.secondary_released(),
            i.pointer.secondary_down(),
            i.modifiers.shift,
            i.modifiers.alt,
        )
    });
    let mut paste = ui.input(|i| i.modifiers.command && i.key_pressed(Key::V));
    if ui.ctx().egui_wants_keyboard_input() {
        paste = false;
    }
    if paste && let (Some(at), Some((rows, layer))) = (hover, ed.clipboard.clone()) {
        ed.edit(|d| {
            for (dr, row) in rows.iter().enumerate() {
                for (dc, &ch) in row.iter().enumerate() {
                    d.set(at.0 + dc, at.1 + dr, ch);
                    d.set_tweak(at.0 + dc, at.1 + dr, layer[dr][dc]);
                }
            }
        });
        ed.status = "pasted".into();
    }
    if pressed && resp.hovered() {
        ed.stroke += 1;
        drag = crate::app::Drag { from: hover, last: None, erase: sec_down || ui.input(|i| i.pointer.secondary_pressed()) };
        if alt && let Some((c, r)) = hover {
            if ed.tool == Tool::Tweaks {
                ed.tweak_brush = ed.doc.tweak(c, r);
            } else {
                ed.brush = ed.doc.get(c, r);
                ed.tool = Tool::Tiles;
            }
            drag.from = None;
        }
    }
    let active = drag.from.is_some() && (down || sec_down);
    let rect_tool = matches!(ed.tool, Tool::Gates | Tool::Select) || (shift && matches!(ed.tool, Tool::Tiles | Tool::Tweaks));
    if active && !rect_tool && let Some(cur) = hover {
        // Freehand: paint every cell from the last one to this.
        let from = drag.last.unwrap_or(cur);
        let cells = line(from, cur);
        let erase = drag.erase;
        let (tool, brush, tb) = (ed.tool, ed.brush, ed.tweak_brush);
        match tool {
            Tool::Tiles => ed.stroke_edit(|d| {
                for (c, r) in cells {
                    paint(d, c, r, if erase { '.' } else { brush });
                }
            }),
            Tool::Tweaks => ed.stroke_edit(|d| {
                for (c, r) in cells {
                    d.set_tweak(c, r, if erase { '.' } else { tb });
                }
            }),
            _ => {}
        }
        drag.last = Some(cur);
    }
    if released && let Some(from) = drag.from.take() {
        let to = hover.unwrap_or(from);
        let (a, b) = ((from.0.min(to.0), from.1.min(to.1)), (from.0.max(to.0), from.1.max(to.1)));
        let erase = drag.erase;
        match ed.tool {
            Tool::Tiles if shift => {
                let brush = if erase { '.' } else { ed.brush };
                ed.stroke_edit(|d| {
                    for r in a.1..=b.1 {
                        for c in a.0..=b.0 {
                            paint(d, c, r, brush);
                        }
                    }
                });
            }
            Tool::Tweaks if shift => {
                let tb = if erase { '.' } else { ed.tweak_brush };
                ed.stroke_edit(|d| {
                    for r in a.1..=b.1 {
                        for c in a.0..=b.0 {
                            d.set_tweak(c, r, tb);
                        }
                    }
                });
            }
            Tool::Gates if !erase => {
                let line = format!("gate: {} {},{} {},{}", ed.gate_brush.word(), a.0, a.1, b.0, b.1);
                ed.edit(|d| d.push_line(line));
                ed.status = format!("marked a {} gate", ed.gate_brush.word());
            }
            Tool::Gates => {
                // Right-click: remove the marks over the cell.
                let idx: Vec<usize> = ed.doc.lines_where(|k| k == "gate");
                let hit: Vec<usize> = idx
                    .into_iter()
                    .filter(|&i| gate_rect(value_of(&ed.doc.header[i])).is_some_and(|(_, c0, r0, c1, r1)| (c0..=c1).contains(&(a.0 as i32)) && (r0..=r1).contains(&(a.1 as i32))))
                    .collect();
                ed.edit(|d| {
                    for i in hit.into_iter().rev() {
                        d.header.remove(i);
                    }
                });
            }
            Tool::Spots => {
                ed.selected = Some(from);
                let at = format!("{},{}", from.0, from.1);
                let exists = !ed.doc.lines_where(|k| k.starts_with(&format!("hint@{at}")) || k == format!("say@{at}")).is_empty();
                if !exists && !erase {
                    let line = if ed.doc.get(from.0, from.1) == 'C' { format!("say@{at}: Two-ply!") } else { format!("hint@{at} toot: Toot!") };
                    ed.edit(|d| d.push_line(line));
                }
            }
            Tool::Select => {
                ed.selected = Some(from);
                ed.selection = (a != b).then_some((a, b));
            }
            _ => {}
        }
    }
    let preview = drag.from.filter(|_| rect_tool).zip(hover);
    ed.drag = drag;

    // --- Draw.
    let world = ed.parsed.level.as_ref().map_or(1, |l| l.world);
    let kinds: HashMap<char, String> = ('1'..='9')
        .filter_map(|d| {
            let v = ed.doc.get_field(&d.to_string())?;
            let k = v.split_whitespace().find_map(|p| p.strip_prefix("kind="))?;
            Some((d, k.to_string()))
        })
        .collect();
    painter.rect_filled(Rect::from_min_size(origin, vec2(w as f32 * s, h as f32 * s)), 0.0, Color32::from_rgb(0x22, 0x1a, 0x13));
    let visible = |c: usize, r: usize| rect.intersects(cell_rect(c, r).expand(s));
    let (c_lo, r_lo) = cell_at(rect.min).unwrap_or((0, 0));
    let (c_hi, r_hi) = cell_at(rect.max).map_or((w, h), |(c, r)| ((c + 2).min(w), (r + 2).min(h)));
    let (c_lo, r_lo) = (c_lo.saturating_sub(1), r_lo.saturating_sub(2));
    for r in r_lo..r_hi {
        for c in c_lo..c_hi {
            let ch = ed.doc.get(c, r);
            if nat_han_adventures::level::doc::is_empty(ch) || !visible(c, r) {
                continue;
            }
            let top = r == 0 || ed.doc.get(c, r - 1) != ch;
            draw_glyph(&painter, icons, ch, cell_rect(c, r), world, kinds.get(&ch).map(String::as_str), top);
        }
    }
    if ed.show_grid && s >= 12.0 {
        let stroke = Stroke::new(1.0, Color32::from_white_alpha(10));
        for c in c_lo..=c_hi.min(w) {
            let x = origin.x + c as f32 * s;
            let strong = c % 10 == 0;
            painter.line_segment([pos2(x, origin.y), pos2(x, origin.y + h as f32 * s)], if strong { Stroke::new(1.0, Color32::from_white_alpha(28)) } else { stroke });
        }
        for r in r_lo..=r_hi.min(h) {
            let y = origin.y + r as f32 * s;
            painter.line_segment([pos2(origin.x, y), pos2(origin.x + w as f32 * s, y)], stroke);
        }
    }
    // Column / row rulers.
    for c in (c_lo..c_hi).filter(|c| c % 10 == 0) {
        painter.text(pos2(origin.x + c as f32 * s + 2.0, rect.min.y + 2.0), egui::Align2::LEFT_TOP, c.to_string(), mono(10.0), TEXT_FAINT);
    }
    for r in (r_lo..r_hi).filter(|r| r % 5 == 0) {
        painter.text(pos2(rect.min.x + 2.0, origin.y + r as f32 * s + 2.0), egui::Align2::LEFT_TOP, r.to_string(), mono(10.0), TEXT_FAINT);
    }
    painter.rect_stroke(Rect::from_min_size(origin, vec2(w as f32 * s, h as f32 * s)), 0.0, Stroke::new(1.0, LINE), egui::StrokeKind::Outside);

    // Moving platforms' paths: a ghost where each run turns around.
    if ed.show_marks
        && let Ok(l) = &ed.parsed.level
    {
        for p in &l.platforms {
            let a = cell_rect(p.col, p.row).min;
            let size = vec2(p.width as f32 * s, s * 0.5);
            let b = a + vec2(p.dx * s, -p.dy * s);
            painter.rect_stroke(Rect::from_min_size(b, size), 2.0, Stroke::new(1.0, Color32::from_white_alpha(90)), egui::StrokeKind::Inside);
            painter.arrow(a + size / 2.0, b - a, Stroke::new(1.0, Color32::from_white_alpha(70)));
        }
    }
    // The tweak layer.
    if ed.show_tweaks || ed.tool == Tool::Tweaks {
        for r in r_lo..r_hi {
            for c in c_lo..c_hi {
                let t = ed.doc.tweak(c, r);
                if t == '.' {
                    continue;
                }
                let cr = cell_rect(c, r);
                painter.rect_filled(cr, 0.0, tweak_color(t).gamma_multiply(0.35));
                if s >= 16.0 {
                    painter.text(cr.right_top() + vec2(-2.0, 1.0), egui::Align2::RIGHT_TOP, t, mono(9.0 * ed.zoom.min(2.0)), tweak_color(t));
                }
            }
        }
    }
    // The validator's reach map.
    if ed.show_reach
        && let Some(v) = &ed.validation
    {
        let alpha = if ed.validation_fresh() { 1.0 } else { 0.4 };
        for (r, row) in v.reach.iter().enumerate() {
            for (c, &ch) in row.iter().enumerate() {
                if r >= h || c >= w || !visible(c, r) {
                    continue;
                }
                let cr = cell_rect(c, r);
                match ch {
                    '+' => {
                        painter.circle_filled(cr.center(), (s * 0.12).max(1.5), GREEN.gamma_multiply(0.8 * alpha));
                    }
                    ',' => {
                        painter.circle_filled(cr.center(), (s * 0.06).max(1.0), GREEN.gamma_multiply(0.35 * alpha));
                    }
                    'X' | '!' => {
                        painter.rect_stroke(cr.shrink(1.0), 2.0, Stroke::new(2.0, RED.gamma_multiply(alpha)), egui::StrokeKind::Inside);
                    }
                    'W' | 'R' | 'Z' | 'Y' | 'K' | 'M' | 'N' | 'U' | 'H' | 'T' => {
                        painter.rect_filled(cr.shrink(s * 0.2), 2.0, GOLD.gamma_multiply(0.85 * alpha));
                        painter.text(cr.center(), egui::Align2::CENTER_CENTER, ch, mono(s * 0.5), BG);
                    }
                    _ => {}
                }
            }
        }
    }
    // Gate marks and Han's lines.
    if ed.show_marks || ed.tool == Tool::Gates {
        for i in ed.doc.lines_where(|k| k == "gate") {
            if let Some((topic, c0, r0, c1, r1)) = gate_rect(value_of(&ed.doc.header[i])) {
                let a = cell_rect(c0.max(0) as usize, r0.max(0) as usize).min;
                let b = cell_rect(c1.max(0) as usize, r1.max(0) as usize).max;
                let col = topic.map_or(TEXT_DIM, gate_color);
                painter.rect_stroke(Rect::from_min_max(a, b), 2.0, Stroke::new(2.0, col), egui::StrokeKind::Inside);
                painter.text(a + vec2(3.0, 2.0), egui::Align2::LEFT_TOP, topic.map_or("?", |t| t.word()), mono(11.0), col);
            }
        }
    }
    if ed.show_marks || ed.tool == Tool::Spots {
        for i in ed.doc.lines_where(|k| k.starts_with("hint@") || k.starts_with("say@")) {
            let line = &ed.doc.header[i];
            let key = key_of(line).unwrap_or("");
            let Some((c, r)) = key.split_once('@').and_then(|(_, rest)| {
                let (c, r) = rest.split_whitespace().next()?.split_once(',')?;
                Some((c.parse::<usize>().ok()?, r.parse::<usize>().ok()?))
            }) else {
                continue;
            };
            let cr = cell_rect(c, r);
            let hint = key.starts_with("hint@");
            let col = if hint { Color32::from_rgb(0x7a, 0xd8, 0xff) } else { Color32::from_rgb(0xff, 0xf0, 0xc0) };
            painter.circle_filled(cr.center(), s * 0.3, col.gamma_multiply(0.85));
            painter.text(cr.center(), egui::Align2::CENTER_CENTER, if hint { "?" } else { "\"" }, mono(s * 0.45), BG);
            if s >= 24.0 || ed.selected == Some((c, r)) {
                painter.text(cr.center_top() - vec2(0.0, 2.0), egui::Align2::CENTER_BOTTOM, value_of(line), mono(10.0), col);
            }
        }
    }
    // Selection, preview, hover.
    if let Some((a, b)) = ed.selection {
        painter.rect_stroke(Rect::from_min_max(cell_rect(a.0, a.1).min, cell_rect(b.0, b.1).max), 0.0, Stroke::new(1.5, GOLD), egui::StrokeKind::Inside);
    }
    if let Some((c, r)) = ed.selected {
        painter.rect_stroke(cell_rect(c, r), 0.0, Stroke::new(2.0, GOLD), egui::StrokeKind::Inside);
    }
    if let Some((from, to)) = preview {
        let (a, b) = ((from.0.min(to.0), from.1.min(to.1)), (from.0.max(to.0), from.1.max(to.1)));
        let col = if ed.tool == Tool::Gates { gate_color(ed.gate_brush) } else { TEXT };
        painter.rect_stroke(Rect::from_min_max(cell_rect(a.0, a.1).min, cell_rect(b.0, b.1).max), 0.0, Stroke::new(1.5, col), egui::StrokeKind::Inside);
        painter.text(cell_rect(b.0, b.1).max + vec2(4.0, 0.0), egui::Align2::LEFT_BOTTOM, format!("{}x{}", b.0 - a.0 + 1, b.1 - a.1 + 1), mono(11.0), col);
    }
    if let Some((c, r)) = hover {
        let cr = cell_rect(c, r);
        painter.rect_stroke(cr, 0.0, Stroke::new(1.0, Color32::from_white_alpha(120)), egui::StrokeKind::Inside);
        if ed.tool == Tool::Tiles && !nat_han_adventures::level::doc::is_empty(ed.brush) && preview.is_none() {
            draw_glyph(&painter.with_clip_rect(cr), icons, ed.brush, cr, world, kinds.get(&ed.brush).map(String::as_str), true);
        }
        let ch = ed.doc.get(c, r);
        let what = glyph_info(ch).map_or("", |g| g.name);
        let tw = ed.doc.tweak(c, r);
        let tws = ed.tweaks().into_iter().find(|t| t.0 == tw).map(|t| format!("  tweak {tw}: {}", t.1.to_value())).unwrap_or_default();
        painter.text(rect.left_bottom() + vec2(6.0, -6.0), egui::Align2::LEFT_BOTTOM, format!("{c},{r}  {what}{tws}"), mono(11.0), TEXT_DIM);
    }
}


/// A `gate:` line's value as (topic, c0, r0, c1, r1).
fn gate_rect(v: &str) -> Option<(Option<Topic>, i32, i32, i32, i32)> {
    let w: Vec<&str> = v.split_whitespace().collect();
    let [t, a, b] = w[..] else { return None };
    let cell = |s: &str| -> Option<(i32, i32)> {
        let (c, r) = s.split_once(',')?;
        Some((c.trim().parse().ok()?, r.trim().parse().ok()?))
    };
    let ((c0, r0), (c1, r1)) = (cell(a)?, cell(b)?);
    Some((Topic::from_word(t), c0.min(c1), r0.min(r1), c0.max(c1), r0.max(r1)))
}

/// Paint one cell: `P` and `G` move (there's one of each), a platform digit gets its header
/// line.
fn paint(d: &mut LevelDoc, c: usize, r: usize, ch: char) {
    if ch == 'P' || ch == 'G' {
        for row in 0..d.height() {
            for col in 0..d.width() {
                if d.get(col, row) == ch {
                    d.set(col, row, '.');
                }
            }
        }
    }
    if let Some(Glyph::Platform(n)) = glyph_info(ch).map(|g| g.glyph)
        && d.line_of(&n.to_string()).is_none()
    {
        d.push_line(format!("{n}: dx=4 dy=0 period=4"));
    }
    d.set(c, r, ch);
}

/// The cells on a line from `a` to `b` (fast strokes leave no gaps).
fn line(a: (usize, usize), b: (usize, usize)) -> Vec<(usize, usize)> {
    let (dx, dy) = (b.0 as i32 - a.0 as i32, b.1 as i32 - a.1 as i32);
    let n = dx.abs().max(dy.abs()).max(1);
    (0..=n)
        .map(|k| {
            let t = k as f32 / n as f32;
            ((a.0 as f32 + dx as f32 * t).round() as usize, (a.1 as f32 + dy as f32 * t).round() as usize)
        })
        .collect()
}


enum HanAct {
    Keep(String),
    Stop,
}

/// Side B: Han's proof of the level (`game::sideb_solve`).
fn han_proof(ui: &mut egui::Ui, h: &HanProof, proving: bool) -> Option<HanAct> {
    let mut act = None;
    let line = |ui: &mut egui::Ui, t: String, c: egui::Color32| {
        ui.add(egui::Label::new(egui::RichText::new(t).color(c).size(11.0)).wrap());
    };
    match h {
        HanProof::Replaying => line(ui, "Han: running the level...".into(), TEXT_DIM),
        HanProof::Searching { plays, best_col } => {
            line(ui, format!("Han: searching for a plan ({plays} plays, furthest col {best_col:.0})"), TEXT_DIM);
        }
        HanProof::Holds => line(ui, "Han: the solution clears it".into(), GREEN),
        HanProof::Fails { fate, col } => line(ui, format!("Han: the solution fails: {fate} at col {col:.0}"), ERR_TEXT),
        HanProof::Found { plan, .. } => {
            line(ui, format!("Han clears it: {plan}"), GREEN);
            if ui.button("keep as the solution").on_hover_text("write it as the level's `solution:` line; Validate replays it").clicked() {
                act = Some(HanAct::Keep(plan.clone()));
            }
        }
        HanProof::Stuck { plan, fate, col, plays } => {
            line(ui, format!("Han: no plan clears it ({plays} plays); furthest: {fate} at col {col:.0}"), ERR_TEXT);
            if !plan.is_empty() {
                line(ui, format!("with {plan}"), TEXT_DIM);
            }
        }
        HanProof::Error(e) => line(ui, format!("Han: {e}"), ERR_TEXT),
    }
    if proving && ui.small_button("stop Han").clicked() {
        act = Some(HanAct::Stop);
    }
    act
}
