use super::{
    color::Color,
    furniture::FurnitureRender,
    layout::{
        Action, GlobalMaterial, Home, HomeRender, OpeningType, Operation, Room, RoomRender, Shape,
        Triangles, Walls,
    },
    light_render::{render_room_lighting, LightData},
    utils::{rotate_point_i32, Material},
};
use geo::{
    triangulate_spade::SpadeTriangulationConfig, BoundingRect, Contains, Intersects,
    TriangulateEarcut, TriangulateSpade,
};
use geo_types::{Coord, MultiPolygon, Polygon};
use glam::{dvec2 as vec2, DVec2 as Vec2};
use indexmap::IndexMap;
use rayon::prelude::*;
use std::hash::{DefaultHasher, Hash, Hasher};

pub const WALL_WIDTH: f64 = 0.1;

pub const CLIPPER_PRECISION: f64 = 1e4; // How many decimal places to use for clipper

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
        for (index, hash, polygons, mat_polys, mat_tris, wall_polys, wall_lines) in self
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
                    let (wall_polys, wall_lines) = if room.walls == Walls::NONE && !any_add {
                        (EMPTY_MULTI_POLYGON, vec![])
                    } else {
                        room.wall_polygons(&polygons)
                    };
                    let (mat_polys, mat_tris) = room.material_polygons(&self.materials);
                    Some((
                        index, hash, polygons, mat_polys, mat_tris, wall_polys, wall_lines,
                    ))
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
                    material_polygons: mat_polys,
                    material_triangles: mat_tris,
                    wall_polygons: wall_polys,
                    wall_lines,
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
                    Some((index, hash, furniture.render(&self.materials)))
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
        let mut wall_polygons = vec![];
        let mut wall_lines: Vec<Line> = vec![];
        for room in &self.rooms {
            if let Some(rendered_data) = &room.rendered_data {
                for poly in &mut wall_polygons {
                    *poly = geo::SpadeBoolops::difference(poly, &rendered_data.polygons).unwrap();
                }
                for poly in &rendered_data.wall_polygons {
                    wall_polygons.push(poly.clone().into());
                }
                for poly in &rendered_data.polygons {
                    wall_lines = difference_lines(&wall_lines, poly);
                }
                for line in &rendered_data.wall_lines {
                    wall_lines.push(*line);
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

        let compute_shadows = || polygons_to_shadows(wall_polygons.iter().collect(), 1.0);
        let wall_shadows = self.rendered_data.take().map_or_else(
            || (walls_hash, compute_shadows()),
            |rendered_data| {
                if rendered_data.wall_shadows.0 == walls_hash {
                    rendered_data.wall_shadows
                } else {
                    (walls_hash, compute_shadows())
                }
            },
        );

        self.rendered_data = Some(HomeRender {
            hash: home_hash,
            walls_hash,
            wall_triangles,
            wall_polygons,
            wall_lines,
            wall_shadows,
        });
    }

    pub fn render_lighting(&mut self) {
        let mut hasher = DefaultHasher::new();
        for room in &self.rooms {
            room.hash(&mut hasher);
            room.lights.hash(&mut hasher);
        }
        let hash = hasher.finish();
        if let Some(light_data) = &self.light_data {
            if light_data.hash == hash {
                return;
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        let start = std::time::Instant::now();

        let all_walls = &self
            .rendered_data
            .as_ref()
            .map_or_else(Vec::new, |data| data.wall_lines.clone());
        // Remove walls that have size close to zero
        let all_walls = all_walls
            .iter()
            .filter(|(start, end)| start.distance(*end) > f64::EPSILON)
            .copied()
            .collect::<Vec<_>>();

        let (bounds_min, bounds_max) = self.bounds();
        let light_data = render_room_lighting(bounds_min, bounds_max, self, &all_walls);

        #[cfg(not(target_arch = "wasm32"))]
        log::info!("Lighting render time: {:?}", start.elapsed());

        self.light_data = Some(LightData {
            hash,
            image: light_data.image,
            image_center: light_data.image_center,
            image_size: light_data.image_size,
        });
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

    pub fn contains(&self, point: Vec2) -> bool {
        for room in &self.rooms {
            if room.contains(point) {
                return true;
            }
        }
        false
    }
}

pub fn get_global_material(materials: &[GlobalMaterial], string: &str) -> GlobalMaterial {
    if string.ends_with("-grout") {
        let string = string.trim_end_matches("-grout");
        for material in materials {
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
    for material in materials {
        if material.name == string {
            return material.clone();
        }
    }
    GlobalMaterial::new(string, Material::Carpet, Color::WHITE)
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
        let mut inside = Shape::Rectangle.contains(point, self.pos, self.size, 0);
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
        let mut polygons = Shape::Rectangle.polygons(self.pos, self.size, 0);
        for operation in &self.operations {
            match operation.action {
                Action::Add => {
                    polygons = union_polygons(&polygons, &operation.polygons(self.pos));
                }
                Action::Subtract => {
                    polygons = difference_polygons(&polygons, &operation.polygons(self.pos));
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

                    let (startx, endx) = (bounds.min().x, bounds.max().x);
                    let num_grout_x = ((endx - startx) / tile.spacing).floor() as usize;
                    for i in 0..num_grout_x {
                        let x_pos = (i as f64 - (num_grout_x - 1) as f64 / 2.0) * tile.spacing;
                        let line = Shape::Rectangle.polygons(
                            self.pos + vec2(x_pos, 0.0),
                            vec2(tile.grout_width, self.size.y),
                            0,
                        );
                        new_polygons.push(intersection_polygons(&line, poly));
                    }

                    let (starty, endy) = (bounds.min().y, bounds.max().y);
                    let num_grout_y = ((endy - starty) / tile.spacing).floor() as usize;
                    for i in 0..num_grout_y {
                        let y_pos = (i as f64 - (num_grout_y - 1) as f64 / 2.0) * tile.spacing;
                        let line = Shape::Rectangle.polygons(
                            self.pos + vec2(0.0, y_pos),
                            vec2(self.size.x, tile.grout_width),
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

        (polygons, triangles)
    }

    pub fn wall_polygons(&self, polygons: &MultiPolygon) -> (MultiPolygon, Vec<Line>) {
        let bounds = self.bounds_with_walls();
        let center = (bounds.0 + bounds.1) / 2.0;
        let size = bounds.1 - bounds.0;

        // Filter out inner polygons
        let mut lines = vec![];
        let mut new_polygons = MultiPolygon(vec![]);
        for polygon in polygons {
            new_polygons = union_polygons(
                &new_polygons,
                &MultiPolygon::new(vec![Polygon::new(polygon.exterior().clone(), vec![])]),
            );
            for line in polygon.exterior().lines() {
                lines.push((coord_to_vec2(line.start), coord_to_vec2(line.end)));
            }
        }

        let width_half = WALL_WIDTH / 2.0;

        let mut polygon_outside = EMPTY_MULTI_POLYGON;
        let mut polygon_inside = EMPTY_MULTI_POLYGON;
        for polygon in &new_polygons.0 {
            polygon_outside = union_polygons(
                &polygon_outside,
                &offset_polygon(polygon, width_half, JoinType::Miter),
            );
            polygon_inside = union_polygons(
                &polygon_inside,
                &offset_polygon(polygon, -width_half, JoinType::Miter),
            );
        }

        let mut new_polys = difference_polygons(&polygon_outside, &polygon_inside);

        // Subtract operations that are SubtractWall
        for operation in &self.operations {
            if operation.action == Action::SubtractWall {
                let operation_polygon = operation.polygon(self.pos);
                lines = difference_lines(&lines, &operation_polygon);
                new_polys = difference_polygons(&new_polys, &operation_polygon.into());
            }
        }

        // If walls arent on all sides, trim as needed
        let any_add = self
            .operations
            .iter()
            .any(|operation| operation.action == Action::AddWall);
        if self.walls == Walls::WALL && !any_add {
            return (new_polys, lines);
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
                subtract_shape =
                    union_polygons(&subtract_shape, &create_polygons(&vertices[index]));
            }
        }
        // Add corners
        let directions = [(self.walls.left, -right), (self.walls.right, right)];
        let verticals = [(self.walls.top, up), (self.walls.bottom, -up)];
        for (wall_horizontal, horizontal_multiplier) in &directions {
            for (wall_vertical, vertical_multiplier) in &verticals {
                if !wall_horizontal && !wall_vertical {
                    subtract_shape = union_polygons(
                        &subtract_shape,
                        &create_polygons(&[
                            center + vec2(*horizontal_multiplier * 0.9, *vertical_multiplier * 0.9),
                            center + vec2(*horizontal_multiplier * 4.0, *vertical_multiplier * 0.9),
                            center + vec2(*horizontal_multiplier * 4.0, *vertical_multiplier * 4.0),
                            center + vec2(*horizontal_multiplier * 0.9, *vertical_multiplier * 4.0),
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

        for poly in &subtract_shape {
            lines = difference_lines(&lines, poly);
        }
        (difference_polygons(&new_polys, &subtract_shape), lines)
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

pub struct ShadowTriangles {
    pub indices: Vec<u32>,
    pub vertices: Vec<Vec2>,
    pub inners: Vec<bool>,
}

#[derive(Clone, Copy, PartialEq)]
enum JoinType {
    Miter,
    Round,
}

fn offset_polygon(polygon: &Polygon, offset_size: f64, join_type: JoinType) -> MultiPolygon {
    #[cfg(target_arch = "wasm32")]
    {
        let join_round = join_type == JoinType::Round;
        crate::common::clipper_wasm::offset_polygon_wasm(polygon, offset_size, join_round).unwrap()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let join_type = match join_type {
            JoinType::Miter => geo_clipper::JoinType::Miter(0.0),
            JoinType::Round => geo_clipper::JoinType::Round(0.0),
        };
        geo_clipper::Clipper::offset(
            polygon,
            offset_size,
            join_type,
            geo_clipper::EndType::ClosedPolygon,
            CLIPPER_PRECISION,
        )
    }
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

fn difference_lines(lines: &Vec<Line>, poly: &Polygon) -> Vec<Line> {
    let mut intersects = false;
    for (start, end) in lines {
        let geo_line = geo::Line::new(vec2_to_coord(start), vec2_to_coord(end));
        if poly.intersects(&geo_line) {
            intersects = true;
            break;
        }
    }
    if !intersects {
        return lines.clone();
    }

    // First go through line and on any intersections add a point
    let mut new_lines = vec![];
    for (start, end) in lines {
        // Get all intersections
        let mut intersections = vec![];
        for pline in poly.exterior().lines() {
            let (pstart, pend) = (coord_to_vec2(pline.start), coord_to_vec2(pline.end));
            if let Some(intersect) = line_intersect_point(*start, *end, pstart, pend) {
                intersections.push(intersect);
            }
        }
        // Sort intersections by distance to start of line
        intersections.sort_by(|a, b| start.distance(*a).partial_cmp(&start.distance(*b)).unwrap());

        // Subdivide line at intersections
        if intersections.is_empty() {
            new_lines.push((*start, *end));
        } else {
            let mut current = *start;
            for intersect in intersections {
                new_lines.push((current, intersect));
                current = intersect;
            }
            new_lines.push((current, *end));
        }
    }

    // Loop on new lines and remove any that are inside the polygon
    new_lines
        .into_iter()
        .filter(|(start, end)| !poly.contains(&vec2_to_coord(&((*start + *end) / 2.0))))
        .collect()
}

/// Checks if two lines (p1, p2) and (q1, q2) intersect and returns the point of intersection if there is one.
fn line_intersect_point(p1: Vec2, p2: Vec2, q1: Vec2, q2: Vec2) -> Option<Vec2> {
    let r = p2 - p1;
    let s = q2 - q1;
    let rxs = r.perp_dot(s);
    let delta_pq = q1 - p1;
    let qpxr = delta_pq.perp_dot(r);

    if rxs.abs() < f64::EPSILON {
        // Lines are parallel, could be collinear but no single point of intersection
        return None;
    }

    let t = delta_pq.perp_dot(s) / rxs;
    let u = qpxr / rxs;

    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        Some(p1 + r * t)
    } else {
        None
    }
}

pub type ShadowsData = (Color, Vec<ShadowTriangles>);

pub fn polygons_to_shadows(polygons: Vec<&MultiPolygon>, height: f64) -> ShadowsData {
    let offset_size = height * 0.05;
    let mut shadow_exteriors = EMPTY_MULTI_POLYGON;
    let mut shadow_interiors = EMPTY_MULTI_POLYGON;
    let mut interior_points = Vec::new();
    for multipoly in polygons {
        for poly in multipoly {
            let exterior = offset_polygon(poly, offset_size, JoinType::Round);
            let interior = offset_polygon(poly, -0.025, JoinType::Miter);

            shadow_exteriors = union_polygons(&shadow_exteriors, &exterior);
            shadow_interiors = union_polygons(&shadow_interiors, &interior);

            for p in interior.0.iter().flat_map(|p| p.exterior().points()) {
                interior_points.push(vec2(p.x(), p.y()));
            }
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

    let intensity = 1.0 - height;
    let (low, high) = (80.0, 150.0);
    let shadow_color = Color::from_alpha((low + (high - low) * intensity) as u8);

    (shadow_color, shadow_triangles)
}

impl Shape {
    pub fn contains(self, point: Vec2, center: Vec2, size: Vec2, rotation: i32) -> bool {
        let point = rotate_point_i32(point, center, rotation);
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
                        let angle = (i as f64 / quality as f64) * std::f64::consts::PI * 2.0;
                        (angle.cos() * 0.5, angle.sin() * 0.5)
                    })
                    .collect()
            }
            Self::Triangle => vec![(-0.5, 0.5), (0.5, 0.5), (-0.5, -0.5)],
        }
        .iter()
        .map(|(x_offset, y_offset)| {
            rotate_point_i32(
                vec2(x_offset * size.x, y_offset * size.y),
                Vec2::ZERO,
                -rotation,
            ) + pos
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
