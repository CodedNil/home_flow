use super::{
    color::Color,
    layout::{Shape, Triangles},
    shape::{polygons_to_shadows, triangulate_polygon, ShadowsData},
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
    #[serde(skip)]
    #[derivative(Clone(clone_with = "clone_as_none"))]
    pub rendered_data: Option<FurnitureRender>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, EnumIter, Hash)]
pub enum FurnitureType {
    // General
    Chair(ChairType),
    Table(TableType),
    Rug(Color),
    Radiator,
    Display,
    Kitchen(KitchenType),
    Bathroom(BathroomType),
    Bed(Color),
    Storage(StorageType),
    Boiler,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, EnumIter, Default, Hash)]
pub enum ChairType {
    #[default]
    Dining,
    Office,
    Sofa(Color),
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, EnumIter, Default, Hash)]
pub enum TableType {
    #[default]
    Dining,
    Desk(Color),
    Empty,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, EnumIter, Default, Hash)]
pub enum KitchenType {
    #[default]
    WoodenShelf,
    GraniteCounter,
    MarbleCounter,
    Fridge,
    Oven,
    Hob,
    Microwave,
    Sink,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, EnumIter, Default, Hash)]
pub enum BathroomType {
    #[default]
    Toilet,
    Shower,
    Bath,
    Sink,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, EnumIter, Default, Hash)]
pub enum StorageType {
    #[default]
    Wardrobe,
    WardrobeColor(Color),
    Cupboard,
    CupboardColor(Color),
    Drawer,
    DrawerColor(Color),
}

const WOOD: FurnitureMaterial =
    FurnitureMaterial::new(Material::Wood, Color::from_rgb(190, 120, 80));
const CERAMIC: FurnitureMaterial =
    FurnitureMaterial::new(Material::Empty, Color::from_rgb(230, 220, 200));
const METAL_DARK: FurnitureMaterial =
    FurnitureMaterial::new(Material::Empty, Color::from_rgb(80, 80, 80));

impl Furniture {
    pub fn new(furniture_type: FurnitureType, pos: Vec2, size: Vec2, rotation: f64) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            furniture_type,
            pos,
            size,
            rotation,
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

    pub fn render(
        &self,
    ) -> (
        FurniturePolygons,
        FurnitureTriangles,
        ShadowsData,
        Vec<Self>,
    ) {
        let polygons = self.polygons();

        // Create triangles for each material
        let mut triangles = Vec::new();
        for (material, poly) in &polygons {
            let mut material_triangles = Vec::new();
            for polygon in &poly.0 {
                let (indices, vertices) = triangulate_polygon(polygon);
                material_triangles.push(Triangles { indices, vertices });
            }
            triangles.push((*material, material_triangles));
        }

        // Use simple shape for shadow unless complex is needed
        let full_shape = self.full_shape();
        let shadow_polys = polygons.iter().map(|(_, p)| p).collect::<Vec<_>>();
        let shadow_polys = match self.furniture_type {
            FurnitureType::Bed(_) => shadow_polys,
            FurnitureType::Bathroom(sub_type) => match sub_type {
                BathroomType::Toilet | BathroomType::Sink => shadow_polys,
                _ => vec![&full_shape],
            },
            _ => vec![&full_shape],
        };
        let shadows_data = polygons_to_shadows(shadow_polys);

        let children = self.render_children();

        (polygons, triangles, shadows_data, children)
    }

    fn polygons(&self) -> FurniturePolygons {
        let mut polygons = Vec::new();
        match self.furniture_type {
            FurnitureType::Chair(sub_type) => self.chair_render(&mut polygons, sub_type),
            FurnitureType::Table(sub_type) => self.table_render(&mut polygons, sub_type),
            FurnitureType::Bed(color) => self.bed_render(&mut polygons, color),
            FurnitureType::Storage(sub_type) => self.storage_render(&mut polygons, sub_type),
            FurnitureType::Rug(color) => self.rug_render(&mut polygons, color),
            FurnitureType::Kitchen(sub_type) => self.kitchen_render(&mut polygons, sub_type),
            FurnitureType::Bathroom(sub_type) => self.bathroom_render(&mut polygons, sub_type),
            FurnitureType::Boiler => polygons.push((METAL_DARK, self.full_shape())),
            FurnitureType::Radiator => self.radiator_render(&mut polygons),
            FurnitureType::Display => self.display_render(&mut polygons),
        }
        polygons
    }

    fn render_children(&self) -> Vec<Self> {
        let mut children = Vec::new();
        if let FurnitureType::Table(sub_type) = self.furniture_type {
            self.table_children(&mut children, sub_type);
        }
        for child in &mut children {
            let (polygons, triangles, shadow_triangles, children) = child.render();
            child.rendered_data = Some(FurnitureRender {
                hash: 0,
                polygons,
                triangles,
                shadow_triangles,
                children,
            });
        }
        children
    }

    fn full_shape(&self) -> MultiPolygon {
        rect(Vec2::ZERO, self.size)
    }

    fn chair_render(&self, polygons: &mut FurniturePolygons, sub_type: ChairType) {
        let material = match sub_type {
            ChairType::Dining => WOOD,
            ChairType::Office => {
                FurnitureMaterial::new(Material::Empty, Color::from_rgb(40, 40, 40))
            }
            ChairType::Sofa(color) => FurnitureMaterial::new(Material::Fabric, color),
        };

        polygons.push((material, rect(Vec2::ZERO, self.size)));
        let inset = 0.1;
        if self.size.x > inset * 3.0 && self.size.y > inset * 3.0 {
            polygons.push((
                material.lighten(0.1),
                rect(
                    vec2(0.0, -inset * 0.5),
                    self.size - vec2(inset * 2.0, inset),
                ),
            ));
        }
    }

    fn table_render(&self, polygons: &mut FurniturePolygons, sub_type: TableType) {
        let color = match sub_type {
            TableType::Desk(color) => color,
            _ => Color::from_rgb(190, 120, 80),
        };
        fancy_rectangle(
            polygons,
            Vec2::ZERO,
            self.size,
            0.0,
            FurnitureMaterial::new(Material::Wood, color),
            0.04,
            0.1,
        );
    }

    fn table_children(&self, children: &mut Vec<Self>, sub_type: TableType) {
        let chair_size = vec2(0.5, 0.5);
        let chair_push = 0.1;
        match sub_type {
            TableType::Desk(_) => {
                children.push(Self::new(
                    FurnitureType::Chair(ChairType::Office),
                    vec2(0.0, self.size.y * 0.5 + chair_push),
                    chair_size,
                    0.0,
                ));
            }
            TableType::Dining => {
                let spacing = 0.1;
                let chairs_wide = (self.size.x / (chair_size.x + spacing)).floor() as usize;
                let chairs_high = (self.size.y / (chair_size.y + spacing)).floor() as usize;
                for i in 0..chairs_wide {
                    let x_pos =
                        (i as f64 - (chairs_wide - 1) as f64 * 0.5) * (chair_size.x + spacing);
                    children.push(Self::new(
                        FurnitureType::Chair(ChairType::Dining),
                        vec2(x_pos, self.size.y * 0.5 + chair_push),
                        chair_size,
                        0.0,
                    ));
                    children.push(Self::new(
                        FurnitureType::Chair(ChairType::Dining),
                        vec2(x_pos, -self.size.y * 0.5 - chair_push),
                        chair_size,
                        180.0,
                    ));
                }
                for i in 0..chairs_high {
                    let y_pos =
                        (i as f64 - (chairs_high - 1) as f64 * 0.5) * (chair_size.y + spacing);
                    children.push(Self::new(
                        FurnitureType::Chair(ChairType::Dining),
                        vec2(self.size.x * 0.5 + chair_push, y_pos),
                        chair_size,
                        90.0,
                    ));
                    children.push(Self::new(
                        FurnitureType::Chair(ChairType::Dining),
                        vec2(-self.size.x * 0.5 - chair_push, y_pos),
                        chair_size,
                        -90.0,
                    ));
                }
            }
            TableType::Empty => {}
        }
    }

    fn kitchen_render(&self, polygons: &mut FurniturePolygons, sub_type: KitchenType) {
        match sub_type {
            KitchenType::GraniteCounter => {
                polygons.push((
                    FurnitureMaterial::new(Material::Granite, Color::from_rgb(80, 80, 80)),
                    self.full_shape(),
                ));
            }
            KitchenType::MarbleCounter => {
                polygons.push((
                    FurnitureMaterial::new(Material::Marble, Color::from_rgb(255, 255, 255)),
                    self.full_shape(),
                ));
            }
            _ => {
                polygons.push((WOOD, self.full_shape()));
            }
        }
    }

    fn bathroom_render(&self, polygons: &mut FurniturePolygons, sub_type: BathroomType) {
        let ceramic_light = 0.06;
        match sub_type {
            BathroomType::Bath => {
                polygons.push((CERAMIC, rect(Vec2::ZERO, self.size)));
                let inset = 0.1;
                if self.size.x > inset * 3.0 && self.size.y > inset * 4.0 {
                    polygons.push((
                        CERAMIC.lighten(ceramic_light),
                        rect(
                            vec2(0.0, -inset * 0.5),
                            self.size - vec2(inset * 2.0, inset * 3.0),
                        ),
                    ));
                    // Tap
                    polygons.push((
                        METAL_DARK,
                        rect(vec2(0.0, self.size.y * 0.5 - 0.15), vec2(0.2, 0.1)),
                    ));
                }
            }
            BathroomType::Shower => {
                fancy_rectangle(
                    polygons,
                    Vec2::ZERO,
                    self.size,
                    0.0,
                    CERAMIC,
                    ceramic_light,
                    0.1,
                );
                // Tap
                polygons.push((
                    METAL_DARK,
                    rect(vec2(0.0, self.size.y * 0.5 - 0.05), vec2(0.2, 0.1)),
                ));
            }
            BathroomType::Toilet => {
                let rounding_factor = 0.3;
                polygons.push((
                    CERAMIC.lighten(ceramic_light),
                    rect(
                        vec2(0.0, self.size.y * -0.5 + self.size.y * 0.35),
                        vec2(self.size.x * (0.8 - rounding_factor), self.size.y * 0.7),
                    ),
                ));
                polygons.push((
                    CERAMIC.lighten(ceramic_light),
                    rect(
                        vec2(0.0, self.size.y * -0.5 + self.size.y * 0.35),
                        vec2(self.size.x * 0.8, self.size.y * (0.7 - rounding_factor)),
                    ),
                ));
                polygons.push((
                    CERAMIC,
                    rect(
                        vec2(0.0, self.size.y * 0.5 - self.size.y * 0.15),
                        vec2(self.size.x, self.size.y * 0.3),
                    ),
                ));
                // Flusher
                polygons.push((
                    METAL_DARK,
                    rect(vec2(0.0, self.size.y * 0.5 - 0.05), vec2(0.1, 0.1)),
                ));
            }
            BathroomType::Sink => {
                let inset = 0.1;
                polygons.push((
                    CERAMIC,
                    rect(
                        vec2(0.0, inset * 0.5),
                        vec2(self.size.x, self.size.y - inset),
                    ),
                ));
                polygons.push((
                    CERAMIC.lighten(ceramic_light),
                    rect(
                        vec2(0.0, inset * 0.5),
                        vec2(self.size.x - inset * 2.0, self.size.y - inset),
                    ),
                ));
                polygons.push((
                    CERAMIC,
                    rect(
                        vec2(0.0, -self.size.y * 0.5 + inset * 0.5),
                        vec2(self.size.x - inset * 2.0, inset),
                    ),
                ));
                // Tap
                polygons.push((
                    METAL_DARK,
                    rect(vec2(0.0, self.size.y * 0.5 - 0.05), vec2(0.1, 0.1)),
                ));
            }
        }
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
        for i in 0..num_pillows {
            let pillow_pos = vec2(
                pillow_full_width * i as f64 - ((num_pillows - 1) as f64 * pillow_full_width) * 0.5,
                (self.size.y - pillow_height) * 0.5 - pillow_spacing,
            );
            fancy_rectangle(
                polygons,
                pillow_pos,
                vec2(pillow_width, pillow_height),
                0.0,
                FurnitureMaterial::new(Material::Empty, pillow_color),
                -0.015,
                0.03,
            );
        }

        // Add covers
        let covers_size = (self.size.y - pillow_height - pillow_spacing * 2.0) / self.size.y;
        fancy_rectangle(
            polygons,
            -vec2(0.0, self.size.y * (1.0 - covers_size) / 2.0),
            vec2(self.size.x, self.size.y * covers_size),
            0.0,
            FurnitureMaterial::new(Material::Fabric, color),
            -0.025,
            0.05,
        );

        // Add backboard
        let backboard_polygon = rect(
            vec2(0.0, self.size.y * 0.5 + 0.025),
            vec2(self.size.x + 0.05, 0.05),
        );
        polygons.push((WOOD, backboard_polygon));
    }

    fn storage_render(&self, polygons: &mut FurniturePolygons, sub_type: StorageType) {
        let color = match sub_type {
            StorageType::WardrobeColor(color)
            | StorageType::CupboardColor(color)
            | StorageType::DrawerColor(color) => color,
            _ => WOOD.tint,
        };
        polygons.push((
            FurnitureMaterial::new(WOOD.material, color),
            self.full_shape(),
        ));
    }

    fn radiator_render(&self, polygons: &mut FurniturePolygons) {
        polygons.push((
            FurnitureMaterial::new(Material::Empty, Color::from_rgb(255, 255, 255)),
            self.full_shape(),
        ));
        if self.size.x > 0.2 && self.size.y > 0.05 {
            let stripe_width = 0.1;
            let total_stripe_width = self.size.x / 2.0 - stripe_width * 0.5;
            let num_stripes = (total_stripe_width / stripe_width).floor() as usize;
            let adjusted_stripe_width = total_stripe_width / num_stripes as f64;
            for i in 0..num_stripes {
                let x_pos =
                    (i as f64 - (num_stripes - 1) as f64 / 2.0) * adjusted_stripe_width * 2.0;
                polygons.push((
                    FurnitureMaterial::new(Material::Empty, Color::from_rgb(200, 200, 200)),
                    rect(vec2(x_pos, 0.0), vec2(adjusted_stripe_width, self.size.y)),
                ));
            }
        }
    }

    fn display_render(&self, polygons: &mut FurniturePolygons) {
        polygons.push((
            METAL_DARK,
            rect(
                vec2(0.0, -self.size.y * 0.25),
                vec2(self.size.x, self.size.y * 0.5),
            ),
        ));
        polygons.push((
            FurnitureMaterial::new(Material::Empty, Color::from_rgb(50, 150, 255)),
            rect(
                vec2(0.0, self.size.y * 0.25),
                vec2(self.size.x, self.size.y * 0.5),
            ),
        ));
    }

    fn rug_render(&self, polygons: &mut FurniturePolygons, color: Color) {
        fancy_rectangle(
            polygons,
            Vec2::ZERO,
            self.size,
            0.0,
            FurnitureMaterial::new(Material::Carpet, color),
            -0.05,
            0.1,
        );
    }
}

fn rect(pos: Vec2, size: Vec2) -> MultiPolygon {
    Shape::Rectangle.polygons(pos, size, 0.0)
}

fn fancy_rectangle(
    polygons: &mut FurniturePolygons,
    pos: Vec2,
    size: Vec2,
    rotation: f64,
    material: FurnitureMaterial,
    lighten: f64,
    inset: f64,
) {
    polygons.push((material, Shape::Rectangle.polygons(pos, size, rotation)));
    if size.x > inset * 3.0 && size.y > inset * 3.0 {
        polygons.push((
            material.lighten(lighten),
            Shape::Rectangle.polygons(pos, size - vec2(inset * 2.0, inset * 2.0), rotation),
        ));
    }
}

impl Hash for Furniture {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.furniture_type.hash(state);
        hash_vec2(self.size, state);
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FurnitureMaterial {
    pub material: Material,
    pub tint: Color,
}

impl FurnitureMaterial {
    const fn new(material: Material, tint: Color) -> Self {
        Self { material, tint }
    }

    fn lighten(self, lighten: f64) -> Self {
        Self {
            material: self.material,
            tint: self.tint.lighten(lighten),
        }
    }
}

type FurniturePolygons = Vec<(FurnitureMaterial, MultiPolygon)>;
type FurnitureTriangles = Vec<(FurnitureMaterial, Vec<Triangles>)>;

pub struct FurnitureRender {
    pub hash: u64,
    pub polygons: FurniturePolygons,
    pub triangles: FurnitureTriangles,
    pub shadow_triangles: ShadowsData,
    pub children: Vec<Furniture>,
}
