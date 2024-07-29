use super::{PostStatesPacket, StatesPacket};
use crate::common::layout::Home;
use anyhow::Result;

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

pub fn post_state(
    host: &str,
    packets: &Vec<PostStatesPacket>,
    on_done: impl 'static + Send + FnOnce(Result<()>),
) {
    ehttp::fetch(
        ehttp::Request::post(
            &format!("http://{host}/post_state"),
            bincode::serialize(packets).unwrap(),
        ),
        Box::new(move |_| {
            on_done(Ok(()));
        }),
    );
}
