use eframe::egui::{self, Color32, FontFamily, FontId, Margin, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use crate::app::AnnotatorApp;
use crate::models::ToolMode;
use crate::render::draw_lucide_lock;
use crate::theme::{MUTED, PANEL, RED};

pub fn render_left_sidebar(app: &mut AnnotatorApp, ctx: &egui::Context) {
    egui::SidePanel::left("left_sidebar")
        .exact_width(230.0)
        .resizable(false)
        .frame(
            egui::Frame::none()
                .fill(PANEL)
                .inner_margin(Margin::same(14.0)),
        )
        .show(ctx, |ui| {
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("DRAWING TOOL")
                        .size(10.0)
                        .strong()
                        .color(MUTED),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let is_poly = app.tool_mode == ToolMode::Polygon;
                    let poly_btn = ui.add_sized(
                        Vec2::new(76.0, 18.0),
                        egui::Button::new(
                            RichText::new("POLYGON [P]")
                                .size(8.5)
                                .monospace()
                                .strong()
                                .color(if is_poly { Color32::WHITE } else { MUTED }),
                        )
                        .fill(if is_poly { Color32::from_rgb(30, 60, 120) } else { Color32::from_gray(24) })
                        .stroke(Stroke::new(1.0_f32, if is_poly { Color32::from_rgb(70, 130, 240) } else { Color32::from_gray(45) })),
                    );
                    if poly_btn.clicked() {
                        app.tool_mode = ToolMode::Polygon;
                        app.draft = None;
                        app.status = "POLYGON TOOL (CLICK TO PLACE POINTS, 3+ TO CLOSE)".into();
                    }

                    let is_rect = app.tool_mode == ToolMode::Rectangle;
                    let rect_btn = ui.add_sized(
                        Vec2::new(54.0, 18.0),
                        egui::Button::new(
                            RichText::new("BOX [B]")
                                .size(8.5)
                                .monospace()
                                .strong()
                                .color(if is_rect { Color32::WHITE } else { MUTED }),
                        )
                        .fill(if is_rect { Color32::from_rgb(30, 60, 120) } else { Color32::from_gray(24) })
                        .stroke(Stroke::new(1.0_f32, if is_rect { Color32::from_rgb(70, 130, 240) } else { Color32::from_gray(45) })),
                    );
                    if rect_btn.clicked() {
                        app.tool_mode = ToolMode::Rectangle;
                        app.draft_polygon = None;
                        app.status = "BOX TOOL (DRAG TO DRAW)".into();
                    }
                });
            });

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);

            if !app.image_files.is_empty() {
                let current_idx = app.current_image_idx.unwrap_or(0);
                let total_images = app.image_files.len();
                let folder_name = app
                    .dataset_folder
                    .as_ref()
                    .and_then(|f| f.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("BATCH");

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("DATASET")
                            .size(9.0)
                            .strong()
                            .color(MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{:02}/{:02}", current_idx + 1, total_images))
                                .size(9.0)
                                .monospace()
                                .strong()
                                .color(RED),
                        );
                    });
                });

                ui.add_space(2.0);
                ui.label(
                    RichText::new(folder_name)
                        .size(11.0)
                        .monospace()
                        .color(Color32::WHITE),
                );

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);
            }

            let presets_open_id = ui.make_persistent_id("presets_section_open");
            let mut presets_open: bool = ui.data_mut(|d| *d.get_temp_mut_or(presets_open_id, true));

            ui.horizontal(|ui| {
                let arrow = if presets_open { "▼" } else { "▶" };
                if ui
                    .add(egui::Button::new(RichText::new(arrow).monospace().size(8.0).color(MUTED)).frame(false))
                    .clicked()
                {
                    presets_open = !presets_open;
                    ui.data_mut(|d| d.insert_temp(presets_open_id, presets_open));
                }
                ui.label(
                    RichText::new("CLASS PRESETS")
                        .size(10.0)
                        .strong()
                        .color(MUTED),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(active) = app.presets.get(app.active_preset_idx) {
                        ui.label(
                            RichText::new(format!("[{}] {}", app.active_preset_idx + 1, active.prefix.to_uppercase()))
                                .size(9.0)
                                .monospace()
                                .strong()
                                .color(active.color32()),
                        );
                    }
                });
            });

            if presets_open {
                ui.add_space(4.0);
                let mut apply_idx = None;

                for idx in 0..app.presets.len() {
                    let is_active = app.active_preset_idx == idx;
                    let preset_color = app.presets[idx].color32();

                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;

                        let key_badge = format!("{}:", idx + 1);
                        let badge_resp = ui.add(
                            egui::Button::new(
                                RichText::new(key_badge)
                                    .size(9.0)
                                    .monospace()
                                    .color(if is_active { Color32::WHITE } else { MUTED }),
                            )
                            .fill(if is_active { Color32::from_gray(36) } else { Color32::TRANSPARENT })
                            .frame(is_active),
                        );
                        if badge_resp.clicked() {
                            apply_idx = Some(idx);
                        }

                        let popup_id = ui.make_persistent_id(format!("preset_color_picker_{idx}"));
                        let (swatch_rect, swatch_resp) =
                            ui.allocate_exact_size(Vec2::splat(12.0), Sense::click());
                        if ui.is_rect_visible(swatch_rect) {
                            ui.painter().rect_filled(swatch_rect, 2.0, preset_color);
                            ui.painter().rect_stroke(
                                swatch_rect,
                                2.0,
                                Stroke::new(1.0_f32, if is_active { Color32::WHITE } else { Color32::from_gray(60) }),
                            );
                        }
                        if swatch_resp.clicked() {
                            ui.memory_mut(|mem| mem.toggle_popup(popup_id));
                        }

                        egui::popup_below_widget(
                            ui,
                            popup_id,
                            &swatch_resp,
                            egui::PopupCloseBehavior::CloseOnClickOutside,
                            |ui| {
                                ui.set_max_width(200.0);
                                let mut color32 = preset_color;
                                if egui::color_picker::color_picker_color32(
                                    ui,
                                    &mut color32,
                                    egui::color_picker::Alpha::Opaque,
                                ) {
                                    app.presets[idx].color = [color32.r(), color32.g(), color32.b()];
                                }
                            },
                        );

                        let edit = ui.add(
                            egui::TextEdit::singleline(&mut app.presets[idx].prefix)
                                .font(FontId::monospace(9.5))
                                .desired_width(ui.available_width())
                                .text_color(if is_active { Color32::WHITE } else { Color32::from_gray(190) }),
                        );
                        if edit.clicked() {
                            apply_idx = Some(idx);
                        }
                    });
                }

                if let Some(idx) = apply_idx {
                    app.apply_preset(idx);
                }
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("SCENE HIERARCHY")
                        .size(10.0)
                        .strong()
                        .color(MUTED),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let count_text = if app.selected.len() > 1 {
                        format!("{}/{}", app.selected.len(), app.annotations.len())
                    } else {
                        app.annotations.len().to_string()
                    };
                    ui.label(
                        RichText::new(count_text)
                            .size(11.0)
                            .strong()
                            .color(RED),
                    );
                });
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            if app.annotations.is_empty() {
                ui.add_space(24.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("NO REGIONS")
                            .family(FontFamily::Monospace)
                            .size(11.0)
                            .color(MUTED),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Draw rectangles on the image\nto build a scene hierarchy.")
                            .size(10.0)
                            .color(Color32::from_gray(92)),
                    );
                });
            } else {
                let root_ids: Vec<u32> = app
                    .annotations
                    .iter()
                    .filter(|a| a.parent_id.is_none())
                    .map(|a| a.id)
                    .collect();

                let total_roots = root_ids.len();

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for (idx, &root_id) in root_ids.iter().enumerate() {
                            let is_last_root = idx == total_roots - 1;
                            render_tree_branch(ui, app, root_id, 0, &[], is_last_root);
                        }
                    });
            }
        });
}

fn render_tree_branch(
    ui: &mut egui::Ui,
    app: &mut AnnotatorApp,
    annotation_id: u32,
    depth: usize,
    ancestors_is_last: &[bool],
    is_last_sibling: bool,
) {
    let (label, color32, is_selected, is_locked) = {
        let Some(a) = app.annotations.iter().find(|a| a.id == annotation_id) else {
            return;
        };
        (a.label.clone(), a.color32(), app.is_selected(annotation_id), a.locked)
    };

    let children_ids: Vec<u32> = app
        .annotations
        .iter()
        .filter(|a| a.parent_id == Some(annotation_id))
        .map(|a| a.id)
        .collect();
    let has_children = !children_ids.is_empty();

    let open_id = ui.make_persistent_id(("tree_open", annotation_id));
    let mut is_open: bool = ui.data_mut(|d| *d.get_temp_mut_or(open_id, true));

    let row_height = 24.0;
    let row_rect = Rect::from_min_size(
        ui.cursor().min,
        Vec2::new(ui.available_width(), row_height),
    );

    let response = ui.allocate_rect(row_rect, Sense::click());

    let bg_color = if is_selected {
        Color32::from_gray(38)
    } else if response.hovered() {
        Color32::from_gray(26)
    } else {
        Color32::TRANSPARENT
    };

    let indent_step = 14.0_f32;
    let line_color = Color32::from_gray(65);

    let mut lock_clicked = false;

    if ui.is_rect_visible(row_rect) {
        ui.painter().rect_filled(row_rect, 2.0, bg_color);

        // Render ancestor vertical spine lines for levels above direct parent
        for (d, &ancestor_last) in ancestors_is_last.iter().enumerate() {
            if !ancestor_last && d < depth.saturating_sub(1) {
                let ancestor_spine_x = row_rect.left() + (d as f32 * indent_step) + 23.0;
                ui.painter().line_segment(
                    [
                        Pos2::new(ancestor_spine_x, row_rect.top()),
                        Pos2::new(ancestor_spine_x, row_rect.bottom()),
                    ],
                    Stroke::new(1.0_f32, line_color),
                );
            }
        }

        // Render direct parent connector line for depth > 0
        if depth > 0 {
            let parent_spine_x = row_rect.left() + ((depth - 1) as f32 * indent_step) + 23.0;

            // If this child is the last child of its parent, end vertical line at row center (└──)
            let end_vertical_y = if is_last_sibling {
                row_rect.center().y
            } else {
                row_rect.bottom()
            };

            // Vertical spine line
            ui.painter().line_segment(
                [
                    Pos2::new(parent_spine_x, row_rect.top()),
                    Pos2::new(parent_spine_x, end_vertical_y),
                ],
                Stroke::new(1.0_f32, line_color),
            );

            // Horizontal branch line extending rightward to child's swatch
            let child_swatch_left = parent_spine_x + 9.0;
            ui.painter().line_segment(
                [
                    Pos2::new(parent_spine_x, row_rect.center().y),
                    Pos2::new(child_swatch_left, row_rect.center().y),
                ],
                Stroke::new(1.0_f32, line_color),
            );
        }

        // Continue this node's spine into the first child row. Child rows draw
        // their connector starting at their top edge, so without this segment
        // there is a visible half-row gap between a parent and its children.
        if has_children && is_open {
            let child_spine_x = row_rect.left() + (depth as f32 * indent_step) + 23.0;
            ui.painter().line_segment(
                [
                    Pos2::new(child_spine_x, row_rect.center().y),
                    Pos2::new(child_spine_x, row_rect.bottom()),
                ],
                Stroke::new(1.0_f32, line_color),
            );
        }

        let indent_x = (depth as f32) * indent_step;
        let mut curr_x = row_rect.left() + indent_x + 4.0;

        if has_children {
            let arrow_rect = Rect::from_center_size(
                Pos2::new(curr_x + 6.0, row_rect.center().y),
                Vec2::splat(12.0),
            );
            let arrow_response = ui.allocate_rect(arrow_rect, Sense::click());

            let arrow_text = if is_open { "▼" } else { "▶" };
            let arrow_color = if arrow_response.hovered() {
                Color32::WHITE
            } else {
                Color32::from_gray(140)
            };
            ui.painter().text(
                arrow_rect.center(),
                egui::Align2::CENTER_CENTER,
                arrow_text,
                FontId::monospace(8.0),
                arrow_color,
            );

            if arrow_response.clicked() {
                is_open = !is_open;
                ui.data_mut(|d| d.insert_temp(open_id, is_open));
            }
        }

        curr_x += 14.0;

        let swatch_rect = Rect::from_center_size(
            Pos2::new(curr_x + 5.0, row_rect.center().y),
            Vec2::splat(10.0),
        );
        ui.painter().rect_filled(swatch_rect, 2.0, color32);
        curr_x += 14.0;

        let text_color = if is_selected {
            Color32::WHITE
        } else if is_locked {
            Color32::from_gray(150)
        } else {
            Color32::from_gray(190)
        };
        ui.painter().text(
            Pos2::new(curr_x, row_rect.center().y),
            egui::Align2::LEFT_CENTER,
            &label,
            FontId::monospace(11.0),
            text_color,
        );

        let id_tag = format!("#{:02}", annotation_id);
        ui.painter().text(
            Pos2::new(row_rect.right() - 8.0, row_rect.center().y),
            egui::Align2::RIGHT_CENTER,
            id_tag,
            FontId::monospace(9.0),
            Color32::from_gray(100),
        );

        let lock_center = Pos2::new(row_rect.right() - 36.0, row_rect.center().y);
        let lock_rect = Rect::from_center_size(lock_center, Vec2::splat(18.0));
        let lock_response = ui.allocate_rect(lock_rect, Sense::click());
        if lock_response.clicked() {
            lock_clicked = true;
        }

        if is_locked {
            draw_lucide_lock(
                ui.painter(),
                lock_center,
                11.0,
                true,
                Color32::from_rgb(255, 179, 0),
                1.3,
            );
        } else if response.hovered() || lock_response.hovered() {
            draw_lucide_lock(
                ui.painter(),
                lock_center,
                11.0,
                false,
                Color32::from_gray(110),
                1.3,
            );
        }
    }

    ui.advance_cursor_after_rect(row_rect);

    if lock_clicked {
        app.toggle_lock_annotation(annotation_id);
    } else if response.double_clicked() {
        let is_locked = app.annotations.iter().find(|a| a.id == annotation_id).map_or(false, |a| a.locked);
        if !is_locked {
            app.select_single(annotation_id);
            app.history.begin_edit(app.current_snapshot());
            app.editing_label = Some(annotation_id);
            app.request_label_focus = true;
        }
    } else if response.clicked() {
        let modifier = ui.input(|i| i.modifiers.shift || i.modifiers.command || i.modifiers.ctrl);
        if modifier {
            app.toggle_select(annotation_id);
        } else {
            app.select_single(annotation_id);
        }
    }

    if has_children && is_open {
        let total_children = children_ids.len();
        let mut new_ancestors = ancestors_is_last.to_vec();

        // Roots do not have a parent spine. Only record the sibling state for
        // non-root nodes so ancestor spine indices stay aligned with depth.
        if depth > 0 {
            new_ancestors.push(is_last_sibling);
        }

        for (c_idx, &child_id) in children_ids.iter().enumerate() {
            let is_last_child = c_idx == total_children - 1;
            render_tree_branch(
                ui,
                app,
                child_id,
                depth + 1,
                &new_ancestors,
                is_last_child,
            );
        }
    }
}
