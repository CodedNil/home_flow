use super::{
    layout::{Action, Furniture, Home, Operation, RenderOptions, Room, TileOptions, Wall, Walls},
    shape::{Material, Shape},
};
use anyhow::{anyhow, bail, Result};
use egui::Color32;
use glam::{vec2, Vec2};
use std::hash::{Hash, Hasher};

pub fn hash_vec2<H: Hasher>(vec: Vec2, state: &mut H) {
    vec.x.to_bits().hash(state);
    vec.y.to_bits().hash(state);
}

pub const fn vec2_to_egui(vec: Vec2) -> egui::Vec2 {
    egui::Vec2::new(vec.x, vec.y)
}

pub const fn egui_to_vec2(vec: egui::Vec2) -> Vec2 {
    vec2(vec.x, vec.y)
}

pub const fn egui_pos_to_vec2(vec: egui::Pos2) -> Vec2 {
    vec2(vec.x, vec.y)
}

pub const fn vec2_to_egui_pos(vec: Vec2) -> egui::Pos2 {
    egui::pos2(vec.x, vec.y)
}

pub fn point_within_segment(point: Vec2, start: Vec2, end: Vec2, width: f32) -> bool {
    let line_vec = end - start;
    let line_len = line_vec.length();

    if line_len < 0.025 {
        return false;
    }

    // Project 'point' onto the line segment, but keep within the segment
    let n = (point - start).dot(line_vec);
    let t = n / line_len.powi(2);
    if (0.0..=1.0).contains(&t) {
        // Projection is within the segment
        let projection = start + line_vec * t;
        (point - projection).length() <= width
    } else if t < 0.0 {
        let distance_rotated = (point - start).dot(vec2(-line_vec.y, line_vec.x).normalize());
        (t * line_len).abs() < width && distance_rotated.abs() <= width
    } else {
        let distance_rotated = (point - end).dot(vec2(-line_vec.y, line_vec.x).normalize());
        ((t - 1.0) * line_len).abs() < width && distance_rotated.abs() <= width
    }
}

// Helper function to rotate a point around a pivot
pub fn rotate_point(point: Vec2, pivot: Vec2, angle: f32) -> Vec2 {
    let cos_theta = angle.to_radians().cos();
    let sin_theta = angle.to_radians().sin();

    vec2(
        cos_theta * (point.x - pivot.x) - sin_theta * (point.y - pivot.y) + pivot.x,
        sin_theta * (point.x - pivot.x) + cos_theta * (point.y - pivot.y) + pivot.y,
    )
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

fn color_to_string(color: Color32) -> String {
    format!(
        "#{:02x}{:02x}{:02x}{:02x}",
        color.r(),
        color.g(),
        color.b(),
        color.a()
    )
}

impl std::fmt::Display for Home {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut string = String::new();
        for room in &self.rooms {
            string.push_str(format!("{room}\n").as_str());
        }
        write!(f, "{string}")
    }
}

impl Room {
    pub fn new(
        name: &str,
        pos: Vec2,
        size: Vec2,
        render_options: RenderOptions,
        walls: Walls,
        operations: Vec<Operation>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            name: name.to_owned(),
            render_options,
            pos,
            size,
            walls,
            operations,
            rendered_data: None,
        }
    }
}
impl Hash for Room {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_vec2(self.pos, state);
        hash_vec2(self.size, state);
        self.walls.hash(state);
        self.operations.hash(state);
        self.render_options.hash(state);
    }
}
impl std::fmt::Display for Room {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut string = format!(
            "Room: {} - {}x{}m @ {}x{}m\n",
            self.name, self.size.x, self.size.y, self.pos.x, self.pos.y
        );
        for operation in &self.operations {
            let op_string = operation.to_string().replace('\n', "\n        ");
            string.push_str(format!("    Operation: {op_string}\n").as_str());
        }

        // Walls
        string.push_str("    Walls: ");
        for index in 0..4 {
            let (wall_type, wall_side) = match index {
                0 => (self.walls.left, "Left"),
                1 => (self.walls.top, "Top"),
                2 => (self.walls.right, "Right"),
                _ => (self.walls.bottom, "Bottom"),
            };
            string.push_str(format!("[{wall_side}: {wall_type}] ").as_str());
        }
        string.push('\n');

        write!(f, "{string}")
    }
}

impl std::fmt::Display for Furniture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut string = format!(
            "Furniture: {}x{}m @ {}x{}m",
            self.size.x, self.size.y, self.pos.x, self.pos.y
        );
        if self.rotation != 0.0 {
            string.push_str(format!(" - {}°", self.rotation).as_str());
        }
        string.push('\n');

        for child in &self.children {
            string.push_str(format!("    Child: {child}\n").as_str());
        }

        write!(f, "{string}")
    }
}
impl Hash for Furniture {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_vec2(self.pos, state);
        hash_vec2(self.size, state);
        self.rotation.to_bits().hash(state);
        for child in &self.children {
            child.hash(state);
        }
    }
}

impl Operation {
    pub fn new(action: Action, shape: Shape, pos: Vec2, size: Vec2, rotation: f32) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            action,
            shape,
            render_options: None,
            pos,
            size,
            rotation,
        }
    }
}
impl std::fmt::Display for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut string = format!(
            "Operation: {} {} - {}x{}m @ {}x{}m",
            self.action, self.shape, self.size.x, self.size.y, self.pos.x, self.pos.y
        );
        if self.rotation != 0.0 {
            string.push_str(format!(" - {}°", self.rotation).as_str());
        }
        if let Some(render_options) = &self.render_options {
            string.push_str(format!("\nRender options: {render_options}").as_str());
        }

        write!(f, "{string}")
    }
}
impl Hash for Operation {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_vec2(self.pos, state);
        hash_vec2(self.size, state);
        self.rotation.to_bits().hash(state);
        self.action.hash(state);
        self.shape.hash(state);
        self.render_options.hash(state);
    }
}

impl Hash for Wall {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for point in &self.points {
            hash_vec2(*point, state);
        }
        self.wall_type.hash(state);
    }
}

impl RenderOptions {
    pub fn new(
        material: Material,
        scale: f32,
        tint: Option<&str>,
        tiles: Option<TileOptions>,
    ) -> Self {
        let tint = tint.map(|tint| {
            let color = hex_to_rgba(tint).unwrap_or([255, 255, 255, 255]);
            Color32::from_rgba_premultiplied(color[0], color[1], color[2], color[3])
        });
        Self {
            material,
            scale,
            tint,
            tiles,
        }
    }
}
impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            material: Material::default(),
            scale: 1.0,
            tint: None,
            tiles: None,
        }
    }
}
impl std::fmt::Display for RenderOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut string = format!("Material: {}", self.material);
        if (self.scale - 1.0).abs() > f32::EPSILON {
            string.push_str(format!(" - Scale: {}", self.scale).as_str());
        }
        if let Some(tint) = self.tint {
            string.push_str(format!(" - Tint: {}", color_to_string(tint)).as_str());
        }
        if let Some(tiles) = &self.tiles {
            string.push_str(format!(" - Tiles: {tiles}").as_str());
        }
        write!(f, "{string}")
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

impl TileOptions {
    pub fn new(scale: u8, odd_tint: &str, grout_width: f32, grout_tint: &str) -> Self {
        let odd_tint = hex_to_rgba(odd_tint).unwrap_or([255, 255, 255, 255]);
        let grout_tint = hex_to_rgba(grout_tint).unwrap_or([255, 255, 255, 255]);
        Self {
            scale,
            odd_tint: Color32::from_rgba_premultiplied(
                odd_tint[0],
                odd_tint[1],
                odd_tint[2],
                odd_tint[3],
            ),
            grout_width,
            grout_tint: Color32::from_rgba_premultiplied(
                grout_tint[0],
                grout_tint[1],
                grout_tint[2],
                grout_tint[3],
            ),
        }
    }
}
impl std::fmt::Display for TileOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut string = format!("Num: {}", self.scale);
        if self.odd_tint.a() != 0 {
            string.push_str(format!(" - Odd tint: {}", color_to_string(self.odd_tint)).as_str());
        }
        string.push_str(format!(" - Grout width: {}", self.grout_width).as_str());
        if self.grout_tint.a() != 0 {
            string
                .push_str(format!(" - Grout tint: {}", color_to_string(self.grout_tint)).as_str());
        }
        write!(f, "{string}")
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
