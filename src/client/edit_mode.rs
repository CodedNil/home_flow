use crate::{
    client::{
        edit_mode_utils::{
            apply_standard_transform, combo_box_for_enum, combo_box_for_materials, edit_option,
            edit_rotation, edit_vec2, labelled_widget,
        },
        networking::save_layout,
        vec2_to_egui_pos, HomeFlow,
    },
    common::{
        color::Color,
        furniture::{ChairType, Furniture, FurnitureType, TableType},
        layout::{
            Action, GlobalMaterial, Home, Light, MultiLight, Opening, OpeningType, Operation,
            Outline, Room, Sensor, TileOptions, Walls, Zone,
        },
        utils::Material,
    },
};
use egui::{
    collapsing_header::CollapsingState, Align2, Button, Color32, CursorIcon, DragValue,
    PointerButton, TextEdit, Ui, Window,
};
use glam::{dvec2 as vec2, DVec2 as Vec2};
use std::time::Duration;
use uuid::Uuid;

nestify::nest! {
    #[derive(Default)]
    pub struct EditDetails {
        pub enabled: bool,
        pub drag_data: Option<pub struct DragData {
            pub id: Uuid,
            pub object_type: ObjectType,
            pub manipulation_type: ManipulationType,
            pub mouse_start_pos: Vec2,
            pub start_pos: Vec2,
            pub start_size: Vec2,
            pub start_rotation: i32,
        }>,
        pub selected_id: Option<Uuid>,
        pub selected_type: Option<ObjectType>,
        pub preview_edits: bool,
        pub resize_enabled: bool,
        pub material_editor_open: bool,
    }
}

#[derive(Debug)]
pub struct HoverDetails {
    pub id: Uuid,
    pub object_type: ObjectType,
    pub can_drag: bool,
    pub pos: Vec2,
    pub size: Vec2,
    pub rotation: i32,
    pub manipulation_type: ManipulationType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectType {
    Room,
    Operation,
    Zone,
    Opening,
    Light,
    Furniture,
}

#[derive(Clone, Copy, Debug)]
pub enum ManipulationType {
    Move,
    ResizeLeft,
    ResizeRight,
    ResizeTop,
    ResizeBottom,
}

impl ManipulationType {
    pub const fn sign(self) -> f64 {
        match self {
            Self::Move => 0.0,
            Self::ResizeLeft | Self::ResizeBottom => -1.0,
            Self::ResizeRight | Self::ResizeTop => 1.0,
        }
    }
}

pub struct EditResponse {
    pub used_dragged: bool,
    pub hovered_id: Option<Uuid>,
    pub snap_line_x: Option<f64>,
    pub snap_line_y: Option<f64>,
}

#[derive(Clone, Copy)]
enum AlterObject {
    None,
    Delete,
    MoveUp,
    MoveDown,
    Duplicate,
}

impl HomeFlow {
    pub fn edit_mode_settings(&mut self, ui: &mut Ui) {
        if self.edit_mode.enabled {
            ui.checkbox(&mut self.edit_mode.resize_enabled, "Resizing");
            if ui.button("Materials Editor").clicked() {
                self.edit_mode.material_editor_open = !self.edit_mode.material_editor_open;
            }
            if ui.button("Preview Edits").clicked() {
                self.edit_mode.preview_edits = !self.edit_mode.preview_edits;
            }
            if ui.button("Save Edits").clicked() {
                let toasts_store = self.toasts.clone();
                toasts_store
                    .lock()
                    .info("Saving Layout")
                    .duration(Some(Duration::from_secs(2)));
                save_layout(
                    &self.host,
                    &self.stored.auth_token,
                    &self.layout,
                    move |result| match result {
                        Ok(()) => {
                            toasts_store
                                .lock()
                                .success("Layout Saved")
                                .duration(Some(Duration::from_secs(2)));
                        }
                        Err(_) => {
                            toasts_store
                                .lock()
                                .error("Failed to save layout")
                                .duration(Some(Duration::from_secs(2)));
                        }
                    },
                );
                self.layout_server = self.layout.clone();
                self.edit_mode.enabled = false;
            }
            if ui.button("Discard Edits").clicked() {
                self.layout = self.layout_server.clone();
                self.edit_mode.enabled = false;
            }

            // Show preview edits
            Window::new("Preview Edits")
                .default_size([500.0, 500.0])
                .pivot(Align2::CENTER_CENTER)
                .resizable(true)
                .max_height(500.0)
                .open(&mut self.edit_mode.preview_edits)
                .show(ui.ctx(), |ui| {
                    let initial_layout = ron::ser::to_string_pretty(
                        &self.layout_server,
                        ron::ser::PrettyConfig::default(),
                    )
                    .unwrap();
                    let current_layout =
                        ron::ser::to_string_pretty(&self.layout, ron::ser::PrettyConfig::default())
                            .unwrap();
                    let diffs = diff::lines(&initial_layout, &current_layout);
                    egui::ScrollArea::vertical()
                        .auto_shrink(true)
                        .show(ui, |ui| {
                            for diff in diffs {
                                match diff {
                                    diff::Result::Left(l) => {
                                        ui.colored_label(Color32::RED, l);
                                    }
                                    diff::Result::Right(r) => {
                                        ui.colored_label(Color32::GREEN, r);
                                    }
                                    diff::Result::Both(l, _) => {
                                        ui.label(l);
                                    }
                                }
                            }
                        });
                });
        }
        // If not in edit mode, show button to enter edit mode
        else if ui.button("Edit Mode").clicked() {
            self.edit_mode.enabled = true;
        }
        if ui.button("Refresh").clicked() {
            self.edit_mode.enabled = false;
            self.layout = Home::empty();
            self.layout_server = Home::empty();
        }
    }

    pub fn run_edit_mode(&mut self, response: &egui::Response, ui: &Ui) -> EditResponse {
        if !self.edit_mode.enabled {
            return EditResponse {
                used_dragged: false,
                hovered_id: None,
                snap_line_x: None,
                snap_line_y: None,
            };
        }

        let snap_enabled = !ui.input(|i| i.modifiers.shift); // Shift to disable snap
        let hover_details = self.hover_select(response, ui);

        // Cursor for hovered
        let can_drag = hover_details.as_ref().is_some_and(|h| h.can_drag);
        if can_drag || self.edit_mode.drag_data.is_some() {
            if let Some(hover_details) = &hover_details {
                let rotation_normalized = hover_details.rotation.rem_euclid(360);
                let flip_cursor = (rotation_normalized > 45 && rotation_normalized < 135)
                    || (rotation_normalized > 225 && rotation_normalized < 315);

                match hover_details.manipulation_type {
                    ManipulationType::Move => {
                        ui.ctx()
                            .set_cursor_icon(if self.edit_mode.drag_data.is_some() {
                                CursorIcon::Grabbing
                            } else {
                                CursorIcon::PointingHand
                            });
                    }
                    ManipulationType::ResizeLeft | ManipulationType::ResizeRight => {
                        ui.ctx().set_cursor_icon(if flip_cursor {
                            CursorIcon::ResizeVertical
                        } else {
                            CursorIcon::ResizeHorizontal
                        });
                    }
                    ManipulationType::ResizeTop | ManipulationType::ResizeBottom => {
                        ui.ctx().set_cursor_icon(if flip_cursor {
                            CursorIcon::ResizeHorizontal
                        } else {
                            CursorIcon::ResizeVertical
                        });
                    }
                }
            }
        }

        let mouse_down = ui
            .ctx()
            .input(|i| i.pointer.button_down(egui::PointerButton::Primary));
        if let Some(hover_details) = &hover_details {
            // Start drag
            if mouse_down && self.edit_mode.drag_data.is_none() && can_drag {
                self.edit_mode.drag_data = Some(DragData {
                    id: hover_details.id,
                    object_type: hover_details.object_type,
                    manipulation_type: hover_details.manipulation_type,
                    mouse_start_pos: self.mouse_pos_world,
                    start_pos: hover_details.pos,
                    start_size: hover_details.size,
                    start_rotation: hover_details.rotation,
                });
            }
        }

        let mut used_dragged = false;
        let mut snap_line_x = None;
        let mut snap_line_y = None;

        if response.dragged_by(PointerButton::Primary) {
            if let Some(drag_data) = &self.edit_mode.drag_data {
                used_dragged = true;

                let (new_pos, new_rotation, snap_x, snap_y) =
                    self.handle_drag(drag_data, snap_enabled);
                Window::new("Dragging Info")
                    .fixed_pos(vec2_to_egui_pos(
                        self.world_to_screen(self.mouse_pos_world) + vec2(0.0, -60.0),
                    ))
                    .fixed_size([200.0, 0.0])
                    .pivot(Align2::CENTER_CENTER)
                    .title_bar(false)
                    .resizable(false)
                    .interactable(false)
                    .show(ui.ctx(), |ui| {
                        ui.label(format!("Pos: ({:.3}m, {:.3}m)", new_pos.x, new_pos.y));
                        if drag_data.start_size.length() > 0.0 {
                            ui.label(format!(
                                "Size: ({:.3}m, {:.3}m)",
                                drag_data.start_size.x, drag_data.start_size.y
                            ));
                        }
                    });

                let delta = new_pos - drag_data.start_pos;
                for room in &mut self.layout.rooms {
                    if drag_data.id == room.id {
                        apply_standard_transform(
                            &mut room.pos,
                            &mut room.size,
                            drag_data,
                            delta,
                            new_pos,
                            Vec2::ZERO,
                        );
                    } else {
                        for operation in &mut room.operations {
                            if operation.id == drag_data.id {
                                apply_standard_transform(
                                    &mut operation.pos,
                                    &mut operation.size,
                                    drag_data,
                                    delta,
                                    new_pos,
                                    room.pos,
                                );
                            }
                        }
                        for zone in &mut room.zones {
                            if zone.id == drag_data.id {
                                apply_standard_transform(
                                    &mut zone.pos,
                                    &mut zone.size,
                                    drag_data,
                                    delta,
                                    new_pos,
                                    room.pos,
                                );
                            }
                        }
                        for opening in &mut room.openings {
                            if opening.id == drag_data.id {
                                opening.pos = new_pos - room.pos;
                                opening.rotation = new_rotation;
                            }
                        }
                        for light in &mut room.lights {
                            if light.id == drag_data.id {
                                light.pos = new_pos - room.pos;
                            }
                        }
                        for furniture in &mut room.furniture {
                            if furniture.id == drag_data.id {
                                apply_standard_transform(
                                    &mut furniture.pos,
                                    &mut furniture.size,
                                    drag_data,
                                    delta,
                                    new_pos,
                                    room.pos,
                                );
                            }
                        }
                    }
                }
                snap_line_x = snap_x;
                snap_line_y = snap_y;
            }
        }
        if !mouse_down {
            self.edit_mode.drag_data = None;
        }

        if let Some(selected_id) = self.edit_mode.selected_id {
            let mut window_open: bool = true;
            Window::new(format!("Edit {selected_id}"))
                .default_pos(vec2_to_egui_pos(vec2(self.canvas_center.x, 20.0)))
                .fixed_size([0.0, 0.0])
                .pivot(Align2::CENTER_TOP)
                .movable(true)
                .resizable(false)
                .collapsible(true)
                .open(&mut window_open)
                .show(ui.ctx(), |ui| self.edit_widgets(ui, selected_id));
            if !window_open {
                self.edit_mode.selected_id = None;
                self.edit_mode.selected_type = None;
            }
        }

        Window::new("Edit Materials".to_string())
            .fixed_pos(vec2_to_egui_pos(vec2(
                self.canvas_center.x,
                self.canvas_center.y,
            )))
            .fixed_size([300.0, 0.0])
            .pivot(Align2::CENTER_CENTER)
            .open(&mut self.edit_mode.material_editor_open)
            .show(ui.ctx(), |ui| {
                ui.vertical_centered(|ui| {
                    let num_objects = self.layout.materials.len();
                    let mut alterations = vec![AlterObject::None; num_objects];
                    for (index, material) in self.layout.materials.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label("Material");
                            TextEdit::singleline(&mut material.name)
                                .min_size(egui::vec2(100.0, 0.0))
                                .desired_width(0.0)
                                .show(ui);
                            combo_box_for_enum(
                                ui,
                                format!("Material {index}"),
                                &mut material.material,
                                "",
                            );
                            ui.color_edit_button_srgba_unmultiplied(material.tint.mut_array());

                            edit_option(
                                ui,
                                "Tiles",
                                &mut material.tiles,
                                TileOptions::default,
                                |ui, tiles| {
                                    labelled_widget(ui, "Spacing", |ui| {
                                        ui.add(
                                            DragValue::new(&mut tiles.spacing)
                                                .speed(0.1)
                                                .range(0.01..=5.0)
                                                .suffix("m"),
                                        );
                                    });
                                    labelled_widget(ui, "Width", |ui| {
                                        ui.add(
                                            DragValue::new(&mut tiles.grout_width)
                                                .speed(0.1)
                                                .range(0.01..=5.0)
                                                .suffix("m"),
                                        );
                                    });
                                    labelled_widget(ui, "", |ui| {
                                        ui.color_edit_button_srgba_unmultiplied(
                                            tiles.grout_color.mut_array(),
                                        );
                                    });
                                },
                            );

                            if ui.button("Delete").clicked() {
                                alterations[index] = AlterObject::Delete;
                            }
                            if index > 0 && ui.button("^").clicked() {
                                alterations[index] = AlterObject::MoveUp;
                            }
                            if index < num_objects - 1 && ui.button("v").clicked() {
                                alterations[index] = AlterObject::MoveDown;
                            }
                        });
                    }
                    for (index, alteration) in alterations.into_iter().enumerate().rev() {
                        match alteration {
                            AlterObject::Delete => {
                                self.layout.materials.remove(index);
                            }
                            AlterObject::MoveUp => {
                                self.layout.materials.swap(index, index - 1);
                            }
                            AlterObject::MoveDown => {
                                self.layout.materials.swap(index, index + 1);
                            }
                            _ => {}
                        }
                    }

                    // Add button
                    if ui.button("Add Material").clicked() {
                        self.layout.materials.push(GlobalMaterial {
                            name: "New Material".to_string(),
                            material: Material::Empty,
                            tint: Color::WHITE,
                            tiles: None,
                        });
                    }
                });
            });

        EditResponse {
            used_dragged,
            hovered_id: hover_details.map(|h| h.id),
            snap_line_x,
            snap_line_y,
        }
    }

    fn edit_widgets(&mut self, ui: &mut Ui, selected_id: Uuid) {
        if self.edit_mode.selected_type.unwrap() == ObjectType::Room {
            let room_and_index = self.layout.rooms.iter_mut().enumerate().find_map(|obj| {
                if obj.1.id == selected_id {
                    Some(obj)
                } else {
                    None
                }
            });
            if let Some((index, room)) = room_and_index {
                let alter_type = room_edit_widgets(ui, &self.layout.materials, room);
                match alter_type {
                    AlterObject::Delete => {
                        self.layout.rooms.retain(|r| r.id != selected_id);
                        self.edit_mode.selected_id = None;
                        self.edit_mode.selected_type = None;
                    }
                    AlterObject::MoveUp => {
                        if index < self.layout.rooms.len() - 1 {
                            self.layout.rooms.swap(index, index + 1);
                        }
                    }
                    AlterObject::MoveDown => {
                        if index > 0 {
                            self.layout.rooms.swap(index, index - 1);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn room_edit_widgets(
    ui: &mut egui::Ui,
    materials: &[GlobalMaterial],
    room: &mut Room,
) -> AlterObject {
    let mut alter_type = AlterObject::None;
    ui.horizontal(|ui| {
        ui.label("Room");
        TextEdit::singleline(&mut room.name)
            .min_size(egui::vec2(200.0, 0.0))
            .show(ui);
        if ui.add(Button::new("Delete")).clicked() {
            alter_type = AlterObject::Delete;
        }
        if ui.add(Button::new("^")).clicked() {
            alter_type = AlterObject::MoveUp;
        }
        if ui.add(Button::new("v")).clicked() {
            alter_type = AlterObject::MoveDown;
        }
    });
    ui.separator();

    egui::Grid::new("Room Edit Grid")
        .num_columns(4)
        .spacing([20.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            edit_vec2(ui, "Pos", &mut room.pos, 0.1);
            edit_vec2(ui, "Size", &mut room.size, 0.1);
            ui.end_row();

            // Wall selection
            for index in 0..4 {
                let (mut is_wall, wall_side, flag) = match index {
                    0 => (room.walls.contains(Walls::LEFT), "Left", Walls::LEFT),
                    1 => (room.walls.contains(Walls::TOP), "Top", Walls::TOP),
                    2 => (room.walls.contains(Walls::RIGHT), "Right", Walls::RIGHT),
                    _ => (room.walls.contains(Walls::BOTTOM), "Bottom", Walls::BOTTOM),
                };

                ui.horizontal(|ui| {
                    labelled_widget(ui, &format!("{wall_side} Wall"), |ui| {
                        if ui.checkbox(&mut is_wall, "").changed() {
                            if is_wall {
                                room.walls.insert(flag);
                            } else {
                                room.walls.remove(flag);
                            }
                        }
                    });
                });
            }
            ui.end_row();

            combo_box_for_materials(ui, &room.id.to_string(), materials, &mut room.material);

            edit_option(
                ui,
                "Outline",
                &mut room.outline,
                Outline::default,
                |ui, outline| {
                    labelled_widget(ui, "Thickness", |ui| {
                        ui.add(
                            DragValue::new(&mut outline.thickness)
                                .speed(0.1)
                                .range(0.01..=5.0)
                                .suffix("m"),
                        );
                    });
                    labelled_widget(ui, "Color", |ui| {
                        ui.color_edit_button_srgba_unmultiplied(outline.color.mut_array());
                    });
                },
            );
        });

    ui.separator();

    let persist_id = ui.make_persistent_id("operations_collapsing_header");
    CollapsingState::load_with_default_open(ui.ctx(), persist_id, false)
        .show_header(ui, |ui| {
            ui.horizontal(|ui| {
                labelled_widget(ui, "Operations", |ui| {
                    if ui.add(Button::new("Add")).clicked() {
                        room.operations.push(Operation::default());
                    }
                });
            });
        })
        .body(|ui| {
            let num_objects = room.operations.len();
            let mut alterations = vec![AlterObject::None; num_objects];
            for (index, operation) in room.operations.iter_mut().enumerate() {
                let color = match operation.action {
                    Action::Add => Color32::from_rgb(50, 200, 50),
                    Action::Subtract => Color32::from_rgb(200, 50, 50),
                    Action::AddWall => Color32::from_rgb(50, 100, 50),
                    Action::SubtractWall => Color32::from_rgb(160, 90, 50),
                }
                .gamma_multiply(0.15);
                egui::Frame::fill(egui::Frame::central_panel(ui.style()), color).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        combo_box_for_enum(
                            ui,
                            format!("Operation {index}"),
                            &mut operation.action,
                            "",
                        );
                        combo_box_for_enum(ui, format!("Shape {index}"), &mut operation.shape, "");

                        if ui.button("Delete").clicked() {
                            alterations[index] = AlterObject::Delete;
                        }
                        if index > 0 && ui.button("^").clicked() {
                            alterations[index] = AlterObject::MoveUp;
                        }
                        if index < num_objects - 1 && ui.button("v").clicked() {
                            alterations[index] = AlterObject::MoveDown;
                        }
                    });

                    ui.horizontal(|ui| {
                        edit_vec2(ui, "Pos", &mut operation.pos, 0.1);
                        edit_vec2(ui, "Size", &mut operation.size, 0.1);
                        edit_rotation(ui, &mut operation.rotation);
                    });

                    if operation.action == Action::Add {
                        ui.horizontal(|ui| {
                            edit_option(
                                ui,
                                "Custom Material",
                                &mut operation.material,
                                || room.material.clone(),
                                |ui, content| {
                                    combo_box_for_materials(
                                        ui,
                                        &operation.id.to_string(),
                                        materials,
                                        content,
                                    );
                                },
                            );
                        });
                    }
                });
            }
            for (index, alteration) in alterations.into_iter().enumerate().rev() {
                match alteration {
                    AlterObject::Delete => {
                        room.operations.remove(index);
                    }
                    AlterObject::MoveUp => {
                        room.operations.swap(index, index - 1);
                    }
                    AlterObject::MoveDown => {
                        room.operations.swap(index, index + 1);
                    }
                    _ => {}
                }
            }
        });

    let persist_id = ui.make_persistent_id("zones_collapsing_header");
    CollapsingState::load_with_default_open(ui.ctx(), persist_id, false)
        .show_header(ui, |ui| {
            ui.horizontal(|ui| {
                labelled_widget(ui, "Zones", |ui| {
                    if ui.add(Button::new("Add")).clicked() {
                        room.zones.push(Zone::default());
                    }
                });
            });
        })
        .body(|ui| {
            let num_objects = room.zones.len();
            let mut alterations = vec![AlterObject::None; num_objects];
            for (index, zone) in room.zones.iter_mut().enumerate() {
                egui::Frame::fill(
                    egui::Frame::central_panel(ui.style()),
                    Color32::from_rgb(160, 90, 50).gamma_multiply(0.15),
                )
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        TextEdit::singleline(&mut zone.name)
                            .min_size(egui::vec2(100.0, 0.0))
                            .show(ui);
                        combo_box_for_enum(ui, format!("Shape {index}"), &mut zone.shape, "");

                        if ui.button("Delete").clicked() {
                            alterations[index] = AlterObject::Delete;
                        }
                        if index > 0 && ui.button("^").clicked() {
                            alterations[index] = AlterObject::MoveUp;
                        }
                        if index < num_objects - 1 && ui.button("v").clicked() {
                            alterations[index] = AlterObject::MoveDown;
                        }
                    });

                    ui.horizontal(|ui| {
                        edit_vec2(ui, "Pos", &mut zone.pos, 0.1);
                        edit_vec2(ui, "Size", &mut zone.size, 0.1);
                        edit_rotation(ui, &mut zone.rotation);
                    });
                });
            }
            for (index, alteration) in alterations.into_iter().enumerate().rev() {
                match alteration {
                    AlterObject::Delete => {
                        room.zones.remove(index);
                    }
                    AlterObject::MoveUp => {
                        room.zones.swap(index, index - 1);
                    }
                    AlterObject::MoveDown => {
                        room.zones.swap(index, index + 1);
                    }
                    _ => {}
                }
            }
        });

    CollapsingState::load_with_default_open(
        ui.ctx(),
        ui.make_persistent_id("openings_collapsing_header"),
        false,
    )
    .show_header(ui, |ui| {
        ui.horizontal(|ui| {
            labelled_widget(ui, "Openings", |ui| {
                if ui.add(Button::new("Add")).clicked() {
                    room.openings.push(Opening::default());
                }
            });
        });
    })
    .body(|ui| {
        let num_objects = room.openings.len();
        let mut alterations = vec![AlterObject::None; num_objects];
        for (index, opening) in room.openings.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                combo_box_for_enum(
                    ui,
                    format!("Opening {}", opening.id),
                    &mut opening.opening_type,
                    "",
                );
                edit_vec2(ui, "Pos", &mut opening.pos, 0.1);
                edit_rotation(ui, &mut opening.rotation);
                labelled_widget(ui, "Width", |ui| {
                    ui.add(
                        DragValue::new(&mut opening.width)
                            .speed(0.1)
                            .range(0.1..=5.0)
                            .suffix("m"),
                    );
                });
                if opening.opening_type == OpeningType::Door {
                    labelled_widget(ui, "Flipped", |ui| {
                        ui.checkbox(&mut opening.flipped, "");
                    });
                }
                if ui.button("Delete").clicked() {
                    alterations[index] = AlterObject::Delete;
                }
                if index > 0 && ui.button("^").clicked() {
                    alterations[index] = AlterObject::MoveUp;
                }
                if num_objects > 0 && index < num_objects - 1 && ui.button("v").clicked() {
                    alterations[index] = AlterObject::MoveDown;
                }
            });
        }
        for (index, alteration) in alterations.into_iter().enumerate().rev() {
            match alteration {
                AlterObject::Delete => {
                    room.openings.remove(index);
                }
                AlterObject::MoveUp => {
                    room.openings.swap(index, index - 1);
                }
                AlterObject::MoveDown => {
                    room.openings.swap(index, index + 1);
                }
                _ => {}
            }
        }
    });

    CollapsingState::load_with_default_open(
        ui.ctx(),
        ui.make_persistent_id("lights_collapsing_header"),
        false,
    )
    .show_header(ui, |ui| {
        ui.horizontal(|ui| {
            labelled_widget(ui, "Lights", |ui| {
                if ui.add(Button::new("Add")).clicked() {
                    room.lights.push(Light::default());
                }
            });
        });
    })
    .body(|ui| {
        let num_objects = room.lights.len();
        let mut alterations = vec![AlterObject::None; num_objects];
        for (index, light) in room.lights.iter_mut().enumerate() {
            egui::Frame::fill(
                egui::Frame::central_panel(ui.style()),
                Color32::from_rgb(100, 80, 20),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    TextEdit::singleline(&mut light.name)
                        .min_size(egui::vec2(100.0, 0.0))
                        .show(ui);
                    edit_vec2(ui, "Pos", &mut light.pos, 0.1);
                    if ui.button("Delete").clicked() {
                        alterations[index] = AlterObject::Delete;
                    }
                    if index > 0 && ui.button("^").clicked() {
                        alterations[index] = AlterObject::MoveUp;
                    }
                    if num_objects > 0 && index < num_objects - 1 && ui.button("v").clicked() {
                        alterations[index] = AlterObject::MoveDown;
                    }
                });
                ui.horizontal(|ui| {
                    labelled_widget(ui, "Intensity", |ui| {
                        ui.add(
                            DragValue::new(&mut light.intensity)
                                .speed(0.1)
                                .range(0.1..=10.0)
                                .suffix("cd"),
                        );
                    });
                    labelled_widget(ui, "Radius", |ui| {
                        ui.add(
                            DragValue::new(&mut light.radius)
                                .speed(0.01)
                                .range(0.01..=0.5)
                                .suffix("m"),
                        );
                    });
                    edit_option(
                        ui,
                        "Multi",
                        &mut light.multi,
                        MultiLight::default,
                        |ui, content| {
                            edit_vec2(ui, "Room Padding", &mut content.room_padding, 0.1);
                            labelled_widget(ui, "Rows", |ui| {
                                ui.add(DragValue::new(&mut content.rows).range(1..=20));
                            });
                            labelled_widget(ui, "Cols", |ui| {
                                ui.add(DragValue::new(&mut content.cols).range(1..=20));
                            });
                        },
                    );
                });
            });
        }
        for (index, alteration) in alterations.into_iter().enumerate().rev() {
            match alteration {
                AlterObject::Delete => {
                    room.lights.remove(index);
                }
                AlterObject::MoveUp => {
                    room.lights.swap(index, index - 1);
                }
                AlterObject::MoveDown => {
                    room.lights.swap(index, index + 1);
                }
                _ => {}
            }
        }
    });

    CollapsingState::load_with_default_open(
        ui.ctx(),
        ui.make_persistent_id("furniture_collapsing_header"),
        false,
    )
    .show_header(ui, |ui| {
        ui.horizontal(|ui| {
            labelled_widget(ui, "Furniture", |ui| {
                if ui.add(Button::new("Add")).clicked() {
                    room.furniture.push(Furniture::default());
                }
            });
        });
    })
    .body(|ui| {
        let num_objects = room.furniture.len();
        let mut alterations = vec![AlterObject::None; num_objects];
        for (index, furniture) in room.furniture.iter_mut().enumerate() {
            egui::Frame::fill(
                egui::Frame::central_panel(ui.style()),
                Color32::from_rgb(20, 60, 20),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    TextEdit::singleline(&mut furniture.name)
                        .min_size(egui::vec2(100.0, 0.0))
                        .show(ui);

                    egui::ComboBox::from_id_salt(format!("Furniture {}", furniture.id))
                        .selected_text(furniture.furniture_type.to_string())
                        .show_ui(ui, |ui| {
                            for variant in <FurnitureType as strum::IntoEnumIterator>::iter() {
                                if matches!(variant, FurnitureType::AnimatedPiece(_)) {
                                    continue;
                                }
                                ui.selectable_value(
                                    &mut furniture.furniture_type,
                                    variant,
                                    variant.to_string(),
                                );
                            }
                        });
                    match &mut furniture.furniture_type {
                        FurnitureType::Chair(ref mut chair_type) => {
                            combo_box_for_enum(ui, format!("{}-c", furniture.id), chair_type, "");
                            if let ChairType::Sofa(ref mut color) = chair_type {
                                ui.color_edit_button_srgba_unmultiplied(color.mut_array());
                            }
                        }
                        FurnitureType::Table(ref mut table_type) => {
                            combo_box_for_enum(ui, format!("{}-t", furniture.id), table_type, "");
                            if let TableType::DiningCustomChairs(
                                ref mut top_chairs,
                                ref mut bottom_chairs,
                                ref mut left_chairs,
                                ref mut right_chairs,
                            ) = table_type
                            {
                                ui.add(DragValue::new(top_chairs).speed(1).range(0..=20));
                                ui.add(DragValue::new(bottom_chairs).speed(1).range(0..=20));
                                ui.add(DragValue::new(left_chairs).speed(1).range(0..=20));
                                ui.add(DragValue::new(right_chairs).speed(1).range(0..=20));
                            }
                        }
                        FurnitureType::Bed(ref mut color) | FurnitureType::Rug(ref mut color) => {
                            ui.color_edit_button_srgba_unmultiplied(color.mut_array());
                        }
                        FurnitureType::Kitchen(ref mut kitchen_type) => {
                            combo_box_for_enum(ui, format!("{}-k", furniture.id), kitchen_type, "");
                        }
                        FurnitureType::Bathroom(ref mut bathroom_type) => {
                            combo_box_for_enum(
                                ui,
                                format!("{}-b", furniture.id),
                                bathroom_type,
                                "",
                            );
                        }
                        FurnitureType::Storage(ref mut storage_type) => {
                            combo_box_for_enum(ui, format!("{}-s", furniture.id), storage_type, "");
                        }
                        FurnitureType::Electronic(ref mut electronic_type) => {
                            combo_box_for_enum(
                                ui,
                                format!("{}-e", furniture.id),
                                electronic_type,
                                "",
                            );
                        }
                        _ => {}
                    }
                    combo_box_for_enum(
                        ui,
                        format!("{} Render Order", furniture.id),
                        &mut furniture.render_order,
                        "Render Order",
                    );
                    if furniture.has_material() {
                        combo_box_for_materials(
                            ui,
                            &furniture.id.to_string(),
                            materials,
                            &mut furniture.material,
                        );
                    }
                    if furniture.has_children_material() {
                        combo_box_for_materials(
                            ui,
                            &format!("{} Children", furniture.id),
                            materials,
                            &mut furniture.material_children,
                        );
                    }

                    if index > 0 && ui.button("^").clicked() {
                        alterations[index] = AlterObject::MoveUp;
                    }
                    if num_objects > 0 && index < num_objects - 1 && ui.button("v").clicked() {
                        alterations[index] = AlterObject::MoveDown;
                    }
                    if ui.button("Duplicate").clicked() {
                        alterations[index] = AlterObject::Duplicate;
                    }
                    if ui.button("Delete").clicked() {
                        alterations[index] = AlterObject::Delete;
                    }
                });

                ui.horizontal(|ui| {
                    edit_vec2(ui, "Pos", &mut furniture.pos, 0.1);
                    edit_vec2(ui, "Size", &mut furniture.size, 0.1);
                    edit_rotation(ui, &mut furniture.rotation);
                    ui.label("Power Entity");
                    TextEdit::singleline(&mut furniture.power_draw_entity)
                        .min_size(egui::vec2(200.0, 0.0))
                        .show(ui);
                });
            });
        }
        for (index, alteration) in alterations.into_iter().enumerate().rev() {
            match alteration {
                AlterObject::Delete => {
                    room.furniture.remove(index);
                }
                AlterObject::MoveUp => {
                    room.furniture.swap(index, index - 1);
                }
                AlterObject::MoveDown => {
                    room.furniture.swap(index, index + 1);
                }
                AlterObject::Duplicate => {
                    let mut new_furniture = room.furniture[index].clone();
                    new_furniture.id = Uuid::new_v4();
                    room.furniture.insert(index + 1, new_furniture);
                }
                AlterObject::None => {}
            }
        }
    });

    edit_vec2(ui, "Sensors Offset", &mut room.sensors_offset, 0.1);
    CollapsingState::load_with_default_open(
        ui.ctx(),
        ui.make_persistent_id("sensors_collapsing_header"),
        false,
    )
    .show_header(ui, |ui| {
        ui.horizontal(|ui| {
            labelled_widget(ui, "Sensors", |ui| {
                if ui.add(Button::new("Add")).clicked() {
                    room.sensors.push(Sensor::default());
                }
            });
        });
    })
    .body(|ui| {
        let num_objects = room.sensors.len();
        let mut alterations = vec![AlterObject::None; num_objects];
        for (index, sensor) in room.sensors.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                TextEdit::singleline(&mut sensor.entity_id)
                    .min_size(egui::vec2(100.0, 0.0))
                    .show(ui);
                TextEdit::singleline(&mut sensor.display_name)
                    .min_size(egui::vec2(50.0, 0.0))
                    .show(ui);
                TextEdit::singleline(&mut sensor.unit)
                    .min_size(egui::vec2(50.0, 0.0))
                    .show(ui);
                if ui.button("Delete").clicked() {
                    alterations[index] = AlterObject::Delete;
                }
                if index > 0 && ui.button("^").clicked() {
                    alterations[index] = AlterObject::MoveUp;
                }
                if num_objects > 0 && index < num_objects - 1 && ui.button("v").clicked() {
                    alterations[index] = AlterObject::MoveDown;
                }
            });
        }
        for (index, alteration) in alterations.into_iter().enumerate().rev() {
            match alteration {
                AlterObject::Delete => {
                    room.sensors.remove(index);
                }
                AlterObject::MoveUp => {
                    room.sensors.swap(index, index - 1);
                }
                AlterObject::MoveDown => {
                    room.sensors.swap(index, index + 1);
                }
                _ => {}
            }
        }
    });

    alter_type
}
