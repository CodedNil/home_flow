use super::{
    color::Color,
    furniture::{
        BathroomType, DataPoint, ElectronicType, Furniture, FurnitureType, KitchenType,
        RenderOrder, StorageType, TableType,
    },
    layout::{GlobalMaterial, Home, LightType, Outline, Room, Sensor, LAYOUT_VERSION},
    utils::Material,
};
use glam::{dvec2 as vec2, DVec2 as Vec2};

pub fn default() -> Home {
    Home {
        version: LAYOUT_VERSION.to_string(),
        materials: vec![
            GlobalMaterial::new("Carpet", Material::Carpet, Color::from_rgb(240, 230, 210)),
            GlobalMaterial::new("Wood", Material::Wood, Color::from_rgb(190, 120, 80)),
            GlobalMaterial::new("WoodDark", Material::Wood, Color::from_rgb(60, 60, 60)),
            GlobalMaterial::new("Marble", Material::Marble, Color::from_rgb(255, 255, 255)),
            GlobalMaterial::new("Granite", Material::Granite, Color::from_rgb(50, 50, 50)),
            GlobalMaterial::new("Ceramic", Material::Empty, Color::from_rgb(230, 220, 200)),
            GlobalMaterial::new("MetalDark", Material::Empty, Color::from_rgb(80, 80, 80)),
            GlobalMaterial::new(
                "MarbleTiles",
                Material::Marble,
                Color::from_rgb(255, 250, 230),
            )
            .tiles(0.4, 0.02, Color::from_rgba(80, 80, 80, 100)),
            GlobalMaterial::new(
                "GraniteTiles",
                Material::Granite,
                Color::from_rgb(40, 40, 40),
            )
            .tiles(0.4, 0.02, Color::from_rgba(60, 60, 60, 200)),
        ],
        rooms: vec![
            Room::new("Hall", vec2(0.5, 0.5), vec2(6.2, 1.10), "Carpet")
                .no_wall_left()
                .no_wall_right()
                .no_wall_bottom()
                .add_material(vec2(-0.85, 1.55), vec2(1.1, 2.0), "Wood")
                .door(vec2(-0.85, 2.55), 0)
                .lights_grid_offset("Hall Downlights", 3, 1, vec2(3.0, 1.75), vec2(0.8, 0.0))
                .light("Hall Downlights", -0.85, 1.55)
                .furniture(vec![Furniture::new(
                    "Hall Radiator",
                    FurnitureType::Radiator,
                    vec2(0.45, 0.45),
                    vec2(1.4, 0.1),
                    0,
                )]),
            Room::new("Lounge", vec2(-2.75, -1.4), vec2(6.1, 2.7), "Carpet")
                .no_wall_top()
                .window_width(vec2(-1.15, -1.35), 0, 1.4)
                .window(vec2(1.75, -1.35), 0)
                .lights_grid_offset("Lounge Downlights", 4, 2, vec2(2.0, 1.25), Vec2::ZERO)
                .furniture(vec![
                    Furniture::new(
                        "Lounge Radiator",
                        FurnitureType::Radiator,
                        vec2(-1.15, -1.25),
                        vec2(1.4, 0.1),
                        0,
                    ),
                    Furniture::new(
                        "Ultimate Mini",
                        FurnitureType::Electronic(ElectronicType::UltimateSensorMini),
                        vec2(-2.975, -1.125),
                        vec2(0.1, 0.1),
                        45,
                    )
                    .add_sensors(vec![
                        "ultimatesensor_mini_target_1_x",
                        "ultimatesensor_mini_target_1_y",
                        "ultimatesensor_mini_target_2_x",
                        "ultimatesensor_mini_target_2_y",
                        "ultimatesensor_mini_target_3_x",
                        "ultimatesensor_mini_target_3_y",
                    ])
                    .add_datas(vec![
                        ("calib_point1", DataPoint::Vec2(vec2(0.0, 0.0))),
                        ("calib_world1", DataPoint::Vec2(vec2(-5.8, -2.5))),
                        ("calib_point2", DataPoint::Vec2(vec2(2950.0, 3800.0))),
                        ("calib_world2", DataPoint::Vec2(vec2(-0.8, -1.8))),
                        ("calib_point3", DataPoint::Vec2(vec2(-3000.0, 3000.0))),
                        ("calib_world3", DataPoint::Vec2(vec2(-5.1, 2.3))),
                    ]),
                    Furniture::new(
                        "Desk",
                        FurnitureType::Table(TableType::Desk),
                        vec2(2.55, -0.475),
                        vec2(1.6, 0.8),
                        -90,
                    )
                    .material("WoodDark"),
                    Furniture::new(
                        "Desk TV",
                        FurnitureType::Electronic(ElectronicType::Display),
                        vec2(2.975, -0.25),
                        vec2(1.0, 0.05),
                        -90,
                    )
                    .power_draw_entity("living_tv_current_consumption"),
                    Furniture::new(
                        "Desktop",
                        FurnitureType::Electronic(ElectronicType::Computer),
                        vec2(2.625, -1.1),
                        vec2(0.5, 0.3),
                        0,
                    )
                    .power_draw_entity("desktop_current_consumption"),
                    Furniture::new(
                        "Router",
                        FurnitureType::Electronic(ElectronicType::Computer),
                        vec2(2.325, -1.1),
                        vec2(0.07, 0.3),
                        0,
                    ),
                    Furniture::new(
                        "Raspberry Pi",
                        FurnitureType::Electronic(ElectronicType::Computer),
                        vec2(2.225, -1.2),
                        vec2(0.07, 0.1),
                        0,
                    ),
                    Furniture::new(
                        "Desk Fan",
                        FurnitureType::Electronic(ElectronicType::Computer),
                        vec2(2.55, 0.175),
                        vec2(0.2, 0.2),
                        45,
                    )
                    .power_draw_entity("desk_fan_current_consumption"),
                    Furniture::new_ordered(
                        "Desk Mat",
                        FurnitureType::Rug(Color::from_rgb(88, 155, 238)),
                        RenderOrder::Mid,
                        vec2(2.425, -0.45),
                        vec2(0.375, 0.85),
                        0,
                    )
                    .material("Wood"),
                    Furniture::new(
                        "Keyboard",
                        FurnitureType::Misc,
                        vec2(2.5, -0.375),
                        vec2(0.175, 0.35),
                        0,
                    )
                    .material("Marble"),
                    Furniture::new(
                        "Mini Display",
                        FurnitureType::Electronic(ElectronicType::Display),
                        vec2(2.7, -0.85),
                        vec2(0.2, 0.025),
                        300,
                    )
                    .material("Wood"),
                    Furniture::new(
                        "Mini Control",
                        FurnitureType::Misc,
                        vec2(2.55, -0.8),
                        vec2(0.1, 0.15),
                        15,
                    )
                    .material("Marble"),
                    Furniture::new(
                        "Mouse",
                        FurnitureType::Misc,
                        vec2(2.5, -0.625),
                        vec2(0.1, 0.07),
                        0,
                    )
                    .material("MetalDark"),
                ])
                .add_sensors(&[
                    Sensor::new("ultimatesensor_mini_scd41_temperature", "TMP", "°C"),
                    Sensor::new("ultimatesensor_mini_scd41_humidity", "HUM", "%"),
                    Sensor::new("ultimatesensor_mini_scd41_co2", "CO2", "ppm"),
                    Sensor::new("ultimatesensor_mini_voc_index", "VOC", "idx"),
                ]),
            Room::new("Kitchen", vec2(-4.2, 1.5), vec2(3.2, 3.1), "MarbleTiles")
                .no_wall_right()
                .no_wall_bottom()
                .add(vec2(1.65, 0.55), vec2(0.3, 2.0))
                .window_width(vec2(0.3, 1.55), 0, 1.4)
                .lights_grid_offset("Kitchen Downlights", 2, 1, vec2(2.0, 2.0), vec2(0.1, 0.0))
                .outline(Outline::new(0.05, Color::from_rgb(200, 170, 150)))
                .furniture_bulk_material(
                    "Granite Counters",
                    FurnitureType::Misc,
                    RenderOrder::Mid,
                    "Granite",
                    vec![
                        (vec2(-1.275, 1.225), vec2(0.55, 0.55), 0),
                        (vec2(1.475, 1.225), vec2(0.55, 0.55), 0),
                    ],
                )
                .furniture_bulk_material(
                    "Drawers",
                    FurnitureType::Storage(StorageType::Drawer),
                    RenderOrder::Mid,
                    "Granite",
                    vec![
                        (vec2(-1.275, -0.15), vec2(2.2, 0.55), -90),
                        (vec2(0.1, 1.225), vec2(2.2, 0.55), 0),
                        (vec2(1.475, 0.4), vec2(1.1, 0.55), 90),
                    ],
                )
                .furniture_bulk(
                    "High Cupboards",
                    FurnitureType::Storage(StorageType::Cupboard),
                    RenderOrder::High,
                    vec![
                        (vec2(-1.425, -0.15), vec2(2.2, 0.25), -90),
                        (vec2(-0.725, 1.375), vec2(0.55, 0.25), 0),
                        (vec2(-1.225, 1.175), vec2(0.55, 0.25), -45),
                        (vec2(1.625, 0.4), vec2(1.1, 0.25), 90),
                        (vec2(1.425, 1.175), vec2(0.55, 0.25), 45),
                    ],
                )
                .furniture_bulk(
                    "High Cupboards Corners",
                    FurnitureType::Misc,
                    RenderOrder::High,
                    vec![
                        (vec2(0.1, 1.3625), vec2(3.3, 0.275), 0),
                        (vec2(-1.425, 1.225), vec2(0.55, 0.275), -90),
                        (vec2(1.625, 1.225), vec2(0.55, 0.275), 90),
                    ],
                )
                .furniture(vec![
                    Furniture::new_ordered(
                        "Fridge",
                        FurnitureType::Storage(StorageType::Cupboard),
                        RenderOrder::High,
                        vec2(1.475, -0.425),
                        vec2(0.55, 0.55),
                        90,
                    ),
                    Furniture::new(
                        "Microwave",
                        FurnitureType::Storage(StorageType::Cupboard),
                        vec2(1.4, 1.15),
                        vec2(0.5, 0.4),
                        45,
                    )
                    .materials("MetalDark"),
                    Furniture::new(
                        "Oven",
                        FurnitureType::Storage(StorageType::Drawer),
                        vec2(-1.275, 0.125),
                        vec2(0.55, 0.55),
                        -90,
                    )
                    .materials("Granite")
                    .material_children("MetalDark"),
                    Furniture::new(
                        "Hob",
                        FurnitureType::Kitchen(KitchenType::Hob),
                        vec2(-1.275, 0.125),
                        vec2(0.45, 0.45),
                        -90,
                    ),
                    Furniture::new_ordered(
                        "Extractor Vent",
                        FurnitureType::Misc,
                        RenderOrder::High,
                        vec2(-1.45, 0.125),
                        vec2(0.2, 0.2),
                        0,
                    )
                    .materials("MetalDark"),
                    Furniture::new(
                        "Kitchen Sink",
                        FurnitureType::Kitchen(KitchenType::Sink),
                        vec2(0.2, 1.2),
                        vec2(0.65, 0.5),
                        0,
                    ),
                ]),
            Room::new("Storage1", vec2(-1.65, 2.5), vec2(1.5, 1.1), "Carpet")
                .door(vec2(0.75, 0.0), -90),
            Room::new("Storage2", vec2(-1.65, 1.4), vec2(1.5, 1.1), "Carpet")
                .door(vec2(0.75, 0.0), -90),
            Room::new("Bedroom", vec2(3.85, -0.95), vec2(3.9, 3.6), "Carpet")
                .subtract(vec2(-1.1, 1.4), vec2(1.7, 1.0))
                .door(vec2(-0.25, 1.35), 90)
                .window(vec2(0.0, -1.8), 0)
                .lights_grid_offset("Bedroom Downlights", 2, 2, vec2(2.0, 2.5), vec2(0.0, -0.4))
                .light_full("Bedside Light", 1.65, -1.45, LightType::Binary, 1.0, 0.025)
                .furniture(vec![
                    Furniture::new(
                        "Bed",
                        FurnitureType::Bed(Color::from_rgb(110, 120, 130)),
                        vec2(0.8, -0.45),
                        vec2(1.4, 2.1),
                        90,
                    ),
                    Furniture::new(
                        "Bedside Table",
                        FurnitureType::Storage(StorageType::Drawer),
                        vec2(1.625, -1.45),
                        vec2(0.4, 0.55),
                        90,
                    )
                    .materials("WoodDark"),
                    Furniture::new(
                        "Wardrobe",
                        FurnitureType::Storage(StorageType::Cupboard),
                        vec2(1.225, 1.45),
                        vec2(1.35, 0.6),
                        0,
                    ),
                    Furniture::new(
                        "Drawers",
                        FurnitureType::Storage(StorageType::Drawer),
                        vec2(-1.5, 0.08),
                        vec2(1.54, 0.8),
                        -90,
                    )
                    .materials("WoodDark"),
                    Furniture::new(
                        "Bedroom Radiator",
                        FurnitureType::Radiator,
                        vec2(0.0, -1.7),
                        vec2(1.4, 0.1),
                        0,
                    ),
                ])
                .add_sensors(&[Sensor::new("tower_fan_temperature", "TMP", "°C")])
                .sensor_offset(vec2(0.0, -0.4)),
            Room::new("Ensuite", vec2(1.1, -1.4), vec2(1.6, 2.7), "GraniteTiles")
                .door(vec2(0.8, -0.85), -90)
                .window(vec2(0.0, -1.35), 0)
                .light("Ensuite Downlight", 0.0, -0.4)
                .light("Ensuite Shower Downlight", -0.4, 0.7)
                .furniture(vec![
                    Furniture::new(
                        "Ensuite Shower",
                        FurnitureType::Bathroom(BathroomType::Shower),
                        vec2(-0.4, 0.65),
                        vec2(0.7, 1.3),
                        0,
                    ),
                    Furniture::new(
                        "Ensuite Toilet",
                        FurnitureType::Bathroom(BathroomType::Toilet),
                        vec2(-0.425, -0.9),
                        vec2(0.55, 0.65),
                        -90,
                    ),
                    Furniture::new(
                        "Ensuite Sink",
                        FurnitureType::Bathroom(BathroomType::Sink),
                        vec2(0.35, 0.075),
                        vec2(0.45, 0.45),
                        0,
                    ),
                    Furniture::new(
                        "Ensuite Radiator",
                        FurnitureType::Radiator,
                        vec2(0.375, -1.25),
                        vec2(0.7, 0.1),
                        0,
                    ),
                ]),
            Room::new("Boiler Room", vec2(1.5, -0.55), vec2(0.8, 1.0), "Carpet")
                .door_width(vec2(0.0, 0.5), 180, 0.6)
                .furniture(vec![Furniture::new(
                    "Boiler",
                    FurnitureType::Misc,
                    vec2(0.0, -0.1),
                    vec2(0.6, 0.6),
                    0,
                )
                .material("MetalDark")]),
            Room::new("Spare Room", vec2(4.2, 1.95), vec2(3.2, 2.2), "Carpet")
                .subtract(vec2(-1.1, -1.4), vec2(1.0, 1.0))
                .door(vec2(-1.1, -0.9), 180)
                .window(vec2(1.6, 0.0), -90)
                .lights_grid_offset("Office Downlights", 2, 1, vec2(1.75, 1.75), vec2(0.0, -0.1))
                .furniture(vec![
                    Furniture::new(
                        "Spare Wardrobe",
                        FurnitureType::Storage(StorageType::Cupboard),
                        vec2(0.0, 0.75),
                        vec2(3.1, 0.6),
                        0,
                    ),
                    Furniture::new(
                        "Spare Radiator",
                        FurnitureType::Radiator,
                        vec2(1.5, 0.0),
                        vec2(0.75, 0.1),
                        90,
                    ),
                ]),
            Room::new("Bathroom", vec2(1.4, 2.05), vec2(2.4, 2.0), "GraniteTiles")
                .door(vec2(0.7, -1.0), 180)
                .light_center("Bathroom Downlight")
                .light("Bathroom Shower Downlight", 0.85, 0.55)
                .furniture(vec![
                    Furniture::new(
                        "Bathroom Shower",
                        FurnitureType::Bathroom(BathroomType::Shower),
                        vec2(0.85, 0.525),
                        vec2(0.6, 0.85),
                        0,
                    ),
                    Furniture::new(
                        "Bathroom Bath",
                        FurnitureType::Bathroom(BathroomType::Bath),
                        vec2(-0.75, 0.0),
                        vec2(0.8, 1.9),
                        0,
                    ),
                    Furniture::new(
                        "Bathroom Toilet",
                        FurnitureType::Bathroom(BathroomType::Toilet),
                        vec2(0.075, 0.625),
                        vec2(0.55, 0.65),
                        0,
                    ),
                    Furniture::new(
                        "Bathroom Sink",
                        FurnitureType::Bathroom(BathroomType::Sink),
                        vec2(0.0, -0.725),
                        vec2(0.45, 0.45),
                        -180,
                    ),
                    Furniture::new(
                        "Bathroom Radiator",
                        FurnitureType::Radiator,
                        vec2(1.1, -0.375),
                        vec2(0.7, 0.1),
                        90,
                    ),
                ]),
        ],
        rendered_data: None,
        light_data: None,
    }
}
