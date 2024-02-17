use super::{
    color::Color,
    furniture::Furniture,
    light_render::LightData,
    shape::ShadowsData,
    utils::{clone_as_none, Material},
};
use derivative::Derivative;
use geo_types::MultiPolygon;
use glam::DVec2 as Vec2;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::hash::Hash;
use strum_macros::{Display, EnumIter};
use uuid::Uuid;

pub const LAYOUT_VERSION: &str = "0.1";

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
    #[serde(skip)]
    #[derivative(Clone(clone_with = "clone_as_none"))]
    pub light_data: Option<LightData>,
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
    pub lights: Vec<Light>,
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
pub struct Light {
    pub id: Uuid,
    pub pos: Vec2,
    pub intensity: f64,
    pub radius: f64,
    #[serde(skip)]
    pub state: u8,
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

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct GlobalMaterial {
    pub name: String,
    pub material: Material,
    pub tint: Color,
    pub tiles: Option<TileOptions>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
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
    pub wall_polygons_full: Vec<MultiPolygon>,
    pub wall_shadows: (u64, ShadowsData),
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
