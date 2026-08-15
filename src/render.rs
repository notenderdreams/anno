use eframe::egui::{self, Color32, FontId, Pos2, Rect, Stroke, Vec2};

pub fn draw_lucide_lock(
    painter: &egui::Painter,
    center: Pos2,
    size: f32,
    locked: bool,
    color: Color32,
    stroke_width: f32,
) {
    let scale = size / 24.0;
    let top_left = Pos2::new(center.x - 12.0 * scale, center.y - 12.0 * scale);
    let pt = |x: f32, y: f32| Pos2::new(top_left.x + x * scale, top_left.y + y * scale);

    // 1. Body: <rect width="18" height="11" x="3" y="11" rx="2" ry="2"/>
    let body_rect = Rect::from_min_max(pt(3.0, 11.0), pt(21.0, 22.0));
    painter.rect_stroke(body_rect, 2.0 * scale, Stroke::new(stroke_width, color));

    // 2. Shackle
    if locked {
        // <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
        let mut pts = vec![pt(7.0, 11.0)];
        for step in 0..=8 {
            let a = std::f32::consts::PI - (step as f32 / 8.0) * std::f32::consts::PI;
            pts.push(pt(12.0 + 5.0 * a.cos(), 7.0 - 5.0 * a.sin()));
        }
        pts.push(pt(17.0, 11.0));
        for w in pts.windows(2) {
            painter.line_segment([w[0], w[1]], Stroke::new(stroke_width, color));
        }
    } else {
        // <path d="M7 11V7a5 5 0 0 1 9.9-1"/>
        let mut pts = vec![pt(7.0, 11.0)];
        for step in 0..=7 {
            let a = std::f32::consts::PI - (step as f32 / 7.0) * (std::f32::consts::PI - 0.2);
            pts.push(pt(12.0 + 5.0 * a.cos(), 7.0 - 5.0 * a.sin()));
        }
        pts.push(pt(16.9, 6.0));
        for w in pts.windows(2) {
            painter.line_segment([w[0], w[1]], Stroke::new(stroke_width, color));
        }
    }
}

pub fn draw_surveillance_box(
    painter: &egui::Painter,
    rect: Rect,
    label: &str,
    box_color: Color32,
    selected: bool,
    locked: bool,
) {
    let color = if selected {
        box_color
    } else {
        Color32::from_rgb(
            (box_color.r() as f32 * 0.75) as u8,
            (box_color.g() as f32 * 0.75) as u8,
            (box_color.b() as f32 * 0.75) as u8,
        )
    };

    let width = if selected { 1.8_f32 } else { 1.15_f32 };
    painter.rect_stroke(rect, 0.0, Stroke::new(width, color));

    let text = if label.trim().is_empty() {
        "UNLABELED"
    } else {
        label
    };

    let galley =
        painter.layout_no_wrap(text.to_uppercase(), FontId::monospace(10.0), Color32::WHITE);

    let extra_lock_w = if locked { 14.0 } else { 0.0 };
    let tag_size = galley.size() + Vec2::new(10.0 + extra_lock_w, 6.0);
    let tag_rect = Rect::from_min_size(Pos2::new(rect.left(), rect.top() - tag_size.y), tag_size);

    let tag_rect = if tag_rect.top() < painter.clip_rect().top() {
        Rect::from_min_size(rect.min, tag_size)
    } else {
        tag_rect
    };

    painter.rect_filled(tag_rect, 0.0, color);

    if locked {
        let icon_center = Pos2::new(tag_rect.left() + 8.0, tag_rect.center().y);
        draw_lucide_lock(painter, icon_center, 9.0, true, Color32::WHITE, 1.2);
        painter.galley(
            Pos2::new(tag_rect.left() + 15.0, tag_rect.center().y - galley.size().y * 0.5),
            galley,
            Color32::WHITE,
        );
    } else {
        painter.galley(
            tag_rect.center() - galley.size() * 0.5,
            galley,
            Color32::WHITE,
        );
    }

    if selected && !locked {
        for point in [
            rect.left_top(),
            rect.right_top(),
            rect.left_bottom(),
            rect.right_bottom(),
        ] {
            painter.rect_filled(
                Rect::from_center_size(point, Vec2::splat(5.0)),
                0.0,
                Color32::WHITE,
            );
            painter.rect_stroke(
                Rect::from_center_size(point, Vec2::splat(5.0)),
                0.0,
                Stroke::new(1.0_f32, color),
            );
        }
    }
}

pub fn draw_polygon_annotation(
    painter: &egui::Painter,
    screen_points: &[Pos2],
    bounding_rect: Rect,
    label: &str,
    box_color: Color32,
    selected: bool,
    locked: bool,
    selected_vertex: Option<usize>,
    hovered_vertex: Option<usize>,
    edge_insert_hint: Option<Pos2>,
) {
    if screen_points.len() < 2 {
        return;
    }

    let color = if selected {
        box_color
    } else {
        Color32::from_rgb(
            (box_color.r() as f32 * 0.75) as u8,
            (box_color.g() as f32 * 0.75) as u8,
            (box_color.b() as f32 * 0.75) as u8,
        )
    };

    // Semi-transparent polygon fill
    if screen_points.len() >= 3 {
        let fill_alpha = if selected { 45 } else { 25 };
        let fill_color = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), fill_alpha);
        painter.add(egui::Shape::convex_polygon(screen_points.to_vec(), fill_color, Stroke::NONE));
    }

    // Polygon boundary outline
    let stroke_w = if selected { 1.8_f32 } else { 1.15_f32 };
    for w in screen_points.windows(2) {
        painter.line_segment([w[0], w[1]], Stroke::new(stroke_w, color));
    }
    if screen_points.len() >= 3 {
        painter.line_segment(
            [screen_points[screen_points.len() - 1], screen_points[0]],
            Stroke::new(stroke_w, color),
        );
    }

    // Edge insertion preview handle (when hovering near an edge in Select mode)
    if let Some(hint_pos) = edge_insert_hint {
        painter.circle_filled(hint_pos, 4.5, Color32::from_rgba_unmultiplied(255, 255, 255, 230));
        painter.circle_stroke(hint_pos, 4.5, Stroke::new(1.2_f32, color));
        painter.line_segment([Pos2::new(hint_pos.x - 2.5, hint_pos.y), Pos2::new(hint_pos.x + 2.5, hint_pos.y)], Stroke::new(1.2_f32, color));
        painter.line_segment([Pos2::new(hint_pos.x, hint_pos.y - 2.5), Pos2::new(hint_pos.x, hint_pos.y + 2.5)], Stroke::new(1.2_f32, color));
    }

    // Vertex handles (interactive when selected and unlocked)
    for (i, &pt) in screen_points.iter().enumerate() {
        if selected && !locked {
            let is_selected_v = selected_vertex == Some(i);
            let is_hovered_v = hovered_vertex == Some(i);

            if is_selected_v {
                // High-visibility active selected vertex with accent ring and central core
                painter.circle_stroke(pt, 7.0, Stroke::new(2.0_f32, Color32::from_rgb(255, 220, 0)));
                painter.circle_filled(pt, 5.0, Color32::WHITE);
                painter.circle_filled(pt, 2.5, color);
            } else if is_hovered_v {
                // Interactive hover state: crisp white body + colored outline
                painter.circle_filled(pt, 5.5, Color32::WHITE);
                painter.circle_stroke(pt, 6.5, Stroke::new(1.5_f32, color));
                painter.circle_filled(pt, 2.5, color);
            } else {
                // Default selected vertex handle
                painter.circle_filled(pt, 4.0, color);
                painter.circle_stroke(pt, 4.5, Stroke::new(1.0_f32, Color32::WHITE));
            }
        } else {
            // Unselected polygon vertex dot
            painter.circle_filled(pt, 2.2, color);
        }
    }

    // Tag at top-left of bounding rect
    let text = if label.trim().is_empty() {
        "UNLABELED"
    } else {
        label
    };

    let galley = painter.layout_no_wrap(text.to_uppercase(), FontId::monospace(10.0), Color32::WHITE);
    let extra_lock_w = if locked { 14.0 } else { 0.0 };
    let tag_size = galley.size() + Vec2::new(10.0 + extra_lock_w, 6.0);
    let tag_rect = Rect::from_min_size(Pos2::new(bounding_rect.left(), bounding_rect.top() - tag_size.y), tag_size);
    let tag_rect = if tag_rect.top() < painter.clip_rect().top() {
        Rect::from_min_size(bounding_rect.min, tag_size)
    } else {
        tag_rect
    };

    painter.rect_filled(tag_rect, 0.0, color);
    if locked {
        let icon_center = Pos2::new(tag_rect.left() + 8.0, tag_rect.center().y);
        draw_lucide_lock(painter, icon_center, 9.0, true, Color32::WHITE, 1.2);
        painter.galley(
            Pos2::new(tag_rect.left() + 15.0, tag_rect.center().y - galley.size().y * 0.5),
            galley,
            Color32::WHITE,
        );
    } else {
        painter.galley(
            tag_rect.center() - galley.size() * 0.5,
            galley,
            Color32::WHITE,
        );
    }
}

pub fn draw_draft_polygon(
    painter: &egui::Painter,
    screen_points: &[Pos2],
    current_cursor: Option<Pos2>,
    color: Color32,
    label: &str,
    can_close: bool,
) {
    if screen_points.is_empty() {
        return;
    }

    // Semi-transparent polygon fill preview if hovering near start point to close
    if can_close && screen_points.len() >= 3 {
        let fill_color = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 35);
        painter.add(egui::Shape::convex_polygon(
            screen_points.to_vec(),
            fill_color,
            Stroke::NONE,
        ));
    }

    // Connect placed points
    for w in screen_points.windows(2) {
        painter.line_segment([w[0], w[1]], Stroke::new(1.8_f32, color));
    }

    // Connect last point to cursor or start point
    if let Some(cursor) = current_cursor {
        let last = *screen_points.last().unwrap();
        if can_close {
            // When hovering close to start point, connect directly back to start point
            painter.line_segment([last, screen_points[0]], Stroke::new(2.0_f32, color));
        } else {
            painter.line_segment([last, cursor], Stroke::new(1.5_f32, color));

            if screen_points.len() >= 2 {
                painter.line_segment(
                    [cursor, screen_points[0]],
                    Stroke::new(1.0_f32, Color32::from_gray(130)),
                );
            }
        }

        // If hovering near start point, draw Photoshop-style loop indicator circle next to cursor
        if can_close {
            let loop_center = cursor + Vec2::new(10.0, 10.0);
            painter.circle_filled(loop_center, 4.0, Color32::from_black_alpha(200));
            painter.circle_stroke(loop_center, 3.5, Stroke::new(1.5_f32, Color32::WHITE));
        }
    }

    // Draw vertex points
    for (i, &pt) in screen_points.iter().enumerate() {
        if i == 0 {
            if can_close {
                // Large glowing snap indicator for start point
                painter.circle_filled(
                    pt,
                    9.0_f32,
                    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 90),
                );
                painter.circle_filled(pt, 5.5_f32, color);
                painter.circle_stroke(pt, 7.5_f32, Stroke::new(2.0_f32, Color32::WHITE));
            } else {
                painter.circle_filled(pt, 4.5_f32, color);
                painter.circle_stroke(pt, 6.0_f32, Stroke::new(1.5_f32, Color32::WHITE));
            }
        } else {
            painter.circle_filled(pt, 3.5_f32, color);
            painter.circle_stroke(pt, 4.5_f32, Stroke::new(1.0_f32, Color32::WHITE));
        }
    }

    // Render draft label tag near first point
    let tag_pos = screen_points[0] - Vec2::new(0.0, 20.0);
    let galley = painter.layout_no_wrap(label.to_uppercase(), FontId::monospace(9.5), Color32::WHITE);
    let tag_rect = Rect::from_min_size(tag_pos, galley.size() + Vec2::new(8.0, 4.0));
    painter.rect_filled(tag_rect, 0.0, color);
    painter.galley(tag_rect.center() - galley.size() * 0.5, galley, Color32::WHITE);
}
