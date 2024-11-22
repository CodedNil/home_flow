use crate::common::layout::{DataPoint, Home};
use ahash::AHashMap;
use glam::DVec2;
use serde::{Deserialize, Serialize};

pub mod color;
pub mod furniture;
pub mod geo_buffer;
pub mod layout;
pub mod shape;
pub mod template;
pub mod utils;

// Packet for communication between the server to the client
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct HAState {
    pub lights: AHashMap<String, u8>,
    pub sensors: AHashMap<String, String>,
    pub presence_points: Vec<DVec2>,
}

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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PostActionsPacket {
    pub token: String,
    pub data: Vec<PostActionsData>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PostActionsData {
    pub entity_id: String,
    pub domain: String,
    pub action: String,
    pub additional_data: AHashMap<String, DataPoint>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LoginPacket {
    pub username: String,
    pub password: String,
}
