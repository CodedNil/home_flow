use super::HomeFlow;
use crate::common::{
    layout::{OpeningType, Shape},
    shape::{TEXTURES, WALL_WIDTH},
    utils::{rotate_point, vec2_to_egui_pos},
};
use egui::{
    epaint::Vertex, Color32, ColorImage, Mesh, Painter, Rect, Shape as EShape, Stroke, TextureId,
    TextureOptions,
};
use glam::dvec2 as vec2;
use std::collections::{HashMap, HashSet};

const WALL_COLOR: Color32 = Color32::from_rgb(130, 80, 20);
const DOOR_COLOR: Color32 = Color32::from_rgb(200, 130, 40);
const WINDOW_COLOR: Color32 = Color32::from_rgb(80, 140, 240);

impl HomeFlow {
    pub fn render_grid(&self, painter: &Painter, visible_rect: &Rect) {
        let grid_interval = 2.0_f64.powf((160.0 / self.zoom).abs().log2().round());
        let grid_intervals = [
            (
                grid_interval,
                Stroke::new(1.5, Color32::from_rgb(85, 85, 100)),
            ),
            (
                grid_interval / 4.0,
                Stroke::new(1.5, Color32::from_rgb(55, 55, 70)),
            ),
        ];

        let (bottom_edge_world, top_edge_world) = (
            self.pixels_to_world(0.0, visible_rect.bottom() as f64).y,
            self.pixels_to_world(0.0, visible_rect.top() as f64).y,
        );
        let (left_edge_world, right_edge_world) = (
            self.pixels_to_world(visible_rect.left() as f64, 0.0).x,
            self.pixels_to_world(visible_rect.right() as f64, 0.0).x,
        );

        let mut rendered_vertical = HashSet::new();
        let mut rendered_horizontal = HashSet::new();
        let mut lines = Vec::new();
        for (grid_interval, stroke) in grid_intervals {
            // Draw vertical grid lines
            for x in ((left_edge_world / grid_interval).ceil() as i32)
                ..=((right_edge_world / grid_interval).floor() as i32)
            {
                let grid_line_pixel = self.world_to_pixels(x as f64 * grid_interval, 0.0).x;
                let grid_line_pixel_int = (grid_line_pixel * 100.0).round() as i32;
                if rendered_vertical.contains(&grid_line_pixel_int) {
                    continue;
                }
                rendered_vertical.insert(grid_line_pixel_int);
                lines.push((
                    egui::pos2(grid_line_pixel as f32, visible_rect.top()),
                    egui::pos2(grid_line_pixel as f32, visible_rect.bottom()),
                    stroke,
                ));
            }

            // Draw horizontal grid lines
            for y in ((bottom_edge_world / grid_interval).ceil() as i32)
                ..=((top_edge_world / grid_interval).floor() as i32)
            {
                let grid_line_pixel = self.world_to_pixels(0.0, y as f64 * grid_interval).y;
                let grid_line_pixel_int = (grid_line_pixel * 100.0).round() as i32;
                if rendered_horizontal.contains(&grid_line_pixel_int) {
                    continue;
                }
                rendered_horizontal.insert(grid_line_pixel_int);
                lines.push((
                    egui::pos2(visible_rect.left(), grid_line_pixel as f32),
                    egui::pos2(visible_rect.right(), grid_line_pixel as f32),
                    stroke,
                ));
            }
        }
        for line in lines.iter().rev() {
            painter.line_segment([line.0, line.1], line.2);
        }
    }

    pub fn render_layout(&mut self, painter: &Painter, ctx: &egui::Context) {
        self.layout.render();

        // Render rooms
        for room in &self.layout.rooms {
            let rendered_data = room.rendered_data.as_ref().unwrap();
            for (material, multi_triangles) in &rendered_data.material_triangles {
                for triangles in multi_triangles {
                    let texture_id = self
                        .textures
                        .entry(material.to_string())
                        .or_insert_with(|| {
                            let texture = TEXTURES.get(material).unwrap();
                            let canvas_size = texture.dimensions();
                            let egui_image = ColorImage::from_rgba_unmultiplied(
                                [canvas_size.0 as usize, canvas_size.1 as usize],
                                texture,
                            );
                            ctx.load_texture(
                                material.to_string(),
                                egui_image,
                                TextureOptions::NEAREST_REPEAT,
                            )
                        })
                        .id();

                    let color = room.render_options.tint.unwrap_or(Color32::WHITE);
                    let vertices = triangles
                        .vertices
                        .iter()
                        .map(|&v| {
                            let local_pos = v * 0.2;
                            Vertex {
                                pos: vec2_to_egui_pos(self.world_to_pixels(v.x, v.y)),
                                uv: egui::pos2(local_pos.x as f32, local_pos.y as f32),
                                color,
                            }
                        })
                        .collect();
                    painter.add(EShape::mesh(Mesh {
                        indices: triangles.indices.clone(),
                        vertices,
                        texture_id,
                    }));
                }
            }
            // Render outline line around each of the rooms polygons
            if let Some(outline) = &room.outline {
                let rendered_data = room.rendered_data.as_ref().unwrap();
                for polygon in &rendered_data.polygons {
                    let vertices = polygon
                        .exterior()
                        .points()
                        .map(|v| vec2_to_egui_pos(self.world_to_pixels(v.x(), v.y())))
                        .collect();
                    painter.add(EShape::closed_line(
                        vertices,
                        Stroke::new((outline.thickness * self.zoom) as f32, outline.color),
                    ));
                }
            }
        }
        for room in &self.layout.rooms {}

        // Render walls
        let rendered_data = self.layout.rendered_data.as_ref().unwrap();
        for wall in &rendered_data.wall_triangles {
            let vertices = wall
                .vertices
                .iter()
                .map(|v| Vertex {
                    pos: vec2_to_egui_pos(self.world_to_pixels(v.x, v.y)),
                    uv: egui::Pos2::default(),
                    color: WALL_COLOR,
                })
                .collect();
            painter.add(EShape::mesh(Mesh {
                indices: wall.indices.clone(),
                vertices,
                texture_id: TextureId::Managed(0),
            }));
        }

        // Open the opening if mouse is nearby
        for room in &mut self.layout.rooms {
            for opening in &mut room.openings {
                if opening.opening_type != OpeningType::Door {
                    continue;
                }
                let mouse_distance = self.mouse_pos_world.distance(room.pos + opening.pos);
                let target = if mouse_distance < opening.width / 2.0 {
                    1.0
                } else {
                    0.0
                };

                // Linearly interpolate open_amount towards the target value.
                opening.open_amount += (target - opening.open_amount) * (self.frame_time * 5.0);
                opening.open_amount = opening.open_amount.clamp(0.0, 1.0);
            }
        }
        // Render openings
        for room in &self.layout.rooms {
            for opening in &room.openings {
                let color = match opening.opening_type {
                    OpeningType::Door => DOOR_COLOR,
                    OpeningType::Window => WINDOW_COLOR,
                };
                let rot_dir = vec2(
                    opening.rotation.to_radians().cos(),
                    opening.rotation.to_radians().sin(),
                );
                let hinge_pos = room.pos + opening.pos + rot_dir * (opening.width) / 2.0;

                let vertices = Shape::Rectangle
                    .vertices(
                        room.pos + opening.pos,
                        vec2(opening.width, WALL_WIDTH * 0.8),
                        opening.rotation,
                    )
                    .iter()
                    .map(|v| {
                        let rotated = rotate_point(*v, hinge_pos, opening.open_amount * 40.0);
                        Vertex {
                            pos: vec2_to_egui_pos(self.world_to_pixels(rotated.x, rotated.y)),
                            uv: egui::Pos2::default(),
                            color,
                        }
                    })
                    .collect();
                painter.add(EShape::mesh(Mesh {
                    indices: vec![0, 1, 2, 2, 3, 0],
                    vertices,
                    texture_id: TextureId::Managed(0),
                }));
            }
        }
    }
}
