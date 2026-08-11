use eframe::egui::{self, Color32, FontFamily, FontId, Margin, RichText, Stroke, Vec2};
use crate::app::AnnotatorApp;
use crate::theme::{MUTED, PANEL, RED};

pub fn render_right_sidebar(app: &mut AnnotatorApp, ctx: &egui::Context) {
    let mut export_requested = false;

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

                    ui.add_space(10.0);
                    ui.label(RichText::new("DESCRIPTION").size(9.0).color(MUTED));
                    ui.add_space(4.0);

                    let mut desc = annotation.description.clone().unwrap_or_default();
                    let desc_response = ui.add(
                        egui::TextEdit::multiline(&mut desc)
                            .font(FontId::monospace(11.0))
                            .desired_width(f32::INFINITY)
                            .desired_rows(3)
                            .margin(Vec2::new(8.0, 7.0)),
                    );
                    if desc_response.changed() {
                        annotation.description = if desc.trim().is_empty() {
                            None
                        } else {
                            Some(desc)
                        };
                    }

                    ui.add_space(12.0);
                    ui.label(RichText::new("COLOR PRESETS").size(9.0).color(MUTED));
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        let presets: [[u8; 3]; 6] = [
                            [255, 0, 0],   // Red
                            [0, 230, 118], // Green
                            [41, 121, 255], // Blue
                            [255, 214, 0], // Yellow
                            [255, 145, 0], // Orange
                            [0, 229, 255], // Cyan
                        ];

                        for rgb in presets {
                            let color32 = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                            let is_selected = annotation.color == rgb;

                            let (rect, response) =
                                ui.allocate_exact_size(Vec2::splat(20.0), egui::Sense::click());
                            if ui.is_rect_visible(rect) {
                                ui.painter().rect_filled(rect, 2.0, color32);
                                if is_selected {
                                    ui.painter().rect_stroke(
                                        rect.expand(2.0),
                                        2.0,
                                        Stroke::new(2.0_f32, Color32::WHITE),
                                    );
                                } else if response.hovered() {
                                    ui.painter().rect_stroke(
                                        rect.expand(1.0),
                                        2.0,
                                        Stroke::new(1.0_f32, Color32::GRAY),
                                    );
                                }
                            }
                            if response.clicked() {
                                annotation.color = rgb;
                            }
                        }

                        ui.add_space(4.0);

                        let (rect, picker_response) =
                            ui.allocate_exact_size(Vec2::splat(20.0), egui::Sense::click());
                        if ui.is_rect_visible(rect) {
                            ui.painter().rect_filled(rect, 2.0, annotation.color32());
                            ui.painter().rect_stroke(
                                rect,
                                2.0,
                                Stroke::new(1.0_f32, Color32::from_gray(120)),
                            );
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "+",
                                FontId::monospace(12.0),
                                Color32::WHITE,
                            );
                        }

                        let popup_id = ui.make_persistent_id("custom_color_picker_popup");
                        if picker_response.clicked() {
                            ui.memory_mut(|mem| mem.toggle_popup(popup_id));
                        }

                        egui::popup_below_widget(
                            ui,
                            popup_id,
                            &picker_response,
                            egui::PopupCloseBehavior::CloseOnClickOutside,
                            |ui| {
                                ui.set_max_width(200.0);
                                let mut color32 = annotation.color32();
                                if egui::color_picker::color_picker_color32(
                                    ui,
                                    &mut color32,
                                    egui::color_picker::Alpha::Opaque,
                                ) {
                                    annotation.color = [color32.r(), color32.g(), color32.b()];
                                }
                            },
                        );
                    });

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

            if app.image.is_some() {
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    export_requested = ui
                        .add_sized(
                            [ui.available_width(), 32.0],
                            egui::Button::new(
                                RichText::new("EXPORT JSON")
                                    .size(10.0)
                                    .strong()
                                    .color(Color32::BLACK),
                            )
                            .fill(Color32::WHITE),
                        )
                        .clicked();
                });
            }
        });

    if export_requested {
        app.save_dialog();
    }
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
