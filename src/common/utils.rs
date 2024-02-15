use super::{
    color::Color,
    layout::{
        Action, GlobalMaterial, Home, Opening, OpeningType, Operation, Outline, Room, Shape,
        TileOptions, Walls,
    },
};
use glam::{dvec2 as vec2, DVec2 as Vec2};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use strum_macros::{Display, EnumIter};

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

pub fn rotate_point(point: Vec2, pivot: Vec2, angle: f64) -> Vec2 {
    let cos_theta = angle.to_radians().cos();
    let sin_theta = angle.to_radians().sin();

    vec2(
        cos_theta * (point.x - pivot.x) - sin_theta * (point.y - pivot.y) + pivot.x,
        sin_theta * (point.x - pivot.x) + cos_theta * (point.y - pivot.y) + pivot.y,
    )
}

pub const fn clone_as_none<T>(_x: &Option<T>) -> Option<T> {
    None
}

impl Hash for Home {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.version.hash(state);
        self.materials.hash(state);
        self.rooms.hash(state);
        self.furniture.hash(state);
    }
}

impl Room {
    pub fn new(
        name: &str,
        pos: Vec2,
        size: Vec2,
        material: &str,
        walls: Walls,
        operations: Vec<Operation>,
        openings: Vec<Opening>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            name: name.to_owned(),
            material: material.to_owned(),
            pos,
            size,
            walls,
            operations,
            openings,
            outline: None,
            rendered_data: None,
        }
    }

    pub fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            name: "New Room".to_owned(),
            material: String::new(),
            pos: Vec2::ZERO,
            size: vec2(1.0, 1.0),
            walls: Walls::WALL,
            operations: Vec::new(),
            openings: Vec::new(),
            outline: None,
            rendered_data: None,
        }
    }

    pub fn outline(&self, outline: Outline) -> Self {
        Self {
            outline: Some(outline),
            name: self.name.clone(),
            material: self.material.clone(),
            operations: self.operations.clone(),
            openings: self.openings.clone(),
            rendered_data: None,
            ..*self
        }
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
    }
}

impl Opening {
    pub fn new(opening_type: OpeningType, pos: Vec2) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            opening_type,
            pos,
            rotation: 0.0,
            width: 0.8,
            open_amount: 0.0,
        }
    }

    pub fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            opening_type: OpeningType::Door,
            pos: Vec2::ZERO,
            rotation: 0.0,
            width: 0.8,
            open_amount: 0.0,
        }
    }

    pub const fn rotate(&self, angle: f64) -> Self {
        Self {
            rotation: angle,
            ..*self
        }
    }

    pub const fn width(&self, width: f64) -> Self {
        Self { width, ..*self }
    }
}
impl Hash for Opening {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.opening_type.hash(state);
        hash_vec2(self.pos, state);
        self.rotation.to_bits().hash(state);
        self.width.to_bits().hash(state);
    }
}

impl Operation {
    pub fn new(action: Action, shape: Shape, pos: Vec2, size: Vec2) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            action,
            shape,
            material: None,
            pos,
            size,
            rotation: 0.0,
        }
    }

    pub fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            action: Action::Add,
            shape: Shape::Rectangle,
            material: None,
            pos: Vec2::ZERO,
            size: vec2(1.0, 1.0),
            rotation: 0.0,
        }
    }

    pub fn set_material(&self, material: &str) -> Self {
        Self {
            material: Some(material.to_owned()),
            ..*self
        }
    }
}
impl Hash for Operation {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.action.hash(state);
        self.shape.hash(state);
        self.material.hash(state);
        hash_vec2(self.pos, state);
        hash_vec2(self.size, state);
        self.rotation.to_bits().hash(state);
    }
}

impl Outline {
    pub const fn new(thickness: f64, color: Color) -> Self {
        Self { thickness, color }
    }

    pub const fn default() -> Self {
        Self {
            thickness: 0.05,
            color: Color::WHITE,
        }
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

    pub fn tiles(&self, spacing: f64, grout_width: f64, grout_color: Color) -> Self {
        Self {
            name: self.name.clone(),
            tiles: Some(TileOptions {
                spacing,
                grout_width,
                grout_color,
            }),
            ..*self
        }
    }
}
impl Hash for GlobalMaterial {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.material.hash(state);
        self.tint.hash(state);
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
    Limestone,
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
            Self::Limestone => include_bytes!("../../assets/textures/limestone.png"),
            Self::Wood => include_bytes!("../../assets/textures/wood.png"),
        }
    }
}
