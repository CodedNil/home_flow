use super::{
    color::Color,
    layout::{Shape, Triangles},
    shape::{triangulate_polygon, EMPTY_MULTI_POLYGON},
    utils::{clone_as_none, display_vec2, hash_vec2, rotate_point, Material},
};
use derivative::Derivative;
use geo::{triangulate_spade::SpadeTriangulationConfig, BooleanOps, TriangulateSpade};
use geo_types::MultiPolygon;
use glam::{dvec2 as vec2, DVec2 as Vec2};
use indexmap::IndexMap;
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
    Bed(Color),
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

    pub fn polygons(&self) -> FurniturePolygons {
        let mut polygons = IndexMap::new();
        match self.furniture_type {
            FurnitureType::Chair(sub_type) => self.chair_render(&mut polygons, sub_type),
            FurnitureType::Table(sub_type) => self.table_render(&mut polygons, sub_type),
            FurnitureType::Bed(color) => self.bed_render(&mut polygons, color),
            FurnitureType::Wardrobe => self.wardrobe_render(&mut polygons),
            FurnitureType::Rug(color) => self.rug_render(&mut polygons, color),
        }
        polygons
    }

    pub fn render(&self) -> (FurniturePolygons, FurnitureTriangles, FurnitureShadows) {
        let mut new_furniture = self.clone();
        new_furniture.rotation = 0.0;
        new_furniture.pos = Vec2::ZERO;
        let polygons = new_furniture.polygons();

        // Create triangles for each material
        let mut triangles = IndexMap::new();
        for (material, poly) in &polygons {
            let mut material_triangles = Vec::new();
            for polygon in &poly.0 {
                let (indices, vertices) = triangulate_polygon(polygon);
                material_triangles.push(Triangles { indices, vertices });
            }
            triangles.insert(material.clone(), material_triangles);
        }

        let mut shadow_exterior = EMPTY_MULTI_POLYGON;
        let mut shadow_interior = EMPTY_MULTI_POLYGON;
        for (_, poly) in &polygons {
            shadow_exterior =
                shadow_exterior.union(&geo_buffer::buffer_multi_polygon_rounded(poly, 0.05));
            shadow_interior =
                shadow_interior.union(&geo_buffer::buffer_multi_polygon_rounded(poly, -0.025));
        }

        // Create shadow triangles
        let interior_points = shadow_interior
            .0
            .iter()
            .flat_map(|p| p.exterior().points())
            .map(|p| vec2(p.x(), p.y()))
            .collect::<Vec<_>>();
        let shadow_polygons = shadow_exterior.difference(&shadow_interior);

        let mut shadow_triangles = Vec::new();
        for polygon in &shadow_polygons {
            let (indices, vertices) = {
                let triangles = polygon
                    .constrained_triangulation(SpadeTriangulationConfig::default())
                    .unwrap();
                let mut indices = Vec::new();
                let mut vertices = Vec::new();
                for triangle in triangles {
                    for point in triangle.to_array() {
                        let index = vertices.len() as u32;
                        indices.push(index);
                        vertices.push(vec2(point.x, point.y));
                    }
                }
                (indices, vertices)
            };
            let mut vertex_position_map = HashMap::new();
            for (index, vertex) in vertices.iter().enumerate() {
                let is_interior = interior_points
                    .iter()
                    .any(|p| p.distance(*vertex) < f64::EPSILON);
                vertex_position_map.insert(index, is_interior);
            }
            shadow_triangles.push((Triangles { indices, vertices }, vertex_position_map));
        }

        (polygons, triangles, shadow_triangles)
    }

    fn full_shape(&self) -> MultiPolygon {
        Shape::Rectangle.polygons(Vec2::ZERO, self.size, 0.0)
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

    fn bed_render(&self, polygons: &mut FurniturePolygons, color: Color) {
        let sheet_color = Color::from_rgb(250, 230, 210);
        let pillow_color = Color::from_rgb(255, 255, 255);

        // Add sheets
        polygons.insert(
            FurnitureMaterial::new(Material::Empty, sheet_color),
            self.full_shape(),
        );

        // Add pillows, 65x50cm
        let pillow_spacing = 0.05;
        let available_width = self.size.x - pillow_spacing;
        let (pillow_width, pillow_height) = (0.62, 0.45);
        let pillow_full_width = pillow_width + 0.05;
        let num_pillows = (available_width / pillow_full_width).floor().max(1.0) as usize;
        let mut pillow_polygon = EMPTY_MULTI_POLYGON;
        for i in 0..num_pillows {
            let pillow_pos = self.pos
                + vec2(
                    pillow_full_width * i as f64
                        - ((num_pillows - 1) as f64 * pillow_full_width) * 0.5,
                    (self.size.y - pillow_height) * 0.5 - pillow_spacing,
                );
            let pillow =
                Shape::Rectangle.polygon(pillow_pos, vec2(pillow_width, pillow_height), 0.0);
            pillow_polygon.0.push(pillow);
        }
        fancy_inlay(
            polygons,
            pillow_polygon,
            Material::Empty,
            pillow_color,
            -0.015,
            0.03,
        );

        // Add covers
        let covers_size = (self.size.y - pillow_height - pillow_spacing * 2.0) / self.size.y;
        let cover_polygon = Shape::Rectangle.polygons(
            self.pos - vec2(0.0, self.size.y * (1.0 - covers_size) / 2.0),
            vec2(self.size.x, self.size.y * covers_size),
            self.rotation,
        );
        fancy_inlay(
            polygons,
            cover_polygon,
            Material::Empty,
            color,
            -0.025,
            0.05,
        );

        // Add backboard
        let backboard_polygon = Shape::Rectangle.polygons(
            self.pos + vec2(0.0, self.size.y * 0.5 + 0.025),
            vec2(self.size.x + 0.05, 0.05),
            self.rotation,
        );
        polygons.insert(
            FurnitureMaterial::new(Material::Wood, Color::from_rgb(190, 120, 80)),
            backboard_polygon,
        );
    }

    fn wardrobe_render(&self, polygons: &mut FurniturePolygons) {
        polygons.insert(
            FurnitureMaterial::new(Material::Wood, Color::from_rgb(190, 120, 80)),
            self.full_shape(),
        );
    }

    fn rug_render(&self, polygons: &mut FurniturePolygons, color: Color) {
        fancy_inlay(
            polygons,
            self.full_shape(),
            Material::Carpet,
            color,
            -0.05,
            0.1,
        );
    }
}

fn fancy_inlay(
    polygons: &mut FurniturePolygons,
    poly: MultiPolygon,
    material: Material,
    color: Color,
    lighten: f64,
    inset: f64,
) {
    let inset_poly = inset_polygon(&poly, inset);
    polygons.insert(FurnitureMaterial::new(material, color), poly);
    polygons.insert(
        FurnitureMaterial::new(material, color.lighten(lighten)),
        inset_poly,
    );
}

fn inset_polygon(polygon: &MultiPolygon, inset: f64) -> MultiPolygon {
    geo_buffer::buffer_multi_polygon(polygon, -inset)
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
        hash_vec2(self.size, state);
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

type FurniturePolygons = IndexMap<FurnitureMaterial, MultiPolygon>;
type FurnitureTriangles = IndexMap<FurnitureMaterial, Vec<Triangles>>;
type FurnitureShadows = Vec<(Triangles, HashMap<usize, bool>)>;

pub struct FurnitureRender {
    pub hash: u64,
    pub polygons: FurniturePolygons,
    pub triangles: FurnitureTriangles,
    pub shadow_triangles: FurnitureShadows,
}
