use super::{edit_mode_utils::apply_standard_transform, HomeFlow};
use crate::common::{
    layout::{
        Action, Furniture, GlobalMaterial, Home, Opening, OpeningType, Operation, Outline, Room,
        Shape,
    },
    shape::coord_to_vec2,
    utils::vec2_to_egui_pos,
};
use egui::{
    collapsing_header::CollapsingState, Align2, Button, Color32, ComboBox, Context, CursorIcon,
    DragValue, Painter, PointerButton, Shape as EShape, Stroke, TextEdit, Ui, Window,
};
use glam::{dvec2 as vec2, DVec2 as Vec2};
use std::time::Duration;
use strum::IntoEnumIterator;
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
    pub bounds: (Vec2, Vec2),
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
    hovered_id: Option<Uuid>,
    snap_line_x: Option<f64>,
    snap_line_y: Option<f64>,
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

        let hover_details = self.hover_select(response, ui);

        let snap_enabled = !ui.input(|i| i.modifiers.shift); // Shift to disable snap

        if let Some(hover_details) = &hover_details {
            let mouse_down = ctx.input(|i| i.pointer.button_down(egui::PointerButton::Primary));
            if mouse_down && self.edit_mode.drag_data.is_none() && hover_details.can_drag {
                self.edit_mode.drag_data = Some(DragData {
                    id: hover_details.id,
                    object_type: hover_details.object_type,
                    manipulation_type: hover_details.manipulation_type,
                    mouse_start_pos: self.mouse_pos_world,
                    object_start_pos: hover_details.pos,
                    bounds: hover_details.bounds,
                });
            }
        }

        let mut used_dragged = false;
        let mut snap_line_x = None;
        let mut snap_line_y = None;

        if response.dragged_by(PointerButton::Primary) {
            if let Some(drag_data) = &self.edit_mode.drag_data {
                used_dragged = true;
                ctx.set_cursor_icon(CursorIcon::Grab);

                let (new_pos, new_rotation, snap_x, snap_y) =
                    self.handle_drag(drag_data, snap_enabled);
                Window::new("Dragging Info")
                    .fixed_pos(vec2_to_egui_pos(
                        self.world_to_pixels(self.mouse_pos_world.x, self.mouse_pos_world.y)
                            + vec2(0.0, -60.0),
                    ))
                    .fixed_size([200.0, 0.0])
                    .pivot(Align2::CENTER_CENTER)
                    .title_bar(false)
                    .resizable(false)
                    .interactable(false)
                    .show(ctx, |ui| {
                        ui.label(format!("Pos: ({:.1}, {:.1})", new_pos.x, new_pos.y));
                        let (bounds_min, bounds_max) = drag_data.bounds;
                        if bounds_min.distance(bounds_max) > 0.0 {
                            ui.label(format!(
                                "Size: ({:.1}, {:.1})",
                                bounds_max.x - bounds_min.x,
                                bounds_max.y - bounds_min.y
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
        if response.drag_released_by(PointerButton::Primary) {
            self.edit_mode.drag_data = None;
        }

        EditResponse {
            used_dragged,
            hovered_id: hover_details.map(|h| h.id),
            snap_line_x,
            snap_line_y,
        }
    }

    pub fn paint_edit_mode(
        &mut self,
        painter: &Painter,
        edit_response: &EditResponse,
        ctx: &Context,
    ) {
        if let Some(snap_line_x) = edit_response.snap_line_x {
            let y_level = self.world_to_pixels(-1000.0, snap_line_x).y;
            painter.add(EShape::dashed_line(
                &[
                    vec2_to_egui_pos(vec2(0.0, y_level)),
                    vec2_to_egui_pos(vec2(self.canvas_center.x * 2.0, y_level)),
                ],
                Stroke::new(10.0, Color32::from_rgba_premultiplied(50, 150, 50, 150)),
                40.0,
                20.0,
            ));
        }
        if let Some(snap_line_y) = edit_response.snap_line_y {
            let x_level = self.world_to_pixels(snap_line_y, -1000.0).x;
            painter.add(EShape::dashed_line(
                &[
                    vec2_to_egui_pos(vec2(x_level, 0.0)),
                    vec2_to_egui_pos(vec2(x_level, self.canvas_center.y * 2.0)),
                ],
                Stroke::new(10.0, Color32::from_rgba_premultiplied(50, 150, 50, 150)),
                40.0,
                20.0,
            ));
        }

        Window::new("Edit mode instructions".to_string())
            .fixed_pos(vec2_to_egui_pos(vec2(
                self.canvas_center.x,
                self.canvas_center.y * 2.0 - 10.0,
            )))
            .fixed_size([300.0, 0.0])
            .pivot(Align2::CENTER_BOTTOM)
            .title_bar(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label("Drag to move room or operation");
                    ui.label("Click to select room, escape to deselect");
                    ui.label("Shift to disable snap");
                    ui.horizontal(|ui| {
                        ui.add_space(ui.available_width() / 4.0);
                        if ui.button("Add Room").clicked() {
                            self.layout.rooms.push(Room::default());
                        }
                        if ui.button("Add Furniture").clicked() {
                            self.layout.furniture.push(Furniture::default());
                        }
                        ui.add_space(ui.available_width() / 4.0);
                    });
                });
            });

        // Get hovered room or selected room if there isn't one
        if let Some(room) = [edit_response.hovered_id, self.edit_mode.selected_id]
            .iter()
            .filter_map(|&id| id)
            .find_map(|id| self.layout.rooms.iter().find(|r| r.id == id))
        {
            let rendered_data = room.rendered_data.as_ref().unwrap();

            // Render outline
            for poly in &rendered_data.polygons {
                let points: Vec<Vec2> = poly
                    .exterior()
                    .points()
                    .map(coord_to_vec2)
                    .map(|p| self.world_to_pixels(p.x, p.y))
                    .collect();
                closed_dashed_line_with_offset(
                    painter,
                    &points,
                    Stroke::new(6.0, Color32::from_rgba_premultiplied(255, 255, 255, 150)),
                    60.0,
                    self.time * 50.0,
                );
                for interior in poly.interiors() {
                    let points: Vec<Vec2> = interior
                        .points()
                        .map(coord_to_vec2)
                        .map(|p| self.world_to_pixels(p.x, p.y))
                        .collect();
                    closed_dashed_line_with_offset(
                        painter,
                        &points,
                        Stroke::new(4.0, Color32::from_rgba_premultiplied(255, 200, 200, 150)),
                        60.0,
                        self.time * 50.0,
                    );
                }
            }

            // Render original shape
            let vertices = Shape::Rectangle.vertices(room.pos, room.size, 0.0);
            let points = vertices
                .iter()
                .map(|v| self.world_to_pixels(v.x, v.y))
                .collect::<Vec<_>>();
            let stroke = Stroke::new(3.0, Color32::from_rgb(50, 200, 50).gamma_multiply(0.6));
            closed_dashed_line_with_offset(painter, &points, stroke, 35.0, self.time * 50.0);

            // Render operations
            for operation in &room.operations {
                let vertices = operation.vertices(room.pos);
                let points = vertices
                    .iter()
                    .map(|v| self.world_to_pixels(v.x, v.y))
                    .collect::<Vec<_>>();
                let stroke = Stroke::new(
                    3.0,
                    match operation.action {
                        Action::Add => Color32::from_rgb(50, 200, 50),
                        Action::Subtract => Color32::from_rgb(200, 50, 50),
                        Action::AddWall => Color32::from_rgb(50, 100, 50),
                        Action::SubtractWall => Color32::from_rgb(160, 90, 50),
                    }
                    .gamma_multiply(0.6),
                );
                closed_dashed_line_with_offset(painter, &points, stroke, 35.0, self.time * 50.0);
            }

            // Render openings
            for opening in &room.openings {
                let selected = edit_response.hovered_id == Some(opening.id);
                // Draw a circle for each opening
                let pos = room.pos + opening.pos;
                let pos = self.world_to_pixels(pos.x, pos.y);
                let color = match opening.opening_type {
                    OpeningType::Door => Color32::from_rgb(255, 100, 0),
                    OpeningType::Window => Color32::from_rgb(0, 70, 230),
                }
                .gamma_multiply(0.8);
                painter.add(EShape::circle_filled(
                    vec2_to_egui_pos(pos),
                    if selected { 16.0 } else { 10.0 },
                    color,
                ));
                painter.add(EShape::circle_filled(
                    vec2_to_egui_pos(pos),
                    if selected { 6.0 } else { 2.0 },
                    Color32::from_rgb(0, 0, 0),
                ));
                // Add a line along its rotation
                let rot_dir = vec2(
                    opening.rotation.to_radians().cos(),
                    opening.rotation.to_radians().sin(),
                ) * (opening.width / 2.0 * self.zoom);
                let start = vec2_to_egui_pos(pos - rot_dir);
                let end = vec2_to_egui_pos(pos + rot_dir);
                painter.line_segment([start, end], Stroke::new(6.0, color));
            }
        }

        // Render furniture
        for furniture in &self.layout.furniture {
            let selected = edit_response.hovered_id == Some(furniture.id)
                || self.edit_mode.selected_id == Some(furniture.id);
            painter.add(EShape::closed_line(
                Shape::Rectangle
                    .vertices(furniture.pos, furniture.size, furniture.rotation)
                    .iter()
                    .map(|v| vec2_to_egui_pos(self.world_to_pixels(v.x, v.y)))
                    .collect(),
                Stroke::new(
                    if selected { 8.0 } else { 4.0 },
                    Color32::from_rgb(150, 0, 50).gamma_multiply(0.8),
                ),
            ));
        }

        if let Some(selected_id) = self.edit_mode.selected_id {
            let selected_type = self.edit_mode.selected_type.unwrap();
            let mut alter_type = AlterObject::None;
            let mut window_open: bool = true;
            Window::new(format!("Edit {selected_id}"))
                .default_pos(vec2_to_egui_pos(vec2(self.canvas_center.x, 20.0)))
                .fixed_size([0.0, 0.0])
                .pivot(Align2::CENTER_TOP)
                .movable(true)
                .resizable(false)
                .collapsible(true)
                .open(&mut window_open)
                .show(ctx, |ui| match selected_type {
                    ObjectType::Room => {
                        let room = self.layout.rooms.iter_mut().find(|r| r.id == selected_id);
                        if let Some(room) = room {
                            alter_type = room_edit_widgets(ui, &self.layout.materials, room);
                        }
                    }
                    ObjectType::Furniture => {
                        let furniture = self
                            .layout
                            .furniture
                            .iter_mut()
                            .find(|f| f.id == selected_id);
                        if let Some(furniture) = furniture {
                            // alter_type = furniture_edit_widgets(ui, furniture);
                        }
                    }
                    _ => {}
                });
            if !window_open {
                self.edit_mode.selected_id = None;
                self.edit_mode.selected_type = None;
            }
            match alter_type {
                AlterObject::Delete => {
                    match selected_type {
                        ObjectType::Room => {
                            self.layout.rooms.retain(|r| r.id != selected_id);
                        }
                        ObjectType::Furniture => {
                            self.layout.furniture.retain(|f| f.id != selected_id);
                        }
                        _ => {}
                    }
                    self.edit_mode.selected_id = None;
                    self.edit_mode.selected_type = None;
                }
                AlterObject::MoveUp | AlterObject::MoveDown => match selected_type {
                    ObjectType::Room => {
                        let index = self
                            .layout
                            .rooms
                            .iter()
                            .position(|r| r.id == selected_id)
                            .unwrap();
                        match alter_type {
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
                    ObjectType::Furniture => {
                        let index = self
                            .layout
                            .furniture
                            .iter()
                            .position(|f| f.id == selected_id)
                            .unwrap();
                        match alter_type {
                            AlterObject::MoveUp => {
                                if index < self.layout.furniture.len() - 1 {
                                    self.layout.furniture.swap(index, index + 1);
                                }
                            }
                            AlterObject::MoveDown => {
                                if index > 0 {
                                    self.layout.furniture.swap(index, index - 1);
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                },
                AlterObject::None => {}
            }
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
                false,
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
                                "Use Parent Material",
                                true,
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

// Helper function to create a combo box for an enum
fn combo_box_for_enum<T>(ui: &mut egui::Ui, id: impl std::hash::Hash, selected: &mut T, label: &str)
where
    T: ToString + PartialEq + Copy + IntoEnumIterator,
{
    let display_label = if label.is_empty() {
        selected.to_string()
    } else {
        format!("{}: {}", label, selected.to_string())
    };

    ComboBox::from_id_source(id)
        .selected_text(display_label)
        .show_ui(ui, |ui| {
            for variant in T::iter() {
                ui.selectable_value(selected, variant, variant.to_string());
            }
        });
}

// Helper function to create a combo box for materials
fn combo_box_for_materials(
    ui: &mut egui::Ui,
    id: Uuid,
    materials: &Vec<GlobalMaterial>,
    selected: &mut String,
) {
    ComboBox::from_id_source(format!("Materials {id}"))
        .selected_text(selected.clone())
        .show_ui(ui, |ui| {
            for material in materials {
                ui.selectable_value(selected, material.name.clone(), &material.name);
            }
        });
}

// Helper function to edit Vec2 using two DragValue widgets
fn edit_vec2(ui: &mut egui::Ui, label: &str, vec2: &mut Vec2, speed: f32, fixed_decimals: usize) {
    labelled_widget(ui, label, |ui| {
        ui.add(
            egui::DragValue::new(&mut vec2.x)
                .speed(speed)
                .fixed_decimals(fixed_decimals)
                .prefix("X: "),
        );
        ui.add(
            egui::DragValue::new(&mut vec2.y)
                .speed(speed)
                .fixed_decimals(fixed_decimals)
                .prefix("Y: "),
        );
    });
}

// Helper function to edit rotation using a DragValue widget and returning if changed
fn edit_rotation(ui: &mut egui::Ui, rotation: &mut f64) {
    labelled_widget(ui, "Rotation", |ui| {
        if ui
            .add(
                DragValue::new(rotation)
                    .speed(5)
                    .fixed_decimals(0)
                    .suffix("°"),
            )
            .changed()
        {
            *rotation = rotation.rem_euclid(360.0);
        }
    });
}

// Helper function to wrap any widget in a horizontal layout with label
fn labelled_widget<F>(ui: &mut egui::Ui, label: &str, widget: F)
where
    F: FnOnce(&mut egui::Ui),
{
    ui.horizontal(|ui| {
        ui.label(label);
        widget(ui);
    });
}

// Helper function to edit an option using a checkbox and a widget
fn edit_option<T, F, D>(
    ui: &mut egui::Ui,
    label: &str,
    inverted: bool,
    option: &mut Option<T>,
    default: D,
    mut widget: F,
) where
    F: FnMut(&mut egui::Ui, &mut T),
    D: FnOnce() -> T,
{
    let mut checkbox_state = if inverted {
        option.is_none()
    } else {
        option.is_some()
    };

    if ui
        .add(egui::Checkbox::new(&mut checkbox_state, label))
        .changed()
    {
        *option = if (checkbox_state && inverted) || (!checkbox_state && !inverted) {
            None
        } else {
            Some(default())
        };
    }

    // Pass mutable reference to the content to the widget closure
    if let Some(ref mut content) = option {
        widget(ui, content);
    }
}

fn closed_dashed_line_with_offset(
    painter: &Painter,
    points: &[Vec2],
    stroke: Stroke,
    desired_combined_length: f64,
    time: f64,
) {
    let mut points = points.to_vec();
    points.push(points[0]);

    let mut total_length = 0.0;
    for i in 0..points.len() {
        let next_index = (i + 1) % points.len();
        total_length += points[i].distance(points[next_index]);
    }

    let num_dashes = (total_length / desired_combined_length).round() as usize;
    let combined_length = total_length / num_dashes as f64;
    let dash_length = combined_length * 0.6;
    let gap_length = combined_length - dash_length;

    let offset = time % combined_length;
    let normal = (points[1] - points[0]).normalize();
    points.push(points[0] + normal * offset);

    let points = points
        .iter()
        .map(|p| vec2_to_egui_pos(*p))
        .collect::<Vec<_>>();

    painter.add(EShape::dashed_line_with_offset(
        &points,
        stroke,
        &[dash_length as f32],
        &[gap_length as f32],
        offset as f32,
    ));
}
