use crate::{
    common::{
        furniture::Furniture, layout::DataPoint, HAState, PostActionsData, PostActionsPacket,
        TokenPacket,
    },
    server::{auth::verify_token, presence, routing::HOME},
};
use ahash::AHashMap;
use anyhow::Result;
use axum::{body::Bytes, http::StatusCode, response::IntoResponse};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    env,
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc, LazyLock,
    },
};
use tokio::{net::TcpStream, sync::Mutex};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

fn get_env_variable(key: &str) -> String {
    match env::var(key) {
        Ok(value) => value,
        Err(e) => match e {
            env::VarError::NotPresent => panic!("Environment variable {key} not found."),
            env::VarError::NotUnicode(oss) => {
                panic!("Environment variable {key} contains invalid data: {oss:?}")
            }
        },
    }
}

static HA_STATE: LazyLock<Mutex<Option<HAState>>> = LazyLock::new(|| Mutex::new(None));

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
static WS_STREAM: LazyLock<Arc<Mutex<Option<WsStream>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(None)));

pub async fn get_states_server(body: Bytes) -> impl IntoResponse {
    let packet: TokenPacket = match bincode::deserialize(&body) {
        Ok(packet) => packet,
        Err(e) => {
            log::error!("Failed to deserialize get_states_server packet: {:?}", e);
            return (StatusCode::BAD_REQUEST, Vec::new());
        }
    };
    if !verify_token(&packet.token).await.unwrap_or(false) {
        return (StatusCode::UNAUTHORIZED, Vec::new());
    }

    let ha_state = HA_STATE.lock().await;
    ha_state.as_ref().map_or_else(
        || {
            log::error!("State not found in memory");
            (StatusCode::INTERNAL_SERVER_ERROR, Vec::new())
        },
        |states| match bincode::serialize(states) {
            Ok(serialized) => (StatusCode::OK, serialized),
            Err(e) => {
                log::error!("Failed to serialize states: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Vec::new())
            }
        },
    )
}

pub async fn post_actions_server(body: Bytes) -> impl IntoResponse {
    let packet: PostActionsPacket = match bincode::deserialize(&body) {
        Ok(packet) => packet,
        Err(e) => {
            log::error!("Failed to deserialize post_actions_server packet: {:?}", e);
            return StatusCode::BAD_REQUEST.into_response();
        }
    };
    if !verify_token(&packet.token).await.unwrap_or(false) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    post_actions_impl(packet.data).await;
    StatusCode::OK.into_response()
}

#[derive(Debug, Deserialize)]
pub struct HassState {
    pub entity_id: String,
    pub state: String,
    #[allow(dead_code)]
    last_changed: String,
    attributes: AHashMap<String, serde_json::Value>,
}

pub async fn run_server() -> Result<()> {
    // Connect to the WebSocket
    let (mut ws_stream, _) = connect_async(format!(
        "ws://{}/api/websocket",
        get_env_variable("HASS_URL")
    ))
    .await?;

    // Send authentication message
    ws_stream
        .send(Message::Text(
            json!({"type": "auth", "access_token": get_env_variable("HASS_TOKEN")}).to_string(),
        ))
        .await?;

    *WS_STREAM.lock().await = Some(ws_stream);

    // Listen for messages
    let mut slow_refresh_interval = tokio::time::interval(std::time::Duration::from_secs(2));
    loop {
        tokio::select! {
            // WebSocket message handling
            message = async {
                let mut ws_stream = WS_STREAM.lock().await;
                if let Some(ref mut ws_stream) = *ws_stream {
                    ws_stream.next().await
                } else {
                    None
                }
            } => {
                if let Some(message) = message {
                    handle_ws_message(message).await?;
                }
            }

            // Slow refresh interval for presence calculation
            _ = slow_refresh_interval.tick() => {
                let mut ha_state = HA_STATE.lock().await;
                if let Some(state) = ha_state.as_mut() {
                    let presence_points = presence::calculate(&state.sensors).await?;
                    state.presence_points = presence_points;
                }
            }
        }
    }
}

async fn handle_ws_message(
    message: Result<Message, tokio_tungstenite::tungstenite::Error>,
) -> Result<()> {
    match message {
        Ok(Message::Text(txt)) => {
            let mut response: Value = serde_json::from_str(&txt)?;
            if response["type"] == "auth_ok" {
                let mut ws_stream = WS_STREAM.lock().await;
                if let Some(ref mut ws_stream) = *ws_stream {
                    ws_stream
                            .send(Message::Text(
                                json!({"id": 1, "type": "subscribe_events", "event_type": "state_changed"})
                                    .to_string(),
                            ))
                            .await?;
                    ws_stream
                        .send(Message::Text(
                            json!({"id": 2, "type": "get_states"}).to_string(),
                        ))
                        .await?;
                }
            } else if response["type"] == "event"
                && response["id"].as_u64() == Some(1)
                && response["event"]["event_type"] == "state_changed"
            {
                process_state(&response["event"]["data"]).await?;
            } else if response["type"] == "result" && response["id"].as_u64() == Some(2) {
                if let Err(e) = process_full_states(response["result"].take()).await {
                    log::error!("{}", e);
                }
            }
        }
        Err(e) => {
            log::error!("{}", e);
        }
        _ => {}
    }
    Ok(())
}

async fn process_full_states(states_raw: Value) -> Result<()> {
    let states_raw = serde_json::from_value::<Vec<HassState>>(states_raw)?;

    let target_sensors = get_target_sensors().await;
    let mut lights = AHashMap::new();
    let mut sensors = AHashMap::new();

    for state_raw in &states_raw {
        if let Some((domain, entity_id)) = state_raw.entity_id.split_once('.') {
            match domain {
                "light" => {
                    lights.insert(
                        entity_id.to_string(),
                        state_raw
                            .attributes
                            .get("brightness")
                            .and_then(serde_json::Value::as_u64)
                            .map_or_else(
                                || if state_raw.state == "on" { 255 } else { 0 },
                                |b| b as u8,
                            ),
                    );
                }
                "sensor" if target_sensors.contains(&entity_id.to_string()) => {
                    sensors.insert(entity_id.to_string(), state_raw.state.clone());
                }
                _ => {}
            }
        }
    }

    let presence_points = presence::calculate(&sensors).await?;

    // Update the state
    *HA_STATE.lock().await = Some(HAState {
        lights,
        sensors,
        presence_points,
    });
    Ok(())
}

async fn process_state(data: &Value) -> Result<()> {
    let target_sensors = get_target_sensors().await;
    let entity_id = data["entity_id"].as_str().unwrap();
    let new_state = &data["new_state"];

    let mut ha_state = HA_STATE.lock().await;
    let mut needs_presence_update = false;
    if let Some((domain, id)) = entity_id.split_once('.') {
        if let Some(ha_state) = ha_state.as_mut() {
            match domain {
                "light" => {
                    ha_state.lights.insert(
                        id.to_string(),
                        new_state["attributes"]["brightness"].as_u64().map_or_else(
                            || if new_state["state"] == "on" { 255 } else { 0 },
                            |b| b as u8,
                        ),
                    );
                }
                "sensor" if target_sensors.contains(&entity_id.to_string()) => {
                    ha_state.sensors.insert(
                        entity_id.to_string(),
                        new_state["state"].as_str().unwrap_or("unknown").to_string(),
                    );
                    if entity_id == "input_boolean.presence_calibration" {
                        needs_presence_update = true;
                    } else if let Some((_, suffix)) = entity_id.split_once("_target_") {
                        if suffix.contains("_x") || suffix.contains("_y") {
                            needs_presence_update = true;
                        }
                    }
                }
                _ => {}
            }
        }
    }
    drop(ha_state);

    if needs_presence_update {
        let mut ha_state = HA_STATE.lock().await;
        if let Some(state) = ha_state.as_mut() {
            let presence_points = presence::calculate(&state.sensors).await?;
            state.presence_points = presence_points;
        }
    }

    Ok(())
}

const DEFAULT_SENSORS: &[&str] = &["input_boolean.presence_calibration"];

async fn get_target_sensors() -> Vec<String> {
    HOME.lock()
        .await
        .rooms
        .iter()
        .flat_map(|room| {
            room.sensors
                .iter()
                .map(|sensor| sensor.entity_id.clone())
                .chain(room.furniture.iter().flat_map(Furniture::wanted_sensors))
        })
        .chain(DEFAULT_SENSORS.iter().map(ToString::to_string))
        .collect()
}

static NEXT_ID: LazyLock<AtomicI64> = LazyLock::new(|| AtomicI64::new(3));

pub async fn post_actions_impl(data: Vec<PostActionsData>) {
    let mut new_actions = Vec::new();
    for param in data {
        let service_data = json!(param
            .additional_data
            .into_iter()
            .map(|(key, value)| (
                key,
                match value {
                    DataPoint::String(s) => serde_json::Value::String(s),
                    DataPoint::Float(f) =>
                        serde_json::Value::Number(serde_json::Number::from_f64(f).unwrap()),
                    DataPoint::Int(i) => serde_json::Value::Number(serde_json::Number::from(i)),
                    DataPoint::Vec2(v) => serde_json::json!([v.x, v.y]),
                    DataPoint::Vec4((a, b, c, d)) => serde_json::json!([a, b, c, d]),
                }
            ))
            .collect::<serde_json::Map<_, _>>());

        new_actions.push(json!({
            "id": NEXT_ID.fetch_add(1, Ordering::SeqCst),
            "type": "call_service",
            "domain": param.domain,
            "service": param.action,
            "service_data": service_data,
            "target": {
                "entity_id": param.entity_id,
            }
        }));
    }
    let mut ws_stream = WS_STREAM.lock().await;
    if let Some(ref mut ws_stream) = *ws_stream {
        for action in new_actions {
            if let Err(e) = ws_stream.send(Message::Text(action.to_string())).await {
                log::error!("Failed to send action: {:?}", e);
            }
        }
    }
}
