use super::{layout::Light, shape::Line};
use glam::{dvec2 as vec2, DVec2 as Vec2};
use image::{ImageBuffer, Luma};
use rayon::prelude::*;
use std::f64::consts::PI;

const PIXELS_PER_METER: f64 = 40.0;
const CHUNK_SIZE: usize = 512;
const LIGHT_SAMPLES: u8 = 10; // Number of samples within the light's radius for anti-aliasing

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
    lines: &[&Line],
) -> LightData {
    let new_center = (bounds_min + bounds_max) / 2.0;
    let new_size = bounds_max - bounds_min;

    // Calculate the size of the image based on the home size and resolution factor
    let width = new_size.x * PIXELS_PER_METER;
    let height = new_size.y * PIXELS_PER_METER;

    // Create an image buffer with the calculated size, filled with transparent pixels
    let mut image_buffer = ImageBuffer::new(width as u32, height as u32);
    let image_width = image_buffer.width() as usize;

    image_buffer
        .par_chunks_mut(CHUNK_SIZE)
        .enumerate()
        .for_each(|(chunk_index, chunk)| {
            let start_x = (chunk_index * CHUNK_SIZE) % image_width;
            let start_y = (chunk_index * CHUNK_SIZE) / image_width;

            for (i, pixel) in chunk.iter_mut().enumerate() {
                // Calculate x and y for the current pixel
                let x = (start_x + i % CHUNK_SIZE) % image_width;
                let y = start_y + (start_x + i % CHUNK_SIZE) / image_width;

                // Use x and y here
                let point = vec2(x as f64 / width, 1.0 - (y as f64 / height));
                let point_in_world = bounds_min + point * new_size;

                let mut total_light_intensity = 0.0;
                for light in lights {
                    let distance_to_light = (point_in_world - light.pos).length();
                    let lights_intensity = light.intensity * (light.state as f64 / 255.0);
                    if distance_to_light > lights_intensity * 8.0 {
                        continue;
                    }
                    let mut sampled_light_intensity = 0.0;

                    // Do more samples the closer we are to the light
                    let dynamic_samples = ((LIGHT_SAMPLES as f64
                        * (1.0 - distance_to_light / (lights_intensity * 8.0)))
                        .round() as u8)
                        .max(1);

                    for i in 0..dynamic_samples {
                        // Calculate offset for current sample
                        let sample_light_position = if dynamic_samples == 1 {
                            light.pos
                        } else {
                            let angle = 2.0 * PI * (i as f64 / dynamic_samples as f64);
                            light.pos + vec2(light.radius * angle.cos(), light.radius * angle.sin())
                        };

                        // Check if the sample light position and pixel intersect with any lines
                        let mut light_visible = true;
                        for (p1, p2) in lines {
                            if lines_intersect(*p1, *p2, sample_light_position, point_in_world) {
                                light_visible = false;
                                break;
                            }
                        }

                        if !light_visible {
                            continue;
                        }

                        // Calculate distance and intensity for the sample
                        let distance = (point_in_world - sample_light_position).length() * 2.0
                            / lights_intensity;
                        sampled_light_intensity += 1.0 / (1.0 + distance * distance);
                    }

                    // Average the light intensity from all samples
                    total_light_intensity += sampled_light_intensity / dynamic_samples as f64;
                    if total_light_intensity > 1.0 {
                        total_light_intensity = 1.0;
                        break;
                    }
                }
                let pixel_alpha = ((1.0 - total_light_intensity) * 200.0) as u8;
                *pixel = pixel_alpha;
            }
        });

    LightData {
        hash: 0,
        image: image_buffer,
        image_center: new_center,
        image_size: new_size,
    }
}

/// Checks if two lines (p1, p2) and (q1, q2) intersect.
fn lines_intersect(p1: Vec2, p2: Vec2, q1: Vec2, q2: Vec2) -> bool {
    let r = p2 - p1;
    let s = q2 - q1;
    let rxs = r.perp_dot(s);
    let qpxr = (q1 - p1).perp_dot(r);

    if rxs.abs() < f64::EPSILON {
        // Lines are parallel
        return qpxr.abs() < f64::EPSILON; // Collinear if true, non-intersecting if false
    }

    // Compute t and u to check for intersection
    let t = (q1 - p1).perp_dot(s) / rxs;
    let u = qpxr / rxs;

    (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)
}
