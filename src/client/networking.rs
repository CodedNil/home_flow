use crate::common::{HAState, LoginPacket, PostActionsData, PostActionsPacket, TokenPacket};
use anyhow::Result;

pub fn get_states(host: &str, token: &str, on_done: impl 'static + Send + FnOnce(Result<HAState>)) {
    ehttp::fetch(
        ehttp::Request::post(
            format!("http://{host}/get_states"),
            postcard::to_allocvec(&TokenPacket {
                token: token.to_string(),
            })
            .unwrap(),
        ),
        Box::new(move |res: std::result::Result<ehttp::Response, String>| {
            on_done(match res {
                Ok(res) => {
                    if res.status == 200 {
                        postcard::from_bytes(&res.bytes)
                            .map_or_else(|_| Err(anyhow::anyhow!("Failed to load states")), Ok)
                    } else {
                        Err(anyhow::anyhow!(
                            "Failed to load states, status code: {}",
                            res.status
                        ))
                    }
                }
                Err(e) => Err(anyhow::anyhow!("Network error loading states: {e}")),
            });
        }),
    );
}

pub fn post_actions(
    host: &str,
    token: &str,
    data: &[PostActionsData],
    on_done: impl 'static + Send + FnOnce(Result<()>),
) {
    ehttp::fetch(
        ehttp::Request::post(
            format!("http://{host}/post_actions"),
            postcard::to_allocvec(&PostActionsPacket {
                token: token.to_string(),
                data: data.to_vec(),
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
            format!("http://{host}/login"),
            postcard::to_allocvec(&LoginPacket {
                username: username.to_string(),
                password: password.to_string(),
            })
            .unwrap(),
        ),
        Box::new(move |res: std::result::Result<ehttp::Response, String>| {
            on_done(match res {
                Ok(res) => {
                    if res.status == 200 {
                        res.text()
                            .map(std::string::ToString::to_string)
                            .ok_or_else(|| anyhow::anyhow!("Failed to extract text from response"))
                    } else {
                        Err(anyhow::anyhow!(
                            "Login failed: {}",
                            res.text().unwrap_or_default()
                        ))
                    }
                }
                Err(e) => Err(anyhow::anyhow!("Failed to login: {e}")),
            });
        }),
    );
}
