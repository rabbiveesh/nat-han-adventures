//! The look: the approved mockup's dark chiptune palette, monospace everywhere.

use bevy_egui::egui::{self, Color32, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle};

pub const BG: Color32 = Color32::from_rgb(0x14, 0x10, 0x0c);
pub const PANEL: Color32 = Color32::from_rgb(0x1d, 0x17, 0x12);
pub const RAISED: Color32 = Color32::from_rgb(0x25, 0x1d, 0x16);
pub const HILITE: Color32 = Color32::from_rgb(0x2f, 0x25, 0x19);
pub const LINE: Color32 = Color32::from_rgb(0x3a, 0x2f, 0x25);
pub const LINE_SOFT: Color32 = Color32::from_rgb(0x2b, 0x23, 0x1b);
pub const BAR_LINE: Color32 = Color32::from_rgb(0x5a, 0x46, 0x32);
pub const TEXT: Color32 = Color32::from_rgb(0xf1, 0xe4, 0xc6);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0xb8, 0xa8, 0x88);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(0x8c, 0x7d, 0x65);
pub const TEXT_GHOST: Color32 = Color32::from_rgb(0x4d, 0x41, 0x34);
pub const ROW_NUM: Color32 = Color32::from_rgb(0x5f, 0x52, 0x43);
pub const GOLD: Color32 = Color32::from_rgb(0xf0, 0xb4, 0x29);
pub const PLAYHEAD_ROW: Color32 = Color32::from_rgb(0x4a, 0x3a, 0x14);
pub const BAR_ROW: Color32 = Color32::from_rgb(0x2b, 0x22, 0x19);
pub const BEAT_ROW: Color32 = Color32::from_rgb(0x21, 0x1a, 0x14);
pub const RED: Color32 = Color32::from_rgb(0xff, 0x6b, 0x5b);
pub const ERR_BG: Color32 = Color32::from_rgb(0x3a, 0x1a, 0x14);
pub const ERR_LINE_BG: Color32 = Color32::from_rgb(0x2a, 0x16, 0x12);
pub const ERR_TEXT: Color32 = Color32::from_rgb(0xff, 0xb3, 0xa8);
pub const GREEN: Color32 = Color32::from_rgb(0x9f, 0xd3, 0x9a);
pub const KEY_WHITE: Color32 = Color32::from_rgb(0xd9, 0xcd, 0xb2);
pub const KEY_BLACK: Color32 = Color32::from_rgb(0x2a, 0x22, 0x1a);
pub const LANE_WHITE: Color32 = Color32::from_rgb(0x1d, 0x17, 0x12);
pub const LANE_BLACK: Color32 = Color32::from_rgb(0x18, 0x13, 0x0f);

/// pulse 1, pulse 2, triangle, noise.
pub const CHANNEL: [Color32; 4] = [
    Color32::from_rgb(0x7a, 0xb8, 0xff),
    Color32::from_rgb(0xff, 0x9a, 0x52),
    Color32::from_rgb(0xc9, 0xa2, 0xff),
    Color32::from_rgb(0x9f, 0xd3, 0x9a),
];
pub const CHANNEL_NAME: [&str; 4] = ["PULSE 1", "PULSE 2", "TRIANGLE", "NOISE"];
pub const ROLE: [&str; 4] = ["lead", "comp", "bass", "drums"];

/// The pixel font, for the logo.
pub const PIXEL: &str = "pixel";

pub fn pixel(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(PIXEL.into()))
}

pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        PIXEL.into(),
        std::sync::Arc::new(FontData::from_static(include_bytes!("../../../assets/fonts/PressStart2P-Regular.ttf"))),
    );
    fonts.families.insert(FontFamily::Name(PIXEL.into()), vec![PIXEL.into()]);
    // Monospace for everything (the mockup is all IBM Plex Mono).
    let mono = fonts.families[&FontFamily::Monospace].clone();
    fonts.families.insert(FontFamily::Proportional, mono);
    ctx.set_fonts(fonts);
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Small, mono_id(10.5)),
            (TextStyle::Body, mono_id(12.0)),
            (TextStyle::Monospace, mono_id(12.0)),
            (TextStyle::Button, mono_id(12.0)),
            (TextStyle::Heading, mono_id(14.0)),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
        style.spacing.interact_size.y = 22.0;
        let v = &mut style.visuals;
        v.dark_mode = true;
        v.override_text_color = Some(TEXT);
        v.panel_fill = PANEL;
        v.window_fill = PANEL;
        v.extreme_bg_color = BG;
        v.faint_bg_color = RAISED;
        v.code_bg_color = BG;
        v.selection.bg_fill = Color32::from_rgb(0x5a, 0x46, 0x20);
        v.selection.stroke = Stroke::new(1.0, GOLD);
        v.hyperlink_color = GOLD;
        v.text_cursor.stroke = Stroke::new(2.0, GOLD);
        for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
            w.corner_radius = egui::CornerRadius::same(4);
        }
        v.widgets.noninteractive.bg_fill = PANEL;
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_DIM);
        v.widgets.inactive.bg_fill = RAISED;
        v.widgets.inactive.weak_bg_fill = RAISED;
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE);
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
        v.widgets.hovered.bg_fill = HILITE;
        v.widgets.hovered.weak_bg_fill = HILITE;
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, BAR_LINE);
        v.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
        v.widgets.active.bg_fill = GOLD;
        v.widgets.active.weak_bg_fill = GOLD;
        v.widgets.active.fg_stroke = Stroke::new(1.0, TEXT);
        v.slider_trailing_fill = true;
        v.selection.bg_fill = GOLD.gamma_multiply(0.35);
    });
}

fn mono_id(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

/// A little section title ("GAMEPLAY FEED").
pub fn caption(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).color(TEXT_FAINT).size(11.0));
}

/// A toggle chip (gold when on).
pub fn chip(ui: &mut egui::Ui, on: bool, enabled: bool, label: &str) -> egui::Response {
    let (fill, fg, stroke) = if on { (GOLD, BG, GOLD) } else { (RAISED, TEXT_DIM, LINE) };
    let text = egui::RichText::new(label).color(if enabled { fg } else { TEXT_GHOST }).size(11.0);
    let text = if on { text.strong() } else { text };
    ui.add_enabled(enabled, egui::Button::new(text).fill(fill).stroke(Stroke::new(1.0, stroke)).corner_radius(4))
}
