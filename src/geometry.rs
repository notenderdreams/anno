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
    let extra = if annotation.locked { 18.0 } else { 0.0 };
    let width = (label.len() as f32 * 6.5 + 10.0 + extra).max(35.0);
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

pub fn image_to_screen(point: Pos2, image_rect: Rect, image_size: Vec2) -> Pos2 {
    Pos2::new(
        image_rect.left() + (point.x / image_size.x) * image_rect.width(),
        image_rect.top() + (point.y / image_size.y) * image_rect.height(),
    )
}

pub fn point_in_polygon(point: Pos2, vertices: &[Pos2]) -> bool {
    if vertices.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = vertices.len() - 1;
    for i in 0..vertices.len() {
        let pi = vertices[i];
        let pj = vertices[j];
        if ((pi.y > point.y) != (pj.y > point.y))
            && (point.x < (pj.x - pi.x) * (point.y - pi.y) / (pj.y - pi.y) + pi.x)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

pub fn polygon_bounding_box(points: &[Pos2]) -> (f32, f32, f32, f32) {
    if points.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for p in points {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }
    (min_x, min_y, (max_x - min_x).max(1.0), (max_y - min_y).max(1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_in_polygon() {
        let triangle = vec![
            Pos2::new(0.0, 0.0),
            Pos2::new(10.0, 0.0),
            Pos2::new(5.0, 10.0),
        ];

        // Center inside
        assert!(point_in_polygon(Pos2::new(5.0, 3.0), &triangle));

        // Outside
        assert!(!point_in_polygon(Pos2::new(0.0, 10.0), &triangle));
        assert!(!point_in_polygon(Pos2::new(15.0, 5.0), &triangle));
    }

    #[test]
    fn test_polygon_bounding_box() {
        let points = vec![
            Pos2::new(10.0, 20.0),
            Pos2::new(50.0, 80.0),
            Pos2::new(30.0, 10.0),
        ];

        let (x, y, w, h) = polygon_bounding_box(&points);
        assert_eq!(x, 10.0);
        assert_eq!(y, 10.0);
        assert_eq!(w, 40.0);
        assert_eq!(h, 70.0);
    }
}
