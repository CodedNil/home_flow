use super::{vec2_to_egui_pos, HomeFlow};
use crate::common::{
    color::Color,
    furniture::{AnimatedPieceType, Furniture, FurnitureType},
    layout::{OpeningType, Shape},
    shape::WALL_WIDTH,
    utils::{rotate_point, rotate_point_i32, Material},
};
use egui::{
    epaint::Vertex, Color32, ColorImage, Mesh, Painter, Shape as EShape, Stroke, TextureId,
    TextureOptions,
};
use glam::{dvec2 as vec2, DVec2 as Vec2};
use std::collections::HashMap;

const WALL_COLOR: Color32 = Color32::from_rgb(130, 80, 20);
const DOOR_COLOR: Color32 = Color32::from_rgb(200, 130, 40);
const WINDOW_COLOR: Color32 = Color32::from_rgb(80, 140, 240);

impl HomeFlow {
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

        // Get bounds
        let mut min = Vec2::splat(f64::INFINITY);
        let mut max = Vec2::splat(f64::NEG_INFINITY);
        for room in &self.layout.rooms {
            let (room_min, room_max) = room.bounds_with_walls();
            min = min.min(room_min);
            max = max.max(room_max);
        }
        self.bounds = (min, max);

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
            let rendered_data = furniture.rendered_data.as_ref().unwrap();
            for (material, _) in &rendered_data.triangles {
                materials_to_ready.push(material.material);
            }
            for child in &rendered_data.children {
                for (material, _) in &child.rendered_data.as_ref().unwrap().triangles {
                    materials_to_ready.push(material.material);
                }
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
                            (outline.thickness * self.stored.zoom) as f32,
                            outline.color.to_egui(),
                        ),
                    ));
                }
            }
        }

        // Hover furniture
        let mut furnitures_hovered = Vec::new();
        for furniture in &self.layout.furniture {
            if furniture.can_hover()
                && Shape::Rectangle.contains(
                    self.mouse_pos_world,
                    furniture.pos,
                    furniture.size * 1.2,
                    furniture.rotation,
                )
            {
                furnitures_hovered.push(furniture);
            }
            let rendered_data = furniture.rendered_data.as_ref().unwrap();
            for child in &rendered_data.children {
                if child.can_hover()
                    && Shape::Rectangle.contains(
                        self.mouse_pos_world,
                        furniture.pos
                            + rotate_point_i32(child.pos, Vec2::ZERO, -furniture.rotation),
                        child.size * 1.2,
                        furniture.rotation + child.rotation,
                    )
                {
                    furnitures_hovered.push(child);
                }
            }
        }
        let mut furniture_sorted = furnitures_hovered.clone();
        furniture_sorted.sort_by_key(|f| f.render_order());
        let top_hover = furniture_sorted.last().map(|f| f.id);

        for furniture in &mut self.layout.furniture {
            let target = (Some(furniture.id) == top_hover) as u8 as f64;
            let difference = target - furniture.hover_amount;
            if difference.abs() > f64::EPSILON {
                furniture.hover_amount = (furniture.hover_amount
                    + difference.signum() * self.frame_time * 10.0)
                    .clamp(0.0, 1.0);
            }
            let rendered_data = furniture.rendered_data.as_mut().unwrap();
            for child in &mut rendered_data.children {
                let target = (Some(child.id) == top_hover) as u8 as f64;
                let difference = target - child.hover_amount;
                if difference.abs() > f64::EPSILON {
                    child.hover_amount = (child.hover_amount
                        + difference.signum() * self.frame_time * 10.0)
                        .clamp(0.0, 1.0);
                }
            }
        }

        // Gather furniture and children
        let mut furniture_map = HashMap::new();
        let mut furniture_adjustments = HashMap::new();

        let mut handle_furniture_child = |obj: &Furniture, child: &Furniture| {
            let hover = child.hover_amount;
            let (offset, offset_rot) = match child.furniture_type {
                FurnitureType::Chair(_) => (vec2(hover * 0.15, hover * 0.3), hover * 20.0),
                FurnitureType::AnimatedPiece(animated_piece_type) => match animated_piece_type {
                    AnimatedPieceType::Drawer
                    | AnimatedPieceType::DrawerMid
                    | AnimatedPieceType::DrawerHigh => {
                        (vec2(0.0, child.size.y * hover * -0.6), 0.0)
                    }
                    AnimatedPieceType::Door(side)
                    | AnimatedPieceType::DoorMid(side)
                    | AnimatedPieceType::DoorHigh(side) => {
                        if side {
                            let rotate = -hover * 60.0;
                            let offset = rotate_point(
                                Vec2::ZERO,
                                vec2(-child.size.x / 2.0, -child.size.y / 2.0),
                                rotate,
                            );
                            (offset, -rotate)
                        } else {
                            let rotate = hover * 60.0;
                            let offset = rotate_point(
                                Vec2::ZERO,
                                vec2(child.size.x / 2.0, -child.size.y / 2.0),
                                rotate,
                            );
                            (offset, -rotate)
                        }
                    }
                },
                _ => (Vec2::ZERO, 0.0), // Handles other FurnitureTypes
            };

            let offset = rotate_point_i32(offset, Vec2::ZERO, -(obj.rotation + child.rotation));
            furniture_adjustments.insert(
                child.id,
                (
                    obj.pos + rotate_point_i32(child.pos, Vec2::ZERO, -obj.rotation) + offset,
                    obj.rotation as f64 + child.rotation as f64 + offset_rot,
                ),
            );
        };

        for obj in &self.layout.furniture {
            let rendered_data = obj.rendered_data.as_ref().unwrap();
            furniture_map
                .entry(obj.render_order())
                .or_insert_with(Vec::new)
                .push(obj);
            for child in &rendered_data.children {
                handle_furniture_child(obj, child);
                furniture_map
                    .entry(child.render_order())
                    .or_insert_with(Vec::new)
                    .push(child);
            }
        }

        let mut order_keys: Vec<&u8> = furniture_map.keys().collect();
        order_keys.sort();

        // Render furniture
        for key in order_keys {
            if let Some(furnitures) = furniture_map.get(key) {
                for furniture in furnitures {
                    let rendered_data = furniture.rendered_data.as_ref().unwrap();
                    let &(pos, rot) = furniture_adjustments
                        .get(&furniture.id)
                        .unwrap_or(&(furniture.pos, furniture.rotation as f64));

                    // Render shadow
                    let shadow_offset = vec2(0.01, -0.02);
                    let (shadow_color, shadow_triangles) = &rendered_data.shadow_triangles;
                    for triangles in shadow_triangles {
                        let vertices = triangles
                            .vertices
                            .iter()
                            .enumerate()
                            .map(|(i, &v)| {
                                let is_interior = *triangles.inners.get(i).unwrap_or(&false);
                                let adjusted_v =
                                    rotate_point(v, Vec2::ZERO, -rot) + pos + shadow_offset;
                                Vertex {
                                    pos: vec2_to_egui_pos(self.world_to_pixels(adjusted_v)),
                                    uv: egui::Pos2::ZERO,
                                    color: if is_interior {
                                        *shadow_color
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
                }
                for furniture in furnitures {
                    let rendered_data = furniture.rendered_data.as_ref().unwrap();
                    let &(pos, rot) = furniture_adjustments
                        .get(&furniture.id)
                        .unwrap_or(&(furniture.pos, furniture.rotation as f64));

                    for (material, multi_triangles) in &rendered_data.triangles {
                        let texture_id = self.load_texture(material.material);
                        for triangles in multi_triangles {
                            let vertices = triangles
                                .vertices
                                .iter()
                                .map(|&v| {
                                    let adjusted_v = rotate_point(v, Vec2::ZERO, -rot) + pos;
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
            }
        }

        // Render wall shadows
        let rendered_data = self.layout.rendered_data.as_ref().unwrap();
        let shadow_offset = vec2(0.01, -0.02);
        let (shadow_color, shadow_triangles) = &rendered_data.wall_shadows.1;
        for triangles in shadow_triangles {
            if triangles.vertices.is_empty() {
                continue;
            }
            let vertices = triangles
                .vertices
                .iter()
                .enumerate()
                .map(|(i, &v)| {
                    let is_interior = *triangles.inners.get(i).unwrap_or(&false);
                    Vertex {
                        pos: vec2_to_egui_pos(self.world_to_pixels(v + shadow_offset)),
                        uv: egui::Pos2::ZERO,
                        color: if is_interior {
                            *shadow_color
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

        // Open the door if mouse is nearby
        for room in &mut self.layout.rooms {
            for opening in &mut room.openings {
                if opening.opening_type != OpeningType::Door {
                    continue;
                }
                let mouse_distance = self.mouse_pos_world.distance(room.pos + opening.pos);
                let target = (mouse_distance < opening.width / 2.0) as u8 as f64;
                let difference = target - opening.open_amount;
                if difference.abs() > f64::EPSILON {
                    // Linearly interpolate open_amount towards the target value.
                    opening.open_amount = (opening.open_amount
                        + (target - opening.open_amount) * self.frame_time * 8.0)
                        .clamp(0.0, 1.0);
                }
            }
        }
        // Render openings
        let mut window_meshes = Vec::new();
        for room in &self.layout.rooms {
            for opening in &room.openings {
                let color = match opening.opening_type {
                    OpeningType::Door => DOOR_COLOR,
                    OpeningType::Window => WINDOW_COLOR,
                };
                let depth = match opening.opening_type {
                    OpeningType::Door => WALL_WIDTH * 0.8,
                    OpeningType::Window => WALL_WIDTH,
                };
                let rot_dir = vec2(
                    (opening.rotation as f64).to_radians().cos(),
                    (opening.rotation as f64).to_radians().sin(),
                );
                let hinge_pos = room.pos + opening.pos + rot_dir * (opening.width) / 2.0;

                let vertices = Shape::Rectangle
                    .vertices(
                        room.pos + opening.pos,
                        vec2(opening.width, depth),
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
                if opening.opening_type == OpeningType::Window {
                    window_meshes.push(Mesh {
                        indices: vec![0, 1, 2, 2, 3, 0],
                        vertices,
                        texture_id: TextureId::Managed(0),
                    });
                } else {
                    painter.add(EShape::mesh(Mesh {
                        indices: vec![0, 1, 2, 2, 3, 0],
                        vertices,
                        texture_id: TextureId::Managed(0),
                    }));
                }
            }
        }

        // Render walls
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

        // Render windows above walls
        for mesh in window_meshes {
            painter.add(EShape::mesh(mesh));
        }

        // Render lights
        for room in &self.layout.rooms {
            for light in &room.lights {
                let pos = self.world_to_pixels(room.pos + light.pos);
                let color = Color32::from_rgb(255, 255, 0).gamma_multiply(0.8);
                painter.add(EShape::circle_filled(
                    vec2_to_egui_pos(pos),
                    0.1 * self.stored.zoom as f32,
                    color,
                ));
            }
        }
    }
}
