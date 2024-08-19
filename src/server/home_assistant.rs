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
use glam::{dvec2 as vec2, DVec2 as Vec2};
use nalgebra::DMatrix;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::env;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::Instant;

static LOOP_INTERVAL_MS: u64 = 200;
static CALIBRATION_DURATION: f64 = 30.0;
static OCCUPANCY_DURATION: f64 = 3.0;

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
    let client = reqwest::Client::new();
    let url = format!("{}/api/states", get_env_variable("HASS_URL"));
    let token = format!("Bearer {}", get_env_variable("HASS_TOKEN"));
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

        if let Ok(states) = get_states_impl(&client, &url, &token, sensors).await {
            *HA_STATE.lock().await = Some(states);
        }
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

#[derive(Debug, Deserialize)]
struct HassState {
    entity_id: String,
    state: String,
    #[allow(dead_code)]
    last_changed: String,
    attributes: HashMap<String, serde_json::Value>,
}

// Room name -> (Occupied, Targets, Last Occupied)
type OccupancyData = HashMap<String, (bool, u8, Instant)>;

static LAST_OCCUPANCY: LazyLock<Mutex<OccupancyData>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
type PresenceCalibration = (Instant, Vec<Vec2>);
static PRESENCE_CALIBRATION: LazyLock<Mutex<Option<PresenceCalibration>>> =
    LazyLock::new(|| Mutex::new(None));

async fn get_states_impl(
    client: &reqwest::Client,
    url: &str,
    token: &str,
    target_sensors: Vec<String>,
) -> Result<HAState> {
    let states_raw = client
        .get(url)
        .header(AUTHORIZATION, token)
        .header(CONTENT_TYPE, "application/json")
        .send()
        .await?
        .json::<Vec<HassState>>()
        .await?;

    let mut lights = Vec::new();
    let mut sensors = Vec::new();

    for state_raw in &states_raw {
        if let Some((domain, entity_id)) = state_raw.entity_id.split_once('.') {
            match domain {
                "light" => lights.push(LightPacket {
                    entity_id: entity_id.to_string(),
                    state: state_raw
                        .attributes
                        .get("brightness")
                        .and_then(serde_json::Value::as_u64)
                        .map_or_else(
                            || if state_raw.state == "on" { 255 } else { 0 },
                            |b| b as u8,
                        ),
                }),
                "sensor" if target_sensors.contains(&entity_id.to_string()) => {
                    sensors.push(SensorPacket {
                        entity_id: entity_id.to_string(),
                        state: state_raw.state.clone(),
                    });
                }
                _ => {}
            }
        }
    }

    // Begin calibration if needed
    let mut calibration_lock = PRESENCE_CALIBRATION.lock().await;
    let presence_calibration = states_raw.iter().any(|state| {
        state.entity_id == "input_boolean.presence_calibration" && state.state == "on"
    });
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
            if furniture.furniture_type
                == FurnitureType::Electronic(ElectronicType::UltimateSensorMini)
            {
                // Read targets from the sensor
                let targets = (1..)
                    .map(|i| {
                        let mut x = f64::NAN;
                        let mut y = f64::NAN;

                        for id in &furniture.misc_sensors {
                            if let Some(value) = states_raw
                                .iter()
                                .find(|state| state.entity_id == format!("sensor.{id}"))
                                .and_then(|state| state.state.parse::<f64>().ok())
                            {
                                if id.ends_with(&format!("target_{i}_x")) {
                                    x = value;
                                } else if id.ends_with(&format!("target_{i}_y")) {
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
                    let sensor_matrix_pseudo_inverse = (sensor_matrix.transpose() * &sensor_matrix)
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
        }
    }
    // Merge close points
    presence_points = {
        let mut points = presence_points.clone();
        let mut merged_points = Vec::new();

        while let Some(point) = points.pop() {
            let mut cluster = vec![point];

            points.retain(|&other_point| {
                if (point - other_point).length() <= 0.1 {
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
        } else {
            calibration_points.extend(presence_points_raw);
        }
    }
    drop(calibration);

    // Calculate zone occupancy
    let mut zone_occupancy = HashMap::new();
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
        if *last_occupied != *occupied {
            *last_occupied = *occupied;

            if *occupied || last_time.elapsed().as_secs_f64() > OCCUPANCY_DURATION {
                post_data.push(PostActionsData {
                    domain: "input_boolean".to_string(),
                    action: if *occupied { "turn_on" } else { "turn_off" }.to_string(),
                    entity_id: format!("input_boolean.{zone_name}_occupancy"),
                    additional_data: HashMap::new(),
                });
            }
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
        post_actions_impl(post_data).await?;
    }

    Ok(HAState {
        lights,
        sensors,
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
                    DataPoint::Vec4((a, b, c, d)) => serde_json::json!([a, b, c, d]),
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
