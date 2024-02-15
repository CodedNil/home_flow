use super::{
    color::Color,
    furniture::FurnitureRender,
    layout::{
        Action, GlobalMaterial, Home, HomeRender, OpeningType, Operation, Room, RoomRender, Shape,
        Triangles, Walls,
    },
    utils::{rotate_point, Material},
};
use geo::{
    triangulate_spade::SpadeTriangulationConfig, BooleanOps, BoundingRect, TriangulateEarcut,
    TriangulateSpade,
};
use geo_buffer::{buffer_multi_polygon, buffer_multi_polygon_rounded};
use geo_types::{MultiPolygon, Polygon};
use glam::{dvec2 as vec2, DVec2 as Vec2};
use indexmap::IndexMap;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::hash::{DefaultHasher, Hash, Hasher};

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

        // Process all rooms in parallel
        for (index, hash, polygons, material_polygons, material_triangles, wall_polygons) in self
            .rooms
            .par_iter()
            .enumerate()
            .filter_map(|(index, room)| {
                let mut hasher = DefaultHasher::new();
                room.hash(&mut hasher);
                let hash = hasher.finish();
                if room.rendered_data.is_none() || room.rendered_data.as_ref().unwrap().hash != hash
                {
                    let polygons = room.polygons();
                    let any_add = room.operations.iter().any(|o| o.action == Action::AddWall);
                    let wall_polygons = if room.walls == Walls::NONE && !any_add {
                        EMPTY_MULTI_POLYGON
                    } else {
                        room.wall_polygons(&polygons)
                    };
                    let (mat_polys, mat_tris) = room.material_polygons(&self.materials);
                    Some((index, hash, polygons, mat_polys, mat_tris, wall_polygons))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
        {
            if let Some(room) = self.rooms.get_mut(index) {
                room.rendered_data = Some(RoomRender {
                    hash,
                    polygons,
                    material_polygons,
                    material_triangles,
                    wall_polygons,
                });
            }
        }

        // Process all furniture in parallel
        for (index, hash, (polygons, triangles, shadow_triangles, children)) in self
            .furniture
            .par_iter()
            .enumerate()
            .filter_map(|(index, furniture)| {
                let mut hasher = DefaultHasher::new();
                furniture.hash(&mut hasher);
                let hash = hasher.finish();
                if furniture.rendered_data.is_none()
                    || furniture.rendered_data.as_ref().unwrap().hash != hash
                {
                    Some((index, hash, furniture.render()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
        {
            if let Some(furniture) = self.furniture.get_mut(index) {
                furniture.rendered_data = Some(FurnitureRender {
                    hash,
                    polygons,
                    triangles,
                    shadow_triangles,
                    children,
                });
            }
        }

        // Collect all the rooms together to build up the walls
        let mut wall_polygons: Vec<MultiPolygon> = vec![];
        for room in &self.rooms {
            if let Some(rendered_data) = &room.rendered_data {
                for poly in &mut wall_polygons {
                    *poly = geo::SpadeBoolops::difference(poly, &rendered_data.polygons).unwrap();
                }
                for poly in &rendered_data.wall_polygons {
                    wall_polygons.push(poly.clone().into());
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
                    *poly = geo::SpadeBoolops::difference(poly, &opening_polygon).unwrap();
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

        let mut wall_shadows: Option<(u64, Vec<ShadowTriangles>)> = None;
        let mut hash_match = false;
        if let Some(rendered_data) = self.rendered_data.take() {
            if let Some((existing_hash, _)) = &rendered_data.wall_shadows {
                if *existing_hash == walls_hash {
                    hash_match = true;
                }
            }
            wall_shadows = rendered_data.wall_shadows;
        }

        if !hash_match {
            // If on wasm, don't render out shadows that will be done by the server
            #[cfg(not(target_arch = "wasm32"))]
            {
                wall_shadows = Some((
                    walls_hash,
                    polygons_to_shadows_clipper(wall_polygons.iter().collect()),
                ));
            }
        }

        self.rendered_data = Some(HomeRender {
            hash: home_hash,
            walls_hash,
            wall_triangles,
            wall_polygons,
            wall_shadows,
        });
    }

    pub fn get_global_material(&self, string: &str) -> GlobalMaterial {
        if string.ends_with("-grout") {
            let string = string.trim_end_matches("-grout");
            for material in &self.materials {
                if material.name == string {
                    let tiles_colour = material
                        .tiles
                        .as_ref()
                        .map(|t| t.grout_color)
                        .unwrap_or_default();
                    return GlobalMaterial::new(string, material.material, tiles_colour);
                }
            }
        }
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
        let (min, max) = self.bounds();
        (min - Vec2::ONE * WALL_WIDTH, max + Vec2::ONE * WALL_WIDTH)
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
            match operation.action {
                Action::Add => {
                    polygons = polygons.union(&operation.polygons(self.pos));
                }
                Action::Subtract => {
                    polygons = polygons.difference(&operation.polygons(self.pos));
                }
                _ => {}
            }
        }
        polygons
    }

    pub fn material_polygons(
        &self,
        global_materials: &[GlobalMaterial],
    ) -> (
        IndexMap<String, MultiPolygon>,
        IndexMap<String, Vec<Triangles>>,
    ) {
        let mut polygons = IndexMap::new();
        polygons.insert(
            self.material.clone(),
            Shape::Rectangle.polygons(self.pos, self.size, 0.0),
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
                            *poly = poly.union(&operation.polygons(self.pos));
                        })
                        .or_insert_with(|| operation.polygons(self.pos));
                    // Remove from all other polygons
                    for (other_material, poly) in &mut polygons {
                        if other_material != &material {
                            *poly = poly.difference(&operation.polygons(self.pos));
                        }
                    }
                }
                Action::Subtract => {
                    for poly in polygons.values_mut() {
                        *poly = poly.difference(&operation.polygons(self.pos));
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

                    let (startx, endx) = (bounds.min().x, bounds.max().x);
                    let num_grout_x = ((endx - startx) / tile.spacing).floor() as usize;
                    for i in 0..num_grout_x {
                        let x_pos = (i as f64 - (num_grout_x - 1) as f64 / 2.0) * tile.spacing;
                        let line = Shape::Rectangle.polygons(
                            self.pos + vec2(x_pos, 0.0),
                            vec2(tile.grout_width, self.size.y),
                            0.0,
                        );
                        new_polygons.push(line.intersection(poly));
                    }

                    let (starty, endy) = (bounds.min().y, bounds.max().y);
                    let num_grout_y = ((endy - starty) / tile.spacing).floor() as usize;
                    for i in 0..num_grout_y {
                        let y_pos = (i as f64 - (num_grout_y - 1) as f64 / 2.0) * tile.spacing;
                        let line = Shape::Rectangle.polygons(
                            self.pos + vec2(0.0, y_pos),
                            vec2(self.size.x, tile.grout_width),
                            0.0,
                        );
                        new_polygons.push(line.intersection(poly));
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

        let polygon_outside = buffer_multi_polygon(&new_polygons, width_half);
        let polygon_inside = buffer_multi_polygon(&new_polygons, -width_half);

        let diff = polygon_outside.difference(&polygon_inside);
        new_polys = new_polys.union(&diff);

        // Subtract operations that are SubtractWall
        for operation in &self.operations {
            if operation.action == Action::SubtractWall {
                new_polys = new_polys.difference(&operation.polygons(self.pos));
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
                subtract_shape = subtract_shape.union(&create_polygons(&vertices[index]));
            }
        }
        // Add corners
        let directions = [(self.walls.left, -right), (self.walls.right, right)];
        let verticals = [(self.walls.top, up), (self.walls.bottom, -up)];
        for (wall_horizontal, horizontal_multiplier) in &directions {
            for (wall_vertical, vertical_multiplier) in &verticals {
                if !wall_horizontal && !wall_vertical {
                    subtract_shape = subtract_shape.union(&create_polygons(&[
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
                let operation_polygon = operation.polygons(self.pos);
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

    pub fn polygon(&self, room_pos: Vec2) -> Polygon {
        create_polygon(&self.vertices(room_pos))
    }

    pub fn polygons(&self, room_pos: Vec2) -> MultiPolygon {
        self.polygon(room_pos).into()
    }
}

pub fn coord_to_vec2(c: geo_types::Point) -> Vec2 {
    vec2(c.x(), c.y())
}

pub const fn vec2_to_coord(v: &Vec2) -> geo_types::Coord {
    geo_types::Coord { x: v.x, y: v.y }
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

#[derive(Serialize, Deserialize, Debug)]
pub struct ShadowTriangles {
    pub indices: Vec<u32>,
    pub vertices: Vec<Vec2>,
    pub inners: Vec<bool>,
}

pub fn polygons_to_shadows(polygons: Vec<&MultiPolygon>) -> Vec<ShadowTriangles> {
    let mut shadow_exteriors = EMPTY_MULTI_POLYGON;
    let mut shadow_interiors = EMPTY_MULTI_POLYGON;
    let mut interior_points = Vec::new();
    for poly in polygons {
        let exterior = buffer_multi_polygon_rounded(poly, 0.05);
        let interior = buffer_multi_polygon_rounded(poly, -0.025);

        shadow_exteriors = shadow_exteriors.union(&exterior);
        shadow_interiors = shadow_interiors.union(&interior);

        for p in interior.0.iter().flat_map(|p| p.exterior().points()) {
            interior_points.push(vec2(p.x(), p.y()));
        }
    }
    let shadow_polygons = shadow_exteriors.difference(&shadow_interiors);

    triangulate_shadows(shadow_polygons, &interior_points)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn polygons_to_shadows_clipper(polygons: Vec<&MultiPolygon>) -> Vec<ShadowTriangles> {
    use geo_clipper::Clipper;
    const CLIPPER_PRECISION: f64 = 1e4; // How many decimal places to use for clipper

    let mut shadow_exteriors = EMPTY_MULTI_POLYGON;
    let mut shadow_interiors = EMPTY_MULTI_POLYGON;
    let mut interior_points = Vec::new();
    for poly in polygons {
        let exterior = poly.offset(
            0.05,
            geo_clipper::JoinType::Round(0.0),
            geo_clipper::EndType::ClosedPolygon,
            CLIPPER_PRECISION,
        );
        let interior = poly.offset(
            -0.025,
            geo_clipper::JoinType::Miter(0.0),
            geo_clipper::EndType::ClosedPolygon,
            CLIPPER_PRECISION,
        );

        shadow_exteriors = Clipper::union(&shadow_exteriors, &exterior, CLIPPER_PRECISION);
        shadow_interiors = Clipper::union(&shadow_interiors, &interior, CLIPPER_PRECISION);

        for p in interior.0.iter().flat_map(|p| p.exterior().points()) {
            interior_points.push(vec2(p.x(), p.y()));
        }
    }
    let shadow_polygons =
        Clipper::difference(&shadow_exteriors, &shadow_interiors, CLIPPER_PRECISION);

    triangulate_shadows(shadow_polygons, &interior_points)
}

pub fn triangulate_shadows(
    shadow_polygons: MultiPolygon,
    interior_points: &[Vec2],
) -> Vec<ShadowTriangles> {
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
        let mut inners = Vec::new();
        for vertex in &vertices {
            let is_interior = interior_points.iter().any(|p| p.distance(*vertex) < 0.001);
            inners.push(is_interior);
        }
        shadow_triangles.push(ShadowTriangles {
            indices,
            vertices,
            inners,
        });
    }
    shadow_triangles
}

impl Shape {
    pub fn contains(self, point: Vec2, center: Vec2, size: Vec2, rotation: f64) -> bool {
        let point = rotate_point(point, center, rotation);
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

    pub fn vertices(self, pos: Vec2, size: Vec2, rotation: f64) -> Vec<Vec2> {
        match self {
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
        .map(|(x_offset, y_offset)| {
            rotate_point(
                vec2(x_offset * size.x, y_offset * size.y),
                Vec2::ZERO,
                -rotation,
            ) + pos
        })
        .collect()
    }

    pub fn polygon(self, pos: Vec2, size: Vec2, rotation: f64) -> Polygon {
        create_polygon(&self.vertices(pos, size, rotation))
    }

    pub fn polygons(self, pos: Vec2, size: Vec2, rotation: f64) -> MultiPolygon {
        self.polygon(pos, size, rotation).into()
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
