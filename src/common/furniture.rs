use super::{
    layout::{Shape, Triangles},
    shape::{create_multipolygon, triangulate_polygon},
    utils::{clone_as_none, display_vec2, hash_vec2, Material},
};
use derivative::Derivative;
use egui::Color32;
use geo_types::MultiPolygon;
use glam::{dvec2 as vec2, DVec2 as Vec2};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
};
use strum_macros::{Display, EnumIter};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Derivative)]
#[derivative(Clone)]
pub struct Furniture {
    pub id: Uuid,
    pub furniture_type: FurnitureType,
    pub pos: Vec2,
    pub size: Vec2,
    pub rotation: f64,
    pub children: Vec<Furniture>,
    #[serde(skip)]
    #[derivative(Clone(clone_with = "clone_as_none"))]
    pub rendered_data: Option<FurnitureRender>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, EnumIter, Hash)]
pub enum FurnitureType {
    Chair(ChairType),
    Table(TableType),
    Bed,
    Wardrobe,
    Rug,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, EnumIter, Default, Hash)]
pub enum ChairType {
    #[default]
    DiningChair,
    OfficeChair,
    Sofa,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, EnumIter, Default, Hash)]
pub enum TableType {
    #[default]
    DiningTable,
    Desk,
}

impl Furniture {
    pub fn new(furniture_type: FurnitureType, pos: Vec2, size: Vec2, rotation: f64) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            furniture_type,
            pos,
            size,
            rotation,
            children: Vec::new(),
            rendered_data: None,
        }
    }

    pub fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            furniture_type: FurnitureType::Chair(ChairType::default()),
            pos: Vec2::ZERO,
            size: vec2(1.0, 1.0),
            rotation: 0.0,
            children: Vec::new(),
            rendered_data: None,
        }
    }

    pub fn polygons(
        &self,
    ) -> (
        HashMap<FurnitureMaterial, MultiPolygon>,
        HashMap<FurnitureMaterial, Vec<Triangles>>,
    ) {
        let mut polygons = HashMap::new();

        let first_material = FurnitureMaterial {
            material: Material::Wood,
            tint: Color32::from_rgb(190, 120, 80),
        };
        let vertices = Shape::Rectangle.vertices(self.pos, self.size, self.rotation);
        polygons.insert(first_material, create_multipolygon(&vertices));

        // Create triangles for each material
        let mut triangles = HashMap::new();
        for (material, poly) in &polygons {
            let mut material_triangles = Vec::new();
            for polygon in &poly.0 {
                let (indices, vertices) = triangulate_polygon(polygon);
                material_triangles.push(Triangles { indices, vertices });
            }
            triangles.insert(material.clone(), material_triangles);
        }

        (polygons, triangles)
    }
}

impl std::fmt::Display for Furniture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut string = format!(
            "Furniture: {} {}m @ {}m",
            self.furniture_type.fancy_display(),
            display_vec2(self.size),
            display_vec2(self.pos)
        );
        if self.rotation != 0.0 {
            string.push_str(format!(" {}°", self.rotation).as_str());
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
        self.furniture_type.hash(state);
        hash_vec2(self.pos, state);
        hash_vec2(self.size, state);
        self.rotation.to_bits().hash(state);
        for child in &self.children {
            child.hash(state);
        }
    }
}

impl FurnitureType {
    fn fancy_display(self) -> String {
        match self {
            Self::Chair(sub) => format!("{self}: {sub}"),
            Self::Table(sub) => format!("{self}: {sub}"),
            _ => self.to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct FurnitureMaterial {
    pub material: Material,
    pub tint: Color32,
}

pub struct FurnitureRender {
    pub hash: u64,
    pub polygons: HashMap<FurnitureMaterial, MultiPolygon>,
    pub triangles: HashMap<FurnitureMaterial, Vec<Triangles>>,
}
