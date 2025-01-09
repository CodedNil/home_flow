use crate::{
    common::{
        furniture::{FurnitureType, SensorType},
        layout::DataPoint,
        utils::rotate_point_i32,
        PostActionsData,
    },
    server::{home_assistant::post_actions_impl, routing::HOME},
};
use ahash::AHashMap;
use anyhow::Result;
use glam::{dvec2 as vec2, DVec2 as Vec2};
use nalgebra::DMatrix;
use std::{sync::LazyLock, time::Duration};
use tokio::{sync::Mutex, time::Instant};

static CALIBRATION_DURATION: f64 = 30.0;
static OCCUPANCY_DURATION: f64 = 30.0;

// Room name -> (Occupied, Targets, Last Occupied)
type OccupancyData = AHashMap<String, (bool, u8, Instant)>;

static LAST_OCCUPANCY: LazyLock<Mutex<OccupancyData>> =
    LazyLock::new(|| Mutex::new(AHashMap::new()));
type PresenceCalibration = (Instant, Vec<Vec2>);
static PRESENCE_CALIBRATION: LazyLock<Mutex<Option<PresenceCalibration>>> =
    LazyLock::new(|| Mutex::new(None));

pub async fn calculate(sensors: &AHashMap<String, String>) -> Result<Vec<Vec2>> {
    // Begin calibration if needed
    let mut calibration_lock = PRESENCE_CALIBRATION.lock().await;
    let presence_calibration = sensors
        .get("input_boolean.presence_calibration")
        .is_some_and(|s| s == "on");
    match (&*calibration_lock, presence_calibration) {
        (None, true) => {
            *calibration_lock = Some((Instant::now(), Vec::new()));
            log::info!("Presence calibration started");
        }
        (Some(_), false) => {
            *calibration_lock = None;
        }
        _ => {}
    }
    let is_calibrating = calibration_lock.is_some();
    drop(calibration_lock);

    let layout = HOME.lock().await.clone();

    let mut presence_points = Vec::new();
    let mut presence_points_raw = Vec::new();
    for room in &layout.rooms {
        for furniture in &room.furniture {
            match furniture.furniture_type {
                FurnitureType::Sensor(SensorType::UltimateSensorMini) => {
                    // Read targets from the sensor
                    let targets = (1..)
                        .map(|i| {
                            let mut x = f64::NAN;
                            let mut y = f64::NAN;

                            for id in &furniture.misc_sensors {
                                if let Some(value) =
                                    sensors.get(id).and_then(|state| state.parse::<f64>().ok())
                                {
                                    if id.contains(&format!("_target_{i}_x")) {
                                        x = value;
                                    } else if id.contains(&format!("_target_{i}_y")) {
                                        y = value;
                                    }
                                }
                            }

                            if x.is_nan() || y.is_nan() {
                                None
                            } else {
                                Some(vec2(x, y))
                            }
                        })
                        .take_while(Option::is_some)
                        .flatten()
                        .filter(|&v| v != Vec2::ZERO)
                        .collect::<Vec<_>>();

                    if is_calibrating {
                        presence_points_raw.extend(targets.clone());
                    }

                    // Collect calibration points if available
                    let calibration_data = (1..)
                        .map(|i| {
                            furniture
                                .misc_data
                                .get(&format!("calib_{i}"))
                                .and_then(|data| {
                                    if let DataPoint::Vec4((wx, wy, sx, sy)) = data {
                                        Some((vec2(*wx, *wy), vec2(*sx, *sy)))
                                    } else {
                                        None
                                    }
                                })
                        })
                        .take_while(Option::is_some)
                        .flatten()
                        .collect::<Vec<_>>();

                    if calibration_data.len() >= 3 {
                        // Create matrices for sensor and world points
                        let mut sensor_matrix = DMatrix::zeros(calibration_data.len(), 3);
                        let mut world_matrix_x = DMatrix::zeros(calibration_data.len(), 1);
                        let mut world_matrix_y = DMatrix::zeros(calibration_data.len(), 1);

                        for (i, &(world, sensor)) in calibration_data.iter().enumerate() {
                            sensor_matrix[(i, 0)] = sensor.x;
                            sensor_matrix[(i, 1)] = sensor.y;
                            sensor_matrix[(i, 2)] = 1.0; // Homogeneous coordinate

                            world_matrix_x[(i, 0)] = world.x;
                            world_matrix_y[(i, 0)] = world.y;
                        }

                        // Solve for the transformation matrix using the least squares method
                        let sensor_matrix_pseudo_inverse = (sensor_matrix.transpose()
                            * &sensor_matrix)
                            .try_inverse()
                            .unwrap()
                            * sensor_matrix.transpose();

                        let transform_x = sensor_matrix_pseudo_inverse.clone() * world_matrix_x;
                        let transform_y = sensor_matrix_pseudo_inverse * world_matrix_y;

                        let a = transform_x[(0, 0)];
                        let b = transform_x[(1, 0)];
                        let tx = transform_x[(2, 0)];

                        let c = transform_y[(0, 0)];
                        let d = transform_y[(1, 0)];
                        let ty = transform_y[(2, 0)];

                        presence_points.extend(targets.iter().map(|target| {
                            vec2(
                                a * target.x + b * target.y + tx,
                                c * target.x + d * target.y + ty,
                            )
                        }));
                    } else {
                        presence_points.extend(targets.iter().map(|target| {
                            room.pos
                                + furniture.pos
                                + rotate_point_i32(*target / 1000.0, -furniture.rotation)
                        }));
                    };
                }
                FurnitureType::Sensor(SensorType::PresenceBoolean) => {
                    // If sensed, add a presence point on the furniture's position
                    if furniture
                        .misc_sensors
                        .iter()
                        .any(|id| sensors.get(id).is_some_and(|state| state == "on"))
                    {
                        presence_points.push(room.pos + furniture.pos);
                    }
                }
                _ => {}
            }
        }
    }
    // Merge close points
    presence_points = {
        let mut points = presence_points.clone();
        let mut merged_points = Vec::new();

        while let Some(point) = points.pop() {
            let mut cluster = vec![point];

            points.retain(|&other_point| {
                if (point - other_point).length() <= 0.4 {
                    cluster.push(other_point);
                    false
                } else {
                    true
                }
            });

            let centroid = cluster.iter().copied().sum::<Vec2>() / cluster.len() as f64;
            merged_points.push(centroid);
        }

        merged_points
    };

    // If calibrating, add raw points to data
    let mut calibration = PRESENCE_CALIBRATION.lock().await;
    if let Some((start_time, calibration_points)) = calibration.as_mut() {
        if start_time.elapsed().as_secs_f64() > CALIBRATION_DURATION {
            let average = (calibration_points.iter().copied().sum::<Vec2>()
                / calibration_points.len() as f64)
                .round();
            log::info!("Calibration ended, average point: {:?}", average);

            // Set input_boolean.presence_calibration to false and input_text.presence_calibration_output to the average point
            post_actions_impl(vec![
                PostActionsData {
                    domain: "input_boolean".to_string(),
                    action: "turn_off".to_string(),
                    entity_id: "input_boolean.presence_calibration".to_string(),
                    additional_data: AHashMap::new(),
                },
                PostActionsData {
                    domain: "input_text".to_string(),
                    action: "set_value".to_string(),
                    entity_id: "input_text.presence_calibration_output".to_string(),
                    additional_data: vec![(
                        "value".to_string(),
                        DataPoint::String(format!("{},{}", average.x, average.y)),
                    )]
                    .into_iter()
                    .collect(),
                },
            ])
            .await;

            *calibration = None;
        } else {
            calibration_points.extend(presence_points_raw);
        }
    }
    drop(calibration);

    // Calculate zone occupancy
    let mut zone_occupancy = AHashMap::new();
    for room in &layout.rooms {
        let room_occupancy = presence_points
            .iter()
            .filter(|&&p| room.contains(p))
            .count();

        zone_occupancy.insert(
            room.name.to_lowercase().replace(' ', "_"),
            (room_occupancy > 0, room_occupancy as u8),
        );

        for zone in &room.zones {
            let zone_occupancy_count = presence_points
                .iter()
                .filter(|&&p| zone.contains(room.pos, p))
                .count();

            zone_occupancy
                .entry(zone.name.to_lowercase().replace(' ', "_"))
                .and_modify(|e| {
                    e.0 |= zone_occupancy_count > 0;
                    e.1 += zone_occupancy_count as u8;
                })
                .or_insert((zone_occupancy_count > 0, zone_occupancy_count as u8));
        }
    }

    // Compare to LAST_OCCUPANCY and for differences, post actions
    let mut last_occupancy = LAST_OCCUPANCY.lock().await;
    let mut post_data = Vec::new();

    for (zone_name, (occupied, targets)) in &zone_occupancy {
        let (last_occupied, last_targets, last_time) =
            last_occupancy.entry(zone_name.clone()).or_insert_with(|| {
                (
                    *occupied,
                    *targets,
                    if *occupied {
                        Instant::now()
                    } else {
                        Instant::now() - Duration::from_secs(3600)
                    },
                )
            });

        // Update the time if occupied
        if *occupied {
            *last_time = Instant::now();
        }

        // Update the occupancy if necessary
        if *last_occupied != *occupied
            && (*occupied || last_time.elapsed().as_secs_f64() > OCCUPANCY_DURATION)
        {
            *last_occupied = *occupied;
            post_data.push(PostActionsData {
                domain: "input_boolean".to_string(),
                action: if *occupied { "turn_on" } else { "turn_off" }.to_string(),
                entity_id: format!("input_boolean.{zone_name}_occupancy"),
                additional_data: AHashMap::new(),
            });
        }

        // Update targets if necessary
        if *last_targets != *targets {
            *last_targets = *targets;
            post_data.push(PostActionsData {
                domain: "input_number".to_string(),
                action: "set_value".to_string(),
                entity_id: format!("input_number.{zone_name}_targets"),
                additional_data: vec![("value".to_string(), DataPoint::Int(*targets))]
                    .into_iter()
                    .collect(),
            });
        }
    }
    drop(last_occupancy);

    if !post_data.is_empty() {
        post_actions_impl(post_data).await;
    }

    Ok(presence_points)
}
