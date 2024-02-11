use crate::common::layout::Home;
use anyhow::Result;

pub fn get_layout(host: &str, on_done: impl 'static + Send + FnOnce(Result<Home>)) {
    #[cfg(not(target_arch = "wasm32"))]
    on_done(Ok(super::routing::load_layout_impl()));

    #[cfg(target_arch = "wasm32")]
    ehttp::fetch(
        ehttp::Request::get(format!("http://{host}/load_layout")),
        move |res| {
            on_done(match res {
                Ok(res) => bincode::deserialize(&res.bytes)
                    .map_err(|e| anyhow::anyhow!("Failed to load layout: {}", e)),
                Err(e) => Err(anyhow::anyhow!("Failed to load layout: {}", e)),
            });
        },
    );
}

pub fn save_layout(host: &str, home: &Home, on_done: impl 'static + Send + FnOnce(Result<()>)) {
    #[cfg(not(target_arch = "wasm32"))]
    on_done(super::routing::save_layout_impl(home));

    #[cfg(target_arch = "wasm32")]
    ehttp::fetch(
        ehttp::Request {
            method: "POST".to_owned(),
            url: format!("http://{host}/save_layout"),
            body: bincode::serialize(home).unwrap(),
            headers: ehttp::Headers::new(&[("Accept", "*/*")]),
        },
        move |_| {
            on_done(Ok(()));
        },
    );
}
