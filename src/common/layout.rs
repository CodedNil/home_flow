use super::{
    shape::{Material, Shape, WallType},
    utils::clone_as_none,
};
use derivative::Derivative;
use egui::Color32;
use geo_types::MultiPolygon;
use glam::{dvec2 as vec2, DVec2 as Vec2};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, hash::Hash};
use strum_macros::{Display, VariantArray};
use uuid::Uuid;

const LAYOUT_VERSION: &str = "0.1";

#[derive(Serialize, Deserialize, Default, Derivative)]
#[derivative(Clone)]
pub struct Home {
    pub version: String,
    pub rooms: Vec<Room>,
    pub furniture: Vec<Furniture>,
    #[serde(skip)]
    #[derivative(Clone(clone_with = "clone_as_none"))]
    pub rendered_data: Option<HomeRender>,
}

#[derive(Serialize, Deserialize, Derivative)]
#[derivative(Clone)]
pub struct Room {
    pub id: Uuid,
    pub name: String,
    pub render_options: RenderOptions,
    pub pos: Vec2,
    pub size: Vec2,
    pub operations: Vec<Operation>,
    pub walls: Walls,
    #[serde(skip)]
    #[derivative(Clone(clone_with = "clone_as_none"))]
    pub rendered_data: Option<RoomRender>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Hash, PartialEq, Eq)]
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

#[derive(Serialize, Deserialize, Clone, Default, Hash)]
pub struct RenderOptions {
    pub material: Material,
    pub tint: Option<Color32>,
}

#[derive(
    Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, VariantArray, Default, Hash,
)]
pub enum Action {
    #[default]
    Add,
    Subtract,
    SubtractWall,
}

pub struct HomeRender {
    pub hash: u64,
    pub wall_polygons: MultiPolygon,
    pub wall_triangles: Vec<Triangles>,
}

pub struct RoomRender {
    pub hash: u64,
    pub polygons: MultiPolygon,
    pub material_polygons: HashMap<Material, MultiPolygon>,
    pub material_triangles: HashMap<Material, Vec<Triangles>>,
    pub wall_polygons: MultiPolygon,
}

pub struct Triangles {
    pub indices: Vec<u32>,
    pub vertices: Vec<Vec2>,
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
                    RenderOptions::new(Material::Wood, None),
                    Walls::NONE,
                    vec![],
                ),
                Room::new(
                    "Balcony",
                    vec2(-0.2, -4.0),
                    vec2(4.0, 1.8),
                    RenderOptions::new(Material::Limestone, None),
                    Walls::NONE,
                    vec![],
                ),
                Room::new(
                    "Lounge",
                    vec2(-0.2, 0.55),
                    vec2(4.0, 7.3),
                    RenderOptions::new(Material::Wood, None),
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
                    RenderOptions::new(Material::Marble, Some("#fff8e8ff")),
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
                    RenderOptions::new(Material::Marble, Some("#fff8e8ff")),
                    Walls::WALL,
                    vec![
                        Operation::new(
                            Action::Subtract,
                            Shape::Triangle,
                            vec2(0.45, -0.3),
                            vec2(0.5, 0.5),
                            180.0,
                        ),
                        Operation::new(
                            Action::Subtract,
                            Shape::Triangle,
                            vec2(0.4, 0.1),
                            vec2(1.0, 1.0),
                            90.0,
                        ),
                    ],
                ),
                Room::new(
                    "Storage1",
                    vec2(-2.5, 1.3),
                    vec2(1.0, 0.8),
                    RenderOptions::new(Material::Carpet, None),
                    Walls::WALL,
                    vec![],
                ),
                Room::new(
                    "Office",
                    vec2(4.2, 2.7),
                    vec2(4.0, 3.0),
                    RenderOptions::new(Material::Carpet, None),
                    Walls::WALL,
                    vec![Operation::new(
                        Action::Subtract,
                        Shape::Triangle,
                        vec2(-1.5, -1.0),
                        vec2(1.2, 1.2),
                        -90.0,
                    )],
                ),
                Room::new(
                    "Bedroom",
                    vec2(3.8, -4.5),
                    vec2(4.0, 4.0),
                    RenderOptions::new(Material::Carpet, None),
                    Walls::WALL,
                    vec![],
                ),
                Room::new(
                    "Storage2",
                    vec2(2.4, -0.1),
                    vec2(1.2, 1.6),
                    RenderOptions::new(Material::Carpet, None),
                    Walls::WALL,
                    vec![Operation::new(
                        Action::Subtract,
                        Shape::Triangle,
                        vec2(0.2, 0.4),
                        vec2(1.2, 1.2),
                        90.0,
                    )],
                ),
                Room::new(
                    "Storage3",
                    vec2(2.4, -1.7),
                    vec2(1.2, 1.6),
                    RenderOptions::new(Material::Carpet, None),
                    Walls::WALL,
                    vec![],
                ),
                Room::new(
                    "Closet",
                    vec2(4.9, -1.9),
                    vec2(1.8, 1.2),
                    RenderOptions::new(Material::Carpet, None),
                    Walls::WALL,
                    vec![],
                ),
                Room::new(
                    "Bathroom",
                    vec2(4.9, -0.05),
                    vec2(1.8, 2.5),
                    RenderOptions::new(Material::Granite, Some("#fff8e8")),
                    Walls::WALL,
                    vec![],
                ),
                Room::new(
                    "Storage4",
                    vec2(3.9, 0.8),
                    vec2(0.8, 0.8),
                    RenderOptions::new(Material::Carpet, None),
                    Walls::WALL,
                    vec![],
                ),
            ],
            furniture: vec![],
            rendered_data: None,
        }
    }
}
