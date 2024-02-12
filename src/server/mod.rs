#[cfg(not(target_arch = "wasm32"))]
pub mod routing;

#[cfg(feature = "gui")]
pub mod common_api;

#[cfg(target_arch = "wasm32")]
pub mod fetch;
