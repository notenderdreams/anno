use std::path::{Path, PathBuf};
use eframe::egui::{self, Key};

use crate::canvas::render_canvas;
use crate::models::{ActiveDrag, Annotation, AnnotationFile, Draft, LoadedImage};
use crate::sidebar_right::render_right_sidebar;
use crate::theme::configure_style;

pub struct AnnotatorApp {
    pub image: Option<LoadedImage>,
    pub annotations: Vec<Annotation>,
    pub selected: Option<u32>,
    pub editing_label: Option<u32>,
    pub next_id: u32,
    pub draft: Option<Draft>,
    pub active_drag: Option<ActiveDrag>,
    pub status: String,
    pub request_label_focus: bool,
}

impl AnnotatorApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_style(&cc.egui_ctx);
        Self {
            image: None,
            annotations: Vec::new(),
            selected: None,
            editing_label: None,
            next_id: 1,
            draft: None,
            active_drag: None,
            status: "OPEN AN IMAGE TO BEGIN".into(),
            request_label_focus: false,
        }
    }

    pub fn open_dialog(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(
                "Images",
                &["png", "jpg", "jpeg", "webp", "bmp", "tif", "tiff"],
            )
            .pick_file()
        {
            self.load_image(ctx, path);
        }
    }

    pub fn load_image(&mut self, ctx: &egui::Context, path: PathBuf) {
        match image::open(&path) {
            Ok(decoded) => {
                let rgba = decoded.to_rgba8();
                let (width, height) = rgba.dimensions();
                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                    [width as usize, height as usize],
                    rgba.as_raw(),
                );
                let texture = ctx.load_texture(
                    path.to_string_lossy(),
                    color_image,
                    egui::TextureOptions::LINEAR,
                );
                self.image = Some(LoadedImage {
                    texture,
                    path,
                    width,
                    height,
                });
                self.annotations.clear();
                self.selected = None;
                self.editing_label = None;
                self.active_drag = None;
                self.next_id = 1;
                self.status = format!("{} × {}  •  READY", width, height);
            }
            Err(error) => self.status = format!("COULD NOT OPEN IMAGE: {error}"),
        }
    }

    pub fn save_dialog(&mut self) {
        let Some(image) = &self.image else {
            self.status = "OPEN AN IMAGE BEFORE SAVING".into();
            return;
        };

        let default_name = image
            .path
            .file_stem()
            .and_then(|name| name.to_str())
            .map(|name| format!("{name}.annotations.json"))
            .unwrap_or_else(|| "annotations.json".into());

        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(default_name)
            .add_filter("JSON", &["json"])
            .save_file()
        {
            self.save_to(&path);
        }
    }

    pub fn save_to(&mut self, path: &Path) {
        let Some(image) = &self.image else { return };
        let data = AnnotationFile {
            image: image.path.to_string_lossy().into_owned(),
            image_width: image.width,
            image_height: image.height,
            annotations: &self.annotations,
        };
        match serde_json::to_string_pretty(&data)
            .map_err(|error| error.to_string())
            .and_then(|json| std::fs::write(path, json).map_err(|error| error.to_string()))
        {
            Ok(()) => self.status = format!("SAVED  •  {}", path.display()),
            Err(error) => self.status = format!("SAVE FAILED: {error}"),
        }
    }

    pub fn delete_selected(&mut self) {
        if let Some(id) = self.selected.take() {
            self.annotations.retain(|annotation| annotation.id != id);
            self.editing_label = None;
            self.active_drag = None;
            self.status = "ANNOTATION DELETED".into();
        }
    }

    pub fn shortcuts_and_drops(&mut self, ctx: &egui::Context) {
        let (open, save, delete, escape, dropped) = ctx.input(|input| {
            (
                (input.modifiers.command || input.modifiers.ctrl) && input.key_pressed(Key::O),
                (input.modifiers.command || input.modifiers.ctrl) && input.key_pressed(Key::S),
                input.key_pressed(Key::Delete) || input.key_pressed(Key::Backspace),
                input.key_pressed(Key::Escape),
                input.raw.dropped_files.clone(),
            )
        });

        if open {
            self.open_dialog(ctx);
        }
        if save {
            self.save_dialog();
        }
        if delete && !ctx.wants_keyboard_input() {
            self.delete_selected();
        }
        if escape {
            self.draft = None;
            self.active_drag = None;
            self.editing_label = None;
            self.selected = None;
        }
        if let Some(path) = dropped.into_iter().find_map(|file| file.path) {
            self.load_image(ctx, path);
        }
    }
}

impl eframe::App for AnnotatorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.shortcuts_and_drops(ctx);
        render_right_sidebar(self, ctx);
        render_canvas(self, ctx);
    }
}
