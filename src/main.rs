pub mod app;
pub mod canvas;
pub mod geometry;
pub mod models;
pub mod render;
pub mod sidebar_left;
pub mod sidebar_right;
pub mod theme;

use app::AnnotatorApp;
use eframe::egui::ViewportBuilder;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("ANNO")
            .with_inner_size([1280.0, 780.0])
            .with_min_inner_size([900.0, 560.0]),
        ..Default::default()
    };

    eframe::run_native(
        "ANNO",
        options,
        Box::new(|cc| Ok(Box::new(AnnotatorApp::new(cc)))),
    )
}
