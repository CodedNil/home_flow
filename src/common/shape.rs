use super::{
    layout::{Action, Room, Wall, Walls},
    utils::rotate_point,
};
use geo::BooleanOps;
use geo_types::{MultiPolygon, Polygon};
use glam::{dvec2 as vec2, DVec2 as Vec2};
use image::RgbaImage;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, hash::Hash};
use strum::VariantArray;
use strum_macros::{Display, VariantArray};

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
            }
        }

        polygons
    }

    // pub fn walls(&self, vertices: &[Vec2]) -> Vec<Wall> {
    //     if vertices.is_empty() {
    //         return Vec::new();
    //     }

    //     let mut top_left_index = 0;
    //     let mut top_right_index = 0;
    //     let mut bottom_left_index = 0;
    //     let mut bottom_right_index = 0;

    //     let top_left_corner = self.pos + vec2(-99999.0, 99999.0);
    //     let mut top_left_distance = f64::MAX;
    //     let top_right_corner = self.pos + vec2(99999.0, 99999.0);
    //     let mut top_right_distance = f64::MAX;
    //     let bottom_left_corner = self.pos + vec2(-99999.0, -99999.0);
    //     let mut bottom_left_distance = f64::MAX;
    //     let bottom_right_corner = self.pos + vec2(99999.0, -99999.0);
    //     let mut bottom_right_distance = f64::MAX;

    //     for (i, vertex) in vertices.iter().enumerate() {
    //         let distance_top_left = (*vertex - top_left_corner).length();
    //         if distance_top_left < top_left_distance {
    //             top_left_distance = distance_top_left;
    //             top_left_index = i;
    //         }
    //         let distance_top_right = (*vertex - top_right_corner).length();
    //         if distance_top_right < top_right_distance {
    //             top_right_distance = distance_top_right;
    //             top_right_index = i;
    //         }
    //         let distance_bottom_left = (*vertex - bottom_left_corner).length();
    //         if distance_bottom_left < bottom_left_distance {
    //             bottom_left_distance = distance_bottom_left;
    //             bottom_left_index = i;
    //         }
    //         let distance_bottom_right = (*vertex - bottom_right_corner).length();
    //         if distance_bottom_right < bottom_right_distance {
    //             bottom_right_distance = distance_bottom_right;
    //             bottom_right_index = i;
    //         }
    //     }

    //     let get_wall_vertices = |start_index: usize, end_index: usize| -> Vec<Vec2> {
    //         if start_index <= end_index {
    //             vertices[start_index..=end_index].to_vec()
    //         } else {
    //             vertices[start_index..]
    //                 .iter()
    //                 .chain(vertices[..=end_index].iter())
    //                 .copied()
    //                 .collect()
    //         }
    //     };

    //     let mut walls = vec![
    //         (
    //             get_wall_vertices(top_left_index, bottom_left_index),
    //             self.walls.left,
    //         ),
    //         (
    //             get_wall_vertices(top_right_index, top_left_index),
    //             self.walls.top,
    //         ),
    //         (
    //             get_wall_vertices(bottom_right_index, top_right_index),
    //             self.walls.right,
    //         ),
    //         (
    //             get_wall_vertices(bottom_left_index, bottom_right_index),
    //             self.walls.bottom,
    //         ),
    //     ];

    //     let merge1 = merge_walls_if_same_type(&mut walls, 1, 0);
    //     let merge2 = merge_walls_if_same_type(&mut walls, 2, 1);
    //     let merge3 = merge_walls_if_same_type(&mut walls, 3, 2);
    //     let merge4 = merge_walls_if_same_type(&mut walls, 0, 3);

    //     walls
    //         .into_iter()
    //         .filter(|wall| wall.0.len() >= 2 && wall.1 != WallType::None)
    //         .map(|(points, _)| {
    //             let points = points.iter().fold(Vec::new(), |mut acc, &p| {
    //                 if !acc.iter().any(|mp| p.distance(*mp) < f64::EPSILON) {
    //                     acc.push(p);
    //                 }
    //                 acc
    //             });
    //             Wall {
    //                 points,
    //                 closed: (merge1 && merge2 && merge3 && merge4),
    //             }
    //         })
    //         .collect()
    // }
}

// fn merge_walls_if_same_type(walls: &mut [(Vec<Vec2>, WallType)], idx1: usize, idx2: usize) -> bool {
//     if walls[idx1].1 != WallType::None && walls[idx1].1 == walls[idx2].1 {
//         let points_to_extend = walls[idx2].0[1..].to_vec();

//         // Extend the points of the first wall with the points of the second wall, skipping the first point to avoid duplication
//         walls[idx1].0.extend_from_slice(&points_to_extend);
//         walls[idx2].1 = WallType::None;
//         return true;
//     }
//     false
// }

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
            Self::Wall
            | Self::Carpet
            | Self::Marble
            | Self::Granite
            | Self::Limestone
            | Self::Wood
            | Self::WoodPlanks => 40.0,
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

pub fn wall_polygons(polygons: &MultiPolygon) -> MultiPolygon {
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
