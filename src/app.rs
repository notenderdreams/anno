use std::path::{Path, PathBuf};
use eframe::egui::{self, Key};

use crate::canvas::render_canvas;
use crate::geometry::update_hierarchy;
use crate::history::{AppSnapshot, History};
use crate::menubar::{handle_native_menu_events, NativeMenuBar};
use crate::models::{
    export_annotation_tree, ActiveDrag, Annotation, AnnotationFile, Draft, LoadedImage, ProjectFile,
};
use crate::sidebar_left::render_left_sidebar;
use crate::sidebar_right::render_right_sidebar;
use crate::theme::configure_style;

fn resolve_image_path(anno_path: &Path, image_str: &str) -> PathBuf {
    let raw_path = PathBuf::from(image_str);
    if raw_path.exists() {
        return raw_path;
    }

    if let Some(parent) = anno_path.parent() {
        let relative = parent.join(&raw_path);
        if relative.exists() {
            return relative;
        }

        if let Some(file_name) = raw_path.file_name() {
            let in_same_dir = parent.join(file_name);
            if in_same_dir.exists() {
                return in_same_dir;
            }
        }
    }

    raw_path
}

pub struct AnnotatorApp {
    pub image: Option<LoadedImage>,
    pub annotations: Vec<Annotation>,
    pub selected: Option<u32>,
    pub editing_label: Option<u32>,
    pub next_id: u32,
    pub draft: Option<Draft>,
    pub active_drag: Option<ActiveDrag>,
    pub zoom: f32,
    pub pan: egui::Vec2,
    pub status: String,
    pub request_label_focus: bool,
    pub native_menubar: Option<NativeMenuBar>,
    pub project_description: Option<String>,
    pub history: History,
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
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            status: "OPEN AN IMAGE OR PROJECT TO BEGIN".into(),
            request_label_focus: false,
            native_menubar: Some(NativeMenuBar::new()),
            project_description: None,
            history: History::new(),
        }
    }

    pub fn open_dialog(&mut self, ctx: &egui::Context) {
        self.open_image_dialog(ctx);
    }

    pub fn open_image_dialog(&mut self, ctx: &egui::Context) {
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

    pub fn open_project_dialog(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Anno Project (*.anno)", &["anno"])
            .add_filter("JSON File (*.json)", &["json"])
            .pick_file()
        {
            self.load_project(ctx, &path);
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
                self.zoom = 1.0;
                self.pan = egui::Vec2::ZERO;
                self.next_id = 1;
                self.clear_history();
                self.status = format!("{} × {}  •  READY", width, height);
            }
            Err(error) => self.status = format!("COULD NOT OPEN IMAGE: {error}"),
        }
    }

    pub fn load_project(&mut self, ctx: &egui::Context, path: &Path) {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(error) => {
                self.status = format!("COULD NOT READ PROJECT FILE: {error}");
                return;
            }
        };

        let project: ProjectFile = match serde_json::from_str(&content) {
            Ok(p) => p,
            Err(error) => {
                self.status = format!("INVALID PROJECT FILE: {error}");
                return;
            }
        };

        let img_path = resolve_image_path(path, &project.image);
        match image::open(&img_path) {
            Ok(decoded) => {
                let rgba = decoded.to_rgba8();
                let (width, height) = rgba.dimensions();
                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                    [width as usize, height as usize],
                    rgba.as_raw(),
                );
                let texture = ctx.load_texture(
                    img_path.to_string_lossy(),
                    color_image,
                    egui::TextureOptions::LINEAR,
                );
                self.image = Some(LoadedImage {
                    texture,
                    path: img_path,
                    width,
                    height,
                });
                self.project_description = project.description;
                self.annotations = project.annotations;
                update_hierarchy(&mut self.annotations);
                let max_id = self.annotations.iter().map(|a| a.id).max().unwrap_or(0);
                self.next_id = project.next_id.max(max_id + 1);
                self.selected = None;
                self.editing_label = None;
                self.active_drag = None;
                self.zoom = 1.0;
                self.pan = egui::Vec2::ZERO;
                self.clear_history();
                self.status = format!("PROJECT LOADED  •  {}", path.display());
            }
            Err(error) => {
                self.status = format!("COULD NOT OPEN IMAGE ({}): {error}", img_path.display());
            }
        }
    }

    pub fn current_snapshot(&self) -> AppSnapshot {
        AppSnapshot {
            annotations: self.annotations.clone(),
            selected: self.selected,
            next_id: self.next_id,
        }
    }

    pub fn apply_snapshot(&mut self, snapshot: AppSnapshot) {
        self.annotations = snapshot.annotations;
        self.selected = snapshot.selected;
        self.next_id = snapshot.next_id;
        update_hierarchy(&mut self.annotations);
        self.editing_label = None;
        self.active_drag = None;
        self.draft = None;
    }

    pub fn undo(&mut self) {
        let current = self.current_snapshot();
        if let Some(snapshot) = self.history.undo(current) {
            self.apply_snapshot(snapshot);
            self.status = "UNDO".into();
        }
    }

    pub fn redo(&mut self) {
        let current = self.current_snapshot();
        if let Some(snapshot) = self.history.redo(current) {
            self.apply_snapshot(snapshot);
            self.status = "REDO".into();
        }
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    pub fn save_dialog(&mut self) {
        self.export_dialog();
    }

    pub fn save_project_dialog(&mut self) {
        let Some(image) = &self.image else {
            self.status = "OPEN AN IMAGE BEFORE SAVING PROJECT".into();
            return;
        };

        let default_name = image
            .path
            .file_stem()
            .and_then(|name| name.to_str())
            .map(|name| format!("{name}.anno"))
            .unwrap_or_else(|| "project.anno".into());

        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(default_name)
            .add_filter("Anno Project (*.anno)", &["anno"])
            .save_file()
        {
            self.save_project_to(&path);
        }
    }

    pub fn save_project_to(&mut self, path: &Path) {
        let Some(image) = &self.image else { return };
        let project = ProjectFile {
            image: image.path.to_string_lossy().into_owned(),
            image_width: image.width,
            image_height: image.height,
            description: self.project_description.clone(),
            next_id: self.next_id,
            annotations: self.annotations.clone(),
        };
        match serde_json::to_string_pretty(&project)
            .map_err(|error| error.to_string())
            .and_then(|json| std::fs::write(path, json).map_err(|error| error.to_string()))
        {
            Ok(()) => self.status = format!("PROJECT SAVED  •  {}", path.display()),
            Err(error) => self.status = format!("SAVE FAILED: {error}"),
        }
    }

    pub fn export_dialog(&mut self) {
        let Some(image) = &self.image else {
            self.status = "OPEN AN IMAGE BEFORE EXPORTING".into();
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
            .add_filter("JSON File (*.json)", &["json"])
            .save_file()
        {
            self.export_to(&path);
        }
    }

    pub fn export_to(&mut self, path: &Path) {
        let Some(image) = &self.image else { return };
        let data = AnnotationFile {
            image: image.path.to_string_lossy().into_owned(),
            image_width: image.width,
            image_height: image.height,
            annotations: export_annotation_tree(&self.annotations),
        };
        match serde_json::to_string_pretty(&data)
            .map_err(|error| error.to_string())
            .and_then(|json| std::fs::write(path, json).map_err(|error| error.to_string()))
        {
            Ok(()) => self.status = format!("EXPORTED  •  {}", path.display()),
            Err(error) => self.status = format!("EXPORT FAILED: {error}"),
        }
    }

    pub fn save_to(&mut self, path: &Path) {
        self.export_to(path);
    }

    pub fn delete_selected(&mut self) {
        if let Some(id) = self.selected {
            self.history.record(self.current_snapshot());
            self.selected = None;
            self.annotations.retain(|annotation| annotation.id != id);
            update_hierarchy(&mut self.annotations);
            self.editing_label = None;
            self.active_drag = None;
            self.status = "ANNOTATION DELETED".into();
        }
    }

    pub fn shortcuts_and_drops(&mut self, ctx: &egui::Context) {
        let (open_img, open_proj, save_proj, export_json, undo, redo, delete, escape, dropped) = ctx.input(|input| {
            let cmd_or_ctrl = input.modifiers.command || input.modifiers.ctrl;
            let shift = input.modifiers.shift;
            (
                cmd_or_ctrl && !shift && input.key_pressed(Key::O),
                cmd_or_ctrl && shift && input.key_pressed(Key::O),
                cmd_or_ctrl && input.key_pressed(Key::S),
                cmd_or_ctrl && input.key_pressed(Key::E),
                cmd_or_ctrl && !shift && input.key_pressed(Key::Z),
                (cmd_or_ctrl && shift && input.key_pressed(Key::Z))
                    || (cmd_or_ctrl && input.key_pressed(Key::Y)),
                input.key_pressed(Key::Delete) || input.key_pressed(Key::Backspace),
                input.key_pressed(Key::Escape),
                input.raw.dropped_files.clone(),
            )
        });

        if open_img {
            self.open_image_dialog(ctx);
        }
        if open_proj {
            self.open_project_dialog(ctx);
        }
        if save_proj {
            self.save_project_dialog();
        }
        if export_json {
            self.export_dialog();
        }
        if !ctx.wants_keyboard_input() {
            if redo {
                self.redo();
            } else if undo {
                self.undo();
            } else if delete {
                self.delete_selected();
            }
        }
        if escape {
            self.draft = None;
            self.active_drag = None;
            self.editing_label = None;
            self.selected = None;
        }
        if let Some(path) = dropped.into_iter().find_map(|file| file.path) {
            if path.extension().and_then(|ext| ext.to_str()) == Some("anno") {
                self.load_project(ctx, &path);
            } else {
                self.load_image(ctx, path);
            }
        }
    }
}

impl eframe::App for AnnotatorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        handle_native_menu_events(self, ctx);
        self.shortcuts_and_drops(ctx);
        render_left_sidebar(self, ctx);
        render_right_sidebar(self, ctx);
        render_canvas(self, ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app() -> AnnotatorApp {
        AnnotatorApp {
            image: None,
            annotations: Vec::new(),
            selected: None,
            editing_label: None,
            next_id: 1,
            draft: None,
            active_drag: None,
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            status: String::new(),
            request_label_focus: false,
            native_menubar: None,
            project_description: None,
            history: History::new(),
        }
    }

    fn sample_annotation(id: u32) -> Annotation {
        Annotation {
            id,
            label: format!("region_{id}"),
            description: None,
            x: 10.0 * id as f32,
            y: 10.0 * id as f32,
            width: 50.0,
            height: 50.0,
            color: [255, 0, 0],
            parent_id: None,
        }
    }

    #[test]
    fn test_undo_redo_basic() {
        let mut app = test_app();
        assert!(!app.can_undo());
        assert!(!app.can_redo());

        // Snapshot initial empty state and add annotation 1
        app.history.record(app.current_snapshot());
        app.annotations.push(sample_annotation(1));
        app.selected = Some(1);
        app.next_id = 2;

        assert!(app.can_undo());
        assert!(!app.can_redo());
        assert_eq!(app.annotations.len(), 1);

        // Undo
        app.undo();
        assert_eq!(app.annotations.len(), 0);
        assert_eq!(app.selected, None);
        assert!(!app.can_undo());
        assert!(app.can_redo());

        // Redo
        app.redo();
        assert_eq!(app.annotations.len(), 1);
        assert_eq!(app.selected, Some(1));
        assert!(app.can_undo());
        assert!(!app.can_redo());
    }

    #[test]
    fn test_undo_redo_delete_selected() {
        let mut app = test_app();
        app.annotations.push(sample_annotation(1));
        app.selected = Some(1);

        app.delete_selected();
        assert_eq!(app.annotations.len(), 0);
        assert_eq!(app.selected, None);
        assert!(app.can_undo());

        app.undo();
        assert_eq!(app.annotations.len(), 1);
        assert_eq!(app.annotations[0].id, 1);
        assert_eq!(app.selected, Some(1));
    }

    #[test]
    fn test_redo_stack_cleared_on_new_action() {
        let mut app = test_app();
        app.history.record(app.current_snapshot());
        app.annotations.push(sample_annotation(1));

        app.undo();
        assert!(app.can_redo());

        // Perform a new mutation
        app.history.record(app.current_snapshot());
        app.annotations.push(sample_annotation(2));

        assert!(!app.can_redo());
    }
}
