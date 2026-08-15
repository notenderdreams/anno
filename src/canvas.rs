use std::collections::HashSet;

use eframe::egui::{
    self, Align2, Color32, CursorIcon, FontId, Key, Margin, PointerButton, Pos2, Rect, RichText, Sense, Stroke, Vec2,
};

use crate::app::AnnotatorApp;
use crate::geometry::{
    annotation_screen_rect, annotation_tag_rect, hit_polygon_edge_with_projection,
    hit_polygon_vertex, image_to_screen, point_in_polygon, screen_to_image, update_hierarchy,
};
use crate::models::{
    match_class_presets, next_category_label, next_category_label_from_labels, ActiveDrag,
    Annotation, Draft, DraftPolygon, ResizeHandle, ToolMode,
};
use crate::render::{draw_draft_polygon, draw_polygon_annotation, draw_surveillance_box};
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

    // 2. If no tag hit, check hit inside annotation shape:
    // If polygon, test polygon vertices in screen space; else check bounding box.
    annotations
        .iter()
        .enumerate()
        .filter(|(_, a)| {
            if let Some(points) = &a.points {
                let screen_pts: Vec<Pos2> = points
                    .iter()
                    .map(|&[px, py]| image_to_screen(Pos2::new(px, py), image_rect, image_size))
                    .collect();
                point_in_polygon(pointer, &screen_pts)
            } else {
                annotation_screen_rect(a, image_rect, image_size).contains(pointer)
            }
        })
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

            let (texture_id, image_size) = match &app.image {
                Some(image) => (
                    image.texture.id(),
                    Vec2::new(image.width as f32, image.height as f32),
                ),
                None => {
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
                }
            };

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
                texture_id,
                image_rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );

            let show_minimap = app.zoom > 1.01;
            let (minimap_rect, minimap_img_rect) = if show_minimap {
                let max_w = 160.0_f32;
                let max_h = 105.0_f32;
                let img_aspect = (image_size.x / image_size.y).max(0.01);
                let (inner_w, inner_h) = if img_aspect >= max_w / max_h {
                    (max_w, (max_w / img_aspect).clamp(24.0, max_h))
                } else {
                    ((max_h * img_aspect).clamp(24.0, max_w), max_h)
                };
                let header_h = 15.0_f32;
                let pad = 6.0_f32;
                let outer_w = inner_w + pad * 2.0;
                let outer_h = inner_h + pad * 2.0 + header_h;
                let m_rect = Rect::from_min_size(
                    canvas.right_bottom() - Vec2::new(outer_w + 14.0, outer_h + 14.0),
                    Vec2::new(outer_w, outer_h),
                );
                let m_img_rect = Rect::from_min_size(
                    m_rect.min + Vec2::new(pad, pad + header_h),
                    Vec2::new(inner_w, inner_h),
                );
                (Some(m_rect), Some(m_img_rect))
            } else {
                (None, None)
            };

            let mut hovered_polygon_vertex: Option<(u32, usize)> = None;
            let mut hovered_edge_hint: Option<(u32, Pos2)> = None;

            if let Some(ActiveDrag::MoveVertex { id, vertex_idx, .. }) = &app.active_drag {
                hovered_polygon_vertex = Some((*id, *vertex_idx));
                ctx.set_cursor_icon(CursorIcon::Grabbing);
            } else if !is_panning && !space_held {
                if let Some(pointer) = response.hover_pos() {
                    if minimap_rect.map_or(false, |r| r.contains(pointer)) {
                        ctx.set_cursor_icon(CursorIcon::PointingHand);
                    } else if image_rect.contains(pointer) {
                        if app.tool_mode == ToolMode::Polygon {
                            if let Some(poly) = &app.draft_polygon {
                                let can_close = if let Some(first) = poly.points.first() {
                                    let first_screen = image_to_screen(*first, image_rect, image_size);
                                    poly.points.len() >= 3 && first_screen.distance(pointer) <= 16.0
                                } else {
                                    false
                                };
                                if can_close {
                                    ctx.set_cursor_icon(CursorIcon::PointingHand);
                                } else {
                                    ctx.set_cursor_icon(CursorIcon::Crosshair);
                                }
                            } else {
                                ctx.set_cursor_icon(CursorIcon::Crosshair);
                            }
                        } else if app.tool_mode == ToolMode::Rectangle {
                            ctx.set_cursor_icon(CursorIcon::Crosshair);
                        } else {
                            // ToolMode::Select
                            let mut cursor_set = false;

                            // 1. Check vertex handles on selected UNLOCKED polygon annotations (threshold: 12px)
                            for &selected_id in &app.selected {
                                if let Some(annotation) = app.annotations.iter().find(|a| a.id == selected_id && !a.locked) {
                                    if let Some(points) = &annotation.points {
                                        if let Some(vertex_idx) = hit_polygon_vertex(points, image_rect, image_size, pointer, 12.0) {
                                            hovered_polygon_vertex = Some((selected_id, vertex_idx));
                                            ctx.set_cursor_icon(CursorIcon::PointingHand);
                                            cursor_set = true;
                                            break;
                                        }
                                    }
                                }
                            }

                            // If not on selected polygon, check any unlocked polygon's vertices
                            if !cursor_set {
                                for annotation in app.annotations.iter().rev() {
                                    if !annotation.locked {
                                        if let Some(points) = &annotation.points {
                                            if let Some(vertex_idx) = hit_polygon_vertex(points, image_rect, image_size, pointer, 12.0) {
                                                hovered_polygon_vertex = Some((annotation.id, vertex_idx));
                                                ctx.set_cursor_icon(CursorIcon::PointingHand);
                                                cursor_set = true;
                                                break;
                                            }
                                        }
                                    }
                                }
                            }

                            // 2. Check resize handles on selected annotations (boxes only)
                            if !cursor_set {
                                for &selected_id in &app.selected {
                                    if let Some(annotation) = app.annotations.iter().find(|a| a.id == selected_id && !a.locked) {
                                        if annotation.points.is_none() {
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
                                }
                            }

                            // 3. Check edge insert hover hint on selected unlocked polygon annotations
                            if !cursor_set {
                                for &selected_id in &app.selected {
                                    if let Some(annotation) = app.annotations.iter().find(|a| a.id == selected_id && !a.locked) {
                                        if let Some(points) = &annotation.points {
                                            if let Some((_edge_idx, proj)) = hit_polygon_edge_with_projection(points, image_rect, image_size, pointer, 10.0) {
                                                hovered_edge_hint = Some((selected_id, proj));
                                                ctx.set_cursor_icon(CursorIcon::Crosshair);
                                                cursor_set = true;
                                                break;
                                            }
                                        }
                                    }
                                }
                            }

                            // 4. Check tag or selected body
                            if !cursor_set {
                                let over_tag = app.annotations.iter().any(|a| {
                                    !a.locked && annotation_tag_rect(a, image_rect, image_size).contains(pointer)
                                });
                                let over_selected_body = app.annotations.iter().any(|a| {
                                    !a.locked
                                        && app.selected.contains(&a.id)
                                        && if let Some(points) = &a.points {
                                            let screen_pts: Vec<Pos2> = points
                                                .iter()
                                                .map(|&[px, py]| image_to_screen(Pos2::new(px, py), image_rect, image_size))
                                                .collect();
                                            point_in_polygon(pointer, &screen_pts)
                                        } else {
                                            annotation_screen_rect(a, image_rect, image_size).contains(pointer)
                                        }
                                });

                                if over_tag || over_selected_body {
                                    ctx.set_cursor_icon(CursorIcon::Move);
                                } else {
                                    ctx.set_cursor_icon(CursorIcon::Default);
                                }
                            }
                        }
                    }
                }
            }

            if response.secondary_clicked() {
                if let Some(poly) = &mut app.draft_polygon {
                    poly.undo_point();
                    if poly.points.is_empty() {
                        app.draft_polygon = None;
                        app.status = "PEN TOOL DRAWING CANCELED".into();
                    } else {
                        app.status = format!("PEN TOOL: POINT REMOVED ({} REMAINING)", poly.points.len());
                    }
                } else if app.tool_mode == ToolMode::Select {
                    if let Some(pointer) = response.interact_pointer_pos() {
                        if image_rect.contains(pointer) {
                            let mut delete_target = None;
                            for &selected_id in &app.selected {
                                if let Some(annotation) = app.annotations.iter().find(|a| a.id == selected_id && !a.locked) {
                                    if let Some(points) = &annotation.points {
                                        if let Some(vertex_idx) = hit_polygon_vertex(points, image_rect, image_size, pointer, 12.0) {
                                            delete_target = Some((selected_id, vertex_idx, points.len()));
                                            break;
                                        }
                                    }
                                }
                            }

                            if let Some((id, vertex_idx, count)) = delete_target {
                                if count > 3 {
                                    app.history.record(app.current_snapshot());
                                    if let Some(annotation) = app.annotations.iter_mut().find(|a| a.id == id) {
                                        if let Some(points) = &mut annotation.points {
                                            points.remove(vertex_idx);
                                            let poly_pos: Vec<Pos2> = points.iter().map(|p| Pos2::new(p[0], p[1])).collect();
                                            let (x, y, w, h) = crate::geometry::polygon_bounding_box(&poly_pos);
                                            annotation.x = x.round();
                                            annotation.y = y.round();
                                            annotation.width = w.round();
                                            annotation.height = h.round();
                                            app.status = format!("POLYGON VERTEX REMOVED ({} REMAINING)", points.len());
                                        }
                                    }
                                    app.selected_vertex = None;
                                    update_hierarchy(&mut app.annotations);
                                } else {
                                    app.status = "POLYGON REQUIRES AT LEAST 3 VERTICES".into();
                                }
                            }
                        }
                    }
                }
            }

            if response.double_clicked() {
                if app.tool_mode == ToolMode::Polygon && app.draft_polygon.as_ref().map_or(false, |p| p.points.len() >= 3) {
                    app.finish_draft_polygon();
                } else if app.tool_mode == ToolMode::Select {
                    if let Some(pointer) = response.interact_pointer_pos() {
                        if !minimap_rect.map_or(false, |r| r.contains(pointer)) && image_rect.contains(pointer) {
                            let mut insert_target = None;
                            for &selected_id in &app.selected {
                                if let Some(annotation) = app.annotations.iter().find(|a| a.id == selected_id && !a.locked) {
                                    if let Some(points) = &annotation.points {
                                        if hit_polygon_vertex(points, image_rect, image_size, pointer, 12.0).is_none() {
                                            if let Some((edge_idx, proj)) = hit_polygon_edge_with_projection(points, image_rect, image_size, pointer, 10.0) {
                                                insert_target = Some((selected_id, edge_idx, proj));
                                                break;
                                            }
                                        }
                                    }
                                }
                            }

                            if let Some((id, edge_idx, proj_pos)) = insert_target {
                                app.history.record(app.current_snapshot());
                                let img_pos = screen_to_image(proj_pos, image_rect, image_size);
                                if let Some(annotation) = app.annotations.iter_mut().find(|a| a.id == id) {
                                    if let Some(points) = &mut annotation.points {
                                        let new_idx = edge_idx + 1;
                                        points.insert(new_idx, [img_pos.x.round(), img_pos.y.round()]);
                                        let poly_pos: Vec<Pos2> = points.iter().map(|p| Pos2::new(p[0], p[1])).collect();
                                        let (x, y, w, h) = crate::geometry::polygon_bounding_box(&poly_pos);
                                        annotation.x = x.round();
                                        annotation.y = y.round();
                                        annotation.width = w.round();
                                        annotation.height = h.round();
                                        app.status = format!("POLYGON VERTEX INSERTED ({} TOTAL)", points.len());
                                        app.selected_vertex = Some((id, new_idx));
                                    }
                                }
                                update_hierarchy(&mut app.annotations);
                            } else if let Some(hit) = hit_annotation(&app.annotations, image_rect, image_size, pointer) {
                                let hit_id = hit.id;
                                app.history.begin_edit(app.current_snapshot());
                                app.select_single(hit_id);
                                app.editing_label = Some(hit_id);
                                app.request_label_focus = true;
                            }
                        }
                    }
                }
            } else if response.drag_started_by(PointerButton::Primary) && !space_held {
                if let Some(pointer) = response.interact_pointer_pos() {
                    if minimap_rect.map_or(false, |r| r.contains(pointer)) {
                        if let Some(m_img_rect) = minimap_img_rect {
                            app.pan = pan_from_minimap_click(pointer, m_img_rect, display_size, canvas.size());
                            app.active_drag = Some(ActiveDrag::MinimapPan {
                                start_pointer: pointer,
                            });
                        }
                    } else if image_rect.contains(pointer) {
                        let shift_held = ctx.input(|i| i.modifiers.shift || i.modifiers.command || i.modifiers.ctrl);
                        let mut handled = false;

                        if app.tool_mode == ToolMode::Select {
                            // 1. Check vertex handles on selected UNLOCKED polygon annotations (or any unlocked polygon if clicking on vertex)
                            let mut vertex_drag = None;
                            for &selected_id in &app.selected {
                                if let Some(annotation) = app.annotations.iter().find(|a| a.id == selected_id && !a.locked) {
                                    if let Some(points) = &annotation.points {
                                        if let Some(vertex_idx) = hit_polygon_vertex(points, image_rect, image_size, pointer, 12.0) {
                                            vertex_drag = Some((selected_id, vertex_idx, points[vertex_idx]));
                                            break;
                                        }
                                    }
                                }
                            }

                            if vertex_drag.is_none() {
                                for annotation in app.annotations.iter().rev() {
                                    if !annotation.locked {
                                        if let Some(points) = &annotation.points {
                                            if let Some(vertex_idx) = hit_polygon_vertex(points, image_rect, image_size, pointer, 12.0) {
                                                vertex_drag = Some((annotation.id, vertex_idx, points[vertex_idx]));
                                                if !app.selected.contains(&annotation.id) {
                                                    if shift_held {
                                                        app.selected.insert(annotation.id);
                                                    } else {
                                                        app.select_single(annotation.id);
                                                    }
                                                }
                                                break;
                                            }
                                        }
                                    }
                                }
                            }

                            if let Some((id, vertex_idx, initial_point)) = vertex_drag {
                                app.selected_vertex = Some((id, vertex_idx));
                                app.history.begin_edit(app.current_snapshot());
                                app.active_drag = Some(ActiveDrag::MoveVertex {
                                    id,
                                    vertex_idx,
                                    start_pointer: pointer,
                                    initial_point,
                                });
                                handled = true;
                            }

                            // 2. Check resize handle on selected UNLOCKED box annotations
                            if !handled {
                                for &selected_id in &app.selected {
                                    if let Some(annotation) = app.annotations.iter().find(|a| a.id == selected_id && !a.locked) {
                                        if annotation.points.is_none() {
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
                                                    initial_points: annotation.points.clone(),
                                                });
                                                handled = true;
                                                break;
                                            }
                                        }
                                    }
                                }
                            }

                            // 3. Check hit on annotation tag or body to move
                            if !handled {
                                let hit_tag = app
                                    .annotations
                                    .iter()
                                    .rev()
                                    .find(|a| annotation_tag_rect(a, image_rect, image_size).contains(pointer));

                                let hit_body = app
                                    .annotations
                                    .iter()
                                    .rev()
                                    .find(|a| {
                                        if let Some(points) = &a.points {
                                            let screen_pts: Vec<Pos2> = points
                                                .iter()
                                                .map(|&[px, py]| image_to_screen(Pos2::new(px, py), image_rect, image_size))
                                                .collect();
                                            point_in_polygon(pointer, &screen_pts)
                                        } else {
                                            annotation_screen_rect(a, image_rect, image_size).contains(pointer)
                                        }
                                    });

                                if let Some(hit) = hit_tag.or(hit_body) {
                                    let id = hit.id;
                                    if shift_held {
                                        app.selected.insert(id);
                                    } else if !app.selected.contains(&id) {
                                        app.select_single(id);
                                    }

                                    let initial_positions: Vec<(u32, f32, f32, Option<Vec<[f32; 2]>>)> = app
                                        .annotations
                                        .iter()
                                        .filter(|a| app.selected.contains(&a.id) && !a.locked)
                                        .map(|a| (a.id, a.x, a.y, a.points.clone()))
                                        .collect();

                                    if !initial_positions.is_empty() {
                                        app.history.begin_edit(app.current_snapshot());
                                        app.active_drag = Some(ActiveDrag::Move {
                                            initial_positions,
                                            start_pointer: pointer,
                                        });
                                    }
                                    handled = true;
                                }
                            }

                            // 4. Marquee selection drag on empty space in Select mode
                            if !handled {
                                if !shift_held {
                                    app.selected.clear();
                                    app.editing_label = None;
                                }
                                app.marquee = Some(Draft {
                                    start: pointer,
                                    current: pointer,
                                });
                            }
                        } else if app.tool_mode == ToolMode::Rectangle {
                            // Drawing new box in Box mode
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
                            ActiveDrag::MinimapPan { .. } => {
                                if let Some(m_img_rect) = minimap_img_rect {
                                    app.pan = pan_from_minimap_click(pointer, m_img_rect, display_size, canvas.size());
                                }
                            }
                            ActiveDrag::Move {
                                initial_positions,
                                ..
                            } => {
                                let mut min_dx = -f32::INFINITY;
                                let mut max_dx = f32::INFINITY;
                                let mut min_dy = -f32::INFINITY;
                                let mut max_dy = f32::INFINITY;

                                for (id, init_x, init_y, init_pts) in initial_positions {
                                    if let Some(a) = app.annotations.iter().find(|a| a.id == *id) {
                                        min_dx = min_dx.max(-*init_x);
                                        max_dx = max_dx.min(image_size.x - (*init_x + a.width));
                                        min_dy = min_dy.max(-*init_y);
                                        max_dy = max_dy.min(image_size.y - (*init_y + a.height));
                                        if let Some(pts) = init_pts {
                                            for &[px, py] in pts {
                                                min_dx = min_dx.max(-px);
                                                max_dx = max_dx.min(image_size.x - px);
                                                min_dy = min_dy.max(-py);
                                                max_dy = max_dy.min(image_size.y - py);
                                            }
                                        }
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

                                for (id, init_x, init_y, init_pts) in initial_positions {
                                    if let Some(annotation) = app.annotations.iter_mut().find(|a| a.id == *id) {
                                        annotation.x = (*init_x + clamped_dx).round();
                                        annotation.y = (*init_y + clamped_dy).round();
                                        if let Some(pts) = init_pts {
                                            let moved_pts: Vec<[f32; 2]> = pts
                                                .iter()
                                                .map(|&[px, py]| [(px + clamped_dx).round(), (py + clamped_dy).round()])
                                                .collect();
                                            annotation.points = Some(moved_pts);
                                        }
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
                                initial_points,
                                ..
                            } => {
                                if let Some(annotation) = app.annotations.iter_mut().find(|a| a.id == *id) {
                                    match handle {
                                        ResizeHandle::TopLeft => {
                                            let max_x = initial_x + initial_w - 8.0;
                                            let max_y = initial_y + initial_h - 8.0;
                                            let new_x = (initial_x + delta_x).clamp(0.0, max_x);
                                            let new_y = (initial_y + delta_y).clamp(0.0, max_y);
                                            annotation.width = (initial_x + initial_w - new_x).round();
                                            annotation.height = (initial_y + initial_h - new_y).round();
                                            annotation.x = new_x.round();
                                            annotation.y = new_y.round();
                                        }
                                        ResizeHandle::TopRight => {
                                            let max_w = (initial_x + initial_w + delta_x).clamp(initial_x + 8.0, image_size.x);
                                            let max_y = initial_y + initial_h - 8.0;
                                            let new_y = (initial_y + delta_y).clamp(0.0, max_y);
                                            annotation.width = (max_w - initial_x).round();
                                            annotation.height = (initial_y + initial_h - new_y).round();
                                            annotation.y = new_y.round();
                                        }
                                        ResizeHandle::BottomLeft => {
                                            let max_x = initial_x + initial_w - 8.0;
                                            let new_x = (initial_x + delta_x).clamp(0.0, max_x);
                                            let max_h = (initial_y + initial_h + delta_y).clamp(initial_y + 8.0, image_size.y);
                                            annotation.width = (initial_x + initial_w - new_x).round();
                                            annotation.height = (max_h - initial_y).round();
                                            annotation.x = new_x.round();
                                        }
                                        ResizeHandle::BottomRight => {
                                            let max_x = (initial_x + initial_w + delta_x).clamp(initial_x + 8.0, image_size.x);
                                            let max_y = (initial_y + initial_h + delta_y).clamp(initial_y + 8.0, image_size.y);
                                            annotation.width = (max_x - initial_x).round();
                                            annotation.height = (max_y - initial_y).round();
                                        }
                                    }
                                    if let Some(init_pts) = initial_points {
                                        if *initial_w > 0.0 && *initial_h > 0.0 {
                                            let scaled: Vec<[f32; 2]> = init_pts
                                                .iter()
                                                .map(|&[px, py]| {
                                                    let norm_x = (px - initial_x) / initial_w;
                                                    let norm_y = (py - initial_y) / initial_h;
                                                    [
                                                        (annotation.x + norm_x * annotation.width).round(),
                                                        (annotation.y + norm_y * annotation.height).round(),
                                                    ]
                                                })
                                                .collect();
                                            annotation.points = Some(scaled);
                                        }
                                    }
                                }
                            }
                            ActiveDrag::MoveVertex {
                                id,
                                vertex_idx,
                                initial_point,
                                ..
                            } => {
                                let shift_held = ctx.input(|i| i.modifiers.shift);
                                let (eff_dx, eff_dy) = if shift_held {
                                    if delta_x.abs() > delta_y.abs() {
                                        (delta_x, 0.0)
                                    } else {
                                        (0.0, delta_y)
                                    }
                                } else {
                                    (delta_x, delta_y)
                                };

                                if let Some(annotation) = app.annotations.iter_mut().find(|a| a.id == *id) {
                                    if let Some(points) = &mut annotation.points {
                                        if *vertex_idx < points.len() {
                                            let new_x = (initial_point[0] + eff_dx).clamp(0.0, image_size.x).round();
                                            let new_y = (initial_point[1] + eff_dy).clamp(0.0, image_size.y).round();
                                            points[*vertex_idx] = [new_x, new_y];

                                            let poly_pos: Vec<Pos2> = points.iter().map(|p| Pos2::new(p[0], p[1])).collect();
                                            let (bb_x, bb_y, bb_w, bb_h) = crate::geometry::polygon_bounding_box(&poly_pos);
                                            annotation.x = bb_x.round();
                                            annotation.y = bb_y.round();
                                            annotation.width = bb_w.round();
                                            annotation.height = bb_h.round();
                                            app.status = format!("MOVING VERTEX #{}/{}: X: {:.0}, Y: {:.0}", vertex_idx + 1, points.len(), new_x, new_y);
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
                if let Some(active_drag) = app.active_drag.take() {
                    if !matches!(active_drag, ActiveDrag::MinimapPan { .. }) {
                        app.history.commit_edit(&app.current_snapshot());
                        if let ActiveDrag::MoveVertex { vertex_idx, .. } = active_drag {
                            app.status = format!("VERTEX #{} MOVED", vertex_idx + 1);
                        }
                    }
                }
                if let Some(draft) = app.draft.take() {
                    let rect = Rect::from_two_pos(draft.start, draft.current);

                    if rect.width() >= 8.0 && rect.height() >= 8.0 {
                        app.history.record(app.current_snapshot());

                        let min = screen_to_image(rect.min, image_rect, image_size);
                        let max = screen_to_image(rect.max, image_rect, image_size);

                        let id = app.next_id;
                        app.next_id += 1;

                        let (prefix, color) = if let Some(preset) = app.presets.get(app.active_preset_idx) {
                            (preset.prefix.clone(), preset.color)
                        } else {
                            ("object".to_string(), [255, 0, 0])
                        };

                        let auto_label = next_category_label(&prefix, &app.annotations, None);

                        app.annotations.push(Annotation {
                            id,
                            label: auto_label,
                            description: None,
                            x: min.x.round(),
                            y: min.y.round(),
                            width: (max.x - min.x).round(),
                            height: (max.y - min.y).round(),
                            color,
                            parent_id: None,
                            locked: false,
                            points: None,
                        });

                        app.select_single(id);
                        app.editing_label = None;
                        app.request_label_focus = false;
                        app.status = format!("BOX REGION {:02} CREATED", id);
                    }
                }
                if let Some(marquee) = app.marquee.take() {
                    let rect = Rect::from_two_pos(marquee.start, marquee.current);
                    if rect.width() >= 4.0 || rect.height() >= 4.0 {
                        let shift_held = ctx.input(|i| i.modifiers.shift || i.modifiers.command || i.modifiers.ctrl);
                        let mut newly_selected = HashSet::new();

                        for annotation in &app.annotations {
                            let anno_rect = annotation_screen_rect(annotation, image_rect, image_size);
                            if rect.intersects(anno_rect) {
                                newly_selected.insert(annotation.id);
                            }
                        }

                        if shift_held {
                            app.selected.extend(newly_selected);
                        } else {
                            app.selected = newly_selected;
                        }

                        if !app.selected.is_empty() {
                            app.status = format!("{} REGIONS SELECTED VIA MARQUEE", app.selected.len());
                        }
                    }
                }
                update_hierarchy(&mut app.annotations);
            }

            if response.clicked() && !response.double_clicked() {
                if let Some(pointer) = response.interact_pointer_pos() {
                    if minimap_rect.map_or(false, |r| r.contains(pointer)) {
                        if let Some(m_img_rect) = minimap_img_rect {
                            app.pan = pan_from_minimap_click(pointer, m_img_rect, display_size, canvas.size());
                        }
                    } else if image_rect.contains(pointer) {
                        if app.tool_mode == ToolMode::Polygon {
                            let img_pos = screen_to_image(pointer, image_rect, image_size);

                            if app.draft_polygon.is_none() {
                                app.selected.clear();
                                app.editing_label = None;
                                app.draft_polygon = Some(DraftPolygon::new(img_pos));
                                app.status = "PEN TOOL: 1 POINT PLACED  •  CLICK TO ADD MORE (RETURN TO START TO CLOSE)".into();
                            } else if let Some(poly) = &mut app.draft_polygon {
                                let start_screen = image_to_screen(poly.points[0], image_rect, image_size);
                                if poly.points.len() >= 3 && start_screen.distance(pointer) <= 16.0 {
                                    app.finish_draft_polygon();
                                } else {
                                    poly.add_point(img_pos);
                                    app.status = format!(
                                        "PEN TOOL: {} POINTS PLACED  •  CLICK START POINT OR PRESS ENTER TO CLOSE",
                                        poly.points.len()
                                    );
                                }
                            }
                        } else if app.tool_mode == ToolMode::Select {
                            let shift_held = ctx.input(|i| i.modifiers.shift || i.modifiers.command || i.modifiers.ctrl);

                            // 1. Check if clicking on a vertex of a selected unlocked polygon
                            let mut clicked_vertex = None;
                            for &selected_id in &app.selected {
                                if let Some(anno) = app.annotations.iter().find(|a| a.id == selected_id && !a.locked) {
                                    if let Some(points) = &anno.points {
                                        if let Some(v_idx) = hit_polygon_vertex(points, image_rect, image_size, pointer, 12.0) {
                                            clicked_vertex = Some((selected_id, v_idx, points.len()));
                                            break;
                                        }
                                    }
                                }
                            }

                            if let Some((id, v_idx, count)) = clicked_vertex {
                                app.selected_vertex = Some((id, v_idx));
                                app.status = format!("VERTEX #{}/{} SELECTED (ARROW KEYS TO NUDGE, DEL TO REMOVE)", v_idx + 1, count);
                            } else {
                                app.selected_vertex = None;
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
                    }
                }
            }

            let editing_id = app.editing_label;
            let selected_ids = app.selected.clone();
            let other_labels: Vec<(u32, String)> = app.annotations.iter().map(|a| (a.id, a.label.clone())).collect();
            let mut close_editing = false;

            for annotation in &mut app.annotations {
                let rect = annotation_screen_rect(annotation, image_rect, image_size);
                let is_editing = editing_id == Some(annotation.id);

                if let Some(points) = &annotation.points {
                    let screen_pts: Vec<Pos2> = points
                        .iter()
                        .map(|&[px, py]| image_to_screen(Pos2::new(px, py), image_rect, image_size))
                        .collect();
                    let h_vertex = if hovered_polygon_vertex.map_or(false, |(id, _)| id == annotation.id) {
                        hovered_polygon_vertex.map(|(_, idx)| idx)
                    } else {
                        None
                    };
                    let sel_vertex = if app.selected_vertex.map_or(false, |(id, _)| id == annotation.id) {
                        app.selected_vertex.map(|(_, idx)| idx)
                    } else {
                        None
                    };
                    let edge_hint = if hovered_edge_hint.map_or(false, |(id, _)| id == annotation.id) {
                        hovered_edge_hint.map(|(_, proj)| proj)
                    } else {
                        None
                    };
                    draw_polygon_annotation(
                        &painter,
                        &screen_pts,
                        rect,
                        &annotation.label,
                        annotation.color32(),
                        selected_ids.contains(&annotation.id),
                        annotation.locked,
                        sel_vertex,
                        h_vertex,
                        edge_hint,
                    );
                } else {
                    draw_surveillance_box(
                        &painter,
                        rect,
                        &annotation.label,
                        annotation.color32(),
                        selected_ids.contains(&annotation.id),
                        annotation.locked,
                    );
                }

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

                    let suggestions = match_class_presets(&annotation.label, &app.presets);
                    let mut canvas_applied = None;
                    let show_canvas_autocomplete = (edit.has_focus() || edit.lost_focus()) && !suggestions.is_empty();

                    if show_canvas_autocomplete {
                        let max_count = suggestions.len().min(4);

                        if ui.input(|i| i.key_pressed(Key::ArrowDown)) {
                            app.autocomplete_nav = Some(match app.autocomplete_nav {
                                Some(curr) => (curr + 1) % max_count,
                                None => 0,
                            });
                        } else if ui.input(|i| i.key_pressed(Key::ArrowUp)) {
                            app.autocomplete_nav = Some(match app.autocomplete_nav {
                                Some(curr) => if curr == 0 { max_count - 1 } else { curr - 1 },
                                None => max_count - 1,
                            });
                        }

                        let enter_pressed = ui.input(|i| i.key_pressed(Key::Enter));
                        let tab_pressed = ui.input(|i| i.key_pressed(Key::Tab));

                        if enter_pressed || tab_pressed {
                            if let Some(selected_idx) = app.autocomplete_nav {
                                if let Some((_, preset)) = suggestions.get(selected_idx) {
                                    let tag = next_category_label_from_labels(
                                        &preset.prefix,
                                        other_labels.iter().filter(|(id, _)| *id != annotation.id).map(|(_, l)| l.as_str()),
                                    );
                                    canvas_applied = Some((preset.color, tag));
                                    app.autocomplete_nav = None;
                                }
                            } else if tab_pressed {
                                if let Some((_, preset)) = suggestions.first() {
                                    let tag = next_category_label_from_labels(
                                        &preset.prefix,
                                        other_labels.iter().filter(|(id, _)| *id != annotation.id).map(|(_, l)| l.as_str()),
                                    );
                                    canvas_applied = Some((preset.color, tag));
                                    app.autocomplete_nav = None;
                                }
                            }
                        }

                        if edit.has_focus() {
                            let popup_w = 160.0_f32.max(edit_width);
                            let popup_rect = Rect::from_min_size(
                                Pos2::new(edit_rect.left(), edit_rect.bottom() + 2.0),
                                Vec2::new(popup_w, (max_count as f32) * 20.0 + 8.0),
                            );
                            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(popup_rect), |ui| {
                                let frame = egui::Frame::none()
                                    .fill(Color32::from_black_alpha(235))
                                    .stroke(Stroke::new(1.0_f32, Color32::from_gray(65)))
                                    .rounding(2.0)
                                    .inner_margin(Margin::same(3.0));
                                frame.show(ui, |ui| {
                                    ui.spacing_mut().item_spacing.y = 2.0;
                                    for (i, (idx, preset)) in suggestions.iter().take(4).enumerate() {
                                        let is_highlighted = app.autocomplete_nav == Some(i);
                                        let tag = next_category_label_from_labels(
                                            &preset.prefix,
                                            other_labels.iter().filter(|(id, _)| *id != annotation.id).map(|(_, l)| l.as_str()),
                                        );
                                        let arrow_prefix = if is_highlighted { "▶ " } else { "  " };
                                        let btn = egui::Button::new(
                                            RichText::new(format!("{}[{}] {}", arrow_prefix, idx + 1, tag))
                                                .size(9.0)
                                                .monospace()
                                                .color(if is_highlighted { Color32::WHITE } else { Color32::from_gray(200) }),
                                        )
                                        .fill(if is_highlighted { Color32::from_gray(50) } else { Color32::from_gray(30) })
                                        .stroke(Stroke::new(if is_highlighted { 1.5_f32 } else { 1.0_f32 }, preset.color32()));
                                        let resp = ui.add_sized([ui.available_width(), 18.0], btn);
                                        if resp.hovered() {
                                            app.autocomplete_nav = Some(i);
                                        }
                                        if resp.clicked() {
                                            canvas_applied = Some((preset.color, tag));
                                            app.autocomplete_nav = None;
                                        }
                                    }
                                });
                            });
                        }
                    }

                    if let Some((col, tag)) = canvas_applied {
                        annotation.color = col;
                        annotation.label = tag;
                        close_editing = true;
                        app.autocomplete_nav = None;
                    }

                    if !edit.gained_focus() && (edit.lost_focus() || (edit.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))) {
                        close_editing = true;
                        app.autocomplete_nav = None;
                    }
                }
            }

            if close_editing {
                app.editing_label = None;
                app.history.commit_edit(&app.current_snapshot());
            }

            if let Some(draft) = &app.draft {
                let (draft_label, color) = if let Some(preset) = app.presets.get(app.active_preset_idx) {
                    let seq_label = next_category_label(&preset.prefix, &app.annotations, None);
                    (seq_label.to_uppercase(), preset.color32())
                } else {
                    (format!("REGION {:02}", app.next_id), RED)
                };
                draw_surveillance_box(
                    &painter,
                    Rect::from_two_pos(draft.start, draft.current),
                    &draft_label,
                    color,
                    true,
                    false,
                );
            }

            if let Some(poly) = &app.draft_polygon {
                let screen_points: Vec<Pos2> = poly
                    .points
                    .iter()
                    .map(|&p| image_to_screen(p, image_rect, image_size))
                    .collect();

                let hover_pos = response.hover_pos();
                let can_close = if let Some(pointer) = hover_pos {
                    screen_points.len() >= 3 && screen_points[0].distance(pointer) <= 16.0
                } else {
                    false
                };

                let (prefix, color) = if let Some(preset) = app.presets.get(app.active_preset_idx) {
                    (preset.prefix.as_str(), preset.color32())
                } else {
                    ("object", RED)
                };
                let draft_label = next_category_label(prefix, &app.annotations, None);

                draw_draft_polygon(
                    &painter,
                    &screen_points,
                    hover_pos,
                    color,
                    &draft_label,
                    can_close,
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

            if let Some(preset) = app.presets.get(app.active_preset_idx) {
                let preset_label = format!("[{}] {}", app.active_preset_idx + 1, preset.prefix.to_uppercase());
                let preset_galley = painter.layout_no_wrap(
                    preset_label,
                    FontId::monospace(9.0),
                    Color32::WHITE,
                );
                let swatch_w = 8.0_f32;
                let preset_pill_size = preset_galley.size() + Vec2::new(16.0 + swatch_w, 8.0);
                let preset_rect = Rect::from_min_size(
                    Pos2::new(zoom_rect.right() + 6.0, zoom_rect.min.y),
                    preset_pill_size,
                );
                painter.rect_filled(preset_rect, 2.0, Color32::from_black_alpha(190));
                let swatch_rect = Rect::from_center_size(
                    Pos2::new(preset_rect.left() + 9.0, preset_rect.center().y),
                    Vec2::splat(swatch_w),
                );
                painter.rect_filled(swatch_rect, 1.5, preset.color32());
                painter.galley(
                    Pos2::new(preset_rect.left() + 16.0, preset_rect.min.y + 4.0),
                    preset_galley,
                    Color32::WHITE,
                );
            }

            // Minimap Overview Overlay
            if show_minimap {
                if let (Some(m_rect), Some(m_img_rect)) = (minimap_rect, minimap_img_rect) {
                    painter.rect_filled(m_rect, 3.0, Color32::from_black_alpha(225));
                    painter.rect_stroke(m_rect, 3.0, Stroke::new(1.0_f32, Color32::from_gray(55)));

                    painter.text(
                        m_rect.min + Vec2::new(6.0, 9.0),
                        Align2::LEFT_CENTER,
                        "OVERVIEW",
                        FontId::monospace(8.0),
                        MUTED,
                    );
                    painter.text(
                        Pos2::new(m_rect.right() - 6.0, m_rect.min.y + 9.0),
                        Align2::RIGHT_CENTER,
                        format!("{:>3.0}%", app.zoom * 100.0),
                        FontId::monospace(8.0),
                        RED,
                    );

                    painter.image(
                        texture_id,
                        m_img_rect,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                    painter.rect_stroke(m_img_rect, 0.0, Stroke::new(1.0_f32, Color32::from_gray(40)));

                    for a in &app.annotations {
                        let norm_x = (a.x / image_size.x).clamp(0.0, 1.0);
                        let norm_y = (a.y / image_size.y).clamp(0.0, 1.0);
                        let norm_w = (a.width / image_size.x).clamp(0.0, 1.0);
                        let norm_h = (a.height / image_size.y).clamp(0.0, 1.0);

                        let box_min = m_img_rect.min + Vec2::new(norm_x * m_img_rect.width(), norm_y * m_img_rect.height());
                        let box_max = box_min + Vec2::new(norm_w * m_img_rect.width(), norm_h * m_img_rect.height());
                        let mini_box = Rect::from_min_max(box_min, box_max);

                        let col = a.color32();
                        painter.rect_filled(
                            mini_box,
                            0.0,
                            Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 60),
                        );
                        painter.rect_stroke(mini_box, 0.0, Stroke::new(1.0_f32, col));
                    }

                    let vp_rect = calculate_minimap_viewport(image_rect, canvas, m_img_rect);
                    painter.rect_filled(
                        vp_rect,
                        0.0,
                        Color32::from_rgba_unmultiplied(255, 59, 48, 45),
                    );
                    painter.rect_stroke(vp_rect, 0.0, Stroke::new(1.5_f32, RED));
                }
            }
        });
}

pub fn calculate_minimap_viewport(
    image_screen_rect: Rect,
    canvas_rect: Rect,
    minimap_img_rect: Rect,
) -> Rect {
    let visible = image_screen_rect.intersect(canvas_rect);
    if visible.width() <= 0.0
        || visible.height() <= 0.0
        || image_screen_rect.width() <= 0.0
        || image_screen_rect.height() <= 0.0
    {
        return minimap_img_rect;
    }
    let norm_min_x = ((visible.left() - image_screen_rect.left()) / image_screen_rect.width()).clamp(0.0, 1.0);
    let norm_min_y = ((visible.top() - image_screen_rect.top()) / image_screen_rect.height()).clamp(0.0, 1.0);
    let norm_max_x = ((visible.right() - image_screen_rect.left()) / image_screen_rect.width()).clamp(0.0, 1.0);
    let norm_max_y = ((visible.bottom() - image_screen_rect.top()) / image_screen_rect.height()).clamp(0.0, 1.0);

    let vp_min = Pos2::new(
        minimap_img_rect.left() + norm_min_x * minimap_img_rect.width(),
        minimap_img_rect.top() + norm_min_y * minimap_img_rect.height(),
    );
    let vp_max = Pos2::new(
        minimap_img_rect.left() + norm_max_x * minimap_img_rect.width(),
        minimap_img_rect.top() + norm_max_y * minimap_img_rect.height(),
    );
    Rect::from_min_max(vp_min, vp_max).intersect(minimap_img_rect)
}

pub fn pan_from_minimap_click(
    click_pos: Pos2,
    minimap_img_rect: Rect,
    display_size: Vec2,
    canvas_size: Vec2,
) -> Vec2 {
    let norm_x = ((click_pos.x - minimap_img_rect.left()) / minimap_img_rect.width()).clamp(0.0, 1.0);
    let norm_y = ((click_pos.y - minimap_img_rect.top()) / minimap_img_rect.height()).clamp(0.0, 1.0);

    let raw_pan = Vec2::new(0.5 - norm_x, 0.5 - norm_y) * display_size;
    clamp_pan(raw_pan, canvas_size, display_size)
}

fn active_drag_start_pointer(drag: &ActiveDrag) -> Pos2 {
    match drag {
        ActiveDrag::Move { start_pointer, .. } => *start_pointer,
        ActiveDrag::Resize { start_pointer, .. } => *start_pointer,
        ActiveDrag::MoveVertex { start_pointer, .. } => *start_pointer,
        ActiveDrag::MinimapPan { start_pointer } => *start_pointer,
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
            locked: false,
            points: None,
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

    #[test]
    fn test_calculate_minimap_viewport() {
        let canvas = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(800.0, 600.0));
        // Image centered at (400, 300) with size (1600, 1200) -> image spans (-400..1200, -300..900)
        let image_rect = Rect::from_center_size(canvas.center(), Vec2::new(1600.0, 1200.0));
        let minimap_img_rect = Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::new(160.0, 120.0));

        let vp = calculate_minimap_viewport(image_rect, canvas, minimap_img_rect);

        // Visible portion on screen is (0..800, 0..600), which is center 50% width and 50% height
        // normalized min: 400/1600 = 0.25, normalized max: 1200/1600 = 0.75
        // normalized min_y: 300/1200 = 0.25, normalized max_y: 900/1200 = 0.75
        assert!((vp.left() - 140.0).abs() < 1.0);
        assert!((vp.top() - 130.0).abs() < 1.0);
        assert!((vp.width() - 80.0).abs() < 1.0);
        assert!((vp.height() - 60.0).abs() < 1.0);
    }

    #[test]
    fn test_pan_from_minimap_click() {
        let canvas_size = Vec2::new(800.0, 600.0);
        let display_size = Vec2::new(1600.0, 1200.0);
        let minimap_img_rect = Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::new(160.0, 120.0));

        // Click center of minimap: (180, 160)
        let pan_center = pan_from_minimap_click(Pos2::new(180.0, 160.0), minimap_img_rect, display_size, canvas_size);
        assert!((pan_center.x).abs() < 0.01);
        assert!((pan_center.y).abs() < 0.01);

        // Click top-left of minimap: (100, 100) -> pans to show top-left of image
        let pan_tl = pan_from_minimap_click(Pos2::new(100.0, 100.0), minimap_img_rect, display_size, canvas_size);
        // limit is (1600 - 800) * 0.5 = 400 for x, (1200 - 600) * 0.5 = 300 for y
        assert_eq!(pan_tl, Vec2::new(400.0, 300.0));

        // Click bottom-right of minimap: (260, 220) -> pans to show bottom-right of image
        let pan_br = pan_from_minimap_click(Pos2::new(260.0, 220.0), minimap_img_rect, display_size, canvas_size);
        assert_eq!(pan_br, Vec2::new(-400.0, -300.0));
    }

    #[test]
    fn test_polygon_mode_does_not_select_existing_rectangle() {
        let mut app = crate::app::AnnotatorApp::default();
        app.tool_mode = ToolMode::Polygon;

        // Existing rectangle (0,0) to (200,200) with id 1
        app.annotations.push(sample_annotation(1, 0.0, 0.0, 200.0, 200.0));
        app.next_id = 2;
        assert!(app.selected.is_empty());

        // Point inside the rectangle
        let point_inside = Pos2::new(50.0, 50.0);
        let mut draft = DraftPolygon::new(point_inside);
        draft.add_point(Pos2::new(100.0, 50.0));
        draft.add_point(Pos2::new(100.0, 100.0));
        app.draft_polygon = Some(draft);

        // Still nothing selected, polygon has 3 points
        assert!(app.selected.is_empty());
        assert_eq!(app.draft_polygon.as_ref().unwrap().points.len(), 3);

        // Finish polygon
        assert!(app.finish_draft_polygon());
        assert_eq!(app.annotations.len(), 2);
        // New polygon (id 0 or next_id) is selected, not the container rectangle
        assert_eq!(app.selected.len(), 1);
        let selected_id = *app.selected.iter().next().unwrap();
        let selected_anno = app.annotations.iter().find(|a| a.id == selected_id).unwrap();
        assert!(selected_anno.points.is_some());
    }

    #[test]
    fn test_polygon_move_single_vertex_recomputes_bounding_box() {
        let mut app = crate::app::AnnotatorApp::default();
        let anno = Annotation {
            id: 1,
            label: "poly".into(),
            description: None,
            x: 10.0,
            y: 10.0,
            width: 80.0,
            height: 80.0,
            color: [255, 0, 0],
            parent_id: None,
            locked: false,
            points: Some(vec![[10.0, 10.0], [90.0, 10.0], [90.0, 90.0], [10.0, 90.0]]),
        };
        app.annotations.push(anno);
        app.select_single(1);

        // Move top-left vertex from (10, 10) to (0, 0)
        let pts = app.annotations[0].points.as_mut().unwrap();
        pts[0] = [0.0, 0.0];
        let poly_pos: Vec<Pos2> = pts.iter().map(|p| Pos2::new(p[0], p[1])).collect();
        let (bb_x, bb_y, bb_w, bb_h) = crate::geometry::polygon_bounding_box(&poly_pos);
        app.annotations[0].x = bb_x;
        app.annotations[0].y = bb_y;
        app.annotations[0].width = bb_w;
        app.annotations[0].height = bb_h;

        assert_eq!(app.annotations[0].x, 0.0);
        assert_eq!(app.annotations[0].y, 0.0);
        assert_eq!(app.annotations[0].width, 90.0);
        assert_eq!(app.annotations[0].height, 90.0);
    }

    #[test]
    fn test_polygon_vertex_insertion_and_deletion() {
        let mut app = crate::app::AnnotatorApp::default();
        let anno = Annotation {
            id: 1,
            label: "triangle".into(),
            description: None,
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            color: [0, 255, 0],
            parent_id: None,
            locked: false,
            points: Some(vec![[0.0, 0.0], [100.0, 0.0], [50.0, 100.0]]),
        };
        app.annotations.push(anno);
        app.select_single(1);

        // 1. Insert new vertex on edge (between vertex 0 and 1 at index 1)
        let pts = app.annotations[0].points.as_mut().unwrap();
        pts.insert(1, [50.0, 0.0]);
        assert_eq!(pts.len(), 4);
        assert_eq!(pts[1], [50.0, 0.0]);

        // 2. Remove the newly inserted vertex
        pts.remove(1);
        assert_eq!(pts.len(), 3);
        assert_eq!(pts[1], [100.0, 0.0]);
    }

    #[test]
    fn test_select_mode_vs_rectangle_mode() {
        let mut app = crate::app::AnnotatorApp::default();
        app.tool_mode = ToolMode::Select;
        assert_eq!(app.tool_mode, ToolMode::Select);

        app.annotations.push(sample_annotation(1, 0.0, 0.0, 100.0, 100.0));
        app.select_single(1);
        assert!(app.is_selected(1));

        // Switch to Rectangle tool
        app.tool_mode = ToolMode::Rectangle;
        assert_eq!(app.tool_mode, ToolMode::Rectangle);
    }
}
