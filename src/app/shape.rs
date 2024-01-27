use super::layout::{
    Action, RenderOptions, Room, RoomRender, RoomSide, TileOptions, Vec2, Wall, WallType,
    RESOLUTION_FACTOR,
};
use anyhow::{anyhow, bail, Result};
use egui::Color32;
use geo::BooleanOps;
use image::{ImageBuffer, Pixel, Rgba, RgbaImage};
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
                    *pixel = apply_render_options(
                        &self.render_options,
                        texture,
                        x as f32,
                        y as f32,
                        point,
                        width / height,
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
                                    *pixel = apply_render_options(
                                        render_options,
                                        texture,
                                        x as f32,
                                        y as f32,
                                        point,
                                        width / height,
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

fn apply_render_options(
    render_options: &RenderOptions,
    texture: &RgbaImage,
    x: f32,
    y: f32,
    point: Vec2,
    aspect_ratio: f32,
) -> Rgba<u8> {
    // Get texture
    let scale = render_options.material.get_scale() * render_options.scale / RESOLUTION_FACTOR;
    let mut texture_color = *texture.get_pixel(
        (x * scale) as u32 % texture.width(),
        (y * scale) as u32 % texture.height(),
    );
    // Tint the texture if a tint color is specified
    if let Some(tint) = render_options.tint {
        texture_color = Rgba([
            (texture_color[0] as f32 * tint.r() as f32 / 255.0) as u8,
            (texture_color[1] as f32 * tint.g() as f32 / 255.0) as u8,
            (texture_color[2] as f32 * tint.b() as f32 / 255.0) as u8,
            texture_color[3],
        ]);
    }
    // Add tiles if specified
    if let Some(tile_options) = &render_options.tiles {
        let tile_scale_x = tile_options.scale as f32;
        let tile_scale_y = (tile_scale_x / aspect_ratio).round();
        let tile_x = point.x * tile_scale_x;
        let tile_y = point.y * tile_scale_y;

        if tile_options.odd_tint.a() > 0 {
            let odd_tile = (tile_x as u32 + tile_y as u32) % 2 == 1;
            if odd_tile {
                let tile_color = tile_options.odd_tint;
                texture_color.blend(&Rgba([
                    tile_color.r(),
                    tile_color.g(),
                    tile_color.b(),
                    tile_color.a(),
                ]));
            }
        }
        // Add grout
        if tile_options.grout_width > 0.0 {
            let grout_x = tile_x % 1.0;
            let grout_y = tile_y % 1.0;
            let grout_width_x = tile_options.grout_width;
            let grout_width_y = grout_width_x * aspect_ratio;
            if grout_x >= 1.0 - grout_width_x
                || grout_x < grout_width_x
                || grout_y >= 1.0 - grout_width_y
                || grout_y < grout_width_y
            {
                let tile_color = tile_options.grout_tint;
                texture_color.blend(&Rgba([
                    tile_color.r(),
                    tile_color.g(),
                    tile_color.b(),
                    tile_color.a(),
                ]));
            }
        }
    }

    texture_color
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Display, PartialEq, Eq, Hash)]
pub enum Material {
    Carpet,
    Marble,
    Granite,
    Wood,
    WoodPlanks,
}

impl Material {
    pub const fn get_scale(&self) -> f32 {
        match self {
            Self::Carpet | Self::Granite | Self::Wood => 25.0,
            Self::Marble | Self::WoodPlanks => 40.0,
        }
    }

    pub const fn get_image(&self) -> &[u8] {
        match self {
            Self::Carpet => include_bytes!("../../assets/textures/carpet.png"),
            Self::Marble => include_bytes!("../../assets/textures/marble.png"),
            Self::Granite => include_bytes!("../../assets/textures/granite.png"),
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
                let mut vertices = Vec::with_capacity(quality);
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

fn hex_to_rgba(hex: &str) -> Result<[u8; 4]> {
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

impl Room {
    pub fn new(
        name: &str,
        pos: Vec2,
        size: Vec2,
        render_options: RenderOptions,
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
            render_options,
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
