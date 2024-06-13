use super::{
    color::Color,
    furniture::Furniture,
    light_render::{LightData, LightsData},
    shape::{Line, ShadowsData},
    utils::Material,
};
use geo_types::MultiPolygon;
use glam::DVec2 as Vec2;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::hash::Hash;
use strum_macros::{Display, EnumIter};
use uuid::Uuid;

pub const LAYOUT_VERSION: &str = "0.2";

nestify::nest! {
    #[derive(Serialize, Deserialize, Clone)]*
    pub struct Home {
        pub version: String,

        pub materials: Vec<pub struct GlobalMaterial {
            pub name: String,
            pub material: Material,
            pub tint: Color,
            #>[derive(Default)]
            pub tiles: Option<pub struct TileOptions {
                pub spacing: f64,
                pub grout_width: f64,
                pub grout_color: Color,
            }>,
        }>,

        pub rooms: Vec<pub struct Room {
            pub id: Uuid,
            pub name: String,
            pub material: String,
            pub pos: Vec2,
            pub size: Vec2,

            pub operations: Vec<pub struct Operation {
                pub id: Uuid,
                #>[derive(Copy, PartialEq, Eq, Display, EnumIter, Hash)]
                pub action: pub enum Action {
                    Add,
                    Subtract,
                    AddWall,
                    SubtractWall,
                },
                #>[derive(Copy, PartialEq, Eq, Display, EnumIter, Hash)]
                pub shape: pub enum Shape {
                    Rectangle,
                    Circle,
                    Triangle,
                },
                pub material: Option<String>,
                pub pos: Vec2,
                pub size: Vec2,
                pub rotation: i32,
            }>,

            #>[derive(Copy, Hash, PartialEq, Eq)]
            pub walls: pub struct Walls {
                pub left: bool,
                pub top: bool,
                pub right: bool,
                pub bottom: bool,
            },

            pub openings: Vec<pub struct Opening {
                pub id: Uuid,
                #>[derive(Copy, PartialEq, Eq, Display, EnumIter, Hash)]
                pub opening_type: pub enum OpeningType {
                    Door,
                    Window,
                },
                pub pos: Vec2,
                pub rotation: i32,
                pub width: f64,

                #[serde(skip)]
                pub open_amount: f64,
            }>,

            pub lights: Vec<pub struct Light {
                pub id: Uuid,
                pub name: String,
                pub pos: Vec2,
                pub multi: Option<pub struct MultiLight {
                    pub room_padding: Vec2,
                    pub rows: u8,
                    pub cols: u8,
                }>,
                pub intensity: f64,
                pub radius: f64,

                #[serde(skip)]
                pub state: u8,
                #[serde(skip)]
                pub light_data: Option<LightsData>,
            }>,

            pub outline: Option<pub struct Outline {
                pub thickness: f64,
                pub color: Color,
            }>,

            #[serde(skip)]
            pub rendered_data: Option<RoomRender>,
        }>,

        pub furniture: Vec<Furniture>,

        #[serde(skip)]
        pub rendered_data: Option<HomeRender>,
        #[serde(skip)]
        pub light_data: Option<LightData>,
    }
}

#[derive(Clone)]
pub struct HomeRender {
    pub hash: u64,
    pub wall_triangles: Vec<Triangles>,
    pub wall_lines: Vec<Line>,
    pub wall_shadows: (u64, ShadowsData),
}

#[derive(Clone)]
pub struct RoomRender {
    pub hash: u64,
    pub polygons: MultiPolygon,
    pub material_triangles: IndexMap<String, Vec<Triangles>>,
    pub wall_polygons: MultiPolygon,
}

#[derive(Clone)]
pub struct Triangles {
    pub indices: Vec<u32>,
    pub vertices: Vec<Vec2>,
}
