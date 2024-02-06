use super::{
    edit_mode_utils::{
        apply_standard_transform, combo_box_for_enum, combo_box_for_materials, edit_option,
        edit_rotation, edit_vec2, labelled_widget,
    },
    HomeFlow,
};
use crate::common::{
    layout::{Action, GlobalMaterial, Home, Opening, Operation, Outline, Room},
    utils::vec2_to_egui_pos,
};
use egui::{
    collapsing_header::CollapsingState, Align2, Button, Color32, Context, CursorIcon, DragValue,
    PointerButton, TextEdit, Ui, Window,
};
use glam::{dvec2 as vec2, DVec2 as Vec2};
use std::time::Duration;
use uuid::Uuid;

#[derive(Default)]
pub struct EditDetails {
    pub enabled: bool,
    pub drag_data: Option<DragData>,
    pub selected_id: Option<Uuid>,
    pub selected_type: Option<ObjectType>,
    pub preview_edits: bool,
}

pub struct DragData {
    pub id: Uuid,
    pub object_type: ObjectType,
    pub manipulation_type: ManipulationType,
    pub mouse_start_pos: Vec2,
    pub object_start_pos: Vec2,
    pub object_size: Vec2,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ObjectType {
    Room,
    Operation,
    Opening,
    Furniture,
}

#[derive(Clone, Copy)]
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
            Self::ResizeLeft | Self::ResizeTop => -1.0,
            Self::ResizeRight | Self::ResizeBottom => 1.0,
        }
    }
}

pub struct EditResponse {
    pub used_dragged: bool,
    pub hovered_id: Option<Uuid>,
    pub snap_line_x: Option<f64>,
    pub snap_line_y: Option<f64>,
}

impl HomeFlow {
    pub fn edit_mode_settings(&mut self, ctx: &Context, ui: &mut Ui) {
        // If in edit mode, show button to view save and discard changes
        if self.edit_mode.enabled {
            if ui.button("Preview Edits").clicked() {
                self.edit_mode.preview_edits = !self.edit_mode.preview_edits;
            }
            if ui.button("Save Edits").clicked() {
                let toasts_store = self.toasts.clone();
                toasts_store
                    .lock()
                    .info("Saving Layout")
                    .set_duration(Some(Duration::from_secs(2)));
                ehttp::fetch(
                    ehttp::Request::post(
                        format!("http://{}/save_layout", self.host),
                        serde_json::to_string(&self.layout)
                            .unwrap()
                            .as_bytes()
                            .to_vec(),
                    ),
                    move |result| match result {
                        Ok(_) => {
                            toasts_store
                                .lock()
                                .success("Layout Saved")
                                .set_duration(Some(Duration::from_secs(2)));
                        }
                        Err(_) => {
                            toasts_store
                                .lock()
                                .error("Failed to save layout")
                                .set_duration(Some(Duration::from_secs(2)));
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
                .show(ctx, |ui| {
                    let current_layout = self.layout.to_string();
                    let initial_layout = self.layout_server.to_string();
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
            self.layout = Home::default();
            self.layout_server = Home::default();
        }
    }

    pub fn run_edit_mode(
        &mut self,
        response: &egui::Response,
        ctx: &Context,
        ui: &Ui,
    ) -> EditResponse {
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
        let can_drag = self.edit_mode.selected_id.is_some()
            || hover_details
                .as_ref()
                .is_some_and(|h| h.object_type == ObjectType::Furniture);

        if can_drag || self.edit_mode.drag_data.is_some() {
            if let Some(hover_details) = &hover_details {
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
                        ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
                    }
                    ManipulationType::ResizeTop | ManipulationType::ResizeBottom => {
                        ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
                    }
                }
            }
        }

        let mouse_down = ctx.input(|i| i.pointer.button_down(egui::PointerButton::Primary));
        if let Some(hover_details) = &hover_details {
            // Start drag
            if mouse_down && self.edit_mode.drag_data.is_none() && can_drag {
                self.edit_mode.drag_data = Some(DragData {
                    id: hover_details.id,
                    object_type: hover_details.object_type,
                    manipulation_type: hover_details.manipulation_type,
                    mouse_start_pos: self.mouse_pos_world,
                    object_start_pos: hover_details.pos,
                    object_size: hover_details.size,
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
                        self.world_to_pixels(self.mouse_pos_world) + vec2(0.0, -60.0),
                    ))
                    .fixed_size([200.0, 0.0])
                    .pivot(Align2::CENTER_CENTER)
                    .title_bar(false)
                    .resizable(false)
                    .interactable(false)
                    .show(ctx, |ui| {
                        ui.label(format!("Pos: ({:.1}, {:.1})", new_pos.x, new_pos.y));
                        if drag_data.object_size.length() > 0.0 {
                            ui.label(format!(
                                "Size: ({:.1}, {:.1})",
                                drag_data.object_size.x, drag_data.object_size.y
                            ));
                        }
                    });

                let delta = new_pos - drag_data.object_start_pos;
                for room in &mut self.layout.rooms {
                    if drag_data.id == room.id {
                        apply_standard_transform(
                            &mut room.pos,
                            &mut room.size,
                            drag_data,
                            delta,
                            new_pos,
                        );
                    } else {
                        for operation in &mut room.operations {
                            if operation.id == drag_data.id {
                                apply_standard_transform(
                                    &mut operation.pos,
                                    &mut operation.size,
                                    drag_data,
                                    delta,
                                    new_pos - room.pos,
                                );
                            }
                        }
                        for opening in &mut room.openings {
                            if opening.id == drag_data.id {
                                opening.pos = new_pos - room.pos;
                                opening.rotation = new_rotation;
                            }
                        }
                    }
                }
                for furniture in &mut self.layout.furniture {
                    if furniture.id == drag_data.id {
                        apply_standard_transform(
                            &mut furniture.pos,
                            &mut furniture.size,
                            drag_data,
                            delta,
                            new_pos,
                        );
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
                .show(ctx, |ui| self.edit_widgets(ui, selected_id));
            if !window_open {
                self.edit_mode.selected_id = None;
                self.edit_mode.selected_type = None;
            }
        }

        EditResponse {
            used_dragged,
            hovered_id: hover_details.map(|h| h.id),
            snap_line_x,
            snap_line_y,
        }
    }

    fn edit_widgets(&mut self, ui: &mut Ui, selected_id: Uuid) {
        match self.edit_mode.selected_type.unwrap() {
            ObjectType::Room => {
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
                        AlterObject::None => {}
                    }
                }
            }
            ObjectType::Furniture => {
                let furniture_and_index =
                    self.layout.rooms.iter_mut().enumerate().find_map(|obj| {
                        if obj.1.id == selected_id {
                            Some(obj)
                        } else {
                            None
                        }
                    });
                if let Some((index, furniture)) = furniture_and_index {
                    // let alter_type = furniture_edit_widgets(ui, furniture);
                }
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy)]
enum AlterObject {
    None,
    Delete,
    MoveUp,
    MoveDown,
}

fn room_edit_widgets(
    ui: &mut egui::Ui,
    materials: &Vec<GlobalMaterial>,
    room: &mut Room,
) -> AlterObject {
    let mut alter_type = AlterObject::None;
    ui.horizontal(|ui| {
        ui.label("Room ");
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
            edit_vec2(ui, "Pos", &mut room.pos, 0.1, 2);
            edit_vec2(ui, "Size", &mut room.size, 0.1, 2);
            ui.end_row();

            // Wall selection
            for index in 0..4 {
                let (is_wall, wall_side) = match index {
                    0 => (&mut room.walls.left, "Left"),
                    1 => (&mut room.walls.top, "Top"),
                    2 => (&mut room.walls.right, "Right"),
                    _ => (&mut room.walls.bottom, "Bottom"),
                };
                ui.horizontal(|ui| {
                    labelled_widget(ui, &format!("{wall_side} Wall"), |ui| {
                        ui.checkbox(is_wall, "");
                    });
                });
            }
            ui.end_row();

            combo_box_for_materials(ui, room.id, materials, &mut room.material);

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
                                .fixed_decimals(2)
                                .clamp_range(0.01..=5.0)
                                .suffix("m"),
                        );
                    });
                    labelled_widget(ui, "Color", |ui| {
                        ui.color_edit_button_srgba(&mut outline.color);
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
                        edit_vec2(ui, "Pos", &mut operation.pos, 0.1, 2);
                        edit_vec2(ui, "Size", &mut operation.size, 0.1, 2);
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
                                    combo_box_for_materials(ui, operation.id, materials, content);
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
                    AlterObject::None => {}
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
        let num_objects = room.operations.len();
        let mut alterations = vec![AlterObject::None; num_objects];
        for (index, opening) in room.openings.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                combo_box_for_enum(
                    ui,
                    format!("Opening {}", opening.id),
                    &mut opening.opening_type,
                    "",
                );
                edit_vec2(ui, "Pos", &mut opening.pos, 0.1, 2);
                edit_rotation(ui, &mut opening.rotation);
                labelled_widget(ui, "Width", |ui| {
                    ui.add(
                        DragValue::new(&mut opening.width)
                            .speed(0.1)
                            .fixed_decimals(1)
                            .clamp_range(0.1..=5.0)
                            .suffix("m"),
                    );
                });
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
                    room.openings.remove(index);
                }
                AlterObject::MoveUp => {
                    room.openings.swap(index, index - 1);
                }
                AlterObject::MoveDown => {
                    room.openings.swap(index, index + 1);
                }
                AlterObject::None => {}
            }
        }
    });

    alter_type
}
