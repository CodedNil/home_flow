use std::collections::HashMap;

use super::HomeFlow;
use crate::common::{
    layout::{Action, Operation, RenderOptions, Room, TileOptions, Vec2},
    shape::{Material, Shape, WallType},
};
use egui::{
    Align2, Button, Checkbox, Color32, ComboBox, Context, DragValue, Painter, Pos2, Rect,
    Shape as EShape, Stroke, Ui, Window,
};
use strum::VariantArray;
use uuid::Uuid;

#[derive(Default)]
pub struct EditDetails {
    pub enabled: bool,
    drag_data: Option<DragData>,
    room_window_bounds: Option<(Uuid, Rect)>,
    preview_edits: bool,
}

#[derive(Clone)]
struct DragData {
    room_id: Uuid,
    id: Uuid,
    mouse_start_pos: Pos2,
    room_start_pos: Vec2,
}

pub struct EditResponse {
    pub used_dragged: bool,
    room_hovered: Option<Uuid>,
    snap_line_horizontal: Option<f32>,
    snap_line_vertical: Option<f32>,
}

impl HomeFlow {
    pub fn edit_mode_settings(&mut self, ctx: &Context, ui: &mut Ui) {
        // If in edit mode, show button to view save and discard changes
        if self.edit_mode.enabled {
            if ui.button("Preview Edits").clicked() {
                self.edit_mode.preview_edits = !self.edit_mode.preview_edits;
            }
            if ui.button("Save Edits").clicked() {
                log::info!("Saving layout");
                ehttp::fetch(
                    ehttp::Request::post(
                        format!("http://{}/save_layout", self.host),
                        serde_json::to_string(&self.layout)
                            .unwrap()
                            .as_bytes()
                            .to_vec(),
                    ),
                    move |_| {},
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

    pub fn run_edit_mode(
        &mut self,
        response: &egui::Response,
        mouse_pos: Pos2,
        mouse_pos_world: Pos2,
    ) -> EditResponse {
        if !self.edit_mode.enabled {
            return EditResponse {
                used_dragged: false,
                room_hovered: None,
                snap_line_horizontal: None,
                snap_line_vertical: None,
            };
        }

        let mut used_dragged = false;

        let mut room_hovered = None;
        let mut operation_hovered = None;
        for room in &self.layout.rooms {
            if room.contains_full(mouse_pos_world.x, mouse_pos_world.y) {
                room_hovered = Some(room.id);
            }
            for operation in &room.operations {
                if operation.shape.contains(
                    Vec2::new(mouse_pos_world.x, mouse_pos_world.y),
                    room.pos + operation.pos,
                    operation.size,
                ) {
                    operation_hovered = Some(operation.id);
                }
            }
        }
        if let Some(drag_data) = self.edit_mode.drag_data.clone() {
            room_hovered = Some(drag_data.room_id);
            if drag_data.id != drag_data.room_id {
                operation_hovered = Some(drag_data.id);
            }
        }
        // Check if within bounds of room window and set as hovered if so
        if let Some(room_window_bounds) = self.edit_mode.room_window_bounds {
            if room_hovered.is_none() && room_window_bounds.1.contains(mouse_pos) {
                room_hovered = Some(room_window_bounds.0);
            }
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
                                mouse_start_pos: mouse_pos_world,
                                room_start_pos: room.pos + operation.pos,
                            });
                        }
                    }
                } else {
                    self.edit_mode.drag_data = Some(DragData {
                        room_id: room.id,
                        id: room.id,
                        mouse_start_pos: mouse_pos_world,
                        room_start_pos: room.pos,
                    });
                }
            }
            if response.dragged() {
                if let Some(drag_data) = self.edit_mode.drag_data.clone() {
                    used_dragged = true;

                    (snap_line_horizontal, snap_line_vertical) =
                        self.handle_room_drag(&drag_data, mouse_pos_world, drag_data.room_id);
                }
            }
            if response.drag_released() {
                self.edit_mode.drag_data = None;
            }
        }

        EditResponse {
            used_dragged,
            room_hovered,
            snap_line_horizontal,
            snap_line_vertical,
        }
    }

    fn handle_room_drag(
        &mut self,
        drag_data: &DragData,
        mouse_pos_world: Pos2,
        rooms_id: Uuid,
    ) -> (Option<f32>, Option<f32>) {
        let mut snap_line_horizontal = None;
        let mut snap_line_vertical = None;

        let delta = mouse_pos_world - drag_data.mouse_start_pos;
        let mut new_pos = drag_data.room_start_pos + Vec2::new(delta.x, delta.y);

        let room = self.layout.rooms.iter().find(|r| r.id == rooms_id).unwrap();
        let (bounds_min, bounds_max) = if drag_data.id == rooms_id {
            let (bounds_min, bounds_max) = room.self_bounds();
            (
                bounds_min - room.pos + new_pos,
                bounds_max - room.pos + new_pos,
            )
        } else {
            let (mut bounds_min, mut bounds_max) = (Vec2::new(0.0, 0.0), Vec2::new(0.0, 0.0));
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
        canvas_center: Pos2,
        edit_response: &EditResponse,
        ctx: &Context,
    ) {
        if let Some(snap_line_horizontal) = edit_response.snap_line_horizontal {
            let y_level = self
                .world_to_pixels(canvas_center, -1000.0, snap_line_horizontal)
                .y;
            painter.add(EShape::dashed_line(
                &[
                    Pos2::new(0.0, y_level),
                    Pos2::new(canvas_center.x * 2.0, y_level),
                ],
                Stroke::new(10.0, Color32::from_rgba_premultiplied(50, 150, 50, 150)),
                40.0,
                20.0,
            ));
        }
        if let Some(snap_line_vertical) = edit_response.snap_line_vertical {
            let x_level = self
                .world_to_pixels(canvas_center, snap_line_vertical, -1000.0)
                .x;
            painter.add(EShape::dashed_line(
                &[
                    Pos2::new(x_level, 0.0),
                    Pos2::new(x_level, canvas_center.y * 2.0),
                ],
                Stroke::new(10.0, Color32::from_rgba_premultiplied(50, 150, 50, 150)),
                40.0,
                20.0,
            ));
        }
        let home_render = self.layout.rendered_data.clone().unwrap();
        if let Some(room_id) = &edit_response.room_hovered {
            let room = self.layout.rooms.iter().find(|r| &r.id == room_id).unwrap();

            // Render outline
            let vertices = home_render.vertices.get(room_id).unwrap();
            let points = vertices
                .iter()
                .map(|v| self.world_to_pixels(canvas_center, v.x, v.y))
                .collect::<Vec<_>>();
            closed_dashed_line_with_offset(
                painter,
                &points,
                Stroke::new(6.0, Color32::from_rgba_premultiplied(255, 255, 255, 150)),
                60.0,
                self.time as f32 * 50.0,
            );

            // Render operations
            for operation in &room.operations {
                let vertices = operation
                    .shape
                    .vertices(room.pos + operation.pos, operation.size);
                let points = vertices
                    .iter()
                    .map(|v| self.world_to_pixels(canvas_center, v.x, v.y))
                    .collect::<Vec<_>>();
                let stroke = Stroke::new(
                    3.0,
                    match operation.action {
                        Action::Add => Color32::from_rgb(50, 200, 50),
                        Action::Subtract => Color32::from_rgb(200, 50, 50),
                    }
                    .gamma_multiply(0.6),
                );
                closed_dashed_line_with_offset(
                    painter,
                    &points,
                    stroke,
                    35.0,
                    self.time as f32 * 50.0,
                );
            }
        }
        let mut room_positions = HashMap::new();
        for room in &self.layout.rooms {
            let room_pos =
                self.world_to_pixels(canvas_center, room.pos.x, room.pos.y + room.size.y / 2.0)
                    + egui::Vec2::new(0.0, -20.0);
            room_positions.insert(room.id, room_pos);
        }
        if let Some(room_id) = &edit_response.room_hovered {
            let room = self
                .layout
                .rooms
                .iter_mut()
                .find(|r| &r.id == room_id)
                .unwrap();
            Window::new(format!("Edit {}", room.id))
                .fixed_pos(room_positions[room_id])
                .fixed_size([200.0, 0.0])
                .pivot(Align2::CENTER_BOTTOM)
                .title_bar(false)
                .resizable(false)
                .show(ctx, |ui| {
                    room_edit_widgets(ui, room);

                    let mut padding = 60.0;
                    ui.memory(|memory| {
                        if memory.any_popup_open() {
                            padding += 50.0;
                        }
                    });
                    let window_rect = ui.min_rect().expand(padding);
                    self.edit_mode.room_window_bounds = Some((room.id, window_rect));
                });
        } else {
            self.edit_mode.room_window_bounds = None;
        }
    }
}

fn room_edit_widgets(ui: &mut egui::Ui, room: &mut Room) {
    ui.horizontal(|ui| {
        ui.label("Room ");
        ui.text_edit_singleline(&mut room.name);
    });
    ui.separator();

    egui::Grid::new("my_grid")
        .num_columns(2)
        .spacing([40.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label("Position ");
            ui.horizontal(|ui| {
                ui.add(DragValue::new(&mut room.pos.x).speed(0.1).fixed_decimals(1));
                ui.add(DragValue::new(&mut room.pos.y).speed(0.1).fixed_decimals(1));
            });
            ui.end_row();

            ui.label("Size ");
            ui.horizontal(|ui| {
                ui.add(
                    DragValue::new(&mut room.size.x)
                        .speed(0.1)
                        .fixed_decimals(1),
                );
                ui.add(
                    DragValue::new(&mut room.size.y)
                        .speed(0.1)
                        .fixed_decimals(1),
                );
            });
            ui.end_row();

            // Wall selection
            for (wall_side, wall_type) in room.walls.iter_mut().enumerate() {
                let room_side = match wall_side {
                    0 => "Left",
                    1 => "Top",
                    2 => "Right",
                    _ => "Bottom",
                };
                combo_box_for_enum(
                    ui,
                    format!("{room_side} Wall"),
                    wall_type,
                    WallType::VARIANTS,
                    &format!("{room_side} Wall"),
                );
                if wall_side == 1 {
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

    // List operations with buttons to delete and button to add, and drag to reorder
    ui.horizontal(|ui| {
        ui.label("Operations");
        if ui.add(Button::new("Add")).clicked() {
            room.operations.push(Operation {
                id: Uuid::new_v4(),
                action: Action::Add,
                shape: Shape::Rectangle,
                render_options: RenderOptions::default(),
                pos: Vec2::new(0.0, 0.0),
                size: Vec2::new(1.0, 1.0),
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
        });

        if operation.action == Action::Add {
            render_options_widgets(
                ui,
                &mut operation.render_options,
                format!("Materials Operation {index}"),
            );
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
}

fn render_options_widgets(ui: &mut egui::Ui, render_options: &mut RenderOptions, id: String) {
    ui.horizontal(|ui| {
        combo_box_for_enum(ui, id, &mut render_options.material, Material::VARIANTS, "");
        ui.add(
            DragValue::new(&mut render_options.scale)
                .speed(0.1)
                .fixed_decimals(1)
                .clamp_range(0.1..=100.0),
        );

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

    // Tiles boolean and then pub struct TileOptions { scale: u8, odd_tint: Color32, grout_width: f32, grout_tint: Color32 }
    ui.horizontal(|ui| {
        if ui
            .add(Checkbox::new(&mut render_options.tiles.is_some(), "Tiles"))
            .changed()
        {
            if render_options.tiles.is_some() {
                render_options.tiles = None;
            } else {
                render_options.tiles = Some(TileOptions::default());
            }
        }
        if let Some(tile_options) = &mut render_options.tiles {
            ui.add(
                DragValue::new(&mut tile_options.scale)
                    .speed(1)
                    .fixed_decimals(0)
                    .clamp_range(0..=100),
            );
            ui.color_edit_button_srgba(&mut tile_options.odd_tint);
            ui.add(
                DragValue::new(&mut tile_options.grout_width)
                    .speed(0.005)
                    .fixed_decimals(1)
                    .clamp_range(0.0..=1.0),
            );
            ui.color_edit_button_srgba(&mut tile_options.grout_tint);
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
    points: &[Pos2],
    stroke: Stroke,
    desired_combined_length: f32,
    time: f32,
) {
    let mut points = points.to_vec();
    points.push(points[0]);

    let mut total_length = 0.0;
    for i in 0..points.len() {
        let next_index = (i + 1) % points.len();
        total_length += points[i].distance(points[next_index]);
    }

    let num_dashes = (total_length / desired_combined_length).round() as usize;
    let combined_length = total_length / num_dashes as f32;
    let dash_length = combined_length * 0.6;
    let gap_length = combined_length - dash_length;

    let offset = time % combined_length;
    let normal = (points[1] - points[0]).normalized();
    points.push(points[0] + normal * offset);

    painter.add(EShape::dashed_line_with_offset(
        &points,
        stroke,
        &[dash_length],
        &[gap_length],
        offset,
    ));
}
