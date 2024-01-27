use super::layout::{
    Action, RenderOptions, Room, RoomRender, RoomSide, Vec2, Wall, WallType, RESOLUTION_FACTOR,
};
use geo::BooleanOps;
use image::{ImageBuffer, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use strum_macros::Display;

impl Room {
    pub fn bounds(&self) -> (Vec2, Vec2) {
        let mut min = self.pos - self.size / 2.0;
        let mut max = self.pos + self.size / 2.0;

        for operation in &self.operations {
            if operation.action == Action::Add {
                let (operation_min, operation_max) = (
                    operation.pos - operation.size / 2.0,
                    operation.pos + operation.size / 2.0,
                );
                min = min.min(&operation_min);
                max = max.max(&operation_max);
            }
        }

        (min, max)
    }

    // Check if the point is inside the room's shape after operations applied
    pub fn contains(&self, x: f32, y: f32) -> bool {
        let point = Vec2 { x, y };
        let mut inside = Shape::Rectangle.contains(point, self.pos, self.size);
        for operation in &self.operations {
            match operation.action {
                Action::Add => {
                    if operation
                        .shape
                        .contains(point, operation.pos, operation.size)
                    {
                        inside = true;
                    }
                }
                Action::Subtract => {
                    if operation
                        .shape
                        .contains(point, operation.pos, operation.size)
                    {
                        inside = false;
                        break;
                    }
                }
            }
        }
        inside
    }

    pub fn render(&self) -> RoomRender {
        // Calculate the center and size of the room
        let (bounds_min, bounds_max) = self.bounds();
        let new_center = (bounds_min + bounds_max) / 2.0;
        let new_size = bounds_max - bounds_min;

        // Calculate the size of the canvas based on the room size and resolution factor
        let width = new_size.x * RESOLUTION_FACTOR;
        let height = new_size.y * RESOLUTION_FACTOR;

        // Create an image buffer with the calculated size, filled with transparent pixels
        let mut canvas = ImageBuffer::new(width as u32, height as u32);

        // Load required textures
        let mut textures: HashMap<Material, RgbaImage> = HashMap::new();
        textures.insert(
            self.render_options.material,
            image::load_from_memory(self.render_options.material.get_image())
                .unwrap()
                .into_rgba8(),
        );
        for operation in &self.operations {
            if operation.action == Action::Add {
                if let Some(render) = &operation.render_options {
                    textures.entry(render.material).or_insert_with(|| {
                        image::load_from_memory(render.material.get_image())
                            .unwrap()
                            .into_rgba8()
                    });
                }
            }
        }

        // Draw the room's shape on the canvas
        for (x, y, pixel) in canvas.enumerate_pixels_mut() {
            let point = Vec2 {
                x: x as f32 / width,
                y: 1.0 - (y as f32 / height),
            };
            let point_in_world = bounds_min + point * new_size;
            if Shape::Rectangle.contains(point_in_world, self.pos, self.size) {
                if let Some(texture) = textures.get(&self.render_options.material) {
                    let scale = self.render_options.material.get_scale() / RESOLUTION_FACTOR;
                    *pixel = *texture.get_pixel(
                        (x as f32 * scale) as u32 % texture.width(),
                        (y as f32 * scale) as u32 % texture.height(),
                    );
                }
            }

            for operation in &self.operations {
                match operation.action {
                    Action::Add => {
                        if let Some(render_options) = &operation.render_options {
                            if operation.shape.contains(
                                point_in_world,
                                operation.pos,
                                operation.size,
                            ) {
                                if let Some(texture) = textures.get(&render_options.material) {
                                    let scale =
                                        render_options.material.get_scale() / RESOLUTION_FACTOR;
                                    *pixel = *texture.get_pixel(
                                        (x as f32 * scale) as u32 % texture.width(),
                                        (y as f32 * scale) as u32 % texture.height(),
                                    );
                                }
                            }
                        }
                    }
                    Action::Subtract => {
                        if operation
                            .shape
                            .contains(point_in_world, operation.pos, operation.size)
                        {
                            *pixel = Rgba([0, 0, 0, 0]);
                        }
                    }
                }
            }
        }

        RoomRender {
            texture: canvas.clone(),
            center: new_center,
            size: new_size,
            vertices: self.vertices(),
        }
    }

    pub fn vertices(&self) -> Vec<Vec2> {
        let mut vertices = Shape::Rectangle.vertices(self.pos, self.size);
        let poly1 = create_polygon(&vertices);
        for operation in &self.operations {
            let operation_vertices = operation.shape.vertices(operation.pos, operation.size);
            let poly2 = create_polygon(&operation_vertices);

            let operated: geo_types::MultiPolygon = match operation.action {
                Action::Add => poly1.union(&poly2),
                Action::Subtract => poly1.difference(&poly2),
            };

            if let Some(polygon) = operated.0.first() {
                vertices = polygon.exterior().points().map(coord_to_vec2).collect();
            } else {
                return Vec::new();
            }
        }
        vertices
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Display, PartialEq, Eq, Hash)]
pub enum Material {
    Carpet,
    Marble,
    Granite,
    Tile,
    TileSmall,
    Wood,
    WoodPlanks,
}

impl Material {
    pub const fn get_scale(&self) -> f32 {
        match self {
            Self::Carpet | Self::Granite | Self::Wood => 25.0,
            Self::Marble | Self::WoodPlanks => 40.0,
            Self::Tile => 80.0,
            Self::TileSmall => 110.0,
        }
    }

    pub const fn get_image(&self) -> &[u8] {
        match self {
            Self::Carpet => include_bytes!("../../assets/textures/carpet.png"),
            Self::Marble => include_bytes!("../../assets/textures/marble.png"),
            Self::Granite => include_bytes!("../../assets/textures/granite.png"),
            Self::Tile => include_bytes!("../../assets/textures/tile.png"),
            Self::TileSmall => include_bytes!("../../assets/textures/tile_small.png"),
            Self::Wood => include_bytes!("../../assets/textures/wood.png"),
            Self::WoodPlanks => include_bytes!("../../assets/textures/wood_planks.png"),
        }
    }
}

const fn vec2_to_coord(v: &Vec2) -> geo_types::Coord<f64> {
    geo_types::Coord {
        x: v.x as f64,
        y: v.y as f64,
    }
}

fn coord_to_vec2(c: geo_types::Point<f64>) -> Vec2 {
    Vec2 {
        x: c.x() as f32,
        y: c.y() as f32,
    }
}

fn create_polygon(vertices: &[Vec2]) -> geo::Polygon<f64> {
    geo::Polygon::new(
        geo::LineString::from(vertices.iter().map(vec2_to_coord).collect::<Vec<_>>()),
        vec![],
    )
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Shape {
    Rectangle,
    Circle,
}

impl Shape {
    fn contains(&self, point: Vec2, center: Vec2, size: Vec2) -> bool {
        match *self {
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

    pub fn vertices(&self, pos: Vec2, size: Vec2) -> Vec<Vec2> {
        match self {
            Self::Rectangle => {
                vec![
                    Vec2 {
                        x: pos.x - size.x / 2.0,
                        y: pos.y - size.y / 2.0,
                    },
                    Vec2 {
                        x: pos.x + size.x / 2.0,
                        y: pos.y - size.y / 2.0,
                    },
                    Vec2 {
                        x: pos.x + size.x / 2.0,
                        y: pos.y + size.y / 2.0,
                    },
                    Vec2 {
                        x: pos.x - size.x / 2.0,
                        y: pos.y + size.y / 2.0,
                    },
                ]
            }
            Self::Circle => {
                let radius_x = size.x / 2.0;
                let radius_y = size.y / 2.0;
                let quality = 90;
                let mut vertices = Vec::new();
                for i in 0..quality {
                    let angle = (i as f32 / quality as f32) * std::f32::consts::PI * 2.0;
                    vertices.push(Vec2 {
                        x: pos.x + angle.cos() * radius_x,
                        y: pos.y + angle.sin() * radius_y,
                    });
                }
                vertices
            }
        }
    }
}

impl std::ops::Add for Vec2 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl std::ops::Sub for Vec2 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl std::ops::Div<f32> for Vec2 {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self {
            x: self.x / rhs,
            y: self.y / rhs,
        }
    }
}
impl std::ops::Div<Self> for Vec2 {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x / rhs.x,
            y: self.y / rhs.y,
        }
    }
}

impl std::ops::Mul<f32> for Vec2 {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}
impl std::ops::Mul<Self> for Vec2 {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x * rhs.x,
            y: self.y * rhs.y,
        }
    }
}

impl Vec2 {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn min(&self, other: &Self) -> Self {
        Self {
            x: self.x.min(other.x),
            y: self.y.min(other.y),
        }
    }

    pub fn max(&self, other: &Self) -> Self {
        Self {
            x: self.x.max(other.x),
            y: self.y.max(other.y),
        }
    }

    pub fn dot(&self, other: &Self) -> f32 {
        self.x * other.x + self.y * other.y
    }

    pub fn normalize(&self) -> Self {
        let length = self.x.hypot(self.y);
        Self {
            x: self.x / length,
            y: self.y / length,
        }
    }

    pub fn length(&self) -> f32 {
        self.x.hypot(self.y)
    }

    pub fn approx_eq(&self, other: &Self) -> bool {
        const EPSILON: f32 = 1e-6;
        (self.x - other.x).abs() < EPSILON && (self.y - other.y).abs() < EPSILON
    }
}

impl Wall {
    pub fn is_mirrored_equal(&self, other: &Self) -> bool {
        (self.start.approx_eq(&other.start) && self.end.approx_eq(&other.end))
            || (self.start.approx_eq(&other.end) && self.end.approx_eq(&other.start))
    }
}

impl RenderOptions {
    pub const fn new(material: Material) -> Self {
        Self {
            material,
            tint: None,
        }
    }
}

impl Room {
    pub fn new(
        name: &str,
        pos: Vec2,
        size: Vec2,
        material: Material,
        wall_data: Vec<(RoomSide, WallType)>,
    ) -> Self {
        // Transform input wall data into Wall structs
        let walls = wall_data
            .into_iter()
            .map(|(side, wall_type)| {
                let (start, end) = calculate_wall_positions(&pos, &size, &side);
                Wall {
                    start,
                    end,
                    wall_type,
                }
            })
            .collect();

        Self {
            name: name.to_owned(),
            render_options: RenderOptions::new(material),
            render: None,
            pos,
            size,
            operations: Vec::new(),
            walls,
        }
    }
}

fn calculate_wall_positions(pos: &Vec2, size: &Vec2, room_side: &RoomSide) -> (Vec2, Vec2) {
    match room_side {
        RoomSide::Left => (
            *pos + Vec2::new(-size.x / 2.0, -size.y / 2.0),
            *pos + Vec2::new(-size.x / 2.0, size.y / 2.0),
        ),
        RoomSide::Top => (
            *pos + Vec2::new(-size.x / 2.0, size.y / 2.0),
            *pos + Vec2::new(size.x / 2.0, size.y / 2.0),
        ),
        RoomSide::Right => (
            *pos + Vec2::new(size.x / 2.0, size.y / 2.0),
            *pos + Vec2::new(size.x / 2.0, -size.y / 2.0),
        ),
        RoomSide::Bottom => (
            *pos + Vec2::new(size.x / 2.0, -size.y / 2.0),
            *pos + Vec2::new(-size.x / 2.0, -size.y / 2.0),
        ),
    }
}
