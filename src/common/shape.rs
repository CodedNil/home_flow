use super::{
    layout::{Action, Room, Wall, Walls},
    utils::{point_within_segment, rotate_point},
};
use geo::BooleanOps;
use glam::{vec2, Vec2};
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

    pub fn contains_full(&self, x: f32, y: f32) -> bool {
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

    pub fn vertices(&self) -> Vec<Vec2> {
        let mut vertices = Shape::Rectangle.vertices(self.pos, self.size, 0.0);
        let mut poly1 = create_polygon(&vertices);
        for operation in &self.operations {
            let operation_vertices = operation.shape.vertices(
                self.pos + operation.pos,
                operation.size,
                operation.rotation,
            );
            let poly2 = create_polygon(&operation_vertices);

            let operated: geo_types::MultiPolygon = match operation.action {
                Action::Add => poly1.union(&poly2),
                Action::Subtract => poly1.difference(&poly2),
            };

            if let Some(polygon) = operated.0.first() {
                vertices = polygon.exterior().points().map(coord_to_vec2).collect();
                poly1 = create_polygon(&vertices);
            }
        }

        vertices
    }

    pub fn walls(&self, vertices: &[Vec2]) -> Vec<Wall> {
        if vertices.is_empty() {
            return Vec::new();
        }

        let mut top_left_index = 0;
        let mut top_right_index = 0;
        let mut bottom_left_index = 0;
        let mut bottom_right_index = 0;

        let top_left_corner = self.pos + vec2(-99999.0, 99999.0);
        let mut top_left_distance = f32::MAX;
        let top_right_corner = self.pos + vec2(99999.0, 99999.0);
        let mut top_right_distance = f32::MAX;
        let bottom_left_corner = self.pos + vec2(-99999.0, -99999.0);
        let mut bottom_left_distance = f32::MAX;
        let bottom_right_corner = self.pos + vec2(99999.0, -99999.0);
        let mut bottom_right_distance = f32::MAX;

        for (i, vertex) in vertices.iter().enumerate() {
            let distance_top_left = (*vertex - top_left_corner).length();
            if distance_top_left < top_left_distance {
                top_left_distance = distance_top_left;
                top_left_index = i;
            }
            let distance_top_right = (*vertex - top_right_corner).length();
            if distance_top_right < top_right_distance {
                top_right_distance = distance_top_right;
                top_right_index = i;
            }
            let distance_bottom_left = (*vertex - bottom_left_corner).length();
            if distance_bottom_left < bottom_left_distance {
                bottom_left_distance = distance_bottom_left;
                bottom_left_index = i;
            }
            let distance_bottom_right = (*vertex - bottom_right_corner).length();
            if distance_bottom_right < bottom_right_distance {
                bottom_right_distance = distance_bottom_right;
                bottom_right_index = i;
            }
        }

        let get_wall_vertices = |start_index: usize, end_index: usize| -> Vec<Vec2> {
            if start_index <= end_index {
                vertices[start_index..=end_index].to_vec()
            } else {
                vertices[start_index..]
                    .iter()
                    .chain(vertices[..=end_index].iter())
                    .copied()
                    .collect()
            }
        };

        let mut walls = vec![
            (
                get_wall_vertices(top_left_index, bottom_left_index),
                self.walls.left,
            ),
            (
                get_wall_vertices(top_right_index, top_left_index),
                self.walls.top,
            ),
            (
                get_wall_vertices(bottom_right_index, top_right_index),
                self.walls.right,
            ),
            (
                get_wall_vertices(bottom_left_index, bottom_right_index),
                self.walls.bottom,
            ),
        ];

        let merge1 = merge_walls_if_same_type(&mut walls, 1, 0);
        let merge2 = merge_walls_if_same_type(&mut walls, 2, 1);
        let merge3 = merge_walls_if_same_type(&mut walls, 3, 2);
        let merge4 = merge_walls_if_same_type(&mut walls, 0, 3);

        walls
            .into_iter()
            .filter(|wall| wall.0.len() >= 2 && wall.1 != WallType::None)
            .map(|(points, _)| {
                let points = points.iter().fold(Vec::new(), |mut acc, &p| {
                    if !acc.iter().any(|mp| p.distance(*mp) < f32::EPSILON) {
                        acc.push(p);
                    }
                    acc
                });
                let mut wall = Wall {
                    points,
                    polygon: vec![],
                    closed: (merge1 && merge2 && merge3 && merge4),
                };
                wall.polygon = wall.to_polygon();
                wall
            })
            .collect()
    }
}

// Helper function to merge two walls if they have the same WallType and are not None
fn merge_walls_if_same_type(walls: &mut [(Vec<Vec2>, WallType)], idx1: usize, idx2: usize) -> bool {
    if walls[idx1].1 != WallType::None && walls[idx1].1 == walls[idx2].1 {
        let points_to_extend = walls[idx2].0[1..].to_vec();

        // Extend the points of the first wall with the points of the second wall, skipping the first point to avoid duplication
        walls[idx1].0.extend_from_slice(&points_to_extend);
        walls[idx2].1 = WallType::None;
        return true;
    }
    false
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
    Wood,
    WoodPlanks,
}

impl Material {
    pub const fn get_scale(self) -> f32 {
        match self {
            Self::Wall
            | Self::Carpet
            | Self::Granite
            | Self::Wood
            | Self::Marble
            | Self::WoodPlanks => 40.0,
        }
    }

    pub const fn get_image(&self) -> &[u8] {
        match self {
            Self::Wall => include_bytes!("../../assets/textures/wall.png"),
            Self::Carpet => include_bytes!("../../assets/textures/carpet.png"),
            Self::Marble => include_bytes!("../../assets/textures/marble.png"),
            Self::Granite => include_bytes!("../../assets/textures/granite.png"),
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

fn coord_to_vec2(c: geo_types::Point<f64>) -> Vec2 {
    vec2(c.x() as f32, c.y() as f32)
}

fn create_polygon(vertices: &[Vec2]) -> geo::Polygon<f64> {
    geo::Polygon::new(
        geo::LineString::from(
            vertices
                .iter()
                .map(|v| geo_types::Coord {
                    x: v.x as f64,
                    y: v.y as f64,
                })
                .collect::<Vec<_>>(),
        ),
        vec![],
    )
}

#[derive(
    Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, VariantArray, Default, Hash,
)]
pub enum Shape {
    #[default]
    Rectangle,
    Circle,
}

impl Shape {
    pub fn contains(self, point: Vec2, center: Vec2, size: Vec2, rotation: f32) -> bool {
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
        }
    }

    pub fn vertices(self, pos: Vec2, size: Vec2, rotation: f32) -> Vec<Vec2> {
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
                    let angle = (i as f32 / quality as f32) * std::f32::consts::PI * 2.0;
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
        }
    }
}

impl Wall {
    pub fn point_within(&self, point: Vec2) -> bool {
        let width = WallType::Wall.width() / 2.0;

        let mut min = Vec2::MAX;
        let mut max = Vec2::MIN;
        for point in &self.points {
            min = vec2(min.x.min(point.x - width), min.y.min(point.y - width));
            max = vec2(max.x.max(point.x + width), max.y.max(point.y + width));
        }
        if point.x < min.x || point.x > max.x || point.y < min.y || point.y > max.y {
            return false;
        }

        self.points
            .windows(2)
            .any(|window| point_within_segment(point, window[0], window[1], width))
    }

    pub fn to_polygon(&self) -> Vec<Vec2> {
        let mut points = self.points.clone();
        let points_len = points.len();
        if points_len < 2 {
            return vec![]; // Return empty if not enough points to form a line
        }

        let width_half = WallType::Wall.width() / 2.0;

        // Handle the simple case of exactly two points
        if points_len == 2 {
            let start = points[0];
            let end = points[1];
            let direction = (end - start).normalize();
            let perp = direction.perp() * width_half;
            let extend_dir = direction * width_half; // Directly compute the extension direction for square caps

            return vec![
                start - extend_dir + perp, // Top left
                start - extend_dir - perp, // Bottom left
                end + extend_dir - perp,   // Bottom right
                end + extend_dir + perp,   // Top right
            ];
        }

        // Extend first and last segment by half the wall width for square caps
        let first_direction = (points[1] - points[0]).normalize();
        let last_direction = (points[points_len - 1] - points[points_len - 2]).normalize();
        let first_perp = first_direction.perp() * width_half;
        let last_perp = last_direction.perp() * width_half;
        points[0] += first_perp;
        points[points_len - 1] -= last_perp;

        let mut polygon = Vec::new();

        // Add vertices for the left side
        for i in 0..points_len - 1 {
            let start = points[i];
            let end = points[i + 1];
            let segment_direction = (end - start).normalize();
            let perp = segment_direction.perp() * -width_half;

            polygon.push(start + perp);
            polygon.push(end + perp);
        }

        polygon
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, Hash, VariantArray)]
pub enum WallType {
    None,
    Wall,
    Door,
    Window,
}

impl WallType {
    pub const fn width(self) -> f32 {
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
