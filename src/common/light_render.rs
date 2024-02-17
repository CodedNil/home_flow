use std::f64::consts::PI;

use super::{layout::Light, shape::vec2_to_coord};
use geo::Intersects;
use geo_types::{LineString, MultiPolygon};
use glam::DVec2 as Vec2;
use image::{ImageBuffer, Luma};
use rayon::prelude::*;

const PIXELS_PER_METER: f64 = 40.0;
const CHUNK_SIZE: u32 = 64;

pub struct LightData {
    pub hash: u64,
    pub image: ImageBuffer<Luma<u8>, Vec<u8>>,
    pub image_center: Vec2,
    pub image_size: Vec2,
}

pub fn render_room_lighting(
    bounds_min: Vec2,
    bounds_max: Vec2,
    lights: &[Light],
    polygons: &[MultiPolygon],
) -> LightData {
    let new_center = (bounds_min + bounds_max) / 2.0;
    let new_size = bounds_max - bounds_min;

    // Calculate the size of the image based on the home size and resolution factor
    let width = new_size.x * PIXELS_PER_METER;
    let height = new_size.y * PIXELS_PER_METER;

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

            let mut chunk_buffer = vec![Luma([0]); (chunk_width * chunk_height) as usize];
            let start_x = chunk_x * CHUNK_SIZE;
            let start_y = chunk_y * CHUNK_SIZE;

            for chunk_y in 0..chunk_height {
                for chunk_x in 0..chunk_width {
                    let x = start_x + chunk_x;
                    let y = start_y + chunk_y;

                    let point = Vec2 {
                        x: x as f64 / width,
                        y: 1.0 - (y as f64 / height),
                    };
                    let point_in_world = bounds_min + point * new_size;

                    let mut total_light_intensity = 0.0;
                    for light in lights {
                        let samples = 10; // Number of samples within the light's radius for anti-aliasing
                        let mut sampled_light_intensity = 0.0;

                        for i in 0..samples {
                            // Calculate offset for current sample
                            let angle = 2.0 * PI * (i as f64 / samples as f64);
                            let radius_offset = light.radius * angle.cos();
                            let vertical_offset = light.radius * angle.sin();
                            let sample_light_position =
                                light.pos + Vec2::new(radius_offset, vertical_offset);

                            // Check if the sample light position and pixel intersect with any polygon
                            let mut light_visible = true;
                            for polygon in polygons {
                                if polygon.intersects(&LineString::new(vec![
                                    vec2_to_coord(&sample_light_position),
                                    vec2_to_coord(&point_in_world),
                                ])) {
                                    light_visible = false;
                                    break;
                                }
                            }

                            if !light_visible {
                                continue;
                            }

                            // Calculate distance and intensity for the sample
                            let distance = (point_in_world - sample_light_position).length() * 2.0
                                / (light.intensity * (light.state as f64 / 255.0));
                            let light_intensity = 1.0 / (1.0 + distance * distance);
                            sampled_light_intensity += light_intensity;
                        }

                        // Average the light intensity from all samples
                        let averaged_light_intensity = if samples > 0 {
                            sampled_light_intensity / samples as f64
                        } else {
                            0.0
                        };
                        total_light_intensity += averaged_light_intensity;
                    }
                    let pixel_alpha = ((1.0 - total_light_intensity) * 255.0) as u8;

                    chunk_buffer[(chunk_y * chunk_width + chunk_x) as usize] = Luma([pixel_alpha]);
                }
            }
            (start_x, start_y, chunk_width, chunk_height, chunk_buffer)
        })
        .collect();

    // Combine the chunks back into the main image buffer
    for (start_x, start_y, chunk_width, chunk_height, chunk) in chunks {
        for y in 0..chunk_height {
            for x in 0..chunk_width {
                let pixel = chunk[(y * chunk_width + x) as usize];
                if start_x + x < image_width && start_y + y < image_height {
                    image_buffer.put_pixel(start_x + x, start_y + y, pixel);
                }
            }
        }
    }

    LightData {
        hash: 0,
        image: image_buffer,
        image_center: new_center,
        image_size: new_size,
    }
}
