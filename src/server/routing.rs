use crate::common::{layout::Home, template::template_home};
use anyhow::{anyhow, Result};
use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use std::{fs, path::Path};
use time::{format_description, OffsetDateTime};

const LAYOUT_PATH: &str = "home_layout.toml";

pub fn setup_routes(app: Router) -> Router {
    app.route("/load_layout", get(load_layout_server))
        .route("/save_layout", post(save_layout_server))
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

pub fn load_layout_impl() -> Home {
    fs::read_to_string(LAYOUT_PATH).map_or_else(
        |_| template_home(),
        |contents| toml::from_str::<Home>(&contents).map_or_else(|_| template_home(), |home| home),
    )
}

pub fn save_layout_impl(home: &Home) -> Result<()> {
    let home_toml = toml::to_string(home)?;
    let temp_path = Path::new(LAYOUT_PATH).with_extension("tmp");
    fs::write(&temp_path, home_toml)
        .map_err(|e| anyhow!("Failed to write temporary layout: {}", e))?;

    if Path::new(LAYOUT_PATH).exists() {
        let metadata = fs::metadata(LAYOUT_PATH)?;
        let modified_time = metadata.modified()?;
        let modified_time = OffsetDateTime::from(modified_time);
        let format = format_description::parse("[year]-[month]-[day]_[hour]-[minute]-[second]")?;
        let backup_filename = format!(
            "backups/home_layout_{}.toml",
            modified_time.format(&format)?
        );

        fs::create_dir_all("backups")?;
        fs::rename(LAYOUT_PATH, backup_filename)?;
    }

    fs::rename(&temp_path, LAYOUT_PATH)?;
    Ok(())
}
