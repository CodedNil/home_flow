use super::HomeFlow;
use crate::common::utils::Lerp;
use egui::{pos2, Color32, Painter, Pos2, Response, Stroke};

#[derive(Default)]
pub struct InteractionState {
    pub light_drag: Option<LightDrag>,
}

pub struct LightDrag {
    pub group_name: String,
    pub start_state: u8,
    pub start_pos: Pos2,
}

impl HomeFlow {
    pub fn interact_with_layout(&mut self, response: &Response, painter: &Painter) {
        let mut light_hovered = None;
        for room in &self.layout.rooms {
            for light in &room.lights {
                let pos_world = room.pos + light.pos;
                let mouse_dist = self.mouse_pos_world.distance(pos_world) as f32;
                if mouse_dist < 0.2 {
                    let mut clone = light.clone();
                    clone.pos = room.pos + light.pos;
                    light_hovered = Some(clone);
                }
            }
        }
        // Toggle light with a right click
        if response.clicked_by(egui::PointerButton::Secondary) {
            if let Some(light_hovered) = &light_hovered {
                for room in &mut self.layout.rooms {
                    for light in &mut room.lights {
                        if light.name == light_hovered.name {
                            light.state = if light.state < 130 { 255 } else { 0 };
                        }
                    }
                }
            }
        }
        // Drag light with a right click
        if response.drag_started_by(egui::PointerButton::Secondary) {
            if let Some(light_hovered) = &light_hovered {
                self.interaction_state.light_drag = Some(LightDrag {
                    group_name: light_hovered.name.clone(),
                    start_state: light_hovered.state,
                    start_pos: self.world_to_screen_pos(light_hovered.pos),
                });
            }
        }
        if response.dragged_by(egui::PointerButton::Secondary) {
            if let Some(light_drag) = &self.interaction_state.light_drag {
                let widget_height = 150.0;
                let start_percent = light_drag.start_state as f32 / 255.0;

                let vertical_distance = light_drag.start_pos.y - self.mouse_pos.y as f32;
                let new_percent =
                    (start_percent + vertical_distance / widget_height).clamp(0.0, 1.0);
                let new_state = (new_percent * 255.0) as u8;

                let pos_bottom = pos2(
                    light_drag.start_pos.x,
                    light_drag.start_pos.y + widget_height * start_percent,
                )
                .round();
                let pos_top = pos2(
                    light_drag.start_pos.x,
                    light_drag.start_pos.y - widget_height * (1.0 - start_percent),
                )
                .round();
                let pos_current = pos2(
                    light_drag.start_pos.x,
                    pos_bottom.y - widget_height * new_percent,
                )
                .round();

                paint_line_circle_caps(painter, pos_bottom, pos_top, 20.0, Color32::WHITE);
                paint_line_circle_caps(painter, pos_bottom, pos_top, 16.0, Color32::BLACK);
                paint_line_circle_caps(painter, pos_bottom, pos_top, 12.0, Color32::WHITE);

                // Calculate the color based on the light's state
                let color_off = Color32::from_rgb(200, 200, 200);
                let color_on = Color32::from_rgb(255, 255, 50);
                let color = if new_state == 0 {
                    color_off
                } else {
                    let factor = 0.25 + (0.75 * (new_state - 1) as f64 / (255.0 - 1.0));
                    Color32::from_rgb(
                        color_off.r().lerp(color_on.r(), factor),
                        color_off.g().lerp(color_on.g(), factor),
                        color_off.b().lerp(color_on.b(), factor),
                    )
                };

                paint_line_circle_caps(painter, pos_bottom, pos_current, 16.0, Color32::BLACK);
                paint_line_circle_caps(painter, pos_bottom, pos_current, 12.0, color);

                // Set lights to the new state
                for room in &mut self.layout.rooms {
                    for light in &mut room.lights {
                        if light.name == light_drag.group_name {
                            light.state = new_state;
                        }
                    }
                }
            }
        }
    }
}

fn paint_line_circle_caps(painter: &Painter, start: Pos2, end: Pos2, width: f32, color: Color32) {
    painter.circle_filled(start, width / 2.0, color);
    painter.circle_filled(end, width / 2.0, color);
    painter.line_segment([start, end], Stroke::new(width, color));
}
