use super::common_api::{LightPacket, StatesPacket};
use crate::common::{layout::Home, template};
use anyhow::{anyhow, Result};
use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use home_assistant_rest::{get::StateEnum, Client as HomeAssistantClient};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use time::{format_description, OffsetDateTime};

const LAYOUT_PATH: &str = "home_layout.ron";

#[derive(Deserialize, Serialize)]
struct Config {
    pub home_assistant_url: String,
    pub home_assistant_token: String,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            home_assistant_url: "http://localhost:8123".to_string(),
            home_assistant_token: "Long-Lived Access Token".to_string(),
        }
    }
}

fn get_config() -> Config {
    std::fs::read_to_string("config.ron").map_or_else(
        |_| {
            let config = Config::default();
            std::fs::write(
                "config.ron",
                ron::ser::to_string_pretty(&config, ron::ser::PrettyConfig::default()).unwrap(),
            )
            .unwrap();
            config
        },
        |contents| ron::from_str::<Config>(&contents).unwrap_or_else(|_| Config::default()),
    )
}

pub fn setup_routes(app: Router) -> Router {
    app.route("/load_layout", get(load_layout_server))
        .route("/save_layout", post(save_layout_server))
        .route("/get_states", get(get_states_server))
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
    match bincode::serialize(&get_states_impl().await) {
        Ok(serialised) => (StatusCode::OK, serialised),
        Err(e) => {
            log::error!("Failed to serialise states: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Vec::<u8>::new())
        }
    }
}

pub fn load_layout_impl() -> Home {
    fs::read_to_string(LAYOUT_PATH).map_or_else(
        |_| template::default(),
        |contents| ron::from_str::<Home>(&contents).unwrap_or_else(|_| template::default()),
    )
}

pub fn save_layout_impl(home: &Home) -> Result<()> {
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

pub async fn get_states_impl() -> StatesPacket {
    let config = get_config();
    let home_assistant =
        HomeAssistantClient::new(&config.home_assistant_url, &config.home_assistant_token).unwrap();

    let states_raw = home_assistant.get_states().await.unwrap_or_default();

    let mut light_states = Vec::new();
    for state_raw in &states_raw {
        if state_raw.entity_id.starts_with("light.") {
            let state_value = match state_raw.state {
                Some(StateEnum::Boolean(x)) => u8::from(x) * 255,
                Some(StateEnum::Decimal(x)) => x.round() as u8,
                Some(StateEnum::Integer(x)) => x as u8,
                Some(StateEnum::String(ref x)) => match x.as_str() {
                    "on" => 255,
                    _ => 0,
                },
                None => 0,
            };

            let state = LightPacket {
                entity_id: state_raw.entity_id.split('.').nth(1).unwrap().to_string(),
                state: state_value,
            };
            light_states.push(state);
        }
    }
    StatesPacket {
        lights: light_states,
    }
}
