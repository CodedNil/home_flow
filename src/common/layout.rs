use super::shape::{Material, Shape, WallType};
use egui::Color32;
use geo_types::MultiPolygon;
use glam::{dvec2 as vec2, DVec2 as Vec2};
use image::{ImageBuffer, Rgba};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
};
use strum_macros::{Display, VariantArray};
use uuid::Uuid;

const LAYOUT_VERSION: &str = "0.1";
pub const RESOLUTION_FACTOR: f64 = 80.0; // Pixels per meter

#[derive(Serialize, Deserialize, Clone)]
pub struct Home {
    pub version: String,
    pub rooms: Vec<Room>,
    pub furniture: Vec<Furniture>,
    #[serde(skip)]
    pub rendered_data: Option<HomeRender>,
}

impl Hash for Home {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.version.hash(state);
        self.rooms.hash(state);
        self.furniture.hash(state);
    }
}

impl Home {
    pub fn template() -> Self {
        Self {
            version: LAYOUT_VERSION.to_string(),
            rooms: vec![
                Room::new(
                    "Hall",
                    vec2(2.9, -0.1),
                    vec2(2.2, 4.8),
                    RenderOptions::new(Material::Carpet, 1.0, None, None),
                    false,
                    Walls::NONE,
                    vec![],
                ),
                Room::new(
                    "Balcony",
                    vec2(-0.2, -4.0),
                    vec2(4.0, 1.8),
                    RenderOptions::new(Material::Limestone, 1.5, None, None),
                    false,
                    Walls::NONE,
                    vec![],
                ),
                Room::new(
                    "Lounge",
                    vec2(-0.2, 0.55),
                    vec2(4.0, 7.3),
                    RenderOptions::new(Material::Carpet, 1.0, None, None),
                    true,
                    Walls::WALL.right(WallType::None),
                    vec![
                        Operation::new(
                            Action::Add,
                            Shape::Rectangle,
                            vec2(-2.3, -0.35),
                            vec2(1.0, 2.2),
                            0.0,
                        ),
                        Operation::new(
                            Action::Add,
                            Shape::Rectangle,
                            vec2(2.2, 2.4),
                            vec2(0.5, 2.5),
                            0.0,
                        ),
                    ],
                ),
                Room::new(
                    "Kitchen",
                    vec2(-1.5, 2.95),
                    vec2(3.0, 2.5),
                    RenderOptions::new(
                        Material::Marble,
                        2.0,
                        Some("#fff8e8ff"),
                        Some(TileOptions::new(7, "#ffffff00", 0.015, "#505050cc")),
                    ),
                    true,
                    Walls::WALL.right(WallType::None).bottom(WallType::None),
                    vec![Operation::new(
                        Action::Subtract,
                        Shape::Rectangle,
                        vec2(1.7, -0.55),
                        vec2(1.0, 2.0),
                        20.0,
                    )],
                ),
                Room::new(
                    "Pantry",
                    vec2(-1.3, 1.3),
                    vec2(1.4, 0.8),
                    RenderOptions::new(
                        Material::Marble,
                        2.0,
                        Some("#fff8e8ff"),
                        Some(TileOptions::new(2, "#ffffff00", 0.015, "#505050cc")),
                    ),
                    true,
                    Walls::WALL,
                    vec![
                        Operation::new(
                            Action::Subtract,
                            Shape::Rectangle,
                            vec2(0.7, 0.2),
                            vec2(1.5, 0.6),
                            45.0,
                        ),
                        Operation::new(
                            Action::Subtract,
                            Shape::Rectangle,
                            vec2(0.6, -0.5),
                            vec2(0.6, 0.6),
                            45.0,
                        ),
                    ],
                ),
                Room::new(
                    "Storage1",
                    vec2(-2.5, 1.3),
                    vec2(1.0, 0.8),
                    RenderOptions::new(Material::Carpet, 1.0, None, None),
                    true,
                    Walls::WALL,
                    vec![],
                ),
                Room::new(
                    "Office",
                    vec2(4.2, 2.7),
                    vec2(4.0, 3.0),
                    RenderOptions::new(Material::Carpet, 1.0, None, None),
                    true,
                    Walls::WALL,
                    vec![Operation::new(
                        Action::Subtract,
                        Shape::Rectangle,
                        vec2(-1.8, -1.3),
                        vec2(1.7, 1.0),
                        45.0,
                    )],
                ),
                Room::new(
                    "Bedroom",
                    vec2(3.8, -4.5),
                    vec2(4.0, 4.0),
                    RenderOptions::new(Material::Carpet, 1.0, None, None),
                    true,
                    Walls::WALL,
                    vec![],
                ),
                Room::new(
                    "Storage2",
                    vec2(2.4, -0.1),
                    vec2(1.2, 1.6),
                    RenderOptions::new(Material::Carpet, 1.0, None, None),
                    true,
                    Walls::WALL,
                    vec![Operation::new(
                        Action::Subtract,
                        Shape::Rectangle,
                        vec2(0.4, 0.5),
                        vec2(1.8, 0.8),
                        45.0,
                    )],
                ),
                Room::new(
                    "Storage3",
                    vec2(2.4, -1.7),
                    vec2(1.2, 1.6),
                    RenderOptions::new(Material::Carpet, 1.0, None, None),
                    true,
                    Walls::WALL,
                    vec![],
                ),
                Room::new(
                    "Closet",
                    vec2(4.9, -1.9),
                    vec2(1.8, 1.2),
                    RenderOptions::new(Material::Carpet, 1.0, None, None),
                    true,
                    Walls::WALL,
                    vec![],
                ),
                Room::new(
                    "Bathroom",
                    vec2(4.9, -0.05),
                    vec2(1.8, 2.5),
                    RenderOptions::new(
                        Material::Granite,
                        2.0,
                        Some("#fff8e8"),
                        Some(TileOptions::new(4, "#ffffff00", 0.015, "#505050cc")),
                    ),
                    true,
                    Walls::WALL,
                    vec![],
                ),
                Room::new(
                    "Storage4",
                    vec2(3.9, 0.8),
                    vec2(0.8, 0.8),
                    RenderOptions::new(Material::Carpet, 1.0, None, None),
                    true,
                    Walls::WALL,
                    vec![],
                ),
            ],
            furniture: vec![],
            rendered_data: None,
        }
    }

    pub fn empty() -> Self {
        Self {
            version: String::new(),
            rooms: vec![],
            furniture: vec![],
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
    pub has_walls: bool,
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
    pub rotation: f64,
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
    pub rotation: f64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct RenderOptions {
    pub material: Material,
    pub scale: f64,
    pub tint: Option<Color32>,
    pub tiles: Option<TileOptions>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct TileOptions {
    pub scale: u8,
    pub odd_tint: Color32,
    pub grout_width: f64,
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

#[derive(Clone)]
pub struct Wall {
    pub points: Vec<Vec2>,
    pub closed: bool,
}

#[derive(Clone)]
pub struct HomeRender {
    pub hash: u64,
}

#[derive(Clone)]
pub struct RoomRender {
    pub hash: u64,
    pub texture: ImageBuffer<Rgba<u8>, Vec<u8>>,
    pub center: Vec2,
    pub size: Vec2,
    pub polygons: MultiPolygon,
    pub material_polygons: HashMap<Material, MultiPolygon>,
    pub wall_polygons: MultiPolygon,
}
