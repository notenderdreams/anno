use crate::app::AnnotatorApp;
use eframe::egui;
use muda::{
    Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu,
    accelerator::{Accelerator, Code, Modifiers},
};

pub struct NativeMenuBar {
    pub menu: Menu,
    pub open_image: MenuItem,
    pub open_folder: MenuItem,
    pub open_project: MenuItem,
    pub save_project: MenuItem,
    pub export_json: MenuItem,
    pub export_dataset_json: MenuItem,
    pub crop_export: MenuItem,
    pub copy_position: MenuItem,
    pub undo: MenuItem,
    pub redo: MenuItem,
    pub select_all: MenuItem,
    pub toggle_lock: MenuItem,
    pub delete_region: MenuItem,
    pub deselect: MenuItem,
    pub prev_image: MenuItem,
    pub next_image: MenuItem,
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
        let open_folder = MenuItem::with_id(
            "open_folder",
            "Open Folder...",
            true,
            Some(Accelerator::new(
                Some(Modifiers::SUPER | Modifiers::ALT),
                Code::KeyO,
            )),
        );
        let open_project = MenuItem::with_id(
            "open_project",
            "Open Project or Batch...",
            true,
            Some(Accelerator::new(
                Some(Modifiers::SUPER | Modifiers::SHIFT),
                Code::KeyO,
            )),
        );
        let save_project = MenuItem::with_id(
            "save_project",
            "Save Project or Batch...",
            false,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyS)),
        );
        let export_json = MenuItem::with_id(
            "export_json",
            "Export Image JSON...",
            false,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyE)),
        );
        let export_dataset_json = MenuItem::with_id(
            "export_dataset_json",
            "Export Dataset JSON...",
            false,
            Some(Accelerator::new(
                Some(Modifiers::SUPER | Modifiers::SHIFT),
                Code::KeyE,
            )),
        );
        let crop_export = MenuItem::with_id(
            "crop_export",
            "Crop & Export Selected...",
            false,
            Some(Accelerator::new(
                Some(Modifiers::SUPER | Modifiers::SHIFT),
                Code::KeyC,
            )),
        );

        let file_menu = Submenu::new("File", true);
        let _ = file_menu.append_items(&[
            &open_image,
            &open_folder,
            &open_project,
            &PredefinedMenuItem::separator(),
            &save_project,
            &export_json,
            &export_dataset_json,
            &PredefinedMenuItem::separator(),
            &crop_export,
        ]);

        // 3. Edit Menu
        let undo = MenuItem::with_id(
            "undo",
            "Undo",
            false,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyZ)),
        );
        let redo = MenuItem::with_id(
            "redo",
            "Redo",
            false,
            Some(Accelerator::new(
                Some(Modifiers::SUPER | Modifiers::SHIFT),
                Code::KeyZ,
            )),
        );
        let copy_position = MenuItem::with_id(
            "copy_position",
            "Copy Position (JSON)",
            false,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyC)),
        );
        let select_all = MenuItem::with_id(
            "select_all",
            "Select All",
            false,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyA)),
        );
        let toggle_lock = MenuItem::with_id(
            "toggle_lock",
            "Lock / Unlock Layer",
            false,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyL)),
        );
        let delete_region = MenuItem::with_id(
            "delete_region",
            "Delete Selected",
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
            &undo,
            &redo,
            &PredefinedMenuItem::separator(),
            &copy_position,
            &select_all,
            &toggle_lock,
            &PredefinedMenuItem::separator(),
            &delete_region,
            &deselect,
        ]);

        // 4. View Menu
        let prev_image = MenuItem::with_id(
            "prev_image",
            "Previous Image",
            false,
            Some(Accelerator::new(None, Code::BracketLeft)),
        );
        let next_image = MenuItem::with_id(
            "next_image",
            "Next Image",
            false,
            Some(Accelerator::new(None, Code::BracketRight)),
        );
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
            &prev_image,
            &next_image,
            &PredefinedMenuItem::separator(),
            &reset_view,
            &zoom_in,
            &zoom_out,
        ]);

        let _ = menu.append_items(&[&app_menu, &file_menu, &edit_menu, &view_menu]);

        #[cfg(target_os = "macos")]
        {
            menu.init_for_nsapp();
        }

        Self {
            menu,
            open_image,
            open_folder,
            open_project,
            save_project,
            export_json,
            export_dataset_json,
            crop_export,
            copy_position,
            undo,
            redo,
            select_all,
            toggle_lock,
            delete_region,
            deselect,
            prev_image,
            next_image,
            reset_view,
            zoom_in,
            zoom_out,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_states(
        &self,
        has_image: bool,
        has_dataset: bool,
        has_selection: bool,
        has_annotations: bool,
        can_undo: bool,
        can_redo: bool,
        can_prev: bool,
        can_next: bool,
    ) {
        self.save_project.set_enabled(has_image);
        self.export_json.set_enabled(has_image);
        self.export_dataset_json.set_enabled(has_dataset);
        self.crop_export.set_enabled(has_image && has_selection);
        self.copy_position.set_enabled(has_selection);
        self.reset_view.set_enabled(has_image);
        self.zoom_in.set_enabled(has_image);
        self.zoom_out.set_enabled(has_image);

        self.prev_image.set_enabled(can_prev);
        self.next_image.set_enabled(can_next);

        self.undo.set_enabled(can_undo);
        self.redo.set_enabled(can_redo);

        self.select_all.set_enabled(has_annotations);
        self.toggle_lock.set_enabled(has_selection);
        self.delete_region.set_enabled(has_selection);
        self.deselect.set_enabled(has_selection);
    }
}

pub fn handle_native_menu_events(app: &mut AnnotatorApp, ctx: &egui::Context) {
    if let Some(menubar) = &app.native_menubar {
        let current_idx = app.current_image_idx.unwrap_or(0);
        let total_images = app.image_files.len();
        let can_prev = !app.image_files.is_empty() && current_idx > 0;
        let can_next = !app.image_files.is_empty() && current_idx + 1 < total_images;

        menubar.update_states(
            app.image.is_some(),
            !app.image_files.is_empty(),
            !app.selected.is_empty(),
            !app.annotations.is_empty(),
            app.can_undo(),
            app.can_redo(),
            can_prev,
            can_next,
        );
    }

    while let Ok(event) = MenuEvent::receiver().try_recv() {
        match event.id().as_ref() {
            "open_image" => app.open_image_dialog(ctx),
            "open_folder" => app.open_folder_dialog(ctx),
            "open_project" => app.open_project_dialog(ctx),
            "save_project" => app.save_project_dialog(),
            "export_json" => app.export_dialog(),
            "export_dataset_json" => app.export_unified_dataset_dialog(),
            "crop_export" => app.crop_and_export_selected(),
            "copy_position" => app.copy_selected_position_to_clipboard(ctx),
            "undo" => app.undo(),
            "redo" => app.redo(),
            "select_all" => app.select_all(),
            "toggle_lock" => app.toggle_lock_selected(),
            "prev_image" => app.previous_image(ctx),
            "next_image" => app.next_image(ctx),
            "delete_region" => app.delete_selected(),
            "deselect" => app.deselect_all(),
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
