use crate::common::{
    color::Color,
    furniture::{self, Furniture, FurnitureType},
    layout::{
        Action, GlobalMaterial, Home, Light, LightType, MultiLight, Opening, OpeningType,
        Operation, Outline, Room, Sensor, Shape, TileOptions, Walls, Zone,
    },
};
use ahash::AHashMap;
use glam::{dvec2 as vec2, DVec2 as Vec2};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use strum_macros::{Display, EnumIter};
use uuid::Uuid;

pub fn hash_vec2<H: Hasher>(vec: Vec2, state: &mut H) {
    vec.x.to_bits().hash(state);
    vec.y.to_bits().hash(state);
}

pub trait RoundFactor {
    fn round_factor(&self, factor: f64) -> f64;
}

impl RoundFactor for f64 {
    fn round_factor(&self, factor: f64) -> f64 {
        (self * factor).round() / factor
    }
}

pub trait Lerp {
    fn lerp(self, other: Self, t: f64) -> Self;
}

impl Lerp for u8 {
    fn lerp(self, other: Self, t: f64) -> Self {
        (f64::from(self) + (f64::from(other) - f64::from(self)) * t) as Self
    }
}

pub fn rotate_point(point: Vec2, angle: f64) -> Vec2 {
    let cos_theta = angle.to_radians().cos();
    let sin_theta = angle.to_radians().sin();

    vec2(
        cos_theta * point.x - sin_theta * point.y,
        sin_theta * point.x + cos_theta * point.y,
    )
}

pub fn rotate_point_i32(point: Vec2, angle: i32) -> Vec2 {
    rotate_point(point, f64::from(angle))
}

pub fn rotate_point_pivot(point: Vec2, pivot: Vec2, angle: f64) -> Vec2 {
    let cos_theta = angle.to_radians().cos();
    let sin_theta = angle.to_radians().sin();

    vec2(
        cos_theta * (point.x - pivot.x) - sin_theta * (point.y - pivot.y) + pivot.x,
        sin_theta * (point.x - pivot.x) + cos_theta * (point.y - pivot.y) + pivot.y,
    )
}

pub fn rotate_point_pivot_i32(point: Vec2, pivot: Vec2, angle: i32) -> Vec2 {
    rotate_point_pivot(point, pivot, f64::from(angle))
}

impl Home {
    pub const fn empty() -> Self {
        Self {
            version: String::new(),
            materials: Vec::new(),
            rooms: Vec::new(),
            rendered_data: None,
            light_data: None,
        }
    }
}
impl Hash for Home {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.version.hash(state);
        self.materials.hash(state);
        self.rooms.hash(state);
    }
}

impl Room {
    pub fn new(name: &str, pos: Vec2, size: Vec2, material: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.to_owned(),
            material: material.to_owned(),
            pos,
            size,
            walls: Walls::all(),
            operations: Vec::new(),
            zones: Vec::new(),
            openings: Vec::new(),
            lights: Vec::new(),
            furniture: Vec::new(),
            sensors: Vec::new(),
            sensors_offset: Vec2::ZERO,
            outline: None,
            rendered_data: None,
            hass_data: AHashMap::new(),
        }
    }

    pub fn default() -> Self {
        Self::new("New Room", Vec2::ZERO, vec2(1.0, 1.0), "")
    }

    pub const fn outline(mut self, outline: Outline) -> Self {
        self.outline = Some(outline);
        self
    }

    pub const fn set_walls(mut self, walls: Walls) -> Self {
        self.walls = walls;
        self
    }

    pub fn opening(mut self, opening: Opening) -> Self {
        self.openings.push(opening);
        self
    }

    pub fn window(self, pos: Vec2, rotation: i32) -> Self {
        self.opening(Opening::new(OpeningType::Window, pos, rotation))
    }

    pub fn window_width(self, pos: Vec2, rotation: i32, width: f64) -> Self {
        self.opening(Opening::new(OpeningType::Window, pos, rotation).width(width))
    }

    pub fn door(self, pos: Vec2, rotation: i32) -> Self {
        self.opening(Opening::new(OpeningType::Door, pos, rotation))
    }

    pub fn door_width(self, pos: Vec2, rotation: i32, width: f64) -> Self {
        self.opening(Opening::new(OpeningType::Door, pos, rotation).width(width))
    }

    pub fn door_flipped(self, pos: Vec2, rotation: i32) -> Self {
        self.opening(Opening::new(OpeningType::Door, pos, rotation).flip())
    }

    pub fn light(mut self, name: &str, x: f64, y: f64) -> Self {
        self.lights.push(Light::new(name, vec2(x, y)));
        self
    }

    pub fn light_full(
        mut self,
        name: &str,
        x: f64,
        y: f64,
        light_type: LightType,
        intensity: f64,
        radius: f64,
    ) -> Self {
        self.lights.push({
            Light {
                id: Uuid::new_v4(),
                name: name.to_owned(),
                entity_id: name.to_lowercase().replace(' ', "_"),
                light_type,
                pos: vec2(x, y),
                multi: None,
                intensity,
                radius,
                state: 0,
                lerped_state: 0.0,
                light_data: None,
                last_manual: 0.0,
            }
        });
        self
    }

    pub fn lights_grid(mut self, name: &str, cols: u8, rows: u8, padding: Vec2, off: Vec2) -> Self {
        self.lights
            .push(Light::multi(name, off, padding, rows, cols));
        self
    }

    pub fn light_center(self, name: &str) -> Self {
        self.light(name, 0.0, 0.0)
    }

    pub fn furniture(mut self, furniture: Furniture) -> Self {
        self.furniture.push(furniture);
        self
    }

    pub fn furniture_bulk(
        mut self,
        name: &str,
        furniture_type: FurnitureType,
        render_order: furniture::RenderOrder,
        locations: Vec<(Vec2, Vec2, i32)>,
    ) -> Self {
        for (pos, size, rotation) in locations {
            self.furniture.push(
                Furniture::new(name, furniture_type, pos, size, rotation)
                    .render_order(render_order),
            );
        }
        self
    }

    pub fn furniture_bulk_material(
        mut self,
        name: &str,
        furniture_type: FurnitureType,
        render_order: furniture::RenderOrder,
        material: &str,
        locations: Vec<(Vec2, Vec2, i32)>,
    ) -> Self {
        for (pos, size, rotation) in locations {
            self.furniture.push(
                Furniture::new_materials(name, furniture_type, pos, size, rotation, material)
                    .render_order(render_order),
            );
        }
        self
    }

    pub fn add_sensors(mut self, sensors: &[Sensor]) -> Self {
        self.sensors = sensors.to_vec();
        self
    }

    pub const fn sensor_offset(mut self, offset: Vec2) -> Self {
        self.sensors_offset = offset;
        self
    }

    pub fn operation(mut self, operation: Operation) -> Self {
        self.operations.push(operation);
        self
    }

    pub fn add(self, pos: Vec2, size: Vec2) -> Self {
        self.operation(Operation::new(Action::Add, Shape::Rectangle, pos, size))
    }

    pub fn add_material(self, pos: Vec2, size: Vec2, material: &str) -> Self {
        self.operation(
            Operation::new(Action::Add, Shape::Rectangle, pos, size).set_material(material),
        )
    }

    pub fn subtract(self, pos: Vec2, size: Vec2) -> Self {
        self.operation(Operation::new(
            Action::Subtract,
            Shape::Rectangle,
            pos,
            size,
        ))
    }

    pub fn zone(mut self, zone: Zone) -> Self {
        self.zones.push(zone);
        self
    }
}
impl Hash for Room {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.material.hash(state);
        hash_vec2(self.pos, state);
        hash_vec2(self.size, state);
        self.operations.hash(state);
        self.walls.hash(state);
        self.openings.hash(state);
        self.outline.hash(state);
        self.furniture.hash(state);
    }
}

impl Sensor {
    pub fn new(entity_id: &str, display_name: &str, unit: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            entity_id: entity_id.to_owned(),
            display_name: display_name.to_owned(),
            unit: unit.to_owned(),
        }
    }

    pub fn default() -> Self {
        Self::new("sensor_id", "TMP", "°C")
    }
}

impl Opening {
    pub fn new(opening_type: OpeningType, pos: Vec2, rotation: i32) -> Self {
        Self {
            id: Uuid::new_v4(),
            opening_type,
            pos,
            rotation,
            width: 0.8,
            flipped: false,
            open_amount: 0.0,
        }
    }

    pub fn default() -> Self {
        Self::new(OpeningType::Door, Vec2::ZERO, 0)
    }

    pub const fn width(mut self, width: f64) -> Self {
        self.width = width;
        self
    }

    pub const fn flip(mut self) -> Self {
        self.flipped = !self.flipped;
        self
    }
}
impl Hash for Opening {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.opening_type.hash(state);
        hash_vec2(self.pos, state);
        self.rotation.hash(state);
        self.width.to_bits().hash(state);
    }
}

impl Light {
    pub fn new(name: &str, pos: Vec2) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.to_owned(),
            entity_id: name.to_lowercase().replace(' ', "_"),
            light_type: LightType::Dimmable,
            pos,
            multi: None,
            intensity: 2.0,
            radius: 0.2,
            state: 0,
            lerped_state: 0.0,
            light_data: None,
            last_manual: 0.0,
        }
    }

    pub fn multi(name: &str, pos: Vec2, room_padding: Vec2, rows: u8, cols: u8) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.to_owned(),
            entity_id: name.to_lowercase().replace(' ', "_"),
            light_type: LightType::Dimmable,
            pos,
            multi: Some(MultiLight {
                room_padding,
                rows,
                cols,
            }),
            intensity: 2.0,
            radius: 0.2,
            state: 0,
            lerped_state: 0.0,
            light_data: None,
            last_manual: 0.0,
        }
    }

    pub fn get_points(&self, room_pos: Vec2, room_size: Vec2) -> Vec<Vec2> {
        self.multi.as_ref().map_or_else(
            || vec![room_pos + self.pos],
            |multi| {
                let mut lights_data = Vec::new();
                let size = room_size - multi.room_padding;
                let spacing = if multi.cols > 1 && multi.rows > 1 {
                    size / vec2(f64::from(multi.cols) - 1.0, f64::from(multi.rows) - 1.0)
                } else if multi.cols > 1 {
                    vec2(size.x / (f64::from(multi.cols) - 1.0), 0.0)
                } else if multi.rows > 1 {
                    vec2(0.0, size.y / (f64::from(multi.rows) - 1.0))
                } else {
                    Vec2::ZERO
                };
                for col in 0..multi.cols {
                    let x_pos =
                        self.pos.x + (f64::from(col) - f64::from(multi.cols - 1) * 0.5) * spacing.x;
                    for row in 0..multi.rows {
                        let y_pos = self.pos.y
                            + (f64::from(row) - f64::from(multi.rows - 1) * 0.5) * spacing.y;
                        lights_data.push(room_pos + vec2(x_pos, y_pos));
                    }
                }
                lights_data
            },
        )
    }

    pub fn default() -> Self {
        Self::new("", Vec2::ZERO)
    }
}
impl Hash for Light {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_vec2(self.pos, state);
        self.multi.hash(state);
        self.intensity.to_bits().hash(state);
        self.radius.to_bits().hash(state);
        self.state.hash(state);
        self.lerped_state.to_bits().hash(state);
    }
}
impl MultiLight {
    pub const fn default() -> Self {
        Self {
            room_padding: vec2(0.5, 0.5),
            rows: 1,
            cols: 1,
        }
    }
}
impl Hash for MultiLight {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_vec2(self.room_padding, state);
        self.rows.hash(state);
        self.cols.hash(state);
    }
}

impl Operation {
    pub fn new(action: Action, shape: Shape, pos: Vec2, size: Vec2) -> Self {
        Self {
            id: Uuid::new_v4(),
            action,
            shape,
            material: None,
            pos,
            size,
            rotation: 0,
        }
    }

    pub fn default() -> Self {
        Self::new(Action::Add, Shape::Rectangle, Vec2::ZERO, vec2(1.0, 1.0))
    }

    pub fn set_material(mut self, material: &str) -> Self {
        self.material = Some(material.to_owned());
        self
    }
}
impl Hash for Operation {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.action.hash(state);
        self.shape.hash(state);
        self.material.hash(state);
        hash_vec2(self.pos, state);
        hash_vec2(self.size, state);
        self.rotation.hash(state);
    }
}

impl Zone {
    pub fn new(name: &str, shape: Shape, pos: Vec2, size: Vec2) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.to_owned(),
            shape,
            pos,
            size,
            rotation: 0,
        }
    }

    pub fn default() -> Self {
        Self::new("Zone", Shape::Rectangle, Vec2::ZERO, vec2(1.0, 1.0))
    }
}
impl Hash for Zone {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.shape.hash(state);
        hash_vec2(self.pos, state);
        hash_vec2(self.size, state);
        self.rotation.hash(state);
    }
}

impl Outline {
    pub const fn new(thickness: f64, color: Color) -> Self {
        Self { thickness, color }
    }

    pub const fn default() -> Self {
        Self::new(0.05, Color::WHITE)
    }
}
impl Hash for Outline {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.thickness.to_bits().hash(state);
        self.color.hash(state);
    }
}

impl GlobalMaterial {
    pub fn new(name: &str, material: Material, tint: Color) -> Self {
        Self {
            name: name.to_owned(),
            material,
            tint,
            tiles: None,
        }
    }

    pub const fn tiles(mut self, spacing: f64, grout_width: f64, grout_color: Color) -> Self {
        self.tiles = Some(TileOptions {
            spacing,
            grout_width,
            grout_color,
        });
        self
    }
}
impl Hash for GlobalMaterial {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.material.hash(state);
        self.tint.hash(state);
        self.tiles.hash(state);
    }
}

impl Hash for TileOptions {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.spacing.to_bits().hash(state);
        self.grout_width.to_bits().hash(state);
        self.grout_color.hash(state);
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, EnumIter, Default, Hash)]
pub enum Material {
    #[default]
    Empty,
    Carpet,
    Fabric,
    Marble,
    Granite,
    Wood,
}

impl Material {
    pub const fn get_image(&self) -> &[u8] {
        match self {
            Self::Empty => include_bytes!("../../assets/textures/empty.png"),
            Self::Carpet => include_bytes!("../../assets/textures/carpet.png"),
            Self::Fabric => include_bytes!("../../assets/textures/fabric.png"),
            Self::Marble => include_bytes!("../../assets/textures/marble.png"),
            Self::Granite => include_bytes!("../../assets/textures/granite.png"),
            Self::Wood => include_bytes!("../../assets/textures/wood.png"),
        }
    }
}
