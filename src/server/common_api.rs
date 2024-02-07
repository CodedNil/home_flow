use crate::common::layout::Home;

pub fn save_layout(host: &str, home: &Home) -> ehttp::Request {
    post_json(
        &format!("http://{host}/save_layout"),
        &serde_json::to_string(home).unwrap(),
    )
}

pub fn post_json(url: &impl ToString, body: &String) -> ehttp::Request {
    ehttp::Request {
        method: "POST".to_owned(),
        url: url.to_string(),
        body: body.as_bytes().to_vec(),
        headers: ehttp::Headers::new(&[("Accept", "*/*"), ("Content-Type", "application/json")]),
    }
}
