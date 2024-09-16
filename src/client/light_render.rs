use crate::common::{
    layout::{Light, LightData, LightsData, Room, Walls},
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

const PIXELS_PER_METER: f64 = 30.0;
const LIGHT_SAMPLES: u8 = 12; // Number of samples within the light's radius for anti-aliasing
const MAX_LIGHTS_PER_FRAME: u32 = 4;

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
            if let Some((_, light_data)) = &light.light_data {
                if light_data.len() == image_pixel_count {
                    lights_data.push((
                        light.intensity * (f64::from(light.state) / 255.0),
                        light.get_points(room),
                        light_data,
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
            let world =
                bounds_min + vec2(f64::from(x) / width, 1.0 - (f64::from(y) / height)) * new_size;

            if !rooms.iter().any(|r| r.contains(world)) {
                return;
            }

            let mut total_light_intensity: f64 = 0.0;
            for (light_intensity, light_points, light_image) in &lights_data {
                let light_pixel = f64::from(light_image[i]);
                if light_pixel == 0.0 {
                    continue;
                }
                for light_pos in light_points {
                    let distance = world.distance(*light_pos) * 2.0 / light_intensity;
                    total_light_intensity += light_pixel / distance.powf(2.0);
                    if total_light_intensity >= 255.0 {
                        total_light_intensity = 255.0;
                        break;
                    }
                }
                if total_light_intensity >= 255.0 {
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
) -> (bool, HashMap<Uuid, LightsData>) {
    let mut cur_changed = 0;
    let mut new_light_data = HashMap::new();
    for room in rooms {
        for light in &room.lights {
            let mut hasher = DefaultHasher::new();
            hash_vec2(light.pos, &mut hasher);
            light.multi.hash(&mut hasher);
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
                    &light.get_points(room),
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
    points: &[Vec2],
) -> Vec<u16> {
    // Create a vec of walls that this light can see
    let mut walls_for_light = Vec::with_capacity(points.len());
    for point in points {
        walls_for_light.push(get_visible_walls(*point, all_walls));
    }

    // Calculate the rooms to check against, if lights room is enclosed then only that, if its not then only rooms that arent enclosed
    let mut rooms_to_check: Vec<&Room> = Vec::new();
    let mut is_light_contained = true;
    for point in points {
        let room = rooms.iter().find(|room| room.contains(*point));
        if let Some(room) = room {
            if !rooms_to_check.iter().any(|r| r.id == room.id) {
                rooms_to_check.push(room);
            }
            if room.walls != Walls::WALL {
                is_light_contained = false;
            }
        }
    }
    if !is_light_contained {
        for room in rooms {
            if room.walls != Walls::WALL && !rooms_to_check.iter().any(|r| r.id == room.id) {
                rooms_to_check.push(room);
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
        let world =
            bounds_min + vec2(f64::from(x) / width, 1.0 - (f64::from(y) / height)) * new_size;

        if !rooms_to_check.iter().any(|r| r.contains(world)) {
            return;
        }

        let mut total_light_intensity = 0.0;

        for (light_index, light_pos) in points.iter().enumerate() {
            // Do more samples the closer we are to the light
            let dynamic_samples = ((f64::from(LIGHT_SAMPLES)
                * (1.0 - world.distance(*light_pos) / (light.intensity * 10.0)))
                .round() as u8)
                .max(1);

            // Get 4 positions at the corners of the pixel
            for point in [
                vec2(f64::from(x) - 0.5, f64::from(y) - 0.5),
                vec2(f64::from(x) + 0.5, f64::from(y) - 0.5),
                vec2(f64::from(x) - 0.5, f64::from(y) + 0.5),
                vec2(f64::from(x) + 0.5, f64::from(y) + 0.5),
            ] {
                let world = bounds_min + vec2(point.x / width, 1.0 - (point.y / height)) * new_size;
                let mut sampled_light_intensity = 0.0;

                for i in 0..dynamic_samples {
                    // Calculate offset for current sample
                    let sample_light_position = if dynamic_samples == 1 {
                        *light_pos
                    } else {
                        let angle = 2.0 * PI * (f64::from(i) / f64::from(dynamic_samples));
                        *light_pos + vec2(light.radius * angle.cos(), light.radius * angle.sin())
                    };

                    // Check if the sample light position and pixel intersect with any lines
                    if walls_for_light[light_index]
                        .iter()
                        .any(|(p1, p2)| lines_intersect(sample_light_position, world, *p1, *p2))
                    {
                        continue;
                    }
                    sampled_light_intensity += 1.0;
                }

                // Average the light intensity from all samples
                total_light_intensity +=
                    sampled_light_intensity * 255.0 / f64::from(dynamic_samples) / 4.0;
            }
        }

        *pixel = total_light_intensity as u16;
    });

    data_buffer
}

const POINTS_DISTANCE: f64 = 0.1; // Distance between points on the wall to check for visibility
fn get_visible_walls(light_pos: Vec2, all_walls: &[Line]) -> Vec<Line> {
    let mut visible_walls = Vec::with_capacity(all_walls.len());
    for (i, &(start, end)) in all_walls.iter().enumerate() {
        let light_distance = start.distance(light_pos).min(end.distance(light_pos));

        // Generate points along the wall for more granular visibility checks
        let mut points = vec![start, end];
        let total_distance = start.distance(end);
        if total_distance > POINTS_DISTANCE {
            let direction = (end - start).normalize();
            let num_points = (total_distance / POINTS_DISTANCE).ceil() as usize - 1;
            for i in 1..num_points {
                points.push(start + direction * (POINTS_DISTANCE * i as f64));
            }
        }

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
