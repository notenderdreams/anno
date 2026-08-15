use eframe::egui::{
    self, Align2, Color32, CursorIcon, FontId, Key, Margin, PointerButton, Pos2, Rect, Sense, Stroke, Vec2,
};

use crate::app::AnnotatorApp;
use crate::geometry::{annotation_screen_rect, annotation_tag_rect, screen_to_image, update_hierarchy};
use crate::models::{ActiveDrag, Annotation, Draft, ResizeHandle};
use crate::render::draw_surveillance_box;
use crate::theme::{BG, MUTED, RED};

const MIN_ZOOM: f32 = 1.0;
const MAX_ZOOM: f32 = 20.0;

fn clamp_pan(pan: Vec2, canvas_size: Vec2, display_size: Vec2) -> Vec2 {
    let limit = ((display_size - canvas_size) * 0.5).max(Vec2::ZERO);
    Vec2::new(
        pan.x.clamp(-limit.x, limit.x),
        pan.y.clamp(-limit.y, limit.y),
    )
}

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

pub fn hit_annotation<'a>(
    annotations: &'a [Annotation],
    image_rect: Rect,
    image_size: Vec2,
    pointer: Pos2,
) -> Option<&'a Annotation> {
    // 1. Tags have absolute highest priority so inner annotations can always be selected
    // even if their tag falls inside another annotation's bounding box.
    if let Some(hit) = annotations
        .iter()
        .rev()
        .find(|a| annotation_tag_rect(a, image_rect, image_size).contains(pointer))
    {
        return Some(hit);
    }

    // 2. If no tag hit, check bounding boxes, prioritizing innermost (smallest area) box.
    annotations
        .iter()
        .enumerate()
        .filter(|(_, a)| annotation_screen_rect(a, image_rect, image_size).contains(pointer))
        .min_by(|(idx_a, a), (idx_b, b)| {
            let area_a = a.width * a.height;
            let area_b = b.width * b.height;
            area_a
                .partial_cmp(&area_b)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| idx_b.cmp(idx_a))
        })
        .map(|(_, a)| a)
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
            let fit_scale = (canvas.width() / image_size.x)
                .min(canvas.height() / image_size.y)
                .min(1.0);

            // Keep the image point beneath the cursor fixed while zooming.
            if response.hovered() && !response.dragged() {
                let scroll_y = ctx.input(|input| input.raw_scroll_delta.y);
                if scroll_y != 0.0 {
                    if let Some(pointer) = response.hover_pos() {
                        let old_zoom = app.zoom;
                        let new_zoom = (old_zoom * (scroll_y * 0.02).exp())
                            .clamp(MIN_ZOOM, MAX_ZOOM);
                        let zoom_ratio = new_zoom / old_zoom;
                        let old_center = canvas.center() + app.pan;
                        let new_center = pointer + (old_center - pointer) * zoom_ratio;

                        app.zoom = new_zoom;
                        app.pan = new_center - canvas.center();
                    }
                }
            }

            let display_size = image_size * fit_scale * app.zoom;
            app.pan = clamp_pan(app.pan, canvas.size(), display_size);

            // Space-drag supports trackpads; middle-drag supports mice without
            // taking the primary button away from annotation drawing.
            let space_held = ctx.input(|input| input.key_down(Key::Space));
            let is_panning = response.dragged_by(PointerButton::Middle)
                || (space_held && response.dragged_by(PointerButton::Primary));
            if is_panning {
                let pointer_delta = ctx.input(|input| input.pointer.delta());
                app.pan = clamp_pan(app.pan + pointer_delta, canvas.size(), display_size);
                ctx.set_cursor_icon(CursorIcon::Grabbing);
            } else if response.hovered() && space_held {
                ctx.set_cursor_icon(CursorIcon::Grab);
            }

            let image_rect = Rect::from_center_size(canvas.center() + app.pan, display_size);

            painter.rect_filled(image_rect.expand(8.0), 0.0, Color32::BLACK);
            painter.image(
                image.texture.id(),
                image_rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );

            if !is_panning && !space_held {
                if let Some(pointer) = response.hover_pos() {
                    if image_rect.contains(pointer) {
                        let mut cursor_set = false;
                        for &selected_id in &app.selected {
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
                                    break;
                                }
                            }
                        }
                        if !cursor_set {
                            let over_tag = app.annotations.iter().any(|a| {
                                annotation_tag_rect(a, image_rect, image_size).contains(pointer)
                            });
                            let over_selected_body = app.annotations.iter().any(|a| {
                                app.selected.contains(&a.id)
                                    && annotation_screen_rect(a, image_rect, image_size).contains(pointer)
                            });

                            if over_tag || over_selected_body {
                                ctx.set_cursor_icon(CursorIcon::Move);
                            } else {
                                ctx.set_cursor_icon(CursorIcon::Crosshair);
                            }
                        }
                    }
                }
            }

            if response.double_clicked() {
                if let Some(pointer) = response.interact_pointer_pos() {
                    if let Some(hit) = hit_annotation(&app.annotations, image_rect, image_size, pointer) {
                        let hit_id = hit.id;
                        app.history.begin_edit(app.current_snapshot());
                        app.select_single(hit_id);
                        app.editing_label = Some(hit_id);
                        app.request_label_focus = true;
                    }
                }
            } else if response.drag_started_by(PointerButton::Primary) && !space_held {
                if let Some(pointer) = response
                    .interact_pointer_pos()
                    .filter(|point| image_rect.contains(*point))
                {
                    let shift_held = ctx.input(|i| i.modifiers.shift || i.modifiers.command || i.modifiers.ctrl);
                    let mut handled = false;

                    // 1. Check resize handle on selected annotations
                    for &selected_id in &app.selected {
                        if let Some(annotation) = app.annotations.iter().find(|a| a.id == selected_id) {
                            let rect = annotation_screen_rect(annotation, image_rect, image_size);
                            if let Some(handle) = hit_resize_handle(rect, pointer) {
                                app.history.begin_edit(app.current_snapshot());
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
                                break;
                            }
                        }
                    }

                    // 2. Check hit on annotation tag or ALREADY SELECTED body to move
                    if !handled {
                        let hit_tag = app
                            .annotations
                            .iter()
                            .rev()
                            .find(|a| annotation_tag_rect(a, image_rect, image_size).contains(pointer));

                        let hit_selected_body = app
                            .annotations
                            .iter()
                            .find(|a| {
                                app.selected.contains(&a.id)
                                    && annotation_screen_rect(a, image_rect, image_size).contains(pointer)
                            });

                        if let Some(hit) = hit_tag.or(hit_selected_body) {
                            let id = hit.id;
                            if shift_held {
                                app.selected.insert(id);
                            } else if !app.selected.contains(&id) {
                                app.select_single(id);
                            }

                            let initial_positions: Vec<(u32, f32, f32)> = app
                                .annotations
                                .iter()
                                .filter(|a| app.selected.contains(&a.id))
                                .map(|a| (a.id, a.x, a.y))
                                .collect();

                            app.history.begin_edit(app.current_snapshot());
                            app.active_drag = Some(ActiveDrag::Move {
                                initial_positions,
                                start_pointer: pointer,
                            });
                            handled = true;
                        }
                    }

                    // 3. Drawing new annotation (or marquee selection if Shift held)
                    if !handled {
                        if shift_held {
                            app.marquee = Some(Draft {
                                start: pointer,
                                current: pointer,
                            });
                        } else {
                            app.selected.clear();
                            app.editing_label = None;
                            app.draft = Some(Draft {
                                start: pointer,
                                current: pointer,
                            });
                        }
                    }
                }
            }

            if response.dragged_by(PointerButton::Primary) && !space_held {
                if let Some(pointer) = response.interact_pointer_pos() {
                    if let Some(active_drag) = &app.active_drag {
                        let delta_screen = pointer - active_drag_start_pointer(active_drag);
                        let delta_x = delta_screen.x / image_rect.width() * image_size.x;
                        let delta_y = delta_screen.y / image_rect.height() * image_size.y;

                        match active_drag {
                            ActiveDrag::Move {
                                initial_positions,
                                ..
                            } => {
                                let mut min_dx = -f32::INFINITY;
                                let mut max_dx = f32::INFINITY;
                                let mut min_dy = -f32::INFINITY;
                                let mut max_dy = f32::INFINITY;

                                for &(id, init_x, init_y) in initial_positions {
                                    if let Some(a) = app.annotations.iter().find(|a| a.id == id) {
                                        min_dx = min_dx.max(-init_x);
                                        max_dx = max_dx.min(image_size.x - (init_x + a.width));
                                        min_dy = min_dy.max(-init_y);
                                        max_dy = max_dy.min(image_size.y - (init_y + a.height));
                                    }
                                }

                                let clamped_dx = if min_dx <= max_dx {
                                    delta_x.clamp(min_dx, max_dx)
                                } else {
                                    0.0
                                };
                                let clamped_dy = if min_dy <= max_dy {
                                    delta_y.clamp(min_dy, max_dy)
                                } else {
                                    0.0
                                };

                                for &(id, init_x, init_y) in initial_positions {
                                    if let Some(a) = app.annotations.iter_mut().find(|a| a.id == id) {
                                        a.x = (init_x + clamped_dx).round();
                                        a.y = (init_y + clamped_dy).round();
                                    }
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
                    } else if let Some(marquee) = &mut app.marquee {
                        marquee.current = image_rect.clamp(pointer);
                    }
                }
            }

            if response.drag_stopped_by(PointerButton::Primary) {
                if app.active_drag.is_some() {
                    app.active_drag = None;
                    app.history.commit_edit(&app.current_snapshot());
                }
                if let Some(draft) = app.draft.take() {
                    let rect = Rect::from_two_pos(draft.start, draft.current);

                    if rect.width() >= 8.0 && rect.height() >= 8.0 {
                        app.history.record(app.current_snapshot());

                        let min = screen_to_image(rect.min, image_rect, image_size);
                        let max = screen_to_image(rect.max, image_rect, image_size);

                        let id = app.next_id;
                        app.next_id += 1;

                        app.annotations.push(Annotation {
                            id,
                            label: format!("object_{id:02}"),
                            description: None,
                            x: min.x.round(),
                            y: min.y.round(),
                            width: (max.x - min.x).round(),
                            height: (max.y - min.y).round(),
                            color: [255, 0, 0],
                            parent_id: None,
                        });

                        app.select_single(id);
                        app.editing_label = Some(id);
                        app.request_label_focus = true;
                        app.status = format!("REGION {id:02} CREATED");
                    }
                }
                if let Some(marquee) = app.marquee.take() {
                    let screen_rect = Rect::from_two_pos(marquee.start, marquee.current);
                    if screen_rect.width() >= 3.0 || screen_rect.height() >= 3.0 {
                        for a in &app.annotations {
                            let a_rect = annotation_screen_rect(a, image_rect, image_size);
                            if screen_rect.intersects(a_rect) || screen_rect.contains_rect(a_rect) {
                                app.selected.insert(a.id);
                            }
                        }
                        if !app.selected.is_empty() {
                            app.status = format!("{} REGION(S) SELECTED", app.selected.len());
                        }
                    }
                }
                update_hierarchy(&mut app.annotations);
            }

            if response.clicked() && !response.double_clicked() {
                if let Some(pointer) = response.interact_pointer_pos() {
                    let shift_held = ctx.input(|i| i.modifiers.shift || i.modifiers.command || i.modifiers.ctrl);
                    let hit = hit_annotation(&app.annotations, image_rect, image_size, pointer);

                    if let Some(annotation) = hit {
                        let hit_id = annotation.id;
                        if shift_held {
                            app.toggle_select(hit_id);
                        } else {
                            app.select_single(hit_id);
                        }
                    } else if !shift_held {
                        app.deselect_all();
                    }
                }
            }

            let editing_id = app.editing_label;
            let selected_ids = app.selected.clone();
            let mut close_editing = false;

            for annotation in &mut app.annotations {
                let rect = annotation_screen_rect(annotation, image_rect, image_size);
                let is_editing = editing_id == Some(annotation.id);

                draw_surveillance_box(
                    &painter,
                    rect,
                    &annotation.label,
                    annotation.color32(),
                    selected_ids.contains(&annotation.id),
                );

                if is_editing {
                    let tag_height = 20.0;
                    let edit_width = 140.0_f32.max(rect.width());
                    let edit_rect = Rect::from_min_size(
                        Pos2::new(rect.left(), (rect.top() - tag_height).max(painter.clip_rect().top())),
                        Vec2::new(edit_width, tag_height),
                    );

                    let edit = ui.put(
                        edit_rect,
                        egui::TextEdit::singleline(&mut annotation.label)
                            .font(FontId::monospace(10.0))
                            .desired_width(edit_rect.width())
                            .text_color(Color32::WHITE),
                    );

                    if app.request_label_focus {
                        edit.request_focus();
                        app.request_label_focus = false;
                    }

                    if edit.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        close_editing = true;
                    }
                }
            }

            if close_editing {
                app.editing_label = None;
                app.history.commit_edit(&app.current_snapshot());
            }

            if let Some(draft) = &app.draft {
                draw_surveillance_box(
                    &painter,
                    Rect::from_two_pos(draft.start, draft.current),
                    "NEW REGION",
                    RED,
                    true,
                );
            }

            if let Some(marquee) = &app.marquee {
                let m_rect = Rect::from_two_pos(marquee.start, marquee.current);
                painter.rect_filled(
                    m_rect,
                    0.0,
                    Color32::from_rgba_unmultiplied(41, 121, 255, 30),
                );
                painter.rect_stroke(
                    m_rect,
                    0.0,
                    Stroke::new(1.0_f32, Color32::from_rgb(41, 121, 255)),
                );
            }

            let zoom_label = format!("ZOOM {:>3.0}%", app.zoom * 100.0);
            let zoom_galley = painter.layout_no_wrap(
                zoom_label,
                FontId::monospace(9.0),
                Color32::from_gray(180),
            );
            let zoom_rect = Rect::from_min_size(
                canvas.left_bottom() - Vec2::new(0.0, zoom_galley.size().y + 12.0),
                zoom_galley.size() + Vec2::new(12.0, 8.0),
            );
            painter.rect_filled(zoom_rect, 2.0, Color32::from_black_alpha(190));
            painter.galley(
                zoom_rect.min + Vec2::new(6.0, 4.0),
                zoom_galley,
                Color32::WHITE,
            );
        });
}

fn active_drag_start_pointer(drag: &ActiveDrag) -> Pos2 {
    match drag {
        ActiveDrag::Move { start_pointer, .. } => *start_pointer,
        ActiveDrag::Resize { start_pointer, .. } => *start_pointer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_annotation(id: u32, x: f32, y: f32, width: f32, height: f32) -> Annotation {
        Annotation {
            id,
            label: format!("region_{id}"),
            description: None,
            x,
            y,
            width,
            height,
            color: [255, 0, 0],
            parent_id: None,
        }
    }

    #[test]
    fn test_hit_annotation_prioritizes_tag_inside_container_box() {
        let image_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 1000.0));
        let image_size = Vec2::new(1000.0, 1000.0);

        // Parent container (id 1) covering (0,0) to (500,500)
        let parent = sample_annotation(1, 0.0, 0.0, 500.0, 500.0);
        // Child box (id 2) covering (100, 100) to (200, 200)
        let child = sample_annotation(2, 100.0, 100.0, 100.0, 100.0);

        let annotations = vec![parent, child];

        // Child tag is around (100.0, 84.0) to (160.0, 100.0), which falls inside parent (0..500, 0..500)
        let child_tag = annotation_tag_rect(&annotations[1], image_rect, image_size);
        let pointer_on_tag = child_tag.center();

        let hit = hit_annotation(&annotations, image_rect, image_size, pointer_on_tag);
        assert_eq!(hit.map(|a| a.id), Some(2));
    }

    #[test]
    fn test_hit_annotation_prioritizes_innermost_child_box() {
        let image_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 1000.0));
        let image_size = Vec2::new(1000.0, 1000.0);

        // Parent container (id 1) covering (0,0) to (500,500), area = 250,000
        let parent = sample_annotation(1, 0.0, 0.0, 500.0, 500.0);
        // Child box (id 2) covering (100, 100) to (200, 200), area = 10,000
        let child = sample_annotation(2, 100.0, 100.0, 100.0, 100.0);

        let annotations = vec![parent, child];

        // Click inside child body at (150, 150)
        let hit = hit_annotation(&annotations, image_rect, image_size, Pos2::new(150.0, 150.0));
        assert_eq!(hit.map(|a| a.id), Some(2));

        // Click inside parent body but outside child at (50, 50)
        let hit_parent = hit_annotation(&annotations, image_rect, image_size, Pos2::new(50.0, 50.0));
        assert_eq!(hit_parent.map(|a| a.id), Some(1));
    }
}
