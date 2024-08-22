use crate::common::shape::{CLIPPER_PRECISION, EMPTY_MULTI_POLYGON};
use anyhow::{anyhow, Result};
use geo_types::{Coord, LineString, MultiPolygon, Polygon};
use glam::DVec2 as Vec2;
use js_sys::Array;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = window)]
    pub fn is_clipper_loaded() -> bool;

    #[wasm_bindgen(js_namespace = window)]
    fn offset_polygon(polygon: &Array, offset_size: i32, round_join: bool) -> JsValue;
}

pub fn offset_polygon_wasm(
    polygon: &Polygon,
    offset_size: f64,
    round_join: bool,
) -> Result<MultiPolygon> {
    let polygon_coords: Vec<Vec2> = polygon
        .exterior()
        .0
        .iter()
        .map(|p| Vec2::new(p.x, p.y))
        .collect();

    let polygon_js = Array::new();
    for vec in &polygon_coords {
        let x = (vec.x * CLIPPER_PRECISION).round() as i32;
        let y = (vec.y * CLIPPER_PRECISION).round() as i32;
        polygon_js.push(&JsValue::from(x));
        polygon_js.push(&JsValue::from(y));
    }

    // Returns an array of polygons, each polygon is an array of coordinates [x, y, x, y, x, y, ...]
    let result = offset_polygon(
        &polygon_js,
        (offset_size * CLIPPER_PRECISION) as i32,
        round_join,
    );

    let result_array = result
        .dyn_into::<Array>()
        .map_err(|_| anyhow!("Expected Array"))?;
    let mut coords: Vec<Vec<Vec2>> = Vec::new();

    for item in result_array.iter() {
        let item = item
            .dyn_into::<Array>()
            .map_err(|_| anyhow!("Expected Array"))?;
        let mut new_coords = Vec::new();
        for i in (0..item.length()).step_by(2) {
            let x = item.get(i).as_f64().unwrap() / CLIPPER_PRECISION;
            let y = item.get(i + 1).as_f64().unwrap() / CLIPPER_PRECISION;
            new_coords.push(Vec2::new(x, y));
        }
        coords.push(new_coords);
    }

    let mut polygons = EMPTY_MULTI_POLYGON;
    for vecs in &coords {
        let mut new_coords = Vec::new();
        for vec in vecs {
            new_coords.push(Coord::from((vec.x, vec.y)));
        }
        polygons
            .0
            .push(Polygon::new(LineString::new(new_coords), vec![]));
    }

    Ok(polygons)
}
