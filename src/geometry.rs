use crate::models::Annotation;
use eframe::egui::{Pos2, Rect, Vec2};

pub fn screen_to_image(point: Pos2, image_rect: Rect, image_size: Vec2) -> Pos2 {
    let x = ((point.x - image_rect.left()) / image_rect.width() * image_size.x)
        .clamp(0.0, image_size.x);

    let y = ((point.y - image_rect.top()) / image_rect.height() * image_size.y)
        .clamp(0.0, image_size.y);

    Pos2::new(x, y)
}

pub fn annotation_screen_rect(annotation: &Annotation, image_rect: Rect, image_size: Vec2) -> Rect {
    let min = Pos2::new(
        image_rect.left() + (annotation.x / image_size.x) * image_rect.width(),
        image_rect.top() + (annotation.y / image_size.y) * image_rect.height(),
    );

    let max = Pos2::new(
        min.x + (annotation.width / image_size.x) * image_rect.width(),
        min.y + (annotation.height / image_size.y) * image_rect.height(),
    );

    Rect::from_min_max(min, max)
}
