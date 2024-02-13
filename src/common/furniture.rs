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
    Wardrobe,
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

const WOOD: FurnitureMaterial =
    FurnitureMaterial::new(Material::Wood, Color::from_rgb(190, 120, 80));

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

        let shadows_data = polygons_to_shadows(polygons.iter().map(|(_, p)| p).collect::<Vec<_>>());

        let children = self.render_children();

        (polygons, triangles, shadows_data, children)
    }

    fn polygons(&self) -> FurniturePolygons {
        let mut polygons = Vec::new();
        match self.furniture_type {
            FurnitureType::Chair(sub_type) => self.chair_render(&mut polygons, sub_type),
            FurnitureType::Table(sub_type) => self.table_render(&mut polygons, sub_type),
            FurnitureType::Bed(color) => self.bed_render(&mut polygons, color),
            FurnitureType::Wardrobe => self.wardrobe_render(&mut polygons),
            FurnitureType::Rug(color) => self.rug_render(&mut polygons, color),
            FurnitureType::Kitchen(sub_type) => self.kitchen_render(&mut polygons, sub_type),
            _ => polygons.push((
                FurnitureMaterial::new(Material::Empty, Color::from_rgb(255, 0, 0)),
                self.full_shape(),
            )),
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
        Shape::Rectangle.polygons(Vec2::ZERO, self.size, 0.0)
    }

    fn chair_render(&self, polygons: &mut FurniturePolygons, sub_type: ChairType) {
        let material = match sub_type {
            ChairType::Dining => WOOD,
            ChairType::Office => {
                FurnitureMaterial::new(Material::Empty, Color::from_rgb(40, 40, 40))
            }
            ChairType::Sofa(color) => FurnitureMaterial::new(Material::Fabric, color),
        };

        polygons.push((
            material,
            Shape::Rectangle.polygons(Vec2::ZERO, self.size, 0.0),
        ));
        let inset = 0.1;
        if self.size.x > inset * 3.0 && self.size.y > inset * 3.0 {
            polygons.push((
                material.lighten(0.1),
                Shape::Rectangle.polygons(
                    vec2(0.0, -inset * 0.5),
                    self.size - vec2(inset * 2.0, inset),
                    0.0,
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
            Material::Wood,
            color,
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
                    FurnitureMaterial::new(Material::Granite, Color::from_rgb(32, 32, 32)),
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
                Material::Empty,
                pillow_color,
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
            Material::Fabric,
            color,
            -0.025,
            0.05,
        );

        // Add backboard
        let backboard_polygon = Shape::Rectangle.polygons(
            vec2(0.0, self.size.y * 0.5 + 0.025),
            vec2(self.size.x + 0.05, 0.05),
            0.0,
        );
        polygons.push((WOOD, backboard_polygon));
    }

    fn wardrobe_render(&self, polygons: &mut FurniturePolygons) {
        polygons.push((WOOD, self.full_shape()));
    }

    fn rug_render(&self, polygons: &mut FurniturePolygons, color: Color) {
        fancy_rectangle(
            polygons,
            Vec2::ZERO,
            self.size,
            0.0,
            Material::Carpet,
            color,
            -0.05,
            0.1,
        );
    }
}

fn fancy_rectangle(
    polygons: &mut FurniturePolygons,
    pos: Vec2,
    size: Vec2,
    rotation: f64,
    material: Material,
    color: Color,
    lighten: f64,
    inset: f64,
) {
    polygons.push((
        FurnitureMaterial::new(material, color),
        Shape::Rectangle.polygons(pos, size, rotation),
    ));
    if size.x > inset * 3.0 && size.y > inset * 3.0 {
        polygons.push((
            FurnitureMaterial::new(material, color.lighten(lighten)),
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
