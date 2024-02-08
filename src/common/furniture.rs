use super::{
    color::Color,
    layout::{Shape, Triangles},
    shape::triangulate_polygon,
    utils::{clone_as_none, display_vec2, hash_vec2, Material},
};
use derivative::Derivative;
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
    Rug(Color),
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

    pub fn polygons(&self) -> (FurniturePolygons, FurnitureTriangles) {
        let mut polygons = HashMap::new();

        match self.furniture_type {
            FurnitureType::Chair(sub_type) => self.chair_render(&mut polygons, sub_type),
            FurnitureType::Table(sub_type) => self.table_render(&mut polygons, sub_type),
            FurnitureType::Bed => self.bed_render(&mut polygons),
            FurnitureType::Wardrobe => self.wardrobe_render(&mut polygons),
            FurnitureType::Rug(color) => self.rug_render(&mut polygons, color),
        }

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

    fn full_shape(&self) -> MultiPolygon {
        Shape::Rectangle.polygon(self.pos, self.size, self.rotation)
    }

    fn inlayed_shape(&self) -> MultiPolygon {
        Shape::Rectangle.polygon(self.pos, self.size - vec2(0.1, 0.1), self.rotation)
    }

    fn chair_render(&self, polygons: &mut FurniturePolygons, sub_type: ChairType) {
        polygons.insert(
            FurnitureMaterial::new(Material::Wood, Color::from_rgb(190, 120, 80)),
            self.full_shape(),
        );
    }

    fn table_render(&self, polygons: &mut FurniturePolygons, sub_type: TableType) {
        polygons.insert(
            FurnitureMaterial::new(Material::Wood, Color::from_rgb(190, 120, 80)),
            self.full_shape(),
        );
    }

    fn bed_render(&self, polygons: &mut FurniturePolygons) {
        polygons.insert(
            FurnitureMaterial::new(Material::Wood, Color::from_rgb(190, 120, 80)),
            self.full_shape(),
        );
    }

    fn wardrobe_render(&self, polygons: &mut FurniturePolygons) {
        polygons.insert(
            FurnitureMaterial::new(Material::Wood, Color::from_rgb(190, 120, 80)),
            self.full_shape(),
        );
    }

    fn rug_render(&self, polygons: &mut FurniturePolygons, color: Color) {
        polygons.insert(
            FurnitureMaterial::new(Material::Carpet, color.lighten(0.5)),
            self.full_shape(),
        );

        polygons.insert(
            FurnitureMaterial::new(Material::Carpet, color),
            self.inlayed_shape(),
        );
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
    pub tint: Color,
}

impl FurnitureMaterial {
    pub const fn new(material: Material, tint: Color) -> Self {
        Self { material, tint }
    }
}

type FurniturePolygons = HashMap<FurnitureMaterial, MultiPolygon>;
type FurnitureTriangles = HashMap<FurnitureMaterial, Vec<Triangles>>;

pub struct FurnitureRender {
    pub hash: u64,
    pub polygons: FurniturePolygons,
    pub triangles: FurnitureTriangles,
}
