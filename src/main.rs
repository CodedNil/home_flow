#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    // Set up router
    let app = axum::Router::new().nest_service("/", tower_http::services::ServeDir::new("dist"));

    // Start server
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("Listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[cfg(target_arch = "wasm32")]
mod app;

#[cfg(target_arch = "wasm32")]
fn main() {
    eframe::WebLogger::init(log::LevelFilter::Debug).ok(); // Redirect `log` message to `console.log`

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        eframe::WebRunner::new()
            .start(
                "homeflow_canvas",
                web_options,
                Box::new(|cc| Box::new(app::HomeFlow::new(cc))),
            )
            .await
            .expect("failed to start eframe");
    });
}
