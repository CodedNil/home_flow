use super::{
    color::Color,
    layout::{GlobalMaterial, Shape, Triangles},
    shape::{get_global_material, polygons_to_shadows, triangulate_polygon, ShadowsData},
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
    pub material: String,
    pub material_children: String,
    pub pos: Vec2,
    pub size: Vec2,
    pub rotation: i32,
    #[serde(skip)]
    pub hover_amount: f64,
    #[serde(skip)]
    #[derivative(Clone(clone_with = "clone_as_none"))]
    pub rendered_data: Option<FurnitureRender>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, EnumIter, Hash)]
pub enum FurnitureType {
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
    AnimatedPiece(AnimatedPieceType),
    Misc(MiscHeight),
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
    Empty,
    Dining,
    Desk,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, EnumIter, Default, Hash)]
pub enum KitchenType {
    #[default]
    Hob,
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
    Cupboard,
    CupboardMid,
    CupboardHigh,
    Drawer,
    DrawerMid,
    DrawerHigh,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, EnumIter, Default, Hash)]
pub enum MiscHeight {
    #[default]
    Low,
    Mid,
    High,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, EnumIter, Default, Hash)]
pub enum AnimatedPieceType {
    #[default]
    Drawer,
    DrawerMid,
    DrawerHigh,
    Door(bool),
    DoorMid(bool),
    DoorHigh(bool),
}

const WOOD: FurnitureMaterial =
    FurnitureMaterial::new(Material::Wood, Color::from_rgb(190, 120, 80));
const CERAMIC: FurnitureMaterial =
    FurnitureMaterial::new(Material::Empty, Color::from_rgb(230, 220, 200));
const METAL_DARK: FurnitureMaterial =
    FurnitureMaterial::new(Material::Empty, Color::from_rgb(80, 80, 80));

impl FurnitureType {
    pub const fn render_order(self) -> u8 {
        match self {
            Self::Storage(StorageType::CupboardHigh | StorageType::DrawerHigh)
            | Self::Misc(MiscHeight::High) => 6,
            Self::Storage(StorageType::CupboardMid | StorageType::DrawerMid)
            | Self::Misc(MiscHeight::Mid) => 4,
            Self::AnimatedPiece(animated_type) => match animated_type {
                AnimatedPieceType::DrawerHigh | AnimatedPieceType::DoorHigh(_) => 5,
                AnimatedPieceType::DrawerMid | AnimatedPieceType::DoorMid(_) => 3,
                AnimatedPieceType::Drawer | AnimatedPieceType::Door(_) => 1,
            },
            Self::Chair(_) => 1,
            Self::Rug(_) => 0,
            _ => 2,
        }
    }

    pub fn height(self) -> f64 {
        (self.render_order() as f64 / 4.0 + 0.5) / 1.5
    }

    pub const fn can_hover(self) -> bool {
        matches!(self, Self::AnimatedPiece(_) | Self::Chair(_))
    }

    pub const fn has_material(self) -> bool {
        matches!(
            self,
            Self::Table(_) | Self::Chair(ChairType::Dining) | Self::Storage(_) | Self::Kitchen(_)
        )
    }

    pub const fn has_children_material(self) -> bool {
        matches!(
            self,
            Self::Table(TableType::Dining | TableType::Desk) | Self::Storage(_)
        )
    }
}

impl Furniture {
    pub fn new(furniture_type: FurnitureType, pos: Vec2, size: Vec2, rotation: i32) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            furniture_type,
            material: "Wood".to_owned(),
            material_children: "Wood".to_owned(),
            pos,
            size,
            rotation,
            hover_amount: 0.0,
            rendered_data: None,
        }
    }

    pub fn named(
        _: &str,
        furniture_type: FurnitureType,
        pos: Vec2,
        size: Vec2,
        rotation: i32,
    ) -> Self {
        Self::new(furniture_type, pos, size, rotation)
    }

    pub fn materials(&self, material: &str) -> Self {
        let mut clone = self.clone();
        clone.material = material.to_owned();
        clone.material_children = material.to_owned();
        clone
    }

    pub fn material(&self, material: &str) -> Self {
        let mut clone = self.clone();
        clone.material = material.to_owned();
        clone
    }

    pub fn default() -> Self {
        Self::new(
            FurnitureType::Chair(ChairType::default()),
            Vec2::ZERO,
            vec2(1.0, 1.0),
            0,
        )
    }

    pub const fn render_order(&self) -> u8 {
        self.furniture_type.render_order()
    }

    pub const fn can_hover(&self) -> bool {
        self.furniture_type.can_hover()
    }

    pub fn contains(&self, point: Vec2) -> bool {
        Shape::Rectangle.contains(point, self.pos, self.size, self.rotation)
    }

    pub fn render(
        &self,
        materials: &[GlobalMaterial],
    ) -> (
        FurniturePolygons,
        FurnitureTriangles,
        ShadowsData,
        Vec<Self>,
    ) {
        let global_material = get_global_material(materials, &self.material);
        let material = FurnitureMaterial::new(global_material.material, global_material.tint);

        let polygons = self.polygons(material);

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

        let has_shadow = !matches!(self.furniture_type, FurnitureType::AnimatedPiece(_));
        let shadows_data = if has_shadow {
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
            polygons_to_shadows(shadow_polys, self.furniture_type.height())
        } else {
            (Color::TRANSPARENT, Vec::new())
        };

        let children = self.render_children(materials);

        (polygons, triangles, shadows_data, children)
    }

    fn polygons(&self, material: FurnitureMaterial) -> FurniturePolygons {
        let mut polygons = Vec::new();
        match self.furniture_type {
            FurnitureType::Chair(sub_type) => self.chair_render(&mut polygons, material, sub_type),
            FurnitureType::Table(_) => self.table_render(&mut polygons, material),
            FurnitureType::Bed(color) => self.bed_render(&mut polygons, color),
            FurnitureType::Storage(_) => self.storage_render(&mut polygons, material),
            FurnitureType::Rug(color) => self.rug_render(&mut polygons, color),
            FurnitureType::Kitchen(sub_type) => self.kitchen_render(&mut polygons, sub_type),
            FurnitureType::Bathroom(sub_type) => self.bathroom_render(&mut polygons, sub_type),
            FurnitureType::Boiler => polygons.push((METAL_DARK, self.full_shape())),
            FurnitureType::Radiator => self.radiator_render(&mut polygons),
            FurnitureType::Display => self.display_render(&mut polygons),
            FurnitureType::AnimatedPiece(sub_type) => {
                self.animated_render(&mut polygons, material, sub_type);
            }
            FurnitureType::Misc(_) => polygons.push((material, self.full_shape())),
        }
        polygons
    }

    fn render_children(&self, materials: &[GlobalMaterial]) -> Vec<Self> {
        let mut children = Vec::new();
        match self.furniture_type {
            FurnitureType::Table(sub_type) => self.table_children(&mut children, sub_type),
            FurnitureType::Storage(sub_type) => {
                self.storage_children(&mut children, sub_type);
            }
            _ => {}
        }
        for child in &mut children {
            let (polygons, triangles, shadow_triangles, children) = child.render(materials);
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

    fn table_children(&self, children: &mut Vec<Self>, sub_type: TableType) {
        let chair_size = vec2(0.5, 0.5);
        let chair_push = 0.1;

        let mut add_chair = |x: f64, y: f64, rotation: i32| {
            children.push(
                Self::new(
                    FurnitureType::Chair(match sub_type {
                        TableType::Desk => ChairType::Office,
                        _ => ChairType::Dining,
                    }),
                    vec2(x, y),
                    chair_size,
                    rotation,
                )
                .material(&self.material_children),
            );
        };

        match sub_type {
            TableType::Desk => {
                add_chair(0.0, self.size.y * 0.5 + chair_push, 0);
            }
            TableType::Dining => {
                let spacing = 0.1;

                let chairs_wide = (self.size.x / (chair_size.x + spacing)).floor() as usize;
                (0..chairs_wide).for_each(|i| {
                    let x_pos =
                        (i as f64 - (chairs_wide - 1) as f64 * 0.5) * (chair_size.x + spacing);
                    add_chair(x_pos, self.size.y * 0.5 + chair_push, 0);
                    add_chair(x_pos, -self.size.y * 0.5 - chair_push, 180);
                });

                let chairs_high = (self.size.y / (chair_size.y + spacing)).floor() as usize;
                (0..chairs_high).for_each(|i| {
                    let y_pos =
                        (i as f64 - (chairs_high - 1) as f64 * 0.5) * (chair_size.y + spacing);
                    add_chair(self.size.x * 0.5 + chair_push, y_pos, 90);
                    add_chair(-self.size.x * 0.5 - chair_push, y_pos, -90);
                });
            }
            TableType::Empty => {}
        }
    }

    fn storage_children(&self, children: &mut Vec<Self>, sub_type: StorageType) {
        let num_drawers = ((self.size.x - 0.05) / 0.5).floor().max(1.0) as usize;
        let drawer_width = self.size.x / num_drawers as f64;
        for i in 0..num_drawers {
            let x_pos = (i as f64 - (num_drawers - 1) as f64 * 0.5) * drawer_width;
            let side = i % 2 == 0;
            children.push(
                Self::new(
                    FurnitureType::AnimatedPiece(match sub_type {
                        StorageType::Drawer => AnimatedPieceType::Drawer,
                        StorageType::DrawerMid => AnimatedPieceType::DrawerMid,
                        StorageType::DrawerHigh => AnimatedPieceType::DrawerHigh,
                        StorageType::Cupboard => AnimatedPieceType::Door(side),
                        StorageType::CupboardMid => AnimatedPieceType::DoorMid(side),
                        StorageType::CupboardHigh => AnimatedPieceType::DoorHigh(side),
                    }),
                    vec2(x_pos, 0.0),
                    vec2(drawer_width - 0.025, self.size.y),
                    0,
                )
                .material(&self.material_children),
            );
        }
    }

    fn full_shape(&self) -> MultiPolygon {
        rect(Vec2::ZERO, self.size)
    }

    fn chair_render(
        &self,
        polygons: &mut FurniturePolygons,
        material: FurnitureMaterial,
        sub_type: ChairType,
    ) {
        let material = match sub_type {
            ChairType::Dining => material,
            ChairType::Office => {
                FurnitureMaterial::new(Material::Empty, Color::from_rgb(40, 40, 40))
            }
            ChairType::Sofa(color) => FurnitureMaterial::new(Material::Fabric, color),
        };

        polygons.push((material, self.full_shape()));
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

    fn table_render(&self, polygons: &mut FurniturePolygons, material: FurnitureMaterial) {
        fancy_rectangle(polygons, Vec2::ZERO, self.size, 0, material, 0.04, 0.1);
    }

    fn kitchen_render(&self, polygons: &mut FurniturePolygons, sub_type: KitchenType) {
        match sub_type {
            KitchenType::Hob => {
                polygons.push((
                    FurnitureMaterial::new(Material::Empty, Color::from_rgb(80, 80, 80)),
                    self.full_shape(),
                ));
                // Render 4 black circles
                let black = FurnitureMaterial::new(Material::Empty, Color::from_rgb(40, 40, 40));
                let circle_size = self.size.min_element() * 0.3;
                for x in 0..2 {
                    for y in 0..2 {
                        let x_pos = (x as f64 - 0.5) * self.size.x * 0.5;
                        let y_pos = (y as f64 - 0.5) * self.size.y * 0.5;
                        polygons.push((
                            black,
                            Shape::Circle.polygons(vec2(x_pos, y_pos), Vec2::splat(circle_size), 0),
                        ));
                    }
                }
            }
            KitchenType::Sink => {
                fancy_rectangle(polygons, Vec2::ZERO, self.size, 0, METAL_DARK, 0.1, 0.05);
            }
        }
    }

    fn bathroom_render(&self, polygons: &mut FurniturePolygons, sub_type: BathroomType) {
        let ceramic_light = 0.06;
        match sub_type {
            BathroomType::Bath => {
                polygons.push((CERAMIC, self.full_shape()));
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
                    0,
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
                0,
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
            0,
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

    fn storage_render(&self, polygons: &mut FurniturePolygons, material: FurnitureMaterial) {
        polygons.push((material, self.full_shape()));
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
            0,
            FurnitureMaterial::new(Material::Carpet, color),
            -0.05,
            0.1,
        );
    }

    fn animated_render(
        &self,
        polygons: &mut FurniturePolygons,
        material: FurnitureMaterial,
        sub_type: AnimatedPieceType,
    ) {
        match sub_type {
            AnimatedPieceType::Drawer
            | AnimatedPieceType::DrawerMid
            | AnimatedPieceType::DrawerHigh => {
                fancy_rectangle(polygons, Vec2::ZERO, self.size, 0, material, 0.1, 0.05);
            }
            AnimatedPieceType::Door(_)
            | AnimatedPieceType::DoorMid(_)
            | AnimatedPieceType::DoorHigh(_) => {
                let depth = 0.05;
                polygons.push((
                    material.lighten(0.1),
                    Shape::Rectangle.polygons(
                        vec2(0.0, -self.size.y * 0.5 + depth * 0.5),
                        vec2(self.size.x, depth),
                        0,
                    ),
                ));
            }
        }
    }
}

fn rect(pos: Vec2, size: Vec2) -> MultiPolygon {
    Shape::Rectangle.polygons(pos, size, 0)
}

fn fancy_rectangle(
    polygons: &mut FurniturePolygons,
    pos: Vec2,
    size: Vec2,
    rotation: i32,
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
        self.material.hash(state);
        self.material_children.hash(state);
        hash_vec2(self.size, state);
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default, Hash)]
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
