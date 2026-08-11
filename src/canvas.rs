use eframe::egui::{
    self, Align2, Color32, CursorIcon, FontId, Margin, Pos2, Rect, Sense, Stroke, Vec2,
};

use crate::app::AnnotatorApp;
use crate::geometry::{annotation_screen_rect, screen_to_image};
use crate::models::{ActiveDrag, Annotation, Draft, ResizeHandle};
use crate::render::draw_surveillance_box;
use crate::theme::{BG, LINE, MUTED};

fn hit_resize_handle(rect: Rect, pointer: Pos2) -> Option<ResizeHandle> {
    let radius = 10.0;
    if rect.left_top().distance(pointer) <= radius {
        Some(ResizeHandle::TopLeft)
    } else if rect.right_top().distance(pointer) <= radius {
        Some(ResizeHandle::TopRight)
    } else if rect.left_bottom().distance(pointer) <= radius {
        Some(ResizeHandle::BottomLeft)
    } else if rect.right_bottom().distance(pointer) <= radius {
        Some(ResizeHandle::BottomRight)
    } else {
        None
    }
}

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

            if let Some(pointer) = response.hover_pos() {
                if image_rect.contains(pointer) {
                    let mut cursor_set = false;
                    if let Some(selected_id) = app.selected {
                        if let Some(annotation) = app.annotations.iter().find(|a| a.id == selected_id) {
                            let rect = annotation_screen_rect(annotation, image_rect, image_size);
                            if let Some(handle) = hit_resize_handle(rect, pointer) {
                                match handle {
                                    ResizeHandle::TopLeft | ResizeHandle::BottomRight => {
                                        ctx.set_cursor_icon(CursorIcon::ResizeNwSe);
                                    }
                                    ResizeHandle::TopRight | ResizeHandle::BottomLeft => {
                                        ctx.set_cursor_icon(CursorIcon::ResizeNeSw);
                                    }
                                }
                                cursor_set = true;
                            }
                        }
                    }
                    if !cursor_set {
                        if app.annotations.iter().rev().any(|a| {
                            annotation_screen_rect(a, image_rect, image_size).contains(pointer)
                        }) {
                            ctx.set_cursor_icon(CursorIcon::Move);
                        }
                    }
                }
            }

            if response.drag_started() {
                if let Some(pointer) = response
                    .interact_pointer_pos()
                    .filter(|point| image_rect.contains(*point))
                {
                    let mut handled = false;
                    if let Some(selected_id) = app.selected {
                        if let Some(annotation) = app.annotations.iter().find(|a| a.id == selected_id) {
                            let rect = annotation_screen_rect(annotation, image_rect, image_size);
                            if let Some(handle) = hit_resize_handle(rect, pointer) {
                                app.active_drag = Some(ActiveDrag::Resize {
                                    id: selected_id,
                                    handle,
                                    start_pointer: pointer,
                                    initial_x: annotation.x,
                                    initial_y: annotation.y,
                                    initial_w: annotation.width,
                                    initial_h: annotation.height,
                                });
                                handled = true;
                            }
                        }
                    }

                    if !handled {
                        let hit = app
                            .annotations
                            .iter()
                            .rev()
                            .find(|annotation| {
                                annotation_screen_rect(annotation, image_rect, image_size)
                                    .contains(pointer)
                            })
                            .map(|a| (a.id, a.x, a.y));

                        if let Some((id, x, y)) = hit {
                            app.selected = Some(id);
                            app.active_drag = Some(ActiveDrag::Move {
                                id,
                                start_pointer: pointer,
                                initial_x: x,
                                initial_y: y,
                            });
                        } else {
                            app.selected = None;
                            app.draft = Some(Draft {
                                start: pointer,
                                current: pointer,
                            });
                        }
                    }
                }
            }

            if response.dragged() {
                if let Some(pointer) = response.interact_pointer_pos() {
                    if let Some(active_drag) = &app.active_drag {
                        let delta_screen = pointer - active_drag_start_pointer(active_drag);
                        let delta_x = delta_screen.x / image_rect.width() * image_size.x;
                        let delta_y = delta_screen.y / image_rect.height() * image_size.y;

                        match active_drag {
                            ActiveDrag::Move {
                                id,
                                initial_x,
                                initial_y,
                                ..
                            } => {
                                if let Some(annotation) = app.annotations.iter_mut().find(|a| a.id == *id) {
                                    let new_x = (initial_x + delta_x).clamp(0.0, image_size.x - annotation.width);
                                    let new_y = (initial_y + delta_y).clamp(0.0, image_size.y - annotation.height);
                                    annotation.x = new_x.round();
                                    annotation.y = new_y.round();
                                }
                            }
                            ActiveDrag::Resize {
                                id,
                                handle,
                                initial_x,
                                initial_y,
                                initial_w,
                                initial_h,
                                ..
                            } => {
                                if let Some(annotation) = app.annotations.iter_mut().find(|a| a.id == *id) {
                                    match handle {
                                        ResizeHandle::TopLeft => {
                                            let new_x = (initial_x + delta_x).clamp(0.0, initial_x + initial_w - 8.0);
                                            let new_y = (initial_y + delta_y).clamp(0.0, initial_y + initial_h - 8.0);
                                            annotation.x = new_x.round();
                                            annotation.y = new_y.round();
                                            annotation.width = (initial_x + initial_w - new_x).round();
                                            annotation.height = (initial_y + initial_h - new_y).round();
                                        }
                                        ResizeHandle::TopRight => {
                                            let new_y = (initial_y + delta_y).clamp(0.0, initial_y + initial_h - 8.0);
                                            let max_x = (initial_x + initial_w + delta_x).clamp(initial_x + 8.0, image_size.x);
                                            annotation.y = new_y.round();
                                            annotation.width = (max_x - initial_x).round();
                                            annotation.height = (initial_y + initial_h - new_y).round();
                                        }
                                        ResizeHandle::BottomLeft => {
                                            let new_x = (initial_x + delta_x).clamp(0.0, initial_x + initial_w - 8.0);
                                            let max_y = (initial_y + initial_h + delta_y).clamp(initial_y + 8.0, image_size.y);
                                            annotation.x = new_x.round();
                                            annotation.width = (initial_x + initial_w - new_x).round();
                                            annotation.height = (max_y - initial_y).round();
                                        }
                                        ResizeHandle::BottomRight => {
                                            let max_x = (initial_x + initial_w + delta_x).clamp(initial_x + 8.0, image_size.x);
                                            let max_y = (initial_y + initial_h + delta_y).clamp(initial_y + 8.0, image_size.y);
                                            annotation.width = (max_x - initial_x).round();
                                            annotation.height = (max_y - initial_y).round();
                                        }
                                    }
                                }
                            }
                        }
                    } else if let Some(draft) = &mut app.draft {
                        draft.current = image_rect.clamp(pointer);
                    }
                }
            }

            if response.drag_stopped() {
                app.active_drag = None;
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

fn active_drag_start_pointer(drag: &ActiveDrag) -> Pos2 {
    match drag {
        ActiveDrag::Move { start_pointer, .. } => *start_pointer,
        ActiveDrag::Resize { start_pointer, .. } => *start_pointer,
    }
}
