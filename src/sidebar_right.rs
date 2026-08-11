use eframe::egui::{self, Color32, FontFamily, FontId, Margin, RichText, Vec2};
use crate::app::AnnotatorApp;
use crate::theme::{MUTED, PANEL, RED};

pub fn render_right_sidebar(app: &mut AnnotatorApp, ctx: &egui::Context) {
    egui::SidePanel::right("right_sidebar")
        .exact_width(240.0)
        .resizable(false)
        .frame(
            egui::Frame::none()
                .fill(PANEL)
                .inner_margin(Margin::same(16.0)),
        )
        .show(ctx, |ui| {
            ui.add_space(4.0);

            if let Some(selected_id) = app.selected {
                let mut should_delete = false;
                if let Some(annotation) = app.annotations.iter_mut().find(|a| a.id == selected_id) {
                    ui.label(
                        RichText::new(format!("REGION {:02}", annotation.id))
                            .size(9.0)
                            .color(RED),
                    );
                    ui.add_space(8.0);
                    ui.label(RichText::new("LABEL").size(9.0).color(MUTED));

                    let response = ui.add(
                        egui::TextEdit::singleline(&mut annotation.label)
                            .font(FontId::monospace(12.0))
                            .desired_width(f32::INFINITY)
                            .margin(Vec2::new(8.0, 7.0)),
                    );

                    if app.request_label_focus {
                        response.request_focus();
                        app.request_label_focus = false;
                    }

                    ui.add_space(12.0);
                    ui.label(RichText::new("BOUNDS (PX)").size(9.0).color(MUTED));
                    ui.add_space(4.0);

                    egui::Grid::new("right_bounds_grid")
                        .num_columns(2)
                        .spacing([20.0, 6.0])
                        .show(ui, |ui| {
                            bound_row(ui, "X", annotation.x);
                            bound_row(ui, "Y", annotation.y);
                            ui.end_row();
                            bound_row(ui, "W", annotation.width);
                            bound_row(ui, "H", annotation.height);
                            ui.end_row();
                        });

                    ui.add_space(16.0);
                    should_delete = ui
                        .add_sized(
                            [ui.available_width(), 30.0],
                            egui::Button::new(RichText::new("DELETE REGION").size(10.0).color(RED)),
                        )
                        .clicked();
                }

                if should_delete {
                    app.delete_selected();
                }
            } else {
                ui.add_space(16.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("┌   ┐\n\n└   ┘")
                            .family(FontFamily::Monospace)
                            .size(18.0)
                            .color(Color32::from_gray(62)),
                    );
                    ui.add_space(8.0);
                    ui.label(RichText::new("NO REGION SELECTED").size(10.0).color(MUTED));
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Click or drag on image to annotate.")
                            .size(10.0)
                            .color(Color32::from_gray(92)),
                    );
                });
            }
        });
}

fn bound_row(ui: &mut egui::Ui, label: &str, val: f32) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(9.0).color(RED));
        ui.label(
            RichText::new(format!("{val:.0}"))
                .family(FontFamily::Monospace)
                .size(11.0),
        );
    });
}
