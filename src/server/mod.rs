use crate::common::layout::Home;
use anyhow::Result;
use axum::{response::IntoResponse, routing::get, Router};
use std::{
    fs::File,
    io::{Read, Write},
};

const LAYOUT_PATH: &str = "home_layout.json";

pub fn setup_routes(app: Router) -> Router {
    app.route("/load_layout", get(load_layout))
}

pub async fn load_layout() -> impl IntoResponse {
    serde_json::to_string(&load_file()).unwrap()
}

pub fn load_file() -> Home {
    // Load from file or use default
    File::open(LAYOUT_PATH).map_or_else(
        |_| Home::default(),
        |mut file| {
            let mut contents = String::new();
            file.read_to_string(&mut contents).map_or_else(
                |_| Home::default(),
                |_| serde_json::from_str::<Home>(&contents).unwrap_or_else(|_| Home::template()),
            )
        },
    )
}

pub fn save_file(home: &Home) -> Result<()> {
    let mut file = File::create(LAYOUT_PATH)?;
    let contents = serde_json::to_string_pretty(home)?;
    file.write_all(contents.as_bytes())?;
    Ok(())
}
