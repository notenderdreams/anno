use eframe::egui::{Pos2, TextureHandle};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Clone, Serialize)]
pub struct Annotation {
    pub id: u32,
    pub label: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
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
