use crate::common::{
    layout::{Action, Home, HomeRender, Operation, Room, RoomRender, Walls},
    utils::rotate_point,
};
use geo::BooleanOps;
use geo_types::{MultiPolygon, Polygon};
use glam::{dvec2 as vec2, DVec2 as Vec2};
use image::RgbaImage;
use once_cell::sync::Lazy;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
};
use strum::VariantArray;
use strum_macros::{Display, VariantArray};

impl Home {
    pub fn render(&mut self) {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        let hash = hasher.finish();
        if let Some(rendered_data) = &self.rendered_data {
            if rendered_data.hash == hash {
                return;
            }
        }

        // Process all rooms in parallel
        let room_polygons = self
            .rooms
            .clone()
            .into_par_iter()
            .enumerate()
            .map(|(index, room)| (index, room.id, room.polygons(), room.material_polygons()))
            .collect::<Vec<_>>();

        // For each rooms polygon, subtract rooms above it
        let room_process_data = {
            let mut room_process_data = HashMap::new();
            for (index, id, polygons, material_polygons) in &room_polygons {
                let mut new_polygons = polygons.clone();
                let mut new_material_polygons = material_polygons.clone();
                for (above_index, _, above_polygons, _) in &room_polygons {
                    if above_index > index {
                        new_polygons = new_polygons.difference(above_polygons);
                        for material in material_polygons.keys() {
                            new_material_polygons.entry(*material).and_modify(|e| {
                                *e = e.difference(above_polygons);
                            });
                        }
                    }
                }
                let room = &self.rooms[*index];
                let wall_polygons = if room.walls == Walls::NONE {
                    EMPTY_MULTI_POLYGON
                } else {
                    let bounds = room.bounds_with_walls();
                    let center = (bounds.0 + bounds.1) / 2.0;
                    let size = bounds.1 - bounds.0;
                    wall_polygons(
                        &new_polygons,
                        center,
                        size,
                        &room.walls,
                        room.pos,
                        &room.operations,
                    )
                };
                room_process_data.insert(*id, (new_polygons, new_material_polygons, wall_polygons));
            }
            room_process_data
        };

        for room in &mut self.rooms {
            let (polygons, material_polygons, wall_polygons) =
                room_process_data.get(&room.id).unwrap().clone();
            room.rendered_data = Some(RoomRender {
                polygons,
                material_polygons,
                wall_polygons,
            });
        }

        self.rendered_data = Some(HomeRender { hash });
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
                let center = self.pos + operation.pos;
                let corners = [
                    center - operation.size / 2.0,
                    vec2(
                        center.x + operation.size.x / 2.0,
                        center.y - operation.size.y / 2.0,
                    ),
                    center + operation.size / 2.0,
                    vec2(
                        center.x - operation.size.x / 2.0,
                        center.y + operation.size.y / 2.0,
                    ),
                ];

                let rotated_corners: Vec<_> = corners
                    .iter()
                    .map(|&corner| rotate_point(corner, center, -operation.rotation))
                    .collect();

                for &corner in &rotated_corners {
                    min = min.min(corner);
                    max = max.max(corner);
                }
            }
        }

        (min, max)
    }

    pub fn bounds_with_walls(&self) -> (Vec2, Vec2) {
        let (mut min, mut max) = self.bounds();
        let wall_width = WallType::Wall.width();
        min -= vec2(wall_width, wall_width);
        max += vec2(wall_width, wall_width);
        (min, max)
    }

    pub fn contains_full(&self, x: f64, y: f64) -> bool {
        let point = vec2(x, y);
        let mut inside = Shape::Rectangle.contains(point, self.pos, self.size, 0.0);
        for operation in &self.operations {
            if operation.shape.contains(
                point,
                self.pos + operation.pos,
                operation.size,
                operation.rotation,
            ) {
                inside = true;
            }
        }
        inside
    }

    pub fn polygons(&self) -> MultiPolygon {
        let mut polygons =
            create_multipolygon(&Shape::Rectangle.vertices(self.pos, self.size, 0.0));
        for operation in &self.operations {
            let operation_polygon = create_multipolygon(&operation.shape.vertices(
                self.pos + operation.pos,
                operation.size,
                operation.rotation,
            ));

            match operation.action {
                Action::Add => {
                    polygons = polygons.union(&operation_polygon);
                }
                Action::Subtract => {
                    polygons = polygons.difference(&operation_polygon);
                }
                Action::SubtractWall => {}
            }
        }

        polygons
    }

    pub fn material_polygons(&self) -> HashMap<Material, MultiPolygon> {
        let mut polygons = HashMap::new();
        polygons.insert(
            self.render_options.material,
            create_multipolygon(&Shape::Rectangle.vertices(self.pos, self.size, 0.0)),
        );
        for operation in &self.operations {
            let operation_polygon = create_multipolygon(&operation.shape.vertices(
                self.pos + operation.pos,
                operation.size,
                operation.rotation,
            ));

            match operation.action {
                Action::Add => {
                    // Operation render_options might be none in which case its the same as the room
                    let material = operation
                        .render_options
                        .clone()
                        .unwrap_or_else(|| self.render_options.clone())
                        .material;
                    polygons
                        .entry(material)
                        .and_modify(|poly| *poly = poly.union(&operation_polygon))
                        .or_insert_with(|| operation_polygon.clone());
                    // Remove from all other polygons
                    for (other_material, poly) in &mut polygons {
                        if *other_material != material {
                            *poly = poly.difference(&operation_polygon);
                        }
                    }
                }
                Action::Subtract => {
                    for poly in polygons.values_mut() {
                        *poly = poly.difference(&operation_polygon);
                    }
                }
                Action::SubtractWall => {}
            }
        }

        polygons
    }
}

#[derive(
    Serialize, Deserialize, Clone, Copy, Display, PartialEq, Eq, Hash, VariantArray, Default,
)]
pub enum Material {
    Wall,
    #[default]
    Carpet,
    Marble,
    Granite,
    Limestone,
    Wood,
    WoodPlanks,
}

impl Material {
    pub const fn get_scale(self) -> f64 {
        match self {
            Self::Carpet => 0.4,
            Self::WoodPlanks => 0.5,
            _ => 0.25,
        }
    }

    pub const fn get_image(&self) -> &[u8] {
        match self {
            Self::Wall => include_bytes!("../../assets/textures/wall.png"),
            Self::Carpet => include_bytes!("../../assets/textures/carpet.png"),
            Self::Marble => include_bytes!("../../assets/textures/marble.png"),
            Self::Granite => include_bytes!("../../assets/textures/granite.png"),
            Self::Limestone => include_bytes!("../../assets/textures/limestone.png"),
            Self::Wood => include_bytes!("../../assets/textures/wood.png"),
            Self::WoodPlanks => include_bytes!("../../assets/textures/wood_planks.png"),
        }
    }
}

pub static TEXTURES: Lazy<HashMap<Material, RgbaImage>> = Lazy::new(|| {
    let mut m = HashMap::new();
    for variant in Material::VARIANTS {
        m.insert(
            *variant,
            image::load_from_memory(variant.get_image())
                .unwrap()
                .into_rgba8(),
        );
    }
    m
});

pub fn coord_to_vec2(c: geo_types::Point<f64>) -> Vec2 {
    vec2(c.x(), c.y())
}

pub fn create_multipolygon(vertices: &[Vec2]) -> MultiPolygon {
    MultiPolygon(vec![Polygon::new(
        geo::LineString::from(
            vertices
                .iter()
                .map(|v| geo_types::Coord { x: v.x, y: v.y })
                .collect::<Vec<_>>(),
        ),
        vec![],
    )])
}

pub const EMPTY_MULTI_POLYGON: MultiPolygon = MultiPolygon(vec![]);

pub fn triangulate_polygon(polygon: &Polygon) -> (Vec<u32>, Vec<Vec2>) {
    // Convert the geo Polygon into the Vec<Vec<Vec<T>>> format expected by flatten
    let mut data = Vec::new();
    let mut exterior_ring = Vec::new();
    for point in polygon.exterior().points() {
        exterior_ring.push(vec![point.x(), point.y()]);
    }
    data.push(exterior_ring);

    for interior in polygon.interiors() {
        let mut interior_ring = Vec::new();
        for point in interior.points() {
            interior_ring.push(vec![point.x(), point.y()]);
        }
        data.push(interior_ring);
    }

    // Use the flatten function to prepare data for earcut
    let (vertices, hole_indices, dims) = earcutr::flatten(&data);

    // Perform triangulation
    let triangle_indices = earcutr::earcut(&vertices, &hole_indices, dims);

    triangle_indices.map_or_else(
        |_| (vec![], vec![]),
        |triangle_indices| {
            // Convert flat vertex list to Vec<glam::Vec2>
            let vertices_vec2: Vec<Vec2> = vertices
                .chunks(dims)
                .map(|chunk| vec2(chunk[0], chunk[1]))
                .collect();
            // Convert triangle indices to Vec<u32>
            let triangle_indices: Vec<u32> = triangle_indices.iter().map(|&i| i as u32).collect();

            (triangle_indices, vertices_vec2)
        },
    )
}

#[derive(
    Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, VariantArray, Default, Hash,
)]
pub enum Shape {
    #[default]
    Rectangle,
    Circle,
    Triangle,
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
                let a = size.x / 2.0;
                let b = size.y / 2.0;
                let dx = (point.x - center.x) / a;
                let dy = (point.y - center.y) / b;

                dx * dx + dy * dy <= 1.0
            }
            Self::Triangle => {
                let base = size.x;
                let height = size.y;
                let hypotenuse_slope = height / base;

                let relative_point_x = point.x - center.x + size.x / 2.0;
                let relative_point_y = center.y - point.y + size.y / 2.0;

                relative_point_x >= 0.0
                    && relative_point_y >= 0.0
                    && relative_point_y <= height
                    && relative_point_y <= (-hypotenuse_slope) * relative_point_x + height
            }
        }
    }

    pub fn vertices(self, pos: Vec2, size: Vec2, rotation: f64) -> Vec<Vec2> {
        match self {
            Self::Rectangle => {
                let mut vertices = vec![
                    vec2(pos.x - size.x / 2.0, pos.y - size.y / 2.0),
                    vec2(pos.x + size.x / 2.0, pos.y - size.y / 2.0),
                    vec2(pos.x + size.x / 2.0, pos.y + size.y / 2.0),
                    vec2(pos.x - size.x / 2.0, pos.y + size.y / 2.0),
                ];
                for vertex in &mut vertices {
                    *vertex = rotate_point(*vertex, pos, -rotation);
                }
                vertices
            }
            Self::Circle => {
                let radius_x = size.x / 2.0;
                let radius_y = size.y / 2.0;
                let quality = 90;
                let mut vertices = Vec::with_capacity(quality);
                for i in 0..quality {
                    let angle = (i as f64 / quality as f64) * std::f64::consts::PI * 2.0;
                    vertices.push(vec2(
                        pos.x + angle.cos() * radius_x,
                        pos.y + angle.sin() * radius_y,
                    ));
                }
                for vertex in &mut vertices {
                    *vertex = rotate_point(*vertex, pos, -rotation);
                }
                vertices
            }
            Self::Triangle => {
                let mut vertices = vec![
                    vec2(pos.x - size.x / 2.0, pos.y + size.y / 2.0), // Right angle at top left
                    vec2(pos.x + size.x / 2.0, pos.y + size.y / 2.0), // Bottom right
                    vec2(pos.x - size.x / 2.0, pos.y - size.y / 2.0), // Top right
                ];
                for vertex in &mut vertices {
                    *vertex = rotate_point(*vertex, pos, -rotation);
                }
                vertices
            }
        }
    }
}

pub fn wall_polygons(
    polygons: &MultiPolygon,
    center: Vec2,
    size: Vec2,
    walls: &Walls,
    room_pos: Vec2,
    operations: &Vec<Operation>,
) -> MultiPolygon {
    // Filter out inner polygons
    let mut new_polygons = MultiPolygon(vec![]);
    for polygon in polygons {
        new_polygons = new_polygons.union(&MultiPolygon::new(vec![Polygon::new(
            polygon.exterior().clone(),
            vec![],
        )]));
    }

    let width_half = WallType::Wall.width() / 2.0;
    let mut new_polys = MultiPolygon(vec![]);

    let polygon_outside = geo_buffer::buffer_multi_polygon(&new_polygons, width_half);
    let polygon_inside = geo_buffer::buffer_multi_polygon(&new_polygons, -width_half);

    let diff = polygon_outside.difference(&polygon_inside);
    new_polys = new_polys.union(&diff);

    // If walls arent on all sides, trim as needed
    if walls == &Walls::WALL {
        return new_polys;
    }

    let up = size.y * 0.5 - width_half * 3.0;
    let right = size.x * 0.5 - width_half * 3.0;
    let vertices = vec![
        // Left
        vec![
            center,
            center + vec2(-right, up),
            center + vec2(-right * 4.0, up),
            center + vec2(-right * 4.0, -up),
            center + vec2(-right, -up),
        ],
        // Top
        vec![
            center,
            center + vec2(-right, up),
            center + vec2(-right, up * 4.0),
            center + vec2(right, up * 4.0),
            center + vec2(right, up),
        ],
        // Right
        vec![
            center,
            center + vec2(right, up),
            center + vec2(right * 4.0, up),
            center + vec2(right * 4.0, -up),
            center + vec2(right, -up),
        ],
        // Bottom
        vec![
            center,
            center + vec2(-right, -up),
            center + vec2(-right, -up * 4.0),
            center + vec2(right, -up * 4.0),
            center + vec2(right, -up),
        ],
    ];
    for index in 0..4 {
        let (wall_type, vertices) = match index {
            0 => (walls.left, vertices[0].clone()),
            1 => (walls.top, vertices[1].clone()),
            2 => (walls.right, vertices[2].clone()),
            _ => (walls.bottom, vertices[3].clone()),
        };
        if wall_type == WallType::None {
            // Subtract the new polygon from wall polygons
            let wall_polygon = create_multipolygon(&vertices);
            new_polys = new_polys.difference(&wall_polygon);
        }
    }

    // Subtract operations that are SubtractWall
    for operation in operations {
        if operation.action == Action::SubtractWall {
            let operation_polygon = create_multipolygon(&operation.shape.vertices(
                room_pos + operation.pos,
                operation.size,
                operation.rotation,
            ));
            new_polys = new_polys.difference(&operation_polygon);
        }
    }

    new_polys
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, Hash, VariantArray)]
pub enum WallType {
    None,
    Wall,
    Door,
    Window,
}

impl WallType {
    pub const fn width(self) -> f64 {
        match self {
            Self::None => 0.0,
            Self::Wall => 0.1,
            Self::Door | Self::Window => 0.05,
        }
    }
}

impl Walls {
    pub const NONE: Self = Self {
        left: WallType::None,
        top: WallType::None,
        right: WallType::None,
        bottom: WallType::None,
    };

    pub const WALL: Self = Self {
        left: WallType::Wall,
        top: WallType::Wall,
        right: WallType::Wall,
        bottom: WallType::Wall,
    };

    pub const fn left(self, wall_type: WallType) -> Self {
        Self {
            left: wall_type,
            ..self
        }
    }

    pub const fn top(self, wall_type: WallType) -> Self {
        Self {
            top: wall_type,
            ..self
        }
    }

    pub const fn right(self, wall_type: WallType) -> Self {
        Self {
            right: wall_type,
            ..self
        }
    }

    pub const fn bottom(self, wall_type: WallType) -> Self {
        Self {
            bottom: wall_type,
            ..self
        }
    }
}
