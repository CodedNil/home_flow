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
    pub openings: Vec<Opening>,
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
pub struct Opening {
    pub id: Uuid,
    pub pos: Vec2,
    pub opening_type: OpeningType,
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

#[derive(Serialize, Deserialize, Clone, Copy, Hash, PartialEq, Eq)]
pub enum OpeningType {
    Door,
    Window,
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
    AddWall,
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
                    vec2(0.525, 0.5),
                    vec2(5.65, 1.10),
                    Material::Carpet.into(),
                    Walls::NONE.top(WallType::Wall),
                    vec![Operation::new(
                        Action::Add,
                        Shape::Rectangle,
                        vec2(-0.875, 1.55),
                        vec2(1.1, 2.0),
                        0.0,
                    )
                    .set_material(Material::Wood)],
                ),
                Room::new(
                    "Lounge",
                    vec2(-2.75, -1.4),
                    vec2(6.1, 2.7),
                    Material::Carpet.into(),
                    Walls::WALL.top(WallType::None),
                    vec![],
                ),
                Room::new(
                    "Kitchen",
                    vec2(-4.05, 1.5),
                    vec2(3.5, 3.1),
                    RenderOptions::new(Material::Marble, Color32::from_rgb(255, 250, 230)),
                    Walls::WALL.right(WallType::None).bottom(WallType::None),
                    vec![],
                ),
                Room::new(
                    "Storage1",
                    vec2(-1.6, 2.55),
                    vec2(1.4, 1.0),
                    Material::Carpet.into(),
                    Walls::WALL,
                    vec![],
                ),
                Room::new(
                    "Storage2",
                    vec2(-1.6, 1.55),
                    vec2(1.4, 1.0),
                    Material::Carpet.into(),
                    Walls::WALL,
                    vec![],
                ),
                Room::new(
                    "Master Bedroom",
                    vec2(3.85, -0.95),
                    vec2(3.9, 3.6),
                    Material::Carpet.into(),
                    Walls::WALL,
                    vec![Operation::new(
                        Action::Subtract,
                        Shape::Rectangle,
                        vec2(-1.3, 1.4),
                        vec2(1.6, 1.0),
                        0.0,
                    )],
                ),
                Room::new(
                    "Ensuite",
                    vec2(1.1, -1.4),
                    vec2(1.6, 2.7),
                    Material::Granite.into(),
                    Walls::WALL,
                    vec![],
                ),
                Room::new(
                    "Bedroom Two",
                    vec2(4.2, 1.95),
                    vec2(3.2, 2.2),
                    Material::Carpet.into(),
                    Walls::WALL,
                    vec![Operation::new(
                        Action::Subtract,
                        Shape::Rectangle,
                        vec2(-1.35, -1.4),
                        vec2(1.0, 1.0),
                        0.0,
                    )],
                ),
                Room::new(
                    "Bathroom",
                    vec2(1.4, 2.05),
                    vec2(2.4, 2.0),
                    Material::Granite.into(),
                    Walls::WALL,
                    vec![],
                ),
            ],
            furniture: vec![],
            openings: vec![],
            rendered_data: None,
        }
    }
}
