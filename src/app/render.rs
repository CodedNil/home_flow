use super::{vec2_to_egui_pos, HomeFlow};
use crate::common::{
    color::Color,
    layout::{OpeningType, Shape},
    shape::WALL_WIDTH,
    utils::{rotate_point, Material},
};
use egui::{
    epaint::Vertex, Color32, ColorImage, Mesh, Painter, Rect, Shape as EShape, Stroke, TextureId,
    TextureOptions,
};
use glam::{dvec2 as vec2, DVec2 as Vec2};

const WALL_COLOR: Color32 = Color32::from_rgb(130, 80, 20);
const DOOR_COLOR: Color32 = Color32::from_rgb(200, 130, 40);
const WINDOW_COLOR: Color32 = Color32::from_rgb(80, 140, 240);

const GRID_THIN: Stroke = Stroke {
    width: 1.5,
    color: Color32::from_rgb(55, 55, 70),
};
const GRID_THICK: Stroke = Stroke {
    width: 1.5,
    color: Color32::from_rgb(85, 85, 100),
};

impl HomeFlow {
    pub fn render_grid(&self, painter: &Painter, visible_rect: &Rect) {
        let grid_interval = 2.0_f64.powf((160.0 / self.zoom).abs().log2().round()) / 4.0;

        let bottom_edge_world = self.pixels_to_world_y(visible_rect.bottom() as f64);
        let top_edge_world = self.pixels_to_world_y(visible_rect.top() as f64);
        let left_edge_world = self.pixels_to_world_x(visible_rect.left() as f64);
        let right_edge_world = self.pixels_to_world_x(visible_rect.right() as f64);

        let vertical_lines = ((left_edge_world / grid_interval).ceil() as i32)
            ..=((right_edge_world / grid_interval).floor() as i32);
        for i in vertical_lines {
            let x = self.world_to_pixels_x(i as f64 * grid_interval);
            painter.line_segment(
                [
                    egui::pos2(x as f32, visible_rect.top()),
                    egui::pos2(x as f32, visible_rect.bottom()),
                ],
                if i % 4 == 0 { GRID_THICK } else { GRID_THIN },
            );
        }

        let horizontal_lines = ((bottom_edge_world / grid_interval).ceil() as i32)
            ..=((top_edge_world / grid_interval).floor() as i32);
        for i in horizontal_lines {
            let y = self.world_to_pixels_y(i as f64 * grid_interval);
            painter.line_segment(
                [
                    egui::pos2(visible_rect.left(), y as f32),
                    egui::pos2(visible_rect.right(), y as f32),
                ],
                if i % 4 == 0 { GRID_THICK } else { GRID_THIN },
            );
        }
    }

    pub fn ready_texture(&mut self, material: Material, ctx: &egui::Context) {
        self.textures
            .entry(material.to_string())
            .or_insert_with(|| {
                let texture = image::load_from_memory(material.get_image())
                    .unwrap()
                    .into_rgba8();
                let (width, height) = texture.dimensions();
                ctx.load_texture(
                    material.to_string(),
                    ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &texture),
                    TextureOptions::NEAREST_REPEAT,
                )
            });
    }

    pub fn load_texture(&self, material: Material) -> TextureId {
        self.textures.get(&material.to_string()).unwrap().id()
    }

    pub fn render_layout(&mut self, painter: &Painter, ctx: &egui::Context) {
        self.layout.render();

        // Ready textures
        let mut materials_to_ready = Vec::new();
        for room in &self.layout.rooms {
            if let Some(data) = &room.rendered_data {
                for material in data.material_triangles.keys() {
                    materials_to_ready.push(self.layout.get_global_material(material).material);
                }
            }
        }
        for furniture in &self.layout.furniture {
            for (material, _) in &furniture.rendered_data.as_ref().unwrap().triangles {
                materials_to_ready.push(material.material);
            }
        }
        for material in materials_to_ready {
            self.ready_texture(material, ctx);
        }

        // Render rooms
        for room in &self.layout.rooms {
            let rendered_data = room.rendered_data.as_ref().unwrap();
            for (material, multi_triangles) in &rendered_data.material_triangles {
                let global_material = self.layout.get_global_material(material);
                let texture_id = self.load_texture(global_material.material);
                for triangles in multi_triangles {
                    let vertices = triangles
                        .vertices
                        .iter()
                        .map(|&v| Vertex {
                            pos: vec2_to_egui_pos(self.world_to_pixels(v)),
                            uv: vec2_to_egui_pos(v * 0.2),
                            color: global_material.tint.to_egui(),
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
                        .map(|v| vec2_to_egui_pos(self.world_to_pixels_xy(v.x(), v.y())))
                        .collect();
                    painter.add(EShape::closed_line(
                        vertices,
                        Stroke::new(
                            (outline.thickness * self.zoom) as f32,
                            outline.color.to_egui(),
                        ),
                    ));
                }
            }
        }

        // Render furniture
        for furniture in &self.layout.furniture {
            let rendered_data = furniture.rendered_data.as_ref().unwrap();

            // Render shadow
            let shadow_offset = vec2(0.01, -0.02);
            for (triangles, interior_points) in &rendered_data.shadow_triangles {
                let vertices = triangles
                    .vertices
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| {
                        let is_interior = interior_points.get(&i).is_some_and(|&b| b);
                        let adjusted_v = rotate_point(v, Vec2::ZERO, -furniture.rotation)
                            + furniture.pos
                            + shadow_offset;
                        Vertex {
                            pos: vec2_to_egui_pos(self.world_to_pixels(adjusted_v)),
                            uv: egui::Pos2::ZERO,
                            color: if is_interior {
                                Color::from_alpha(150)
                            } else {
                                Color::TRANSPARENT
                            }
                            .to_egui(),
                        }
                    })
                    .collect();
                painter.add(EShape::mesh(Mesh {
                    indices: triangles.indices.clone(),
                    vertices,
                    texture_id: TextureId::Managed(0),
                }));
            }

            for (material, multi_triangles) in &rendered_data.triangles {
                let texture_id = self.load_texture(material.material);
                for triangles in multi_triangles {
                    let vertices = triangles
                        .vertices
                        .iter()
                        .map(|&v| {
                            let adjusted_v =
                                rotate_point(v, Vec2::ZERO, -furniture.rotation) + furniture.pos;
                            Vertex {
                                pos: vec2_to_egui_pos(self.world_to_pixels(adjusted_v)),
                                uv: vec2_to_egui_pos(v * 0.2),
                                color: material.tint.to_egui(),
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
        }

        // Render walls
        let rendered_data = self.layout.rendered_data.as_ref().unwrap();
        for wall in &rendered_data.wall_triangles {
            let vertices = wall
                .vertices
                .iter()
                .map(|v| Vertex {
                    pos: vec2_to_egui_pos(self.world_to_pixels(*v)),
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
                            pos: vec2_to_egui_pos(self.world_to_pixels(rotated)),
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
