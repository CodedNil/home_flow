use super::{
    edit_mode::{DragData, ManipulationType, ObjectType},
    HomeFlow,
};
use crate::common::{
    layout::GlobalMaterial,
    shape::coord_to_vec2,
    utils::{rotate_point, RoundFactor},
};
use egui::{ComboBox, DragValue, Key, Ui};
use glam::{dvec2 as vec2, DVec2 as Vec2};
use strum::IntoEnumIterator;
use uuid::Uuid;

#[derive(Debug)]
pub struct HoverDetails {
    pub id: Uuid,
    pub object_type: ObjectType,
    pub can_drag: bool,
    pub pos: Vec2,
    pub size: Vec2,
    pub rotation: f64,
    pub manipulation_type: ManipulationType,
}

impl HomeFlow {
    pub fn hover_select(&mut self, response: &egui::Response, ui: &Ui) -> Option<HoverDetails> {
        // Hover over rooms and furniture
        let mut hovered_data = None;
        for room in self.layout.rooms.iter().rev() {
            if room.contains(self.mouse_pos_world) {
                hovered_data = Some(HoverDetails {
                    id: room.id,
                    object_type: ObjectType::Room,
                    can_drag: true,
                    pos: room.pos,
                    size: room.size,
                    rotation: 0.0,
                    manipulation_type: ManipulationType::Move,
                });
                break;
            }
        }
        for obj in self.layout.furniture.iter().rev() {
            if obj.contains(self.mouse_pos_world) {
                hovered_data = Some(HoverDetails {
                    id: obj.id,
                    object_type: ObjectType::Furniture,
                    can_drag: true,
                    pos: obj.pos,
                    size: obj.size,
                    rotation: obj.rotation,
                    manipulation_type: ManipulationType::Move,
                });
                break;
            }
        }

        // Click to select room
        if response.clicked() {
            self.edit_mode.selected_id = hovered_data.as_ref().map(|d| d.id);
            self.edit_mode.selected_type = hovered_data.as_ref().map(|d| d.object_type);
            self.edit_mode.drag_data = None;
        }

        // If dragging use drag_data
        if let Some(drag_data) = &self.edit_mode.drag_data {
            return Some(HoverDetails {
                id: drag_data.id,
                object_type: drag_data.object_type,
                can_drag: true,
                pos: drag_data.start_pos,
                size: drag_data.start_size,
                rotation: drag_data.start_rotation,
                manipulation_type: drag_data.manipulation_type,
            });
        }

        // Selected room limits hover scope
        if self.edit_mode.selected_type == Some(ObjectType::Room) {
            hovered_data = None;
            let selected_id = self.edit_mode.selected_id.unwrap();
            let room = self.layout.rooms.iter().find(|r| r.id == selected_id);
            if let Some(room) = room {
                if room.contains(self.mouse_pos_world) {
                    hovered_data = Some(HoverDetails {
                        id: room.id,
                        object_type: ObjectType::Room,
                        can_drag: true,
                        pos: room.pos,
                        size: room.size,
                        rotation: 0.0,
                        manipulation_type: ManipulationType::Move,
                    });
                }
                for obj in room.operations.iter().rev() {
                    if obj.contains(room.pos, self.mouse_pos_world) {
                        hovered_data = Some(HoverDetails {
                            id: obj.id,
                            object_type: ObjectType::Operation,
                            can_drag: true,
                            pos: room.pos + obj.pos,
                            size: obj.size,
                            rotation: obj.rotation,
                            manipulation_type: ManipulationType::Move,
                        });
                        break;
                    }
                }
                for obj in room.openings.iter().rev() {
                    if (self.mouse_pos_world - (room.pos + obj.pos)).length() < 0.2 {
                        hovered_data = Some(HoverDetails {
                            id: obj.id,
                            object_type: ObjectType::Opening,
                            can_drag: true,
                            pos: room.pos + obj.pos,
                            size: Vec2::ZERO,
                            rotation: obj.rotation,
                            manipulation_type: ManipulationType::Move,
                        });
                        break;
                    }
                }
            }
        }

        // Escape to deselect object
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            self.edit_mode.selected_id = None;
            self.edit_mode.selected_type = None;
            self.edit_mode.drag_data = None;
        }

        // If room or operation or furniture, check if at the edge of bounds to resize
        if let Some(data) = &mut hovered_data {
            if self.edit_mode.resize_enabled
                && matches!(
                    data.object_type,
                    ObjectType::Room | ObjectType::Operation | ObjectType::Furniture
                )
            {
                // Local mouse pos is -1 to 1 in x and y
                let local_mouse_pos = (rotate_point(self.mouse_pos_world, data.pos, data.rotation)
                    - data.pos)
                    / data.size
                    * 2.0;

                // Calculate the rotated direction vectors for the four directions
                let right_dir = rotate_point(vec2(1.0, 0.0), Vec2::ZERO, -data.rotation);
                let up_dir = rotate_point(vec2(0.0, 1.0), Vec2::ZERO, -data.rotation);
                let screen_size = data.size / 2.0 * self.zoom;

                let threshold = 20.0;

                if (local_mouse_pos.x + 1.0).abs() * screen_size.x < threshold {
                    data.manipulation_type = ManipulationType::ResizeLeft;
                    data.pos -= right_dir * data.size.x / 2.0;
                } else if (local_mouse_pos.x - 1.0).abs() * screen_size.x < threshold {
                    data.manipulation_type = ManipulationType::ResizeRight;
                    data.pos += right_dir * data.size.x / 2.0;
                } else if (local_mouse_pos.y - 1.0).abs() * screen_size.y < threshold {
                    data.manipulation_type = ManipulationType::ResizeTop;
                    data.pos += up_dir * data.size.y / 2.0;
                } else if (local_mouse_pos.y + 1.0).abs() * screen_size.y < threshold {
                    data.manipulation_type = ManipulationType::ResizeBottom;
                    data.pos -= up_dir * data.size.y / 2.0;
                }
            }
        }

        hovered_data
    }

    pub fn handle_drag(
        &self,
        drag_data: &DragData,
        snap: bool,
    ) -> (Vec2, f64, Option<f64>, Option<f64>) {
        let mut snap_line_x = None;
        let mut snap_line_y = None;

        let delta = self.mouse_pos_world - drag_data.mouse_start_pos;
        let mut new_pos = drag_data.start_pos + vec2(delta.x, delta.y);
        let mut new_rotation = 0.0;

        let snap_amount = match drag_data.object_type {
            ObjectType::Room | ObjectType::Operation | ObjectType::Opening => 10.0,
            ObjectType::Furniture => 20.0,
        };
        if drag_data.object_type == ObjectType::Opening {
            if let Some(room) = self
                .layout
                .rooms
                .iter()
                .find(|r| r.openings.iter().any(|o| o.id == drag_data.id))
            {
                let mut closest_distance = f64::MAX;
                let mut closest_point = None;
                let mut closest_rotation = None;

                for poly in &room.rendered_data.as_ref().unwrap().polygons {
                    // Iterate over pairs of consecutive points to represent the edges of the polygon
                    let points: Vec<_> = poly.exterior().points().collect();
                    for i in 0..points.len() {
                        let p1 = coord_to_vec2(points[i]);
                        let p2 = coord_to_vec2(points[(i + 1) % points.len()]);

                        // Calculate the closest point on the line segment from p1 to p2 to new_pos
                        let line_vec = p2 - p1;
                        let t = ((new_pos - p1).dot(line_vec)) / line_vec.length_squared();
                        let closest_point_on_segment = p1 + line_vec * t.clamp(0.0, 1.0);

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

                    // If rotation is 0, 90, 180 or 270 degrees, snap to grid along the line
                    if snap {
                        if new_rotation.abs() < f64::EPSILON
                            || (new_rotation - 180.0).abs() < f64::EPSILON
                        {
                            new_pos.x = new_pos.x.round_factor(snap_amount);
                        } else if (new_rotation - 90.0).abs() < f64::EPSILON
                            || (new_rotation - 270.0).abs() < f64::EPSILON
                        {
                            new_pos.y = new_pos.y.round_factor(snap_amount);
                        }
                    }
                } else if snap {
                    new_pos.x = new_pos.x.round_factor(snap_amount);
                    new_pos.y = new_pos.y.round_factor(snap_amount);
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
            let bounds = match drag_data.manipulation_type {
                ManipulationType::Move => vec2(0.5, 0.5),
                ManipulationType::ResizeLeft | ManipulationType::ResizeRight => vec2(0.0, 0.5),
                ManipulationType::ResizeTop | ManipulationType::ResizeBottom => vec2(0.5, 0.0),
            };
            let (bounds_min, bounds_max) = (
                new_pos - bounds * drag_data.start_size,
                new_pos + bounds * drag_data.start_size,
            );
            let snap_threshold = 0.1;

            for other_room in &self.layout.rooms {
                if other_room.id == drag_data.id {
                    continue;
                }
                let (other_min, other_max) =
                    if other_room.operations.iter().any(|o| o.id == drag_data.id) {
                        other_room.self_bounds()
                    } else {
                        other_room.bounds()
                    };

                for is_vertical in [false, true] {
                    let (bounds, other_bounds, closest_snap_line) = if is_vertical {
                        (
                            [bounds_min.x, bounds_max.x],
                            [other_min.x, other_max.x],
                            &mut closest_vertical_snap_line,
                        )
                    } else {
                        (
                            [bounds_min.y, bounds_max.y],
                            [other_min.y, other_max.y],
                            &mut closest_horizontal_snap_line,
                        )
                    };

                    for (index, &room_edge) in bounds.iter().enumerate() {
                        for &other_edge in &other_bounds {
                            if is_vertical {
                                if !(bounds_min.y < other_max.y + snap_threshold
                                    && bounds_max.y > other_min.y - snap_threshold)
                                {
                                    continue;
                                }
                            } else if !(bounds_min.x < other_max.x + snap_threshold
                                && bounds_max.x > other_min.x - snap_threshold)
                            {
                                continue;
                            }

                            let distance = (room_edge - other_edge).abs();
                            if distance < snap_threshold
                                && closest_snap_line.map_or(true, |(_, dist, _)| distance < dist)
                            {
                                *closest_snap_line = Some((other_edge, distance, index));
                            }
                        }
                    }
                }
            }
            new_pos.y = if let Some((snap_line, _, edge)) = closest_horizontal_snap_line {
                snap_line_x = Some(snap_line);
                snap_line + (bounds_max.y - bounds_min.y) / 2.0 * if edge == 0 { 1.0 } else { -1.0 }
            } else {
                new_pos.y.round_factor(snap_amount)
            };
            new_pos.x = if let Some((snap_line, _, edge)) = closest_vertical_snap_line {
                snap_line_y = Some(snap_line);
                snap_line + (bounds_max.x - bounds_min.x) / 2.0 * if edge == 0 { 1.0 } else { -1.0 }
            } else {
                new_pos.x.round_factor(snap_amount)
            };
        } else {
            new_pos.x = new_pos.x.round_factor(snap_amount);
            new_pos.y = new_pos.y.round_factor(snap_amount);
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
    offset: Vec2,
) {
    let sign = drag_data.manipulation_type.sign();

    let rotated_delta = rotate_point(delta, Vec2::ZERO, drag_data.start_rotation);
    match drag_data.manipulation_type {
        ManipulationType::Move => {
            *pos = new_pos - offset;
        }
        ManipulationType::ResizeLeft | ManipulationType::ResizeRight => {
            let new_size = drag_data.start_size.x + rotated_delta.x * sign;
            size.x = new_size.abs();
            let left_dir = rotate_point(vec2(-1.0, 0.0), Vec2::ZERO, -drag_data.start_rotation);
            *pos = drag_data.start_pos + left_dir * new_size * 0.5 * sign
                - left_dir * rotated_delta.x
                - offset;
        }
        ManipulationType::ResizeTop | ManipulationType::ResizeBottom => {
            let new_size = drag_data.start_size.y + rotated_delta.y * sign;
            size.y = new_size.abs();
            let up_dir = rotate_point(vec2(0.0, -1.0), Vec2::ZERO, -drag_data.start_rotation);
            *pos = drag_data.start_pos + up_dir * new_size * 0.5 * sign
                - up_dir * rotated_delta.y
                - offset;
        }
    }
}

pub fn combo_box_for_enum<T>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    selected: &mut T,
    label: &str,
) where
    T: ToString + PartialEq + Copy + IntoEnumIterator,
{
    ComboBox::from_id_source(id)
        .selected_text(if label.is_empty() {
            selected.to_string()
        } else {
            format!("{}: {}", label, selected.to_string())
        })
        .show_ui(ui, |ui| {
            for variant in T::iter() {
                ui.selectable_value(selected, variant, variant.to_string());
            }
        });
}

pub fn combo_box_for_materials(
    ui: &mut egui::Ui,
    id: Uuid,
    materials: &[GlobalMaterial],
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

pub fn edit_vec2(
    ui: &mut egui::Ui,
    label: &str,
    vec2: &mut Vec2,
    speed: f32,
    fixed_decimals: usize,
) {
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

pub fn edit_rotation(ui: &mut egui::Ui, rotation: &mut f64) {
    labelled_widget(ui, "Rotation", |ui| {
        let widget = ui.add(
            DragValue::new(rotation)
                .speed(5)
                .fixed_decimals(0)
                .suffix("°"),
        );
        if widget.changed() {
            *rotation = rotation.rem_euclid(360.0);
        }
    });
}

pub fn labelled_widget<F>(ui: &mut egui::Ui, label: &str, widget: F)
where
    F: FnOnce(&mut egui::Ui),
{
    ui.horizontal(|ui| {
        ui.label(label);
        widget(ui);
    });
}

pub fn edit_option<T, F, D>(
    ui: &mut egui::Ui,
    label: &str,
    option: &mut Option<T>,
    default: D,
    mut widget: F,
) where
    F: FnMut(&mut egui::Ui, &mut T),
    D: FnOnce() -> T,
{
    let mut checkbox_state = option.is_some();
    let checkbox = ui.add(egui::Checkbox::new(&mut checkbox_state, label));
    if checkbox.changed() {
        *option = if checkbox_state {
            Some(default())
        } else {
            None
        };
    }
    if let Some(content) = option {
        widget(ui, content);
    }
}
