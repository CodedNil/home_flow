use super::{
    color::Color,
    furniture::{Furniture, FurnitureRender},
    layout::{
        Action, GlobalMaterial, Home, HomeRender, Operation, Room, RoomRender, Shape, Triangles,
        Walls,
    },
    utils::{rotate_point, Material},
};
use geo::{BooleanOps, TriangulateEarcut};
use geo_types::{MultiPolygon, Polygon};
use glam::{dvec2 as vec2, DVec2 as Vec2};
use rayon::prelude::*;
use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
};

pub const WALL_WIDTH: f64 = 0.1;

impl Home {
    pub fn render(&mut self) {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        let home_hash = hasher.finish();
        if let Some(rendered_data) = &self.rendered_data {
            if rendered_data.hash == home_hash {
                return;
            }
        }

        // Find rooms to update which have been modified, get (index, id, hash)
        let rooms_to_update = self
            .rooms
            .iter()
            .enumerate()
            .filter_map(|(index, room)| {
                let mut hasher = DefaultHasher::new();
                room.hash(&mut hasher);
                let hash = hasher.finish();
                if room.rendered_data.is_none() || room.rendered_data.as_ref().unwrap().hash != hash
                {
                    Some((index, room.id, hash))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        // Process all rooms in parallel
        let new_data = rooms_to_update
            .par_iter()
            .map(|(index, id, hash)| {
                let room = &self.rooms[*index];
                let polygons = room.polygons();
                let any_add = room.operations.iter().any(|o| o.action == Action::AddWall);
                let wall_polygons = if room.walls == Walls::NONE && !any_add {
                    EMPTY_MULTI_POLYGON
                } else {
                    room.wall_polygons(&polygons)
                };
                let (mat_polys, mat_tris) = room.material_polygons();
                (*id, *hash, polygons, mat_polys, mat_tris, wall_polygons)
            })
            .collect::<Vec<_>>();
        // Update rooms with new data
        for (id, hash, polygons, material_polygons, material_triangles, wall_polygons) in new_data {
            if let Some(room) = self.rooms.iter_mut().find(|room| room.id == id) {
                room.rendered_data = Some(RoomRender {
                    hash,
                    polygons,
                    material_polygons,
                    material_triangles,
                    wall_polygons,
                });
            }
        }

        // Find furniture to update which have been modified, get (index, id, hash)
        let furniture_to_update = self
            .furniture
            .iter()
            .enumerate()
            .filter_map(|(index, furniture)| {
                let mut hasher = DefaultHasher::new();
                furniture.hash(&mut hasher);
                let hash = hasher.finish();
                if furniture.rendered_data.is_none()
                    || furniture.rendered_data.as_ref().unwrap().hash != hash
                {
                    Some((index, furniture.id, hash))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        // Process all furniture in parallel
        let new_data = furniture_to_update
            .par_iter()
            .map(|(index, id, hash)| {
                let furniture = &self.furniture[*index];
                let (polygons, triangles, shadow_triangles) = furniture.render();
                (*id, *hash, polygons, triangles, shadow_triangles)
            })
            .collect::<Vec<_>>();
        // Update furniture with new data
        for (id, hash, polygons, triangles, shadow_triangles) in new_data {
            if let Some(furniture) = self
                .furniture
                .iter_mut()
                .find(|furniture| furniture.id == id)
            {
                furniture.rendered_data = Some(FurnitureRender {
                    hash,
                    polygons,
                    triangles,
                    shadow_triangles,
                });
            }
        }

        // Collect all the rooms together to build up the walls
        let mut wall_polygons = MultiPolygon(vec![]);
        for room in &self.rooms {
            if let Some(rendered_data) = &room.rendered_data {
                wall_polygons = wall_polygons.difference(&rendered_data.polygons);
                wall_polygons = wall_polygons.union(&rendered_data.wall_polygons);
            }
        }
        // Subtract doors and windows
        for room in &self.rooms {
            for opening in &room.openings {
                let opening_polygon = Shape::Rectangle.polygons(
                    room.pos + opening.pos,
                    vec2(opening.width, WALL_WIDTH * 1.01),
                    opening.rotation,
                );
                wall_polygons = wall_polygons.difference(&opening_polygon);
            }
        }

        // Create triangles for each polygon
        let mut wall_triangles = Vec::new();
        for polygon in &wall_polygons.0 {
            let (indices, vertices) = triangulate_polygon(polygon);
            wall_triangles.push(Triangles { indices, vertices });
        }

        self.rendered_data = Some(HomeRender {
            hash: home_hash,
            wall_polygons,
            wall_triangles,
        });
    }

    pub fn get_global_material(&self, string: &str) -> GlobalMaterial {
        for material in &self.materials {
            if material.name == string {
                return material.clone();
            }
        }
        GlobalMaterial::new(string, Material::Carpet, Color::WHITE)
    }
}

impl Room {
    pub fn self_bounds(&self) -> (Vec2, Vec2) {
        (self.pos - self.size / 2.0, self.pos + self.size / 2.0)
    }

    pub fn bounds(&self) -> (Vec2, Vec2) {
        let (mut min, mut max) = self.self_bounds();

        for operation in &self.operations {
            if operation.action == Action::Add {
                for corner in operation.vertices(self.pos) {
                    min = min.min(corner);
                    max = max.max(corner);
                }
            }
        }

        (min, max)
    }

    pub fn bounds_with_walls(&self) -> (Vec2, Vec2) {
        let (mut min, mut max) = self.bounds();
        min -= vec2(WALL_WIDTH, WALL_WIDTH);
        max += vec2(WALL_WIDTH, WALL_WIDTH);
        (min, max)
    }

    pub fn contains(&self, point: Vec2) -> bool {
        let mut inside = Shape::Rectangle.contains(point, self.pos, self.size, 0.0);
        for operation in &self.operations {
            if operation.contains(self.pos, point) {
                match operation.action {
                    Action::Add => {
                        inside = true;
                    }
                    Action::Subtract => {
                        inside = false;
                    }
                    _ => {}
                }
            }
        }
        inside
    }

    pub fn polygons(&self) -> MultiPolygon {
        let mut polygons = Shape::Rectangle.polygons(self.pos, self.size, 0.0);
        for operation in &self.operations {
            let operation_polygon = operation.polygon(self.pos);
            match operation.action {
                Action::Add => {
                    polygons = polygons.union(&operation_polygon);
                }
                Action::Subtract => {
                    polygons = polygons.difference(&operation_polygon);
                }
                _ => {}
            }
        }
        polygons
    }

    pub fn material_polygons(
        &self,
    ) -> (
        HashMap<String, MultiPolygon>,
        HashMap<String, Vec<Triangles>>,
    ) {
        let mut polygons = HashMap::new();
        polygons.insert(
            self.material.clone(),
            Shape::Rectangle.polygons(self.pos, self.size, 0.0),
        );
        for operation in &self.operations {
            let op_polygon = operation.polygon(self.pos);
            match operation.action {
                Action::Add => {
                    let material = operation
                        .material
                        .clone()
                        .unwrap_or_else(|| self.material.clone());
                    polygons
                        .entry(material.clone())
                        .and_modify(|poly| *poly = poly.union(&op_polygon))
                        .or_insert_with(|| op_polygon.clone());
                    // Remove from all other polygons
                    for (other_material, poly) in &mut polygons {
                        if other_material != &material {
                            *poly = poly.difference(&op_polygon);
                        }
                    }
                }
                Action::Subtract => {
                    for poly in polygons.values_mut() {
                        *poly = poly.difference(&op_polygon);
                    }
                }
                _ => {}
            }
        }

        // Create triangles for each material
        let mut triangles = HashMap::new();
        for (material, poly) in &polygons {
            let mut material_triangles = Vec::new();
            for polygon in &poly.0 {
                let (indices, vertices) = triangulate_polygon(polygon);
                material_triangles.push(Triangles { indices, vertices });
            }
            triangles.insert(material.clone(), material_triangles);
        }

        (polygons, triangles)
    }

    pub fn wall_polygons(&self, polygons: &MultiPolygon) -> MultiPolygon {
        let bounds = self.bounds_with_walls();
        let center = (bounds.0 + bounds.1) / 2.0;
        let size = bounds.1 - bounds.0;

        // Filter out inner polygons
        let mut new_polygons = MultiPolygon(vec![]);
        for polygon in polygons {
            new_polygons = new_polygons.union(&MultiPolygon::new(vec![Polygon::new(
                polygon.exterior().clone(),
                vec![],
            )]));
        }

        let width_half = WALL_WIDTH / 2.0;
        let mut new_polys = MultiPolygon(vec![]);

        let polygon_outside = geo_buffer::buffer_multi_polygon(&new_polygons, width_half);
        let polygon_inside = geo_buffer::buffer_multi_polygon(&new_polygons, -width_half);

        let diff = polygon_outside.difference(&polygon_inside);
        new_polys = new_polys.union(&diff);

        // Subtract operations that are SubtractWall
        for operation in &self.operations {
            if operation.action == Action::SubtractWall {
                new_polys = new_polys.difference(&operation.polygon(self.pos));
            }
        }

        // If walls arent on all sides, trim as needed
        let any_add = self
            .operations
            .iter()
            .any(|operation| operation.action == Action::AddWall);
        if self.walls == Walls::WALL && !any_add {
            return new_polys;
        }

        let up = size.y * 0.5 - width_half * 3.0;
        let right = size.x * 0.5 - width_half * 3.0;

        let mut subtract_shape = EMPTY_MULTI_POLYGON;
        for index in 0..4 {
            if !match index {
                0 => self.walls.left,
                1 => self.walls.top,
                2 => self.walls.right,
                _ => self.walls.bottom,
            } {
                let pos_neg = vec2(1.0, -1.0);
                let neg_pos = vec2(-1.0, 1.0);
                let neg = vec2(-1.0, -1.0);
                let pos = vec2(1.0, 1.0);
                let mut vertices = vec![
                    vec![Vec2::ZERO, neg_pos, vec2(-4.0, 1.0), vec2(-4.0, -1.0), neg], // Left
                    vec![Vec2::ZERO, neg_pos, vec2(-1.0, 4.0), vec2(1.0, 4.0), pos],   // Top
                    vec![Vec2::ZERO, pos, vec2(4.0, 1.0), vec2(4.0, -1.0), pos_neg],   // Right
                    vec![Vec2::ZERO, neg, vec2(-1.0, -4.0), vec2(1.0, -4.0), pos_neg], // Bottom
                ];
                vertices[index]
                    .iter_mut()
                    .for_each(|vertex| *vertex = center + *vertex * vec2(right, up));
                subtract_shape = subtract_shape.union(&create_multipolygon(&vertices[index]));
            }
        }
        // Add corners
        let directions = [(self.walls.left, -right), (self.walls.right, right)];
        let verticals = [(self.walls.top, up), (self.walls.bottom, -up)];
        for (wall_horizontal, horizontal_multiplier) in &directions {
            for (wall_vertical, vertical_multiplier) in &verticals {
                if !wall_horizontal && !wall_vertical {
                    subtract_shape = subtract_shape.union(&create_multipolygon(&[
                        center + vec2(*horizontal_multiplier * 0.9, *vertical_multiplier * 0.9),
                        center + vec2(*horizontal_multiplier * 4.0, *vertical_multiplier * 0.9),
                        center + vec2(*horizontal_multiplier * 4.0, *vertical_multiplier * 4.0),
                        center + vec2(*horizontal_multiplier * 0.9, *vertical_multiplier * 4.0),
                    ]));
                }
            }
        }

        // Add back operations that are AddWall
        for operation in &self.operations {
            if operation.action == Action::AddWall {
                let operation_polygon = operation.polygon(self.pos);
                subtract_shape = subtract_shape.difference(&operation_polygon);
            }
        }

        new_polys.difference(&subtract_shape)
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

    pub fn polygon(&self, room_pos: Vec2) -> MultiPolygon {
        MultiPolygon(vec![Polygon::new(
            geo::LineString::from(
                self.vertices(room_pos)
                    .iter()
                    .map(vec2_to_coord)
                    .collect::<Vec<_>>(),
            ),
            vec![],
        )])
    }
}

impl Furniture {
    pub fn contains(&self, point: Vec2) -> bool {
        Shape::Rectangle.contains(point, self.pos, self.size, self.rotation)
    }
}

pub fn coord_to_vec2(c: geo_types::Point) -> Vec2 {
    vec2(c.x(), c.y())
}

pub const fn vec2_to_coord(v: &Vec2) -> geo_types::Coord {
    geo_types::Coord { x: v.x, y: v.y }
}

pub fn create_multipolygon(vertices: &[Vec2]) -> MultiPolygon {
    MultiPolygon(vec![Polygon::new(
        geo::LineString::from(vertices.iter().map(vec2_to_coord).collect::<Vec<_>>()),
        vec![],
    )])
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

impl Shape {
    pub fn contains(self, point: Vec2, center: Vec2, size: Vec2, rotation: f64) -> bool {
        let point = rotate_point(point, center, rotation);
        match self {
            Self::Rectangle => {
                point.x >= center.x - size.x / 2.0
                    && point.x <= center.x + size.x / 2.0
                    && point.y >= center.y - size.y / 2.0
                    && point.y <= center.y + size.y / 2.0
            }
            Self::Circle => {
                let dx = (point.x - center.x) / (size.x / 2.0);
                let dy = (point.y - center.y) / (size.y / 2.0);
                dx * dx + dy * dy <= 1.0
            }
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

    pub fn vertices(self, pos: Vec2, size: Vec2, rotation: f64) -> Vec<Vec2> {
        let mut vertices = match self {
            Self::Rectangle => vec![(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)],
            Self::Circle => {
                let quality = 90;
                (0..quality)
                    .map(|i| {
                        let angle = (i as f64 / quality as f64) * std::f64::consts::PI * 2.0;
                        (angle.cos() * 0.5, angle.sin() * 0.5)
                    })
                    .collect()
            }
            Self::Triangle => vec![(-0.5, 0.5), (0.5, 0.5), (-0.5, -0.5)],
        }
        .iter()
        .map(|(x_offset, y_offset)| vec2(pos.x + x_offset * size.x, pos.y + y_offset * size.y))
        .collect::<Vec<_>>();
        vertices
            .iter_mut()
            .for_each(|vertex| *vertex = rotate_point(*vertex, pos, -rotation));
        vertices
    }

    pub fn polygon(self, pos: Vec2, size: Vec2, rotation: f64) -> Polygon {
        Polygon::new(
            geo::LineString::from(
                self.vertices(pos, size, rotation)
                    .iter()
                    .map(vec2_to_coord)
                    .collect::<Vec<_>>(),
            ),
            vec![],
        )
    }

    pub fn polygons(self, pos: Vec2, size: Vec2, rotation: f64) -> MultiPolygon {
        MultiPolygon(vec![Polygon::new(
            geo::LineString::from(
                self.vertices(pos, size, rotation)
                    .iter()
                    .map(vec2_to_coord)
                    .collect::<Vec<_>>(),
            ),
            vec![],
        )])
    }
}

#[allow(dead_code)]
impl Walls {
    pub const NONE: Self = Self {
        left: false,
        top: false,
        right: false,
        bottom: false,
    };

    pub const WALL: Self = Self {
        left: true,
        top: true,
        right: true,
        bottom: true,
    };

    pub const fn left(self, is_wall: bool) -> Self {
        Self {
            left: is_wall,
            ..self
        }
    }

    pub const fn top(self, is_wall: bool) -> Self {
        Self {
            top: is_wall,
            ..self
        }
    }

    pub const fn right(self, is_wall: bool) -> Self {
        Self {
            right: is_wall,
            ..self
        }
    }

    pub const fn bottom(self, is_wall: bool) -> Self {
        Self {
            bottom: is_wall,
            ..self
        }
    }
}
