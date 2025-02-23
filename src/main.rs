#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::suboptimal_flops,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    clippy::cognitive_complexity
)]

mod common;

#[cfg(feature = "gui")]
mod client;

#[cfg(not(target_arch = "wasm32"))]
mod server;

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()
        .unwrap();

    // Set up router
    let app = server::routing::setup_routes(
        axum::Router::new()
            .fallback_service(tower_http::services::ServeDir::new("dist"))
            .layer(tower_http::compression::CompressionLayer::new()),
    );

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 8127));
    println!("Listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    tokio::spawn(async move {
        server::routing::start_server().await;
    });

    #[cfg(not(feature = "gui"))]
    axum::serve(listener, app).await.unwrap();

    #[cfg(feature = "gui")]
    {
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let native_options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([400.0, 300.0])
                .with_min_inner_size([300.0, 220.0])
                .with_icon(
                    eframe::icon_data::from_png_bytes(
                        &include_bytes!("../assets/icon-256.png")[..],
                    )
                    .unwrap(),
                ),
            ..Default::default()
        };
        let _ = eframe::run_native(
            "HomeFlow",
            native_options,
            Box::new(|cc| Ok(Box::new(client::HomeFlow::new(cc)))),
        );
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;
    eframe::WebLogger::init(log::LevelFilter::Info).ok(); // Redirect `log` message to `console.log`

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("No window")
            .document()
            .expect("No document");

        let canvas = document
            .get_element_by_id("homeflow_canvas")
            .expect("Failed to find homeflow_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("homeflow_canvas was not a HtmlCanvasElement");

        eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(client::HomeFlow::new(cc)))),
            )
            .await
            .expect("failed to start eframe");
    });
}
