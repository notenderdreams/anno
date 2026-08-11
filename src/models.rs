use eframe::egui::{Color32, Pos2, TextureHandle};
use serde::Serialize;
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
        id: u32,
        start_pointer: Pos2,
        initial_x: f32,
        initial_y: f32,
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
}

#[derive(Clone, Serialize)]
pub struct Annotation {
    pub id: u32,
    pub label: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub color: [u8; 3],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<u32>,
}

impl Annotation {
    pub fn color32(&self) -> Color32 {
        Color32::from_rgb(self.color[0], self.color[1], self.color[2])
    }
}

#[derive(Serialize)]
pub struct AnnotationFile<'a> {
    pub image: String,
    pub image_width: u32,
    pub image_height: u32,
    pub annotations: &'a [Annotation],
}

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
