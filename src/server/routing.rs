use super::{LightPacket, PostStatesPacket, StatesPacket};
use crate::common::{layout::Home, template};
use anyhow::{anyhow, Result};
use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::Deserialize;
use std::env;
use std::{collections::HashMap, fs, path::Path};
use time::{format_description, OffsetDateTime};

const LAYOUT_PATH: &str = "home_layout.ron";

pub fn get_env_variable(key: &str) -> String {
    env::var(key).unwrap_or_else(|_| {
        panic!("Environment variable {key} is not set or contains invalid data.")
    })
}

pub fn setup_routes(app: Router) -> Router {
    app.route("/load_layout", get(load_layout_server))
        .route("/save_layout", post(save_layout_server))
        .route("/get_states", get(get_states_server))
        .route("/post_state", post(post_state_server))
}

async fn load_layout_server() -> impl IntoResponse {
    match bincode::serialize(&load_layout_impl()) {
        Ok(serialised) => (StatusCode::OK, serialised),
        Err(e) => {
            log::error!("Failed to serialise layout: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Vec::<u8>::new())
        }
    }
}

async fn save_layout_server(body: axum::body::Bytes) -> impl IntoResponse {
    log::info!("Saving layout");
    match bincode::deserialize(&body) {
        Ok(home) => match save_layout_impl(&home) {
            Ok(()) => StatusCode::OK.into_response(),
            Err(e) => {
                log::error!("Failed to save layout: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        },
        Err(e) => {
            log::error!("Failed to deserialise layout: {:?}", e);
            StatusCode::BAD_REQUEST.into_response()
        }
    }
}

async fn get_states_server() -> impl IntoResponse {
    match get_states_impl().await {
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

async fn post_state_server(body: axum::body::Bytes) -> impl IntoResponse {
    match bincode::deserialize(&body) {
        Ok(params) => match post_state_impl(params).await {
            Ok(()) => StatusCode::OK.into_response(),
            Err(e) => {
                log::error!("Failed to post state: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        },
        Err(e) => {
            log::error!("Failed to deserialise state: {:?}", e);
            StatusCode::BAD_REQUEST.into_response()
        }
    }
}

fn load_layout_impl() -> Home {
    fs::read_to_string(LAYOUT_PATH).map_or_else(
        |_| template::default(),
        |contents| ron::from_str::<Home>(&contents).unwrap_or_else(|_| template::default()),
    )
}

fn save_layout_impl(home: &Home) -> Result<()> {
    let home_ron = ron::ser::to_string_pretty(home, ron::ser::PrettyConfig::default())?;
    let temp_path = Path::new(LAYOUT_PATH).with_extension("tmp");
    fs::write(&temp_path, home_ron)
        .map_err(|e| anyhow!("Failed to write temporary layout: {}", e))?;

    if Path::new(LAYOUT_PATH).exists() {
        let metadata = fs::metadata(LAYOUT_PATH)?;
        let modified_time = metadata.modified()?;
        let modified_time = OffsetDateTime::from(modified_time);
        let format = format_description::parse("[year]-[month]-[day]_[hour]-[minute]-[second]")?;
        let backup_filename = format!("backups/home_layout_{}.ron", modified_time.format(&format)?);

        fs::create_dir_all("backups")?;
        fs::rename(LAYOUT_PATH, backup_filename)?;
    }

    fs::rename(&temp_path, LAYOUT_PATH)?;
    Ok(())
}

#[derive(Debug, Deserialize)]
struct HassState {
    entity_id: String,
    state: String,
    _last_changed: String,
    _attributes: HashMap<String, serde_json::Value>,
}

async fn get_states_impl() -> Result<StatesPacket> {
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
    for state_raw in &states_raw {
        if state_raw.entity_id.starts_with("light.") {
            let state_value = match state_raw.state.as_str() {
                "on" => 255,
                _ => 0,
            };

            let state = LightPacket {
                entity_id: state_raw.entity_id.split('.').nth(1).unwrap().to_string(),
                state: state_value,
            };
            light_states.push(state);
        }
    }

    Ok(StatesPacket {
        lights: light_states,
    })
}

async fn post_state_impl(params: Vec<PostStatesPacket>) -> Result<()> {
    let client = reqwest::Client::new();
    let mut errors = Vec::new();

    // Convert params to the format required by the Home Assistant API
    for param in params {
        let response = client
            .post(&format!(
                "{}/api/states/{}",
                &get_env_variable("HASS_URL"),
                param.entity_id
            ))
            .header(
                AUTHORIZATION,
                format!("Bearer {}", get_env_variable("HASS_TOKEN")),
            )
            .header(CONTENT_TYPE, "application/json")
            .json(&param)
            .send()
            .await;

        if let Err(e) = response {
            errors.push(anyhow!("Failed to post state: {}", e));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(
            "Errors occurred while posting states: {:?}",
            errors
        ))
    }
}
