use super::HomeFlow;
use crate::common::{
    furniture::{DataPoint, ElectronicType, FurnitureType},
    utils::rotate_point_i32,
};
use egui::{Color32, Painter, Stroke};
use glam::{dvec2 as vec2, dvec3 as vec3, DMat3, DVec2 as Vec2};

impl HomeFlow {
    pub fn render_presence_sensors(&self, painter: &Painter) {
        let mut presence_points = Vec::new();
        for room in &self.layout.rooms {
            for furniture in &room.furniture {
                if furniture.furniture_type
                    == FurnitureType::Electronic(ElectronicType::UltimateSensorMini)
                {
                    // Read targets from the sensor
                    let mut targets: Vec<Vec2> = Vec::new();
                    for i in 1..=5 {
                        for entity_id in &furniture.misc_sensors {
                            if entity_id.ends_with(&format!("target_{i}_x"))
                                || entity_id.ends_with(&format!("target_{i}_y"))
                            {
                                let is_x = entity_id.ends_with("_x");
                                let value = furniture
                                    .hass_data
                                    .get(entity_id)
                                    .and_then(|value| value.parse::<f64>().ok())
                                    .unwrap_or(0.0);

                                // If target x already exists, override the x or y, else add a new target
                                if targets.len() >= i {
                                    let target = *targets.get(i - 1).unwrap();
                                    if is_x {
                                        targets[i - 1] = vec2(value, target.y);
                                    } else {
                                        targets[i - 1] = vec2(target.x, value);
                                    }
                                } else {
                                    targets.push(if is_x {
                                        vec2(value, 0.0)
                                    } else {
                                        vec2(0.0, value)
                                    });
                                }
                            }
                        }
                    }

                    // Filter out zero targets
                    targets.retain(|&v| v != Vec2::ZERO);

                    // Check if sensor has reference points
                    if let (
                        Some(DataPoint::Vec2(reference_point1)),
                        Some(DataPoint::Vec2(reference_world1)),
                        Some(DataPoint::Vec2(reference_point2)),
                        Some(DataPoint::Vec2(reference_world2)),
                        Some(DataPoint::Vec2(reference_point3)),
                        Some(DataPoint::Vec2(reference_world3)),
                    ) = (
                        furniture.misc_data.get("calib_point1"),
                        furniture.misc_data.get("calib_world1"),
                        furniture.misc_data.get("calib_point2"),
                        furniture.misc_data.get("calib_world2"),
                        furniture.misc_data.get("calib_point3"),
                        furniture.misc_data.get("calib_world3"),
                    ) {
                        let (a, b, c, d, tx, ty) = solve_affine_transformation(
                            reference_point1,
                            reference_point2,
                            reference_point3,
                            reference_world1,
                            reference_world2,
                            reference_world3,
                        );

                        // Transform live sensor point to real-world position
                        for target in &targets {
                            let real_world_pos = vec2(
                                a * target.x + b * target.y + tx,
                                c * target.x + d * target.y + ty,
                            );
                            presence_points.push(real_world_pos);
                        }
                    } else {
                        // Transform live sensor point to real-world position
                        for target in &targets {
                            let real_world_pos = room.pos
                                + furniture.pos
                                + rotate_point_i32(*target / 1000.0, -furniture.rotation);
                            presence_points.push(real_world_pos);
                        }
                    };
                }
            }
        }
        // Merge close points
        presence_points = merge_close_points(&presence_points, 0.1);

        // If point is near a chair, snap it to the chair
        let mut chair_positions = Vec::new();
        for room in &self.layout.rooms {
            for furniture in &room.furniture {
                if matches!(furniture.furniture_type, FurnitureType::Chair(_)) {
                    chair_positions.push(room.pos + furniture.pos);
                }
                let rendered_data = furniture.rendered_data.as_ref().unwrap();
                for child in &rendered_data.children {
                    if matches!(child.furniture_type, FurnitureType::Chair(_)) {
                        let hover = child.hover_amount.max(0.0);
                        let pos = room.pos
                            + furniture.pos
                            + rotate_point_i32(child.pos, -furniture.rotation)
                            + rotate_point_i32(
                                vec2(hover * 0.15, hover * 0.3),
                                -(furniture.rotation + child.rotation),
                            );
                        chair_positions.push(pos);
                    }
                }
            }
        }
        for point in &mut presence_points {
            for chair_pos in &chair_positions {
                if (*point - *chair_pos).length() < 0.4 {
                    *point = *chair_pos;
                }
            }
        }

        // Render presence points
        for point in presence_points {
            painter.circle(
                self.world_to_screen_pos(point),
                0.1 * self.stored.zoom as f32,
                Color32::from_rgb(0, 240, 140).gamma_multiply(0.5),
                Stroke::new(
                    0.02 * self.stored.zoom as f32,
                    Color32::from_rgb(0, 200, 100).gamma_multiply(0.7),
                ),
            );
        }
    }
}

fn merge_close_points(points: &[Vec2], radius: f64) -> Vec<Vec2> {
    let mut merged_points = Vec::new();
    let mut used = vec![false; points.len()];

    for (i, &point) in points.iter().enumerate() {
        if used[i] {
            continue;
        }

        let mut cluster = Vec::new();

        for (j, &other_point) in points.iter().enumerate().skip(i) {
            if !used[j] && (point - other_point).length() <= radius {
                cluster.push(other_point);
                used[j] = true;
            }
        }

        let centroid = cluster.iter().copied().sum::<Vec2>() / cluster.len() as f64;
        merged_points.push(centroid);
    }

    merged_points
}

fn solve_affine_transformation(
    p1: &Vec2,
    p2: &Vec2,
    p3: &Vec2,
    w1: &Vec2,
    w2: &Vec2,
    w3: &Vec2,
) -> (f64, f64, f64, f64, f64, f64) {
    // Create matrix for sensor points
    let sensor_matrix = DMat3::from_cols(
        vec3(p1.x, p1.y, 1.0),
        vec3(p2.x, p2.y, 1.0),
        vec3(p3.x, p3.y, 1.0),
    );

    // Create matrix for world points
    let world_matrix = DMat3::from_cols(
        vec3(w1.x, w1.y, 1.0),
        vec3(w2.x, w2.y, 1.0),
        vec3(w3.x, w3.y, 1.0),
    );

    // Inverse the sensor matrix
    let sensor_matrix_inv = sensor_matrix.inverse();

    // Calculate the transformation matrix
    let transformation_matrix = world_matrix * sensor_matrix_inv;

    // Extract transformation parameters
    let a = transformation_matrix.x_axis.x;
    let b = transformation_matrix.y_axis.x;
    let tx = transformation_matrix.z_axis.x;
    let c = transformation_matrix.x_axis.y;
    let d = transformation_matrix.y_axis.y;
    let ty = transformation_matrix.z_axis.y;

    (a, b, c, d, tx, ty)
}
