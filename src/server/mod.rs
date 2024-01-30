use crate::common::layout::Home;
use anyhow::Result;
use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use std::{
    fs::File,
    io::{Read, Write},
};

const LAYOUT_PATH: &str = "home_layout.json";

pub fn setup_routes(app: Router) -> Router {
    app.route("/load_layout", get(load_layout))
        .route("/save_layout", post(save_layout))
}

pub async fn load_layout() -> impl IntoResponse {
    let result = || -> Result<String> {
        let Ok(mut file) = File::open(LAYOUT_PATH) else {
            return Ok(serde_json::to_string(&Home::template())?);
        };

        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let home: Home = serde_json::from_str(&contents).unwrap_or_else(|_| Home::template());
        Ok(serde_json::to_string(&home)?)
    }();

    result.map_or_else(
        |_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load layout").into_response(),
        IntoResponse::into_response,
    )
}
pub async fn save_layout(home: String) -> impl IntoResponse {
    log::info!("Saving layout");
    let result = || -> Result<()> {
        let mut file = File::create(LAYOUT_PATH)?;
        let home: Home = serde_json::from_str(&home)?;
        let contents = serde_json::to_string_pretty(&home)?;
        file.write_all(contents.as_bytes())?;
        Ok(())
    }();

    match result {
        Ok(()) => StatusCode::OK.into_response(),
        Err(response) => {
            log::error!("Failed to save layout: {}", response);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to save layout").into_response()
        }
    }
}
