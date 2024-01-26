use super::layout::{Action, Room, RoomRender, Vec2, RESOLUTION_FACTOR, RESOLUTION_FACTOR_NOISE};
use geo::BooleanOps;
use image::{ImageBuffer, Rgba};
use noise::{NoiseFn, Perlin};
use serde::{Deserialize, Serialize};

impl Room {
    pub fn bounds(&self) -> (Vec2, Vec2) {
        let mut min = self.pos - self.size / 2.0;
        let mut max = self.pos + self.size / 2.0;

        for operation in &self.operations {
            if operation.action == Action::Add {
                let (operation_min, operation_max) = (operation.pos - operation.size / 2.0, operation.pos + operation.size / 2.0);
                min = min.min(operation_min);
                max = max.max(operation_max);
            }
        }

        (min, max)
    }

    // Check if the point is inside the room's shape after operations applied
    pub fn contains(&self, x: f32, y: f32) -> bool {
        let point = Vec2 { x, y };
        let mut inside = self.shape.contains(point, self.pos, self.size);
        for operation in &self.operations {
            match operation.action {
                Action::Add => {
                    if operation.shape.contains(point, operation.pos, operation.size) {
                        inside = true;
                    }
                }
                Action::Subtract => {
                    if operation.shape.contains(point, operation.pos, operation.size) {
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

        let perlin = Perlin::new(1230);

        // Draw the room's shape on the canvas
        for (x, y, pixel) in canvas.enumerate_pixels_mut() {
            let point = Vec2 {
                x: x as f32 / width,
                y: y as f32 / height,
            };
            let point_in_world = bounds_min + point * new_size;
            if let Some(render) = &self.render_options {
                if self.shape.contains(point_in_world, self.pos, self.size) {
                    let noise_value = render
                        .noise
                        .map_or(0, |noise| generate_fixed_resolution_noise(&perlin, x as f64, y as f64, noise));

                    *pixel = Rgba([
                        (render.color[0] as i32 + noise_value).clamp(0, 255) as u8,
                        (render.color[1] as i32 + noise_value).clamp(0, 255) as u8,
                        (render.color[2] as i32 + noise_value).clamp(0, 255) as u8,
                        255,
                    ]);
                }
            }

            for operation in &self.operations {
                match operation.action {
                    Action::Add => {
                        if let Some(render) = &operation.render_options {
                            if operation.shape.contains(point_in_world, operation.pos, operation.size) {
                                let noise_value = render
                                    .noise
                                    .map_or(0, |noise| generate_fixed_resolution_noise(&perlin, x as f64, y as f64, noise));

                                *pixel = Rgba([
                                    (render.color[0] as i32 + noise_value).clamp(0, 255) as u8,
                                    (render.color[1] as i32 + noise_value).clamp(0, 255) as u8,
                                    (render.color[2] as i32 + noise_value).clamp(0, 255) as u8,
                                    255,
                                ]);
                            }
                        }
                    }
                    Action::Subtract => {
                        if operation.shape.contains(point_in_world, operation.pos, operation.size) {
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
        }
    }

    pub fn vertices(&self) -> Vec<Vec2> {
        let mut vertices = self.shape.vertices(self.pos, self.size);
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

fn generate_fixed_resolution_noise(perlin: &Perlin, x: f64, y: f64, amount: f64) -> i32 {
    let base_factor = RESOLUTION_FACTOR as f64;
    let x_rounded = (x / base_factor * RESOLUTION_FACTOR_NOISE).floor() * base_factor / RESOLUTION_FACTOR_NOISE;
    let y_rounded = (y / base_factor * RESOLUTION_FACTOR_NOISE).floor() * base_factor / RESOLUTION_FACTOR_NOISE;
    (perlin.get([x_rounded * 1.11, y_rounded * 1.11]) * amount) as i32
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
                    && point.x < center.x + size.x / 2.0
                    && point.y >= center.y - size.y / 2.0
                    && point.y < center.y + size.y / 2.0
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
    pub fn min(&self, other: Self) -> Self {
        Self {
            x: self.x.min(other.x),
            y: self.y.min(other.y),
        }
    }

    pub fn max(&self, other: Self) -> Self {
        Self {
            x: self.x.max(other.x),
            y: self.y.max(other.y),
        }
    }
}
