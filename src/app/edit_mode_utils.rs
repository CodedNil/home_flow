use super::{
    edit_mode::{DragData, ManipulationType, ObjectType},
    HomeFlow,
};
use crate::common::{layout::Shape, shape::coord_to_vec2};
use egui::{CursorIcon, Key, Ui};
use glam::{dvec2 as vec2, DVec2 as Vec2};
use uuid::Uuid;

pub struct HoverDetails {
    pub id: Uuid,
    pub object_type: ObjectType,
    pub pos: Vec2,
    pub bounds: (Vec2, Vec2),
    pub manipulation_type: ManipulationType,
    pub can_drag: bool,
}

impl HomeFlow {
    pub fn hover_select(&mut self, response: &egui::Response, ui: &Ui) -> Option<HoverDetails> {
        let mut hovered_id = None;
        let mut hovered_type = None;
        let mut hovered_pos = None;
        let mut hovered_bounds = None;

        if self.edit_mode.selected_type == Some(ObjectType::Room) {
            // Selected room hover
            let selected_id = self.edit_mode.selected_id.unwrap();
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
                    hovered_bounds = Some((-furniture.size / 2.0, furniture.size / 2.0));
                }
            }
        }
        // Double click to select room
        if response.clicked() {
            let mut best_selected = None;
            let mut best_type = None;
            for room in &self.layout.rooms {
                if room.contains(self.mouse_pos_world) {
                    best_selected = Some(room.id);
                    best_type = Some(ObjectType::Room);
                }
            }
            for furniture in &self.layout.furniture {
                if Shape::Rectangle.contains(
                    self.mouse_pos_world,
                    furniture.pos,
                    furniture.size,
                    furniture.rotation,
                ) {
                    best_selected = Some(furniture.id);
                    best_type = Some(ObjectType::Furniture);
                }
            }
            self.edit_mode.selected_id = best_selected;
            self.edit_mode.selected_type = best_type;
            self.edit_mode.drag_data = None;
        }
        // Escape to deselect room
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            self.edit_mode.selected_id = None;
            self.edit_mode.selected_type = None;
            self.edit_mode.drag_data = None;
        }

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
                if (self.mouse_pos.x - min.x).abs() < threshold {
                    manipulation_type = ManipulationType::ResizeLeft;
                    hovered_pos = Some(hovered_pos.unwrap() - vec2(size.x / 2.0, 0.0));
                } else if (self.mouse_pos.x - max.x).abs() < threshold {
                    manipulation_type = ManipulationType::ResizeRight;
                    hovered_pos = Some(hovered_pos.unwrap() + vec2(size.x / 2.0, 0.0));
                } else if (self.mouse_pos.y - min.y).abs() < threshold {
                    manipulation_type = ManipulationType::ResizeBottom;
                    hovered_pos = Some(hovered_pos.unwrap() + vec2(0.0, size.y / 2.0));
                } else if (self.mouse_pos.y - max.y).abs() < threshold {
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
        let can_drag = self.edit_mode.selected_id.is_some()
            || matches!(hovered_type, Some(ObjectType::Furniture));
        if hovered_id.is_some() && can_drag {
            match manipulation_type {
                ManipulationType::Move => ui.ctx().set_cursor_icon(CursorIcon::PointingHand),
                ManipulationType::ResizeLeft | ManipulationType::ResizeRight => {
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
                }
                ManipulationType::ResizeTop | ManipulationType::ResizeBottom => {
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
                }
            }
        }

        hovered_id.map(|id| HoverDetails {
            id,
            object_type: hovered_type.unwrap(),
            pos: hovered_pos.unwrap(),
            bounds: hovered_bounds.unwrap(),
            manipulation_type,
            can_drag,
        })
    }

    pub fn handle_drag(
        &self,
        drag_data: &DragData,
        snap: bool,
    ) -> (Vec2, f64, Option<f64>, Option<f64>) {
        let mut snap_line_x = None;
        let mut snap_line_y = None;

        let delta = self.mouse_pos_world - drag_data.mouse_start_pos;
        let mut new_pos = drag_data.object_start_pos + vec2(delta.x, delta.y);
        let mut new_rotation = 0.0;

        let snap_amount = match drag_data.object_type {
            ObjectType::Room | ObjectType::Operation => 10.0,
            _ => 20.0,
        };
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
                    let operation_exists = other_room
                        .operations
                        .iter()
                        .any(|ops| ops.id == drag_data.id);
                    let (other_min, other_max) = if operation_exists {
                        other_room.self_bounds()
                    } else {
                        other_room.bounds()
                    };
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
            } else {
                // Snap to grid
                (new_pos.y * snap_amount).round() / snap_amount
            };
            new_pos.x = if let Some((snap_line, _, edge)) = closest_vertical_snap_line {
                // Snap to other room
                snap_line_y = Some(snap_line);
                if edge == 0 {
                    snap_line + (bounds_max.x - bounds_min.x) / 2.0
                } else {
                    snap_line - (bounds_max.x - bounds_min.x) / 2.0
                }
            } else {
                // Snap to grid
                (new_pos.x * snap_amount).round() / snap_amount
            };
        } else {
            // Snap to grid
            new_pos.x = (new_pos.x * snap_amount).round() / snap_amount;
            new_pos.y = (new_pos.y * snap_amount).round() / snap_amount;
        }

        (new_pos, new_rotation, snap_line_x, snap_line_y)
    }
}

pub fn apply_standard_transform(
    pos: &mut Vec2,
    size: &mut Vec2,
    drag_data: &DragData,
    delta: Vec2,
    new_pos: Vec2,
) {
    let start_size = drag_data.bounds.1 - drag_data.bounds.0;
    let sign = drag_data.manipulation_type.sign();
    match drag_data.manipulation_type {
        ManipulationType::Move => {
            *pos = new_pos;
        }
        ManipulationType::ResizeLeft | ManipulationType::ResizeRight => {
            let new_size = start_size.x + delta.x * sign;
            size.x = new_size.abs();
            pos.x = new_pos.x - new_size / 2.0 * sign;
        }
        ManipulationType::ResizeTop | ManipulationType::ResizeBottom => {
            let new_size = start_size.y + delta.y * sign;
            size.y = new_size.abs();
            pos.y = new_pos.y - new_size / 2.0 * sign;
        }
    }
}
