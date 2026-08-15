use eframe::egui::{self, Color32, FontId, Margin, Pos2, Rect, RichText, Stroke, Vec2};
use crate::app::AnnotatorApp;
use crate::theme::{MUTED, PANEL, RED};

pub fn render_bottom_bar(app: &mut AnnotatorApp, ctx: &egui::Context) {
    let is_batch = app.dataset_folder.is_some() || app.image_files.len() > 1;
    if !is_batch {
        return;
    }

    let mut switch_to_idx = None;
    let current_idx = app.current_image_idx.unwrap_or(0);
    let total_images = app.image_files.len();
    let total_annotated = app.annotation_counts.values().filter(|&&count| count > 0).count();

    egui::TopBottomPanel::bottom("dataset_bottom_bar")
        .exact_height(54.0)
        .resizable(false)
        .frame(
            egui::Frame::none()
                .fill(PANEL)
                .inner_margin(Margin::symmetric(10.0, 4.0)),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;

                // 1. Counter Display (Sharp)
                let counter_text = format!("{:02} / {:02}", current_idx + 1, total_images);
                ui.label(
                    RichText::new(counter_text)
                        .font(FontId::monospace(11.0))
                        .strong()
                        .color(RED),
                );

                ui.separator();

                // 2. Sharp Thumbnails Filmstrip
                let scroll_width = (ui.available_width() - 190.0).max(80.0);
                egui::ScrollArea::horizontal()
                    .id_salt("dataset_filmstrip_scroll")
                    .auto_shrink([false, false])
                    .max_width(scroll_width)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;

                        for idx in 0..total_images {
                            let path = app.image_files[idx].clone();
                            app.request_thumbnail(&path);

                            let texture = app.thumbnail_cache.get(&path).cloned();
                            let is_active = Some(idx) == app.current_image_idx;
                            let count = app.annotation_counts.get(&path).copied().unwrap_or(0);

                            let file_name = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("image");

                            let card_size = Vec2::new(52.0, 44.0);
                            let (card_rect, response) =
                                ui.allocate_exact_size(card_size, egui::Sense::click());

                            if ui.is_rect_visible(card_rect) {
                                let painter = ui.painter();

                                let bg_color = if is_active {
                                    Color32::from_rgb(34, 26, 26)
                                } else if response.hovered() {
                                    Color32::from_gray(28)
                                } else {
                                    Color32::from_gray(16)
                                };

                                let border_stroke = if is_active {
                                    Stroke::new(1.0_f32, RED)
                                } else if response.hovered() {
                                    Stroke::new(1.0_f32, Color32::from_gray(75))
                                } else {
                                    Stroke::new(1.0_f32, Color32::from_gray(36))
                                };

                                // Sharp card background and border (0.0 radius)
                                painter.rect(card_rect, 0.0, bg_color, border_stroke);

                                // Thumbnail preview area (Sharp)
                                let thumb_area = Rect::from_min_size(
                                    card_rect.min + Vec2::new(2.0, 2.0),
                                    Vec2::new(card_size.x - 4.0, 28.0),
                                );

                                if let Some(tex) = &texture {
                                    let tex_size = tex.size_vec2();
                                    let aspect = tex_size.x / tex_size.y.max(1.0);
                                    let max_w = thumb_area.width();
                                    let max_h = thumb_area.height();

                                    let (w, h) = if aspect > max_w / max_h {
                                        (max_w, max_w / aspect)
                                    } else {
                                        (max_h * aspect, max_h)
                                    };

                                    let img_rect =
                                        Rect::from_center_size(thumb_area.center(), Vec2::new(w, h));
                                    painter.image(
                                        tex.id(),
                                        img_rect,
                                        Rect::from_min_max(
                                            Pos2::new(0.0, 0.0),
                                            Pos2::new(1.0, 1.0),
                                        ),
                                        Color32::WHITE,
                                    );
                                } else {
                                    painter.rect_filled(
                                        thumb_area,
                                        0.0,
                                        Color32::from_gray(22),
                                    );
                                    painter.text(
                                        thumb_area.center(),
                                        egui::Align2::CENTER_CENTER,
                                        "...",
                                        FontId::monospace(8.0),
                                        Color32::from_gray(70),
                                    );
                                }

                                // Bottom label bar (Sharp)
                                let label_rect = Rect::from_min_max(
                                    Pos2::new(card_rect.left() + 1.0, card_rect.bottom() - 13.0),
                                    Pos2::new(card_rect.right() - 1.0, card_rect.bottom() - 1.0),
                                );
                                let label_text = format!("#{:02}", idx + 1);
                                painter.text(
                                    label_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    label_text,
                                    FontId::monospace(9.0),
                                    if is_active {
                                        Color32::WHITE
                                    } else {
                                        Color32::from_gray(160)
                                    },
                                );

                                // Sharp annotation badge (top-right corner)
                                if count > 0 {
                                    let badge_rect = Rect::from_min_size(
                                        Pos2::new(card_rect.right() - 6.0, card_rect.top() + 2.0),
                                        Vec2::splat(4.0),
                                    );
                                    painter.rect_filled(badge_rect, 0.0, Color32::from_rgb(0, 230, 118));
                                }
                            }

                            response.clone().on_hover_ui(|ui| {
                                ui.label(
                                    RichText::new(format!("#{:02}  {}", idx + 1, file_name))
                                        .font(FontId::monospace(10.0))
                                        .strong(),
                                );
                                if count > 0 {
                                    ui.label(
                                        RichText::new(format!("{count} annotation(s) saved"))
                                            .font(FontId::monospace(9.0))
                                            .color(Color32::from_rgb(0, 230, 118)),
                                    );
                                } else {
                                    ui.label(
                                        RichText::new("No annotations yet")
                                            .font(FontId::monospace(9.0))
                                            .color(MUTED),
                                    );
                                }
                            });

                            if response.clicked() {
                                switch_to_idx = Some(idx);
                            }
                        }
                    });

                // 3. Right Aligned Summary & Sharp Auto-Save Indicator
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let auto_save_color = if app.auto_save_dataset {
                        Color32::from_rgb(0, 230, 118)
                    } else {
                        MUTED
                    };

                    let auto_save_btn = ui.add(
                        egui::Button::new(
                            RichText::new(if app.auto_save_dataset { "AUTO-SAVE: ON" } else { "AUTO-SAVE: OFF" })
                                .font(FontId::monospace(9.0))
                                .color(auto_save_color),
                        )
                        .fill(Color32::from_gray(22))
                        .rounding(0.0)
                        .min_size(Vec2::new(0.0, 20.0)),
                    );
                    if auto_save_btn.clicked() {
                        app.auto_save_dataset = !app.auto_save_dataset;
                    }

                    let progress_text = format!("{total_annotated}/{total_images} ANNOTATED");
                    ui.label(
                        RichText::new(progress_text)
                            .font(FontId::monospace(9.0))
                            .color(MUTED),
                    );
                });
            });
        });

    if let Some(idx) = switch_to_idx {
        app.switch_to_image_index(ctx, idx);
    }
}
