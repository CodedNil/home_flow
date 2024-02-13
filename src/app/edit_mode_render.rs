use super::{edit_mode::EditResponse, vec2_to_egui_pos, HomeFlow};
use crate::common::{
    furniture::Furniture,
    layout::{Action, OpeningType, Room, Shape},
    shape::coord_to_vec2,
};
use egui::{Align2, Color32, Context, Painter, Shape as EShape, Stroke, Window};
use glam::{dvec2 as vec2, DVec2 as Vec2};

impl HomeFlow {
    pub fn paint_edit_mode(
        &mut self,
        painter: &Painter,
        edit_response: &EditResponse,
        ctx: &Context,
    ) {
        if let Some(snap_line_x) = edit_response.snap_line_x {
            let y_level = self.world_to_pixels_xy(0.0, snap_line_x).y;
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
            let x_level = self.world_to_pixels_xy(snap_line_y, 0.0).x;
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
                let points: Vec<Vec2> = poly.exterior().points().map(coord_to_vec2).collect();
                self.closed_dashed_line_with_offset(
                    painter,
                    &points,
                    Stroke::new(6.0, Color32::from_rgba_premultiplied(255, 255, 255, 150)),
                    60.0,
                    self.time * 50.0,
                );
                for interior in poly.interiors() {
                    let points: Vec<Vec2> = interior.points().map(coord_to_vec2).collect();
                    self.closed_dashed_line_with_offset(
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
            let stroke = Stroke::new(3.0, Color32::from_rgb(50, 200, 50).gamma_multiply(0.6));
            self.closed_dashed_line_with_offset(painter, &vertices, stroke, 35.0, self.time * 50.0);

            // Render operations
            for operation in &room.operations {
                let vertices = operation.vertices(room.pos);
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
                self.closed_dashed_line_with_offset(
                    painter,
                    &vertices,
                    stroke,
                    35.0,
                    self.time * 50.0,
                );
            }

            // Render openings
            for opening in &room.openings {
                let selected = edit_response.hovered_id == Some(opening.id);
                let pos = self.world_to_pixels(room.pos + opening.pos);
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
                    .map(|v| vec2_to_egui_pos(self.world_to_pixels(*v)))
                    .collect(),
                Stroke::new(
                    if selected { 8.0 } else { 4.0 },
                    Color32::from_rgb(150, 0, 50).gamma_multiply(0.8),
                ),
            ));
        }
    }

    fn closed_dashed_line_with_offset(
        &self,
        painter: &Painter,
        points: &[Vec2],
        stroke: Stroke,
        desired_combined_length: f64,
        time: f64,
    ) {
        let mut points = points
            .iter()
            .map(|v| self.world_to_pixels(*v))
            .collect::<Vec<_>>();
        points.push(points[0]);

        let mut total_length = 0.0;
        for i in 0..points.len() {
            let next_index = (i + 1) % points.len();
            total_length += points[i].distance(points[next_index]);
        }

        let combined_length = total_length / (total_length / desired_combined_length).round();
        let dash_length = combined_length * 0.6;
        let gap_length = combined_length - dash_length;

        let offset = time % combined_length;
        // Go through points until reaching the offset
        let mut current_length = 0.0;
        for i in 0..points.len() {
            let next_index = (i + 1) % points.len();
            let dist = points[i].distance(points[next_index]);
            if current_length + dist > offset {
                let dir = (points[next_index] - points[i]).normalize();
                points.push(points[i] + dir * (offset - current_length));
                break;
            }
            current_length += dist;
            points.push(points[next_index]);
        }

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
}
