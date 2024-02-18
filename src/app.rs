use self::edit_mode::EditDetails;
use crate::{
    common::{layout::Home, template::template_home, utils::rotate_point},
    server::common_api::get_layout,
};
use anyhow::Result;
use egui::{
    util::History, Align2, CentralPanel, Color32, Context, Frame, Sense, TextureHandle, Window,
};
use egui_notify::Toasts;
use glam::{dvec2 as vec2, DVec2 as Vec2};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};

mod edit_mode;
mod edit_mode_render;
mod edit_mode_utils;
mod interaction;
mod render;

pub struct HomeFlow {
    time: f64,
    frame_time: f64,

    canvas_center: Vec2,
    mouse_pos: Vec2,
    mouse_pos_world: Vec2,

    layout_server: Home,
    layout: Home,
    textures: HashMap<String, TextureHandle>,
    light_data: Option<(u64, TextureHandle)>,
    bounds: (Vec2, Vec2),
    rotate_speed: f64,

    toasts: Arc<Mutex<Toasts>>,
    edit_mode: EditDetails,
    frame_times: History<f32>,
    host: String,

    stored: StoredData,

    download_data: Arc<Mutex<DownloadData>>,
}

#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct StoredData {
    translation: Vec2,
    zoom: f64, // Zoom is meter to pixels
    rotation: f64,
}

impl Default for StoredData {
    fn default() -> Self {
        Self {
            translation: Vec2::ZERO,
            zoom: 100.0,
            rotation: 0.0,
        }
    }
}

impl Default for HomeFlow {
    fn default() -> Self {
        Self {
            time: 0.0,
            frame_time: 0.0,
            canvas_center: Vec2::ZERO,
            mouse_pos: Vec2::ZERO,
            mouse_pos_world: Vec2::ZERO,

            layout_server: Home::default(),
            layout: Home::default(),
            textures: HashMap::new(),
            light_data: None,
            bounds: (Vec2::ZERO, Vec2::ZERO),
            rotate_speed: 0.0,

            toasts: Arc::new(Mutex::new(Toasts::default())),
            edit_mode: EditDetails::default(),
            frame_times: History::new(0..300, 1.0),
            host: "localhost:3000".to_string(),
            stored: StoredData::default(),
            download_data: Arc::new(Mutex::new(DownloadData::default())),
        }
    }
}

#[derive(Default)]
struct DownloadData {
    layout: DownloadLayout,
}

#[derive(Default)]
enum DownloadLayout {
    #[default]
    None,
    InProgress,
    Done(Result<Home>),
}

impl HomeFlow {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let stored = cc.storage.map_or_else(StoredData::default, |storage| {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        });

        Self {
            stored,
            ..Default::default()
        }
    }

    fn pixels_to_world(&self, v: Vec2) -> Vec2 {
        let pivot = vec2(-self.stored.translation.x, self.stored.translation.y);
        rotate_point(
            vec2(
                (v.x - self.canvas_center.x) / self.stored.zoom - self.stored.translation.x,
                (self.canvas_center.y - v.y) / self.stored.zoom + self.stored.translation.y,
            ),
            pivot,
            -self.stored.rotation,
        )
    }

    fn world_to_pixels(&self, v: Vec2) -> Vec2 {
        let pivot = vec2(-self.stored.translation.x, self.stored.translation.y);
        let v = rotate_point(v, pivot, self.stored.rotation);
        vec2(
            (v.x + self.stored.translation.x) * self.stored.zoom + self.canvas_center.x,
            (self.stored.translation.y - v.y) * self.stored.zoom + self.canvas_center.y,
        )
    }
    fn world_to_pixels_pos(&self, v: Vec2) -> egui::Pos2 {
        let v = self.world_to_pixels(v);
        egui::pos2(v.x as f32, v.y as f32)
    }

    fn handle_pan_zoom(&mut self, response: &egui::Response, ui: &egui::Ui) {
        // Drag
        let mut translation_delta = if response.dragged() {
            egui_to_vec2(response.drag_delta()) * 0.01
        } else {
            Vec2::ZERO
        };

        // Zoom
        let mut scroll_delta = egui_to_vec2(ui.input(|i| i.raw_scroll_delta)).y;
        if scroll_delta.abs() > 0.0 {
            scroll_delta = scroll_delta.signum() * 15.0;
        }
        let mut is_multi_touch = false;
        let mut multi_touch_rotation = 0.0;
        if let Some(multi_touch) = ui.ctx().multi_touch() {
            is_multi_touch = true;
            scroll_delta = (multi_touch.zoom_delta as f64 - 1.0) * 80.0;
            translation_delta = egui_to_vec2(multi_touch.translation_delta) * 0.01;
            multi_touch_rotation = multi_touch.rotation_delta as f64;
        }
        if scroll_delta.abs() > 0.0 {
            let zoom_amount = scroll_delta * (self.stored.zoom / 100.0);
            let mouse_world_before_zoom = self.pixels_to_world(self.mouse_pos);
            self.stored.zoom = (self.stored.zoom + zoom_amount).clamp(60.0, 300.0);
            let mouse_world_after_zoom = self.pixels_to_world(self.mouse_pos);
            let difference = mouse_world_after_zoom - mouse_world_before_zoom;
            self.stored.translation += Vec2::new(difference.x, -difference.y);
        }

        if translation_delta.length() > 0.0 {
            let rotated = rotate_point(translation_delta, Vec2::ZERO, self.stored.rotation);
            self.stored.translation += rotated / (self.stored.zoom / 100.0);
        }

        let (q_down, e_down) = ui.input(|i| (i.key_down(egui::Key::Q), i.key_down(egui::Key::E)));
        let max_speed = 800.0;
        if q_down || e_down {
            let rotation_delta = if q_down { 1.0 } else { -1.0 };
            self.rotate_speed = (self.rotate_speed + rotation_delta * 400.0 * self.frame_time)
                .clamp(-max_speed, max_speed);
        } else if is_multi_touch {
            self.stored.rotation -= multi_touch_rotation.to_degrees();
            self.rotate_speed = 0.0;
        } else {
            // Determine the nearest 90 degree snap target based on current rotation
            let target_rotation =
                ((self.stored.rotation + self.rotate_speed * 0.25) / 90.0).round() * 90.0;
            let rotation_diff = target_rotation - self.stored.rotation;

            // Adjust rotate speed towards the needed speed for snapping, within the max speed limit
            let needed_speed = rotation_diff * self.frame_time * 500.0;
            self.rotate_speed = if rotation_diff.abs() > 0.1 {
                needed_speed.clamp(-max_speed, max_speed)
            } else {
                self.stored.rotation = target_rotation;
                0.0
            };
        }

        // Apply rotation if there's any rotate speed
        if self.rotate_speed.abs() > 0.0 {
            self.stored.rotation += self.rotate_speed * self.frame_time;
            self.stored.rotation = self.stored.rotation.rem_euclid(360.0);
        }

        // Clamp translation to bounds
        if self.bounds.0.is_finite() && self.bounds.1.is_finite() {
            self.stored.translation = self.stored.translation.clamp(self.bounds.0, self.bounds.1);
        }
    }

    fn load_layout(&mut self, ctx: &Context) {
        // Load layout from server if needed
        if !self.layout.version.is_empty() {
            return;
        }
        // If on github use template instead of loading from server
        if self.host.contains("github.io") {
            self.layout = template_home();
            self.layout_server = template_home();
            return;
        }
        let download_store = self.download_data.clone();
        let mut download_data_guard = download_store.lock();
        match &download_data_guard.layout {
            DownloadLayout::None => {
                log::info!("Loading layout from server");
                download_data_guard.layout = DownloadLayout::InProgress;
                drop(download_data_guard);
                get_layout(&self.host, move |res| {
                    download_store.lock().layout = DownloadLayout::Done(res);
                });
            }
            DownloadLayout::InProgress => {
                Window::new("Layout Download")
                    .fixed_pos(egui::pos2(
                        ctx.available_rect().center().x,
                        ctx.available_rect().center().y,
                    ))
                    .pivot(Align2::CENTER_CENTER)
                    .title_bar(false)
                    .resizable(false)
                    .interactable(false)
                    .show(ctx, |ui| {
                        ui.label("Downloading Home Layout");
                    });
            }
            DownloadLayout::Done(ref response) => {
                if let Ok(layout) = response {
                    log::info!("Loaded layout from server");
                    self.layout_server = layout.clone();
                    self.layout = layout.clone();
                } else {
                    log::error!("Failed to fetch or parse layout from server");
                }
                download_data_guard.layout = DownloadLayout::None;
            }
        }
    }
}

impl eframe::App for HomeFlow {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.stored);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        ctx.request_repaint();
        let previous_frame_time = frame.info().cpu_usage.unwrap_or_default();
        if let Some(latest) = self.frame_times.latest_mut() {
            *latest = previous_frame_time; // rewrite history now that we know
        }
        self.frame_times
            .add(ctx.input(|i| i.time), previous_frame_time); // projected
        let fps = 1.0 / self.frame_times.mean_time_interval().unwrap_or_default();
        Window::new("Performance")
            .fixed_pos(egui::pos2(20.0, 20.0))
            .pivot(Align2::LEFT_TOP)
            .title_bar(false)
            .resizable(false)
            .interactable(false)
            .show(ctx, |ui| {
                ui.label(format!("FPS: {fps:.2}"));
            });

        #[cfg(target_arch = "wasm32")]
        {
            let web_info = &frame.info().web_info;
            self.host = web_info.location.host.clone();
        }

        // Styling
        ctx.style_mut(|style| {
            style.visuals.window_shadow = egui::epaint::Shadow::NONE;
        });

        self.load_layout(ctx);

        CentralPanel::default()
            .frame(Frame {
                fill: Color32::from_rgb(25, 25, 35),
                ..Default::default()
            })
            .show(ctx, |ui| {
                self.time = ctx.input(|i| i.time);
                self.frame_time = ui.input(|i| i.unstable_dt) as f64;

                let (response, painter) =
                    ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
                self.canvas_center = egui_pos_to_vec2(response.rect.center());

                let mouse_pos = ui
                    .input(|i| i.pointer.interact_pos())
                    .map_or(self.mouse_pos, egui_pos_to_vec2);
                self.mouse_pos = mouse_pos;
                self.mouse_pos_world = self.pixels_to_world(mouse_pos);

                let edit_mode_response = self.run_edit_mode(&response, ctx, ui);
                if !edit_mode_response.used_dragged {
                    self.handle_pan_zoom(&response, ui);
                }

                self.render_layout(&painter, ctx);

                if self.edit_mode.enabled {
                    self.paint_edit_mode(&painter, &edit_mode_response, ctx);
                } else {
                    self.interact_with_layout(ctx);
                }

                Window::new("Bottom Right")
                    .fixed_pos(egui::pos2(
                        response.rect.right() - 10.0,
                        response.rect.bottom() - 10.0,
                    ))
                    .fixed_size(egui::vec2(100.0, 0.0))
                    .pivot(Align2::RIGHT_BOTTOM)
                    .title_bar(false)
                    .resizable(false)
                    .constrain(false)
                    .show(ctx, |ui| {
                        ui.with_layout(
                            egui::Layout::from_main_dir_and_cross_align(
                                egui::Direction::TopDown,
                                egui::Align::Center,
                            )
                            .with_cross_justify(true),
                            |ui| {
                                self.edit_mode_settings(ctx, ui);
                            },
                        );
                    });

                self.toasts.lock().show(ctx);
            });
    }
}

pub const fn vec2_to_egui_pos(vec: Vec2) -> egui::Pos2 {
    egui::pos2(vec.x as f32, vec.y as f32)
}

pub const fn egui_to_vec2(vec: egui::Vec2) -> Vec2 {
    vec2(vec.x as f64, vec.y as f64)
}

pub const fn egui_pos_to_vec2(vec: egui::Pos2) -> Vec2 {
    vec2(vec.x as f64, vec.y as f64)
}
