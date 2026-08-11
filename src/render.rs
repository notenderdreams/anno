use eframe::egui::{self, Color32, FontId, Pos2, Rect, Stroke, Vec2};

pub fn draw_surveillance_box(
    painter: &egui::Painter,
    rect: Rect,
    label: &str,
    box_color: Color32,
    selected: bool,
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

    let tag_size = galley.size() + Vec2::new(10.0, 6.0);
    let tag_rect = Rect::from_min_size(Pos2::new(rect.left(), rect.top() - tag_size.y), tag_size);

    let tag_rect = if tag_rect.top() < painter.clip_rect().top() {
        Rect::from_min_size(rect.min, tag_size)
    } else {
        tag_rect
    };

    painter.rect_filled(tag_rect, 0.0, color);
    painter.galley(
        tag_rect.center() - galley.size() * 0.5,
        galley,
        Color32::WHITE,
    );

    if selected {
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
