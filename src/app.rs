use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use eframe::egui::{self, Key};

use crate::bottom_bar::render_bottom_bar;
use crate::canvas::render_canvas;
use crate::dataset::{check_sidecar_annotation_count, scan_image_folder};
use crate::geometry::update_hierarchy;
use crate::history::{AppSnapshot, History};
use crate::menubar::{handle_native_menu_events, NativeMenuBar};
use crate::models::{
    assign_preset_to_annotations, default_presets, export_annotation_tree, next_category_label,
    ActiveDrag, Annotation, AnnotationFile, BatchProjectFile, ClassPreset, Draft, DraftPolygon,
    FilmstripFilter, LoadedImage, ProjectFile, ToolMode, UnifiedDatasetExport, UnifiedImageExport,
};
use crate::sidebar_left::render_left_sidebar;
use crate::sidebar_right::render_right_sidebar;
use crate::theme::configure_style;
use crate::thumbnail_loader::BackgroundLoader;

fn resolve_image_path(anno_path: &Path, image_str: &str) -> PathBuf {
    let raw_path = PathBuf::from(image_str);
    if raw_path.exists() {
        return raw_path;
    }

    let candidate = anno_path.parent().unwrap_or(Path::new("")).join(&raw_path);
    if candidate.exists() {
        return candidate;
    }

    raw_path
}

pub struct AnnotatorApp {
    pub image: Option<LoadedImage>,
    pub annotations: Vec<Annotation>,
    pub selected: HashSet<u32>,
    pub editing_label: Option<u32>,
    pub next_id: u32,
    pub tool_mode: ToolMode,
    pub draft: Option<Draft>,
    pub draft_polygon: Option<DraftPolygon>,
    pub marquee: Option<Draft>,
    pub active_drag: Option<ActiveDrag>,
    pub zoom: f32,
    pub pan: egui::Vec2,
    pub status: String,
    pub request_label_focus: bool,
    pub native_menubar: Option<NativeMenuBar>,
    pub project_description: Option<String>,
    pub history: History,
    pub dataset_folder: Option<PathBuf>,
    pub image_files: Vec<PathBuf>,
    pub current_image_idx: Option<usize>,
    pub pending_image_idx: Option<usize>,
    pub auto_save_dataset: bool,
    pub filmstrip_filter: FilmstripFilter,
    pub annotation_counts: HashMap<PathBuf, usize>,
    pub thumbnail_cache: HashMap<PathBuf, egui::TextureHandle>,
    pub image_cache: HashMap<PathBuf, LoadedImage>,
    pub annotations_cache: HashMap<PathBuf, (Vec<Annotation>, Option<String>, u32)>,
    pub loader: BackgroundLoader,
    pub presets: Vec<ClassPreset>,
    pub active_preset_idx: usize,
    pub autocomplete_nav: Option<usize>,
}

impl AnnotatorApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_style(&cc.egui_ctx);
        Self {
            image: None,
            annotations: Vec::new(),
            selected: HashSet::new(),
            editing_label: None,
            next_id: 1,
            tool_mode: ToolMode::Rectangle,
            draft: None,
            draft_polygon: None,
            marquee: None,
            active_drag: None,
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            status: "READY".into(),
            request_label_focus: false,
            native_menubar: Some(NativeMenuBar::new()),
            project_description: None,
            history: History::new(),
            dataset_folder: None,
            image_files: Vec::new(),
            current_image_idx: None,
            pending_image_idx: None,
            auto_save_dataset: true,
            filmstrip_filter: FilmstripFilter::All,
            annotation_counts: HashMap::new(),
            thumbnail_cache: HashMap::new(),
            image_cache: HashMap::new(),
            annotations_cache: HashMap::new(),
            loader: BackgroundLoader::new(),
            presets: default_presets(),
            active_preset_idx: 0,
            autocomplete_nav: None,
        }
    }
}

impl Default for AnnotatorApp {
    fn default() -> Self {
        Self {
            image: None,
            annotations: Vec::new(),
            selected: HashSet::new(),
            editing_label: None,
            next_id: 1,
            tool_mode: ToolMode::Rectangle,
            draft: None,
            draft_polygon: None,
            marquee: None,
            active_drag: None,
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            status: "READY".into(),
            request_label_focus: false,
            native_menubar: None,
            project_description: None,
            history: History::new(),
            dataset_folder: None,
            image_files: Vec::new(),
            current_image_idx: None,
            pending_image_idx: None,
            auto_save_dataset: true,
            filmstrip_filter: FilmstripFilter::All,
            annotation_counts: HashMap::new(),
            thumbnail_cache: HashMap::new(),
            image_cache: HashMap::new(),
            annotations_cache: HashMap::new(),
            loader: BackgroundLoader::new(),
            presets: default_presets(),
            active_preset_idx: 0,
            autocomplete_nav: None,
        }
    }
}

impl AnnotatorApp {

    pub fn finish_draft_polygon(&mut self) -> bool {
        let Some(poly) = self.draft_polygon.take() else {
            return false;
        };
        if poly.points.len() < 3 {
            return false;
        }

        self.history.record(self.current_snapshot());

        let (x, y, w, h) = crate::geometry::polygon_bounding_box(&poly.points);
        let id = self.next_id;
        self.next_id += 1;

        let (prefix, color) = if let Some(preset) = self.presets.get(self.active_preset_idx) {
            (preset.prefix.clone(), preset.color)
        } else {
            ("object".to_string(), [255, 0, 0])
        };

        let label = next_category_label(&prefix, &self.annotations, None);
        let points_array: Vec<[f32; 2]> = poly
            .points
            .iter()
            .map(|p| [p.x.round(), p.y.round()])
            .collect();

        self.annotations.push(Annotation {
            id,
            label,
            description: None,
            x: x.round(),
            y: y.round(),
            width: w.round(),
            height: h.round(),
            color,
            parent_id: None,
            locked: false,
            points: Some(points_array),
        });

        self.select_single(id);
        self.editing_label = None;
        self.request_label_focus = false;
        self.status = format!("POLYGON REGION {id:02} CREATED");
        update_hierarchy(&mut self.annotations);
        true
    }

    pub fn active_preset(&self) -> Option<&ClassPreset> {
        self.presets.get(self.active_preset_idx)
    }

    pub fn apply_preset(&mut self, idx: usize) {
        if idx >= self.presets.len() {
            return;
        }
        self.active_preset_idx = idx;
        let preset = self.presets[idx].clone();

        if !self.selected.is_empty() {
            self.history.record(self.current_snapshot());
            let count = assign_preset_to_annotations(&mut self.annotations, &self.selected, &preset);
            let prefix_upper = preset.prefix.to_uppercase();
            self.status = format!("PRESET {}: {} APPLIED TO {} REGION(S)", idx + 1, prefix_upper, count);
        } else {
            let prefix_upper = preset.prefix.to_uppercase();
            self.status = format!("ACTIVE PRESET: {} (KEY {})", prefix_upper, idx + 1);
        }
    }

    pub fn is_selected(&self, id: u32) -> bool {
        self.selected.contains(&id)
    }

    pub fn select_single(&mut self, id: u32) {
        self.selected.clear();
        self.selected.insert(id);
    }

    pub fn toggle_select(&mut self, id: u32) {
        if self.selected.contains(&id) {
            self.selected.remove(&id);
        } else {
            self.selected.insert(id);
        }
    }

    pub fn select_all(&mut self) {
        self.selected = self.annotations.iter().map(|a| a.id).collect();
        if self.selected.is_empty() {
            self.status = "NO REGIONS TO SELECT".into();
        } else {
            self.status = format!("{} REGION(S) SELECTED", self.selected.len());
        }
    }

    pub fn deselect_all(&mut self) {
        self.selected.clear();
        self.editing_label = None;
    }

    pub fn nudge_selected(&mut self, dx: f32, dy: f32) -> bool {
        if self.selected.is_empty() {
            return false;
        }

        let active_ids: Vec<u32> = self
            .annotations
            .iter()
            .filter(|a| self.selected.contains(&a.id) && !a.locked)
            .map(|a| a.id)
            .collect();

        if active_ids.is_empty() {
            self.status = "LOCKED REGION(S) CANNOT BE MOVED".into();
            return false;
        }

        let mut min_dx = -f32::INFINITY;
        let mut max_dx = f32::INFINITY;
        let mut min_dy = -f32::INFINITY;
        let mut max_dy = f32::INFINITY;

        if let Some(image) = &self.image {
            let img_w = image.width as f32;
            let img_h = image.height as f32;
            for &id in &active_ids {
                if let Some(a) = self.annotations.iter().find(|a| a.id == id) {
                    min_dx = min_dx.max(-a.x);
                    max_dx = max_dx.min(img_w - (a.x + a.width));
                    min_dy = min_dy.max(-a.y);
                    max_dy = max_dy.min(img_h - (a.y + a.height));
                }
            }
        }

        let clamped_dx = if min_dx <= max_dx {
            dx.clamp(min_dx, max_dx)
        } else {
            0.0
        };
        let clamped_dy = if min_dy <= max_dy {
            dy.clamp(min_dy, max_dy)
        } else {
            0.0
        };

        if clamped_dx == 0.0 && clamped_dy == 0.0 {
            return false;
        }

        self.history.record(self.current_snapshot());

        for &id in &active_ids {
            if let Some(a) = self.annotations.iter_mut().find(|a| a.id == id) {
                a.x = (a.x + clamped_dx).round();
                a.y = (a.y + clamped_dy).round();
                if let Some(pts) = &mut a.points {
                    for p in pts.iter_mut() {
                        p[0] = (p[0] + clamped_dx).round();
                        p[1] = (p[1] + clamped_dy).round();
                    }
                }
            }
        }

        update_hierarchy(&mut self.annotations);
        self.status = if active_ids.len() == 1 {
            "REGION NUDGED".into()
        } else {
            format!("{} REGIONS NUDGED", active_ids.len())
        };
        true
    }

    pub fn toggle_lock_annotation(&mut self, id: u32) {
        if let Some(annotation) = self.annotations.iter().find(|a| a.id == id) {
            let new_locked = !annotation.locked;
            self.history.record(self.current_snapshot());
            if let Some(a) = self.annotations.iter_mut().find(|a| a.id == id) {
                a.locked = new_locked;
            }
            let state = if new_locked { "LOCKED" } else { "UNLOCKED" };
            self.status = format!("REGION {:02} {}", id, state);
        }
    }

    pub fn toggle_lock_selected(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        self.history.record(self.current_snapshot());
        let all_locked = self
            .annotations
            .iter()
            .filter(|a| self.selected.contains(&a.id))
            .all(|a| a.locked);
        let new_state = !all_locked;

        for annotation in self
            .annotations
            .iter_mut()
            .filter(|a| self.selected.contains(&a.id))
        {
            annotation.locked = new_state;
        }

        let action_str = if new_state { "LOCKED" } else { "UNLOCKED" };
        self.status = format!("{} REGION(S) {}", self.selected.len(), action_str);
    }

    pub fn request_thumbnail(&mut self, path: &Path) {
        if !self.thumbnail_cache.contains_key(path) {
            self.loader.request_thumbnail(path);
        }
    }

    fn poll_background_loads(&mut self, ctx: &egui::Context) {
        let failures =
            self.loader
                .poll_results(ctx, &mut self.thumbnail_cache, &mut self.image_cache);
        self.prune_image_cache();

        let Some(pending_idx) = self.pending_image_idx else {
            return;
        };
        let pending_path = self.image_files.get(pending_idx).cloned();

        if pending_path
            .as_ref()
            .is_some_and(|path| self.image_cache.contains_key(path))
        {
            self.pending_image_idx = None;
            if self.current_image_idx == Some(pending_idx) {
                if let Some(image) = pending_path
                    .as_ref()
                    .and_then(|path| self.image_cache.get(path))
                {
                    self.image = Some(image.clone());
                }
            } else {
                self.switch_to_image_index(ctx, pending_idx);
            }
        } else if let Some((path, error)) = failures
            .into_iter()
            .find(|(path, _)| pending_path.as_ref().is_some_and(|pending| pending == path))
        {
            self.pending_image_idx = None;
            self.status = format!("COULD NOT OPEN IMAGE ({}): {error}", path.display());
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    fn prune_image_cache(&mut self) {
        const CACHE_RADIUS: usize = 2;

        let mut keep = HashSet::new();
        for center in [self.current_image_idx, self.pending_image_idx]
            .into_iter()
            .flatten()
        {
            let start = center.saturating_sub(CACHE_RADIUS);
            let end = (center + CACHE_RADIUS + 1).min(self.image_files.len());
            keep.extend(self.image_files[start..end].iter().cloned());
        }

        self.image_cache.retain(|path, _| keep.contains(path));
    }

    pub fn preload_adjacent_images(&mut self) {
        let Some(idx) = self.current_image_idx else { return };
        let total = self.image_files.len();

        let targets = [
            if idx + 1 < total { Some(idx + 1) } else { None },
            if idx + 2 < total { Some(idx + 2) } else { None },
            if idx > 0 { Some(idx - 1) } else { None },
        ];

        for target_idx in targets.into_iter().flatten() {
            let path = &self.image_files[target_idx];
            if !self.image_cache.contains_key(path) {
                self.loader.request_preload_image(path);
            }
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

    pub fn open_folder_dialog(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            self.load_folder(ctx, path);
        }
    }

    pub fn open_project_dialog(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Anno Batch (*.annobatch)", &["annobatch"])
            .add_filter("Anno Project (*.anno)", &["anno"])
            .add_filter("JSON File (*.json)", &["json"])
            .pick_file()
        {
            self.load_saved_file(ctx, &path);
        }
    }

    pub fn load_saved_file(&mut self, ctx: &egui::Context, path: &Path) {
        let is_batch = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("annobatch"));
        if is_batch {
            self.load_batch(ctx, path);
        } else {
            self.load_project(ctx, path);
        }
    }

    pub fn load_folder(&mut self, ctx: &egui::Context, folder: PathBuf) {
        self.auto_save_current_image();

        let files = scan_image_folder(&folder);
        if files.is_empty() {
            self.status = format!("NO IMAGES FOUND IN: {}", folder.display());
            return;
        }

        self.thumbnail_cache.clear();
        self.image_cache.clear();
        self.annotations_cache.clear();
        self.loader.clear();
        self.pending_image_idx = None;
        self.current_image_idx = None;
        self.dataset_folder = Some(folder.clone());
        self.image_files = files;
        self.refresh_annotation_counts();
        self.switch_to_image_index(ctx, 0);
    }

    pub fn refresh_annotation_counts(&mut self) {
        self.annotation_counts.clear();
        for path in &self.image_files {
            if let Some(count) = check_sidecar_annotation_count(path) {
                self.annotation_counts.insert(path.clone(), count);
            }
        }
    }

    pub fn switch_to_image_index(&mut self, ctx: &egui::Context, new_index: usize) {
        if self.image_files.is_empty() || new_index >= self.image_files.len() {
            return;
        }

        let path = self.image_files[new_index].clone();
        let target_is_loaded = self.image.as_ref().is_some_and(|image| image.path == path);
        if self.current_image_idx == Some(new_index) && target_is_loaded {
            if let Some(image) = self.image_cache.get(&path) {
                self.image = Some(image.clone());
                self.pending_image_idx = None;
            }
            return;
        }

        if !self.image_cache.contains_key(&path) {
            self.auto_save_current_image();
            self.current_image_idx = Some(new_index);
            self.image = self.thumbnail_cache.get(&path).and_then(|texture| {
                image::image_dimensions(&path)
                    .ok()
                    .map(|(width, height)| LoadedImage {
                        texture: texture.clone(),
                        path: path.clone(),
                        width,
                        height,
                    })
            });
            self.load_image_state(&path);
            self.loader.request_navigation_image(&path);
            self.pending_image_idx = Some(new_index);
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
            return;
        }

        self.auto_save_current_image();
        self.pending_image_idx = None;
        self.current_image_idx = Some(new_index);
        self.load_image_internal(ctx, path);
    }

    pub fn image_annotation_count(&self, path: &Path) -> usize {
        if let Some(current_img) = &self.image {
            if current_img.path == path {
                return self.annotations.len();
            }
        }
        self.annotation_counts.get(path).copied().unwrap_or(0)
    }

    pub fn filtered_image_indices(&self) -> Vec<usize> {
        match self.filmstrip_filter {
            FilmstripFilter::All => (0..self.image_files.len()).collect(),
            FilmstripFilter::Annotated => (0..self.image_files.len())
                .filter(|&idx| {
                    let path = &self.image_files[idx];
                    self.image_annotation_count(path) > 0
                })
                .collect(),
            FilmstripFilter::Unannotated => (0..self.image_files.len())
                .filter(|&idx| {
                    let path = &self.image_files[idx];
                    self.image_annotation_count(path) == 0
                })
                .collect(),
        }
    }

    pub fn next_image(&mut self, ctx: &egui::Context) {
        let indices = self.filtered_image_indices();
        if indices.is_empty() {
            return;
        }
        if let Some(curr) = self.current_image_idx {
            if let Some(&next) = indices.iter().find(|&&idx| idx > curr) {
                self.switch_to_image_index(ctx, next);
            }
        } else if let Some(&first) = indices.first() {
            self.switch_to_image_index(ctx, first);
        }
    }

    pub fn previous_image(&mut self, ctx: &egui::Context) {
        let indices = self.filtered_image_indices();
        if indices.is_empty() {
            return;
        }
        if let Some(curr) = self.current_image_idx {
            if let Some(&prev) = indices.iter().rev().find(|&&idx| idx < curr) {
                self.switch_to_image_index(ctx, prev);
            }
        } else if let Some(&last) = indices.last() {
            self.switch_to_image_index(ctx, last);
        }
    }

    pub fn auto_save_current_image(&mut self) {
        let Some(image) = &self.image else { return };
        self.annotations_cache.insert(
            image.path.clone(),
            (
                self.annotations.clone(),
                self.project_description.clone(),
                self.next_id,
            ),
        );

        if !self.auto_save_dataset {
            return;
        }

        let anno_path = image.path.with_extension("anno");
        if self.annotations.is_empty() {
            if anno_path.exists() {
                let _ = std::fs::remove_file(&anno_path);
            }
            self.annotation_counts.remove(&image.path);
            return;
        }

        let project = ProjectFile {
            image: image.path.to_string_lossy().into_owned(),
            image_width: image.width,
            image_height: image.height,
            description: self.project_description.clone(),
            next_id: self.next_id,
            annotations: self.annotations.clone(),
            presets: self.presets.clone(),
        };

        if let Ok(json) = serde_json::to_string_pretty(&project) {
            let _ = std::fs::write(&anno_path, json);
            self.annotation_counts.insert(image.path.clone(), self.annotations.len());
        }
    }

    pub fn load_image(&mut self, ctx: &egui::Context, path: PathBuf) {
        self.pending_image_idx = None;
        self.auto_save_current_image();

        self.dataset_folder = None;
        self.image_files.clear();
        self.current_image_idx = None;
        self.annotation_counts.clear();

        self.load_image_internal(ctx, path);
    }

    fn load_image_internal(&mut self, ctx: &egui::Context, path: PathBuf) {
        if let Some(cached) = self.image_cache.get(&path) {
            self.image = Some(cached.clone());
        } else {
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
                    let loaded = LoadedImage {
                        texture,
                        path: path.clone(),
                        width,
                        height,
                    };
                    self.image_cache.insert(path.clone(), loaded.clone());
                    self.image = Some(loaded);
                }
                Err(error) => {
                    self.status = format!("COULD NOT OPEN IMAGE: {error}");
                    return;
                }
            }
        }

        self.load_image_state(&path);
    }

    fn load_image_state(&mut self, path: &Path) {
        if let Some((cached_annos, desc, next_id)) = self.annotations_cache.get(path) {
            self.annotations = cached_annos.clone();
            self.project_description = desc.clone();
            self.next_id = *next_id;
            update_hierarchy(&mut self.annotations);
        } else {
            let anno_path = path.with_extension("anno");
            if anno_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&anno_path) {
                    if let Ok(project) = serde_json::from_str::<ProjectFile>(&content) {
                        self.project_description = project.description;
                        self.annotations = project.annotations;
                        update_hierarchy(&mut self.annotations);
                        let max_id = self.annotations.iter().map(|a| a.id).max().unwrap_or(0);
                        self.next_id = project.next_id.max(max_id + 1);
                    } else {
                        self.annotations.clear();
                        self.project_description = None;
                        self.next_id = 1;
                    }
                } else {
                    self.annotations.clear();
                    self.project_description = None;
                    self.next_id = 1;
                }
            } else {
                self.annotations.clear();
                self.project_description = None;
                self.next_id = 1;
            }
            self.annotations_cache.insert(
                path.to_path_buf(),
                (
                    self.annotations.clone(),
                    self.project_description.clone(),
                    self.next_id,
                ),
            );
        }

        self.annotation_counts
            .insert(path.to_path_buf(), self.annotations.len());
        self.selected.clear();
        self.marquee = None;
        self.editing_label = None;
        self.active_drag = None;
        self.zoom = 1.0;
        self.pan = egui::Vec2::ZERO;
        self.clear_history();

        self.preload_adjacent_images();

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("image");

        if let (Some(idx), Some(img)) = (self.current_image_idx, &self.image) {
            let total = self.image_files.len();
            self.status = format!(
                "IMAGE {:02}/{:02}  •  {}  •  {} × {}",
                idx + 1,
                total,
                file_name,
                img.width,
                img.height
            );
        }
    }

    pub fn load_project(&mut self, ctx: &egui::Context, path: &Path) {
        self.pending_image_idx = None;
        self.dataset_folder = None;
        self.image_files.clear();
        self.current_image_idx = None;
        self.annotation_counts.clear();
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
                if !project.presets.is_empty() {
                    self.presets = project.presets;
                }
                update_hierarchy(&mut self.annotations);
                let max_id = self.annotations.iter().map(|a| a.id).max().unwrap_or(0);
                self.next_id = project.next_id.max(max_id + 1);
                self.selected.clear();
                self.marquee = None;
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

    pub fn load_batch(&mut self, ctx: &egui::Context, path: &Path) {
        let content = match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) => {
                self.status = format!("COULD NOT READ BATCH: {error}");
                return;
            }
        };

        let batch: BatchProjectFile = match serde_json::from_str(&content) {
            Ok(batch) => batch,
            Err(error) => {
                self.status = format!("INVALID ANNO BATCH: {error}");
                return;
            }
        };

        if batch.format != "annobatch" {
            self.status = "INVALID ANNO BATCH FORMAT".into();
            return;
        }

        if batch.format_version != 1 {
            self.status = format!(
                "UNSUPPORTED ANNO BATCH VERSION: {}",
                batch.format_version
            );
            return;
        }

        if batch.images.is_empty() {
            self.status = "ANNO BATCH CONTAINS NO IMAGES".into();
            return;
        }

        self.auto_save_current_image();
        self.thumbnail_cache.clear();
        self.image_cache.clear();
        self.annotations_cache.clear();
        self.annotation_counts.clear();
        self.image_files.clear();
        self.loader.clear();

        let saved_dataset_folder = batch.dataset_folder.as_deref().map(PathBuf::from);
        let batch_parent = path.parent().unwrap_or_else(|| Path::new("."));

        for project in &batch.images {
            let raw_path = PathBuf::from(&project.image);
            let image_path = if raw_path.exists() {
                raw_path
            } else if let Some(folder) = &saved_dataset_folder {
                let candidate = folder.join(&raw_path);
                if candidate.exists() {
                    candidate
                } else {
                    batch_parent.join(&raw_path)
                }
            } else {
                batch_parent.join(&raw_path)
            };

            self.image_files.push(image_path.clone());
            self.annotation_counts
                .insert(image_path.clone(), project.annotations.len());
            let max_id = project
                .annotations
                .iter()
                .map(|annotation| annotation.id)
                .max()
                .unwrap_or(0);
            self.annotations_cache.insert(
                image_path,
                (
                    project.annotations.clone(),
                    project.description.clone(),
                    project.next_id.max(max_id + 1),
                ),
            );
        }

        if !batch.presets.is_empty() {
            self.presets = batch.presets;
        }
        self.dataset_folder = self
            .image_files
            .first()
            .and_then(|image_path| image_path.parent())
            .map(Path::to_path_buf);
        self.current_image_idx = None;
        self.pending_image_idx = None;
        let initial_idx = batch.current_image_idx.min(self.image_files.len() - 1);
        self.switch_to_image_index(ctx, initial_idx);
        self.status = format!(
            "BATCH LOADED  •  {}  •  {} IMAGES",
            path.display(),
            self.image_files.len()
        );
    }

    pub fn current_snapshot(&self) -> AppSnapshot {
        AppSnapshot {
            annotations: self.annotations.clone(),
            selected: self.selected.clone(),
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
        self.draft_polygon = None;
        self.marquee = None;
    }

    pub fn undo(&mut self) {
        if let Some(poly) = &mut self.draft_polygon {
            if poly.undo_point().is_some() {
                if poly.points.is_empty() {
                    self.draft_polygon = None;
                    self.status = "PEN TOOL DRAWING CANCELED".into();
                } else {
                    self.status = format!("PEN TOOL: POINT UNDONE ({} REMAINING)", poly.points.len());
                }
                return;
            }
        }

        self.editing_label = None;
        self.draft_polygon = None;
        self.draft = None;
        self.marquee = None;
        self.active_drag = None;

        let current = self.current_snapshot();
        if let Some(snapshot) = self.history.undo(current) {
            self.apply_snapshot(snapshot);
            self.status = "UNDO".into();
        }
    }

    pub fn redo(&mut self) {
        if let Some(poly) = &mut self.draft_polygon {
            if poly.redo_point().is_some() {
                self.status = format!("PEN TOOL: POINT REDONE ({} POINTS)", poly.points.len());
                return;
            }
        }

        self.editing_label = None;
        self.draft_polygon = None;
        self.draft = None;
        self.marquee = None;
        self.active_drag = None;

        let current = self.current_snapshot();
        if let Some(snapshot) = self.history.redo(current) {
            self.apply_snapshot(snapshot);
            self.status = "REDO".into();
        }
    }

    pub fn can_undo(&self) -> bool {
        self.draft_polygon.as_ref().map_or(false, |p| p.can_undo()) || self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.draft_polygon.as_ref().map_or(false, |p| p.can_redo()) || self.history.can_redo()
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    pub fn save_dialog(&mut self) {
        self.export_dialog();
    }

    pub fn save_project_dialog(&mut self) {
        if self.image_files.len() > 1 {
            self.save_batch_dialog();
            return;
        }

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

    pub fn save_batch_dialog(&mut self) {
        if self.image_files.is_empty() {
            self.status = "OPEN A DATASET BEFORE SAVING BATCH".into();
            return;
        }

        let dataset_name = self
            .dataset_folder
            .as_ref()
            .and_then(|folder| folder.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("dataset");
        let default_name = format!("{dataset_name}.annobatch");

        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(default_name)
            .add_filter("Anno Batch (*.annobatch)", &["annobatch"])
            .save_file()
        {
            self.save_batch_to(&path);
        }
    }

    pub fn save_batch_to(&mut self, path: &Path) {
        if self.image_files.is_empty() {
            return;
        }

        self.auto_save_current_image();
        let dataset_name = self
            .dataset_folder
            .as_ref()
            .and_then(|folder| folder.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("dataset")
            .to_string();

        let images = self
            .image_files
            .iter()
            .map(|image_path| {
                let (annotations, description, next_id) = if self
                    .image
                    .as_ref()
                    .is_some_and(|image| image.path == *image_path)
                {
                    (
                        self.annotations.clone(),
                        self.project_description.clone(),
                        self.next_id,
                    )
                } else if let Some(cached) = self.annotations_cache.get(image_path) {
                    cached.clone()
                } else {
                    let sidecar_path = image_path.with_extension("anno");
                    std::fs::read_to_string(sidecar_path)
                        .ok()
                        .and_then(|content| serde_json::from_str::<ProjectFile>(&content).ok())
                        .map(|project| {
                            (project.annotations, project.description, project.next_id)
                        })
                        .unwrap_or_else(|| (Vec::new(), None, 1))
                };

                let (image_width, image_height) = self
                    .image_cache
                    .get(image_path)
                    .map(|image| (image.width, image.height))
                    .or_else(|| {
                        self.image.as_ref().and_then(|image| {
                            (image.path == *image_path).then_some((image.width, image.height))
                        })
                    })
                    .unwrap_or_else(|| image::image_dimensions(image_path).unwrap_or((0, 0)));
                let stored_path = self
                    .dataset_folder
                    .as_ref()
                    .and_then(|folder| image_path.strip_prefix(folder).ok())
                    .unwrap_or(image_path)
                    .to_string_lossy()
                    .into_owned();

                ProjectFile {
                    image: stored_path,
                    image_width,
                    image_height,
                    description,
                    next_id,
                    annotations,
                    presets: Vec::new(),
                }
            })
            .collect();

        let batch = BatchProjectFile {
            format: "annobatch".into(),
            format_version: 1,
            dataset_name,
            dataset_folder: self
                .dataset_folder
                .as_ref()
                .map(|folder| folder.to_string_lossy().into_owned()),
            current_image_idx: self.current_image_idx.unwrap_or(0),
            images,
            presets: self.presets.clone(),
        };

        match serde_json::to_string_pretty(&batch)
            .map_err(|error| error.to_string())
            .and_then(|json| std::fs::write(path, json).map_err(|error| error.to_string()))
        {
            Ok(()) => self.status = format!("BATCH SAVED  •  {}", path.display()),
            Err(error) => self.status = format!("BATCH SAVE FAILED: {error}"),
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
            presets: self.presets.clone(),
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

    pub fn export_unified_dataset_dialog(&mut self) {
        if self.image_files.is_empty() {
            self.status = "NO DATASET LOADED TO EXPORT".into();
            return;
        }

        self.auto_save_current_image();

        let dataset_name = self
            .dataset_folder
            .as_ref()
            .and_then(|f| f.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("dataset");

        let default_name = format!("{dataset_name}.json");

        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(default_name)
            .add_filter("JSON Dataset (*.json)", &["json"])
            .save_file()
        {
            self.export_unified_dataset_to(&path);
        }
    }

    pub fn export_unified_dataset_to(&mut self, path: &Path) {
        if self.image_files.is_empty() {
            return;
        }

        self.auto_save_current_image();

        let dataset_name = self
            .dataset_folder
            .as_ref()
            .and_then(|f| f.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("dataset")
            .to_string();

        let mut images_data = Vec::new();
        let mut annotated_count = 0;

        for image_path in &self.image_files {
            let (width, height, annotations) = if let Some(current_img) = &self.image {
                if current_img.path == *image_path {
                    (current_img.width, current_img.height, self.annotations.clone())
                } else if let Some((cached_annos, _, _)) = self.annotations_cache.get(image_path) {
                    let (w, h) = if let Some(img) = self.image_cache.get(image_path) {
                        (img.width, img.height)
                    } else {
                        image::image_dimensions(image_path).unwrap_or((1920, 1080))
                    };
                    (w, h, cached_annos.clone())
                } else {
                    let anno_path = image_path.with_extension("anno");
                    if let Ok(content) = std::fs::read_to_string(&anno_path) {
                        if let Ok(proj) = serde_json::from_str::<ProjectFile>(&content) {
                            (proj.image_width, proj.image_height, proj.annotations)
                        } else {
                            let (w, h) = image::image_dimensions(image_path).unwrap_or((1920, 1080));
                            (w, h, Vec::new())
                        }
                    } else {
                        let (w, h) = image::image_dimensions(image_path).unwrap_or((1920, 1080));
                        (w, h, Vec::new())
                    }
                }
            } else if let Some((cached_annos, _, _)) = self.annotations_cache.get(image_path) {
                let (w, h) = if let Some(img) = self.image_cache.get(image_path) {
                    (img.width, img.height)
                } else {
                    image::image_dimensions(image_path).unwrap_or((1920, 1080))
                };
                (w, h, cached_annos.clone())
            } else {
                let anno_path = image_path.with_extension("anno");
                if let Ok(content) = std::fs::read_to_string(&anno_path) {
                    if let Ok(proj) = serde_json::from_str::<ProjectFile>(&content) {
                        (proj.image_width, proj.image_height, proj.annotations)
                    } else {
                        let (w, h) = image::image_dimensions(image_path).unwrap_or((1920, 1080));
                        (w, h, Vec::new())
                    }
                } else {
                    let (w, h) = image::image_dimensions(image_path).unwrap_or((1920, 1080));
                    (w, h, Vec::new())
                }
            };

            if !annotations.is_empty() {
                annotated_count += 1;
            }

            let file_name = image_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("image")
                .to_string();

            images_data.push((file_name, width, height, annotations));
        }

        let unified_images: Vec<UnifiedImageExport> = images_data
            .iter()
            .map(|(name, w, h, annos)| UnifiedImageExport {
                image: name.clone(),
                image_width: *w,
                image_height: *h,
                annotations: export_annotation_tree(annos),
            })
            .collect();

        let dataset_export = UnifiedDatasetExport {
            dataset_name,
            total_images: self.image_files.len(),
            annotated_images: annotated_count,
            images: unified_images,
        };

        match serde_json::to_string_pretty(&dataset_export)
            .map_err(|e| e.to_string())
            .and_then(|json| std::fs::write(path, json).map_err(|e| e.to_string()))
        {
            Ok(()) => self.status = format!("DATASET EXPORTED  •  {}", path.display()),
        Err(e) => self.status = format!("DATASET EXPORT FAILED: {e}"),
        }
    }

    pub fn save_to(&mut self, path: &Path) {
        self.export_to(path);
    }

    pub fn delete_selected(&mut self) {
        if !self.selected.is_empty() {
            let locked_count = self
                .annotations
                .iter()
                .filter(|a| self.selected.contains(&a.id) && a.locked)
                .count();

            if locked_count == self.selected.len() {
                self.status = "LOCKED REGION(S) CANNOT BE DELETED".into();
                return;
            }

            self.history.record(self.current_snapshot());
            let count = self
                .annotations
                .iter()
                .filter(|a| self.selected.contains(&a.id) && !a.locked)
                .count();

            self.annotations
                .retain(|annotation| !(self.selected.contains(&annotation.id) && !annotation.locked));
            self.selected.retain(|id| self.annotations.iter().any(|a| a.id == *id));
            update_hierarchy(&mut self.annotations);
            self.editing_label = None;
            self.active_drag = None;
            self.status = if count == 1 {
                "ANNOTATION DELETED".into()
            } else {
                format!("{count} ANNOTATIONS DELETED")
            };
        }
    }

    pub fn shortcuts_and_drops(&mut self, ctx: &egui::Context) {
        let (open_img, open_folder, open_proj, save_proj, export_json, export_dataset, undo, redo, delete, prev_img, next_img, escape, select_all, deselect, toggle_lock, enter, dropped) = ctx.input(|input| {
            let cmd_or_ctrl = input.modifiers.command || input.modifiers.ctrl;
            let shift = input.modifiers.shift;
            let alt = input.modifiers.alt;
            (
                cmd_or_ctrl && !shift && !alt && input.key_pressed(Key::O),
                cmd_or_ctrl && (alt && input.key_pressed(Key::O) || shift && input.key_pressed(Key::F)),
                cmd_or_ctrl && shift && input.key_pressed(Key::O),
                cmd_or_ctrl && input.key_pressed(Key::S),
                cmd_or_ctrl && !shift && input.key_pressed(Key::E),
                cmd_or_ctrl && shift && input.key_pressed(Key::E),
                cmd_or_ctrl && !shift && input.key_pressed(Key::Z),
                (cmd_or_ctrl && shift && input.key_pressed(Key::Z))
                    || (cmd_or_ctrl && input.key_pressed(Key::Y)),
                input.key_pressed(Key::Delete) || input.key_pressed(Key::Backspace),
                input.key_pressed(Key::OpenBracket)
                    || (!cmd_or_ctrl && !alt && !shift && input.key_pressed(Key::A)),
                input.key_pressed(Key::CloseBracket)
                    || (!cmd_or_ctrl && !alt && !shift && input.key_pressed(Key::D)),
                input.key_pressed(Key::Escape),
                cmd_or_ctrl && !shift && !alt && input.key_pressed(Key::A),
                cmd_or_ctrl && !shift && !alt && input.key_pressed(Key::D),
                cmd_or_ctrl && !shift && !alt && input.key_pressed(Key::L),
                !cmd_or_ctrl && !shift && !alt && input.key_pressed(Key::Enter),
                input.raw.dropped_files.clone(),
            )
        });

        if open_img {
            self.open_image_dialog(ctx);
        }
        if open_folder {
            self.open_folder_dialog(ctx);
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
        if export_dataset {
            self.export_unified_dataset_dialog();
        }
        if undo {
            self.undo();
        }
        if redo {
            self.redo();
        }
        if select_all {
            self.select_all();
        }
        if deselect {
            self.deselect_all();
        }
        if toggle_lock {
            self.toggle_lock_selected();
        }

        let digit_preset = ctx.input(|input| {
            if input.modifiers.command || input.modifiers.ctrl || input.modifiers.alt {
                None
            } else {
                [
                    (Key::Num1, 0),
                    (Key::Num2, 1),
                    (Key::Num3, 2),
                    (Key::Num4, 3),
                    (Key::Num5, 4),
                    (Key::Num6, 5),
                    (Key::Num7, 6),
                    (Key::Num8, 7),
                    (Key::Num9, 8),
                ]
                .into_iter()
                .find_map(|(k, idx)| if input.key_pressed(k) { Some(idx) } else { None })
            }
        });

        let (tool_rect, tool_poly) = ctx.input(|input| {
            let no_mod = !input.modifiers.command && !input.modifiers.ctrl && !input.modifiers.shift && !input.modifiers.alt;
            (no_mod && input.key_pressed(Key::B), no_mod && input.key_pressed(Key::P))
        });

        let (arrow_left, arrow_right, arrow_up, arrow_down, arrow_shift) = ctx.input(|input| {
            let no_cmd_ctrl_alt = !input.modifiers.command && !input.modifiers.ctrl && !input.modifiers.alt;
            (
                no_cmd_ctrl_alt && input.key_pressed(Key::ArrowLeft),
                no_cmd_ctrl_alt && input.key_pressed(Key::ArrowRight),
                no_cmd_ctrl_alt && input.key_pressed(Key::ArrowUp),
                no_cmd_ctrl_alt && input.key_pressed(Key::ArrowDown),
                input.modifiers.shift,
            )
        });

        if !ctx.wants_keyboard_input() {
            let mut nudge_x = 0.0_f32;
            let mut nudge_y = 0.0_f32;
            let step = if arrow_shift { 10.0 } else { 1.0 };
            if arrow_left {
                nudge_x -= step;
            }
            if arrow_right {
                nudge_x += step;
            }
            if arrow_up {
                nudge_y -= step;
            }
            if arrow_down {
                nudge_y += step;
            }

            if (nudge_x != 0.0 || nudge_y != 0.0) && !self.selected.is_empty() && self.editing_label.is_none() && self.autocomplete_nav.is_none() {
                self.nudge_selected(nudge_x, nudge_y);
            } else if let Some(idx) = digit_preset {
                self.apply_preset(idx);
            } else if tool_rect {
                self.tool_mode = ToolMode::Rectangle;
                self.draft_polygon = None;
                self.marquee = None;
                self.active_drag = None;
                self.status = "BOX TOOL SELECTED (DRAG TO DRAW)".to_string();
            } else if tool_poly {
                self.tool_mode = ToolMode::Polygon;
                self.selected.clear();
                self.editing_label = None;
                self.draft = None;
                self.marquee = None;
                self.active_drag = None;
                self.status = "POLYGON TOOL SELECTED (CLICK TO PLACE POINTS, 3+ TO CLOSE)".to_string();
            } else if enter && self.draft_polygon.as_ref().map_or(false, |p| p.points.len() >= 3) {
                ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                self.finish_draft_polygon();
            } else if enter && self.selected.len() == 1 {
                let id = *self.selected.iter().next().unwrap();
                let is_locked = self.annotations.iter().find(|a| a.id == id).map_or(false, |a| a.locked);
                if !is_locked {
                    ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                    self.history.begin_edit(self.current_snapshot());
                    self.editing_label = Some(id);
                    self.request_label_focus = true;
                }
            } else if delete {
                if let Some(poly) = &mut self.draft_polygon {
                    poly.undo_point();
                    if poly.points.is_empty() {
                        self.draft_polygon = None;
                        self.status = "PEN TOOL DRAWING CANCELED".to_string();
                    } else {
                        self.status = format!("PEN TOOL: POINT REMOVED ({} REMAINING)", poly.points.len());
                    }
                } else {
                    self.delete_selected();
                }
            } else if prev_img {
                self.previous_image(ctx);
            } else if next_img {
                self.next_image(ctx);
            }
        }
        if escape {
            self.draft = None;
            self.draft_polygon = None;
            self.marquee = None;
            self.active_drag = None;
            self.editing_label = None;
            self.selected.clear();
        }
        if let Some(path) = dropped.into_iter().find_map(|file| file.path) {
            if path.is_dir() {
                self.load_folder(ctx, path);
            } else if matches!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("anno" | "annobatch")
            ) {
                self.load_saved_file(ctx, &path);
            } else {
                self.load_image(ctx, path);
            }
        }
    }
}

impl eframe::App for AnnotatorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_background_loads(ctx);
        handle_native_menu_events(self, ctx);
        self.shortcuts_and_drops(ctx);
        render_bottom_bar(self, ctx);
        render_left_sidebar(self, ctx);
        render_right_sidebar(self, ctx);
        render_canvas(self, ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{match_class_presets, next_category_label, BatchProjectFile};
    use eframe::egui::Pos2;
    use std::path::PathBuf;

    fn test_app() -> AnnotatorApp {
        AnnotatorApp {
            image: None,
            image_files: Vec::new(),
            annotations: Vec::new(),
            selected: HashSet::new(),
            editing_label: None,
            next_id: 1,
            tool_mode: ToolMode::Rectangle,
            draft: None,
            draft_polygon: None,
            marquee: None,
            active_drag: None,
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            status: String::new(),
            request_label_focus: false,
            native_menubar: None,
            project_description: None,
            history: History::new(),
            dataset_folder: None,
            current_image_idx: None,
            pending_image_idx: None,
            auto_save_dataset: true,
            filmstrip_filter: FilmstripFilter::All,
            annotation_counts: HashMap::new(),
            thumbnail_cache: HashMap::new(),
            image_cache: HashMap::new(),
            annotations_cache: HashMap::new(),
            loader: BackgroundLoader::new(),
            presets: default_presets(),
            active_preset_idx: 0,
            autocomplete_nav: None,
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
            locked: false,
            points: None,
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
        app.selected.insert(1);
        app.next_id = 2;

        assert!(app.can_undo());
        assert!(!app.can_redo());
        assert_eq!(app.annotations.len(), 1);

        // Undo
        app.undo();
        assert_eq!(app.annotations.len(), 0);
        assert!(app.selected.is_empty());
        assert!(!app.can_undo());
        assert!(app.can_redo());

        // Redo
        app.redo();
        assert_eq!(app.annotations.len(), 1);
        assert!(app.selected.contains(&1));
        assert!(app.can_undo());
        assert!(!app.can_redo());
    }

    #[test]
    fn test_undo_redo_delete_selected() {
        let mut app = test_app();
        app.annotations.push(sample_annotation(1));
        app.selected.insert(1);

        app.delete_selected();
        assert_eq!(app.annotations.len(), 0);
        assert!(app.selected.is_empty());
        assert!(app.can_undo());

        app.undo();
        assert_eq!(app.annotations.len(), 1);
        assert_eq!(app.annotations[0].id, 1);
        assert!(app.selected.contains(&1));
    }

    #[test]
    fn test_multi_select_helpers() {
        let mut app = test_app();
        app.annotations.push(sample_annotation(1));
        app.annotations.push(sample_annotation(2));
        app.annotations.push(sample_annotation(3));

        app.select_single(1);
        assert_eq!(app.selected.len(), 1);
        assert!(app.is_selected(1));

        app.toggle_select(2);
        assert_eq!(app.selected.len(), 2);
        assert!(app.is_selected(1));
        assert!(app.is_selected(2));

        app.toggle_select(1);
        assert_eq!(app.selected.len(), 1);
        assert!(!app.is_selected(1));
        assert!(app.is_selected(2));

        app.select_all();
        assert_eq!(app.selected.len(), 3);
        assert!(app.is_selected(1));
        assert!(app.is_selected(2));
        assert!(app.is_selected(3));

        app.deselect_all();
        assert!(app.selected.is_empty());
    }

    #[test]
    fn test_multi_select_delete_undo_redo() {
        let mut app = test_app();
        app.annotations.push(sample_annotation(1));
        app.annotations.push(sample_annotation(2));
        app.annotations.push(sample_annotation(3));

        app.selected.insert(1);
        app.selected.insert(3);

        app.delete_selected();
        assert_eq!(app.annotations.len(), 1);
        assert_eq!(app.annotations[0].id, 2);
        assert!(app.selected.is_empty());
        assert!(app.can_undo());

        app.undo();
        assert_eq!(app.annotations.len(), 3);
        assert!(app.is_selected(1));
        assert!(!app.is_selected(2));
        assert!(app.is_selected(3));

        app.redo();
        assert_eq!(app.annotations.len(), 1);
        assert_eq!(app.annotations[0].id, 2);
        assert!(app.selected.is_empty());
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

    #[test]
    fn test_batch_navigation_bounds() {
        let mut app = test_app();
        app.image_files = vec![
            PathBuf::from("img1.png"),
            PathBuf::from("img2.png"),
            PathBuf::from("img3.png"),
        ];
        app.current_image_idx = Some(0);

        // Navigation state checks
        assert_eq!(app.current_image_idx, Some(0));
        assert_eq!(app.image_files.len(), 3);
    }

    #[test]
    fn test_switch_reloads_same_index_when_batch_image_changed() {
        let ctx = egui::Context::default();
        let old_path = PathBuf::from("old_batch/frame_1.png");
        let new_path = PathBuf::from("new_batch/frame_1.png");
        let pixel = egui::ColorImage::new([1, 1], egui::Color32::WHITE);

        let old_image = LoadedImage {
            texture: ctx.load_texture("old", pixel.clone(), egui::TextureOptions::LINEAR),
            path: old_path,
            width: 1,
            height: 1,
        };
        let new_image = LoadedImage {
            texture: ctx.load_texture("new", pixel, egui::TextureOptions::LINEAR),
            path: new_path.clone(),
            width: 1,
            height: 1,
        };

        let mut app = test_app();
        app.image = Some(old_image);
        app.current_image_idx = Some(0);
        app.image_files = vec![new_path.clone()];
        app.image_cache.insert(new_path.clone(), new_image);

        app.switch_to_image_index(&ctx, 0);

        assert_eq!(app.current_image_idx, Some(0));
        assert_eq!(app.image.as_ref().map(|image| &image.path), Some(&new_path));
    }

    #[test]
    fn test_switch_activates_uncached_image_without_blocking_for_decode() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        app.image_files = vec![
            PathBuf::from("frame_01.png"),
            PathBuf::from("frame_02.png"),
        ];
        app.current_image_idx = Some(0);

        app.switch_to_image_index(&ctx, 1);

        assert_eq!(app.current_image_idx, Some(1));
        assert_eq!(app.pending_image_idx, Some(1));
        assert!(!app.status.contains("LOADING"));
    }

    #[test]
    fn test_failed_background_decode_releases_pending_navigation() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let invalid_path = std::env::temp_dir().join(format!("invalid_frame_{unique}.png"));
        std::fs::write(&invalid_path, b"not an image").unwrap();

        let ctx = egui::Context::default();
        let mut app = test_app();
        app.image_files = vec![invalid_path.clone()];
        app.switch_to_image_index(&ctx, 0);

        for _ in 0..100 {
            app.poll_background_loads(&ctx);
            if app.pending_image_idx.is_none() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }

        assert_eq!(app.pending_image_idx, None);
        assert!(app.status.contains("COULD NOT OPEN IMAGE"));
        std::fs::remove_file(invalid_path).unwrap();
    }

    #[test]
    fn test_full_image_cache_stays_near_active_navigation() {
        let ctx = egui::Context::default();
        let pixel = egui::ColorImage::new([1, 1], egui::Color32::WHITE);
        let mut app = test_app();
        app.image_files = (0..10)
            .map(|idx| PathBuf::from(format!("frame_{idx:02}.png")))
            .collect();
        app.current_image_idx = Some(5);

        for path in &app.image_files {
            app.image_cache.insert(
                path.clone(),
                LoadedImage {
                    texture: ctx.load_texture(
                        path.to_string_lossy(),
                        pixel.clone(),
                        egui::TextureOptions::LINEAR,
                    ),
                    path: path.clone(),
                    width: 1,
                    height: 1,
                },
            );
        }

        app.prune_image_cache();

        assert_eq!(app.image_cache.len(), 5);
        for idx in 3..=7 {
            assert!(app.image_cache.contains_key(&app.image_files[idx]));
        }
    }

    #[test]
    fn test_export_unified_dataset_json() {
        let mut app = test_app();
        let path1 = PathBuf::from("frame_01.png");
        let path2 = PathBuf::from("frame_02.png");
        app.dataset_folder = Some(PathBuf::from("/dataset"));
        app.image_files = vec![path1.clone(), path2.clone()];
        app.annotations_cache.insert(
            path1,
            (vec![sample_annotation(1)], Some("car".into()), 2),
        );
        app.annotations_cache.insert(
            path2,
            (vec![sample_annotation(2)], None, 3),
        );

        let temp_dir = std::env::temp_dir();
        let export_path = temp_dir.join("anno_test_unified_export.json");
        app.export_unified_dataset_to(&export_path);

        assert!(export_path.exists());
        let content = std::fs::read_to_string(&export_path).unwrap();
        let val: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(val["total_images"], 2);
        assert_eq!(val["annotated_images"], 2);
        assert_eq!(val["images"][0]["image"], "frame_01.png");
        assert_eq!(val["images"][0]["annotations"][0]["id"], 1);
        assert_eq!(val["images"][1]["image"], "frame_02.png");
        assert_eq!(val["images"][1]["annotations"][0]["id"], 2);

        let _ = std::fs::remove_file(export_path);
    }

    #[test]
    fn test_save_batch_uses_editable_annobatch_format() {
        let mut app = test_app();
        let dataset_folder = PathBuf::from("/dataset/camera_frames");
        let path1 = dataset_folder.join("frame_01.png");
        let path2 = dataset_folder.join("frame_02.png");
        app.dataset_folder = Some(dataset_folder);
        app.image_files = vec![path1.clone(), path2.clone()];
        app.current_image_idx = Some(1);
        app.annotations_cache.insert(
            path1,
            (vec![sample_annotation(1)], Some("first".into()), 2),
        );
        app.annotations_cache
            .insert(path2, (vec![sample_annotation(4)], None, 5));

        let save_path = std::env::temp_dir().join("anno_test_batch.annobatch");
        app.save_batch_to(&save_path);

        let content = std::fs::read_to_string(&save_path).unwrap();
        let batch: BatchProjectFile = serde_json::from_str(&content).unwrap();
        assert_eq!(batch.format, "annobatch");
        assert_eq!(batch.format_version, 1);
        assert_eq!(batch.dataset_name, "camera_frames");
        assert_eq!(batch.current_image_idx, 1);
        assert_eq!(batch.images.len(), 2);
        assert_eq!(batch.images[0].image, "frame_01.png");
        assert_eq!(batch.images[0].description.as_deref(), Some("first"));
        assert_eq!(batch.images[0].annotations[0].id, 1);
        assert_eq!(batch.images[1].next_id, 5);

        let _ = std::fs::remove_file(save_path);
    }

    #[test]
    fn test_annobatch_round_trip_restores_current_image_and_annotations() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dataset_folder = std::env::temp_dir().join(format!("anno_batch_{unique}"));
        std::fs::create_dir_all(&dataset_folder).unwrap();
        let path1 = dataset_folder.join("frame_01.png");
        let path2 = dataset_folder.join("frame_02.png");
        image::RgbaImage::new(2, 2).save(&path1).unwrap();
        image::RgbaImage::new(3, 2).save(&path2).unwrap();

        let mut source = test_app();
        source.dataset_folder = Some(dataset_folder.clone());
        source.image_files = vec![path1, path2.clone()];
        source.current_image_idx = Some(1);
        source.annotations_cache.insert(
            path2.clone(),
            (vec![sample_annotation(4)], Some("active frame".into()), 5),
        );
        let batch_path = dataset_folder.join("camera_frames.annobatch");
        source.save_batch_to(&batch_path);

        let ctx = egui::Context::default();
        let mut restored = test_app();
        restored.load_batch(&ctx, &batch_path);

        for _ in 0..100 {
            restored.poll_background_loads(&ctx);
            if restored.pending_image_idx.is_none() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }

        assert_eq!(restored.current_image_idx, Some(1));
        assert_eq!(restored.image.as_ref().map(|image| &image.path), Some(&path2));
        assert_eq!(restored.annotations[0].id, 4);
        assert_eq!(restored.project_description.as_deref(), Some("active frame"));
        assert_eq!(restored.next_id, 5);

        std::fs::remove_dir_all(dataset_folder).unwrap();
    }

    #[test]
    fn test_multi_select_batch_label_and_color() {
        let mut app = test_app();
        app.annotations.push(sample_annotation(1));
        app.annotations.push(sample_annotation(2));
        app.annotations.push(sample_annotation(3));

        app.selected.insert(1);
        app.selected.insert(2);

        // Batch label update
        for a in app.annotations.iter_mut().filter(|a| app.selected.contains(&a.id)) {
            a.label = "vehicle".into();
        }

        assert_eq!(app.annotations[0].label, "vehicle");
        assert_eq!(app.annotations[1].label, "vehicle");
        assert_eq!(app.annotations[2].label, "region_3");

        // Batch color update
        let new_color = [0, 230, 118];
        for a in app.annotations.iter_mut().filter(|a| app.selected.contains(&a.id)) {
            a.color = new_color;
        }

        assert_eq!(app.annotations[0].color, new_color);
        assert_eq!(app.annotations[1].color, new_color);
        assert_eq!(app.annotations[2].color, [255, 0, 0]);
    }

    #[test]
    fn test_toggle_lock_selected() {
        let mut app = test_app();
        app.annotations.push(sample_annotation(1));
        app.annotations.push(sample_annotation(2));

        app.selected.insert(1);
        assert!(!app.annotations[0].locked);

        // Toggle lock on selected
        app.toggle_lock_selected();
        assert!(app.annotations[0].locked);
        assert!(!app.annotations[1].locked);

        // Toggle again to unlock
        app.toggle_lock_selected();
        assert!(!app.annotations[0].locked);
    }

    #[test]
    fn test_locked_annotation_delete_protection() {
        let mut app = test_app();
        let mut a1 = sample_annotation(1);
        a1.locked = true;
        let a2 = sample_annotation(2);
        app.annotations = vec![a1, a2];

        // Try deleting locked annotation 1
        app.selected.insert(1);
        app.delete_selected();
        assert_eq!(app.annotations.len(), 2);
        assert!(app.status.contains("LOCKED"));

        // Delete unlocked annotation 2
        app.selected.clear();
        app.selected.insert(2);
        app.delete_selected();
        assert_eq!(app.annotations.len(), 1);
        assert_eq!(app.annotations[0].id, 1);
    }

    #[test]
    fn test_apply_preset_without_selection_switches_active_preset() {
        let mut app = test_app();
        assert_eq!(app.active_preset_idx, 0);

        // Switch to preset 2 (person)
        app.apply_preset(1);
        assert_eq!(app.active_preset_idx, 1);
        assert!(app.status.contains("PERSON"));
        assert_eq!(app.active_preset().unwrap().prefix, "person");

        // Switch to preset 3 (vehicle)
        app.apply_preset(2);
        assert_eq!(app.active_preset_idx, 2);
        assert!(app.status.contains("VEHICLE"));
        assert_eq!(app.active_preset().unwrap().prefix, "vehicle");
    }

    #[test]
    fn test_apply_preset_to_selected_annotations() {
        let mut app = test_app();
        app.annotations = vec![sample_annotation(1), sample_annotation(2), sample_annotation(3)];

        // Select annotation 1 and 2
        app.selected.insert(1);
        app.selected.insert(2);

        // Apply preset 2 (person, blue)
        app.apply_preset(1);

        assert_eq!(app.annotations[0].label, "person_01");
        assert_eq!(app.annotations[0].color, [41, 121, 255]);
        assert_eq!(app.annotations[1].label, "person_02");
        assert_eq!(app.annotations[1].color, [41, 121, 255]);
        assert_eq!(app.annotations[2].label, "region_3");
        assert_eq!(app.annotations[2].color, [255, 0, 0]);
        assert!(app.status.contains("PERSON APPLIED"));
    }

    #[test]
    fn test_preset_undo_redo() {
        let mut app = test_app();
        app.annotations = vec![sample_annotation(1)];
        app.selected.insert(1);

        // Apply preset 3 (vehicle, green)
        app.apply_preset(2);
        assert_eq!(app.annotations[0].label, "vehicle_01");
        assert_eq!(app.annotations[0].color, [0, 230, 118]);

        // Undo
        app.undo();
        assert_eq!(app.annotations[0].label, "region_1");
        assert_eq!(app.annotations[0].color, [255, 0, 0]);

        // Redo
        app.redo();
        assert_eq!(app.annotations[0].label, "vehicle_01");
        assert_eq!(app.annotations[0].color, [0, 230, 118]);
    }

    #[test]
    fn test_preset_does_not_mutate_locked_annotations() {
        let mut app = test_app();
        let mut a1 = sample_annotation(1);
        a1.locked = true;
        let a2 = sample_annotation(2);
        app.annotations = vec![a1, a2];

        app.selected.insert(1);
        app.selected.insert(2);

        // Apply preset 2 (person)
        app.apply_preset(1);

        // Locked annotation 1 remains unchanged
        assert_eq!(app.annotations[0].label, "region_1");
        assert_eq!(app.annotations[0].color, [255, 0, 0]);

        // Unlocked annotation 2 is updated with first category number
        assert_eq!(app.annotations[1].label, "person_01");
        assert_eq!(app.annotations[1].color, [41, 121, 255]);
    }

    #[test]
    fn test_autocomplete_navigation_and_selection() {
        let mut app = test_app();
        let mut a = sample_annotation(1);
        a.label = "pe".to_string();
        app.annotations = vec![a];
        app.selected.insert(1);

        let suggestions = match_class_presets(&app.annotations[0].label, &app.presets);
        assert!(!suggestions.is_empty());
        assert_eq!(suggestions[0].1.prefix, "person");

        // Simulate Down arrow navigation
        let max_count = suggestions.len().min(4);
        app.autocomplete_nav = Some(match app.autocomplete_nav {
            Some(curr) => (curr + 1) % max_count,
            None => 0,
        });
        assert_eq!(app.autocomplete_nav, Some(0));

        // Simulate Enter selection
        if let Some(idx) = app.autocomplete_nav {
            let (_, preset) = suggestions[idx];
            app.annotations[0].color = preset.color;
            app.annotations[0].label = next_category_label(&preset.prefix, &app.annotations, Some(app.annotations[0].id));
            app.autocomplete_nav = None;
        }

        assert_eq!(app.annotations[0].label, "person_01");
        assert_eq!(app.annotations[0].color, [41, 121, 255]);
        assert_eq!(app.autocomplete_nav, None);
    }

    #[test]
    fn test_enter_shortcut_renames_single_selected_annotation() {
        let mut app = test_app();
        app.annotations = vec![sample_annotation(1), sample_annotation(2)];
        app.selected.insert(1);

        // Verify single selection
        assert_eq!(app.selected.len(), 1);
        let id = *app.selected.iter().next().unwrap();
        assert_eq!(id, 1);

        // Simulate Enter shortcut behavior
        let is_locked = app.annotations.iter().find(|a| a.id == id).map_or(false, |a| a.locked);
        assert!(!is_locked);
        app.history.begin_edit(app.current_snapshot());
        app.editing_label = Some(id);
        app.request_label_focus = true;

        assert_eq!(app.editing_label, Some(1));
        assert!(app.request_label_focus);
    }

    #[test]
    fn test_single_image_mode_does_not_enable_batch_dataset() {
        let mut app = test_app();

        app.dataset_folder = None;
        app.image_files.clear();
        app.current_image_idx = None;

        // When single image is loaded, dataset_folder is None and image_files is empty
        assert!(app.dataset_folder.is_none());
        assert!(app.image_files.is_empty());
        assert!(app.current_image_idx.is_none());

        let is_batch = app.dataset_folder.is_some() || app.image_files.len() > 1;
        assert!(!is_batch);
    }

    #[test]
    fn test_finish_draft_polygon() {
        let mut app = test_app();
        app.draft_polygon = Some(DraftPolygon::from_points(vec![
            Pos2::new(10.0, 10.0),
            Pos2::new(40.0, 10.0),
            Pos2::new(40.0, 50.0),
            Pos2::new(10.0, 50.0),
        ]));

        let success = app.finish_draft_polygon();
        assert!(success);
        assert_eq!(app.annotations.len(), 1);
        let poly_anno = &app.annotations[0];
        assert_eq!(poly_anno.id, 1);
        assert_eq!(poly_anno.x, 10.0);
        assert_eq!(poly_anno.y, 10.0);
        assert_eq!(poly_anno.width, 30.0);
        assert_eq!(poly_anno.height, 40.0);
        assert!(poly_anno.points.is_some());
        assert_eq!(poly_anno.points.as_ref().unwrap().len(), 4);
        assert_eq!(app.selected.len(), 1);
        assert!(app.is_selected(1));
        assert_eq!(app.editing_label, None);
    }

    #[test]
    fn test_finish_draft_polygon_requires_minimum_3_points() {
        let mut app = test_app();
        app.draft_polygon = Some(DraftPolygon::from_points(vec![
            Pos2::new(10.0, 10.0),
            Pos2::new(40.0, 10.0),
        ]));

        // 2 points is not a polygon region
        let success = app.finish_draft_polygon();
        assert!(!success);
        assert!(app.annotations.is_empty());
    }

    #[test]
    fn test_tool_mode_switching_and_cancellation() {
        let mut app = test_app();
        assert_eq!(app.tool_mode, ToolMode::Rectangle);

        // Switch to polygon mode
        app.tool_mode = ToolMode::Polygon;
        assert_eq!(app.tool_mode, ToolMode::Polygon);

        app.draft_polygon = Some(DraftPolygon::from_points(vec![
            Pos2::new(5.0, 5.0),
            Pos2::new(15.0, 5.0),
        ]));

        // Undo point (simulating Backspace or Cmd+Z)
        app.undo();
        assert_eq!(app.draft_polygon.as_ref().unwrap().points.len(), 1);

        // Redo point (simulating Cmd+Shift+Z / Cmd+Y)
        app.redo();
        assert_eq!(app.draft_polygon.as_ref().unwrap().points.len(), 2);

        // Cancel (simulating Escape)
        app.draft_polygon = None;
        assert!(app.draft_polygon.is_none());
    }

    #[test]
    fn test_polygon_draft_step_by_step_undo_redo() {
        let mut app = test_app();
        app.tool_mode = ToolMode::Polygon;

        // User clicks 3 points
        let mut draft = DraftPolygon::new(Pos2::new(10.0, 10.0));
        draft.add_point(Pos2::new(50.0, 10.0));
        draft.add_point(Pos2::new(50.0, 60.0));
        app.draft_polygon = Some(draft);

        assert_eq!(app.draft_polygon.as_ref().unwrap().points.len(), 3);
        assert!(app.can_undo());

        // 1. Undo 3rd point
        app.undo();
        assert_eq!(app.draft_polygon.as_ref().unwrap().points.len(), 2);
        assert!(app.can_undo());
        assert!(app.can_redo());

        // 2. Undo 2nd point
        app.undo();
        assert_eq!(app.draft_polygon.as_ref().unwrap().points.len(), 1);
        assert!(app.can_undo());
        assert!(app.can_redo());

        // 3. Redo 2nd point
        app.redo();
        assert_eq!(app.draft_polygon.as_ref().unwrap().points.len(), 2);

        // 4. Redo 3rd point
        app.redo();
        assert_eq!(app.draft_polygon.as_ref().unwrap().points.len(), 3);

        // 5. Undo all points to cancel
        app.undo(); // 2 points
        app.undo(); // 1 point
        app.undo(); // 0 points -> draft cancelled
        assert!(app.draft_polygon.is_none());
    }

    #[test]
    fn test_polygon_undo_redo() {
        let mut app = test_app();
        assert_eq!(app.annotations.len(), 0);
        assert!(!app.can_undo());

        app.draft_polygon = Some(DraftPolygon::from_points(vec![
            Pos2::new(10.0, 10.0),
            Pos2::new(40.0, 10.0),
            Pos2::new(40.0, 50.0),
        ]));

        assert!(app.finish_draft_polygon());
        assert_eq!(app.annotations.len(), 1);
        assert!(app.can_undo());

        // Undo polygon creation
        app.undo();
        assert_eq!(app.annotations.len(), 0);
        assert!(!app.can_undo());
        assert!(app.can_redo());

        // Redo polygon creation
        app.redo();
        assert_eq!(app.annotations.len(), 1);
        assert!(app.annotations[0].points.is_some());
        assert_eq!(app.annotations[0].points.as_ref().unwrap().len(), 3);
        assert!(app.can_undo());
        assert!(!app.can_redo());
    }

    #[test]
    fn test_filmstrip_filtering() {
        let mut app = test_app();
        let img1 = PathBuf::from("/path/to/img1.png");
        let img2 = PathBuf::from("/path/to/img2.png");
        let img3 = PathBuf::from("/path/to/img3.png");
        app.image_files = vec![img1.clone(), img2.clone(), img3.clone()];

        // img1: 2 annotations, img2: 0 annotations, img3: 1 annotation
        app.annotation_counts.insert(img1.clone(), 2);
        app.annotation_counts.insert(img2.clone(), 0);
        app.annotation_counts.insert(img3.clone(), 1);

        // 1. Filter: All
        app.filmstrip_filter = FilmstripFilter::All;
        assert_eq!(app.filtered_image_indices(), vec![0, 1, 2]);

        // 2. Filter: Annotated
        app.filmstrip_filter = FilmstripFilter::Annotated;
        assert_eq!(app.filtered_image_indices(), vec![0, 2]);

        // 3. Filter: Unannotated
        app.filmstrip_filter = FilmstripFilter::Unannotated;
        assert_eq!(app.filtered_image_indices(), vec![1]);

        // 4. Live annotation count takes precedence for currently open image
        let ctx = egui::Context::default();
        let pixel = egui::ColorImage::new([1, 1], egui::Color32::WHITE);
        app.image = Some(LoadedImage {
            texture: ctx.load_texture("test_img", pixel, egui::TextureOptions::LINEAR),
            width: 100,
            height: 100,
            path: img2.clone(),
        });
        app.annotations.push(sample_annotation(1));

        // Now img2 has 1 active annotation in memory
        assert_eq!(app.image_annotation_count(&img2), 1);

        app.filmstrip_filter = FilmstripFilter::Annotated;
        assert_eq!(app.filtered_image_indices(), vec![0, 1, 2]);

        app.filmstrip_filter = FilmstripFilter::Unannotated;
        assert!(app.filtered_image_indices().is_empty());
    }

    #[test]
    fn test_polygon_draft_inside_existing_rectangle() {
        let mut app = test_app();
        app.tool_mode = ToolMode::Polygon;

        // Existing rectangle annotation at (0, 0, 100, 100)
        app.annotations.push(Annotation {
            id: 1,
            label: "container".into(),
            description: None,
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            color: [255, 0, 0],
            parent_id: None,
            locked: false,
            points: None,
        });
        app.next_id = 2;

        // Placing anchor point inside the rectangle (50, 50)
        let mut draft = DraftPolygon::new(Pos2::new(50.0, 50.0));
        draft.add_point(Pos2::new(70.0, 50.0));
        draft.add_point(Pos2::new(70.0, 70.0));
        draft.add_point(Pos2::new(50.0, 70.0));
        app.draft_polygon = Some(draft);

        // Selection must not be hijacked by the rectangle
        assert!(app.selected.is_empty());
        assert!(app.finish_draft_polygon());

        // Now we have 2 annotations: 1 rectangle (id 1) and 1 polygon (id 2) nested inside it
        assert_eq!(app.annotations.len(), 2);
        let poly = &app.annotations[1];
        assert_eq!(poly.id, 2);
        assert!(poly.points.is_some());
        assert_eq!(poly.x, 50.0);
        assert_eq!(poly.y, 50.0);
    }

    #[test]
    fn test_nudge_single_box() {
        let mut app = test_app();
        app.annotations.push(sample_annotation(1)); // x: 10, y: 10, w: 50, h: 50
        app.select_single(1);

        // Nudge right by 1px
        assert!(app.nudge_selected(1.0, 0.0));
        assert_eq!(app.annotations[0].x, 11.0);
        assert_eq!(app.annotations[0].y, 10.0);

        // Nudge down by 10px (Shift + ArrowDown)
        assert!(app.nudge_selected(0.0, 10.0));
        assert_eq!(app.annotations[0].x, 11.0);
        assert_eq!(app.annotations[0].y, 20.0);
    }

    #[test]
    fn test_nudge_polygon_translates_points_and_bounding_box() {
        let mut app = test_app();
        app.annotations.push(Annotation {
            id: 1,
            label: "poly".into(),
            description: None,
            x: 20.0,
            y: 30.0,
            width: 40.0,
            height: 40.0,
            color: [0, 255, 0],
            parent_id: None,
            locked: false,
            points: Some(vec![[20.0, 30.0], [60.0, 30.0], [60.0, 70.0], [20.0, 70.0]]),
        });
        app.select_single(1);

        assert!(app.nudge_selected(5.0, -10.0));
        assert_eq!(app.annotations[0].x, 25.0);
        assert_eq!(app.annotations[0].y, 20.0);
        let pts = app.annotations[0].points.as_ref().unwrap();
        assert_eq!(pts[0], [25.0, 20.0]);
        assert_eq!(pts[1], [65.0, 20.0]);
        assert_eq!(pts[2], [65.0, 60.0]);
        assert_eq!(pts[3], [25.0, 60.0]);
    }

    #[test]
    fn test_nudge_multi_selection() {
        let mut app = test_app();
        app.annotations.push(sample_annotation(1)); // (10, 10)
        app.annotations.push(sample_annotation(2)); // (20, 20)
        app.selected.insert(1);
        app.selected.insert(2);

        assert!(app.nudge_selected(-5.0, 15.0));
        assert_eq!(app.annotations[0].x, 5.0);
        assert_eq!(app.annotations[0].y, 25.0);
        assert_eq!(app.annotations[1].x, 15.0);
        assert_eq!(app.annotations[1].y, 35.0);
    }

    #[test]
    fn test_nudge_clamped_to_image_bounds() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        let pixel = egui::ColorImage::new([1, 1], egui::Color32::WHITE);
        app.image = Some(LoadedImage {
            texture: ctx.load_texture("test_img", pixel, egui::TextureOptions::LINEAR),
            width: 100,
            height: 100,
            path: PathBuf::from("/test.png"),
        });

        // Box at (80, 80) with size (20, 20) -> reaches image right-bottom edge (100, 100)
        app.annotations.push(Annotation {
            id: 1,
            label: "edge".into(),
            description: None,
            x: 80.0,
            y: 80.0,
            width: 20.0,
            height: 20.0,
            color: [255, 0, 0],
            parent_id: None,
            locked: false,
            points: None,
        });
        app.select_single(1);

        // Cannot move past right/bottom edge
        assert!(!app.nudge_selected(5.0, 5.0));
        assert_eq!(app.annotations[0].x, 80.0);
        assert_eq!(app.annotations[0].y, 80.0);

        // Moving left by 100px is clamped to x: 0
        assert!(app.nudge_selected(-100.0, 0.0));
        assert_eq!(app.annotations[0].x, 0.0);
    }

    #[test]
    fn test_nudge_locked_annotation_protected() {
        let mut app = test_app();
        let mut anno = sample_annotation(1);
        anno.locked = true;
        app.annotations.push(anno);
        app.select_single(1);

        assert!(!app.nudge_selected(10.0, 10.0));
        assert_eq!(app.annotations[0].x, 10.0);
        assert_eq!(app.annotations[0].y, 10.0);
    }

    #[test]
    fn test_nudge_undo_redo() {
        let mut app = test_app();
        app.annotations.push(sample_annotation(1)); // (10, 10)
        app.select_single(1);

        assert!(app.nudge_selected(5.0, 5.0));
        assert_eq!(app.annotations[0].x, 15.0);
        assert_eq!(app.annotations[0].y, 15.0);

        // Undo
        app.undo();
        assert_eq!(app.annotations[0].x, 10.0);
        assert_eq!(app.annotations[0].y, 10.0);

        // Redo
        app.redo();
        assert_eq!(app.annotations[0].x, 15.0);
        assert_eq!(app.annotations[0].y, 15.0);
    }
}
