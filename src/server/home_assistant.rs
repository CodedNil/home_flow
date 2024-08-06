use super::PostServicesData;
use super::{
    auth::verify_token, GetStatesPacket, LightPacket, PostServicesPacket, SensorPacket,
    StatesPacket,
};
use anyhow::{anyhow, Result};
use axum::body::Bytes;
use axum::{http::StatusCode, response::IntoResponse};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::env;

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

    match get_states_impl(packet.sensors).await {
        Ok(states) => match bincode::serialize(&states) {
            Ok(serialized) => (StatusCode::OK, serialized),
            Err(e) => {
                log::error!("Failed to serialize states: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Vec::<u8>::new())
            }
        },
        Err(e) => {
            log::error!("Failed to get states: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Vec::<u8>::new())
        }
    }
}

pub async fn post_services_server(body: Bytes) -> impl IntoResponse {
    let packet: PostServicesPacket = match bincode::deserialize(&body) {
        Ok(packet) => packet,
        Err(e) => {
            log::error!("Failed to deserialize post_services_server packet: {:?}", e);
            return StatusCode::BAD_REQUEST.into_response();
        }
    };
    if !matches!(verify_token(&packet.token), Ok(true)) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    match post_services_impl(packet.data).await {
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

async fn get_states_impl(sensors: Vec<String>) -> Result<StatesPacket> {
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
            let state_value = match state_raw.state.as_str() {
                "on" => 255,
                _ => 0,
            };

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

    Ok(StatesPacket {
        lights: light_states,
        sensors: sensor_states,
    })
}

async fn post_services_impl(data: Vec<PostServicesData>) -> Result<()> {
    let client = reqwest::Client::new();
    let mut errors = Vec::new();

    // Convert params to the format required by the Home Assistant API
    for param in data {
        // Construct the JSON payload
        let mut data = HashMap::new();
        data.insert("entity_id".to_string(), json!(param.entity_id));
        for (key, value) in param.additional_data {
            data.insert(key, value);
        }

        // Send the request
        let response = client
            .post(&format!(
                "{}/api/services/{}/{}",
                &get_env_variable("HASS_URL"),
                param.domain,
                param.service
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
            errors.push(anyhow!("Failed to post services: {}", e));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(
            "Errors occurred while posting services: {:?}",
            errors
        ))
    }
}
