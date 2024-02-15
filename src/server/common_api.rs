use crate::common::layout::Home;
use anyhow::Result;

#[allow(unused_variables)]
pub fn get_layout(host: &str, on_done: impl 'static + Send + FnOnce(Result<Home>)) {
    #[cfg(not(target_arch = "wasm32"))]
    on_done(Ok(super::routing::load_layout_impl()));

    #[cfg(target_arch = "wasm32")]
    super::fetch::fetch(
        super::fetch::Request::get(format!("http://{host}/load_layout")),
        Box::new(move |res| {
            on_done(match res {
                Ok(res) => bincode::deserialize(&res.bytes)
                    .map_err(|e| anyhow::anyhow!("Failed to load layout: {}", e)),
                Err(e) => Err(anyhow::anyhow!("Failed to load layout: {}", e)),
            });
        }),
    );
}

#[allow(unused_variables)]
pub fn save_layout(host: &str, home: &Home, on_done: impl 'static + Send + FnOnce(Result<()>)) {
    #[cfg(not(target_arch = "wasm32"))]
    on_done(super::routing::save_layout_impl(home));

    #[cfg(target_arch = "wasm32")]
    super::fetch::fetch(
        super::fetch::Request::post(
            format!("http://{host}/save_layout"),
            bincode::serialize(home).unwrap(),
        ),
        Box::new(move |_| {
            on_done(Ok(()));
        }),
    );
}

#[cfg(target_arch = "wasm32")]
pub fn get_wall_shadows(
    host: &str,
    wall_polygons_hash: u64,
    wall_polygons: &[geo_types::MultiPolygon],
    on_done: impl 'static + Send + FnOnce(Option<(u64, Vec<crate::common::shape::ShadowTriangles>)>),
) {
    if self.host.contains("github.io") {
        on_done(None);
        return;
    }
    super::fetch::fetch(
        super::fetch::Request::post(
            format!("http://{host}/wall_shadows"),
            bincode::serialize(wall_polygons).unwrap(),
        ),
        Box::new(move |res| {
            on_done(match res {
                Ok(res) => bincode::deserialize(&res.bytes)
                    .map(|shadows| Some((wall_polygons_hash, shadows)))
                    .unwrap_or_else(|e| {
                        log::error!("Failed to load wall shadows: {}", e);
                        None
                    }),
                Err(e) => {
                    log::error!("Failed to load wall shadows: {}", e);
                    None
                }
            });
        }),
    );
}
