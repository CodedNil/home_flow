use super::{
    layout::{
        Action, Furniture, GlobalMaterial, Home, Opening, OpeningType, Operation, Outline, Room,
        Shape, Walls,
    },
    shape::Material,
};
use egui::Color32;
use glam::{dvec2 as vec2, DVec2 as Vec2};
use std::hash::{Hash, Hasher};

pub fn hash_vec2<H: Hasher>(vec: Vec2, state: &mut H) {
    vec.x.to_bits().hash(state);
    vec.y.to_bits().hash(state);
}

pub const fn vec2_to_egui_pos(vec: Vec2) -> egui::Pos2 {
    egui::pos2(vec.x as f32, vec.y as f32)
}

pub const fn egui_to_vec2(vec: egui::Vec2) -> Vec2 {
    vec2(vec.x as f64, vec.y as f64)
}

pub const fn egui_pos_to_vec2(vec: egui::Pos2) -> Vec2 {
    vec2(vec.x as f64, vec.y as f64)
}

pub fn rotate_point(point: Vec2, pivot: Vec2, angle: f64) -> Vec2 {
    let cos_theta = angle.to_radians().cos();
    let sin_theta = angle.to_radians().sin();

    vec2(
        cos_theta * (point.x - pivot.x) - sin_theta * (point.y - pivot.y) + pivot.x,
        sin_theta * (point.x - pivot.x) + cos_theta * (point.y - pivot.y) + pivot.y,
    )
}

fn color_to_string(color: Color32) -> String {
    format!(
        "#{:02x}{:02x}{:02x}{:02x}",
        color.r(),
        color.g(),
        color.b(),
        color.a()
    )
}

impl Hash for Home {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.version.hash(state);
        self.rooms.hash(state);
        self.furniture.hash(state);
    }
}
impl std::fmt::Display for Home {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut string = String::new();
        for room in &self.rooms {
            string.push_str(format!("{room}\n").as_str());
        }
        write!(f, "{string}")
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
        hash_vec2(self.pos, state);
        hash_vec2(self.size, state);
        self.walls.hash(state);
        self.operations.hash(state);
        self.openings.hash(state);
        self.material.hash(state);
    }
}
impl std::fmt::Display for Room {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut string = format!(
            "Room: {} - {}x{}m @ {}x{}m\n",
            self.name, self.size.x, self.size.y, self.pos.x, self.pos.y
        );
        for operation in &self.operations {
            let op_string = operation.to_string().replace('\n', "\n        ");
            string.push_str(format!("    Operation: {op_string}\n").as_str());
        }

        // Walls
        string.push_str("    Walls: ");
        for index in 0..4 {
            let (is_wall, wall_side) = match index {
                0 => (self.walls.left, "Left"),
                1 => (self.walls.top, "Top"),
                2 => (self.walls.right, "Right"),
                _ => (self.walls.bottom, "Bottom"),
            };
            string.push_str(format!("[{wall_side}: {is_wall}] ").as_str());
        }
        string.push('\n');

        write!(f, "{string}")
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
impl std::fmt::Display for Opening {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut string = format!(
            "Opening: {} - {}m @ {}m",
            self.opening_type, self.width, self.pos
        );
        if self.rotation != 0.0 {
            string.push_str(format!(" - {}°", self.rotation).as_str());
        }
        write!(f, "{string}")
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

impl Furniture {
    pub fn new(pos: Vec2, size: Vec2, rotation: f64) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            pos,
            size,
            rotation,
            children: Vec::new(),
        }
    }
}
impl std::fmt::Display for Furniture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut string = format!(
            "Furniture: {}x{}m @ {}x{}m",
            self.size.x, self.size.y, self.pos.x, self.pos.y
        );
        if self.rotation != 0.0 {
            string.push_str(format!(" - {}°", self.rotation).as_str());
        }
        string.push('\n');

        for child in &self.children {
            string.push_str(format!("    Child: {child}\n").as_str());
        }

        write!(f, "{string}")
    }
}
impl Hash for Furniture {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_vec2(self.pos, state);
        hash_vec2(self.size, state);
        self.rotation.to_bits().hash(state);
        for child in &self.children {
            child.hash(state);
        }
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

    pub fn set_material(&self, material: &str) -> Self {
        Self {
            material: Some(material.to_owned()),
            ..*self
        }
    }
}
impl std::fmt::Display for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut string = format!(
            "Operation: {} {} - {}x{}m @ {}x{}m",
            self.action, self.shape, self.size.x, self.size.y, self.pos.x, self.pos.y
        );
        if self.rotation != 0.0 {
            string.push_str(format!(" - {}°", self.rotation).as_str());
        }
        if let Some(material) = &self.material {
            string.push_str(format!("\nMaterial: {material}").as_str());
        }

        write!(f, "{string}")
    }
}
impl Hash for Operation {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_vec2(self.pos, state);
        hash_vec2(self.size, state);
        self.rotation.to_bits().hash(state);
        self.action.hash(state);
        self.shape.hash(state);
        self.material.hash(state);
    }
}

impl Outline {
    pub const fn new(thickness: f64, color: Color32) -> Self {
        Self { thickness, color }
    }

    pub const fn default() -> Self {
        Self {
            thickness: 0.05,
            color: Color32::WHITE,
        }
    }
}
impl std::fmt::Display for Outline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = format!(
            "Outline: Thickness: {} - Color: {}",
            self.thickness,
            color_to_string(self.color)
        );
        write!(f, "{string}")
    }
}
impl Hash for Outline {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.thickness.to_bits().hash(state);
        self.color.hash(state);
    }
}

impl GlobalMaterial {
    pub fn new(name: &str, material: Material, tint: Color32) -> Self {
        Self {
            name: name.to_owned(),
            material,
            tint: Some(tint),
        }
    }
}
impl std::fmt::Display for GlobalMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut string = format!(
            "GlobalMaterial: {} - Material: {}",
            self.name, self.material
        );
        if let Some(tint) = self.tint {
            string.push_str(format!(" - Tint: {}", color_to_string(tint)).as_str());
        }
        write!(f, "{string}")
    }
}
impl Hash for GlobalMaterial {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.material.hash(state);
        self.tint.hash(state);
    }
}

pub const fn clone_as_none<T>(_x: &Option<T>) -> Option<T> {
    None
}
