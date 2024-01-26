use super::layout::{Action, Room, Vec2, RESOLUTION_FACTOR};
use image::{ImageBuffer, Rgba};
use noise::{NoiseFn, Perlin};
use serde::{Deserialize, Serialize};

pub struct RoomTexture {
    pub texture: ImageBuffer<Rgba<u8>, Vec<u8>>,
    pub center: Vec2,
    pub size: Vec2,
}

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

    #[allow(clippy::cast_sign_loss)]
    pub fn texture(&self) -> RoomTexture {
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
                        .map_or(0, |noise| (perlin.get([x as f64 * 1.11, y as f64 * 1.11]) * noise) as i32);

                    *pixel = Rgba([
                        (render.color[0] as i32 + noise_value).clamp(0, 255) as u8,
                        (render.color[1] as i32 + noise_value).clamp(0, 255) as u8,
                        (render.color[2] as i32 + noise_value).clamp(0, 255) as u8,
                        render.color[3],
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
                                    .map_or(0, |noise| (perlin.get([x as f64 * 1.11, y as f64 * 1.11]) * noise) as i32);

                                *pixel = Rgba([
                                    (render.color[0] as i32 + noise_value).clamp(0, 255) as u8,
                                    (render.color[1] as i32 + noise_value).clamp(0, 255) as u8,
                                    (render.color[2] as i32 + noise_value).clamp(0, 255) as u8,
                                    render.color[3],
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

        RoomTexture {
            texture: canvas,
            center: new_center,
            size: new_size,
        }
    }
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

#[allow(clippy::trivially_copy_pass_by_ref)]
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
