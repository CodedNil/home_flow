use super::HomeFlow;
use crate::common::{
    layout::{
        Action, Furniture, Home, Opening, OpeningType, Operation, Outline, RenderOptions, Room,
        Shape, Walls,
    },
    shape::coord_to_vec2,
    utils::vec2_to_egui_pos,
};
use egui::{
    collapsing_header::CollapsingState, Align2, Button, Checkbox, Color32, ComboBox, Context,
    CursorIcon, DragValue, Key, Painter, PointerButton, Shape as EShape, Stroke, Ui, Window,
};
use glam::{dvec2 as vec2, DVec2 as Vec2};
use std::time::Duration;
use strum::IntoEnumIterator;
use uuid::Uuid;

#[derive(Default)]
pub struct EditDetails {
    pub enabled: bool,
    drag_data: Option<DragData>,
    selected_room: Option<Uuid>,
    preview_edits: bool,
}

struct DragData {
    id: Uuid,
    object_type: ObjectType,
    manipulation_type: ManipulationType,
    mouse_start_pos: Vec2,
    object_start_pos: Vec2,
    bounds: (Vec2, Vec2),
}

#[derive(Clone, Copy, PartialEq)]
enum ObjectType {
    Room,
    Operation,
    Opening,
    Furniture,
}
impl ObjectType {
    const fn snap_to_grid(self) -> bool {
        matches!(self, Self::Room | Self::Operation)
    }
}

#[derive(Clone, Copy)]
enum ManipulationType {
    Move,
    ResizeLeft,
    ResizeRight,
    ResizeTop,
    ResizeBottom,
}

impl ManipulationType {
    const fn sign(self) -> f64 {
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
    room_selected: Option<Uuid>,
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
        if ui.button("Refresn").clicked() {
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
                room_selected: None,
                snap_line_x: None,
                snap_line_y: None,
            };
        }

        let mut used_dragged = false;
        let mut hovered_id = None;
        let mut hovered_type = None;
        let mut hovered_pos = None;
        let mut hovered_bounds = None;

        if let Some(selected_id) = self.edit_mode.selected_room {
            // Selected room hover
            for room in &self.layout.rooms {
                if room.id == selected_id {
                    if room.contains(self.mouse_pos_world) {
                        hovered_id = Some(room.id);
                        hovered_type = Some(ObjectType::Room);
                        hovered_pos = Some(room.pos);
                        hovered_bounds = Some((-room.size / 2.0, room.size / 2.0));
                    }
                    for operation in &room.operations {
                        if operation.contains(room.pos, self.mouse_pos_world) {
                            hovered_id = Some(operation.id);
                            hovered_type = Some(ObjectType::Operation);
                            hovered_pos = Some(room.pos + operation.pos);
                            hovered_bounds = Some((-operation.size / 2.0, operation.size / 2.0));
                        }
                    }
                    for opening in &room.openings {
                        if (self.mouse_pos_world - (room.pos + opening.pos)).length() < 0.2 {
                            hovered_id = Some(opening.id);
                            hovered_type = Some(ObjectType::Opening);
                            hovered_pos = Some(room.pos + opening.pos);
                            hovered_bounds = Some((Vec2::ZERO, Vec2::ZERO));
                        }
                    }
                }
            }
        } else {
            // Hover over rooms
            for room in &self.layout.rooms {
                if room.contains(self.mouse_pos_world) {
                    hovered_id = Some(room.id);
                    hovered_type = Some(ObjectType::Room);
                    hovered_pos = Some(room.pos);
                    hovered_bounds = Some((-room.size / 2.0, room.size / 2.0));
                }
            }
            // Hover over furniture
            for furniture in &self.layout.furniture {
                if Shape::Rectangle.contains(
                    self.mouse_pos_world,
                    furniture.pos,
                    furniture.size,
                    furniture.rotation,
                ) {
                    hovered_id = Some(furniture.id);
                    hovered_type = Some(ObjectType::Furniture);
                    hovered_pos = Some(furniture.pos);
                    hovered_bounds = Some(furniture.bounds());
                }
            }
        }
        // Double click to select room
        if response.double_clicked() {
            self.edit_mode.selected_room = self.layout.rooms.iter().rev().find_map(|room| {
                if room.contains(self.mouse_pos_world) {
                    Some(room.id)
                } else {
                    None
                }
            });
            self.edit_mode.drag_data = None;
        }
        // Escape to deselect room
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            self.edit_mode.selected_room = None;
            self.edit_mode.drag_data = None;
        }
        // Shift to disable snap
        let snap_enabled = !ui.input(|i| i.modifiers.shift);

        // If room or operation or furniture, check if at the edge of bounds to resize
        let mut manipulation_type = ManipulationType::Move;
        if let Some(object_type) = &hovered_type {
            if matches!(
                object_type,
                ObjectType::Room | ObjectType::Operation | ObjectType::Furniture
            ) {
                let (min, max) = hovered_bounds.unwrap();
                let (min, max) = (hovered_pos.unwrap() + min, hovered_pos.unwrap() + max);
                let size = max - min;
                // Min and max for y are swapped because of the coordinate system
                let (min, max) = (
                    self.world_to_pixels(min.x, max.y),
                    self.world_to_pixels(max.x, min.y),
                );
                let threshold = 20.0;
                if self.mouse_pos.x < min.x + threshold {
                    manipulation_type = ManipulationType::ResizeLeft;
                    hovered_pos = Some(hovered_pos.unwrap() - vec2(size.x / 2.0, 0.0));
                } else if self.mouse_pos.x > max.x - threshold {
                    manipulation_type = ManipulationType::ResizeRight;
                    hovered_pos = Some(hovered_pos.unwrap() + vec2(size.x / 2.0, 0.0));
                } else if self.mouse_pos.y < min.y + threshold {
                    manipulation_type = ManipulationType::ResizeBottom;
                    hovered_pos = Some(hovered_pos.unwrap() + vec2(0.0, size.y / 2.0));
                } else if self.mouse_pos.y > max.y - threshold {
                    manipulation_type = ManipulationType::ResizeTop;
                    hovered_pos = Some(hovered_pos.unwrap() - vec2(0.0, size.y / 2.0));
                }
            }
        }

        // If dragging use drag_data
        if let Some(drag_data) = &self.edit_mode.drag_data {
            hovered_id = Some(drag_data.id);
            hovered_type = Some(drag_data.object_type);
            manipulation_type = drag_data.manipulation_type;
            hovered_pos = Some(drag_data.object_start_pos);
            hovered_bounds = Some(drag_data.bounds);
        }

        // Cursor for hovered
        if hovered_id.is_some()
            && (hovered_type == Some(ObjectType::Furniture)
                || self.edit_mode.selected_room.is_some())
        {
            match manipulation_type {
                ManipulationType::Move => ctx.set_cursor_icon(CursorIcon::PointingHand),
                ManipulationType::ResizeLeft | ManipulationType::ResizeRight => {
                    ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
                }
                ManipulationType::ResizeTop | ManipulationType::ResizeBottom => {
                    ctx.set_cursor_icon(CursorIcon::ResizeVertical);
                }
            }
        }

        let can_drag = self.edit_mode.selected_room.is_some()
            || matches!(hovered_type, Some(ObjectType::Furniture));
        if let Some(hovered_id) = hovered_id {
            if response.drag_started_by(egui::PointerButton::Primary) && can_drag {
                self.edit_mode.drag_data = Some(DragData {
                    id: hovered_id,
                    object_type: hovered_type.unwrap(),
                    manipulation_type,
                    mouse_start_pos: self.mouse_pos_world,
                    object_start_pos: hovered_pos.unwrap(),
                    bounds: hovered_bounds.unwrap(),
                });
            }
        }
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
                let start_size = drag_data.bounds.1 - drag_data.bounds.0;
                let sign = drag_data.manipulation_type.sign();
                for room in &mut self.layout.rooms {
                    if drag_data.id == room.id {
                        match drag_data.manipulation_type {
                            ManipulationType::Move => {
                                room.pos = new_pos;
                            }
                            ManipulationType::ResizeLeft | ManipulationType::ResizeRight => {
                                room.size.x = start_size.x + delta.x * sign;
                                room.pos.x = new_pos.x - (room.size.x / 2.0) * sign;
                            }
                            ManipulationType::ResizeTop | ManipulationType::ResizeBottom => {
                                room.size.y = start_size.y + delta.y * sign;
                                room.pos.y = new_pos.y - (room.size.y / 2.0) * sign;
                            }
                        }
                    } else {
                        for operation in &mut room.operations {
                            if operation.id == drag_data.id {
                                let new_pos = new_pos - room.pos;
                                match drag_data.manipulation_type {
                                    ManipulationType::Move => {
                                        operation.pos = new_pos;
                                    }
                                    ManipulationType::ResizeLeft
                                    | ManipulationType::ResizeRight => {
                                        operation.size.x = start_size.x + delta.x * sign;
                                        operation.pos.x =
                                            new_pos.x - (operation.size.x / 2.0) * sign;
                                    }
                                    ManipulationType::ResizeTop
                                    | ManipulationType::ResizeBottom => {
                                        operation.size.y = start_size.y + delta.y * sign;
                                        operation.pos.y =
                                            new_pos.y - (operation.size.y / 2.0) * sign;
                                    }
                                }
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
                        match drag_data.manipulation_type {
                            ManipulationType::Move => {
                                furniture.pos = new_pos;
                            }
                            ManipulationType::ResizeLeft | ManipulationType::ResizeRight => {
                                furniture.size.x = start_size.x + delta.x * sign;
                                furniture.pos.x = new_pos.x - (furniture.size.x / 2.0) * sign;
                            }
                            ManipulationType::ResizeTop | ManipulationType::ResizeBottom => {
                                furniture.size.y = start_size.y + delta.y * sign;
                                furniture.pos.y = new_pos.y - (furniture.size.y / 2.0) * sign;
                            }
                        }
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
            hovered_id,
            room_selected: self.edit_mode.selected_room,
            snap_line_x,
            snap_line_y,
        }
    }

    fn handle_drag(
        &self,
        drag_data: &DragData,
        snap: bool,
    ) -> (Vec2, f64, Option<f64>, Option<f64>) {
        let mut snap_line_x = None;
        let mut snap_line_y = None;

        let delta = self.mouse_pos_world - drag_data.mouse_start_pos;
        let mut new_pos = drag_data.object_start_pos + vec2(delta.x, delta.y);
        let mut new_rotation = 0.0;

        if snap && drag_data.object_type == ObjectType::Opening {
            // Find the room the object is part of
            let mut found_room = None;
            for room in &self.layout.rooms {
                for opening in &room.openings {
                    if opening.id == drag_data.id {
                        found_room = Some(room);
                        break;
                    }
                }
            }
            if let Some(room) = found_room {
                let mut closest_distance = f64::INFINITY;
                let mut closest_point = None;
                let mut closest_rotation = None;

                for poly in &room.rendered_data.as_ref().unwrap().polygons {
                    let points: Vec<_> = poly.exterior().points().collect();

                    // Iterate over pairs of consecutive points to represent the edges of the polygon
                    for i in 0..points.len() {
                        let p1 = coord_to_vec2(points[i]);
                        let p2 = coord_to_vec2(points[(i + 1) % points.len()]);

                        // Calculate the closest point on the line segment from p1 to p2 to new_pos
                        let line_vec = p2 - p1;
                        let t = ((new_pos - p1).dot(line_vec)) / line_vec.length_squared();
                        let t_clamped = t.clamp(0.0, 1.0); // Clamp t to the [0, 1] interval to stay within the segment
                        let closest_point_on_segment = p1 + line_vec * t_clamped;

                        // Calculate the distance from new_pos to this closest point on the segment
                        let distance = (closest_point_on_segment - new_pos).length();
                        if distance < closest_distance {
                            closest_distance = distance;
                            closest_point = Some(closest_point_on_segment);
                            closest_rotation = Some(-line_vec.y.atan2(line_vec.x).to_degrees());
                        }
                    }
                }

                let snap_threshold = 0.25;
                if closest_distance < snap_threshold {
                    new_pos = closest_point.unwrap();
                    new_rotation = closest_rotation.unwrap();
                }
            }
        } else if snap
            && matches!(
                drag_data.object_type,
                ObjectType::Room | ObjectType::Operation
            )
        {
            // Snap to other rooms
            let mut closest_horizontal_snap_line = None;
            let mut closest_vertical_snap_line: Option<(f64, f64, usize)> = None;
            // If its a resize operation or object type is opening, use ZERO bounds
            let (bounds_min, bounds_max) =
                if !matches!(drag_data.manipulation_type, ManipulationType::Move)
                    || drag_data.object_type == ObjectType::Opening
                {
                    match drag_data.manipulation_type {
                        ManipulationType::ResizeLeft | ManipulationType::ResizeRight => (
                            vec2(0.0, drag_data.bounds.0.y),
                            vec2(0.0, drag_data.bounds.1.y),
                        ),
                        ManipulationType::ResizeTop | ManipulationType::ResizeBottom => (
                            vec2(drag_data.bounds.0.x, 0.0),
                            vec2(drag_data.bounds.1.x, 0.0),
                        ),
                        ManipulationType::Move => (Vec2::ZERO, Vec2::ZERO),
                    }
                } else {
                    drag_data.bounds
                };
            let (bounds_min, bounds_max) = (bounds_min + new_pos, bounds_max + new_pos);
            let snap_threshold = 0.1;

            for other_room in &self.layout.rooms {
                if other_room.id != drag_data.id {
                    let (other_min, other_max) = other_room.bounds();
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
                snap_line_x = Some(snap_line);
                if edge == 0 {
                    snap_line + (bounds_max.y - bounds_min.y) / 2.0
                } else {
                    snap_line - (bounds_max.y - bounds_min.y) / 2.0
                }
            } else if drag_data.object_type.snap_to_grid() {
                // Snap to grid
                (new_pos.y * 10.0).round() / 10.0
            } else {
                new_pos.y
            };
            new_pos.x = if let Some((snap_line, _, edge)) = closest_vertical_snap_line {
                // Snap to other room
                snap_line_y = Some(snap_line);
                if edge == 0 {
                    snap_line + (bounds_max.x - bounds_min.x) / 2.0
                } else {
                    snap_line - (bounds_max.x - bounds_min.x) / 2.0
                }
            } else if drag_data.object_type.snap_to_grid() {
                // Snap to grid
                (new_pos.x * 10.0).round() / 10.0
            } else {
                new_pos.x
            };
        } else if drag_data.object_type.snap_to_grid() {
            // Snap to grid
            new_pos.x = (new_pos.x * 10.0).round() / 10.0;
            new_pos.y = (new_pos.y * 10.0).round() / 10.0;
        }

        (new_pos, new_rotation, snap_line_x, snap_line_y)
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
                    ui.label("Double click to select room, escape to deselect");
                    ui.label("Shift to disable snap");
                    ui.horizontal(|ui| {
                        ui.add_space(ui.available_width() / 4.0);
                        if ui.button("Add Room").clicked() {
                            self.layout.rooms.push(Room::new(
                                "New Room",
                                Vec2::ZERO,
                                vec2(1.0, 1.0),
                                RenderOptions::default(),
                                Walls::WALL,
                                vec![],
                                vec![],
                            ));
                        }
                        if ui.button("Add Furniture").clicked() {
                            self.layout.furniture.push(Furniture::new(
                                Vec2::ZERO,
                                vec2(1.0, 1.0),
                                0.0,
                            ));
                        }
                        ui.add_space(ui.available_width() / 4.0);
                    });
                });
            });

        // Get hovered room or selected room if there isn't one
        if let Some(room) = [edit_response.hovered_id, self.edit_mode.selected_room]
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
            let selected = edit_response.hovered_id == Some(furniture.id);
            let vertices =
                Shape::Rectangle.vertices(furniture.pos, furniture.size, furniture.rotation);
            let points = vertices
                .iter()
                .map(|v| vec2_to_egui_pos(self.world_to_pixels(v.x, v.y)))
                .collect::<Vec<_>>();
            let stroke = Stroke::new(
                if selected { 3.0 } else { 1.0 },
                Color32::from_rgb(255, 255, 0).gamma_multiply(0.8),
            );
            painter.add(EShape::closed_line(points, stroke));
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
                .resizable(false)
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
        .num_columns(4)
        .spacing([20.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label("Position");
            edit_vec2(ui, &mut room.pos, 0.1, 2);
            ui.label("Size");
            edit_vec2(ui, &mut room.size, 0.1, 2);
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
                    ui.label(format!("{wall_side} Wall"));
                    ui.checkbox(is_wall, "");
                });
            }
            ui.end_row();
        });
    render_options_widgets(
        ui,
        &mut room.render_options,
        format!("Materials {}", room.id),
    );

    ui.horizontal(|ui| {
        let mut show_outline = room.outline.is_some();
        if ui
            .add(Checkbox::new(&mut show_outline, "Show Outline"))
            .changed()
        {
            if show_outline {
                room.outline = Some(Outline::default());
            } else {
                room.outline = None;
            }
        }
        if let Some(outline) = &mut room.outline {
            ui.label("Thickness");
            ui.add(
                DragValue::new(&mut outline.thickness)
                    .speed(0.1)
                    .fixed_decimals(2)
                    .clamp_range(0.01..=5.0)
                    .suffix("m"),
            );
            ui.label("Color");
            ui.color_edit_button_srgba(&mut outline.color);
        }
    });

    ui.separator();

    CollapsingState::load_with_default_open(
        ui.ctx(),
        ui.make_persistent_id("operations_collapsing_header"),
        false,
    )
    .show_header(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label("Operations");
            if ui.add(Button::new("Add")).clicked() {
                room.operations.push(Operation::new(
                    Action::Add,
                    Shape::Rectangle,
                    Vec2::ZERO,
                    vec2(1.0, 1.0),
                ));
            }
        });
    })
    .body(|ui| {
        let mut operations_to_remove = vec![];
        let mut operations_to_raise = vec![];
        let mut operations_to_lower = vec![];
        let num_operations = room.operations.len();
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
                    ui.label(format!("{index}"));
                    combo_box_for_enum(ui, format!("Operation {index}"), &mut operation.action, "");
                    combo_box_for_enum(ui, format!("Shape {index}"), &mut operation.shape, "");

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
                    edit_vec2(ui, &mut operation.pos, 0.1, 2);
                    ui.label("Size");
                    edit_vec2(ui, &mut operation.size, 0.1, 2);
                    ui.label("Rotation");
                    if ui
                        .add(
                            DragValue::new(&mut operation.rotation)
                                .speed(5)
                                .fixed_decimals(0)
                                .suffix("°"),
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
            });
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

    CollapsingState::load_with_default_open(
        ui.ctx(),
        ui.make_persistent_id("openings_collapsing_header"),
        false,
    )
    .show_header(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label("Openings");
            if ui.add(Button::new("Add")).clicked() {
                room.openings
                    .push(Opening::new(OpeningType::Door, Vec2::ZERO));
            }
        });
    })
    .body(|ui| {
        let mut openings_to_remove = vec![];
        let mut openings_to_raise = vec![];
        let mut openings_to_lower = vec![];
        let num_openings = room.openings.len();
        for (index, opening) in room.openings.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                combo_box_for_enum(
                    ui,
                    format!("Opening {}", opening.id),
                    &mut opening.opening_type,
                    "",
                );
                ui.label("Pos");
                edit_vec2(ui, &mut opening.pos, 0.1, 2);
                ui.label("Rotation");
                if ui
                    .add(
                        DragValue::new(&mut opening.rotation)
                            .speed(5)
                            .fixed_decimals(0)
                            .suffix("°"),
                    )
                    .changed()
                {
                    opening.rotation = opening.rotation.rem_euclid(360.0);
                }
                ui.label("Width");
                ui.add(
                    DragValue::new(&mut opening.width)
                        .speed(0.1)
                        .fixed_decimals(1)
                        .clamp_range(0.1..=5.0)
                        .suffix("m"),
                );
                if ui.add(Button::new("Delete")).clicked() {
                    openings_to_remove.push(opening.id);
                }
                if index > 0 && ui.add(Button::new("^")).clicked() {
                    openings_to_raise.push(index);
                }
                if index < num_openings - 1 && ui.add(Button::new("v")).clicked() {
                    openings_to_lower.push(index);
                }
            });
        }
        for id in openings_to_remove {
            room.openings.retain(|o| o.id != id);
        }
        for index in openings_to_raise {
            if index > 0 {
                room.openings.swap(index, index - 1);
            }
        }
        for index in openings_to_lower {
            if index < room.openings.len() - 1 {
                room.openings.swap(index, index + 1);
            }
        }
    });

    alter_room
}

fn render_options_widgets(ui: &mut egui::Ui, render_options: &mut RenderOptions, id: String) {
    ui.horizontal(|ui| {
        combo_box_for_enum(ui, id, &mut render_options.material, "");

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

// Helper function to edit Vec2 using two DragValue widgets
fn edit_vec2(ui: &mut egui::Ui, vec2: &mut Vec2, speed: f32, fixed_decimals: usize) {
    ui.horizontal(|ui| {
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
