use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
pub mod routing;

#[cfg(feature = "gui")]
pub mod common_api;

#[derive(Debug, Deserialize, Serialize)]
pub struct PostStatesPacket {
    pub entity_id: String,
    pub state: String,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct StatesPacket {
    pub lights: Vec<LightPacket>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LightPacket {
    pub entity_id: String,
    pub state: u8,
}
