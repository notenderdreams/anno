use eframe::egui::{self, Color32, FontFamily, FontId, Margin, Pos2, RichText, Sense, Stroke, Vec2};
use crate::app::AnnotatorApp;
use crate::geometry::update_hierarchy;
use crate::render::draw_lucide_lock;
use crate::theme::{MUTED, PANEL, RED};

pub fn render_right_sidebar(app: &mut AnnotatorApp, ctx: &egui::Context) {
    let mut export_requested = false;

    let (img_w, img_h) = app
        .image
        .as_ref()
        .map(|img| (img.width as f32, img.height as f32))
        .unwrap_or((10000.0, 10000.0));

    let mut edit_started = false;
    let mut edit_committed = false;
    let mut picked_color = None;

    egui::SidePanel::right("right_sidebar")
        .exact_width(240.0)
        .resizable(false)
        .frame(
            egui::Frame::none()
                .fill(PANEL)
                .inner_margin(Margin::same(16.0)),
        )
        .show(ctx, |ui| {
            ui.add_space(4.0);

            if app.selected.len() == 1 {
                let selected_id = *app.selected.iter().next().unwrap();
                let mut should_delete = false;
                let mut bounds_changed = false;

                if let Some(annotation) = app.annotations.iter_mut().find(|a| a.id == selected_id) {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("REGION {:02}", annotation.id))
                                .size(9.0)
                                .color(RED),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let (lock_label, lock_color) = if annotation.locked {
                                ("LOCKED", Color32::from_rgb(255, 179, 0))
                            } else {
                                ("UNLOCKED", MUTED)
                            };
                            let btn_size = Vec2::new(76.0, 20.0);
                            let (btn_rect, btn_response) = ui.allocate_exact_size(btn_size, Sense::click());
                            if ui.is_rect_visible(btn_rect) {
                                let bg = if btn_response.hovered() { Color32::from_gray(32) } else { Color32::from_gray(22) };
                                ui.painter().rect_filled(btn_rect, 2.0, bg);
                                ui.painter().rect_stroke(btn_rect, 2.0, Stroke::new(1.0_f32, Color32::from_gray(50)));
                                let icon_center = Pos2::new(btn_rect.left() + 11.0, btn_rect.center().y);
                                draw_lucide_lock(ui.painter(), icon_center, 10.0, annotation.locked, lock_color, 1.2);
                                ui.painter().text(
                                    Pos2::new(btn_rect.left() + 20.0, btn_rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    lock_label,
                                    FontId::monospace(9.0),
                                    lock_color,
                                );
                            }
                            if btn_response.clicked() {
                                annotation.locked = !annotation.locked;
                                edit_committed = true;
                            }
                        });
                    });
                    ui.add_space(8.0);
                    ui.label(RichText::new("LABEL").size(9.0).color(MUTED));

                    let response = ui.add(
                        egui::TextEdit::singleline(&mut annotation.label)
                            .font(FontId::monospace(12.0))
                            .desired_width(f32::INFINITY)
                            .margin(Vec2::new(8.0, 7.0)),
                    );

                    if app.request_label_focus {
                        response.request_focus();
                        app.request_label_focus = false;
                    }

                    if response.gained_focus() {
                        edit_started = true;
                    }
                    if response.lost_focus() || (response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                        edit_committed = true;
                    }

                    ui.add_space(10.0);
                    ui.label(RichText::new("DESCRIPTION").size(9.0).color(MUTED));
                    ui.add_space(4.0);

                    let mut desc = annotation.description.clone().unwrap_or_default();
                    let desc_response = ui.add(
                        egui::TextEdit::multiline(&mut desc)
                            .font(FontId::monospace(11.0))
                            .desired_width(f32::INFINITY)
                            .desired_rows(3)
                            .margin(Vec2::new(8.0, 7.0)),
                    );
                    if desc_response.gained_focus() {
                        edit_started = true;
                    }
                    if desc_response.changed() {
                        annotation.description = if desc.trim().is_empty() {
                            None
                        } else {
                            Some(desc)
                        };
                    }
                    if desc_response.lost_focus() {
                        edit_committed = true;
                    }

                    ui.add_space(12.0);
                    ui.label(RichText::new("COLOR PRESETS").size(9.0).color(MUTED));
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        let presets: [[u8; 3]; 6] = [
                            [255, 0, 0],   // Red
                            [0, 230, 118], // Green
                            [41, 121, 255], // Blue
                            [255, 214, 0], // Yellow
                            [255, 145, 0], // Orange
                            [0, 229, 255], // Cyan
                        ];

                        for rgb in presets {
                            let color32 = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                            let is_selected = annotation.color == rgb;

                            let (rect, response) =
                                ui.allocate_exact_size(Vec2::splat(20.0), egui::Sense::click());
                            if ui.is_rect_visible(rect) {
                                ui.painter().rect_filled(rect, 2.0, color32);
                                if is_selected {
                                    ui.painter().rect_stroke(
                                        rect.expand(2.0),
                                        2.0,
                                        Stroke::new(2.0_f32, Color32::WHITE),
                                    );
                                } else if response.hovered() {
                                    ui.painter().rect_stroke(
                                        rect.expand(1.0),
                                        2.0,
                                        Stroke::new(1.0_f32, Color32::GRAY),
                                    );
                                }
                            }
                            if response.clicked() && annotation.color != rgb {
                                picked_color = Some(rgb);
                            }
                        }

                        ui.add_space(4.0);

                        let (rect, picker_response) =
                            ui.allocate_exact_size(Vec2::splat(20.0), egui::Sense::click());
                        if ui.is_rect_visible(rect) {
                            ui.painter().rect_filled(rect, 2.0, annotation.color32());
                            ui.painter().rect_stroke(
                                rect,
                                2.0,
                                Stroke::new(1.0_f32, Color32::from_gray(120)),
                            );
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "+",
                                FontId::monospace(12.0),
                                Color32::WHITE,
                            );
                        }

                        let popup_id = ui.make_persistent_id("custom_color_picker_popup");
                        if picker_response.clicked() {
                            edit_started = true;
                            ui.memory_mut(|mem| mem.toggle_popup(popup_id));
                        }

                        egui::popup_below_widget(
                            ui,
                            popup_id,
                            &picker_response,
                            egui::PopupCloseBehavior::CloseOnClickOutside,
                            |ui| {
                                ui.set_max_width(200.0);
                                let mut color32 = annotation.color32();
                                if egui::color_picker::color_picker_color32(
                                    ui,
                                    &mut color32,
                                    egui::color_picker::Alpha::Opaque,
                                ) {
                                    annotation.color = [color32.r(), color32.g(), color32.b()];
                                    edit_started = true;
                                }
                            },
                        );
                    });

                    ui.add_space(12.0);
                    ui.label(RichText::new("BOUNDS (PX)").size(9.0).color(MUTED));
                    ui.add_space(4.0);

                    let is_locked = annotation.locked;
                    let max_x = (img_w - annotation.width).max(0.0);
                    let max_y = (img_h - annotation.height).max(0.0);
                    let max_w = (img_w - annotation.x).max(8.0);
                    let max_h = (img_h - annotation.y).max(8.0);

                    ui.add_enabled_ui(!is_locked, |ui| {
                        egui::Grid::new("right_bounds_grid")
                            .num_columns(2)
                            .spacing([12.0, 6.0])
                            .show(ui, |ui| {
                                let (x_ch, x_start, x_stop) = bound_field(ui, "X", &mut annotation.x, 0.0, max_x);
                                let (y_ch, y_start, y_stop) = bound_field(ui, "Y", &mut annotation.y, 0.0, max_y);
                                ui.end_row();
                                let (w_ch, w_start, w_stop) = bound_field(ui, "W", &mut annotation.width, 8.0, max_w);
                                let (h_ch, h_start, h_stop) = bound_field(ui, "H", &mut annotation.height, 8.0, max_h);
                                ui.end_row();

                                if x_ch || y_ch || w_ch || h_ch {
                                    bounds_changed = true;
                                }
                                if x_start || y_start || w_start || h_start {
                                    edit_started = true;
                                }
                                if x_stop || y_stop || w_stop || h_stop {
                                    edit_committed = true;
                                }
                            });
                    });

                    ui.add_space(16.0);
                    ui.add_enabled_ui(!is_locked, |ui| {
                        if ui
                            .add_sized(
                                [ui.available_width(), 30.0],
                                egui::Button::new(RichText::new("DELETE REGION").size(10.0).color(RED)),
                            )
                            .clicked()
                        {
                            should_delete = true;
                        }
                    });
                }

                if bounds_changed {
                    update_hierarchy(&mut app.annotations);
                }

                if let Some(color) = picked_color {
                    app.history.record(app.current_snapshot());
                    if let Some(annotation) = app.annotations.iter_mut().find(|a| a.id == selected_id) {
                        annotation.color = color;
                    }
                }

                if edit_started {
                    app.history.begin_edit(app.current_snapshot());
                }

                if edit_committed {
                    app.history.commit_edit(&app.current_snapshot());
                }

                if should_delete {
                    app.delete_selected();
                }
            } else if app.selected.len() > 1 {
                let mut should_delete = false;
                let count = app.selected.len();

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{count} REGIONS SELECTED"))
                            .size(9.0)
                            .strong()
                            .color(RED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let all_locked = app
                            .annotations
                            .iter()
                            .filter(|a| app.selected.contains(&a.id))
                            .all(|a| a.locked);
                        let (lock_label, lock_color) = if all_locked {
                            ("UNLOCK ALL", Color32::WHITE)
                        } else {
                            ("LOCK ALL", Color32::from_rgb(255, 179, 0))
                        };
                        let btn_size = Vec2::new(84.0, 20.0);
                        let (btn_rect, btn_response) = ui.allocate_exact_size(btn_size, Sense::click());
                        if ui.is_rect_visible(btn_rect) {
                            let bg = if btn_response.hovered() { Color32::from_gray(32) } else { Color32::from_gray(22) };
                            ui.painter().rect_filled(btn_rect, 2.0, bg);
                            ui.painter().rect_stroke(btn_rect, 2.0, Stroke::new(1.0_f32, Color32::from_gray(50)));
                            let icon_center = Pos2::new(btn_rect.left() + 11.0, btn_rect.center().y);
                            draw_lucide_lock(ui.painter(), icon_center, 10.0, !all_locked, lock_color, 1.2);
                            ui.painter().text(
                                Pos2::new(btn_rect.left() + 20.0, btn_rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                lock_label,
                                FontId::monospace(9.0),
                                lock_color,
                            );
                        }
                        if btn_response.clicked() {
                            app.toggle_lock_selected();
                        }
                    });
                });
                ui.add_space(8.0);
                ui.label(RichText::new("SET LABEL (ALL)").size(9.0).color(MUTED));

                let first_id = *app.selected.iter().next().unwrap();
                let all_same_label = {
                    let first_label = app.annotations.iter().find(|a| a.id == first_id).map(|a| &a.label);
                    app.annotations
                        .iter()
                        .filter(|a| app.selected.contains(&a.id))
                        .all(|a| Some(&a.label) == first_label)
                };
                let mut batch_label = if all_same_label {
                    app.annotations
                        .iter()
                        .find(|a| a.id == first_id)
                        .map(|a| a.label.clone())
                        .unwrap_or_default()
                } else {
                    String::new()
                };

                let response = ui.add(
                    egui::TextEdit::singleline(&mut batch_label)
                        .font(FontId::monospace(12.0))
                        .desired_width(f32::INFINITY)
                        .hint_text("Set common label...")
                        .margin(Vec2::new(8.0, 7.0)),
                );

                if response.gained_focus() {
                    edit_started = true;
                }
                if response.changed() {
                    for a in app.annotations.iter_mut().filter(|a| app.selected.contains(&a.id)) {
                        a.label = batch_label.clone();
                    }
                }
                if response.lost_focus() || (response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                    edit_committed = true;
                }

                ui.add_space(10.0);
                ui.label(RichText::new("SET DESCRIPTION (ALL)").size(9.0).color(MUTED));
                ui.add_space(4.0);

                let all_same_desc = {
                    let first_desc = app
                        .annotations
                        .iter()
                        .find(|a| a.id == first_id)
                        .and_then(|a| a.description.as_ref());
                    app.annotations
                        .iter()
                        .filter(|a| app.selected.contains(&a.id))
                        .all(|a| a.description.as_ref() == first_desc)
                };
                let mut batch_desc = if all_same_desc {
                    app.annotations
                        .iter()
                        .find(|a| a.id == first_id)
                        .and_then(|a| a.description.clone())
                        .unwrap_or_default()
                } else {
                    String::new()
                };

                let desc_response = ui.add(
                    egui::TextEdit::multiline(&mut batch_desc)
                        .font(FontId::monospace(11.0))
                        .desired_width(f32::INFINITY)
                        .desired_rows(2)
                        .hint_text("Set common description...")
                        .margin(Vec2::new(8.0, 7.0)),
                );
                if desc_response.gained_focus() {
                    edit_started = true;
                }
                if desc_response.changed() {
                    let new_desc = if batch_desc.trim().is_empty() {
                        None
                    } else {
                        Some(batch_desc.clone())
                    };
                    for a in app.annotations.iter_mut().filter(|a| app.selected.contains(&a.id)) {
                        a.description = new_desc.clone();
                    }
                }
                if desc_response.lost_focus() {
                    edit_committed = true;
                }

                ui.add_space(12.0);
                ui.label(RichText::new("SET COLOR (ALL)").size(9.0).color(MUTED));
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    let presets: [[u8; 3]; 6] = [
                        [255, 0, 0],   // Red
                        [0, 230, 118], // Green
                        [41, 121, 255], // Blue
                        [255, 214, 0], // Yellow
                        [255, 145, 0], // Orange
                        [0, 229, 255], // Cyan
                    ];

                    for rgb in presets {
                        let color32 = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                        let (rect, response) =
                            ui.allocate_exact_size(Vec2::splat(20.0), egui::Sense::click());
                        if ui.is_rect_visible(rect) {
                            ui.painter().rect_filled(rect, 2.0, color32);
                            if response.hovered() {
                                ui.painter().rect_stroke(
                                    rect.expand(1.0),
                                    2.0,
                                    Stroke::new(1.0_f32, Color32::GRAY),
                                );
                            }
                        }
                        if response.clicked() {
                            picked_color = Some(rgb);
                        }
                    }

                    ui.add_space(4.0);

                    let (rect, picker_response) =
                        ui.allocate_exact_size(Vec2::splat(20.0), egui::Sense::click());
                    if ui.is_rect_visible(rect) {
                        ui.painter().rect_filled(rect, 2.0, Color32::from_gray(40));
                        ui.painter().rect_stroke(
                            rect,
                            2.0,
                            Stroke::new(1.0_f32, Color32::from_gray(120)),
                        );
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "+",
                            FontId::monospace(12.0),
                            Color32::WHITE,
                        );
                    }

                    let popup_id = ui.make_persistent_id("batch_color_picker_popup");
                    if picker_response.clicked() {
                        edit_started = true;
                        ui.memory_mut(|mem| mem.toggle_popup(popup_id));
                    }

                    egui::popup_below_widget(
                        ui,
                        popup_id,
                        &picker_response,
                        egui::PopupCloseBehavior::CloseOnClickOutside,
                        |ui| {
                            ui.set_max_width(200.0);
                            let mut color32 = Color32::RED;
                            if egui::color_picker::color_picker_color32(
                                ui,
                                &mut color32,
                                egui::color_picker::Alpha::Opaque,
                            ) {
                                picked_color = Some([color32.r(), color32.g(), color32.b()]);
                            }
                        },
                    );
                });


                ui.add_space(14.0);
                if ui
                    .add_sized(
                        [ui.available_width(), 30.0],
                        egui::Button::new(RichText::new(format!("DELETE {count} REGIONS")).size(10.0).color(RED)),
                    )
                    .clicked()
                {
                    should_delete = true;
                }

                ui.add_space(4.0);
                if ui
                    .add_sized(
                        [ui.available_width(), 24.0],
                        egui::Button::new(RichText::new("CLEAR SELECTION").size(9.0).color(MUTED)),
                    )
                    .clicked()
                {
                    app.deselect_all();
                }

                if let Some(color) = picked_color {
                    app.history.record(app.current_snapshot());
                    for a in app.annotations.iter_mut().filter(|a| app.selected.contains(&a.id)) {
                        a.color = color;
                    }
                }

                if edit_started {
                    app.history.begin_edit(app.current_snapshot());
                }

                if edit_committed {
                    app.history.commit_edit(&app.current_snapshot());
                }

                if should_delete {
                    app.delete_selected();
                }
            } else {
                ui.add_space(16.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("┌   ┐\n\n└   ┘")
                            .family(FontFamily::Monospace)
                            .size(18.0)
                            .color(Color32::from_gray(62)),
                    );
                    ui.add_space(8.0);
                    ui.label(RichText::new("NO REGION SELECTED").size(10.0).color(MUTED));
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Click, Shift+drag, or drag on image to annotate.")
                            .size(10.0)
                            .color(Color32::from_gray(92)),
                    );
                });
            }

            if app.image.is_some() {
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    if app.image_files.len() > 1 {
                        if ui
                            .add_sized(
                                [ui.available_width(), 30.0],
                                egui::Button::new(
                                    RichText::new("EXPORT DATASET JSON")
                                        .size(10.0)
                                        .strong()
                                        .color(Color32::BLACK),
                                )
                                .fill(Color32::WHITE)
                                .rounding(0.0),
                            )
                            .clicked()
                        {
                            app.export_unified_dataset_dialog();
                        }
                        ui.add_space(4.0);
                        if ui
                            .add_sized(
                                [ui.available_width(), 26.0],
                                egui::Button::new(
                                    RichText::new("EXPORT IMAGE JSON")
                                        .size(9.0)
                                        .color(Color32::from_gray(200)),
                                )
                                .fill(Color32::from_gray(24))
                                .rounding(0.0),
                            )
                            .clicked()
                        {
                            export_requested = true;
                        }
                    } else {
                        export_requested = ui
                            .add_sized(
                                [ui.available_width(), 32.0],
                                egui::Button::new(
                                    RichText::new("EXPORT JSON")
                                        .size(10.0)
                                        .strong()
                                        .color(Color32::BLACK),
                                )
                                .fill(Color32::WHITE)
                                .rounding(0.0),
                            )
                            .clicked();
                    }
                });
            }
        });

    if export_requested {
        app.save_dialog();
    }
}

fn bound_field(
    ui: &mut egui::Ui,
    label: &str,
    val: &mut f32,
    min: f32,
    max: f32,
) -> (bool, bool, bool) {
    let mut changed = false;
    let mut started = false;
    let mut stopped = false;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;

        let label_response = ui.add(
            egui::Label::new(
                RichText::new(label)
                    .font(FontId::monospace(10.0))
                    .strong()
                    .color(RED),
            )
            .sense(egui::Sense::click_and_drag()),
        );

        if label_response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }

        if label_response.drag_started() {
            started = true;
        }

        if label_response.dragged() {
            let delta = ui.input(|i| i.pointer.delta().x);
            if delta != 0.0 {
                let speed = if ui.input(|i| i.modifiers.shift) { 0.1 } else { 1.0 };
                *val = (*val + delta * speed).clamp(min, max).round();
                changed = true;
            }
        }

        if label_response.drag_stopped() {
            stopped = true;
        }

        let drag_val = egui::DragValue::new(val)
            .speed(1.0)
            .range(min..=max);

        let drag_response = ui.add(drag_val);

        if drag_response.drag_started() || drag_response.gained_focus() {
            started = true;
        }

        if drag_response.changed() {
            *val = val.round();
            changed = true;
        }

        if drag_response.drag_stopped() || drag_response.lost_focus() {
            stopped = true;
        }
    });

    (changed, started, stopped)
}
