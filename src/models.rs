use eframe::egui::{Color32, Pos2, TextureHandle};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ToolMode {
    #[default]
    Rectangle,
    Polygon,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FilmstripFilter {
    #[default]
    All,
    Annotated,
    Unannotated,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ResizeHandle {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

pub enum ActiveDrag {
    Move {
        initial_positions: Vec<(u32, f32, f32, Option<Vec<[f32; 2]>>)>,
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
        initial_points: Option<Vec<[f32; 2]>>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub points: Option<Vec<[f32; 2]>>,
}

impl Annotation {
    pub fn color32(&self) -> Color32 {
        Color32::from_rgb(self.color[0], self.color[1], self.color[2])
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClassPreset {
    pub prefix: String,
    pub color: [u8; 3],
}

impl ClassPreset {
    pub fn new(prefix: impl Into<String>, color: [u8; 3]) -> Self {
        Self {
            prefix: prefix.into(),
            color,
        }
    }

    pub fn color32(&self) -> Color32 {
        Color32::from_rgb(self.color[0], self.color[1], self.color[2])
    }
}

pub fn default_presets() -> Vec<ClassPreset> {
    vec![
        ClassPreset::new("object", [255, 0, 0]),      // 1: Red
        ClassPreset::new("person", [41, 121, 255]),    // 2: Blue
        ClassPreset::new("vehicle", [0, 230, 118]),   // 3: Green
        ClassPreset::new("animal", [255, 214, 0]),    // 4: Yellow
        ClassPreset::new("item", [255, 145, 0]),      // 5: Orange
        ClassPreset::new("structure", [0, 229, 255]), // 6: Cyan
        ClassPreset::new("sign", [213, 0, 249]),      // 7: Purple
        ClassPreset::new("face", [255, 110, 64]),     // 8: Coral
        ClassPreset::new("region", [118, 255, 3]),    // 9: Lime
    ]
}

use std::collections::HashSet;

pub fn match_class_presets<'a>(
    query: &str,
    presets: &'a [ClassPreset],
) -> Vec<(usize, &'a ClassPreset)> {
    let q = query.trim().to_lowercase();
    let base_q = q.split('_').next().unwrap_or(&q).trim();
    presets
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            if base_q.is_empty() {
                true
            } else {
                let p_lower = p.prefix.to_lowercase();
                p_lower.starts_with(base_q) || p_lower.contains(base_q)
            }
        })
        .collect()
}

pub fn next_category_label_from_labels<'a, I>(prefix: &str, labels: I) -> String
where
    I: IntoIterator<Item = &'a str>,
{
    let clean_prefix = prefix.trim();
    if clean_prefix.is_empty() {
        return "object_01".to_string();
    }

    let prefix_lower = clean_prefix.to_lowercase();
    let prefix_underscore = format!("{prefix_lower}_");

    let mut max_seq: usize = 0;
    let mut count: usize = 0;

    for label in labels {
        let label_lower = label.trim().to_lowercase();
        if label_lower == prefix_lower {
            count += 1;
            max_seq = max_seq.max(1);
        } else if let Some(suffix) = label_lower.strip_prefix(&prefix_underscore) {
            count += 1;
            if let Ok(num) = suffix.parse::<usize>() {
                max_seq = max_seq.max(num);
            }
        }
    }

    let next_num = max_seq.max(count) + 1;
    format!("{clean_prefix}_{:02}", next_num)
}

pub fn next_category_label(
    prefix: &str,
    annotations: &[Annotation],
    exclude_id: Option<u32>,
) -> String {
    next_category_label_from_labels(
        prefix,
        annotations
            .iter()
            .filter(|a| exclude_id != Some(a.id))
            .map(|a| a.label.as_str()),
    )
}

pub fn assign_preset_to_annotations(
    annotations: &mut [Annotation],
    selected_ids: &HashSet<u32>,
    preset: &ClassPreset,
) -> usize {
    let clean_prefix = preset.prefix.trim();
    let prefix_lower = clean_prefix.to_lowercase();
    let prefix_underscore = format!("{prefix_lower}_");

    let mut max_seq: usize = 0;
    let mut count: usize = 0;

    for a in annotations.iter() {
        if selected_ids.contains(&a.id) || a.locked {
            continue;
        }
        let label_lower = a.label.trim().to_lowercase();
        if label_lower == prefix_lower {
            count += 1;
            max_seq = max_seq.max(1);
        } else if let Some(suffix) = label_lower.strip_prefix(&prefix_underscore) {
            count += 1;
            if let Ok(num) = suffix.parse::<usize>() {
                max_seq = max_seq.max(num);
            }
        }
    }

    let mut next_num = max_seq.max(count) + 1;
    let mut modified = 0;

    for a in annotations.iter_mut() {
        if selected_ids.contains(&a.id) && !a.locked {
            a.color = preset.color;
            a.label = format!("{clean_prefix}_{:02}", next_num);
            next_num += 1;
            modified += 1;
        }
    }

    modified
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub presets: Vec<ClassPreset>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub presets: Vec<ClassPreset>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub points: Option<&'a [[f32; 2]]>,
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
        points: annotation.points.as_deref(),
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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DraftPolygon {
    pub points: Vec<Pos2>,
    pub undone_points: Vec<Pos2>,
}

impl DraftPolygon {
    pub fn new(first_point: Pos2) -> Self {
        Self {
            points: vec![first_point],
            undone_points: Vec::new(),
        }
    }

    pub fn from_points(points: Vec<Pos2>) -> Self {
        Self {
            points,
            undone_points: Vec::new(),
        }
    }

    pub fn add_point(&mut self, point: Pos2) {
        self.points.push(point);
        self.undone_points.clear();
    }

    pub fn undo_point(&mut self) -> Option<Pos2> {
        let pt = self.points.pop()?;
        self.undone_points.push(pt);
        Some(pt)
    }

    pub fn redo_point(&mut self) -> Option<Pos2> {
        let pt = self.undone_points.pop()?;
        self.points.push(pt);
        Some(pt)
    }

    pub fn can_undo(&self) -> bool {
        !self.points.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.undone_points.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            points: None,
        }
    }

    #[test]
    fn test_polygon_annotation_export() {
        let mut poly = annotation(1, None);
        poly.points = Some(vec![[10.0, 20.0], [50.0, 20.0], [30.0, 60.0]]);

        let json = serde_json::to_value(export_annotation_tree(&[poly])).unwrap();
        assert_eq!(
            json[0]["points"],
            serde_json::json!([[10.0, 20.0], [50.0, 20.0], [30.0, 60.0]])
        );
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
            presets: Vec::new(),
        };

        let json_str = serde_json::to_string_pretty(&project).unwrap();
        let decoded: ProjectFile = serde_json::from_str(&json_str).unwrap();

        assert_eq!(project, decoded);
    }

    #[test]
    fn test_match_class_presets() {
        let presets = default_presets();

        // Empty query returns all presets
        assert_eq!(match_class_presets("", &presets).len(), 9);

        // "pe" matches "person"
        let matches = match_class_presets("pe", &presets);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].1.prefix, "person");

        // "person_02" matches "person"
        let matches_tag = match_class_presets("person_02", &presets);
        assert_eq!(matches_tag.len(), 1);
        assert_eq!(matches_tag[0].1.prefix, "person");

        // "veh" matches "vehicle"
        let matches_veh = match_class_presets("veh", &presets);
        assert_eq!(matches_veh.len(), 1);
        assert_eq!(matches_veh[0].1.prefix, "vehicle");

        // "nonexistent" matches 0
        assert_eq!(match_class_presets("nonexistent", &presets).len(), 0);
    }

    #[test]
    fn test_next_category_label() {
        let mut annotations = Vec::new();

        // 1st person
        assert_eq!(next_category_label("person", &annotations, None), "person_01");

        let mut a1 = annotation(1, None);
        a1.label = "person_01".to_string();
        annotations.push(a1);

        // 2nd person
        assert_eq!(next_category_label("person", &annotations, None), "person_02");

        // 1st vehicle (independent count from person)
        assert_eq!(next_category_label("vehicle", &annotations, None), "vehicle_01");

        let mut a2 = annotation(2, None);
        a2.label = "vehicle_01".to_string();
        annotations.push(a2);

        // 3rd person
        let mut a3 = annotation(3, None);
        a3.label = "person_02".to_string();
        annotations.push(a3);

        assert_eq!(next_category_label("person", &annotations, None), "person_03");

        // Changing a2 (id: 2) from vehicle_01 to person: next person should be person_03
        assert_eq!(next_category_label("person", &annotations, Some(2)), "person_03");
    }

    #[test]
    fn test_assign_preset_to_annotations() {
        let mut annotations = vec![
            annotation(1, None),
            annotation(2, None),
            annotation(3, None),
        ];

        let preset = ClassPreset::new("item", [255, 145, 0]);
        let mut selected = HashSet::new();
        selected.insert(1);
        selected.insert(3);

        let modified = assign_preset_to_annotations(&mut annotations, &selected, &preset);
        assert_eq!(modified, 2);
        assert_eq!(annotations[0].label, "item_01");
        assert_eq!(annotations[0].color, [255, 145, 0]);
        assert_eq!(annotations[2].label, "item_02");
        assert_eq!(annotations[2].color, [255, 145, 0]);
        // Annotation 2 was not selected
        assert_eq!(annotations[1].label, "Region 2");
    }
}
