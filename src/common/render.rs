use super::layout::Home;
use super::shape::EMPTY_MULTI_POLYGON;
use crate::common::layout::{HomeRender, RoomRender, Walls};
use crate::common::shape::wall_polygons;
use geo::BooleanOps;
use rayon::prelude::*;
use std::collections::HashMap;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

impl Home {
    pub fn render(&mut self) {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        let hash = hasher.finish();
        if let Some(rendered_data) = &self.rendered_data {
            if rendered_data.hash == hash {
                return;
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        let start_time = std::time::Instant::now();

        // Process all rooms in parallel
        let room_polygons = self
            .rooms
            .clone()
            .into_par_iter()
            .enumerate()
            .map(|(index, room)| (index, room.id, room.polygons(), room.material_polygons()))
            .collect::<Vec<_>>();

        #[cfg(not(target_arch = "wasm32"))]
        println!("Processed polygons in {:?}", start_time.elapsed());

        // For each rooms polygon, subtract rooms above it
        let room_process_data = {
            let mut room_process_data = HashMap::new();
            for (index, id, polygons, material_polygons) in &room_polygons {
                let mut new_polygons = polygons.clone();
                let mut new_material_polygons = material_polygons.clone();
                for (above_index, _, above_polygons, _) in &room_polygons {
                    if above_index > index {
                        new_polygons = new_polygons.difference(above_polygons);
                        for material in material_polygons.keys() {
                            new_material_polygons.entry(*material).and_modify(|e| {
                                *e = e.difference(above_polygons);
                            });
                        }
                    }
                }
                let room = &self.rooms[*index];
                let wall_polygons = if room.walls == Walls::NONE {
                    EMPTY_MULTI_POLYGON
                } else {
                    let bounds = room.bounds_with_walls();
                    let center = (bounds.0 + bounds.1) / 2.0;
                    let size = bounds.1 - bounds.0;
                    wall_polygons(&new_polygons, center, size, &room.walls)
                };
                room_process_data.insert(*id, (new_polygons, new_material_polygons, wall_polygons));
            }
            room_process_data
        };

        for room in &mut self.rooms {
            let (polygons, material_polygons, wall_polygons) =
                room_process_data.get(&room.id).unwrap().clone();
            room.rendered_data = Some(RoomRender {
                polygons,
                material_polygons,
                wall_polygons,
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        println!("Rendered in {:?}", start_time.elapsed());

        self.rendered_data = Some(HomeRender { hash });
    }
}
