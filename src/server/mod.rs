use crate::app::layout;
use axum::{response::IntoResponse, routing::get, Router};

pub fn setup_routes(app: Router) -> Router {
    app.route("/load_layout", get(load_layout))
}

pub async fn load_layout() -> impl IntoResponse {
    serde_json::to_string(&layout::Home::load_file()).unwrap()
}
