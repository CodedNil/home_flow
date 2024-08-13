use crate::common::layout::{DataPoint, Home};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    #[derive(Debug, Serialize, Deserialize, Clone)]*
    pub struct PostActionsPacket {
        pub token: String,
        pub data: Vec<pub struct PostActionsData {
            pub entity_id: String,
            pub domain: String,
            pub action: String,
            pub additional_data: HashMap<String, DataPoint>,
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
