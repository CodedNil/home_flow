use super::{layout, HomeFlow};
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Rounding, Shape, Stroke, Vec2};

#[derive(Default)]
pub struct EditDetails {
    pub enabled: bool,
    pub selected_room: Option<String>,
    pub dragging_room: Option<DragData>,
}

pub struct DragData {
    pub room_name: String,
    pub mouse_start_pos: Pos2,
    pub room_start_pos: layout::Vec2,
}

pub struct EditResponse {
    pub used_dragged: bool,
    pub room_hovered: Option<String>,
}

impl HomeFlow {
    pub fn run_edit_mode(
        &mut self,
        response: &egui::Response,
        mouse_pos_world: Pos2,
    ) -> EditResponse {
        if !self.edit_mode.enabled {
            return EditResponse {
                used_dragged: false,
                room_hovered: None,
            };
        }

        let mut used_dragged = false;

        let mut room_hovered = None;
        for room in &self.layout.rooms {
            if room.contains(mouse_pos_world.x, mouse_pos_world.y) {
                room_hovered = Some(room.name.clone());
            }
        }

        // Select room
        if response.double_clicked() {
            if let Some(room_hovered) = &room_hovered {
                self.edit_mode.selected_room = Some(room_hovered.clone());
            } else {
                self.edit_mode.selected_room = None;
            }
        }

        // Apply edit changes
        if let Some(room_name) = &room_hovered {
            let room = self
                .layout
                .rooms
                .iter_mut()
                .find(|r| &r.name == room_name)
                .unwrap();
            if self.edit_mode.selected_room == Some(room.name.clone()) {
                used_dragged = true;
                if response.dragged() {
                    if self.edit_mode.dragging_room.is_none() {
                        self.edit_mode.dragging_room = Some(DragData {
                            room_name: room.name.clone(),
                            mouse_start_pos: mouse_pos_world,
                            room_start_pos: room.pos,
                        });
                    }
                    let drag_data = self.edit_mode.dragging_room.as_ref().unwrap();

                    let delta = mouse_pos_world - drag_data.mouse_start_pos;
                    let new_pos = drag_data.room_start_pos + layout::Vec2::new(delta.x, delta.y);

                    // Snap to grid
                    let new_pos = layout::Vec2::new(
                        (new_pos.x * 10.0).round() / 10.0,
                        (new_pos.y * 10.0).round() / 10.0,
                    );
                    room.pos = new_pos;
                    // room.render = None;
                }
                if response.drag_released() {
                    room.pos = layout::Vec2::new(
                        (room.pos.x * 10.0).round() / 10.0,
                        (room.pos.y * 10.0).round() / 10.0,
                    );
                    // room.render = None;
                }
            }
        }

        EditResponse {
            used_dragged,
            room_hovered,
        }
    }

    pub fn paint_edit_mode(
        &self,
        painter: &Painter,
        canvas_center: Pos2,
        edit_response: &EditResponse,
    ) {
        for room in &self.layout.rooms {
            if let Some(room_name) = &edit_response.room_hovered {
                let room = self
                    .layout
                    .rooms
                    .iter()
                    .find(|r| &r.name == room_name)
                    .unwrap();
                let room_render = room.render.as_ref().unwrap();

                // Render outline
                let (bounds_min, bounds_max) = room.bounds();
                let room_center = (bounds_min + bounds_max) / 2.0;
                let render_offset = room_center - room_render.center;

                let points = room_render
                    .vertices
                    .iter()
                    .map(|v| {
                        self.world_to_pixels(
                            canvas_center,
                            v.x + render_offset.x,
                            v.y + render_offset.y,
                        )
                    })
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
                            layout::Action::Add => Color32::from_rgb(50, 200, 50),
                            layout::Action::Subtract => Color32::from_rgb(200, 50, 50),
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

            let mut text_lines = vec![
                room.name.to_string(),
                format!("{:.1}m x {:.1}m", room.size.x, room.size.y),
            ];
            if self.edit_mode.selected_room == Some(room.name.clone()) {
                text_lines.push("Selected".to_string());
            }
            paint_text_box(
                painter,
                &text_lines,
                14.0,
                self.world_to_pixels(canvas_center, room.pos.x, room.pos.y),
            );
        }
    }
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

    painter.add(Shape::dashed_line_with_offset(
        &points,
        stroke,
        &[dash_length],
        &[gap_length],
        offset,
    ));
}

fn paint_text_box(painter: &Painter, text_lines: &Vec<String>, font_size: f32, pos: Pos2) {
    let font_id = FontId::monospace(font_size);
    let mut text_positions = Vec::new();
    let mut full_rect = Rect::from_min_size(pos, Vec2::ZERO);
    for (index, text) in text_lines.iter().enumerate() {
        let pos = pos
            + Vec2::new(
                0.0,
                (index as f32 - text_lines.len() as f32 / 2.0) * font_size * 1.2,
            );
        text_positions.push(pos);
        let galley = painter.layout_no_wrap(text.to_string(), font_id.clone(), Color32::WHITE);
        let rect = Align2::CENTER_CENTER.anchor_rect(Rect::from_min_size(pos, galley.size()));
        full_rect = full_rect.union(rect);
    }
    painter.rect_filled(
        full_rect.expand(4.0),
        Rounding::same(8.0),
        Color32::DARK_GRAY,
    );
    for (index, text) in text_lines.iter().enumerate() {
        painter.text(
            text_positions[index],
            Align2::CENTER_CENTER,
            text,
            font_id.clone(),
            Color32::WHITE,
        );
    }
}
