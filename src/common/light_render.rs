use super::{
    layout::{Light, Room, Walls},
    shape::Line,
    utils::hash_vec2,
};
use glam::{dvec2 as vec2, DVec2 as Vec2};
use std::{
    collections::HashMap,
    f64::consts::PI,
    hash::{DefaultHasher, Hash, Hasher},
};
use uuid::Uuid;

const PIXELS_PER_METER: f64 = 40.0;
const LIGHT_SAMPLES: u8 = 12; // Number of samples within the light's radius for anti-aliasing

pub struct LightData {
    pub hash: u64,
    pub image: Vec<u8>,
    pub image_center: Vec2,
    pub image_size: Vec2,
    pub image_width: u32,
    pub image_height: u32,
}

pub struct Bounds {
    pub min: Vec2,
    pub max: Vec2,
}

impl Bounds {
    pub fn contains(&self, point: Vec2) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }
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
    let image_width = width as u32;
    let image_height = height as u32;
    let image_pixel_count = (image_width * image_height) as usize;
    let mut data_buffer = vec![0; image_pixel_count * 4];

    // Create vec of lights references
    let mut lights_data = Vec::new();
    for room in rooms {
        for light in &room.lights {
            if light.state == 0 {
                continue;
            }
            if let Some((_, (light_data, light_bounds))) = &light.light_data {
                if light_data.len() == image_pixel_count {
                    lights_data.push((
                        light.intensity * (light.state as f64 / 255.0),
                        room.pos + light.pos,
                        light_data,
                        light_bounds,
                    ));
                }
            }
        }
    }

    // For each light, add its image to the buffer
    data_buffer
        .chunks_mut(4)
        .enumerate()
        .for_each(|(i, chunk)| {
            let x = i as u32 % image_width;
            let y = i as u32 / image_width;
            let world = bounds_min + vec2(x as f64 / width, 1.0 - (y as f64 / height)) * new_size;

            if !rooms.iter().any(|r| r.contains(world)) {
                return;
            }

            let mut total_light_intensity: f64 = 0.0;
            for (light_intensity, light_pos, light_image, light_bounds) in &lights_data {
                // Quick early exit if outside bounds
                if !light_bounds.contains(world) {
                    continue;
                }
                let distance = world.distance(*light_pos) * 2.0 / light_intensity;
                let light_pixel = light_image[i] as f64;
                // Use greater than inverse square law, since no bouncing light
                total_light_intensity += light_pixel / distance.powf(4.0);
                if total_light_intensity >= 255.0 {
                    total_light_intensity = 255.0;
                    break;
                }
            }
            chunk[3] = ((255.0 - total_light_intensity) * 0.8) as u8;
        });

    LightData {
        hash,
        image: data_buffer,
        image_center: new_center,
        image_size: new_size,
        image_width,
        image_height,
    }
}

pub fn render_lighting(
    bounds_min: Vec2,
    bounds_max: Vec2,
    rooms: &Vec<Room>,
    all_walls: &[Line],
) -> HashMap<Uuid, (u64, (Vec<u8>, Bounds))> {
    let mut new_light_data = HashMap::new();
    for room in rooms {
        for light in &room.lights {
            let mut hasher = DefaultHasher::new();
            hash_vec2(light.pos, &mut hasher);
            light.intensity.to_bits().hash(&mut hasher);
            light.radius.to_bits().hash(&mut hasher);
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
            }
        }
    }
    new_light_data
}

fn render_light(
    bounds_min: Vec2,
    bounds_max: Vec2,
    rooms: &[Room],
    all_walls: &[Line],
    light: &Light,
    light_pos: Vec2,
) -> (Vec<u8>, Bounds) {
    // Create a vec of walls that this light can see
    let walls_for_light = get_visible_walls(light_pos, all_walls);

    // Calculate the rooms to check against, if lights room is enclosed then only that, if its not then only rooms that arent enclosed
    let mut rooms_to_check = Vec::new();
    let light_room = rooms.iter().find(|room| room.contains(light_pos)).unwrap();
    let (min, max) = light_room.bounds();
    let mut light_bounds = Bounds { min, max };
    if light_room.walls == Walls::WALL {
        rooms_to_check.push(light_room);
    } else {
        for room in rooms {
            if room.walls != Walls::WALL {
                rooms_to_check.push(room);
                let bounds = room.bounds();
                light_bounds.min = light_bounds.min.min(bounds.0);
                light_bounds.max = light_bounds.max.max(bounds.1);
            }
        }
    }

    // Calculate the size of the image based on the home size and resolution factor
    let new_size = bounds_max - bounds_min;
    let width = new_size.x * PIXELS_PER_METER;
    let height = new_size.y * PIXELS_PER_METER;

    // Create an image buffer with the calculated size, filled with black pixels
    let image_width = width as u32;
    let image_height = height as u32;
    let mut data_buffer = vec![0; (image_width * image_height) as usize];

    data_buffer.iter_mut().enumerate().for_each(|(i, pixel)| {
        let x = i as u32 % image_width;
        let y = i as u32 / image_width;
        let world = bounds_min + vec2(x as f64 / width, 1.0 - (y as f64 / height)) * new_size;

        if !rooms_to_check.iter().any(|r| r.contains(world)) {
            return;
        }

        let mut total_light_intensity = 0.0;

        let distance_to_light = (world - light_pos).length();
        if distance_to_light > light.intensity * 8.0 {
            return;
        }
        let mut sampled_light_intensity = 0.0;

        // Do more samples the closer we are to the light
        let dynamic_samples = ((LIGHT_SAMPLES as f64
            * (1.0 - distance_to_light / (light.intensity * 4.0)))
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
            sampled_light_intensity += 1.0;
        }

        // Average the light intensity from all samples
        total_light_intensity += sampled_light_intensity / dynamic_samples as f64;
        if total_light_intensity > 1.0 {
            total_light_intensity = 1.0;
        }

        *pixel = (total_light_intensity * 255.0) as u8;
    });

    (data_buffer, light_bounds)
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
