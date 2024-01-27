use super::{
    layout::{self, Vec2, Wall},
    HomeFlow,
};
use egui::{epaint::Vertex, Color32, Mesh, Painter, Pos2, Shape, Stroke, TextureId};

impl HomeFlow {
    pub fn render_walls(&self, painter: &Painter, canvas_center: Pos2) {
        // Get camera (center screen) position in world coordinates
        let camera_pos = self.pixels_to_world(canvas_center, canvas_center.x, canvas_center.y);
        let camera_pos_2d = Vec2::new(camera_pos.x, camera_pos.y);

        // Get all walls
        let mut walls = Vec::new();
        for wall in &self.layout.walls {
            if !walls.iter().any(|w: &Wall| wall.is_mirrored_equal(w)) {
                walls.push(wall.clone());
            }
        }
        for room in &self.layout.rooms {
            for wall in &room.walls {
                if !walls.iter().any(|w: &Wall| wall.is_mirrored_equal(w)) {
                    walls.push(wall.clone());
                }
            }
        }

        // Render walls with faux 3D effect
        let mut wall_render = Vec::new();
        let mut faux_wall_render = Vec::new();
        let wall_distance_scale = 5.0;
        let wall_distance_factor = 0.2;
        for wall in walls {
            let wall_width = match wall.wall_type {
                layout::WallType::Interior => 0.1,
                layout::WallType::Exterior => 0.2,
            };
            let wall_width_half = wall_width / 2.0;
            let wall_center = (wall.start + wall.end) / 2.0;

            // Calculate direction vector of the wall and normalize it
            let direction = wall.end - wall.start;
            let length = direction.x.hypot(direction.y);
            let normalized_direction = if length == 0.0 {
                direction
            } else {
                direction / length
            };

            // Rotate the direction vector by 90 degrees
            let rotated_direction = Vec2::new(-normalized_direction.y, normalized_direction.x);

            // Calculate the distance from the camera to the wall
            let norm_dist_x = ((camera_pos_2d - wall_center).dot(&rotated_direction)
                / wall_distance_scale)
                .clamp(-1.0, 1.0)
                * wall_distance_factor;
            let norm_dist_start_y = ((camera_pos_2d - wall.start).dot(&normalized_direction)
                / wall_distance_scale)
                .clamp(-1.0, 1.0)
                * wall_distance_factor;
            let norm_dist_end_y = ((camera_pos_2d - wall.end).dot(&normalized_direction)
                / wall_distance_scale)
                .clamp(-1.0, 1.0)
                * wall_distance_factor;

            // Draw the main wall line
            let segment_start = wall.start
                + rotated_direction * -norm_dist_x
                + normalized_direction * (-norm_dist_start_y - wall_width_half);
            let segment_end = wall.end
                + rotated_direction * -norm_dist_x
                + normalized_direction * (-norm_dist_end_y + wall_width_half);
            wall_render.push((
                [
                    self.world_to_pixels(canvas_center, segment_start.x, segment_start.y),
                    self.world_to_pixels(canvas_center, segment_end.x, segment_end.y),
                ],
                Stroke::new(wall_width * self.zoom, Color32::from_rgb(130, 80, 20)),
            ));

            // Calculate the light intensity based on the wall direction and light direction
            let light_direction = Vec2::new(1.0, 1.0).normalize();
            let light_intensity = normalized_direction.dot(&light_direction).clamp(0.0, 1.0);
            let shaded_color_multiplier = 0.5 + 0.5 * light_intensity;
            let offset_color = Color32::from_rgb(
                (200.0 * shaded_color_multiplier) as u8,
                (140.0 * shaded_color_multiplier) as u8,
                (50.0 * shaded_color_multiplier) as u8,
            );

            // Calculate triangles for the faux 3D effect
            let camera_on_left = if norm_dist_x < 0.0 { -0.95 } else { 0.95 };
            let offset_start = rotated_direction * (wall_width_half * camera_on_left);
            let offset_end = rotated_direction * (wall_width_half * camera_on_left);
            faux_wall_render.push((
                wall.start + offset_start + normalized_direction * wall_width_half,
                segment_start + offset_start + normalized_direction * wall_width,
                wall.end + offset_end + normalized_direction * -wall_width_half,
                segment_end + offset_end - normalized_direction * wall_width,
                offset_color,
            ));
            // Edges of the wall
            faux_wall_render.push((
                wall.start + offset_start + normalized_direction * wall_width_half,
                wall.start + offset_start + normalized_direction * -wall_width_half,
                segment_start + offset_start + normalized_direction * wall_width,
                segment_start + offset_start,
                offset_color,
            ));
            faux_wall_render.push((
                segment_end + offset_end - normalized_direction * wall_width,
                segment_end + offset_end,
                wall.end + offset_end + normalized_direction * -wall_width_half,
                wall.end + offset_end + normalized_direction * wall_width_half,
                offset_color,
            ));
        }
        // Sort faux walls by distance to camera
        faux_wall_render.sort_by(|a, b| {
            let dist_a = (camera_pos_2d - (a.0 + a.2) / 2.0).length();
            let dist_b = (camera_pos_2d - (b.0 + b.2) / 2.0).length();
            dist_b.partial_cmp(&dist_a).unwrap()
        });
        for (p1, p2, p3, p4, color) in faux_wall_render {
            let indices = vec![0, 1, 2, 2, 1, 3];
            let mut vertices = Vec::with_capacity(4);
            for point in &[p1, p2, p3, p4] {
                vertices.push(Vertex {
                    pos: self.world_to_pixels(canvas_center, point.x, point.y),
                    uv: Pos2::default(),
                    color,
                });
            }
            let mesh = Mesh {
                indices,
                vertices,
                texture_id: TextureId::Managed(0),
            };
            painter.add(Shape::mesh(mesh));
        }
        // TODO Any walls that share a start or end, connect into a line segment for better rendering with Shape::closed_line
        for wall in wall_render {
            painter.line_segment(wall.0, wall.1);
        }
    }
}
