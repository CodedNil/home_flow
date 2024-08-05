use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[cfg(not(target_arch = "wasm32"))]
pub mod routing;

#[cfg(feature = "gui")]
pub mod common_api;

#[derive(Debug, Deserialize, Serialize)]
pub struct PostServicesPacket {
    pub entity_id: String,
    pub domain: String,
    pub service: String,
    pub additional_data: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct StatesPacket {
    pub lights: Vec<LightPacket>,
    pub sensors: Vec<SensorPacket>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LightPacket {
    pub entity_id: String,
    pub state: u8,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SensorPacket {
    pub entity_id: String,
    pub state: String,
}
