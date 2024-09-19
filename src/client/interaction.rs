use crate::{
    client::HomeFlow,
    common::{
        layout::{DataPoint, LightType},
        utils::Lerp,
        PostActionsData,
    },
};
use ahash::AHashMap;
use egui::{pos2, Color32, Painter, Pos2, Response, Stroke};

#[derive(Default)]
pub struct IState {
    pub light_drag: Option<LightDrag>,
}

pub struct LightDrag {
    pub group_id: String,
    pub start_state: u8,
    pub start_pos: Pos2,
    pub light_type: LightType,

    pub active: bool,
    pub start_time: f64,
    pub last_time: f64,
    pub animated_state: f64,
    pub animated_state_target: f64,
}

const POPUP_FADE_TIME: f64 = 0.1;

impl HomeFlow {
    pub fn interact_with_layout(&mut self, response: &Response, painter: &Painter) {
        let interaction_button = if self.is_mobile {
            egui::PointerButton::Primary
        } else {
            egui::PointerButton::Secondary
        };

        let mut light_hovered = None;
        for room in &self.layout.rooms {
            for light in &room.lights {
                let points = light.get_points(room);
                for point in points {
                    let mouse_dist = self.mouse_pos_world.distance(point) as f32;
                    if mouse_dist < (if self.is_mobile { 1.0 } else { 0.3 }) {
                        let mut clone = light.clone();
                        clone.pos = point;
                        light_hovered = Some(clone);
                    }
                }
            }
        }
        // Toggle light with a right click
        if response.clicked_by(interaction_button) {
            if let Some(light_hovered) = &light_hovered {
                let target_state = if light_hovered.state < 130 { 255 } else { 0 };
                let mut is_amended = false;
                if let Some(light_drag) = &mut self.interaction_state.light_drag {
                    if light_drag.group_id == light_hovered.entity_id {
                        is_amended = true;
                        light_drag.last_time = self.time;
                        light_drag.animated_state_target = f64::from(target_state) / 255.0;
                    }
                }
                if !is_amended {
                    self.interaction_state.light_drag = Some(LightDrag {
                        group_id: light_hovered.entity_id.clone(),
                        start_state: light_hovered.state,
                        start_pos: self.world_to_screen_pos(light_hovered.pos),
                        light_type: light_hovered.light_type.clone(),
                        active: false,
                        start_time: self.time,
                        last_time: self.time,
                        animated_state: f64::from(light_hovered.state) / 255.0,
                        animated_state_target: f64::from(target_state) / 255.0,
                    });
                }
            }
        }
        // Drag light with a right click
        if response.drag_started_by(interaction_button) {
            if let Some(light_hovered) = &light_hovered {
                self.interaction_state.light_drag = Some(LightDrag {
                    group_id: light_hovered.entity_id.clone(),
                    start_state: light_hovered.state,
                    start_pos: self.world_to_screen_pos(light_hovered.pos),
                    light_type: light_hovered.light_type.clone(),
                    active: true,
                    start_time: self.time,
                    last_time: self.time,
                    animated_state: f64::from(light_hovered.state) / 255.0,
                    animated_state_target: f64::from(light_hovered.state) / 255.0,
                });
            }
        }
        if response.drag_stopped_by(interaction_button) {
            if let Some(light_drag) = &mut self.interaction_state.light_drag {
                light_drag.active = false;
            }
        }
        let mut should_end = false;
        if let Some(light_drag) = &mut self.interaction_state.light_drag {
            let widget_height = 150.0;
            let start_percent = f32::from(light_drag.start_state) / 255.0;

            if response.dragged_by(interaction_button) {
                let vertical_distance = light_drag.start_pos.y - self.mouse_pos.y as f32;
                let new_percent =
                    (start_percent + vertical_distance / widget_height).clamp(0.0, 1.0);

                if matches!(light_drag.light_type, LightType::Binary) {
                    light_drag.animated_state = if new_percent > 0.5 { 1.0 } else { 0.0 };
                    light_drag.animated_state_target = if new_percent > 0.5 { 1.0 } else { 0.0 };
                } else {
                    light_drag.animated_state = f64::from(new_percent);
                    light_drag.animated_state_target = f64::from(new_percent);
                }
                light_drag.last_time = self.time;
            } else if (light_drag.animated_state - light_drag.animated_state_target).abs()
                > f64::EPSILON
            {
                // Move state towards target
                let diff = (light_drag.animated_state_target - light_drag.animated_state).signum()
                    * self.frame_time
                    * if matches!(light_drag.light_type, LightType::Binary) {
                        8.0
                    } else {
                        3.0
                    };
                light_drag.animated_state = (light_drag.animated_state + diff).clamp(0.0, 1.0);
                light_drag.last_time = self.time;
            }
            if self.time - light_drag.last_time > POPUP_FADE_TIME {
                should_end = true;
            }
            // Fade in and out the widget
            let start_fade =
                ((self.time - light_drag.start_time) / POPUP_FADE_TIME).clamp(0.0, 1.0);
            let reverse_fade =
                1.0 - ((self.time - light_drag.last_time) / POPUP_FADE_TIME).clamp(0.0, 1.0);
            let fade = start_fade.min(reverse_fade);

            // Set lights to the new state
            let new_state = (light_drag.animated_state * 255.0) as u8;
            for room in &mut self.layout.rooms {
                for light in &mut room.lights {
                    if light.entity_id == light_drag.group_id {
                        light.state = new_state;
                        light.last_manual = self.time;

                        // Remove existing post packets for this light, and add a new one
                        let entity_id = format!("light.{}", light.entity_id);
                        self.post_queue.retain(|x| x.entity_id != entity_id);
                        if matches!(light_drag.light_type, LightType::Binary) {
                            self.post_queue.push(PostActionsData {
                                entity_id,
                                domain: "light".to_string(),
                                action: if new_state > 150 {
                                    "turn_on"
                                } else {
                                    "turn_off"
                                }
                                .to_string(),
                                additional_data: AHashMap::new(),
                            });
                        } else {
                            let action =
                                if new_state > 0 { "turn_on" } else { "turn_off" }.to_string();
                            let mut additional_data = AHashMap::new();
                            if new_state > 0 {
                                additional_data.insert(
                                    "brightness_pct".to_string(),
                                    DataPoint::Int((f64::from(new_state) * 100.0 / 255.0) as u8),
                                );
                            }

                            self.post_queue.push(PostActionsData {
                                entity_id,
                                domain: "light".to_string(),
                                action,
                                additional_data,
                            });
                        }
                    }
                }
            }

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
                pos_bottom.y - widget_height * light_drag.animated_state as f32,
            )
            .round();

            paint_line_circle_caps(painter, pos_bottom, pos_top, 20.0, Color32::WHITE, fade);
            paint_line_circle_caps(painter, pos_bottom, pos_top, 16.0, Color32::BLACK, fade);
            paint_line_circle_caps(painter, pos_bottom, pos_top, 12.0, Color32::WHITE, fade);

            // Calculate the color based on the light's state
            let color = if light_drag.animated_state < f64::EPSILON {
                Color32::from_rgb(100, 100, 100)
            } else {
                let color_off = Color32::from_rgb(200, 200, 200);
                let color_on = Color32::from_rgb(255, 255, 50);
                Color32::from_rgb(
                    color_off.r().lerp(color_on.r(), light_drag.animated_state),
                    color_off.g().lerp(color_on.g(), light_drag.animated_state),
                    color_off.b().lerp(color_on.b(), light_drag.animated_state),
                )
            };

            paint_line_circle_caps(painter, pos_bottom, pos_current, 16.0, Color32::BLACK, fade);
            paint_line_circle_caps(painter, pos_bottom, pos_current, 12.0, color, fade);
        }
        if should_end {
            self.interaction_state.light_drag = None;
        }
    }
}

fn paint_line_circle_caps(
    painter: &Painter,
    start: Pos2,
    end: Pos2,
    width: f32,
    color: Color32,
    fade: f64,
) {
    let width = width * fade as f32;
    painter.circle_filled(start, width / 2.0, color);
    painter.circle_filled(end, width / 2.0, color);
    painter.line_segment([start, end], Stroke::new(width, color));
}
