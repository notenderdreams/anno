use eframe::egui::{Color32, Pos2, TextureHandle};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq)]
pub enum ResizeHandle {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

pub enum ActiveDrag {
    Move {
        initial_positions: Vec<(u32, f32, f32)>,
        start_pointer: Pos2,
    },
    Resize {
        id: u32,
        handle: ResizeHandle,
        start_pointer: Pos2,
        initial_x: f32,
        initial_y: f32,
        initial_w: f32,
        initial_h: f32,
    },
    MinimapPan {
        start_pointer: Pos2,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    pub id: u32,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub color: [u8; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<u32>,
    #[serde(default)]
    pub locked: bool,
}

impl Annotation {
    pub fn color32(&self) -> Color32 {
        Color32::from_rgb(self.color[0], self.color[1], self.color[2])
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectFile {
    pub image: String,
    pub image_width: u32,
    pub image_height: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default = "default_next_id")]
    pub next_id: u32,
    pub annotations: Vec<Annotation>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BatchProjectFile {
    pub format: String,
    pub format_version: u32,
    pub dataset_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset_folder: Option<String>,
    #[serde(default)]
    pub current_image_idx: usize,
    pub images: Vec<ProjectFile>,
}

fn default_next_id() -> u32 {
    1
}

#[derive(Serialize)]
pub struct AnnotationFile<'a> {
    pub image: String,
    pub image_width: u32,
    pub image_height: u32,
    pub annotations: Vec<ExportAnnotation<'a>>,
}

#[derive(Serialize)]
pub struct UnifiedDatasetExport<'a> {
    pub dataset_name: String,
    pub total_images: usize,
    pub annotated_images: usize,
    pub images: Vec<UnifiedImageExport<'a>>,
}

#[derive(Serialize)]
pub struct UnifiedImageExport<'a> {
    pub image: String,
    pub image_width: u32,
    pub image_height: u32,
    pub annotations: Vec<ExportAnnotation<'a>>,
}

#[derive(Clone, Serialize)]
pub struct ExportAnnotation<'a> {
    pub id: u32,
    pub label: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<&'a str>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub color: [u8; 3],
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ExportAnnotation<'a>>,
}

pub fn export_annotation_tree(annotations: &[Annotation]) -> Vec<ExportAnnotation<'_>> {
    annotations
        .iter()
        .filter(|annotation| annotation.parent_id.is_none())
        .map(|annotation| export_annotation(annotation, annotations))
        .collect()
}

fn export_annotation<'a>(
    annotation: &'a Annotation,
    annotations: &'a [Annotation],
) -> ExportAnnotation<'a> {
    let children = annotations
        .iter()
        .filter(|child| child.parent_id == Some(annotation.id))
        .map(|child| export_annotation(child, annotations))
        .collect();

    ExportAnnotation {
        id: annotation.id,
        label: &annotation.label,
        description: annotation.description.as_deref(),
        x: annotation.x,
        y: annotation.y,
        width: annotation.width,
        height: annotation.height,
        color: annotation.color,
        children,
    }
}

#[derive(Clone)]
pub struct LoadedImage {
    pub texture: TextureHandle,
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
}

pub struct Draft {
    pub start: Pos2,
    pub current: Pos2,
}

#[cfg(test)]
mod tests {
    use super::{export_annotation_tree, Annotation, ProjectFile};

    fn annotation(id: u32, parent_id: Option<u32>) -> Annotation {
        Annotation {
            id,
            label: format!("Region {id}"),
            description: Some("Sample region note".into()),
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 80.0,
            color: [255, 0, 0],
            parent_id,
            locked: false,
        }
    }

    #[test]
    fn export_nests_children_and_hides_internal_parent_ids() {
        let annotations = vec![
            annotation(1, None),
            annotation(2, Some(1)),
            annotation(3, Some(2)),
        ];

        let json = serde_json::to_value(export_annotation_tree(&annotations)).unwrap();

        assert_eq!(json[0]["id"], 1);
        assert_eq!(json[0]["description"], "Sample region note");
        assert_eq!(json[0]["children"][0]["id"], 2);
        assert_eq!(json[0]["children"][0]["children"][0]["id"], 3);
        assert!(json[0].get("parent_id").is_none());
        assert!(json[0]["children"][0].get("parent_id").is_none());
        assert!(json[0]["children"][0]["children"][0]
            .get("children")
            .is_none());
    }

    #[test]
    fn project_file_round_trip() {
        let project = ProjectFile {
            image: "test_sample.png".into(),
            image_width: 800,
            image_height: 600,
            description: Some("Test project description".into()),
            next_id: 3,
            annotations: vec![annotation(1, None), annotation(2, Some(1))],
        };

        let json_str = serde_json::to_string_pretty(&project).unwrap();
        let decoded: ProjectFile = serde_json::from_str(&json_str).unwrap();

        assert_eq!(project, decoded);
    }
}
