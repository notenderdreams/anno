use eframe::egui::{
    self, Align2, Color32, FontId, Margin, Pos2, Rect, Sense, Stroke, Vec2,
};

use crate::app::AnnotatorApp;
use crate::geometry::{annotation_screen_rect, screen_to_image};
use crate::models::{Annotation, Draft};
use crate::render::draw_surveillance_box;
use crate::theme::{BG, LINE, MUTED};

pub fn render_canvas(app: &mut AnnotatorApp, ctx: &egui::Context) {
    egui::CentralPanel::default()
        .frame(
            egui::Frame::none()
                .fill(BG)
                .inner_margin(Margin::same(18.0)),
        )
        .show(ctx, |ui| {
            let available = ui.available_size();
            let (response, painter) = ui.allocate_painter(available, Sense::click_and_drag());
            let canvas = response.rect;

            let Some(image) = &app.image else {
                painter.rect_stroke(canvas.shrink(1.0), 0.0, Stroke::new(1.0_f32, LINE));
                let center = canvas.center();

                painter.text(
                    center - Vec2::new(0.0, 30.0),
                    Align2::CENTER_CENTER,
                    "+",
                    FontId::monospace(30.0),
                    Color32::from_gray(72),
                );
                painter.text(
                    center + Vec2::new(0.0, 12.0),
                    Align2::CENTER_CENTER,
                    "DROP AN IMAGE HERE",
                    FontId::monospace(13.0),
                    Color32::from_gray(170),
                );
                painter.text(
                    center + Vec2::new(0.0, 35.0),
                    Align2::CENTER_CENTER,
                    "PNG  JPG  WEBP  BMP  TIFF",
                    FontId::monospace(9.0),
                    MUTED,
                );

                if response.clicked() {
                    app.open_dialog(ctx);
                }
                return;
            };

            let image_size = Vec2::new(image.width as f32, image.height as f32);
            let scale = (canvas.width() / image_size.x)
                .min(canvas.height() / image_size.y)
                .min(1.0);

            let display_size = image_size * scale;
            let image_rect = Rect::from_center_size(canvas.center(), display_size);

            painter.rect_filled(image_rect.expand(8.0), 0.0, Color32::BLACK);
            painter.image(
                image.texture.id(),
                image_rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );

            if response.drag_started() {
                if let Some(pointer) = response
                    .interact_pointer_pos()
                    .filter(|point| image_rect.contains(*point))
                {
                    let hit = app
                        .annotations
                        .iter()
                        .rev()
                        .find(|annotation| {
                            annotation_screen_rect(annotation, image_rect, image_size)
                                .contains(pointer)
                        })
                        .map(|annotation| annotation.id);

                    if let Some(id) = hit {
                        app.selected = Some(id);
                        app.draft = None;
                    } else {
                        app.selected = None;
                        app.draft = Some(Draft {
                            start: pointer,
                            current: pointer,
                        });
                    }
                }
            }

            if response.dragged() {
                if let (Some(draft), Some(pointer)) =
                    (&mut app.draft, response.interact_pointer_pos())
                {
                    draft.current = image_rect.clamp(pointer);
                }
            }

            if response.drag_stopped() {
                if let Some(draft) = app.draft.take() {
                    let rect = Rect::from_two_pos(draft.start, draft.current);

                    if rect.width() >= 8.0 && rect.height() >= 8.0 {
                        let min = screen_to_image(rect.min, image_rect, image_size);
                        let max = screen_to_image(rect.max, image_rect, image_size);

                        let id = app.next_id;
                        app.next_id += 1;

                        app.annotations.push(Annotation {
                            id,
                            label: format!("object_{id:02}"),
                            x: min.x.round(),
                            y: min.y.round(),
                            width: (max.x - min.x).round(),
                            height: (max.y - min.y).round(),
                        });

                        app.selected = Some(id);
                        app.request_label_focus = true;
                        app.status = format!("REGION {id:02} CREATED");
                    }
                }
            }

            if response.clicked() {
                if let Some(pointer) = response.interact_pointer_pos() {
                    app.selected = app
                        .annotations
                        .iter()
                        .rev()
                        .find(|annotation| {
                            annotation_screen_rect(annotation, image_rect, image_size)
                                .contains(pointer)
                        })
                        .map(|annotation| annotation.id);
                }
            }

            for annotation in &app.annotations {
                let rect = annotation_screen_rect(annotation, image_rect, image_size);
                draw_surveillance_box(
                    &painter,
                    rect,
                    &annotation.label,
                    app.selected == Some(annotation.id),
                );
            }

            if let Some(draft) = &app.draft {
                draw_surveillance_box(
                    &painter,
                    Rect::from_two_pos(draft.start, draft.current),
                    "NEW REGION",
                    true,
                );
            }
        });
}
