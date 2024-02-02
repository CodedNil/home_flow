use crate::common::layout::{Action, RenderOptions, Room, RoomRender, RESOLUTION_FACTOR};
use crate::common::shape::{Material, Shape, WallType, TEXTURES};
use glam::{dvec2 as vec2, DVec2 as Vec2};
use image::{ImageBuffer, Pixel, Rgba, RgbaImage};
use rayon::prelude::*;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use super::layout::Home;
use super::shape::wall_polygons;

const WALL_COLOR: Rgba<u8> = Rgba([130, 80, 20, 255]);
const CHUNK_SIZE: u32 = 64;

impl Home {
    pub fn render(&mut self) {
        // Render out all rooms in parallel if their hashes have changed
        let rooms_to_update: Vec<(usize, u64)> = self
            .rooms
            .iter()
            .enumerate()
            .filter_map(|(index, room)| {
                let mut hasher = DefaultHasher::new();
                room.hash(&mut hasher);
                let hash = hasher.finish();
                match &room.rendered_data {
                    Some(rendered_data) if rendered_data.hash == hash => None,
                    _ => Some((index, hash)),
                }
            })
            .collect();

        let new_data: Vec<(usize, u64, RoomRender)> = rooms_to_update
            .into_par_iter()
            .map(|(index, hash)| (index, hash, self.rooms[index].render()))
            .collect();
        for (index, hash, data) in new_data {
            self.rooms[index].rendered_data = Some(RoomRender { hash, ..data });
        }
    }
}

impl Room {
    pub fn render(&self) -> RoomRender {
        // Calculate the center and size of the home
        let (bounds_min, bounds_max) = self.bounds_with_walls();
        let new_center = (bounds_min + bounds_max) / 2.0;
        let new_size = bounds_max - bounds_min;

        // Calculate the size of the image based on the home size and resolution factor
        let width = new_size.x * RESOLUTION_FACTOR;
        let height = new_size.y * RESOLUTION_FACTOR;

        // Calculate the vertices and walls of the room
        let polygons = self.polygons();
        let polygon_values: Vec<_> = polygons.values().cloned().collect();
        let wall_polygons = wall_polygons(&polygon_values);

        // Create an image buffer with the calculated size, filled with transparent pixels
        let mut image_buffer = ImageBuffer::new(width as u32, height as u32);

        // Load required textures
        let wall_texture = TEXTURES.get(&Material::Wall).unwrap();

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

                        let point = vec2(x as f64 / width, 1.0 - (y as f64 / height));
                        let point_in_world = bounds_min + point * new_size;

                        let mut rooms_pixel_color = None;
                        if Shape::Rectangle.contains(point_in_world, self.pos, self.size, 0.0) {
                            if let Some(texture) = TEXTURES.get(&self.render_options.material) {
                                // Calculate the relative position within the room
                                let point_within_shape =
                                    (point_in_world - self.pos + self.size / 2.0) / self.size;

                                rooms_pixel_color = Some(apply_render_options(
                                    &self.render_options,
                                    texture,
                                    x as f64,
                                    y as f64,
                                    point_within_shape,
                                    self.size.x / self.size.y,
                                ));
                                chunk_edited = true;
                            }
                        }
                        for operation in &self.operations {
                            match operation.action {
                                Action::Add => {
                                    if operation.shape.contains(
                                        point_in_world,
                                        self.pos + operation.pos,
                                        operation.size,
                                        operation.rotation,
                                    ) {
                                        let render_options = operation
                                            .render_options
                                            .as_ref()
                                            .map_or(&self.render_options, |render_options| {
                                                render_options
                                            });
                                        if let Some(texture) =
                                            TEXTURES.get(&render_options.material)
                                        {
                                            // Calculate the relative position within the room
                                            let point_within_shape = (point_in_world - self.pos
                                                + self.size / 2.0)
                                                / self.size;

                                            rooms_pixel_color = Some(apply_render_options(
                                                render_options,
                                                texture,
                                                x as f64,
                                                y as f64,
                                                point_within_shape,
                                                self.size.x / self.size.y,
                                            ));
                                            chunk_edited = true;
                                        }
                                    }
                                }
                                Action::Subtract => {
                                    if operation.shape.contains(
                                        point_in_world,
                                        self.pos + operation.pos,
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
                        }

                        // Check if within room bounds with walls
                        // let mut is_wall = false;
                        // for wall in &walls {
                        //     if wall.point_within(point_in_world) {
                        //         is_wall = true;
                        //         break;
                        //     }
                        // }

                        // // Walls
                        // if is_wall {
                        //     let scale = Material::Wall.get_scale() / RESOLUTION_FACTOR;
                        //     let mut texture_color = *wall_texture.get_pixel(
                        //         (x as f64 * scale) as u32 % wall_texture.width(),
                        //         (y as f64 * scale) as u32 % wall_texture.height(),
                        //     );
                        //     texture_color.blend(&Rgba([
                        //         WALL_COLOR[0],
                        //         WALL_COLOR[1],
                        //         WALL_COLOR[2],
                        //         200,
                        //     ]));

                        //     pixel_color = texture_color;
                        //     chunk_edited = true;
                        // }

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

        RoomRender {
            hash: 0,
            texture: image_buffer,
            center: new_center,
            size: new_size,
            polygons,
            wall_polygons,
        }
    }
}

fn apply_render_options(
    render_options: &RenderOptions,
    texture: &RgbaImage,
    x: f64,
    y: f64,
    point: Vec2,
    aspect_ratio: f64,
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
        let tile_scale_x = tile_options.scale as f64;
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
