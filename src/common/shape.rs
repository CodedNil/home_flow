use crate::common::{
    color::Color,
    geo_buffer,
    layout::{
        Action, GlobalMaterial, Home, HomeRender, OpeningType, Operation, Room, RoomRender, Shape,
        Triangles, Walls, Zone,
    },
    utils::hash_vec2,
    utils::{rotate_point_i32, rotate_point_pivot_i32, Material},
};
use geo::{
    triangulate_spade::SpadeTriangulationConfig, BoundingRect, CoordsIter, LinesIter,
    TriangulateEarcut, TriangulateSpade,
};
use geo_types::{Coord, MultiPolygon, Polygon};
use glam::{dvec2 as vec2, DVec2 as Vec2};
use indexmap::IndexMap;
use std::hash::{DefaultHasher, Hash, Hasher};

pub const WALL_WIDTH: f64 = 0.1;

impl Home {
    pub fn render(&mut self, edit_mode: bool) {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        edit_mode.hash(&mut hasher);
        let home_hash = hasher.finish();
        if let Some(rendered_data) = &self.rendered_data {
            if rendered_data.hash == home_hash {
                return;
            }
        }

        // Process all rooms
        for room in &mut self.rooms {
            let mut hasher = DefaultHasher::new();
            room.hash(&mut hasher);
            let hash = hasher.finish();
            if room.rendered_data.is_none() || room.rendered_data.as_ref().unwrap().hash != hash {
                let polygons = room.polygons();
                let any_add = room.operations.iter().any(|o| o.action == Action::AddWall);
                let wall_polys = if room.walls.is_empty() && !any_add {
                    EMPTY_MULTI_POLYGON
                } else {
                    room.wall_polygons(&polygons)
                };
                let mat_tris = room.material_polygons(&self.materials);
                room.rendered_data = Some(RoomRender {
                    hash,
                    polygons,
                    material_triangles: mat_tris,
                    wall_polygons: wall_polys,
                });
            }
        }

        // Process all furniture
        let materials = &self.materials;
        for room in &mut self.rooms {
            for furniture in &mut room.furniture {
                let mut hasher = DefaultHasher::new();
                furniture.hash(&mut hasher);
                let hash = hasher.finish();
                if furniture.rendered_data.is_none()
                    || furniture.rendered_data.as_ref().unwrap().hash != hash
                {
                    let material = get_global_material(materials, &furniture.material);
                    let material_child =
                        get_global_material(materials, &furniture.material_children);
                    let mut render = furniture.render(&material, &material_child);
                    render.hash = hash;
                    furniture.rendered_data = Some(render);
                }
            }
        }

        // Collect all the rooms together to build up the walls
        let mut wall_polygons = vec![];
        for room in &self.rooms {
            if let Some(rendered_data) = &room.rendered_data {
                for poly in &mut wall_polygons {
                    *poly = difference_polygons(poly, &rendered_data.polygons);
                }
                for poly in &rendered_data.wall_polygons {
                    wall_polygons.push(poly.clone().into());
                }
            }
        }

        // Gather wall lines from the polygons
        let mut wall_lines = Vec::new();
        for multipoly in &wall_polygons {
            for poly in multipoly {
                let walls_offset = offset_polygon(poly, -0.025);
                for line in walls_offset.lines_iter() {
                    wall_lines.push((coord_to_vec2(line.start), coord_to_vec2(line.end)));
                }
            }
        }

        // Subtract doors
        for room in &self.rooms {
            for opening in &room.openings {
                if opening.opening_type != OpeningType::Door {
                    continue;
                }
                let opening_polygon = Shape::Rectangle.polygons(
                    room.pos + opening.pos,
                    vec2(opening.width, WALL_WIDTH * 1.01),
                    opening.rotation,
                );
                for poly in &mut wall_polygons {
                    *poly = difference_polygons(poly, &opening_polygon);
                }
            }
        }

        // Create triangles for each polygon
        let mut wall_triangles = Vec::new();
        for multipolygon in &wall_polygons {
            for polygon in multipolygon {
                let (indices, vertices) = triangulate_polygon(polygon);
                wall_triangles.push(Triangles { indices, vertices });
            }
        }

        // If the hashes match, reuse the existing shadows
        let walls_hash = {
            let mut hasher = DefaultHasher::new();
            for room in &self.rooms {
                room.hash(&mut hasher);
            }
            hasher.finish()
        };

        let compute_shadows = || polygons_to_shadows(wall_polygons.iter().collect(), 1.0);
        let wall_shadows = if edit_mode {
            (walls_hash, (Color::TRANSPARENT, vec![]))
        } else {
            self.rendered_data.take().map_or_else(
                || (walls_hash, compute_shadows()),
                |rendered_data| {
                    if rendered_data.wall_shadows.0 == walls_hash {
                        rendered_data.wall_shadows
                    } else {
                        (walls_hash, compute_shadows())
                    }
                },
            )
        };

        self.rendered_data = Some(HomeRender {
            hash: home_hash,
            wall_triangles,
            wall_lines,
            wall_shadows,
        });
    }

    #[cfg(feature = "gui")]
    pub fn render_lighting(&mut self) {
        let mut hasher = DefaultHasher::new();
        for room in &self.rooms {
            hash_vec2(room.pos, &mut hasher);
            hash_vec2(room.size, &mut hasher);
            room.operations.hash(&mut hasher);
            room.walls.hash(&mut hasher);
            room.lights.hash(&mut hasher);
        }
        let mut hash = hasher.finish();
        if let Some(light_data) = &self.light_data {
            if light_data.hash == hash {
                return;
            }
        }

        let all_walls = &self.rendered_data.as_ref().unwrap().wall_lines;

        let (bounds_min, bounds_max) = self.bounds();
        let (update_complete, mut light_data) = crate::client::light_render::render_lighting(
            bounds_min,
            bounds_max,
            &self.rooms,
            all_walls,
        );

        // Override light data for each light
        for room in &mut self.rooms {
            for light in &mut room.lights {
                if let Some(data) = light_data.remove(&light.id) {
                    light.light_data = Some(data);
                }
            }
        }

        // Combine each lights contribution into a single image
        if !update_complete {
            hash -= 1;
        }
        self.light_data = Some(crate::client::light_render::combine_lighting(
            bounds_min,
            bounds_max,
            &self.rooms,
            hash,
        ));
    }

    pub fn get_global_material(&self, string: &str) -> GlobalMaterial {
        get_global_material(&self.materials, string)
    }

    pub fn bounds(&self) -> (Vec2, Vec2) {
        let mut min = Vec2::splat(f64::INFINITY);
        let mut max = Vec2::splat(f64::NEG_INFINITY);
        for room in &self.rooms {
            let (room_min, room_max) = room.bounds();
            min = min.min(room_min);
            max = max.max(room_max);
        }
        (min, max)
    }
}

pub fn get_global_material(materials: &[GlobalMaterial], string: &str) -> GlobalMaterial {
    let is_grout = string.ends_with("-grout");
    let search_string = if is_grout {
        string.trim_end_matches("-grout")
    } else {
        string
    };

    materials
        .iter()
        .find(|&material| material.name == search_string)
        .map_or_else(
            || GlobalMaterial::new(string, Material::Carpet, Color::WHITE),
            |material| {
                if is_grout {
                    let tiles_colour = material
                        .tiles
                        .as_ref()
                        .map_or(Color::WHITE, |t| t.grout_color);
                    GlobalMaterial::new(search_string, material.material, tiles_colour)
                } else {
                    material.clone()
                }
            },
        )
}

impl Room {
    pub fn self_bounds(&self) -> (Vec2, Vec2) {
        (self.pos - self.size / 2.0, self.pos + self.size / 2.0)
    }

    pub fn bounds(&self) -> (Vec2, Vec2) {
        self.operations
            .iter()
            .filter(|op| op.action == Action::Add)
            .flat_map(|op| op.vertices(self.pos))
            .fold(self.self_bounds(), |(min, max), corner| {
                (min.min(corner), max.max(corner))
            })
    }

    pub fn contains(&self, point: Vec2) -> bool {
        // Iterate over operations in reverse to give precedence to the last operation
        for operation in self.operations.iter().rev() {
            if operation.contains(self.pos, point) {
                match operation.action {
                    Action::Add => return true,
                    Action::Subtract => return false,
                    _ => continue, // Ignore other actions
                }
            }
        }
        // If no operations contain the point, check the base rectangle
        Shape::Rectangle.contains(point, self.pos, self.size, 0)
    }

    pub fn polygons(&self) -> MultiPolygon {
        let mut polygons = Shape::Rectangle.polygons(self.pos, self.size, 0);
        for operation in &self.operations {
            let polys = operation.polygons(self.pos);
            match operation.action {
                Action::Add => {
                    polygons = union_polygons(&polygons, &polys);
                }
                Action::Subtract => {
                    polygons = difference_polygons(&polygons, &polys);
                }
                _ => {}
            }
        }
        polygons
    }

    pub fn material_polygons(
        &self,
        global_materials: &[GlobalMaterial],
    ) -> IndexMap<String, Vec<Triangles>> {
        let mut polygons = IndexMap::new();
        polygons.insert(
            self.material.clone(),
            Shape::Rectangle.polygons(self.pos, self.size, 0),
        );
        for operation in &self.operations {
            match operation.action {
                Action::Add => {
                    let material = operation
                        .material
                        .clone()
                        .unwrap_or_else(|| self.material.clone());
                    polygons
                        .entry(material.clone())
                        .and_modify(|poly| {
                            *poly = union_polygons(poly, &operation.polygons(self.pos));
                        })
                        .or_insert_with(|| operation.polygons(self.pos));
                    // Remove from all other polygons
                    for (other_material, poly) in &mut polygons {
                        if other_material != &material {
                            *poly = difference_polygons(poly, &operation.polygons(self.pos));
                        }
                    }
                }
                Action::Subtract => {
                    for poly in polygons.values_mut() {
                        *poly = difference_polygons(poly, &operation.polygons(self.pos));
                    }
                }
                _ => {}
            }
        }

        // Add grout lines every x units
        let mut grout_polygons = Vec::new();
        for (material, poly) in &polygons {
            let global_material = global_materials.iter().find(|m| &m.name == material);
            if let Some(global_material) = global_material {
                if let Some(tile) = &global_material.tiles {
                    let mut new_polygons = Vec::new();
                    let bounds = poly.bounding_rect().unwrap();
                    let poly_center = coord_to_vec2((bounds.min() + bounds.max()) / 2.0);

                    let (startx, endx) = (bounds.min().x, bounds.max().x);
                    let num_grout_x = ((endx - startx) / tile.spacing).floor() as usize;
                    for i in 0..num_grout_x {
                        let x_pos = (i as f64 - (num_grout_x - 1) as f64 / 2.0) * tile.spacing;
                        let line = Shape::Rectangle.polygons(
                            poly_center + vec2(x_pos, 0.0),
                            vec2(tile.grout_width, bounds.height()),
                            0,
                        );
                        new_polygons.push(intersection_polygons(&line, poly));
                    }

                    let num_grout_y = (bounds.height() / tile.spacing).floor() as usize;
                    for i in 0..num_grout_y {
                        let y_pos = (i as f64 - (num_grout_y - 1) as f64 / 2.0) * tile.spacing;
                        let line = Shape::Rectangle.polygons(
                            poly_center + vec2(0.0, y_pos),
                            vec2(bounds.width(), tile.grout_width),
                            0,
                        );
                        new_polygons.push(intersection_polygons(&line, poly));
                    }

                    grout_polygons.push((format!("{material}-grout"), new_polygons));
                }
            }
        }
        // Create triangles for each material
        let mut triangles = IndexMap::new();
        for (material, poly) in &polygons {
            let mut material_triangles = Vec::new();
            for polygon in &poly.0 {
                let (indices, vertices) = triangulate_polygon(polygon);
                material_triangles.push(Triangles { indices, vertices });
            }
            triangles.insert(material.clone(), material_triangles);
        }
        // Add grout triangles
        for (material, polys) in grout_polygons {
            let mut material_triangles = Vec::new();
            for multipolygon in &polys {
                for polygon in multipolygon {
                    let (indices, vertices) = triangulate_polygon(polygon);
                    material_triangles.push(Triangles { indices, vertices });
                }
            }
            triangles.insert(material.clone(), material_triangles);
        }

        triangles
    }

    pub fn wall_polygons(&self, polygons: &MultiPolygon) -> MultiPolygon {
        let width_half = WALL_WIDTH / 2.0;

        // Extract exteriors to ignore inner polygons (holes)
        let new_polygons = polygons
            .iter()
            .map(|polygon| Polygon::new(polygon.exterior().clone(), vec![]))
            .collect::<Vec<_>>();

        // Offset polygons to create wall outlines
        let polygons_outside = offset_polygons(&new_polygons, width_half);
        let polygons_inside = offset_polygons(&new_polygons, -width_half);

        let mut wall_polygons = difference_polygons(&polygons_outside, &polygons_inside);

        // Subtract operations that are SubtractWall
        for operation in &self.operations {
            if operation.action == Action::SubtractWall {
                wall_polygons =
                    difference_polygons(&wall_polygons, &operation.polygon(self.pos).into());
            }
        }

        // If walls aren't on all sides, trim as needed
        if self.walls.is_all() && !self.operations.iter().any(|o| o.action == Action::AddWall) {
            return wall_polygons;
        }

        let bounds = {
            let (min, max) = self.bounds();
            (min - Vec2::splat(WALL_WIDTH), max + Vec2::splat(WALL_WIDTH))
        };
        let center = (bounds.0 + bounds.1) / 2.0;
        let size = bounds.1 - bounds.0;

        let up = size.y * 0.5 - width_half * 3.0;
        let right = size.x * 0.5 - width_half * 3.0;

        let mut subtract_shape = EMPTY_MULTI_POLYGON;
        for index in 0..4 {
            if !match index {
                0 => self.walls.contains(Walls::LEFT),
                1 => self.walls.contains(Walls::TOP),
                2 => self.walls.contains(Walls::RIGHT),
                _ => self.walls.contains(Walls::BOTTOM),
            } {
                let pos_neg = vec2(1.0, -1.0);
                let neg_pos = vec2(-1.0, 1.0);
                let neg = vec2(-1.0, -1.0);
                let pos = vec2(1.0, 1.0);
                let mut vertices = [
                    vec![Vec2::ZERO, neg_pos, vec2(-4.0, 1.0), vec2(-4.0, -1.0), neg], // Left
                    vec![Vec2::ZERO, neg_pos, vec2(-1.0, 4.0), vec2(1.0, 4.0), pos],   // Top
                    vec![Vec2::ZERO, pos, vec2(4.0, 1.0), vec2(4.0, -1.0), pos_neg],   // Right
                    vec![Vec2::ZERO, neg, vec2(-1.0, -4.0), vec2(1.0, -4.0), pos_neg], // Bottom
                ];
                vertices[index]
                    .iter_mut()
                    .for_each(|vertex| *vertex = center + *vertex * vec2(right, up));
                subtract_shape =
                    union_polygons(&subtract_shape, &create_polygons(&vertices[index]));
            }
        }
        // Add corners
        let directions = [(-right, Walls::LEFT), (right, Walls::RIGHT)];
        let verticals = [(up, Walls::TOP), (-up, Walls::BOTTOM)];
        for (h_mult, h_wall) in &directions {
            for (v_mult, v_wall) in &verticals {
                if !self.walls.contains(*h_wall) && !self.walls.contains(*v_wall) {
                    subtract_shape = union_polygons(
                        &subtract_shape,
                        &create_polygons(&[
                            center + vec2(*h_mult * 0.9, *v_mult * 0.9),
                            center + vec2(*h_mult * 4.0, *v_mult * 0.9),
                            center + vec2(*h_mult * 4.0, *v_mult * 4.0),
                            center + vec2(*h_mult * 0.9, *v_mult * 4.0),
                        ]),
                    );
                }
            }
        }

        // Add back operations that are AddWall
        for operation in &self.operations {
            if operation.action == Action::AddWall {
                let operation_polygon = operation.polygons(self.pos);
                subtract_shape = difference_polygons(&subtract_shape, &operation_polygon);
            }
        }

        difference_polygons(&wall_polygons, &subtract_shape)
    }
}

impl Operation {
    pub fn contains(&self, room_pos: Vec2, point: Vec2) -> bool {
        self.shape
            .contains(point, room_pos + self.pos, self.size, self.rotation)
    }

    pub fn vertices(&self, room_pos: Vec2) -> Vec<Vec2> {
        self.shape
            .vertices(room_pos + self.pos, self.size, self.rotation)
    }

    pub fn polygon(&self, room_pos: Vec2) -> Polygon {
        create_polygon(&self.vertices(room_pos))
    }

    pub fn polygons(&self, room_pos: Vec2) -> MultiPolygon {
        self.polygon(room_pos).into()
    }
}

impl Zone {
    pub fn contains(&self, room_pos: Vec2, point: Vec2) -> bool {
        self.shape
            .contains(point, room_pos + self.pos, self.size, self.rotation)
    }

    pub fn vertices(&self, room_pos: Vec2) -> Vec<Vec2> {
        self.shape
            .vertices(room_pos + self.pos, self.size, self.rotation)
    }
}

pub fn point_to_vec2(c: geo_types::Point) -> Vec2 {
    vec2(c.x(), c.y())
}

pub const fn coord_to_vec2(c: Coord) -> Vec2 {
    vec2(c.x, c.y)
}

pub const fn vec2_to_coord(v: &Vec2) -> Coord {
    Coord { x: v.x, y: v.y }
}

pub fn create_polygon(vertices: &[Vec2]) -> Polygon {
    Polygon::new(
        geo::LineString::from(vertices.iter().map(vec2_to_coord).collect::<Vec<_>>()),
        vec![],
    )
}

pub fn create_polygons(vertices: &[Vec2]) -> MultiPolygon {
    create_polygon(vertices).into()
}

pub const EMPTY_MULTI_POLYGON: MultiPolygon = MultiPolygon(vec![]);

pub fn triangulate_polygon(polygon: &Polygon) -> (Vec<u32>, Vec<Vec2>) {
    let triangles = polygon.earcut_triangles_raw();
    let (indices, vertices) = (triangles.triangle_indices, triangles.vertices);

    (
        indices.iter().map(|&i| i as u32).collect(),
        vertices
            .chunks(2)
            .map(|chunk| vec2(chunk[0], chunk[1]))
            .collect(),
    )
}

#[derive(Clone)]
pub struct ShadowTriangles {
    pub indices: Vec<u32>,
    pub vertices: Vec<Vec2>,
    pub inners: Vec<bool>,
}

fn offset_polygon(polygon: &Polygon, offset_size: f64) -> MultiPolygon {
    geo_buffer::buffer_polygon(polygon, offset_size)
}

fn offset_polygons(polygons: &[Polygon], distance: f64) -> MultiPolygon {
    polygons
        .iter()
        .map(|polygon| offset_polygon(polygon, distance))
        .fold(EMPTY_MULTI_POLYGON, |acc, poly| union_polygons(&acc, &poly))
}

fn union_polygons(poly_a: &MultiPolygon, poly_b: &MultiPolygon) -> MultiPolygon {
    geo::BooleanOps::union(poly_a, poly_b)
}

fn difference_polygons(poly_a: &MultiPolygon, poly_b: &MultiPolygon) -> MultiPolygon {
    geo::BooleanOps::difference(poly_a, poly_b)
}

fn intersection_polygons(poly_a: &MultiPolygon, poly_b: &MultiPolygon) -> MultiPolygon {
    geo::BooleanOps::intersection(poly_a, poly_b)
}

pub type Line = (Vec2, Vec2);

pub type ShadowsData = (Color, Vec<ShadowTriangles>);

pub fn polygons_to_shadows(polygons: Vec<&MultiPolygon>, height: f64) -> ShadowsData {
    let offset_size = height * 0.05;
    let mut shadow_exteriors = EMPTY_MULTI_POLYGON;
    let mut shadow_interiors = EMPTY_MULTI_POLYGON;
    let mut interior_points = Vec::new();
    for multipoly in polygons {
        for poly in multipoly {
            let exterior = offset_polygon(poly, offset_size);
            shadow_exteriors = union_polygons(&shadow_exteriors, &exterior);

            let interior = offset_polygon(poly, -0.025);
            shadow_interiors = union_polygons(&shadow_interiors, &interior);

            interior_points.extend(interior.exterior_coords_iter().map(coord_to_vec2));
        }
    }
    let shadow_polygons = difference_polygons(&shadow_exteriors, &shadow_interiors);

    let mut shadow_triangles = Vec::new();
    for polygon in shadow_polygons {
        let (indices, vertices) = {
            let triangles = polygon
                .constrained_triangulation(SpadeTriangulationConfig::default())
                .unwrap();
            let mut indices = Vec::new();
            let mut vertices = Vec::new();
            for triangle in triangles {
                for point in triangle.to_array() {
                    let index = vertices.len() as u32;
                    indices.push(index);
                    vertices.push(vec2(point.x, point.y));
                }
            }
            (indices, vertices)
        };

        let inners = vertices
            .iter()
            .map(|vertex| interior_points.iter().any(|p| p.distance(*vertex) < 0.001))
            .collect();

        shadow_triangles.push(ShadowTriangles {
            indices,
            vertices,
            inners,
        });
    }

    let intensity = 1.0 - height;
    let (low, high) = (80.0, 150.0);
    let shadow_color = Color::from_alpha((low + (high - low) * intensity) as u8);

    (shadow_color, shadow_triangles)
}

impl Shape {
    pub fn contains(self, point: Vec2, center: Vec2, size: Vec2, rotation: i32) -> bool {
        let point = if rotation != 0 {
            rotate_point_pivot_i32(point, center, rotation)
        } else {
            point
        };
        match self {
            Self::Rectangle => (point - center).abs().cmple(size * 0.5).all(),
            Self::Circle => ((point - center) / (size * 0.5)).length_squared() <= 1.0,
            Self::Triangle => {
                let relative_x = point.x - center.x + size.x / 2.0;
                let relative_y = center.y - point.y + size.y / 2.0;
                relative_x >= 0.0
                    && relative_y >= 0.0
                    && relative_y <= size.y
                    && relative_y <= -(size.y / size.x) * relative_x + size.y
            }
        }
    }

    pub fn vertices(self, pos: Vec2, size: Vec2, rotation: i32) -> Vec<Vec2> {
        match self {
            Self::Rectangle => vec![(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)],
            Self::Circle => {
                let quality = 45;
                (0..quality)
                    .map(|i| {
                        let angle =
                            (f64::from(i) / f64::from(quality)) * std::f64::consts::PI * 2.0;
                        (angle.cos() * 0.5, angle.sin() * 0.5)
                    })
                    .collect()
            }
            Self::Triangle => vec![(-0.5, 0.5), (0.5, 0.5), (-0.5, -0.5)],
        }
        .iter()
        .map(|(x_offset, y_offset)| {
            rotate_point_i32(vec2(x_offset * size.x, y_offset * size.y), -rotation) + pos
        })
        .collect()
    }

    pub fn polygon(self, pos: Vec2, size: Vec2, rotation: i32) -> Polygon {
        create_polygon(&self.vertices(pos, size, rotation))
    }

    pub fn polygons(self, pos: Vec2, size: Vec2, rotation: i32) -> MultiPolygon {
        self.polygon(pos, size, rotation).into()
    }
}
