use eframe::egui::{self, Color32, Stroke, Vec2};

pub const RED: Color32 = Color32::from_rgb(255, 0, 0);
pub const BG: Color32 = Color32::from_rgb(10, 10, 11);
pub const PANEL: Color32 = Color32::from_rgb(16, 16, 18);
pub const LINE: Color32 = Color32::from_rgb(43, 43, 47);
pub const MUTED: Color32 = Color32::from_rgb(135, 135, 143);

pub fn configure_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals.dark_mode = true;
    style.visuals.panel_fill = PANEL;
    style.visuals.window_fill = PANEL;
    style.visuals.faint_bg_color = Color32::from_rgb(22, 22, 24);
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(25, 25, 28);
    style.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(25, 25, 28);
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, LINE);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(34, 34, 38);
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, Color32::from_gray(80));
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(42, 42, 46);
    style.visuals.selection.bg_fill = Color32::from_rgba_premultiplied(255, 0, 0, 45);
    style.visuals.selection.stroke = Stroke::new(1.0_f32, RED);
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    ctx.set_style(style);
}
