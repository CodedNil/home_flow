use super::layout::Home;
use super::shape::{Material, Shape, EMPTY_MULTI_POLYGON, TEXTURES};
use crate::common::layout::{
    Action, HomeRender, RenderOptions, Room, RoomRender, Walls, RESOLUTION_FACTOR,
};
use crate::common::shape::wall_polygons;
use geo::BooleanOps;
use geo_types::MultiPolygon;
use glam::{dvec2 as vec2, DVec2 as Vec2};
use image::{ImageBuffer, Pixel, Rgba, RgbaImage};
use rayon::prelude::*;
use std::collections::HashMap;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

const CHUNK_SIZE: u32 = 64;

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

        #[cfg(not(target_arch = "wasm32"))]
        let start_time = std::time::Instant::now();

        // Process all rooms in parallel
        let room_polygons = self
            .rooms
            .clone()
            .into_par_iter()
            .enumerate()
            .map(|(index, room)| (index, room.id, room.polygons(), room.material_polygons()))
            .collect::<Vec<_>>();

        #[cfg(not(target_arch = "wasm32"))]
        println!("Processed polygons in {:?}", start_time.elapsed());

        // For each rooms polygon, subtract rooms above it
        let room_process_data = {
            let mut room_process_data = HashMap::new();
            for (index, id, polygons, material_polygons) in &room_polygons {
                let mut new_polygons = polygons.clone();
                let mut new_material_polygons = material_polygons.clone();
                for (above_index, _, above_polygons, _) in &room_polygons {
                    if above_index > index {
                        new_polygons = new_polygons.difference(above_polygons);
                        for material in material_polygons.keys() {
                            new_material_polygons.entry(*material).and_modify(|e| {
                                *e = e.difference(above_polygons);
                            });
                        }
                    }
                }
                let room = &self.rooms[*index];
                let wall_polygons = if room.walls == Walls::NONE {
                    EMPTY_MULTI_POLYGON
                } else {
                    let bounds = room.bounds_with_walls();
                    let center = (bounds.0 + bounds.1) / 2.0;
                    let size = bounds.1 - bounds.0;
                    wall_polygons(&new_polygons, center, size, &room.walls)
                };
                room_process_data.insert(
                    *id,
                    RoomProcess {
                        polygons: new_polygons,
                        material_polygons: new_material_polygons,
                        wall_polygons,
                    },
                );
            }
            room_process_data
        };

        // Render out all rooms in parallel if their hashes have changed
        let rooms_to_update = self
            .rooms
            .iter()
            .enumerate()
            .filter_map(|(index, room)| {
                let mut hasher = DefaultHasher::new();
                room.hash(&mut hasher);
                let hash = hasher.finish();
                match &room.rendered_data {
                    Some(rendered_data) if rendered_data.hash == hash => None,
                    _ => Some((index, room.id, hash)),
                }
            })
            .collect::<Vec<_>>();

        let new_data = rooms_to_update
            .into_par_iter()
            .map(|(index, id, hash)| {
                (
                    id,
                    hash,
                    self.rooms[index].render(room_process_data.get(&id).unwrap().clone()),
                )
            })
            .collect::<Vec<_>>();
        for room in &mut self.rooms {
            let process_data = room_process_data.get(&room.id).unwrap().clone();
            let mut render = None;
            for (id, hash, data) in &new_data {
                if &room.id == id {
                    render = Some(RoomRender {
                        hash: *hash,
                        ..data.clone()
                    });
                }
            }
            if let Some(render) = render {
                room.rendered_data = Some(render);
            } else if let Some(rendered_data) = &mut room.rendered_data {
                rendered_data.polygons = process_data.polygons;
                rendered_data.material_polygons = process_data.material_polygons;
                rendered_data.wall_polygons = process_data.wall_polygons;
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        println!("Rendered in {:?}", start_time.elapsed());

        self.rendered_data = Some(HomeRender { hash });
    }
}

#[derive(Clone)]
pub struct RoomProcess {
    pub polygons: MultiPolygon,
    pub material_polygons: HashMap<Material, MultiPolygon>,
    pub wall_polygons: MultiPolygon,
}

impl Room {
    pub fn render(&self, processed: RoomProcess) -> RoomRender {
        // Calculate the center and size of the home
        let (bounds_min, bounds_max) = self.bounds_with_walls();
        let new_center = (bounds_min + bounds_max) / 2.0;
        let new_size = bounds_max - bounds_min;

        // Calculate the size of the image based on the home size and resolution factor
        let width = new_size.x * RESOLUTION_FACTOR;
        let height = new_size.y * RESOLUTION_FACTOR;

        // Create an image buffer with the calculated size, filled with transparent pixels
        let mut image_buffer = ImageBuffer::new(width as u32, height as u32);

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
            polygons: processed.polygons,
            material_polygons: processed.material_polygons,
            wall_polygons: processed.wall_polygons,
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
