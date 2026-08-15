use std::path::{Path, PathBuf};
use crate::models::ProjectFile;

pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "bmp", "tif", "tiff",
];

pub fn is_image_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            let ext_lower = ext.to_lowercase();
            SUPPORTED_EXTENSIONS.contains(&ext_lower.as_str())
        })
        .unwrap_or(false)
}

pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let mut a_chars = a.chars().peekable();
    let mut b_chars = b.chars().peekable();

    while let (Some(&ca), Some(&cb)) = (a_chars.peek(), b_chars.peek()) {
        if ca.is_ascii_digit() && cb.is_ascii_digit() {
            let mut num_a: u64 = 0;
            while let Some(&d) = a_chars.peek() {
                if let Some(digit) = d.to_digit(10) {
                    num_a = num_a.saturating_mul(10).saturating_add(digit as u64);
                    a_chars.next();
                } else {
                    break;
                }
            }

            let mut num_b: u64 = 0;
            while let Some(&d) = b_chars.peek() {
                if let Some(digit) = d.to_digit(10) {
                    num_b = num_b.saturating_mul(10).saturating_add(digit as u64);
                    b_chars.next();
                } else {
                    break;
                }
            }

            match num_a.cmp(&num_b) {
                std::cmp::Ordering::Equal => continue,
                other => return other,
            }
        } else {
            let ca_lower = ca.to_ascii_lowercase();
            let cb_lower = cb.to_ascii_lowercase();
            match ca_lower.cmp(&cb_lower) {
                std::cmp::Ordering::Equal => {
                    a_chars.next();
                    b_chars.next();
                }
                other => return other,
            }
        }
    }

    a_chars.peek().is_some().cmp(&b_chars.peek().is_some())
}

pub fn scan_image_folder(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };

    let mut files: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && is_image_file(path))
        .collect();

    files.sort_by(|a, b| {
        let name_a = a.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let name_b = b.file_name().and_then(|n| n.to_str()).unwrap_or("");
        natural_cmp(name_a, name_b)
    });

    files
}

pub fn check_sidecar_annotation_count(image_path: &Path) -> Option<usize> {
    let anno_path = image_path.with_extension("anno");
    if !anno_path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&anno_path).ok()?;
    let project: ProjectFile = serde_json::from_str(&content).ok()?;
    Some(project.annotations.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_image_file() {
        assert!(is_image_file(Path::new("sample.png")));
        assert!(is_image_file(Path::new("photo.JPG")));
        assert!(is_image_file(Path::new("dataset/frame.WEBP")));
        assert!(is_image_file(Path::new("img.tiff")));
        assert!(!is_image_file(Path::new("annotations.json")));
        assert!(!is_image_file(Path::new("project.anno")));
        assert!(!is_image_file(Path::new("document.pdf")));
    }

    #[test]
    fn test_natural_sort() {
        let mut names = vec![
            "frame_10.jpg",
            "frame_1.jpg",
            "frame_2.jpg",
            "frame_20.jpg",
            "frame_3.jpg",
        ];

        names.sort_by(|a, b| natural_cmp(a, b));

        assert_eq!(
            names,
            vec![
                "frame_1.jpg",
                "frame_2.jpg",
                "frame_3.jpg",
                "frame_10.jpg",
                "frame_20.jpg",
            ]
        );
    }

    #[test]
    fn test_natural_sort_mixed_case() {
        let mut names = vec!["IMG_2.png", "img_1.png", "Img_10.png"];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(names, vec!["img_1.png", "IMG_2.png", "Img_10.png"]);
    }
}
