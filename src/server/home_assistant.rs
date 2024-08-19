use super::TokenPacket;
use super::{
    auth::verify_token, routing::HOME, HAState, LightPacket, PostActionsData, PostActionsPacket,
    SensorPacket,
};
use crate::common::furniture::{ElectronicType, FurnitureType};
use crate::common::layout::DataPoint;
use crate::common::utils::rotate_point_i32;
use anyhow::{anyhow, Result};
use axum::body::Bytes;
use axum::{http::StatusCode, response::IntoResponse};
use glam::{dvec2 as vec2, dvec3 as vec3, DMat3, DVec2 as Vec2};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::env;
use std::sync::LazyLock;
use tokio::sync::Mutex;
use tokio::time::Instant;

static LOOP_INTERVAL_MS: u64 = 500;
static CALIBRATION_DURATION: f64 = 30.0;

fn get_env_variable(key: &str) -> String {
    match env::var(key) {
        Ok(value) => value,
        Err(e) => match e {
            env::VarError::NotPresent => panic!("Environment variable {key} not found."),
            env::VarError::NotUnicode(oss) => {
                panic!("Environment variable {key} contains invalid data: {oss:?}")
            }
        },
    }
}

static HA_STATE: LazyLock<Mutex<Option<HAState>>> = LazyLock::new(|| Mutex::new(None));

/// Every X seconds, get the states from Home Assistant
pub async fn server_loop() {
    loop {
        // Get list of sensors to fetch
        let layout = HOME.lock().await;
        let mut sensors = Vec::new();
        for room in &layout.rooms {
            for sensor in &room.sensors {
                sensors.push(sensor.entity_id.clone());
            }
            for furniture in &room.furniture {
                let wanted = furniture.wanted_sensors();
                if !wanted.is_empty() {
                    sensors.extend(wanted);
                }
            }
        }
        drop(layout);

        let states = get_states_impl(sensors).await.unwrap();
        *HA_STATE.lock().await = Some(states);
        tokio::time::sleep(tokio::time::Duration::from_millis(LOOP_INTERVAL_MS)).await;
    }
}

pub async fn get_states_server(body: Bytes) -> impl IntoResponse {
    let packet: TokenPacket = match bincode::deserialize(&body) {
        Ok(packet) => packet,
        Err(e) => {
            log::error!("Failed to deserialize get_states_server packet: {:?}", e);
            return (StatusCode::BAD_REQUEST, Vec::new());
        }
    };
    if !matches!(verify_token(&packet.token), Ok(true)) {
        return (StatusCode::UNAUTHORIZED, Vec::new());
    }

    let ha_state = HA_STATE.lock().await;
    ha_state.as_ref().map_or_else(
        || {
            log::error!("State not found in memory");
            (StatusCode::INTERNAL_SERVER_ERROR, Vec::new())
        },
        |states| match bincode::serialize(states) {
            Ok(serialized) => (StatusCode::OK, serialized),
            Err(e) => {
                log::error!("Failed to serialize states: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Vec::new())
            }
        },
    )
}

pub async fn post_actions_server(body: Bytes) -> impl IntoResponse {
    let packet: PostActionsPacket = match bincode::deserialize(&body) {
        Ok(packet) => packet,
        Err(e) => {
            log::error!("Failed to deserialize post_actions_server packet: {:?}", e);
            return StatusCode::BAD_REQUEST.into_response();
        }
    };
    if !matches!(verify_token(&packet.token), Ok(true)) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    match post_actions_impl(packet.data).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => {
            log::error!("Failed to post state: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct HassState {
    entity_id: String,
    state: String,
    last_changed: String,
    attributes: HashMap<String, serde_json::Value>,
}

static LAST_OCCUPANCY: LazyLock<Mutex<HashMap<String, (bool, u8)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static PRESENCE_CALIBRATION: LazyLock<Mutex<Option<(Instant, Vec<Vec2>)>>> =
    LazyLock::new(|| Mutex::new(None));

async fn get_states_impl(sensors: Vec<String>) -> Result<HAState> {
    let client = reqwest::Client::new();
    let states_raw = client
        .get(&format!("{}/api/states", get_env_variable("HASS_URL")))
        .header(
            AUTHORIZATION,
            format!("Bearer {}", get_env_variable("HASS_TOKEN")),
        )
        .header(CONTENT_TYPE, "application/json")
        .send()
        .await?
        .json::<Vec<HassState>>()
        .await?;

    let mut light_states = Vec::new();
    let mut sensor_states = Vec::new();
    for state_raw in &states_raw {
        let entity_id = state_raw.entity_id.split('.').nth(1).unwrap();
        if state_raw.entity_id.starts_with("light.") {
            let mut state_value = match state_raw.state.as_str() {
                "on" => 255,
                _ => 0,
            };
            // Check if attributes has brightness
            if let Some(brightness) = state_raw.attributes.get("brightness") {
                if let Some(brightness) = brightness.as_u64() {
                    state_value = brightness as u8;
                }
            }

            let state = LightPacket {
                entity_id: entity_id.to_string(),
                state: state_value,
            };
            light_states.push(state);
        }
        if state_raw.entity_id.starts_with("sensor.") && sensors.contains(&entity_id.to_string()) {
            let state = SensorPacket {
                entity_id: entity_id.to_string(),
                state: state_raw.state.clone(),
            };
            sensor_states.push(state);
        }
    }

    // Begin calibration if needed
    let calibration = PRESENCE_CALIBRATION.lock().await.clone();
    let presence_calibration = states_raw
        .iter()
        .find(|state| state.entity_id == "input_boolean.presence_calibration")
        .map_or_else(|| false, |state| state.state == "on");
    if calibration.is_none() && presence_calibration {
        *PRESENCE_CALIBRATION.lock().await = Some((Instant::now(), Vec::new()));
        log::info!("Presence calibration started");
    }
    // If calibration becomes false, end it
    if calibration.is_some() && !presence_calibration {
        *PRESENCE_CALIBRATION.lock().await = None;
    }

    let layout = HOME.lock().await.clone();

    let mut presence_points = Vec::new();
    let mut presence_points_raw = Vec::new();
    for room in &layout.rooms {
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
                            let value = states_raw
                                .iter()
                                .find(|state| state.entity_id == format!("sensor.{entity_id}"))
                                .and_then(|state| state.state.parse::<f64>().ok())
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
                if calibration.is_some() {
                    presence_points_raw.extend(targets.clone());
                }

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

    // If calibrating, add raw points to data if they are not already present
    {
        let mut calibration = PRESENCE_CALIBRATION.lock().await;
        let num_calib_points = calibration.as_ref().map_or(0, |(_, points)| points.len());
        if let Some((start_time, calibration_points)) = calibration.as_mut() {
            if start_time.elapsed().as_secs_f64() > CALIBRATION_DURATION {
                // Get average of all points
                let mut sum = Vec2::ZERO;
                for point in calibration_points {
                    sum += *point;
                }
                let average = (sum / num_calib_points as f64).round();
                log::info!("Calibration ended, average point: {:?}", average);
                // Set input_boolean.presence_calibration to false and input_text.presence_calibration_output to the average point
                post_actions_impl(vec![
                    PostActionsData {
                        domain: "input_boolean".to_string(),
                        action: "turn_off".to_string(),
                        entity_id: "input_boolean.presence_calibration".to_string(),
                        additional_data: HashMap::new(),
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
                .await
                .unwrap();
                *calibration = None;
                drop(calibration);
            } else {
                calibration_points.extend(presence_points_raw);
            }
        }
    }

    // Calculate which zones are occupied
    let mut zone_occupancy: HashMap<String, (bool, u8)> = HashMap::new();
    for room in &layout.rooms {
        let mut occupied = false;
        let mut targets = 0;
        for point in &presence_points {
            if room.contains(*point) {
                occupied = true;
                targets += 1;
            }
        }
        zone_occupancy.insert(
            room.name.to_lowercase().replace(' ', "_"),
            (occupied, targets),
        );
        for zone in &room.zones {
            let mut zone_occupancy_entry = (false, 0);
            for point in &presence_points {
                if zone.contains(room.pos, *point) {
                    zone_occupancy_entry.0 = true;
                    zone_occupancy_entry.1 += 1;
                }
            }
            zone_occupancy
                .entry(zone.name.to_lowercase().replace(' ', "_"))
                .and_modify(|e| {
                    e.0 |= zone_occupancy_entry.0; // Set occupied to true if any are occupied
                    e.1 += zone_occupancy_entry.1; // Sum up all targets
                })
                .or_insert(zone_occupancy_entry);
        }
    }
    // Compare to LAST_OCCUPANCY and for differences, post actions
    let mut last_occupancy = LAST_OCCUPANCY.lock().await;
    let mut post_data = Vec::new();
    for (zone_name, (occupied, targets)) in &zone_occupancy {
        let is_different = match last_occupancy.get(zone_name) {
            Some((last_occupied, last_targets)) => {
                *last_occupied != *occupied || *last_targets != *targets
            }
            None => true,
        };
        if is_different {
            last_occupancy.insert(zone_name.clone(), (*occupied, *targets));

            post_data.push(PostActionsData {
                domain: "input_boolean".to_string(),
                action: if *occupied { "turn_on" } else { "turn_off" }.to_string(),
                entity_id: format!("input_boolean.{zone_name}_occupancy"),
                additional_data: HashMap::new(),
            });
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
        post_actions_impl(post_data).await?;
    }

    Ok(HAState {
        lights: light_states,
        sensors: sensor_states,
        presence_points,
    })
}

async fn post_actions_impl(data: Vec<PostActionsData>) -> Result<()> {
    let client = reqwest::Client::new();
    let mut errors = Vec::new();

    // Convert params to the format required by the Home Assistant API
    for param in data {
        // Construct the JSON payload
        let mut data = HashMap::new();
        data.insert("entity_id".to_string(), json!(param.entity_id));
        for (key, value) in param.additional_data {
            data.insert(
                key,
                match value {
                    DataPoint::String(s) => serde_json::Value::String(s),
                    DataPoint::Float(f) => {
                        serde_json::Value::Number(serde_json::Number::from_f64(f).unwrap())
                    }
                    DataPoint::Int(i) => serde_json::Value::Number(serde_json::Number::from(i)),
                    DataPoint::Vec2(v) => serde_json::json!([v.x, v.y]),
                },
            );
        }

        // Send the request
        let response = client
            .post(&format!(
                "{}/api/services/{}/{}",
                &get_env_variable("HASS_URL"),
                param.domain,
                param.action
            ))
            .header(
                AUTHORIZATION,
                format!("Bearer {}", get_env_variable("HASS_TOKEN")),
            )
            .header(CONTENT_TYPE, "application/json")
            .json(&data)
            .send()
            .await;

        if let Err(e) = response {
            errors.push(anyhow!("Failed to post actions: {}", e));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(
            "Errors occurred while posting actions: {:?}",
            errors
        ))
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
