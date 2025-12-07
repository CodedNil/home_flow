use crate::{
    common::{layout::Home, template},
    server::{
        auth::login_server,
        home_assistant::{get_states_server, post_actions_server},
    },
};
use axum::{Router, routing::post};
use std::sync::LazyLock;
use tokio::sync::Mutex;

pub fn setup_routes(app: Router) -> Router {
    app.route("/get_states", post(get_states_server))
        .route("/post_actions", post(post_actions_server))
        .route("/login", post(login_server))
}

pub static HOME: LazyLock<Mutex<Home>> = LazyLock::new(|| Mutex::new(template::default()));

pub async fn start_server() {
    *HOME.lock().await = template::default();

    loop {
        match super::home_assistant::run_server().await {
            Ok(()) => {}
            Err(e) => {
                log::error!("Home assistant websocket error: {e:?}");
            }
        }
        log::info!("Attempting to reconnect websocket");
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}
