use super::{auth::verify_token, GetStatesPacket, PostActionsPacket};
use super::{HAState, LightPacket, PostActionsData, SensorPacket};
use crate::common::layout::DataPoint;
use anyhow::{anyhow, Result};
use axum::body::Bytes;
use axum::{http::StatusCode, response::IntoResponse};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::env;
use std::sync::LazyLock;
use tokio::sync::Mutex;

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
        let states = get_states_impl(Vec::new()).await.unwrap();
        *HA_STATE.lock().await = Some(states);
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
}

pub async fn get_states_server(body: Bytes) -> impl IntoResponse {
    let packet: GetStatesPacket = match bincode::deserialize(&body) {
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

    Ok(HAState {
        lights: light_states,
        sensors: sensor_states,
        presence_points: Vec::new(),
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
