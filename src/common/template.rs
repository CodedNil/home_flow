use crate::common::{
    color::Color,
    furniture::{
        BathroomType, ChairType, ElectronicType, Furniture, FurnitureType, KitchenType,
        RenderOrder, SensorType, StorageType, TableType,
    },
    layout::{
        Action, DataPoint, GlobalMaterial, Home, LightType, Operation, Outline, Room, Sensor,
        Shape, Walls, Zone, LAYOUT_VERSION,
    },
    utils::Material,
};
use glam::dvec2 as vec2;

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
            Room::new("Hall", vec2(1.35, 0.5), vec2(4.5, 1.10), "Carpet")
                .set_walls(Walls::TOP)
                .add_material(vec2(-1.7, 1.55), vec2(1.1, 2.0), "Wood")
                .door_flipped(vec2(-1.7, 2.55), 0)
                .lights_grid("Hall Downlights", 3, 1, vec2(1.15, 1.75), vec2(0.0, 0.0))
                .light("Hall Downlights", -1.7, 1.55)
                .furniture(Furniture::new(
                    "Hall Radiator",
                    FurnitureType::Radiator,
                    vec2(-0.425, 0.45),
                    vec2(1.2, 0.1),
                    0,
                ))
                .furniture(
                    Furniture::new(
                        "Vallhorn Motion Sensor",
                        FurnitureType::Sensor(SensorType::PresenceBoolean),
                        vec2(-1.7, 1.6),
                        vec2(0.2, 0.2),
                        -90,
                    )
                    .add_sensors(&["binary_sensor.hall_vallhorn_motion_sensor_occupancy"]),
                )
                .furniture(
                    Furniture::new(
                        "Parasoll Door Sensor",
                        FurnitureType::Sensor(SensorType::PresenceBoolean),
                        vec2(-1.7, 2.0),
                        vec2(0.2, 0.2),
                        0,
                    )
                    .add_sensors(&["binary_sensor.hall_parasoll_door_sensor_opening"]),
                ),
            Room::new("Lounge", vec2(-2.75, -1.4), vec2(6.1, 2.7), "Carpet")
                .set_walls(Walls::LEFT | Walls::BOTTOM)
                .operation(Operation::new(
                    Action::SubtractWall,
                    Shape::Rectangle,
                    vec2(-2.2, 1.35),
                    vec2(1.6, 0.3),
                ))
                .add(vec2(0.85, 1.8), vec2(2.0, 0.9))
                .window_width(vec2(-1.15, -1.35), 0, 1.4)
                .window(vec2(1.75, -1.35), 0)
                .zone(Zone::new(
                    "Lounge Left",
                    Shape::Rectangle,
                    vec2(1.525, 0.0),
                    vec2(3.05, 2.7),
                ))
                .zone(Zone::new(
                    "Lounge Left",
                    Shape::Rectangle,
                    vec2(0.85, 1.8),
                    vec2(2.0, 0.9),
                ))
                .zone(Zone::new(
                    "Lounge Right",
                    Shape::Rectangle,
                    vec2(-1.525, 0.0),
                    vec2(3.05, 2.7),
                ))
                .lights_grid(
                    "Lounge Left Downlights",
                    2,
                    2,
                    vec2(4.55, 0.8),
                    vec2(1.57, 0.0),
                )
                .lights_grid(
                    "Lounge Right Downlights",
                    2,
                    2,
                    vec2(4.55, 0.8),
                    vec2(-1.57, 0.0),
                )
                .furniture(Furniture::new(
                    "Lounge Radiator",
                    FurnitureType::Radiator,
                    vec2(-1.15, -1.25),
                    vec2(1.4, 0.1),
                    0,
                ))
                .furniture(
                    Furniture::new(
                        "Ultimate Mini",
                        FurnitureType::Sensor(SensorType::UltimateSensorMini),
                        vec2(-2.975, -1.125),
                        vec2(0.1, 0.1),
                        45,
                    )
                    .add_sensors(&[
                        "sensor.ultimatesensor_mini_target_1_x",
                        "sensor.ultimatesensor_mini_target_1_y",
                        "sensor.ultimatesensor_mini_target_2_x",
                        "sensor.ultimatesensor_mini_target_2_y",
                        "sensor.ultimatesensor_mini_target_3_x",
                        "sensor.ultimatesensor_mini_target_3_y",
                    ])
                    .add_data(vec![
                        ("calib_1", DataPoint::Vec4((-5.3, -2.2, 29.0, 809.0))), // Sofa By Sensor
                        ("calib_2", DataPoint::Vec4((-0.8, -1.8, 3419.0, 3266.0))), // Desk
                        ("calib_3", DataPoint::Vec4((-5.1, 2.3, -2414.0, 4127.0))), // Kitchen Corner
                        ("calib_4", DataPoint::Vec4((-2.9, 0.0, 349.0, 3725.0))),   // Kitchen Edge
                    ]),
                )
                .furniture(Furniture::new(
                    "Kivik 3 Seater Sofa",
                    FurnitureType::Chair(ChairType::Sofa(Color::from_rgb(234, 210, 168))),
                    vec2(-2.525, 0.0),
                    vec2(2.3, 0.95),
                    -90,
                ))
                .furniture(
                    Furniture::new(
                        "Kivik Footstool",
                        FurnitureType::Rug(Color::from_rgb(234, 210, 168)),
                        vec2(-1.75, 0.5),
                        vec2(0.8, 0.6),
                        -90,
                    )
                    .render_order(RenderOrder::Mid),
                )
                .furniture(
                    Furniture::new(
                        "Dining Table",
                        FurnitureType::Table(TableType::DiningCustomChairs(1, 1, 0, 0)),
                        vec2(0.35, -0.9),
                        vec2(1.2, 0.8),
                        0,
                    )
                    .render_order(RenderOrder::Mid),
                )
                .furniture(Furniture::new_materials(
                    "Desk",
                    FurnitureType::Table(TableType::Desk),
                    vec2(2.55, -0.475),
                    vec2(1.6, 0.8),
                    -90,
                    "WoodDark",
                ))
                .furniture(
                    Furniture::new(
                        "Desk TV",
                        FurnitureType::Electronic(ElectronicType::Display),
                        vec2(2.975, -0.25),
                        vec2(1.0, 0.05),
                        -90,
                    )
                    .power_draw_entity("living_tv_current_consumption"),
                )
                .furniture(
                    Furniture::new(
                        "Desktop",
                        FurnitureType::Electronic(ElectronicType::Computer),
                        vec2(2.625, -1.1),
                        vec2(0.5, 0.3),
                        0,
                    )
                    .power_draw_entity("desktop_current_consumption"),
                )
                .furniture(
                    Furniture::new(
                        "Desk Fan",
                        FurnitureType::Electronic(ElectronicType::Computer),
                        vec2(2.55, 0.175),
                        vec2(0.2, 0.2),
                        45,
                    )
                    .power_draw_entity("desk_fan_current_consumption"),
                )
                .furniture(
                    Furniture::new_materials(
                        "Desk Mat",
                        FurnitureType::Rug(Color::from_rgb(88, 155, 238)),
                        vec2(2.425, -0.45),
                        vec2(0.375, 0.85),
                        0,
                        "Wood",
                    )
                    .render_order(RenderOrder::Mid),
                )
                .furniture(Furniture::new_materials(
                    "Keyboard",
                    FurnitureType::Misc,
                    vec2(2.5, -0.375),
                    vec2(0.175, 0.35),
                    0,
                    "Marble",
                ))
                .furniture(Furniture::new(
                    "Mini Display",
                    FurnitureType::Electronic(ElectronicType::Display),
                    vec2(2.7, -0.85),
                    vec2(0.2, 0.025),
                    300,
                ))
                .furniture(Furniture::new_materials(
                    "Mini Control",
                    FurnitureType::Misc,
                    vec2(2.55, -0.8),
                    vec2(0.1, 0.15),
                    15,
                    "Marble",
                ))
                .furniture(Furniture::new_materials(
                    "Mouse",
                    FurnitureType::Misc,
                    vec2(2.5, -0.625),
                    vec2(0.1, 0.07),
                    0,
                    "MetalDark",
                ))
                .furniture(
                    Furniture::new(
                        "Desktop On - Chair Occupancy",
                        FurnitureType::Sensor(SensorType::PresenceBoolean),
                        vec2(2.05, -0.475),
                        vec2(0.2, 0.2),
                        0,
                    )
                    .add_sensors(&["binary_sensor.desktop_on"]),
                )
                .add_sensors(&[
                    Sensor::new("ultimatesensor_mini_scd41_temperature", "TMP", "°C"),
                    Sensor::new("ultimatesensor_mini_scd41_humidity", "HUM", "%"),
                    Sensor::new("ultimatesensor_mini_scd41_co2", "CO2", "ppm"),
                    Sensor::new("ultimatesensor_mini_voc_index", "VOC", "idx"),
                    Sensor::new(
                        "ultimatesensor_mini_pm_2_5_m_weight_concentration",
                        "PM",
                        "µg/m³",
                    ),
                ]),
            Room::new("Kitchen", vec2(-4.2, 1.5), vec2(3.2, 3.1), "MarbleTiles")
                .set_walls(Walls::LEFT | Walls::TOP)
                .add(vec2(1.65, 0.45), vec2(0.3, 2.2))
                .subtract(vec2(1.55, -1.15), vec2(0.5, 1.0))
                .window_width(vec2(0.3, 1.55), 0, 1.4)
                .lights_grid("Kitchen Downlights", 2, 1, vec2(2.0, 2.0), vec2(0.1, 0.0))
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
                .furniture(
                    Furniture::new(
                        "Fridge",
                        FurnitureType::Storage(StorageType::Cupboard),
                        vec2(1.475, -0.425),
                        vec2(0.55, 0.55),
                        90,
                    )
                    .render_order(RenderOrder::High),
                )
                .furniture(Furniture::new_materials(
                    "Microwave",
                    FurnitureType::Storage(StorageType::Cupboard),
                    vec2(1.4, 1.15),
                    vec2(0.5, 0.4),
                    45,
                    "MetalDark",
                ))
                .furniture(
                    Furniture::new_materials(
                        "Oven",
                        FurnitureType::Storage(StorageType::Drawer),
                        vec2(-1.275, 0.125),
                        vec2(0.55, 0.55),
                        -90,
                        "Granite",
                    )
                    .material_children("MetalDark"),
                )
                .furniture(Furniture::new(
                    "Hob",
                    FurnitureType::Kitchen(KitchenType::Hob),
                    vec2(-1.275, 0.125),
                    vec2(0.45, 0.45),
                    -90,
                ))
                .furniture(
                    Furniture::new_materials(
                        "Extractor Vent",
                        FurnitureType::Misc,
                        vec2(-1.45, 0.125),
                        vec2(0.2, 0.2),
                        0,
                        "MetalDark",
                    )
                    .render_order(RenderOrder::High),
                )
                .furniture(Furniture::new(
                    "Kitchen Sink",
                    FurnitureType::Kitchen(KitchenType::Sink),
                    vec2(0.2, 1.2),
                    vec2(0.65, 0.5),
                    0,
                ))
                .add_sensors(&[
                    Sensor::new("vindstyrka_air_sensor_kitchen_temperature", "TMP", "°C"),
                    Sensor::new("vindstyrka_air_sensor_kitchen_humidity", "HUM", "%"),
                    Sensor::new("vindstyrka_air_sensor_kitchen_pm2_5", "PM", "µg/m³"),
                ])
                .sensor_offset(vec2(0.1, -0.5)),
            Room::new("Storage1", vec2(-1.65, 2.5), vec2(1.5, 1.1), "Carpet")
                .door(vec2(0.75, 0.0), -90),
            Room::new("Storage2", vec2(-1.65, 1.4), vec2(1.5, 1.1), "Carpet")
                .door(vec2(0.75, 0.0), -90),
            Room::new("Bedroom", vec2(3.85, -0.95), vec2(3.9, 3.6), "Carpet")
                .subtract(vec2(-1.1, 1.4), vec2(1.7, 1.0))
                .door(vec2(-0.25, 1.35), -90)
                .window(vec2(0.0, -1.8), 0)
                .lights_grid("Bedroom Downlights", 2, 2, vec2(2.0, 2.5), vec2(0.0, -0.4))
                .light_full("Bedside Light", 1.65, -1.45, LightType::Binary, 1.0, 0.025)
                .furniture(Furniture::new(
                    "Bed",
                    FurnitureType::Bed(Color::from_rgb(110, 120, 130)),
                    vec2(0.8, -0.45),
                    vec2(1.4, 2.1),
                    90,
                ))
                .furniture(Furniture::new_materials(
                    "Bedside Table",
                    FurnitureType::Storage(StorageType::Drawer),
                    vec2(1.625, -1.45),
                    vec2(0.4, 0.55),
                    90,
                    "WoodDark",
                ))
                .furniture(Furniture::new(
                    "Wardrobe",
                    FurnitureType::Storage(StorageType::Cupboard),
                    vec2(1.225, 1.45),
                    vec2(1.35, 0.6),
                    0,
                ))
                .furniture(Furniture::new_materials(
                    "Drawers",
                    FurnitureType::Storage(StorageType::Drawer),
                    vec2(-1.5, 0.08),
                    vec2(1.54, 0.8),
                    -90,
                    "WoodDark",
                ))
                .furniture(Furniture::new(
                    "Bedroom Radiator",
                    FurnitureType::Radiator,
                    vec2(0.0, -1.7),
                    vec2(1.4, 0.1),
                    0,
                ))
                .furniture(
                    Furniture::new(
                        "Ultimate Mini",
                        FurnitureType::Sensor(SensorType::UltimateSensorMini),
                        vec2(1.85, 0.975),
                        vec2(0.1, 0.1),
                        250,
                    )
                    .add_sensors(&[
                        "sensor.bedroom_ultimatesensor_target_1_x_2",
                        "sensor.bedroom_ultimatesensor_target_1_y_2",
                        "sensor.bedroom_ultimatesensor_target_2_x_2",
                        "sensor.bedroom_ultimatesensor_target_2_y_2",
                        "sensor.bedroom_ultimatesensor_target_3_x_2",
                        "sensor.bedroom_ultimatesensor_target_3_y_2",
                    ])
                    .add_data(vec![
                        ("calib_1", DataPoint::Vec4((2.1, 0.5, 1989.0, 2636.0))), // Outside Bathroom
                        ("calib_2", DataPoint::Vec4((2.2, -2.2, -979.0, 3640.0))), // Outside Ensuite
                        ("calib_3", DataPoint::Vec4((4.9, -2.4, -2050.0, 1420.0))), // By Bed
                    ]),
                )
                .add_sensors(&[Sensor::new("tower_fan_temperature", "TMP", "°C")])
                .sensor_offset(vec2(0.0, -0.4)),
            Room::new("Ensuite", vec2(1.1, -1.4), vec2(1.6, 2.7), "GraniteTiles")
                .door(vec2(0.8, -0.85), 90)
                .window(vec2(0.0, -1.35), 0)
                .light("Ensuite Downlight", 0.0, -0.4)
                .light("Ensuite Shower Downlight", -0.4, 0.7)
                .furniture(Furniture::new(
                    "Ensuite Shower",
                    FurnitureType::Bathroom(BathroomType::Shower),
                    vec2(-0.4, 0.65),
                    vec2(0.7, 1.3),
                    0,
                ))
                .furniture(Furniture::new(
                    "Ensuite Toilet",
                    FurnitureType::Bathroom(BathroomType::Toilet),
                    vec2(-0.425, -0.9),
                    vec2(0.55, 0.65),
                    -90,
                ))
                .furniture(Furniture::new(
                    "Ensuite Sink",
                    FurnitureType::Bathroom(BathroomType::Sink),
                    vec2(0.35, 0.075),
                    vec2(0.45, 0.45),
                    0,
                ))
                .furniture(Furniture::new(
                    "Ensuite Radiator",
                    FurnitureType::Radiator,
                    vec2(0.375, -1.25),
                    vec2(0.7, 0.1),
                    0,
                )),
            Room::new("Boiler Room", vec2(1.5, -0.55), vec2(0.8, 1.0), "Carpet")
                .door_width(vec2(0.0, 0.5), 180, 0.6)
                .furniture(Furniture::new_materials(
                    "Boiler",
                    FurnitureType::Misc,
                    vec2(0.0, -0.1),
                    vec2(0.6, 0.6),
                    0,
                    "MetalDark",
                )),
            Room::new("Spare Room", vec2(4.2, 1.95), vec2(3.2, 2.2), "Carpet")
                .subtract(vec2(-1.1, -1.4), vec2(1.0, 1.0))
                .door(vec2(-1.1, -0.9), 180)
                .window(vec2(1.6, 0.0), -90)
                .lights_grid("Office Downlights", 2, 1, vec2(1.75, 1.75), vec2(0.0, -0.1))
                .furniture(Furniture::new(
                    "Spare Wardrobe",
                    FurnitureType::Storage(StorageType::Cupboard),
                    vec2(0.0, 0.75),
                    vec2(3.1, 0.6),
                    0,
                ))
                .furniture(Furniture::new(
                    "Spare Radiator",
                    FurnitureType::Radiator,
                    vec2(1.5, 0.0),
                    vec2(0.75, 0.1),
                    90,
                )),
            Room::new("Bathroom", vec2(1.4, 2.05), vec2(2.4, 2.0), "GraniteTiles")
                .door_flipped(vec2(0.7, -1.0), 180)
                .light_center("Bathroom Downlight")
                .light("Bathroom Shower Downlight", 0.85, 0.55)
                .furniture(Furniture::new(
                    "Bathroom Shower",
                    FurnitureType::Bathroom(BathroomType::Shower),
                    vec2(0.85, 0.525),
                    vec2(0.6, 0.85),
                    0,
                ))
                .furniture(Furniture::new(
                    "Bathroom Bath",
                    FurnitureType::Bathroom(BathroomType::Bath),
                    vec2(-0.75, 0.0),
                    vec2(0.8, 1.9),
                    0,
                ))
                .furniture(Furniture::new(
                    "Bathroom Toilet",
                    FurnitureType::Bathroom(BathroomType::Toilet),
                    vec2(0.075, 0.625),
                    vec2(0.55, 0.65),
                    0,
                ))
                .furniture(Furniture::new(
                    "Bathroom Sink",
                    FurnitureType::Bathroom(BathroomType::Sink),
                    vec2(0.0, -0.725),
                    vec2(0.45, 0.45),
                    -180,
                ))
                .furniture(Furniture::new(
                    "Bathroom Radiator",
                    FurnitureType::Radiator,
                    vec2(1.1, -0.375),
                    vec2(0.7, 0.1),
                    90,
                )),
        ],
        rendered_data: None,
        light_data: None,
    }
}
