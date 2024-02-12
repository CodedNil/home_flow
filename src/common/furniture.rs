use super::{
    color::Color,
    layout::{Shape, Triangles},
    shape::{polygons_to_shadows, triangulate_polygon, ShadowsData, EMPTY_MULTI_POLYGON},
    utils::{clone_as_none, hash_vec2, Material},
};
use derivative::Derivative;
use geo_types::MultiPolygon;
use glam::{dvec2 as vec2, DVec2 as Vec2};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
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
    Sofa(Color),
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
        Self::new(
            FurnitureType::Chair(ChairType::default()),
            Vec2::ZERO,
            vec2(1.0, 1.0),
            0.0,
        )
    }

    pub fn contains(&self, point: Vec2) -> bool {
        Shape::Rectangle.contains(point, self.pos, self.size, self.rotation)
    }

    pub fn polygons(&self) -> FurniturePolygons {
        let mut polygons = Vec::new();
        match self.furniture_type {
            FurnitureType::Chair(sub_type) => self.chair_render(&mut polygons, sub_type),
            FurnitureType::Table(sub_type) => self.table_render(&mut polygons, sub_type),
            FurnitureType::Bed(color) => self.bed_render(&mut polygons, color),
            FurnitureType::Wardrobe => self.wardrobe_render(&mut polygons),
            FurnitureType::Rug(color) => self.rug_render(&mut polygons, color),
        }
        polygons
    }

    pub fn render(&self) -> (FurniturePolygons, FurnitureTriangles, ShadowsData) {
        let mut new_furniture = self.clone();
        new_furniture.rotation = 0.0;
        new_furniture.pos = Vec2::ZERO;
        let polygons = new_furniture.polygons();

        // Create triangles for each material
        let mut triangles = Vec::new();
        for (material, poly) in &polygons {
            let mut material_triangles = Vec::new();
            for polygon in &poly.0 {
                let (indices, vertices) = triangulate_polygon(polygon);
                material_triangles.push(Triangles { indices, vertices });
            }
            triangles.push((material.clone(), material_triangles));
        }

        let shadows_data = polygons_to_shadows(polygons.iter().map(|(_, p)| p).collect::<Vec<_>>());

        (polygons, triangles, shadows_data)
    }

    fn full_shape(&self) -> MultiPolygon {
        Shape::Rectangle.polygons(Vec2::ZERO, self.size, 0.0)
    }

    fn chair_render(&self, polygons: &mut FurniturePolygons, sub_type: ChairType) {
        let material = match sub_type {
            ChairType::DiningChair | ChairType::OfficeChair => {
                FurnitureMaterial::new(Material::Wood, Color::from_rgb(190, 120, 80))
            }
            ChairType::Sofa(color) => FurnitureMaterial::new(Material::Empty, color),
        };
        polygons.push((material, self.full_shape()));
    }

    fn table_render(&self, polygons: &mut FurniturePolygons, sub_type: TableType) {
        polygons.push((
            FurnitureMaterial::new(Material::Wood, Color::from_rgb(190, 120, 80)),
            self.full_shape(),
        ));
    }

    fn bed_render(&self, polygons: &mut FurniturePolygons, color: Color) {
        let sheet_color = Color::from_rgb(250, 230, 210);
        let pillow_color = Color::from_rgb(255, 255, 255);

        // Add sheets
        polygons.push((
            FurnitureMaterial::new(Material::Empty, sheet_color),
            self.full_shape(),
        ));

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
        polygons.push((
            FurnitureMaterial::new(Material::Wood, Color::from_rgb(190, 120, 80)),
            backboard_polygon,
        ));
    }

    fn wardrobe_render(&self, polygons: &mut FurniturePolygons) {
        polygons.push((
            FurnitureMaterial::new(Material::Wood, Color::from_rgb(190, 120, 80)),
            self.full_shape(),
        ));
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
    polygons.push((FurnitureMaterial::new(material, color), poly));
    polygons.push((
        FurnitureMaterial::new(material, color.lighten(lighten)),
        inset_poly,
    ));
}

fn inset_polygon(polygon: &MultiPolygon, inset: f64) -> MultiPolygon {
    geo_buffer::buffer_multi_polygon(polygon, -inset)
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

type FurniturePolygons = Vec<(FurnitureMaterial, MultiPolygon)>;
type FurnitureTriangles = Vec<(FurnitureMaterial, Vec<Triangles>)>;

pub struct FurnitureRender {
    pub hash: u64,
    pub polygons: FurniturePolygons,
    pub triangles: FurnitureTriangles,
    pub shadow_triangles: ShadowsData,
}
