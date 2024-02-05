use crate::common::layout::Home;
use anyhow::{anyhow, Result};
use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use std::{fs, path::Path};
use time::{format_description, OffsetDateTime};

const LAYOUT_PATH: &str = "home_layout.json";

pub fn setup_routes(app: Router) -> Router {
    app.route("/load_layout", get(load_layout))
        .route("/save_layout", post(save_layout))
}

pub async fn load_layout() -> impl IntoResponse {
    let home_json = fs::read_to_string(LAYOUT_PATH).map_or_else(
        |_| serde_json::to_string(&Home::template()).unwrap_or_default(),
        |contents| match serde_json::from_str::<Home>(&contents) {
            Ok(_) => contents,
            Err(_) => serde_json::to_string(&Home::template()).unwrap_or_default(),
        },
    );

    (StatusCode::OK, home_json).into_response()
}

pub async fn save_layout(home: String) -> impl IntoResponse {
    log::info!("Saving layout");
    match save_layout_impl(&home) {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => {
            log::error!("Failed to save layout: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

fn save_layout_impl(home: &str) -> Result<()> {
    let temp_path = Path::new(LAYOUT_PATH).with_extension("tmp");
    fs::write(&temp_path, home).map_err(|e| anyhow!("Failed to write temporary layout: {}", e))?;

    if Path::new(LAYOUT_PATH).exists() {
        let metadata = fs::metadata(LAYOUT_PATH)?;
        let modified_time = metadata.modified()?;
        let modified_time = OffsetDateTime::from(modified_time);
        let format = format_description::parse("[year]-[month]-[day]_[hour]-[minute]-[second]")?;
        let backup_filename = format!(
            "backups/home_layout_{}.json",
            modified_time.format(&format)?
        );

        fs::create_dir_all("backups")?;
        fs::rename(LAYOUT_PATH, backup_filename)?;
    }

    fs::rename(&temp_path, LAYOUT_PATH)?;
    Ok(())
}
