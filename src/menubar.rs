use muda::{
    accelerator::{Accelerator, Code, Modifiers},
    Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu,
};
use crate::app::AnnotatorApp;
use eframe::egui;

pub struct NativeMenuBar {
    pub menu: Menu,
    pub open_image: MenuItem,
    pub open_project: MenuItem,
    pub save_project: MenuItem,
    pub export_json: MenuItem,
    pub delete_region: MenuItem,
    pub deselect: MenuItem,
    pub reset_view: MenuItem,
    pub zoom_in: MenuItem,
    pub zoom_out: MenuItem,
}

impl Default for NativeMenuBar {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeMenuBar {
    pub fn new() -> Self {
        let menu = Menu::new();

        // 1. App Menu (macOS application menu under "ANNO")
        let app_menu = Submenu::new("ANNO", true);
        let _ = app_menu.append_items(&[
            &PredefinedMenuItem::about(None, None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::services(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(None),
            &PredefinedMenuItem::hide_others(None),
            &PredefinedMenuItem::show_all(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::quit(None),
        ]);

        // 2. File Menu
        let open_image = MenuItem::with_id(
            "open_image",
            "Open Image...",
            true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyO)),
        );
        let open_project = MenuItem::with_id(
            "open_project",
            "Open Project (.anno)...",
            true,
            Some(Accelerator::new(
                Some(Modifiers::SUPER | Modifiers::SHIFT),
                Code::KeyO,
            )),
        );
        let save_project = MenuItem::with_id(
            "save_project",
            "Save Project (.anno)...",
            false,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyS)),
        );
        let export_json = MenuItem::with_id(
            "export_json",
            "Export JSON...",
            false,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyE)),
        );

        let file_menu = Submenu::new("File", true);
        let _ = file_menu.append_items(&[
            &open_image,
            &open_project,
            &PredefinedMenuItem::separator(),
            &save_project,
            &export_json,
        ]);

        // 3. Edit Menu
        let delete_region = MenuItem::with_id(
            "delete_region",
            "Delete Selected Region",
            false,
            Some(Accelerator::new(None, Code::Backspace)),
        );
        let deselect = MenuItem::with_id(
            "deselect",
            "Deselect",
            false,
            Some(Accelerator::new(None, Code::Escape)),
        );

        let edit_menu = Submenu::new("Edit", true);
        let _ = edit_menu.append_items(&[
            &PredefinedMenuItem::undo(None),
            &PredefinedMenuItem::redo(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::cut(None),
            &PredefinedMenuItem::copy(None),
            &PredefinedMenuItem::paste(None),
            &PredefinedMenuItem::select_all(None),
            &PredefinedMenuItem::separator(),
            &delete_region,
            &deselect,
        ]);

        // 4. View Menu
        let reset_view = MenuItem::with_id(
            "reset_view",
            "Reset View (Zoom & Pan)",
            false,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::Digit0)),
        );
        let zoom_in = MenuItem::with_id(
            "zoom_in",
            "Zoom In",
            false,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::Equal)),
        );
        let zoom_out = MenuItem::with_id(
            "zoom_out",
            "Zoom Out",
            false,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::Minus)),
        );

        let view_menu = Submenu::new("View", true);
        let _ = view_menu.append_items(&[
            &reset_view,
            &zoom_in,
            &zoom_out,
        ]);

        let _ = menu.append_items(&[
            &app_menu,
            &file_menu,
            &edit_menu,
            &view_menu,
        ]);

        #[cfg(target_os = "macos")]
        {
            let _ = menu.init_for_nsapp();
        }

        Self {
            menu,
            open_image,
            open_project,
            save_project,
            export_json,
            delete_region,
            deselect,
            reset_view,
            zoom_in,
            zoom_out,
        }
    }

    pub fn update_states(&self, has_image: bool, has_selection: bool) {
        let _ = self.save_project.set_enabled(has_image);
        let _ = self.export_json.set_enabled(has_image);
        let _ = self.reset_view.set_enabled(has_image);
        let _ = self.zoom_in.set_enabled(has_image);
        let _ = self.zoom_out.set_enabled(has_image);

        let _ = self.delete_region.set_enabled(has_selection);
        let _ = self.deselect.set_enabled(has_selection);
    }
}

pub fn handle_native_menu_events(app: &mut AnnotatorApp, ctx: &egui::Context) {
    if let Some(menubar) = &app.native_menubar {
        menubar.update_states(app.image.is_some(), app.selected.is_some());
    }

    while let Ok(event) = MenuEvent::receiver().try_recv() {
        match event.id().as_ref() {
            "open_image" => app.open_image_dialog(ctx),
            "open_project" => app.open_project_dialog(ctx),
            "save_project" => app.save_project_dialog(),
            "export_json" => app.export_dialog(),
            "delete_region" => app.delete_selected(),
            "deselect" => {
                app.selected = None;
                app.editing_label = None;
            }
            "reset_view" => {
                app.zoom = 1.0;
                app.pan = egui::Vec2::ZERO;
            }
            "zoom_in" => {
                app.zoom = (app.zoom * 1.25).min(20.0);
            }
            "zoom_out" => {
                app.zoom = (app.zoom / 1.25).max(1.0);
            }
            _ => {}
        }
    }
}
