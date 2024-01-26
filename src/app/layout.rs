use super::shape::{Material, Shape};
use anyhow::Result;
use image::{ImageBuffer, Rgba};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

const LAYOUT_VERSION: &str = "0.1";
const LAYOUT_PATH: &str = "home_layout.json";
pub const RESOLUTION_FACTOR: f32 = 50.0; // Pixels per meter

#[derive(Serialize, Deserialize, Clone, Debug)]
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
                render_options: RenderOptions {
                    material: Material::Carpet,
                    tint: None,
                },
                render: None,
                pos: Vec2 { x: 0.0, y: 0.0 },
                size: Vec2 { x: 10.0, y: 6.0 },
                operations: vec![Operation {
                    action: Action::Add,
                    shape: Shape::Circle,
                    render_options: Some(RenderOptions {
                        material: Material::Marble,
                        tint: None,
                    }),
                    pos: Vec2 { x: 2.0, y: 3.0 },
                    size: Vec2 { x: 4.0, y: 2.0 },
                }],
            }],
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Room {
    pub name: String,
    pub render_options: RenderOptions,
    #[serde(skip)]
    pub render: Option<RoomRender>,
    pub pos: Vec2,
    pub size: Vec2,
    pub operations: Vec<Operation>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Operation {
    pub action: Action,
    pub shape: Shape,
    pub render_options: Option<RenderOptions>,
    pub pos: Vec2,
    pub size: Vec2,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RenderOptions {
    pub material: Material,
    pub tint: Option<[u8; 3]>,
}

#[derive(Clone, Debug)]
pub struct RoomRender {
    pub texture: ImageBuffer<Rgba<u8>, Vec<u8>>,
    pub center: Vec2,
    pub size: Vec2,
    pub vertices: Vec<Vec2>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Furniture {
    pub pos: Vec2,
    pub size: Vec2,
    pub rotation: f32,
    pub sub_furniture: Vec<Furniture>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Subtract,
    Add,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

static LAYOUT: Lazy<Arc<Mutex<Option<Home>>>> = Lazy::new(|| Arc::new(Mutex::new(None)));

impl Home {
    pub fn load() -> Self {
        let mut layout_lock = LAYOUT.lock().unwrap();
        layout_lock.clone().map_or_else(
            || {
                // Load from file or use default
                let loaded_layout = File::open(LAYOUT_PATH).map_or_else(
                    |_| Self::default(),
                    |mut file| {
                        let mut contents = String::new();
                        file.read_to_string(&mut contents).map_or_else(
                            |_| Self::default(),
                            |_| serde_json::from_str::<Self>(&contents).unwrap_or_else(|_| Self::default()),
                        )
                    },
                );

                // Update the in-memory layout
                *layout_lock = Some(loaded_layout.clone());
                loaded_layout
            },
            |layout| layout,
        )
    }

    pub fn save_memory(&self) {
        let mut layout_lock = LAYOUT.lock().unwrap();
        *layout_lock = Some(self.clone());
    }

    pub fn save(&self) -> Result<()> {
        let mut file = File::create(LAYOUT_PATH)?;
        let contents = serde_json::to_string_pretty(self)?;
        file.write_all(contents.as_bytes())?;
        Ok(())
    }
}
