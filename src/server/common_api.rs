use crate::common::layout::Home;
use anyhow::Result;
use serde::{Deserialize, Serialize};

pub fn get_layout(host: &str, on_done: impl 'static + Send + FnOnce(Result<Home>)) {
    ehttp::fetch(
        ehttp::Request::get(&format!("http://{host}/load_layout")),
        Box::new(move |res: std::result::Result<ehttp::Response, String>| {
            on_done(match res {
                Ok(res) => bincode::deserialize(&res.bytes)
                    .map_err(|e| anyhow::anyhow!("Failed to load layout: {}", e)),
                Err(e) => Err(anyhow::anyhow!("Failed to load layout: {}", e)),
            });
        }),
    );
}

pub fn save_layout(host: &str, home: &Home, on_done: impl 'static + Send + FnOnce(Result<()>)) {
    ehttp::fetch(
        ehttp::Request::post(
            &format!("http://{host}/save_layout"),
            bincode::serialize(home).unwrap(),
        ),
        Box::new(move |_| {
            on_done(Ok(()));
        }),
    );
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

pub fn get_states(host: &str, on_done: impl 'static + Send + FnOnce(Result<StatesPacket>)) {
    ehttp::fetch(
        ehttp::Request::get(&format!("http://{host}/get_states")),
        Box::new(move |res: std::result::Result<ehttp::Response, String>| {
            on_done(match res {
                Ok(res) => bincode::deserialize(&res.bytes)
                    .map_err(|e| anyhow::anyhow!("Failed to load states: {}", e)),
                Err(e) => Err(anyhow::anyhow!("Failed to load states: {}", e)),
            });
        }),
    );
}
