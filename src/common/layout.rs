use super::{
    color::Color,
    furniture::{
        BathroomType, ChairType, Furniture, FurnitureType, KitchenType, StorageType, TableType,
    },
    shape::ShadowTriangles,
    utils::{clone_as_none, Material},
};
use derivative::Derivative;
use geo_types::MultiPolygon;
use glam::{dvec2 as vec2, DVec2 as Vec2};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::hash::Hash;
use strum_macros::{Display, EnumIter};
use uuid::Uuid;

const LAYOUT_VERSION: &str = "0.1";

#[derive(Serialize, Deserialize, Default, Derivative)]
#[derivative(Clone)]
pub struct Home {
    pub version: String,
    pub materials: Vec<GlobalMaterial>,
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
    pub material: String,
    pub pos: Vec2,
    pub size: Vec2,
    pub operations: Vec<Operation>,
    pub walls: Walls,
    pub openings: Vec<Opening>,
    pub outline: Option<Outline>,
    #[serde(skip)]
    #[derivative(Clone(clone_with = "clone_as_none"))]
    pub rendered_data: Option<RoomRender>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Walls {
    pub left: bool,
    pub top: bool,
    pub right: bool,
    pub bottom: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Opening {
    pub id: Uuid,
    pub opening_type: OpeningType,
    pub pos: Vec2,
    pub rotation: i32,
    pub width: f64,
    #[serde(skip)]
    pub open_amount: f64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Operation {
    pub id: Uuid,
    pub action: Action,
    pub shape: Shape,
    pub material: Option<String>,
    pub pos: Vec2,
    pub size: Vec2,
    pub rotation: i32,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, EnumIter, Default, Hash)]
pub enum OpeningType {
    #[default]
    Door,
    Window,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Outline {
    pub thickness: f64,
    pub color: Color,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, EnumIter, Default, Hash)]
pub enum Action {
    #[default]
    Add,
    Subtract,
    AddWall,
    SubtractWall,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, EnumIter, Default, Hash)]
pub enum Shape {
    #[default]
    Rectangle,
    Circle,
    Triangle,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct GlobalMaterial {
    pub name: String,
    pub material: Material,
    pub tint: Color,
    pub tiles: Option<TileOptions>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TileOptions {
    pub spacing: f64,
    pub grout_width: f64,
    pub grout_color: Color,
}

pub struct HomeRender {
    pub hash: u64,
    pub walls_hash: u64,
    pub wall_triangles: Vec<Triangles>,
    pub wall_polygons: Vec<MultiPolygon>,
    pub wall_shadows: (u64, Vec<ShadowTriangles>),
}

pub struct RoomRender {
    pub hash: u64,
    pub polygons: MultiPolygon,
    pub material_polygons: IndexMap<String, MultiPolygon>,
    pub material_triangles: IndexMap<String, Vec<Triangles>>,
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
            materials: vec![
                GlobalMaterial::new("Carpet", Material::Carpet, Color::from_rgb(240, 225, 192)),
                GlobalMaterial::new("Wood", Material::Wood, Color::from_rgb(190, 120, 80)),
                GlobalMaterial::new("WoodDark", Material::Wood, Color::from_rgb(60, 60, 60)),
                GlobalMaterial::new("Marble", Material::Marble, Color::from_rgb(255, 255, 255)),
                GlobalMaterial::new("Granite", Material::Granite, Color::from_rgb(50, 50, 50)),
                GlobalMaterial::new("Ceramic", Material::Empty, Color::from_rgb(230, 220, 200)),
                GlobalMaterial::new("MetalDark", Material::Empty, Color::from_rgb(80, 80, 80)),
                GlobalMaterial::new(
                    "MarbleTiles",
                    Material::Marble,
                    Color::from_rgb(255, 250, 230),
                )
                .tiles(0.4, 0.04, Color::from_rgba(80, 80, 80, 100)),
                GlobalMaterial::new(
                    "GraniteTiles",
                    Material::Granite,
                    Color::from_rgb(50, 50, 50),
                )
                .tiles(0.4, 0.04, Color::from_rgba(80, 80, 80, 200)),
            ],
            rooms: vec![
                Room::new("Hall", vec2(0.5, 0.5), vec2(6.2, 1.10), "Carpet")
                    .no_wall_left()
                    .no_wall_right()
                    .no_wall_bottom()
                    .add_material(vec2(-0.85, 1.55), vec2(1.1, 2.0), "Wood")
                    .door(vec2(-0.85, 2.55), 0),
                Room::new("Lounge", vec2(-2.75, -1.4), vec2(6.1, 2.7), "Carpet")
                    .no_wall_top()
                    .window_width(vec2(-1.0, -1.35), 0, 1.6)
                    .window(vec2(2.1, -1.35), 0),
                Room::new("Kitchen", vec2(-4.2, 1.5), vec2(3.2, 3.1), "MarbleTiles")
                    .no_wall_right()
                    .no_wall_bottom()
                    .add(vec2(1.65, 0.55), vec2(0.3, 2.0))
                    .window(vec2(0.2, 1.55), 0)
                    .outline(Outline::new(0.05, Color::from_rgb(200, 170, 150))),
                Room::new("Storage1", vec2(-1.65, 2.5), vec2(1.5, 1.1), "Carpet")
                    .door(vec2(0.75, 0.0), -90),
                Room::new("Storage2", vec2(-1.65, 1.4), vec2(1.5, 1.1), "Carpet")
                    .door(vec2(0.75, 0.0), -90),
                Room::new("Bedroom", vec2(3.85, -0.95), vec2(3.9, 3.6), "Carpet")
                    .subtract(vec2(-1.1, 1.4), vec2(1.7, 1.0))
                    .door(vec2(-0.25, 1.35), 90)
                    .window(vec2(0.0, -1.8), 0),
                Room::new("Ensuite", vec2(1.1, -1.4), vec2(1.6, 2.7), "GraniteTiles")
                    .door(vec2(0.8, -0.85), -90)
                    .window(vec2(0.0, -1.35), 0),
                Room::new("Boiler Room", vec2(1.5, -0.55), vec2(0.8, 1.0), "Carpet").door_width(
                    vec2(0.0, 0.5),
                    180,
                    0.6,
                ),
                Room::new("Bedroom Two", vec2(4.2, 1.95), vec2(3.2, 2.2), "Carpet")
                    .subtract(vec2(-1.1, -1.4), vec2(1.0, 1.0))
                    .door(vec2(-1.1, -0.9), 180)
                    .window(vec2(1.6, 0.0), -90),
                Room::new("Bathroom", vec2(1.4, 2.05), vec2(2.4, 2.0), "GraniteTiles")
                    .door(vec2(0.7, -1.0), 180),
            ],
            furniture: vec![
                // Kitchen counters
                Furniture::new(
                    FurnitureType::Table(TableType::Empty),
                    vec2(-5.475, 2.725),
                    vec2(0.55, 0.55),
                    0,
                )
                .material("Granite"),
                Furniture::new(
                    FurnitureType::Storage(StorageType::Drawer),
                    vec2(-5.475, 1.35),
                    vec2(2.2, 0.55),
                    -90,
                )
                .material("Granite"),
                Furniture::new(
                    FurnitureType::Storage(StorageType::Drawer),
                    vec2(-4.1, 2.725),
                    vec2(2.2, 0.55),
                    0,
                )
                .material("Granite"),
                Furniture::new(
                    FurnitureType::Table(TableType::Empty),
                    vec2(-2.725, 2.725),
                    vec2(0.55, 0.55),
                    0,
                )
                .material("Granite"),
                Furniture::new(
                    FurnitureType::Storage(StorageType::Drawer),
                    vec2(-2.725, 1.9),
                    vec2(1.1, 0.55),
                    90,
                )
                .material("Granite"),
                // Cupboards
                Furniture::new(
                    FurnitureType::Storage(StorageType::CupboardHigh),
                    vec2(-5.625, 1.35),
                    vec2(2.2, 0.25),
                    -90,
                ),
                Furniture::new(
                    FurnitureType::Storage(StorageType::CupboardHigh),
                    vec2(-4.925, 2.875),
                    vec2(0.55, 0.25),
                    0,
                ),
                Furniture::new(
                    FurnitureType::Storage(StorageType::CupboardHigh),
                    vec2(-5.425, 2.675),
                    vec2(0.55, 0.25),
                    -45,
                ),
                Furniture::new(
                    FurnitureType::Storage(StorageType::CupboardHigh),
                    vec2(-2.575, 1.9),
                    vec2(1.1, 0.25),
                    90,
                ),
                Furniture::new(
                    FurnitureType::Storage(StorageType::CupboardHigh),
                    vec2(-2.775, 2.675),
                    vec2(0.55, 0.25),
                    45,
                ),
                // Fridge
                Furniture::new(
                    FurnitureType::Storage(StorageType::CupboardHigh),
                    vec2(-2.725, 1.075),
                    vec2(0.55, 0.55),
                    90,
                ),
                // Kitchen
                Furniture::new(
                    FurnitureType::Table(TableType::Empty),
                    vec2(-4.1, 1.3),
                    vec2(0.8, 0.8),
                    0,
                ),
                Furniture::new(
                    FurnitureType::Kitchen(KitchenType::Microwave),
                    vec2(-2.8, 2.65),
                    vec2(0.5, 0.4),
                    50,
                ),
                Furniture::new(
                    FurnitureType::Kitchen(KitchenType::Hob),
                    vec2(-5.475, 2.725 - 0.55 * 2.0),
                    vec2(0.55, 0.55),
                    -90,
                ),
                Furniture::new(
                    FurnitureType::Kitchen(KitchenType::Sink),
                    vec2(-4.05, 2.7),
                    vec2(0.65, 0.5),
                    0,
                ),
                // Bedroom
                Furniture::new(
                    FurnitureType::Bed(Color::from_rgb(110, 120, 130)),
                    vec2(4.65, -1.4),
                    vec2(1.4, 2.1),
                    90,
                ),
                Furniture::new(
                    FurnitureType::Storage(StorageType::Drawer),
                    vec2(5.475, -2.4),
                    vec2(0.4, 0.55),
                    90,
                )
                .materials("WoodDark"),
                Furniture::new(
                    FurnitureType::Storage(StorageType::Cupboard),
                    vec2(5.075, 0.5),
                    vec2(1.35, 0.6),
                    0,
                ),
                Furniture::new(
                    FurnitureType::Storage(StorageType::Drawer),
                    vec2(2.35, -0.9),
                    vec2(1.6, 0.8),
                    -90,
                )
                .materials("WoodDark"),
                Furniture::new(
                    FurnitureType::Radiator,
                    vec2(3.85, -2.65),
                    vec2(1.4, 0.1),
                    0,
                ),
                // Ensuite
                Furniture::new(
                    FurnitureType::Bathroom(BathroomType::Shower),
                    vec2(0.7, -0.75),
                    vec2(0.7, 1.3),
                    0,
                ),
                Furniture::new(
                    FurnitureType::Bathroom(BathroomType::Toilet),
                    vec2(0.675, -2.3),
                    vec2(0.55, 0.65),
                    -90,
                ),
                Furniture::new(
                    FurnitureType::Bathroom(BathroomType::Sink),
                    vec2(1.45, -1.325),
                    vec2(0.45, 0.45),
                    0,
                ),
                // Office
                Furniture::new(
                    FurnitureType::Storage(StorageType::Cupboard),
                    vec2(4.2, 2.7),
                    vec2(3.1, 0.6),
                    0,
                ),
                Furniture::new(
                    FurnitureType::Table(TableType::Desk),
                    vec2(4.7, 1.3),
                    vec2(1.6, 0.8),
                    0,
                )
                .material("WoodDark"),
                Furniture::new(
                    FurnitureType::Radiator,
                    vec2(5.7, 1.95),
                    vec2(0.75, 0.1),
                    90,
                ),
                // Bathroom
                Furniture::new(
                    FurnitureType::Bathroom(BathroomType::Shower),
                    vec2(2.25, 2.575),
                    vec2(0.6, 0.85),
                    0,
                ),
                Furniture::new(
                    FurnitureType::Bathroom(BathroomType::Bath),
                    vec2(0.65, 2.05),
                    vec2(0.8, 1.9),
                    0,
                ),
                Furniture::new(
                    FurnitureType::Bathroom(BathroomType::Toilet),
                    vec2(1.475, 2.675),
                    vec2(0.55, 0.65),
                    0,
                ),
                Furniture::new(
                    FurnitureType::Bathroom(BathroomType::Sink),
                    vec2(1.4, 1.325),
                    vec2(0.45, 0.45),
                    -180,
                ),
                // Living Room
                Furniture::new(
                    FurnitureType::Table(TableType::Dining),
                    vec2(-1.45, -1.15),
                    vec2(1.8, 0.8),
                    0,
                ),
                Furniture::new(
                    FurnitureType::Rug(Color::from_rgba(60, 135, 136, 255)),
                    vec2(-4.55, -1.5),
                    vec2(1.6, 1.6),
                    0,
                ),
                Furniture::new(
                    FurnitureType::Chair(ChairType::Sofa(Color::from_rgb(200, 200, 200))),
                    vec2(-4.15, -1.1),
                    vec2(1.2, 0.8),
                    45,
                ),
                Furniture::new(
                    FurnitureType::Storage(StorageType::Drawer),
                    vec2(-5.3, -2.25),
                    vec2(0.8, 0.4),
                    225,
                ),
                Furniture::new(
                    FurnitureType::Display,
                    vec2(-5.3, -2.25),
                    vec2(1.0, 0.1),
                    45,
                ),
                Furniture::new(
                    FurnitureType::Storage(StorageType::Drawer),
                    vec2(-2.0, -2.55),
                    vec2(1.1, 0.3),
                    180,
                ),
                Furniture::new(
                    FurnitureType::Radiator,
                    vec2(-3.75, -2.65),
                    vec2(1.4, 0.1),
                    0,
                ),
                Furniture::new(FurnitureType::Radiator, vec2(0.95, 0.95), vec2(1.4, 0.1), 0),
                // Misc
                Furniture::new(FurnitureType::Boiler, vec2(1.5, -0.65), vec2(0.6, 0.6), 0),
            ],
            rendered_data: None,
        }
    }
}
