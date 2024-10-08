use crate::common::{
    color::Color,
    furniture::Furniture,
    shape::{Line, ShadowsData},
    utils::Material,
};
use ahash::AHashMap;
use geo_types::MultiPolygon;
use glam::DVec2 as Vec2;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::hash::Hash;
use strum_macros::{Display, EnumIter};
use uuid::Uuid;

pub const LAYOUT_VERSION: &str = "0.5";

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

            pub zones: Vec<pub struct Zone {
                pub id: Uuid,
                pub name: String,
                pub shape: Shape,
                pub pos: Vec2,
                pub size: Vec2,
                pub rotation: i32,
            }>,


            pub walls: Walls,
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
                pub flipped: bool,

                #[serde(skip)]
                pub open_amount: f64,
            }>,

            pub lights: Vec<pub struct Light {
                pub id: Uuid,
                pub name: String,
                pub entity_id: String,
                pub light_type: pub enum LightType {
                    Dimmable,
                    Binary,
                },
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
                pub lerped_state: f64,
                #[serde(skip)]
                pub light_data: Option<LightsData>,
                #[serde(skip)]
                pub last_manual: f64,
            }>,

            pub outline: Option<pub struct Outline {
                pub thickness: f64,
                pub color: Color,
            }>,

            pub furniture: Vec<Furniture>,

            pub sensors: Vec<pub struct Sensor {
                pub id: Uuid,
                pub entity_id: String,
                pub display_name: String,
                pub unit: String,
            }>,
            pub sensors_offset: Vec2,

            #[serde(skip)]
            pub rendered_data: Option<RoomRender>,
            #[serde(skip)]
            pub hass_data: AHashMap<String, String>,
        }>,

        #[serde(skip)]
        pub rendered_data: Option<HomeRender>,
        #[serde(skip)]
        pub light_data: Option<LightData>,
    }
}

bitflags::bitflags! {
    #[derive(Serialize, Deserialize, Clone, Copy, Hash, PartialEq, Eq)]
    pub struct Walls: u8 {
        const LEFT   = 0b0001;
        const TOP    = 0b0010;
        const RIGHT  = 0b0100;
        const BOTTOM = 0b1000;
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

pub type Vec4 = (f64, f64, f64, f64);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum DataPoint {
    String(String),
    Float(f64),
    Int(u8),
    Vec2(Vec2),
    Vec4(Vec4),
}

#[derive(Clone)]
pub struct LightData {
    pub hash: u64,
    pub image: Vec<u8>,
    pub image_center: Vec2,
    pub image_size: Vec2,
    pub image_width: u32,
    pub image_height: u32,
}

pub type LightsData = (u64, Vec<u16>);
