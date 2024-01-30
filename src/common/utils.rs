use super::layout::{Furniture, RenderOptions, TileOptions, Vec2};
use anyhow::{anyhow, bail, Result};
use std::hash::{Hash, Hasher};

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

    pub fn min(self, other: Self) -> Self {
        Self {
            x: self.x.min(other.x),
            y: self.y.min(other.y),
        }
    }

    pub fn max(self, other: Self) -> Self {
        Self {
            x: self.x.max(other.x),
            y: self.y.max(other.y),
        }
    }

    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y
    }

    pub fn normalize(self) -> Self {
        let length = self.x.hypot(self.y);
        Self {
            x: self.x / length,
            y: self.y / length,
        }
    }

    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }

    pub const MIN: Self = Self {
        x: std::f32::MIN,
        y: std::f32::MIN,
    };
    pub const MAX: Self = Self {
        x: std::f32::MAX,
        y: std::f32::MAX,
    };
}

impl Hash for Vec2 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.x.to_bits().hash(state);
        self.y.to_bits().hash(state);
    }
}

pub fn point_within_segment(point: Vec2, start: Vec2, end: Vec2, width: f32) -> bool {
    let line_vec = end - start;
    let line_len = line_vec.length();

    if line_len == 0.0 {
        // Line segment is a point
        return point.x < start.x + width
            && point.x > start.x - width
            && point.y < start.y + width
            && point.y > start.y - width;
    }

    // Project 'point' onto the line segment, but keep within the segment
    let n = (point - start).dot(line_vec);
    let t = n / line_len.powi(2);
    if (0.0..=1.0).contains(&t) {
        // Projection is within the segment
        let projection = start + line_vec * t;
        (point - projection).length() <= width
    } else if t < 0.0 {
        let distance_rotated = (point - start).dot(Vec2::new(-line_vec.y, line_vec.x).normalize());
        (t * line_len).abs() < width && distance_rotated.abs() <= width
    } else {
        let distance_rotated = (point - end).dot(Vec2::new(-line_vec.y, line_vec.x).normalize());
        ((t - 1.0) * line_len).abs() < width && distance_rotated.abs() <= width
    }
}

pub fn hex_to_rgba(hex: &str) -> Result<[u8; 4]> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 && hex.len() != 8 {
        bail!("Invalid hex color");
    }

    let parse_color = |i: usize| -> Result<u8> {
        u8::from_str_radix(&hex[i..i + 2], 16)
            .map_err(|_| anyhow!("Invalid value for color component"))
    };

    let r = parse_color(0)?;
    let g = parse_color(2)?;
    let b = parse_color(4)?;
    let a = if hex.len() == 8 { parse_color(6)? } else { 255 };

    Ok([r, g, b, a])
}

impl Hash for Furniture {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.pos.hash(state);
        self.size.hash(state);
        self.rotation.to_bits().hash(state);
        for child in &self.children {
            child.hash(state);
        }
    }
}

impl Hash for RenderOptions {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.material.hash(state);
        self.scale.to_bits().hash(state);
        self.tint.hash(state);
        self.tiles.hash(state);
    }
}

impl Hash for TileOptions {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.scale.hash(state);
        self.odd_tint.hash(state);
        self.grout_width.to_bits().hash(state);
        self.grout_tint.hash(state);
    }
}
