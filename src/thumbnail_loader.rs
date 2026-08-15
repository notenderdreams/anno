use crate::models::LoadedImage;
use eframe::egui;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread;

pub struct BackgroundLoader {
    thumb_request_tx: Sender<PathBuf>,
    thumb_result_rx: Receiver<(PathBuf, egui::ColorImage)>,
    thumb_pending: HashSet<PathBuf>,

    img_request_tx: Sender<PathBuf>,
    foreground_request_tx: Sender<PathBuf>,
    img_result_rx: Receiver<FullImageResult>,
    img_pending: HashSet<PathBuf>,
    foreground_pending: HashSet<PathBuf>,
}

enum FullImageResult {
    Loaded(PathBuf, egui::ColorImage, u32, u32),
    Failed(PathBuf, String),
}

fn decode_full_image(path: PathBuf) -> FullImageResult {
    match image::open(&path) {
        Ok(decoded) => {
            let rgba = decoded.to_rgba8();
            let (width, height) = rgba.dimensions();
            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                [width as usize, height as usize],
                rgba.as_raw(),
            );
            FullImageResult::Loaded(path, color_image, width, height)
        }
        Err(error) => FullImageResult::Failed(path, error.to_string()),
    }
}

impl Default for BackgroundLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl BackgroundLoader {
    pub fn new() -> Self {
        let (thumb_request_tx, thumb_request_rx) = channel::<PathBuf>();
        let (thumb_result_tx, thumb_result_rx) = channel::<(PathBuf, egui::ColorImage)>();

        // Background worker for fast thumbnails
        thread::Builder::new()
            .name("thumbnail_worker".into())
            .spawn(move || {
                while let Ok(path) = thumb_request_rx.recv() {
                    if let Ok(decoded) = image::open(&path) {
                        let thumb = decoded.thumbnail(96, 64);
                        let rgba = thumb.to_rgba8();
                        let (width, height) = rgba.dimensions();
                        let color_image = egui::ColorImage::from_rgba_unmultiplied(
                            [width as usize, height as usize],
                            rgba.as_raw(),
                        );
                        let _ = thumb_result_tx.send((path, color_image));
                    }
                }
            })
            .expect("failed to spawn thumbnail worker");

        let (img_request_tx, img_request_rx) = channel::<PathBuf>();
        let (foreground_request_tx, foreground_request_rx) = channel::<PathBuf>();
        let (img_result_tx, img_result_rx) = channel::<FullImageResult>();

        let foreground_result_tx = img_result_tx.clone();
        thread::Builder::new()
            .name("image_navigation_worker".into())
            .spawn(move || {
                while let Ok(path) = foreground_request_rx.recv() {
                    let _ = foreground_result_tx.send(decode_full_image(path));
                }
            })
            .expect("failed to spawn image navigation worker");

        // Background worker for full image preloading
        thread::Builder::new()
            .name("image_preloader_worker".into())
            .spawn(move || {
                while let Ok(path) = img_request_rx.recv() {
                    let _ = img_result_tx.send(decode_full_image(path));
                }
            })
            .expect("failed to spawn image preloader worker");

        Self {
            thumb_request_tx,
            thumb_result_rx,
            thumb_pending: HashSet::new(),
            img_request_tx,
            foreground_request_tx,
            img_result_rx,
            img_pending: HashSet::new(),
            foreground_pending: HashSet::new(),
        }
    }

    pub fn request_thumbnail(&mut self, path: &Path) {
        if self.thumb_pending.contains(path) {
            return;
        }
        self.thumb_pending.insert(path.to_path_buf());
        let _ = self.thumb_request_tx.send(path.to_path_buf());
    }

    pub fn request_preload_image(&mut self, path: &Path) {
        if self.img_pending.contains(path) {
            return;
        }
        self.img_pending.insert(path.to_path_buf());
        let _ = self.img_request_tx.send(path.to_path_buf());
    }

    pub fn request_navigation_image(&mut self, path: &Path) {
        if self.foreground_pending.contains(path) {
            return;
        }
        self.foreground_pending.insert(path.to_path_buf());
        let _ = self.foreground_request_tx.send(path.to_path_buf());
    }

    pub fn poll_results(
        &mut self,
        ctx: &egui::Context,
        thumb_cache: &mut HashMap<PathBuf, egui::TextureHandle>,
        img_cache: &mut HashMap<PathBuf, LoadedImage>,
    ) -> Vec<(PathBuf, String)> {
        let mut loaded_any = false;
        let mut failures = Vec::new();

        // Process up to 4 thumbnail textures per frame to maintain steady 60+ FPS
        for _ in 0..4 {
            match self.thumb_result_rx.try_recv() {
                Ok((path, color_image)) => {
                    let texture = ctx.load_texture(
                        format!("thumb_{}", path.display()),
                        color_image,
                        egui::TextureOptions::LINEAR,
                    );
                    self.thumb_pending.remove(&path);
                    thumb_cache.insert(path, texture);
                    loaded_any = true;
                }
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }

        // Process up to 2 preloaded full image textures per frame
        for _ in 0..2 {
            match self.img_result_rx.try_recv() {
                Ok(FullImageResult::Loaded(path, color_image, width, height)) => {
                    let texture = ctx.load_texture(
                        path.to_string_lossy(),
                        color_image,
                        egui::TextureOptions::LINEAR,
                    );
                    self.img_pending.remove(&path);
                    self.foreground_pending.remove(&path);
                    img_cache.insert(
                        path.clone(),
                        LoadedImage {
                            texture,
                            path,
                            width,
                            height,
                        },
                    );
                    loaded_any = true;
                }
                Ok(FullImageResult::Failed(path, error)) => {
                    self.img_pending.remove(&path);
                    self.foreground_pending.remove(&path);
                    failures.push((path, error));
                }
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }

        if loaded_any {
            ctx.request_repaint();
        }

        failures
    }

    pub fn clear(&mut self) {
        self.thumb_pending.clear();
        self.img_pending.clear();
        self.foreground_pending.clear();
    }
}
