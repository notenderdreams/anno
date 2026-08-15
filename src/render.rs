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
