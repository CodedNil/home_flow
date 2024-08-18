use super::HomeFlow;
use crate::common::{furniture::FurnitureType, utils::rotate_point_i32};
use egui::{Color32, Painter, Stroke};
use glam::dvec2 as vec2;

impl HomeFlow {
    pub fn render_presence_sensors(&self, painter: &Painter) {
        let mut presence_points = self.presence_points.clone();

        // If point is near a chair, snap it to the chair
        let mut chair_positions = Vec::new();
        for room in &self.layout.rooms {
            for furniture in &room.furniture {
                if matches!(furniture.furniture_type, FurnitureType::Chair(_)) {
                    chair_positions.push(room.pos + furniture.pos);
                }
                let rendered_data = furniture.rendered_data.as_ref().unwrap();
                for child in &rendered_data.children {
                    if matches!(child.furniture_type, FurnitureType::Chair(_)) {
                        let hover = child.hover_amount.max(0.0);
                        let pos = room.pos
                            + furniture.pos
                            + rotate_point_i32(child.pos, -furniture.rotation)
                            + rotate_point_i32(
                                vec2(hover * 0.15, hover * 0.3),
                                -(furniture.rotation + child.rotation),
                            );
                        chair_positions.push(pos);
                    }
                }
            }
        }
        for point in &mut presence_points {
            for chair_pos in &chair_positions {
                if (*point - *chair_pos).length() < 0.4 {
                    *point = *chair_pos;
                }
            }
        }

        // Render presence points
        for point in presence_points {
            painter.circle(
                self.world_to_screen_pos(point),
                0.1 * self.stored.zoom as f32,
                Color32::from_rgb(0, 240, 140).gamma_multiply(0.5),
                Stroke::new(
                    0.02 * self.stored.zoom as f32,
                    Color32::from_rgb(0, 200, 100).gamma_multiply(0.7),
                ),
            );
        }
    }
}
