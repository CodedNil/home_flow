use super::HomeFlow;
use crate::common::{
    layout::{Action, Furniture, Opening, OpeningType, Operation, RenderOptions, Room, Walls},
    shape::{coord_to_vec2, Material, Shape, WallType},
    utils::vec2_to_egui_pos,
};
use egui::{
    Align2, Button, Checkbox, Color32, ComboBox, Context, DragValue, Painter, Shape as EShape,
    Stroke, Ui, Window,
};
use glam::{dvec2 as vec2, DVec2 as Vec2};
use std::time::Duration;
use strum::VariantArray;
use uuid::Uuid;

#[derive(Default)]
pub struct EditDetails {
    pub enabled: bool,
    drag_data: Option<DragData>,
    selected_room: Option<Uuid>,
    preview_edits: bool,
}

#[derive(Clone)]
struct DragData {
    room_id: Uuid,
    id: Uuid,
    mouse_start_pos: Vec2,
    room_start_pos: Vec2,
}

pub struct EditResponse {
    pub used_dragged: bool,
    room_hovered: Option<Uuid>,
    room_selected: Option<Uuid>,
    snap_line_horizontal: Option<f64>,
    snap_line_vertical: Option<f64>,
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
    }

    pub fn run_edit_mode(&mut self, response: &egui::Response, ctx: &Context) -> EditResponse {
        if !self.edit_mode.enabled {
            return EditResponse {
                used_dragged: false,
                room_hovered: None,
                room_selected: None,
                snap_line_horizontal: None,
                snap_line_vertical: None,
            };
        }

        let mut used_dragged = false;

        let mut room_hovered = None;
        let mut operation_hovered = None;
        for room in &self.layout.rooms {
            if room.contains_full(self.mouse_pos_world.x, self.mouse_pos_world.y)
                && (self.edit_mode.selected_room.is_none()
                    || self.edit_mode.selected_room == Some(room.id))
            {
                room_hovered = Some(room.id);
                operation_hovered = None;
            }
            if room_hovered == Some(room.id) {
                for operation in &room.operations {
                    if operation.shape.contains(
                        vec2(self.mouse_pos_world.x, self.mouse_pos_world.y),
                        room.pos + operation.pos,
                        operation.size,
                        operation.rotation,
                    ) {
                        room_hovered = Some(room.id);
                        operation_hovered = Some(operation.id);
                    }
                }
            }
        }
        if let Some(drag_data) = self.edit_mode.drag_data.clone() {
            room_hovered = Some(drag_data.room_id);
            if drag_data.id != drag_data.room_id {
                operation_hovered = Some(drag_data.id);
            }
        }
        if response.double_clicked() {
            self.edit_mode.selected_room = room_hovered;
        }

        // Apply edit changes
        let mut snap_line_horizontal = None;
        let mut snap_line_vertical = None;
        if let Some(room_id) = &room_hovered {
            let room = self.layout.rooms.iter().find(|r| &r.id == room_id).unwrap();
            if response.drag_started() {
                if let Some(op) = operation_hovered {
                    for operation in &room.operations {
                        if operation.id == op {
                            self.edit_mode.drag_data = Some(DragData {
                                room_id: room.id,
                                id: operation.id,
                                mouse_start_pos: self.mouse_pos_world,
                                room_start_pos: room.pos + operation.pos,
                            });
                        }
                    }
                } else {
                    self.edit_mode.drag_data = Some(DragData {
                        room_id: room.id,
                        id: room.id,
                        mouse_start_pos: self.mouse_pos_world,
                        room_start_pos: room.pos,
                    });
                }
            }
            if response.dragged() {
                if let Some(drag_data) = self.edit_mode.drag_data.clone() {
                    if Some(drag_data.room_id) == self.edit_mode.selected_room {
                        used_dragged = true;
                        (snap_line_horizontal, snap_line_vertical) =
                            self.handle_room_drag(&drag_data, drag_data.room_id, ctx);
                    }
                }
            }
            if response.drag_released() {
                self.edit_mode.drag_data = None;
            }
        }

        if let Some(room_id) = &self.edit_mode.selected_room {
            room_hovered = Some(*room_id);
        }

        EditResponse {
            used_dragged,
            room_hovered,
            room_selected: self.edit_mode.selected_room,
            snap_line_horizontal,
            snap_line_vertical,
        }
    }

    fn handle_room_drag(
        &mut self,
        drag_data: &DragData,
        rooms_id: Uuid,
        ctx: &Context,
    ) -> (Option<f64>, Option<f64>) {
        let mut snap_line_horizontal = None;
        let mut snap_line_vertical = None;

        let delta = self.mouse_pos_world - drag_data.mouse_start_pos;
        let mut new_pos = drag_data.room_start_pos + vec2(delta.x, delta.y);

        let room = self.layout.rooms.iter().find(|r| r.id == rooms_id).unwrap();
        let (bounds_min, bounds_max) = if drag_data.id == rooms_id {
            let (bounds_min, bounds_max) = room.self_bounds();
            (
                bounds_min - room.pos + new_pos,
                bounds_max - room.pos + new_pos,
            )
        } else {
            let (mut bounds_min, mut bounds_max) = (vec2(0.0, 0.0), vec2(0.0, 0.0));
            for operation in &room.operations {
                if operation.id == drag_data.id {
                    (bounds_min, bounds_max) = (
                        new_pos - operation.size / 2.0,
                        new_pos + operation.size / 2.0,
                    );
                }
            }
            (bounds_min, bounds_max)
        };

        // Snap to other rooms
        let mut closest_horizontal_snap_line = None;
        let mut closest_vertical_snap_line = None;
        let snap_threshold = 0.1;

        for other_room in &self.layout.rooms {
            if drag_data.id != rooms_id || other_room.id != rooms_id {
                let (other_min, other_max) = other_room.self_bounds();
                // Horizontal snap
                for (index, &room_edge) in [bounds_min.y, bounds_max.y].iter().enumerate() {
                    for &other_edge in &[other_min.y, other_max.y] {
                        // Check if vertically within range
                        if !((bounds_min.x < other_max.x + snap_threshold
                            && bounds_min.x > other_min.x - snap_threshold)
                            || (bounds_max.x < other_max.x + snap_threshold
                                && bounds_max.x > other_min.x - snap_threshold))
                        {
                            continue;
                        }

                        let distance = (room_edge - other_edge).abs();
                        if distance < snap_threshold
                            && closest_horizontal_snap_line
                                .map_or(true, |(_, dist, _)| distance < dist)
                        {
                            closest_horizontal_snap_line = Some((other_edge, distance, index));
                        }
                    }
                }
                // Vertical snap
                for (index, &room_edge) in [bounds_min.x, bounds_max.x].iter().enumerate() {
                    for &other_edge in &[other_min.x, other_max.x] {
                        // Check if horizontally within range
                        if !((bounds_min.y < other_max.y + snap_threshold
                            && bounds_min.y > other_min.y - snap_threshold)
                            || (bounds_max.y < other_max.y + snap_threshold
                                && bounds_max.y > other_min.y - snap_threshold))
                        {
                            continue;
                        }

                        let distance = (room_edge - other_edge).abs();
                        if distance < snap_threshold
                            && closest_vertical_snap_line
                                .map_or(true, |(_, dist, _)| distance < dist)
                        {
                            closest_vertical_snap_line = Some((other_edge, distance, index));
                        }
                    }
                }
            }
        }
        new_pos.y = if let Some((snap_line, _, edge)) = closest_horizontal_snap_line {
            // Snap to other room
            snap_line_horizontal = Some(snap_line);
            if edge == 0 {
                snap_line + (bounds_max.y - bounds_min.y) / 2.0
            } else {
                snap_line - (bounds_max.y - bounds_min.y) / 2.0
            }
        } else {
            // Snap to grid
            (new_pos.y * 10.0).round() / 10.0
        };
        new_pos.x = if let Some((snap_line, _, edge)) = closest_vertical_snap_line {
            // Snap to other room
            snap_line_vertical = Some(snap_line);
            if edge == 0 {
                snap_line + (bounds_max.x - bounds_min.x) / 2.0
            } else {
                snap_line - (bounds_max.x - bounds_min.x) / 2.0
            }
        } else {
            // Snap to grid
            (new_pos.x * 10.0).round() / 10.0
        };

        Window::new(format!("Drag {rooms_id}"))
            .fixed_pos(vec2_to_egui_pos(self.world_to_pixels(new_pos.x, new_pos.y)))
            .fixed_size([200.0, 0.0])
            .pivot(Align2::CENTER_CENTER)
            .title_bar(false)
            .resizable(false)
            .interactable(false)
            .show(ctx, |ui| {
                ui.label(format!("Pos: ({:.1}, {:.1})", new_pos.x, new_pos.y));
                ui.label(format!(
                    "Size: ({:.1}, {:.1})",
                    bounds_max.x - bounds_min.x,
                    bounds_max.y - bounds_min.y
                ));
            });

        let room = self
            .layout
            .rooms
            .iter_mut()
            .find(|r| r.id == rooms_id)
            .unwrap();
        if drag_data.id == rooms_id {
            room.pos = new_pos;
        } else {
            for operation in &mut room.operations {
                if operation.id == drag_data.id {
                    operation.pos = new_pos - room.pos;
                }
            }
        };

        (snap_line_horizontal, snap_line_vertical)
    }

    pub fn paint_edit_mode(
        &mut self,
        painter: &Painter,
        edit_response: &EditResponse,
        ctx: &Context,
    ) {
        if let Some(snap_line_horizontal) = edit_response.snap_line_horizontal {
            let y_level = self.world_to_pixels(-1000.0, snap_line_horizontal).y;
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
        if let Some(snap_line_vertical) = edit_response.snap_line_vertical {
            let x_level = self.world_to_pixels(snap_line_vertical, -1000.0).x;
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
                    ui.label("Double click to select room with options");
                    ui.horizontal(|ui| {
                        if ui.button("Add Room").clicked() {
                            self.layout.rooms.push(Room::new(
                                "New Room",
                                vec2(0.0, 0.0),
                                vec2(1.0, 1.0),
                                RenderOptions::default(),
                                Walls::WALL,
                                vec![],
                            ));
                        }
                        if ui.button("Add Opening").clicked() {
                            self.layout.openings.push(Opening {
                                id: Uuid::new_v4(),
                                opening_type: OpeningType::Door,
                                pos: vec2(0.0, 0.0),
                            });
                        }
                        if ui.button("Add Furniture").clicked() {
                            self.layout.furniture.push(Furniture {
                                id: Uuid::new_v4(),
                                pos: vec2(0.0, 0.0),
                                size: vec2(1.0, 1.0),
                                rotation: 0.0,
                                children: vec![],
                            });
                        }
                    });
                });
            });

        for opening in &self.layout.openings {
            // Draw a circle for each opening
            let pos = self.world_to_pixels(opening.pos.x, opening.pos.y);
            painter.add(EShape::circle_filled(
                vec2_to_egui_pos(pos),
                10.0,
                Color32::from_rgb(255, 255, 0).gamma_multiply(0.5),
            ));
            painter.add(EShape::circle_filled(
                vec2_to_egui_pos(pos),
                2.0,
                Color32::from_rgb(0, 0, 0),
            ));
        }

        if let Some(room_id) = &edit_response.room_hovered {
            let room = self.layout.rooms.iter().find(|r| &r.id == room_id).unwrap();
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
                let vertices = operation.shape.vertices(
                    room.pos + operation.pos,
                    operation.size,
                    operation.rotation,
                );
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
        }
        if let Some(room_id) = &edit_response.room_selected {
            let room = self
                .layout
                .rooms
                .iter_mut()
                .find(|r| &r.id == room_id)
                .unwrap();
            let mut alter_room = AlterRoom::None;
            let mut window_open: bool = true;
            Window::new(format!("Edit {}", room.id))
                .default_pos(vec2_to_egui_pos(vec2(self.canvas_center.x, 20.0)))
                .default_size([0.0, 0.0])
                .pivot(Align2::CENTER_TOP)
                .movable(true)
                .resizable(true)
                .collapsible(true)
                .open(&mut window_open)
                .show(ctx, |ui| {
                    alter_room = room_edit_widgets(ui, room);
                });
            if !window_open {
                self.edit_mode.selected_room = None;
            }
            match alter_room {
                AlterRoom::Delete => {
                    self.layout.rooms.retain(|r| r.id != *room_id);
                    self.edit_mode.selected_room = None;
                }
                AlterRoom::MoveUp => {
                    let index = self
                        .layout
                        .rooms
                        .iter()
                        .position(|r| r.id == *room_id)
                        .unwrap();
                    if index < self.layout.rooms.len() - 1 {
                        self.layout.rooms.swap(index, index + 1);
                    }
                }
                AlterRoom::MoveDown => {
                    let index = self
                        .layout
                        .rooms
                        .iter()
                        .position(|r| r.id == *room_id)
                        .unwrap();
                    if index > 0 {
                        self.layout.rooms.swap(index, index - 1);
                    }
                }
                AlterRoom::None => {}
            }
        }
    }
}

enum AlterRoom {
    None,
    Delete,
    MoveUp,
    MoveDown,
}

fn room_edit_widgets(ui: &mut egui::Ui, room: &mut Room) -> AlterRoom {
    let mut alter_room = AlterRoom::None;
    ui.horizontal(|ui| {
        ui.label("Room ");
        ui.text_edit_singleline(&mut room.name);
        if ui.add(Button::new("Delete")).clicked() {
            alter_room = AlterRoom::Delete;
        }
        if ui.add(Button::new("^")).clicked() {
            alter_room = AlterRoom::MoveUp;
        }
        if ui.add(Button::new("v")).clicked() {
            alter_room = AlterRoom::MoveDown;
        }
    });
    ui.separator();

    egui::Grid::new("Room Edit Grid")
        .num_columns(2)
        .spacing([40.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label("Position ");
            ui.horizontal(|ui| {
                ui.add(DragValue::new(&mut room.pos.x).speed(0.1).fixed_decimals(2));
                ui.add(DragValue::new(&mut room.pos.y).speed(0.1).fixed_decimals(2));
            });
            ui.end_row();

            ui.label("Size ");
            ui.horizontal(|ui| {
                ui.add(
                    DragValue::new(&mut room.size.x)
                        .speed(0.1)
                        .fixed_decimals(2),
                );
                ui.add(
                    DragValue::new(&mut room.size.y)
                        .speed(0.1)
                        .fixed_decimals(2),
                );
            });
            ui.end_row();

            // Wall selection
            for index in 0..4 {
                let (wall_type, wall_side) = match index {
                    0 => (&mut room.walls.left, "Left"),
                    1 => (&mut room.walls.top, "Top"),
                    2 => (&mut room.walls.right, "Right"),
                    _ => (&mut room.walls.bottom, "Bottom"),
                };
                combo_box_for_enum(
                    ui,
                    format!("{wall_side} Wall"),
                    wall_type,
                    WallType::VARIANTS,
                    &format!("{wall_side} Wall"),
                );
                if index == 1 {
                    ui.end_row();
                }
            }
            ui.end_row();
        });
    render_options_widgets(
        ui,
        &mut room.render_options,
        format!("Materials {}", room.id),
    );
    ui.separator();

    ui.collapsing("Operations", |ui| {
        // List operations with buttons to delete and button to add, and drag to reorder
        ui.horizontal(|ui| {
            ui.label("Operations");
            if ui.add(Button::new("Add")).clicked() {
                room.operations.push(Operation {
                    id: Uuid::new_v4(),
                    action: Action::Add,
                    shape: Shape::Rectangle,
                    render_options: None,
                    pos: vec2(0.0, 0.0),
                    size: vec2(1.0, 1.0),
                    rotation: 0.0,
                });
            }
        });
        if !room.operations.is_empty() {
            ui.separator();
        }
        let mut operations_to_remove = vec![];
        let mut operations_to_raise = vec![];
        let mut operations_to_lower = vec![];
        let num_operations = room.operations.len();
        for (index, operation) in room.operations.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{index}"));
                combo_box_for_enum(
                    ui,
                    format!("Operation {index}"),
                    &mut operation.action,
                    Action::VARIANTS,
                    "",
                );
                combo_box_for_enum(
                    ui,
                    format!("Shape {index}"),
                    &mut operation.shape,
                    Shape::VARIANTS,
                    "",
                );

                if ui.add(Button::new("Delete")).clicked() {
                    operations_to_remove.push(index);
                }

                if index > 0 && ui.add(Button::new("^")).clicked() {
                    operations_to_raise.push(index);
                }
                if index < num_operations - 1 && ui.add(Button::new("v")).clicked() {
                    operations_to_lower.push(index);
                }
            });

            ui.horizontal(|ui| {
                ui.label("Pos");
                ui.add(
                    DragValue::new(&mut operation.pos.x)
                        .speed(0.1)
                        .fixed_decimals(2),
                );
                ui.add(
                    DragValue::new(&mut operation.pos.y)
                        .speed(0.1)
                        .fixed_decimals(2),
                );
                ui.label("Size");
                ui.add(
                    DragValue::new(&mut operation.size.x)
                        .speed(0.1)
                        .fixed_decimals(2),
                );
                ui.add(
                    DragValue::new(&mut operation.size.y)
                        .speed(0.1)
                        .fixed_decimals(2),
                );
                ui.label("Rotation");
                if ui
                    .add(
                        DragValue::new(&mut operation.rotation)
                            .speed(5)
                            .fixed_decimals(0),
                    )
                    .changed()
                {
                    operation.rotation = operation.rotation.rem_euclid(360.0);
                }
            });

            if operation.action == Action::Add {
                // Add tickbox to use parents material or custom
                if ui
                    .add(Checkbox::new(
                        &mut operation.render_options.is_none(),
                        "Use Parent Material",
                    ))
                    .changed()
                {
                    if operation.render_options.is_some() {
                        operation.render_options = None;
                    } else {
                        operation.render_options = Some(RenderOptions::default());
                    }
                }
                if let Some(render_options) = &mut operation.render_options {
                    render_options_widgets(
                        ui,
                        render_options,
                        format!("Materials Operation {index}"),
                    );
                }
            }

            ui.separator();
        }
        if room.operations.is_empty() {
            ui.separator();
        }
        for index in operations_to_remove {
            room.operations.remove(index);
        }
        for index in operations_to_raise {
            if index > 0 {
                room.operations.swap(index, index - 1);
            }
        }
        for index in operations_to_lower {
            if index < room.operations.len() - 1 {
                room.operations.swap(index, index + 1);
            }
        }
    });

    alter_room
}

fn render_options_widgets(ui: &mut egui::Ui, render_options: &mut RenderOptions, id: String) {
    ui.horizontal(|ui| {
        combo_box_for_enum(ui, id, &mut render_options.material, Material::VARIANTS, "");

        // Tint boolean and then color picker
        if ui
            .add(Checkbox::new(&mut render_options.tint.is_some(), "Tint"))
            .changed()
        {
            if render_options.tint.is_some() {
                render_options.tint = None;
            } else {
                render_options.tint = Some(Color32::WHITE);
            }
        }
        if let Some(tint) = &mut render_options.tint {
            ui.color_edit_button_srgba(tint);
        }
    });
}

fn combo_box_for_enum<T: ToString + PartialEq + Copy>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    selected: &mut T,
    variants: &[T],
    label: &str,
) {
    let display_label = if label.is_empty() {
        selected.to_string()
    } else {
        format!("{}: {}", label, selected.to_string())
    };

    ComboBox::from_id_source(id)
        .selected_text(display_label)
        .show_ui(ui, |ui| {
            for variant in variants {
                ui.selectable_value(selected, *variant, variant.to_string());
            }
        });
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
