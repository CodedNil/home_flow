use crate::{
    common::{layout::Home, template, SaveLayoutPacket, TokenPacket},
    server::{
        auth::{login_server, verify_token},
        home_assistant::{get_states_server, post_actions_server},
    },
};
use anyhow::{anyhow, Result};
use axum::{body::Bytes, http::StatusCode, response::IntoResponse, routing::post, Router};
use std::{path::Path, sync::LazyLock};
use time::{format_description, OffsetDateTime};
use tokio::{fs, sync::Mutex};

const LAYOUT_PATH: &str = "home_layout.ron";

pub fn setup_routes(app: Router) -> Router {
    app.route("/load_layout", post(load_layout_server))
        .route("/save_layout", post(save_layout_server))
        .route("/get_states", post(get_states_server))
        .route("/post_actions", post(post_actions_server))
        .route("/login", post(login_server))
}

pub static HOME: LazyLock<Mutex<Home>> = LazyLock::new(|| Mutex::new(template::default()));

pub async fn start_server() {
    *HOME.lock().await = fs::read_to_string(LAYOUT_PATH)
        .await
        .ok()
        .and_then(|data| ron::from_str::<Home>(&data).ok())
        .unwrap_or_else(template::default);

    super::home_assistant::server_loop().await;
}

async fn load_layout_server(body: Bytes) -> impl IntoResponse {
    let packet: TokenPacket = match bincode::deserialize(&body) {
        Ok(packet) => packet,
        Err(e) => {
            log::error!("Failed to deserialize load_layout_server packet: {:?}", e);
            return (StatusCode::BAD_REQUEST, Vec::new());
        }
    };
    if !matches!(verify_token(&packet.token), Ok(true)) {
        return (StatusCode::UNAUTHORIZED, Vec::new());
    }

    // Load layout from memory and serialize
    let home = HOME.lock().await;
    match bincode::serialize(&*home) {
        Ok(serialized) => (StatusCode::OK, serialized),
        Err(e) => {
            log::error!("Failed to serialize layout: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Vec::new())
        }
    }
}

async fn save_layout_server(body: Bytes) -> impl IntoResponse {
    let packet: SaveLayoutPacket = match bincode::deserialize(&body) {
        Ok(packet) => packet,
        Err(e) => {
            log::error!("Failed to deserialize save_layout_server packet: {:?}", e);
            return StatusCode::BAD_REQUEST.into_response();
        }
    };
    if !matches!(verify_token(&packet.token), Ok(true)) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    // Save layout to file
    log::info!("Saving layout");
    if let Err(e) = save_layout_impl(&packet.home).await {
        log::error!("Failed to save layout: {:?}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    // Update the in-memory layout
    *HOME.lock().await = packet.home;

    StatusCode::OK.into_response()
}

async fn save_layout_impl(home: &Home) -> Result<()> {
    let home_ron = ron::ser::to_string_pretty(home, ron::ser::PrettyConfig::default())?;
    let temp_path = Path::new(LAYOUT_PATH).with_extension("tmp");
    fs::write(&temp_path, home_ron)
        .await
        .map_err(|e| anyhow!("Failed to write temporary layout: {}", e))?;

    if Path::new(LAYOUT_PATH).exists() {
        let metadata = fs::metadata(LAYOUT_PATH).await?;
        let modified_time = metadata.modified()?;
        let modified_time = OffsetDateTime::from(modified_time);
        let format = format_description::parse("[year]-[month]-[day]_[hour]-[minute]-[second]")?;
        let backup_filename = format!("backups/home_layout_{}.ron", modified_time.format(&format)?);

        fs::create_dir_all("backups").await?;
        fs::rename(LAYOUT_PATH, backup_filename).await?;
    }

    fs::rename(&temp_path, LAYOUT_PATH).await?;
    Ok(())
}
