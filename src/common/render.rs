use crate::common::layout::{Action, Home, HomeRender, RenderOptions, Vec2, RESOLUTION_FACTOR};
use crate::common::shape::{Material, Shape, WallType, TEXTURES};
use image::{ImageBuffer, Pixel, Rgba, RgbaImage};
use rayon::prelude::*;
use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
};

const WALL_COLOR: Rgba<u8> = Rgba([130, 80, 20, 255]);
const CHUNK_SIZE: u32 = 32;

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

        let start = std::time::Instant::now();

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
        let mut room_bounds = HashMap::new();
        for room in &self.rooms {
            room_bounds.insert(room.id, room.bounds_with_walls());
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
                    if let Some(render_options) = &operation.render_options {
                        textures
                            .entry(&render_options.material)
                            .or_insert_with(|| TEXTURES.get(&render_options.material).unwrap());
                    }
                }
            }
        }

        println!("Prepared in {:?}", start.elapsed());

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

                        let mut is_wall = false;
                        for room in &self.rooms {
                            // Check if within rooms bounds using room_bounds HashMap with min and max
                            let (room_min, room_max) = room_bounds[&room.id];
                            if point_in_world.x < room_min.x
                                || point_in_world.x > room_max.x
                                || point_in_world.y < room_min.y
                                || point_in_world.y > room_max.y
                            {
                                continue;
                            }

                            let mut rooms_pixel_color = None;
                            if Shape::Rectangle.contains(point_in_world, room.pos, room.size, 0.0) {
                                if let Some(texture) = textures.get(&room.render_options.material) {
                                    // Calculate the relative position within the room
                                    let point_within_shape =
                                        (point_in_world - room.pos + room.size / 2.0) / room.size;

                                    rooms_pixel_color = Some(apply_render_options(
                                        &room.render_options,
                                        texture,
                                        x as f32,
                                        y as f32,
                                        point_within_shape,
                                        room.size.x / room.size.y,
                                    ));
                                    chunk_edited = true;
                                }
                            }
                            for operation in &room.operations {
                                match operation.action {
                                    Action::Add => {
                                        if operation.shape.contains(
                                            point_in_world,
                                            room.pos + operation.pos,
                                            operation.size,
                                            operation.rotation,
                                        ) {
                                            let render_options =
                                                operation.render_options.as_ref().map_or(
                                                    &room.render_options,
                                                    |render_options| render_options,
                                                );
                                            if let Some(texture) =
                                                textures.get(&render_options.material)
                                            {
                                                // Calculate the relative position within the room
                                                let point_within_shape =
                                                    (point_in_world - room.pos + room.size / 2.0)
                                                        / room.size;

                                                rooms_pixel_color = Some(apply_render_options(
                                                    render_options,
                                                    texture,
                                                    x as f32,
                                                    y as f32,
                                                    point_within_shape,
                                                    room.size.x / room.size.y,
                                                ));
                                                chunk_edited = true;
                                            }
                                        }
                                    }
                                    Action::Subtract => {
                                        if operation.shape.contains(
                                            point_in_world,
                                            room.pos + operation.pos,
                                            operation.size,
                                            operation.rotation,
                                        ) {
                                            rooms_pixel_color = None;
                                        }
                                    }
                                }
                            }
                            if let Some(rooms_pixel_color) = rooms_pixel_color {
                                pixel_color = rooms_pixel_color;
                                is_wall = false;
                            }

                            // Check if within room bounds with walls
                            for wall in walls.get(&room.id).unwrap() {
                                if wall.wall_type != WallType::None
                                    && wall.point_within(point_in_world)
                                {
                                    is_wall = true;
                                    break;
                                }
                            }
                        }

                        // Walls
                        if is_wall {
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

        println!("Processed in {:?}", start.elapsed());

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

        println!("Rendered in {:?}", start.elapsed());

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
        let tile_x = point.x.abs() * tile_scale_x;
        let tile_y = point.y.abs() * tile_scale_y;

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
            let grout_width = tile_options.grout_width;
            if grout_x >= 1.0 - grout_width
                || grout_x < grout_width
                || grout_y >= 1.0 - grout_width
                || grout_y < grout_width
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
