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

pub fn annotation_tag_rect(annotation: &Annotation, image_rect: Rect, image_size: Vec2) -> Rect {
    let rect = annotation_screen_rect(annotation, image_rect, image_size);
    let label = if annotation.label.trim().is_empty() {
        "UNLABELED"
    } else {
        &annotation.label
    };
    let width = (label.len() as f32 * 6.5 + 10.0).max(35.0);
    let height = 16.0;
    let tag_rect = Rect::from_min_size(Pos2::new(rect.left(), rect.top() - height), Vec2::new(width, height));
    if tag_rect.top() < image_rect.top() {
        Rect::from_min_size(rect.min, Vec2::new(width, height))
    } else {
        tag_rect
    }
}

pub fn update_hierarchy(annotations: &mut [Annotation]) {
    let snapshot = annotations.to_vec();
    for item in annotations.iter_mut() {
        let mut best_parent_id: Option<u32> = None;
        let mut min_area = f32::MAX;

        for candidate in &snapshot {
            if candidate.id == item.id {
                continue;
            }

            if candidate.x <= item.x
                && candidate.y <= item.y
                && (candidate.x + candidate.width) >= (item.x + item.width)
                && (candidate.y + candidate.height) >= (item.y + item.height)
            {
                let area = candidate.width * candidate.height;
                if area < min_area {
                    min_area = area;
                    best_parent_id = Some(candidate.id);
                }
            }
        }

        item.parent_id = best_parent_id;
    }
}
