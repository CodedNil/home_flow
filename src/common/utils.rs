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

pub fn rotate_point_i32(point: Vec2, pivot: Vec2, angle: i32) -> Vec2 {
    rotate_point(point, pivot, angle as f64)
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
    pub fn new(name: &str, pos: Vec2, size: Vec2, material: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            name: name.to_owned(),
            material: material.to_owned(),
            pos,
            size,
            walls: Walls::WALL,
            operations: Vec::new(),
            openings: Vec::new(),
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
        let mut clone = self.clone();
        clone.outline = Some(outline);
        clone
    }

    pub fn no_wall_left(&self) -> Self {
        let mut clone = self.clone();
        clone.walls = self.walls.left(false);
        clone
    }

    pub fn no_wall_top(&self) -> Self {
        let mut clone = self.clone();
        clone.walls = self.walls.top(false);
        clone
    }

    pub fn no_wall_right(&self) -> Self {
        let mut clone = self.clone();
        clone.walls = self.walls.right(false);
        clone
    }

    pub fn no_wall_bottom(&self) -> Self {
        let mut clone = self.clone();
        clone.walls = self.walls.bottom(false);
        clone
    }

    pub fn opening(&self, opening: Opening) -> Self {
        let mut clone = self.clone();
        clone.openings.push(opening);
        clone
    }

    pub fn window(&self, pos: Vec2, rotation: i32) -> Self {
        self.opening(Opening::new(OpeningType::Window, pos, rotation))
    }

    pub fn window_width(&self, pos: Vec2, rotation: i32, width: f64) -> Self {
        self.opening(Opening::new(OpeningType::Window, pos, rotation).width(width))
    }

    pub fn door(&self, pos: Vec2, rotation: i32) -> Self {
        self.opening(Opening::new(OpeningType::Door, pos, rotation))
    }

    pub fn door_width(&self, pos: Vec2, rotation: i32, width: f64) -> Self {
        self.opening(Opening::new(OpeningType::Door, pos, rotation).width(width))
    }

    pub fn operation(&self, operation: Operation) -> Self {
        let mut clone = self.clone();
        clone.operations.push(operation);
        clone
    }

    pub fn add(&self, pos: Vec2, size: Vec2) -> Self {
        self.operation(Operation::new(Action::Add, Shape::Rectangle, pos, size))
    }

    pub fn add_material(&self, pos: Vec2, size: Vec2, material: &str) -> Self {
        self.operation(
            Operation::new(Action::Add, Shape::Rectangle, pos, size).set_material(material),
        )
    }

    pub fn subtract(&self, pos: Vec2, size: Vec2) -> Self {
        self.operation(Operation::new(
            Action::Subtract,
            Shape::Rectangle,
            pos,
            size,
        ))
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
    pub fn new(opening_type: OpeningType, pos: Vec2, rotation: i32) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            opening_type,
            pos,
            rotation,
            width: 0.8,
            open_amount: 0.0,
        }
    }

    pub fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            opening_type: OpeningType::Door,
            pos: Vec2::ZERO,
            rotation: 0,
            width: 0.8,
            open_amount: 0.0,
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
        self.rotation.hash(state);
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
            rotation: 0,
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
            rotation: 0,
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
        self.rotation.hash(state);
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
