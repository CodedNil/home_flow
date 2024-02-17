use super::{
    layout::{Light, Room, Walls},
    shape::Line,
    utils::hash_vec2,
};
use glam::{dvec2 as vec2, DVec2 as Vec2};
use image::{ImageBuffer, Luma};
use rayon::prelude::*;
use std::{
    collections::HashMap,
    f64::consts::PI,
    hash::{DefaultHasher, Hash, Hasher},
};
use uuid::Uuid;

const PIXELS_PER_METER: f64 = 40.0;
const CHUNK_SIZE: u32 = 4096;
const LIGHT_SAMPLES: u8 = 16; // Number of samples within the light's radius for anti-aliasing
const MAX_LIGHTS_PER_FRAME: u32 = 1;

pub struct LightData {
    pub hash: u64,
    pub image: ImageBuffer<Luma<u8>, Vec<u8>>,
    pub image_center: Vec2,
    pub image_size: Vec2,
}

pub fn combine_lighting(
    bounds_min: Vec2,
    bounds_max: Vec2,
    rooms: &Vec<Room>,
    hash: u64,
) -> LightData {
    // Calculate the size of the image based on the home size and resolution factor
    let new_center = (bounds_min + bounds_max) / 2.0;
    let new_size = bounds_max - bounds_min;
    let width = new_size.x * PIXELS_PER_METER;
    let height = new_size.y * PIXELS_PER_METER;

    // Create an image buffer with the calculated size, filled with transparent pixels
    let mut image_buffer = ImageBuffer::new(width as u32, height as u32);
    let (image_width, image_height) = (image_buffer.width(), image_buffer.height());

    // Create vec of lights references
    let mut lights = Vec::new();
    for room in rooms {
        for light in &room.lights {
            lights.push(light);
        }
    }

    // For each light, add its image to the buffer
    image_buffer
        .par_chunks_mut(CHUNK_SIZE as usize)
        .enumerate()
        .for_each(|(chunk_index, chunk)| {
            let start_x = (chunk_index as u32 * CHUNK_SIZE) % image_width;
            let start_y = (chunk_index as u32 * CHUNK_SIZE) / image_width;

            for (i, pixel) in chunk.iter_mut().enumerate() {
                let x = (start_x + i as u32 % CHUNK_SIZE) % image_width;
                let y = start_y + (start_x + i as u32 % CHUNK_SIZE) / image_width;
                let world =
                    bounds_min + vec2(x as f64 / width, 1.0 - (y as f64 / height)) * new_size;

                if !rooms.iter().any(|r| r.contains(world)) {
                    *pixel = 0;
                    continue;
                }

                let mut light_intensity: f64 = 0.0;
                for light in &lights {
                    if let Some((_, light_data)) = &light.light_data {
                        let light_image = &light_data.image;

                        // If lights image size doesnt match, skip this one
                        let (w, h) = (light_image.width(), light_image.height());
                        if w != image_width || h != image_height {
                            continue;
                        }

                        let light_pixel = light_image.get_pixel(x, y).0[0];
                        light_intensity += light_pixel as f64 / 255.0;
                        if light_intensity >= 255.0 {
                            light_intensity = 255.0;
                            break;
                        }
                    }
                }
                *pixel = ((1.0 - light_intensity) * 200.0) as u8;
            }
        });

    LightData {
        hash,
        image: image_buffer,
        image_center: new_center,
        image_size: new_size,
    }
}

pub fn render_lighting(
    bounds_min: Vec2,
    bounds_max: Vec2,
    rooms: &Vec<Room>,
    all_walls: &[Line],
) -> (bool, HashMap<Uuid, (u64, LightData)>) {
    let mut cur_changed = 0;
    let mut new_light_data = HashMap::new();
    for room in rooms {
        for light in &room.lights {
            let mut hasher = DefaultHasher::new();
            light.hash(&mut hasher);
            for room in rooms {
                hash_vec2(room.pos, &mut hasher);
                hash_vec2(room.size, &mut hasher);
                room.operations.hash(&mut hasher);
                room.walls.hash(&mut hasher);
            }
            let hash = hasher.finish();

            if light.light_data.is_none() || light.light_data.as_ref().unwrap().0 != hash {
                let light_data = render_light(
                    bounds_min,
                    bounds_max,
                    rooms,
                    all_walls,
                    light,
                    room.pos + light.pos,
                );
                new_light_data.insert(light.id, (hash, light_data));
                cur_changed += 1;
            }
            if cur_changed >= MAX_LIGHTS_PER_FRAME {
                return (false, new_light_data);
            }
        }
    }
    (true, new_light_data)
}

fn render_light(
    bounds_min: Vec2,
    bounds_max: Vec2,
    rooms: &[Room],
    all_walls: &[Line],
    light: &Light,
    light_pos: Vec2,
) -> LightData {
    // Create a vec of walls that this light can see
    let walls_for_light = get_visible_walls(light_pos, all_walls);

    // Calculate the size of the image based on the home size and resolution factor
    let new_center = (bounds_min + bounds_max) / 2.0;
    let new_size = bounds_max - bounds_min;
    let width = new_size.x * PIXELS_PER_METER;
    let height = new_size.y * PIXELS_PER_METER;

    // Calculate the rooms to check against, if lights room is enclosed then only that, if its not then only rooms that arent enclosed
    let mut rooms_to_check = Vec::new();
    let light_room = rooms.iter().find(|room| room.contains(light_pos)).unwrap();
    if light_room.walls == Walls::WALL {
        rooms_to_check.push(light_room);
    } else {
        for room in rooms {
            if room.walls != Walls::WALL {
                rooms_to_check.push(room);
            }
        }
    }

    // Create an image buffer with the calculated size, filled with transparent pixels
    let mut image_buffer = ImageBuffer::new(width as u32, height as u32);
    let image_width = image_buffer.width();

    image_buffer
        .par_chunks_mut(CHUNK_SIZE as usize)
        .enumerate()
        .for_each(|(chunk_index, chunk)| {
            let start_x = (chunk_index as u32 * CHUNK_SIZE) % image_width;
            let start_y = (chunk_index as u32 * CHUNK_SIZE) / image_width;

            for (i, pixel) in chunk.iter_mut().enumerate() {
                let x = (start_x + i as u32 % CHUNK_SIZE) % image_width;
                let y = start_y + (start_x + i as u32 % CHUNK_SIZE) / image_width;
                let world =
                    bounds_min + vec2(x as f64 / width, 1.0 - (y as f64 / height)) * new_size;

                if !rooms_to_check.iter().any(|r| r.contains(world)) {
                    *pixel = 0;
                    continue;
                }

                let mut total_light_intensity = 0.0;

                let light_state_intensity = light.intensity * (light.state as f64 / 255.0);
                let distance_to_light = (world - light_pos).length();
                if distance_to_light > light_state_intensity * 8.0 {
                    continue;
                }
                let mut sampled_light_intensity = 0.0;

                // Do more samples the closer we are to the light
                let dynamic_samples = ((LIGHT_SAMPLES as f64
                    * (1.0 - distance_to_light / (light_state_intensity * 4.0)))
                    .round() as u8)
                    .max(1);

                for i in 0..dynamic_samples {
                    // Calculate offset for current sample
                    let sample_light_position = if dynamic_samples == 1 {
                        light_pos
                    } else {
                        let angle = 2.0 * PI * (i as f64 / dynamic_samples as f64);
                        light_pos + vec2(light.radius * angle.cos(), light.radius * angle.sin())
                    };

                    // Check if the sample light position and pixel intersect with any lines
                    if walls_for_light
                        .iter()
                        .any(|(p1, p2)| lines_intersect(sample_light_position, world, *p1, *p2))
                    {
                        continue;
                    }

                    // Calculate distance and intensity for the sample
                    let distance =
                        (world - sample_light_position).length() * 2.0 / light_state_intensity;
                    sampled_light_intensity += (1.0 / (1.0 + distance * distance)) * 0.75;
                    // Add a little fake light not adhering to inverse square law, for a more natural look
                    sampled_light_intensity += (1.0 / (1.0 + distance)) * 0.25;
                }

                // Average the light intensity from all samples
                total_light_intensity += sampled_light_intensity / dynamic_samples as f64;
                if total_light_intensity > 1.0 {
                    total_light_intensity = 1.0;
                }

                *pixel = (total_light_intensity * 255.0) as u8;
            }
        });

    LightData {
        hash: 0,
        image: image_buffer,
        image_center: new_center,
        image_size: new_size,
    }
}

fn get_visible_walls(light_pos: Vec2, all_walls: &[Line]) -> Vec<Line> {
    let mut visible_walls = Vec::new();
    for (i, &(start, end)) in all_walls.iter().enumerate() {
        let light_distance = start.distance(light_pos).min(end.distance(light_pos));

        // Generate points along the wall for more granular visibility checks
        let points = generate_points_for_wall_segment(start, end);

        // Check if any point on the wall is not blocked by other walls
        if points.iter().any(|&point| {
            !all_walls
                .iter()
                .enumerate()
                .any(|(other_i, &(other_start, other_end))| {
                    i != other_i && lines_intersect(light_pos, point, other_start, other_end)
                })
        }) {
            visible_walls.push((start, end, light_distance));
        }
    }
    // Sort by distance
    visible_walls.sort_by(|(_, _, a), (_, _, b)| a.partial_cmp(b).unwrap());
    // Remove distance from the tuple
    visible_walls
        .iter()
        .map(|(start, end, _)| (*start, *end))
        .collect()
}

const POINTS_DISTANCE: f64 = 0.1; // Distance between points on the wall to check for visibility
fn generate_points_for_wall_segment(start: Vec2, end: Vec2) -> Vec<Vec2> {
    let mut points = vec![start, end];
    let total_distance = start.distance(end);
    if total_distance > POINTS_DISTANCE {
        let direction = (end - start).normalize();
        let num_points = (total_distance / POINTS_DISTANCE).ceil() as usize - 1;
        for i in 1..num_points {
            points.push(start + direction * (POINTS_DISTANCE * i as f64));
        }
    }
    points
}

/// Checks if two lines (p1, p2) and (q1, q2) intersect.
fn lines_intersect(p1: Vec2, p2: Vec2, q1: Vec2, q2: Vec2) -> bool {
    let r = p2 - p1;
    let s = q2 - q1;
    let rxs = r.perp_dot(s);
    let delta_pq = q1 - p1;
    let qpxr = delta_pq.perp_dot(r);

    if rxs.abs() < f64::EPSILON {
        // Lines are parallel
        return qpxr.abs() < f64::EPSILON; // Collinear if true, non-intersecting if false
    }

    // Compute t and u to check for intersection
    let t = delta_pq.perp_dot(s) / rxs;
    let u = qpxr / rxs;

    (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)
}
