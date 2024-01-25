use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::File;
use std::io::{Read, Write};

const LAYOUT_VERSION: &str = "0.1";
const LAYOUT_PATH: &str = "home_layout.json";

#[derive(Serialize, Deserialize, Debug)]
pub struct Home {
    version: String,
    pub rooms: Vec<Room>,
}

impl Default for Home {
    fn default() -> Self {
        Self {
            version: LAYOUT_VERSION.to_string(),
            rooms: vec![Room {
                name: "Living Room".to_string(),
                pos: Vec2 { x: 0.0, y: 0.0 },
                size: Vec2 { x: 10.0, y: 10.0 },
                operations: Vec::new(),
            }],
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Room {
    pub name: String,
    pub pos: Vec2,
    pub size: Vec2,
    pub operations: Vec<Operation>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Operation {
    pub action: Action,
    pub pos: Vec2,
    pub size: Vec2,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Action {
    Subtract,
    Add,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Shape {
    Rectangle,
    Circle,
}

impl Home {
    pub fn load() -> Self {
        if let Ok(mut file) = File::open(LAYOUT_PATH) {
            let mut contents = String::new();
            if file.read_to_string(&mut contents).is_ok() {
                if let Ok(json) = serde_json::from_str::<Value>(&contents) {
                    if let Some(version) = json.get("version").and_then(Value::as_str) {
                        if version != LAYOUT_VERSION {
                            return upgrade_layout_version(&contents, version);
                        }
                    }
                }
                if let Ok(layout) = serde_json::from_str::<Self>(&contents) {
                    return layout;
                }
            }
        }
        let default_layout = Self::default();
        let _ = default_layout.save();
        default_layout
    }

    pub fn save(&self) -> Result<()> {
        let mut file = File::create(LAYOUT_PATH)?;
        let contents = serde_json::to_string_pretty(self)?;
        file.write_all(contents.as_bytes())?;
        Ok(())
    }
}

fn upgrade_layout_version(raw_json: &str, version: &str) -> Home {
    match version {
        "0.0" => Home::default(),
        _ => Home::default(),
    }
}
