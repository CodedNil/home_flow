use super::shape::{Material, Shape, WallType};
use anyhow::Result;
use egui::Color32;
use image::{ImageBuffer, Rgba};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use strum_macros::{Display, VariantArray};

const LAYOUT_VERSION: &str = "0.1";
const LAYOUT_PATH: &str = "home_layout.json";
pub const RESOLUTION_FACTOR: f32 = 80.0; // Pixels per meter

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Home {
    pub version: String,
    pub rooms: Vec<Room>,
    pub furniture: Vec<Furniture>,
    pub walls: Vec<Wall>,
    #[serde(skip)]
    pub rendered_data: Option<HomeRender>,
}

impl Hash for Home {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.version.hash(state);
        self.rooms.hash(state);
        self.furniture.hash(state);
        self.walls.hash(state);
    }
}

impl Home {
    pub fn template() -> Self {
        Self {
            version: LAYOUT_VERSION.to_string(),
            rooms: vec![
                Room::new(
                    "Kitchen",
                    Vec2::new(-1.3, 2.85),
                    Vec2::new(3.5, 3.0),
                    RenderOptions::new(
                        Material::Marble,
                        2.0,
                        Some("#fff8e8"),
                        Some(TileOptions::new(7, "#ffffff00", 0.02, "#505050cc")),
                    ),
                    vec![
                        WallType::Exterior,
                        WallType::Exterior,
                        WallType::None,
                        WallType::None,
                    ],
                    vec![],
                ),
                Room::new(
                    "Lounge",
                    Vec2::new(0.0, 0.0),
                    Vec2::new(6.1, 2.7),
                    RenderOptions::new(Material::Carpet, 1.0, None, None),
                    vec![
                        WallType::Exterior,
                        WallType::None,
                        WallType::Interior,
                        WallType::Exterior,
                    ],
                    vec![],
                ),
                Room::new(
                    "Bathroom",
                    Vec2::new(3.85, 0.0),
                    Vec2::new(1.6, 2.7),
                    RenderOptions::new(
                        Material::Granite,
                        2.0,
                        Some("#fff8e8"),
                        Some(TileOptions::new(4, "#ffffff00", 0.025, "#505050cc")),
                    ),
                    vec![
                        WallType::Interior,
                        WallType::Interior,
                        WallType::Interior,
                        WallType::Exterior,
                    ],
                    vec![Operation {
                        action: Action::Subtract,
                        shape: Shape::Rectangle,
                        render_options: RenderOptions::default(),
                        pos: Vec2::new(0.4, 2.7 / 2.0 - 0.5),
                        size: Vec2::new(0.8, 1.0),
                    }],
                ),
            ],
            furniture: vec![],
            walls: vec![],
            rendered_data: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Hash)]
pub struct Room {
    pub id: uuid::Uuid,
    pub name: String,
    pub render_options: RenderOptions,
    pub pos: Vec2,
    pub size: Vec2,
    pub operations: Vec<Operation>,
    pub walls: Vec<WallType>, // Left, Top, Right, Bottom
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Furniture {
    pub pos: Vec2,
    pub size: Vec2,
    pub rotation: f32,
    pub children: Vec<Furniture>,
}

#[derive(Serialize, Deserialize, Clone, Default, Hash)]
pub struct Operation {
    pub action: Action,
    pub shape: Shape,
    pub render_options: RenderOptions,
    pub pos: Vec2,
    pub size: Vec2,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct RenderOptions {
    pub material: Material,
    pub scale: f32,
    pub tint: Option<Color32>,
    pub tiles: Option<TileOptions>,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            material: Material::default(),
            scale: 1.0,
            tint: None,
            tiles: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct TileOptions {
    pub scale: u8,
    pub odd_tint: Color32,
    pub grout_width: f32,
    pub grout_tint: Color32,
}

#[derive(Clone)]
pub struct HomeRender {
    pub hash: u64,
    pub texture: ImageBuffer<Rgba<u8>, Vec<u8>>,
    pub center: Vec2,
    pub size: Vec2,
    pub vertices: HashMap<uuid::Uuid, Vec<Vec2>>,
    pub walls: HashMap<uuid::Uuid, Vec<Wall>>,
}

#[derive(
    Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, VariantArray, Default, Hash,
)]
pub enum Action {
    #[default]
    Add,
    Subtract,
}

#[derive(Serialize, Deserialize, Clone, Hash)]
pub struct Wall {
    pub points: Vec<Vec2>,
    pub wall_type: WallType,
}

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Default)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Home {
    pub fn load_file() -> Self {
        // Load from file or use default
        File::open(LAYOUT_PATH).map_or_else(
            |_| Self::default(),
            |mut file| {
                let mut contents = String::new();
                file.read_to_string(&mut contents).map_or_else(
                    |_| Self::default(),
                    |_| {
                        serde_json::from_str::<Self>(&contents).unwrap_or_else(|_| Self::template())
                    },
                )
            },
        )
    }

    pub fn save_file(&self) -> Result<()> {
        let mut file = File::create(LAYOUT_PATH)?;
        let contents = serde_json::to_string_pretty(self)?;
        file.write_all(contents.as_bytes())?;
        Ok(())
    }
}
