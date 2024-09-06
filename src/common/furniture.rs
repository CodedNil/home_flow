use crate::common::{
    color::Color,
    layout::{DataPoint, GlobalMaterial, Shape, Triangles},
    shape::{polygons_to_shadows, triangulate_polygon, ShadowsData},
    utils::{hash_vec2, Material},
};
use geo_types::MultiPolygon;
use glam::{dvec2 as vec2, DVec2 as Vec2};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
};
use strum_macros::{Display, EnumIter};
use uuid::Uuid;

nestify::nest! {
    #[derive(Serialize, Deserialize, Clone)]*
    pub struct Furniture {
        pub id: Uuid,
        pub name: String,

        #>[derive(Copy, PartialEq, Eq, Display, EnumIter, Hash, Default)]*
        pub furniture_type: pub enum FurnitureType {
            Chair(pub enum ChairType {
                #[default]
                Dining,
                Office,
                Sofa(Color),
            }),
            Table(pub enum TableType {
                #[default]
                Empty,
                Dining,
                Desk,
            }),
            Kitchen(pub enum KitchenType {
                #[default]
                Hob,
                Sink,
            }),
            Bathroom(pub enum BathroomType {
                #[default]
                Toilet,
                Shower,
                Bath,
                Sink,
            }),
            Bed(Color),
            Storage(pub enum StorageType {
                #[default]
                Cupboard,
                Drawer,
            }),
            Rug(Color),
            Electronic(pub enum ElectronicType {
                #[default]
                Display,
                Computer,
            }),
            Sensor(pub enum SensorType {
                #[default]
                UltimateSensorMini, // https://ultimatesensor.nl/en/mini
                PresenceBoolean, // If the boolean is true, a presence point is added
            }),
            Radiator,
            #[default]
            Misc,
            AnimatedPiece(
                pub enum AnimatedPieceType {
                    #[default]
                    Drawer,
                    Door(bool),
                }),
        },

        #>[derive(Copy, PartialEq, Eq, Display, EnumIter, Hash, Default)]*
        pub render_order: pub enum RenderOrder {
            #[default]
            Default,
            Floor,
            Low,
            Mid,
            High
        },

        pub material: String,
        pub material_children: String,

        pub pos: Vec2,
        pub size: Vec2,
        pub rotation: i32,

        pub power_draw_entity: String,
        pub misc_sensors: Vec<String>,
        pub misc_data: HashMap<String, DataPoint>,

        #[serde(skip)]
        pub hover_amount: f64,
        #[serde(skip)]
        pub rendered_data: Option<FurnRender>,
        #[serde(skip)]
        pub hass_data: HashMap<String, String>,
    }
}

const WOOD: FurnMaterial = FurnMaterial::new(Material::Wood, Color::from_rgb(190, 120, 80));
const CERAMIC: FurnMaterial = FurnMaterial::new(Material::Empty, Color::from_rgb(230, 220, 200));
const METAL_DARK: FurnMaterial = FurnMaterial::new(Material::Empty, Color::from_rgb(80, 80, 80));

impl Furniture {
    pub fn new(
        name: &str,
        furniture_type: FurnitureType,
        pos: Vec2,
        size: Vec2,
        rotation: i32,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.to_owned(),
            furniture_type,
            render_order: RenderOrder::Default,
            material: "Wood".to_owned(),
            material_children: "Wood".to_owned(),
            pos,
            size,
            rotation,
            power_draw_entity: String::new(),
            misc_sensors: Vec::new(),
            misc_data: HashMap::new(),
            hover_amount: 0.0,
            rendered_data: None,
            hass_data: HashMap::new(),
        }
    }

    pub fn new_materials(
        name: &str,
        furniture_type: FurnitureType,
        pos: Vec2,
        size: Vec2,
        rotation: i32,
        material: &str,
    ) -> Self {
        Self::new(name, furniture_type, pos, size, rotation).materials(material)
    }

    pub const fn render_order(mut self, render_order: RenderOrder) -> Self {
        self.render_order = render_order;
        self
    }

    pub fn materials(mut self, material: &str) -> Self {
        material.clone_into(&mut self.material);
        material.clone_into(&mut self.material_children);
        self
    }

    pub fn material(mut self, material: &str) -> Self {
        material.clone_into(&mut self.material);
        self
    }

    pub fn material_children(mut self, material: &str) -> Self {
        material.clone_into(&mut self.material_children);
        self
    }

    pub fn power_draw_entity(mut self, entity: &str) -> Self {
        entity.clone_into(&mut self.power_draw_entity);
        self
    }

    pub fn add_sensors(mut self, entities: &[&str]) -> Self {
        self.misc_sensors
            .extend(entities.iter().map(std::string::ToString::to_string));
        self
    }

    pub fn add_data(mut self, data: Vec<(&str, DataPoint)>) -> Self {
        self.misc_data
            .extend(data.into_iter().map(|(key, value)| (key.to_owned(), value)));
        self
    }

    pub fn default() -> Self {
        Self::new(
            "New Furniture",
            FurnitureType::Chair(ChairType::default()),
            Vec2::ZERO,
            vec2(1.0, 1.0),
            0,
        )
    }

    pub const fn get_render_order(&self) -> u8 {
        let render_order = match self.render_order {
            RenderOrder::Default => match self.furniture_type {
                FurnitureType::Chair(_) => RenderOrder::Low,
                FurnitureType::Rug(_) => RenderOrder::Floor,
                _ => RenderOrder::Mid,
            },
            _ => self.render_order,
        };
        let mut order = match render_order {
            RenderOrder::High => 6,
            RenderOrder::Mid => 4,
            RenderOrder::Low => 2,
            RenderOrder::Floor => 0,
            RenderOrder::Default => panic!("Invalid render order"),
        };

        if matches!(
            self.furniture_type,
            FurnitureType::AnimatedPiece(_) | FurnitureType::Chair(_)
        ) {
            order -= 1;
        }

        order
    }

    pub fn wanted_sensors(&self) -> Vec<String> {
        let mut sensors = Vec::new();
        if !self.power_draw_entity.is_empty() {
            sensors.push(self.power_draw_entity.clone());
        }
        sensors.extend(self.misc_sensors.iter().cloned());
        sensors
    }

    pub fn height_shadow(&self) -> f64 {
        ((f64::from(self.get_render_order()) / 6.0) + 0.5) / 1.5
    }

    pub const fn can_hover(&self) -> bool {
        matches!(
            self.furniture_type,
            FurnitureType::AnimatedPiece(_) | FurnitureType::Chair(_)
        )
    }

    pub const fn has_material(&self) -> bool {
        matches!(
            self.furniture_type,
            FurnitureType::Table(_)
                | FurnitureType::Chair(ChairType::Dining)
                | FurnitureType::Storage(_)
                | FurnitureType::Misc
        )
    }

    pub const fn has_children_material(&self) -> bool {
        matches!(
            self.furniture_type,
            FurnitureType::Table(TableType::Dining) | FurnitureType::Storage(_)
        )
    }

    pub fn contains(&self, room_pos: Vec2, point: Vec2) -> bool {
        Shape::Rectangle.contains(point, room_pos + self.pos, self.size, self.rotation)
    }

    pub fn render(
        &self,
        primary_material: &GlobalMaterial,
        child_material: &GlobalMaterial,
    ) -> FurnRender {
        let material = FurnMaterial::new(primary_material.material, primary_material.tint);

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

        let has_shadow = !matches!(
            self.furniture_type,
            FurnitureType::AnimatedPiece(_) | FurnitureType::Sensor(_)
        );
        let shadow_triangles = if has_shadow {
            // Use simple shape for shadow unless complex is needed
            let use_simple = match self.furniture_type {
                FurnitureType::Bed(_) => false,
                FurnitureType::Bathroom(sub_type) => {
                    !matches!(sub_type, BathroomType::Toilet | BathroomType::Sink)
                }
                _ => true,
            };
            if use_simple {
                polygons_to_shadows(vec![&self.full_shape()], self.height_shadow())
            } else {
                let shadow_polys = polygons.iter().map(|(_, p)| p).collect::<Vec<_>>();
                polygons_to_shadows(shadow_polys, self.height_shadow())
            }
        } else {
            (Color::TRANSPARENT, Vec::new())
        };

        let children = self.render_children(child_material);

        FurnRender {
            hash: 0,
            triangles,
            shadow_triangles,
            children,
        }
    }

    fn polygons(&self, material: FurnMaterial) -> FurniturePolygons {
        match self.furniture_type {
            FurnitureType::Chair(sub_type) => self.chair_render(material, sub_type),
            FurnitureType::Table(_) => self.table_render(material),
            FurnitureType::Bed(color) => self.bed_render(color),
            FurnitureType::Storage(_) => self.storage_render(material),
            FurnitureType::Rug(color) => self.rug_render(color),
            FurnitureType::Kitchen(sub_type) => self.kitchen_render(sub_type),
            FurnitureType::Bathroom(sub_type) => self.bathroom_render(sub_type),
            FurnitureType::Radiator => self.radiator_render(),
            FurnitureType::Electronic(sub_type) => self.electronic_render(sub_type),
            FurnitureType::Sensor(_) => vec![],
            FurnitureType::AnimatedPiece(sub_type) => self.animated_render(material, sub_type),
            FurnitureType::Misc => vec![(material, self.full_shape())],
        }
    }

    fn render_children(&self, material: &GlobalMaterial) -> Vec<Self> {
        let mut children = match self.furniture_type {
            FurnitureType::Table(sub_type) => self.table_children(sub_type),
            FurnitureType::Storage(sub_type) => self.storage_children(sub_type),
            _ => Vec::new(),
        };
        for child in &mut children {
            child.rendered_data = Some(child.render(material, material));
        }
        children
    }

    fn table_children(&self, sub_type: TableType) -> Vec<Self> {
        let mut children = Vec::new();
        let chair_size = vec2(0.5, 0.5);
        let chair_push = 0.1;

        let mut add_chair = |x: f64, y: f64, rotation: i32| {
            children.push(
                Self::new(
                    "Child Chair",
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
        children
    }

    fn storage_children(&self, sub_type: StorageType) -> Vec<Self> {
        let mut children = Vec::new();
        let num_drawers = ((self.size.x - 0.05) / 0.5).floor().max(1.0) as usize;
        let drawer_width = self.size.x / num_drawers as f64;
        for i in 0..num_drawers {
            let x_pos = (i as f64 - (num_drawers - 1) as f64 * 0.5) * drawer_width;
            let side = i % 2 == 0;
            children.push(
                Self::new_materials(
                    "Child Drawer",
                    FurnitureType::AnimatedPiece(match sub_type {
                        StorageType::Drawer => AnimatedPieceType::Drawer,
                        StorageType::Cupboard => AnimatedPieceType::Door(side),
                    }),
                    vec2(x_pos, 0.0),
                    vec2(drawer_width - 0.025, self.size.y),
                    0,
                    &self.material_children,
                )
                .render_order(self.render_order),
            );
        }
        children
    }

    fn full_shape(&self) -> MultiPolygon {
        rect(Vec2::ZERO, self.size)
    }

    fn chair_render(&self, material: FurnMaterial, sub_type: ChairType) -> FurniturePolygons {
        let mut polygons = Vec::new();
        let material = match sub_type {
            ChairType::Dining => material,
            ChairType::Office => FurnMaterial::new(Material::Empty, Color::from_rgb(40, 40, 40)),
            ChairType::Sofa(color) => FurnMaterial::new(Material::Fabric, color),
        };

        polygons.push((material, self.full_shape()));
        let inset = 0.1;
        if self.size.x > inset * 3.0 && self.size.y > inset * 3.0 {
            polygons.push((
                material.lighten(0.1).saturate(-0.1),
                rect(
                    vec2(0.0, -inset * 0.5),
                    self.size - vec2(inset * 2.0, inset),
                ),
            ));
        }
        polygons
    }

    fn table_render(&self, material: FurnMaterial) -> FurniturePolygons {
        fancy_rectangle(Vec2::ZERO, self.size, material, 0.04, 0.1)
    }

    fn kitchen_render(&self, sub_type: KitchenType) -> FurniturePolygons {
        match sub_type {
            KitchenType::Hob => {
                let mut polygons = Vec::with_capacity(5);
                polygons.push((
                    FurnMaterial::new(Material::Empty, Color::from_rgb(80, 80, 80)),
                    self.full_shape(),
                ));
                // Render 4 black circles
                let black = FurnMaterial::new(Material::Empty, Color::from_rgb(40, 40, 40));
                let circle_size = self.size.min_element() * 0.3;
                for x in 0..2 {
                    for y in 0..2 {
                        let x_pos = (f64::from(x) - 0.5) * self.size.x * 0.5;
                        let y_pos = (f64::from(y) - 0.5) * self.size.y * 0.5;
                        polygons.push((
                            black,
                            Shape::Circle.polygons(vec2(x_pos, y_pos), Vec2::splat(circle_size), 0),
                        ));
                    }
                }
                polygons
            }
            KitchenType::Sink => fancy_rectangle(Vec2::ZERO, self.size, METAL_DARK, 0.1, 0.05),
        }
    }

    fn bathroom_render(&self, sub_type: BathroomType) -> FurniturePolygons {
        let mut polygons = Vec::new();
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
                polygons
            }
            BathroomType::Shower => {
                polygons.extend(fancy_rectangle(
                    Vec2::ZERO,
                    self.size,
                    CERAMIC,
                    ceramic_light,
                    0.1,
                ));
                // Tap
                polygons.push((
                    METAL_DARK,
                    rect(vec2(0.0, self.size.y * 0.5 - 0.05), vec2(0.2, 0.1)),
                ));
                polygons
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
                polygons
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
                polygons
            }
        }
    }

    fn bed_render(&self, color: Color) -> FurniturePolygons {
        let mut polygons = Vec::new();
        let sheet_color = Color::from_rgb(250, 230, 210);
        let pillow_color = Color::from_rgb(255, 255, 255);

        // Add sheets
        polygons.push((
            FurnMaterial::new(Material::Empty, sheet_color),
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
            polygons.extend(fancy_rectangle(
                pillow_pos,
                vec2(pillow_width, pillow_height),
                FurnMaterial::new(Material::Empty, pillow_color),
                -0.015,
                0.03,
            ));
        }

        // Add covers
        let covers_size = (self.size.y - pillow_height - pillow_spacing * 2.0) / self.size.y;
        polygons.extend(fancy_rectangle(
            -vec2(0.0, self.size.y * (1.0 - covers_size) / 2.0),
            vec2(self.size.x, self.size.y * covers_size),
            FurnMaterial::new(Material::Fabric, color),
            -0.025,
            0.05,
        ));

        // Add backboard
        let backboard_polygon = rect(
            vec2(0.0, self.size.y * 0.5 + 0.025),
            vec2(self.size.x + 0.05, 0.05),
        );
        polygons.push((WOOD, backboard_polygon));
        polygons
    }

    fn storage_render(&self, material: FurnMaterial) -> FurniturePolygons {
        vec![(material, self.full_shape())]
    }

    fn radiator_render(&self) -> FurniturePolygons {
        let mut polygons = Vec::new();
        polygons.push((
            FurnMaterial::new(Material::Empty, Color::from_rgb(255, 255, 255)),
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
                    FurnMaterial::new(Material::Empty, Color::from_rgb(200, 200, 200)),
                    rect(vec2(x_pos, 0.0), vec2(adjusted_stripe_width, self.size.y)),
                ));
            }
        }
        polygons
    }

    fn electronic_render(&self, sub_type: ElectronicType) -> FurniturePolygons {
        match sub_type {
            ElectronicType::Display => {
                vec![
                    (
                        METAL_DARK,
                        rect(
                            vec2(0.0, -self.size.y * 0.25),
                            vec2(self.size.x, self.size.y * 0.5),
                        ),
                    ),
                    (
                        FurnMaterial::new(Material::Empty, Color::from_rgb(50, 150, 255)),
                        rect(
                            vec2(0.0, self.size.y * 0.25),
                            vec2(self.size.x, self.size.y * 0.5),
                        ),
                    ),
                ]
            }
            ElectronicType::Computer => {
                vec![(METAL_DARK, self.full_shape())]
            }
        }
    }

    fn rug_render(&self, color: Color) -> FurniturePolygons {
        fancy_rectangle(
            Vec2::ZERO,
            self.size,
            FurnMaterial::new(Material::Carpet, color),
            -0.05,
            0.1,
        )
    }

    fn animated_render(
        &self,
        material: FurnMaterial,
        sub_type: AnimatedPieceType,
    ) -> FurniturePolygons {
        match sub_type {
            AnimatedPieceType::Drawer => {
                fancy_rectangle(Vec2::ZERO, self.size, material, 0.1, 0.05)
            }
            AnimatedPieceType::Door(_) => {
                let depth = 0.05;
                vec![(
                    material.lighten(0.1),
                    rect(
                        vec2(0.0, -self.size.y * 0.5 + depth * 0.5),
                        vec2(self.size.x, depth),
                    ),
                )]
            }
        }
    }
}

fn rect(pos: Vec2, size: Vec2) -> MultiPolygon {
    Shape::Rectangle.polygons(pos, size, 0)
}

fn fancy_rectangle(
    pos: Vec2,
    size: Vec2,
    material: FurnMaterial,
    lighten: f64,
    inset: f64,
) -> FurniturePolygons {
    if size.x > inset * 3.0 && size.y > inset * 3.0 {
        vec![
            (material, rect(pos, size)),
            (
                material.lighten(lighten),
                rect(pos, size - vec2(inset * 2.0, inset * 2.0)),
            ),
        ]
    } else {
        vec![(material, rect(pos, size))]
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
pub struct FurnMaterial {
    pub material: Material,
    pub tint: Color,
}

impl FurnMaterial {
    const fn new(material: Material, tint: Color) -> Self {
        Self { material, tint }
    }

    fn lighten(self, lighten: f64) -> Self {
        Self {
            material: self.material,
            tint: self.tint.lighten(lighten),
        }
    }

    fn saturate(self, saturate: f64) -> Self {
        Self {
            material: self.material,
            tint: self.tint.saturate(saturate),
        }
    }
}

type FurniturePolygons = Vec<(FurnMaterial, MultiPolygon)>;
type FurnitureTriangles = Vec<(FurnMaterial, Vec<Triangles>)>;

#[derive(Clone)]
pub struct FurnRender {
    pub hash: u64,
    pub triangles: FurnitureTriangles,
    pub shadow_triangles: ShadowsData,
    pub children: Vec<Furniture>,
}
