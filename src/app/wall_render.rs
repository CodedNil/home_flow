use super::{
    layout::{self, Vec2, Wall},
    HomeFlow,
};
use egui::{epaint::Vertex, Color32, Mesh, Painter, Pos2, Shape, Stroke, TextureId};

const WALL_COLOR: Color32 = Color32::from_rgb(130, 80, 20);
const WALL_SIDE_COLOR: Color32 = Color32::from_rgb(200, 140, 50);

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
        let walls = walls;

        // Render walls with faux 3D effect
        let wall_distance_scale = 5.0;
        let wall_distance_factor = 0.2;

        let mut wall_render = Vec::with_capacity(walls.len());
        let mut faux_wall_render_primaries = Vec::with_capacity(walls.len());
        let mut faux_wall_render_edges = Vec::with_capacity(walls.len() * 2);
        let mut faux_wall_render_caps = Vec::with_capacity(walls.len() * 2);

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
                self.world_to_pixels(canvas_center, segment_start.x, segment_start.y),
                self.world_to_pixels(canvas_center, segment_end.x, segment_end.y),
                wall_width * self.zoom,
            ));

            // Calculate triangles for the faux 3D effect
            let camera_on_left = if norm_dist_x < 0.0 { -0.95 } else { 0.95 };
            let offset_start = rotated_direction * (wall_width_half * camera_on_left);
            let offset_end = rotated_direction * (wall_width_half * camera_on_left);

            let wall_start = wall.start + offset_start;
            let wall_end = wall.end + offset_end;
            let segment_start = segment_start + offset_start;
            let segment_end = segment_end + offset_end;

            let dir_half = normalized_direction * wall_width_half;
            let dir_full = normalized_direction * wall_width;

            faux_wall_render_primaries.push((
                wall_start + dir_half,
                wall_end - dir_half,
                segment_start + dir_full,
                segment_end - dir_full,
                camera_on_left,
            ));
            // Edges of the wall
            faux_wall_render_edges.push((
                wall_start - dir_half,
                wall_start + dir_half,
                segment_start + dir_full,
                segment_start,
                camera_on_left,
            ));
            faux_wall_render_edges.push((
                wall_end - dir_half,
                wall_end + dir_half,
                segment_end - dir_full,
                segment_end,
                camera_on_left,
            ));

            // End caps
            let offset_start_inv = rotated_direction * (wall_width_half * -camera_on_left);
            let offset_end_inv = rotated_direction * (wall_width_half * -camera_on_left);

            let wall_start_inv = wall.start + offset_start_inv;
            let wall_end_inv = wall.end + offset_end_inv;
            let segment_start_inv = segment_start - offset_start + offset_start_inv;
            let segment_end_inv = segment_end - offset_end + offset_end_inv;
            faux_wall_render_caps.push((
                wall_start - dir_half,
                wall_start_inv - dir_half,
                segment_start,
                segment_start_inv,
                camera_on_left,
            ));
            faux_wall_render_caps.push((
                wall_end + dir_half,
                wall_end_inv + dir_half,
                segment_end,
                segment_end_inv,
                camera_on_left,
            ));
        }

        // Sort faux walls by distance to camera
        faux_wall_render_caps.sort_by(|a, b| {
            let dist_a = (camera_pos_2d - (a.0 + a.2) / 2.0).length();
            let dist_b = (camera_pos_2d - (b.0 + b.2) / 2.0).length();
            dist_b.partial_cmp(&dist_a).unwrap()
        });
        faux_wall_render_edges.sort_by(|a, b| {
            let dist_a = (camera_pos_2d - (a.0 + a.2) / 2.0).length();
            let dist_b = (camera_pos_2d - (b.0 + b.2) / 2.0).length();
            dist_b.partial_cmp(&dist_a).unwrap()
        });
        faux_wall_render_primaries.sort_by(|a, b| {
            let dist_a = (camera_pos_2d - (a.0 + a.2) / 2.0).length();
            let dist_b = (camera_pos_2d - (b.0 + b.2) / 2.0).length();
            dist_b.partial_cmp(&dist_a).unwrap()
        });

        // Combine the faux walls into a single list
        let mut faux_wall_render = Vec::with_capacity(
            faux_wall_render_primaries.len()
                + faux_wall_render_edges.len()
                + faux_wall_render_caps.len(),
        );
        faux_wall_render.extend(faux_wall_render_caps);
        faux_wall_render.extend(faux_wall_render_edges);
        faux_wall_render.extend(faux_wall_render_primaries);

        for (p1, p2, p3, p4, on_left) in faux_wall_render {
            // Calculate the centroid of the polygon
            let centroid_x = (p1.x + p2.x + p3.x + p4.x) / 4.0;
            let centroid_y = (p1.y + p2.y + p3.y + p4.y) / 4.0;

            // Sort the points in clockwise order
            let mut points = vec![p1, p2, p3, p4];
            points.sort_by(|a, b| {
                let angle_a = (a.y - centroid_y).atan2(a.x - centroid_x);
                let angle_b = (b.y - centroid_y).atan2(b.x - centroid_x);
                angle_a
                    .partial_cmp(&angle_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            // Calculate the light intensity based on the wall direction and light direction
            let line_direction = if on_left < 0.0 { p2 - p1 } else { p1 - p2 };
            let normal = Vec2::new(-line_direction.y, line_direction.x).normalize();
            let light_direction = Vec2::new(1.0, 0.5).normalize();
            let light_intensity =
                normal.dot(&light_direction).clamp(0.0, 1.0) + normal.x * 0.05 - normal.y * 0.05;
            let shaded_color_multiplier = 0.5 + 0.5 * light_intensity;
            let color = Color32::from_rgb(
                (WALL_SIDE_COLOR.r() as f32 * shaded_color_multiplier) as u8,
                (WALL_SIDE_COLOR.g() as f32 * shaded_color_multiplier) as u8,
                (WALL_SIDE_COLOR.b() as f32 * shaded_color_multiplier) as u8,
            );

            let indices = vec![0, 1, 2, 0, 2, 3];
            let mut vertices = Vec::with_capacity(4);
            for point in &points {
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
        for (start, end, width) in wall_render {
            painter.line_segment([start, end], Stroke::new(width, WALL_COLOR));
        }
    }
}
