use super::shape::{Material, Shape, WallType};
use egui::Color32;
use image::{ImageBuffer, Rgba};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use strum_macros::{Display, VariantArray};
use uuid::Uuid;

const LAYOUT_VERSION: &str = "0.1";
pub const RESOLUTION_FACTOR: f32 = 80.0; // Pixels per meter

#[derive(Serialize, Deserialize, Clone)]
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
                    "Balcony",
                    Vec2::new(-0.2, -4.0),
                    Vec2::new(4.0, 1.8),
                    RenderOptions::new(Material::Marble, 1.5, Some("#979797ff"), None),
                    Walls::NONE,
                    vec![],
                ),
                Room::new(
                    "Lounge",
                    Vec2::new(-0.2, 0.55),
                    Vec2::new(4.0, 7.3),
                    RenderOptions::new(Material::Carpet, 1.0, None, None),
                    Walls::EXTERIOR.right(WallType::None),
                    vec![
                        Operation::new(
                            Action::Add,
                            Shape::Rectangle,
                            Vec2::new(-2.3, -0.35),
                            Vec2::new(1.0, 2.2),
                            0.0,
                        ),
                        Operation::new(
                            Action::Add,
                            Shape::Rectangle,
                            Vec2::new(2.2, 2.4),
                            Vec2::new(0.4, 2.5),
                            0.0,
                        ),
                    ],
                ),
                Room::new(
                    "Kitchen",
                    Vec2::new(-1.5, 2.95),
                    Vec2::new(3.0, 2.5),
                    RenderOptions::new(
                        Material::Marble,
                        2.0,
                        Some("#fff8e8ff"),
                        Some(TileOptions::new(7, "#ffffff00", 0.015, "#505050cc")),
                    ),
                    Walls::EXTERIOR.right(WallType::None).bottom(WallType::None),
                    vec![Operation::new(
                        Action::Subtract,
                        Shape::Rectangle,
                        Vec2::new(1.7, -0.55),
                        Vec2::new(1.0, 2.0),
                        20.0,
                    )],
                ),
                Room::new(
                    "Pantry",
                    Vec2::new(-1.6, 1.3),
                    Vec2::new(0.8, 0.8),
                    RenderOptions::new(
                        Material::Marble,
                        2.0,
                        Some("#fff8e8ff"),
                        Some(TileOptions::new(2, "#ffffff00", 0.015, "#505050cc")),
                    ),
                    Walls::INTERIOR,
                    vec![
                        Operation::new(
                            Action::Add,
                            Shape::Rectangle,
                            Vec2::new(0.4, 0.1),
                            Vec2::new(1.0, 0.4),
                            45.0,
                        ),
                        Operation::new(
                            Action::Subtract,
                            Shape::Rectangle,
                            Vec2::new(0.1, 0.9),
                            Vec2::new(1.0, 1.0),
                            0.0,
                        ),
                        Operation::new(
                            Action::Add,
                            Shape::Rectangle,
                            Vec2::new(0.4, -0.2),
                            Vec2::new(0.4, 0.4),
                            0.0,
                        ),
                    ],
                ),
                Room::new(
                    "Storage1",
                    Vec2::new(-2.5, 1.3),
                    Vec2::new(1.0, 0.8),
                    RenderOptions::new(Material::Carpet, 1.0, None, None),
                    Walls::INTERIOR.left(WallType::Exterior),
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
                        Some(TileOptions::new(4, "#ffffff00", 0.015, "#505050cc")),
                    ),
                    Walls::INTERIOR.bottom(WallType::Exterior),
                    vec![Operation {
                        id: Uuid::new_v4(),
                        action: Action::Subtract,
                        shape: Shape::Rectangle,
                        render_options: None,
                        pos: Vec2::new(0.5, 2.7 / 2.0 - 0.3),
                        size: Vec2::new(1.0, 1.2),
                        rotation: 0.0,
                    }],
                ),
            ],
            furniture: vec![],
            walls: vec![],
            rendered_data: None,
        }
    }

    pub fn empty() -> Self {
        Self {
            version: String::new(),
            rooms: vec![],
            furniture: vec![],
            walls: vec![],
            rendered_data: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Room {
    pub id: Uuid,
    pub name: String,
    pub render_options: RenderOptions,
    pub pos: Vec2,
    pub size: Vec2,
    pub operations: Vec<Operation>,
    pub walls: Walls,
    #[serde(skip)]
    pub rendered_data: Option<RoomRender>,
}

#[derive(Serialize, Deserialize, Clone, Hash)]
pub struct Walls {
    pub left: WallType,
    pub top: WallType,
    pub right: WallType,
    pub bottom: WallType,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Furniture {
    pub id: Uuid,
    pub pos: Vec2,
    pub size: Vec2,
    pub rotation: f32,
    pub children: Vec<Furniture>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Operation {
    pub id: Uuid,
    pub action: Action,
    pub shape: Shape,
    pub render_options: Option<RenderOptions>,
    pub pos: Vec2,
    pub size: Vec2,
    pub rotation: f32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct RenderOptions {
    pub material: Material,
    pub scale: f32,
    pub tint: Option<Color32>,
    pub tiles: Option<TileOptions>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct TileOptions {
    pub scale: u8,
    pub odd_tint: Color32,
    pub grout_width: f32,
    pub grout_tint: Color32,
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

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Default, Debug)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone)]
pub struct HomeRender {
    pub hash: u64,
    pub texture: ImageBuffer<Rgba<u8>, Vec<u8>>,
    pub center: Vec2,
    pub size: Vec2,
}

#[derive(Clone)]
pub struct RoomRender {
    pub hash: u64,
    pub texture: ImageBuffer<Rgba<u8>, Vec<u8>>,
    pub center: Vec2,
    pub size: Vec2,
    pub vertices: Vec<Vec2>,
    pub walls: Vec<Wall>,
}
