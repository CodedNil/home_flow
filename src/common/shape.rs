use super::{
    layout::{
        Action, Home, HomeRender, Operation, RenderOptions, Room, TileOptions, Vec2, Wall,
        RESOLUTION_FACTOR,
    },
    utils::{hex_to_rgba, point_within_segment},
};
use egui::Color32;
use geo::BooleanOps;
use image::{ImageBuffer, Pixel, Rgba, RgbaImage};
use once_cell::sync::Lazy;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
};
use strum::VariantArray;
use strum_macros::{Display, VariantArray};

const WALL_COLOR: Rgba<u8> = Rgba([130, 80, 20, 255]);
const CHUNK_SIZE: u32 = 32;

impl Home {
    pub fn bounds(&self) -> (Vec2, Vec2) {
        let mut min = Vec2::MAX;
        let mut max = Vec2::MIN;

        for room in &self.rooms {
            let (room_min, room_max) = room.bounds();
            min = min.min(&room_min);
            max = max.max(&room_max);
        }

        (min, max)
    }

    pub fn bounds_with_walls(&self) -> (Vec2, Vec2) {
        let (mut min, mut max) = self.bounds();
        let wall_width = WallType::Exterior.width();
        min = min - Vec2::new(wall_width, wall_width);
        max = max + Vec2::new(wall_width, wall_width);
        (min, max)
    }

    pub fn render(&mut self) {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        let hash = hasher.finish();
        if let Some(rendered_data) = &self.rendered_data {
            if rendered_data.hash == hash {
                return;
            }
        }

        // Calculate the center and size of the home
        let (bounds_min, bounds_max) = self.bounds_with_walls();
        let new_center = (bounds_min + bounds_max) / 2.0;
        let new_size = bounds_max - bounds_min;

        // Calculate the size of the image based on the home size and resolution factor
        let width = new_size.x * RESOLUTION_FACTOR;
        let height = new_size.y * RESOLUTION_FACTOR;

        // Calculate the vertices and walls of the room
        let mut vertices = HashMap::new();
        let mut walls = HashMap::new();
        for room in &self.rooms {
            vertices.insert(room.id, room.vertices());
            walls.insert(room.id, room.walls(&vertices[&room.id]));
        }

        // Create an image buffer with the calculated size, filled with transparent pixels
        let mut image_buffer = ImageBuffer::new(width as u32, height as u32);

        // Load required textures
        let wall_texture = TEXTURES.get(&Material::Wall).unwrap();
        let mut textures = HashMap::new();
        for room in &self.rooms {
            textures
                .entry(&room.render_options.material)
                .or_insert_with(|| TEXTURES.get(&room.render_options.material).unwrap());
            for operation in &room.operations {
                if operation.action == Action::Add {
                    textures
                        .entry(&operation.render_options.material)
                        .or_insert_with(|| TEXTURES.get(&room.render_options.material).unwrap());
                }
            }
        }

        let (image_width, image_height) = (image_buffer.width(), image_buffer.height());
        let num_chunks_x = (image_width + CHUNK_SIZE - 1) / CHUNK_SIZE;
        let num_chunks_y = (image_height + CHUNK_SIZE - 1) / CHUNK_SIZE;
        let total_chunks = num_chunks_x * num_chunks_y;

        // Process each chunk in parallel and collect the results
        let chunks: Vec<_> = (0..total_chunks)
            .into_par_iter()
            .map(|chunk_index| {
                let chunk_x = chunk_index % num_chunks_x;
                let chunk_y = chunk_index / num_chunks_x;

                // Calculate actual size of this chunk, handling the last chunk case
                let chunk_width = std::cmp::min(CHUNK_SIZE, image_width - chunk_x * CHUNK_SIZE);
                let chunk_height = std::cmp::min(CHUNK_SIZE, image_height - chunk_y * CHUNK_SIZE);

                let mut chunk_buffer =
                    vec![Rgba([0, 0, 0, 0]); (chunk_width * chunk_height) as usize];
                let mut chunk_edited = false;
                let start_x = chunk_x * CHUNK_SIZE;
                let start_y = chunk_y * CHUNK_SIZE;

                for chunk_y in 0..chunk_height {
                    for chunk_x in 0..chunk_width {
                        let x = start_x + chunk_x;
                        let y = start_y + chunk_y;

                        let mut pixel_color = Rgba([0, 0, 0, 0]);

                        let point = Vec2 {
                            x: x as f32 / width,
                            y: 1.0 - (y as f32 / height),
                        };
                        let point_in_world = bounds_min + point * new_size;

                        let mut walls_to_check = Vec::new();
                        for room in &self.rooms {
                            let mut is_rooms_pixel = false;
                            if Shape::Rectangle.contains(point_in_world, room.pos, room.size) {
                                if let Some(texture) = textures.get(&room.render_options.material) {
                                    // Calculate the relative position within the room
                                    let point_within_shape =
                                        (point_in_world - room.pos + room.size / 2.0) / room.size;

                                    pixel_color = apply_render_options(
                                        &room.render_options,
                                        texture,
                                        x as f32,
                                        y as f32,
                                        point_within_shape,
                                        room.size.x / room.size.y,
                                    );
                                    chunk_edited = true;
                                    is_rooms_pixel = true;
                                }
                            }
                            for operation in &room.operations {
                                match operation.action {
                                    Action::Add => {
                                        if operation.shape.contains(
                                            point_in_world,
                                            room.pos + operation.pos,
                                            operation.size,
                                        ) {
                                            if let Some(texture) =
                                                textures.get(&operation.render_options.material)
                                            {
                                                // Calculate the relative position within the room
                                                let point_within_shape =
                                                    (point_in_world - room.pos + room.size / 2.0)
                                                        / room.size;

                                                pixel_color = apply_render_options(
                                                    &operation.render_options,
                                                    texture,
                                                    x as f32,
                                                    y as f32,
                                                    point_within_shape,
                                                    room.size.x / room.size.y,
                                                );
                                                chunk_edited = true;
                                                is_rooms_pixel = true;
                                            }
                                        }
                                    }
                                    Action::Subtract => {
                                        if operation.shape.contains(
                                            point_in_world,
                                            room.pos + operation.pos,
                                            operation.size,
                                        ) && is_rooms_pixel
                                        {
                                            pixel_color = Rgba([0, 0, 0, 0]);
                                        }
                                    }
                                }
                            }
                            // Check if within room bounds with walls
                            let (room_min, room_max) = room.bounds_with_walls();
                            if point_in_world.x >= room_min.x
                                && point_in_world.x <= room_max.x
                                && point_in_world.y >= room_min.y
                                && point_in_world.y <= room_max.y
                            {
                                walls_to_check.push(room.id);
                            }
                        }

                        // Walls
                        let mut in_wall = false;
                        for room in walls_to_check {
                            for wall in walls.get(&room).unwrap() {
                                if wall.wall_type != WallType::None
                                    && wall.point_within(point_in_world)
                                {
                                    in_wall = true;
                                    break;
                                }
                            }
                            if in_wall {
                                break;
                            }
                        }
                        if in_wall {
                            let scale = Material::Wall.get_scale() / RESOLUTION_FACTOR;
                            let mut texture_color = *wall_texture.get_pixel(
                                (x as f32 * scale) as u32 % wall_texture.width(),
                                (y as f32 * scale) as u32 % wall_texture.height(),
                            );
                            texture_color.blend(&Rgba([
                                WALL_COLOR[0],
                                WALL_COLOR[1],
                                WALL_COLOR[2],
                                200,
                            ]));

                            pixel_color = texture_color;
                            chunk_edited = true;
                        }

                        chunk_buffer[(chunk_y * chunk_width + chunk_x) as usize] = pixel_color;
                    }
                }
                (
                    start_x,
                    start_y,
                    chunk_width,
                    chunk_height,
                    chunk_buffer,
                    chunk_edited,
                )
            })
            .collect();

        // Combine the chunks back into the main image buffer
        for (start_x, start_y, chunk_width, chunk_height, chunk, chunk_edited) in chunks {
            if !chunk_edited {
                continue;
            }
            for y in 0..chunk_height {
                for x in 0..chunk_width {
                    let pixel = chunk[(y * chunk_width + x) as usize];
                    if start_x + x < image_width && start_y + y < image_height {
                        image_buffer.put_pixel(start_x + x, start_y + y, pixel);
                    }
                }
            }
        }

        self.rendered_data = Some(HomeRender {
            hash,
            texture: image_buffer,
            center: new_center,
            size: new_size,
            vertices,
            walls,
        });
    }
}

impl Room {
    pub fn self_bounds(&self) -> (Vec2, Vec2) {
        (self.pos - self.size / 2.0, self.pos + self.size / 2.0)
    }

    pub fn bounds(&self) -> (Vec2, Vec2) {
        let mut min = self.pos - self.size / 2.0;
        let mut max = self.pos + self.size / 2.0;

        for operation in &self.operations {
            if operation.action == Action::Add {
                let (operation_min, operation_max) = (
                    self.pos + operation.pos - operation.size / 2.0,
                    self.pos + operation.pos + operation.size / 2.0,
                );
                min = min.min(&operation_min);
                max = max.max(&operation_max);
            }
        }

        (min, max)
    }

    pub fn bounds_with_walls(&self) -> (Vec2, Vec2) {
        let (mut min, mut max) = self.bounds();
        let wall_width = WallType::Exterior.width();
        min = min - Vec2::new(wall_width, wall_width);
        max = max + Vec2::new(wall_width, wall_width);
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
                        .contains(point, self.pos + operation.pos, operation.size)
                    {
                        inside = true;
                    }
                }
                Action::Subtract => {
                    if operation
                        .shape
                        .contains(point, self.pos + operation.pos, operation.size)
                    {
                        inside = false;
                        break;
                    }
                }
            }
        }
        inside
    }

    pub fn vertices(&self) -> Vec<Vec2> {
        let mut vertices = Shape::Rectangle.vertices(self.pos, self.size);
        let poly1 = create_polygon(&vertices);
        for operation in &self.operations {
            let operation_vertices = operation
                .shape
                .vertices(self.pos + operation.pos, operation.size);
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

    pub fn walls(&self, vertices: &Vec<Vec2>) -> Vec<Wall> {
        if vertices.is_empty() {
            return Vec::new();
        }

        let mut top_left_index = 0;
        let mut top_right_index = 0;
        let mut bottom_left_index = 0;
        let mut bottom_right_index = 0;

        let top_left_corner = self.pos + Vec2::new(-99999.0, 99999.0);
        let mut top_left_distance = f32::MAX;
        let top_right_corner = self.pos + Vec2::new(99999.0, 99999.0);
        let mut top_right_distance = f32::MAX;
        let bottom_left_corner = self.pos + Vec2::new(-99999.0, -99999.0);
        let mut bottom_left_distance = f32::MAX;
        let bottom_right_corner = self.pos + Vec2::new(99999.0, -99999.0);
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

        vec![
            // Left
            Wall {
                points: get_wall_vertices(top_left_index, bottom_left_index),
                wall_type: self.walls[0],
            },
            // Top
            Wall {
                points: get_wall_vertices(top_right_index, top_left_index),
                wall_type: self.walls[1],
            },
            // Right
            Wall {
                points: get_wall_vertices(bottom_right_index, top_right_index),
                wall_type: self.walls[2],
            },
            // Bottom
            Wall {
                points: get_wall_vertices(bottom_left_index, bottom_right_index),
                wall_type: self.walls[3],
            },
        ]
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
        (x * scale).abs() as u32 % texture.width(),
        (y * scale).abs() as u32 % texture.height(),
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
    pub const fn get_scale(&self) -> f32 {
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

static TEXTURES: Lazy<HashMap<Material, RgbaImage>> = Lazy::new(|| {
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

#[derive(
    Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, VariantArray, Default, Hash,
)]
pub enum Shape {
    #[default]
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

impl Wall {
    pub fn point_within(&self, point: Vec2) -> bool {
        let width = self.wall_type.width();

        let mut min = Vec2::MAX;
        let mut max = Vec2::MIN;
        for point in &self.points {
            min = Vec2::new(min.x.min(point.x - width), min.y.min(point.y - width));
            max = Vec2::new(max.x.max(point.x + width), max.y.max(point.y + width));
        }
        if point.x < min.x || point.x > max.x || point.y < min.y || point.y > max.y {
            return false;
        }

        self.points
            .windows(2)
            .any(|window| point_within_segment(point, window[0], window[1], width))
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, Hash, VariantArray)]
pub enum WallType {
    None,
    Interior,
    Exterior,
}

impl WallType {
    pub const fn width(&self) -> f32 {
        match self {
            Self::None => 0.0,
            Self::Interior => 0.05,
            Self::Exterior => 0.1,
        }
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
        walls: Vec<WallType>,
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
        }
    }
}
