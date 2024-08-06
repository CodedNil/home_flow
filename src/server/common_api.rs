use super::{
    GetStatesPacket, LoginPacket, PostServicesData, PostServicesPacket, SaveLayoutPacket,
    StatesPacket, TokenPacket,
};
use crate::common::layout::Home;
use anyhow::Result;

pub fn get_layout(host: &str, token: &str, on_done: impl 'static + Send + FnOnce(Result<Home>)) {
    ehttp::fetch(
        ehttp::Request::post(
            &format!("http://{host}/load_layout"),
            bincode::serialize(&TokenPacket {
                token: token.to_string(),
            })
            .unwrap(),
        ),
        Box::new(move |res: std::result::Result<ehttp::Response, String>| {
            on_done(match res {
                Ok(res) => match bincode::deserialize(&res.bytes) {
                    Ok(home) => Ok(home),
                    Err(_) => Err(anyhow::anyhow!(
                        "Failed to load layout, status code: {}",
                        res.status
                    )),
                },
                Err(e) => Err(anyhow::anyhow!("Network error loading layout: {}", e)),
            });
        }),
    );
}

pub fn save_layout(
    host: &str,
    token: &str,
    home: &Home,
    on_done: impl 'static + Send + FnOnce(Result<()>),
) {
    ehttp::fetch(
        ehttp::Request::post(
            &format!("http://{host}/save_layout"),
            bincode::serialize(&SaveLayoutPacket {
                token: token.to_string(),
                home: home.clone(),
            })
            .unwrap(),
        ),
        Box::new(move |_| {
            on_done(Ok(()));
        }),
    );
}

pub fn get_states(
    host: &str,
    token: &str,
    sensors: &[String],
    on_done: impl 'static + Send + FnOnce(Result<StatesPacket>),
) {
    ehttp::fetch(
        ehttp::Request::post(
            &format!("http://{host}/get_states"),
            bincode::serialize(&GetStatesPacket {
                token: token.to_string(),
                sensors: sensors.to_vec(),
            })
            .unwrap(),
        ),
        Box::new(move |res: std::result::Result<ehttp::Response, String>| {
            on_done(match res {
                Ok(res) => match bincode::deserialize(&res.bytes) {
                    Ok(states) => Ok(states),
                    Err(_) => Err(anyhow::anyhow!(
                        "Failed to load states, status code: {}",
                        res.status
                    )),
                },
                Err(e) => Err(anyhow::anyhow!("Network error loading states: {}", e)),
            });
        }),
    );
}

pub fn post_state(
    host: &str,
    token: &str,
    packets: &[PostServicesData],
    on_done: impl 'static + Send + FnOnce(Result<()>),
) {
    ehttp::fetch(
        ehttp::Request::post(
            &format!("http://{host}/post_services"),
            bincode::serialize(&PostServicesPacket {
                token: token.to_string(),
                data: packets.to_vec(),
            })
            .unwrap(),
        ),
        Box::new(move |_| {
            on_done(Ok(()));
        }),
    );
}

pub fn login(
    host: &str,
    username: &str,
    password: &str,
    on_done: impl 'static + Send + FnOnce(Result<String>),
) {
    ehttp::fetch(
        ehttp::Request::post(
            &format!("http://{host}/login"),
            bincode::serialize(&LoginPacket {
                username: username.to_string(),
                password: password.to_string(),
            })
            .unwrap(),
        ),
        Box::new(move |res: std::result::Result<ehttp::Response, String>| {
            on_done(match res {
                Ok(res) => res
                    .text()
                    .map(std::string::ToString::to_string)
                    .ok_or_else(|| anyhow::anyhow!("Failed to extract text from response")),
                Err(e) => Err(anyhow::anyhow!("Failed to login: {}", e)),
            });
        }),
    );
}
