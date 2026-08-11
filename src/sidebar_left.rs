use eframe::egui::{self, Color32, FontFamily, Margin, RichText, Vec2};
use crate::app::AnnotatorApp;
use crate::theme::{MUTED, PANEL, RED};

pub fn render_left_sidebar(app: &mut AnnotatorApp, ctx: &egui::Context) {
    egui::SidePanel::left("left_sidebar")
        .exact_width(220.0)
        .resizable(false)
        .frame(
            egui::Frame::none()
                .fill(PANEL)
                .inner_margin(Margin::same(16.0)),
        )
        .show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("SCENE REGIONS")
                        .size(10.0)
                        .strong()
                        .color(MUTED),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(app.annotations.len().to_string())
                            .size(11.0)
                            .strong()
                            .color(RED),
                    );
                });
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            if app.annotations.is_empty() {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("NO REGIONS")
                            .family(FontFamily::Monospace)
                            .size(11.0)
                            .color(MUTED),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Draw on the image to\nadd regions to the scene.")
                            .size(10.0)
                            .color(Color32::from_gray(92)),
                    );
                });
            } else {
                let root_ids: Vec<u32> = app
                    .annotations
                    .iter()
                    .filter(|a| a.parent_id.is_none())
                    .map(|a| a.id)
                    .collect();

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for root_id in root_ids {
                            render_tree_node(ui, app, root_id, 0);
                        }
                    });
            }
        });
}

fn render_tree_node(
    ui: &mut egui::Ui,
    app: &mut AnnotatorApp,
    annotation_id: u32,
    depth: usize,
) {
    let (label, color32, is_selected) = {
        let Some(a) = app.annotations.iter().find(|a| a.id == annotation_id) else {
            return;
        };
        (a.label.clone(), a.color32(), app.selected == Some(annotation_id))
    };

    let row_height = 24.0;
    let row_rect = egui::Rect::from_min_size(
        ui.cursor().min,
        Vec2::new(ui.available_width(), row_height),
    );

    let response = ui.allocate_rect(row_rect, egui::Sense::click());

    let bg_color = if is_selected {
        Color32::from_gray(38)
    } else if response.hovered() {
        Color32::from_gray(26)
    } else {
        Color32::TRANSPARENT
    };

    let indent_offset = (depth as f32) * 14.0;

    if ui.is_rect_visible(row_rect) {
        ui.painter().rect_filled(row_rect, 2.0, bg_color);

        if depth > 0 {
            ui.painter().text(
                egui::Pos2::new(row_rect.left() + indent_offset - 2.0, row_rect.center().y),
                egui::Align2::LEFT_CENTER,
                "└─",
                egui::FontId::monospace(9.0),
                Color32::from_gray(100),
            );
        }

        let swatch_x = row_rect.left() + indent_offset + 12.0;
        let swatch_rect = egui::Rect::from_center_size(
            egui::Pos2::new(swatch_x, row_rect.center().y),
            Vec2::splat(10.0),
        );
        ui.painter().rect_filled(swatch_rect, 2.0, color32);

        let text = format!("{:02}  {}", annotation_id, label);
        let text_color = if is_selected {
            Color32::WHITE
        } else {
            Color32::from_gray(190)
        };

        ui.painter().text(
            egui::Pos2::new(swatch_x + 10.0, row_rect.center().y),
            egui::Align2::LEFT_CENTER,
            text,
            egui::FontId::monospace(11.0),
            text_color,
        );
    }

    ui.advance_cursor_after_rect(row_rect);
    ui.add_space(2.0);

    if response.clicked() {
        app.selected = Some(annotation_id);
    }

    let children_ids: Vec<u32> = app
        .annotations
        .iter()
        .filter(|a| a.parent_id == Some(annotation_id))
        .map(|a| a.id)
        .collect();

    for child_id in children_ids {
        render_tree_node(ui, app, child_id, depth + 1);
    }
}
