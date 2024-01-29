use super::{layout, HomeFlow};
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Rounding, Shape, Stroke, Vec2};

#[derive(Default)]
pub struct EditDetails {
    pub enabled: bool,
    pub selected_room: Option<String>,
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
                    let delta = response.drag_delta() * 0.01 / (self.zoom / 100.0);
                    room.pos = room.pos + layout::Vec2::new(delta.x, -delta.y);
                    room.render = None;
                }
                if response.drag_released() {
                    room.pos = layout::Vec2::new(
                        (room.pos.x * 10.0).round() / 10.0,
                        (room.pos.y * 10.0).round() / 10.0,
                    );
                    room.render = None;
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
        // Top layer render
        for room in &self.layout.rooms {
            if let Some(room_name) = &edit_response.room_hovered {
                let room = self
                    .layout
                    .rooms
                    .iter()
                    .find(|r| &r.name == room_name)
                    .unwrap();
                let room_render = room.render.as_ref().unwrap();
                let points = room_render
                    .vertices
                    .iter()
                    .map(|v| self.world_to_pixels(canvas_center, v.x, v.y))
                    .collect::<Vec<_>>();
                painter.add(Shape::closed_line(
                    points,
                    Stroke::new(10.0, Color32::from_rgb(255, 255, 255)),
                ));
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
