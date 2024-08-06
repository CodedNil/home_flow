use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::common::layout::Home;

#[cfg(not(target_arch = "wasm32"))]
pub mod auth;
#[cfg(not(target_arch = "wasm32"))]
pub mod home_assistant;
#[cfg(not(target_arch = "wasm32"))]
pub mod routing;

#[cfg(feature = "gui")]
pub mod common_api;

// Packets for communication between the client to the server
#[derive(Serialize, Deserialize)]
pub struct TokenPacket {
    pub token: String,
}

#[derive(Serialize, Deserialize)]
pub struct SaveLayoutPacket {
    pub token: String,
    pub home: Home,
}

#[derive(Serialize, Deserialize)]
pub struct GetStatesPacket {
    pub token: String,
    pub sensors: Vec<String>,
}

nestify::nest! {
    #[derive(Serialize, Deserialize, Clone)]*
    pub struct PostServicesPacket {
        pub token: String,
        pub data: Vec<pub struct PostServicesData {
            pub entity_id: String,
            pub domain: String,
            pub service: String,
            pub additional_data: HashMap<String, serde_json::Value>,
        }>,
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct LoginPacket {
    username: String,
    password: String,
}

// Packet for communication between the server to the client
nestify::nest! {
    #[derive(Debug, Deserialize, Serialize, Default, Clone)]*
    pub struct StatesPacket {
        pub lights: Vec<pub struct LightPacket {
            pub entity_id: String,
            pub state: u8,
        }>,
        pub sensors: Vec<pub struct SensorPacket {
            pub entity_id: String,
            pub state: String,
        }>,
    }
}
