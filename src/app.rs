use self::edit_mode::EditDetails;
use crate::common::{
    layout,
    utils::{egui_pos_to_vec2, egui_to_vec2},
};
use egui::{
    util::History, Align2, CentralPanel, Color32, Context, Frame, Sense, TextureHandle, Window,
};
use egui_notify::Toasts;
use glam::{dvec2 as vec2, DVec2 as Vec2};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};

mod edit_mode;
mod edit_mode_utils;
mod render;

pub struct HomeFlow {
    time: f64,
    frame_time: f64,

    translation: Vec2,
    zoom: f64, // Zoom is meter to pixels
    canvas_center: Vec2,
    mouse_pos: Vec2,
    mouse_pos_world: Vec2,

    layout_server: layout::Home,
    layout: layout::Home,
    textures: HashMap<String, TextureHandle>,

    toasts: Arc<Mutex<Toasts>>,
    edit_mode: EditDetails,
    frame_times: History<f32>,
    host: String,

    stored_data: StoredData,

    download_data: Arc<Mutex<DownloadData>>,
}

#[derive(Deserialize, Serialize, Default)]
#[serde(default)]
pub struct StoredData {
    pub test: String,
}

impl Default for HomeFlow {
    fn default() -> Self {
        Self {
            time: 0.0,
            frame_time: 0.0,
            translation: Vec2::ZERO,
            zoom: 100.0,
            canvas_center: Vec2::ZERO,
            mouse_pos: Vec2::ZERO,
            mouse_pos_world: Vec2::ZERO,

            layout_server: layout::Home::default(),
            layout: layout::Home::default(),
            textures: HashMap::new(),

            toasts: Arc::new(Mutex::new(Toasts::default())),
            edit_mode: EditDetails::default(),
            frame_times: History::new(0..300, 1.0),
            host: "localhost:3000".to_string(),
            stored_data: StoredData::default(),
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
    Done(ehttp::Result<ehttp::Response>),
}

impl HomeFlow {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let stored_data = cc.storage.map_or_else(StoredData::default, |storage| {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        });

        Self {
            stored_data,
            ..Default::default()
        }
    }

    fn pixels_to_world(&self, x: f64, y: f64) -> Vec2 {
        vec2(x - self.canvas_center.x, self.canvas_center.y - y) / self.zoom
            - vec2(self.translation.x, -self.translation.y)
    }

    fn world_to_pixels(&self, x: f64, y: f64) -> Vec2 {
        vec2(x + self.translation.x, self.translation.y - y) * self.zoom
            + vec2(self.canvas_center.x, self.canvas_center.y)
    }

    fn handle_pan_zoom(&mut self, response: &egui::Response, ui: &egui::Ui) {
        // Drag
        if response.dragged() {
            self.translation += egui_to_vec2(response.drag_delta()) * 0.01 / (self.zoom / 100.0);
        }

        // Zoom
        let scroll_delta = egui_to_vec2(ui.input(|i| i.raw_scroll_delta));
        if scroll_delta != Vec2::ZERO {
            let zoom_amount = (scroll_delta.y.signum() * 15.0) * (self.zoom / 100.0);
            if let Some(mouse_pos) = ui.input(|i| i.pointer.latest_pos()) {
                let mouse_world_before_zoom =
                    self.pixels_to_world(mouse_pos.x as f64, mouse_pos.y as f64);
                self.zoom = (self.zoom + zoom_amount).clamp(20.0, 300.0);
                let mouse_world_after_zoom =
                    self.pixels_to_world(mouse_pos.x as f64, mouse_pos.y as f64);
                let difference = mouse_world_after_zoom - mouse_world_before_zoom;
                self.translation += Vec2::new(difference.x, -difference.y);
            } else {
                self.zoom = (self.zoom + zoom_amount).clamp(20.0, 300.0);
            }
        }

        // Clamp translation to bounds
        let bounds = [-30.0, 30.0, -30.0, 30.0];
        let window_size = ui.ctx().available_rect().size();
        let window_size_meters = vec2(window_size.x as f64, window_size.y as f64) / self.zoom / 2.0;
        let min_translation = Vec2::new(
            bounds[0] + window_size_meters.x,
            bounds[2] + window_size_meters.y,
        );
        let max_translation = Vec2::new(
            bounds[1] - window_size_meters.x,
            bounds[3] - window_size_meters.y,
        );
        if min_translation.x <= max_translation.x {
            self.translation.x = self
                .translation
                .x
                .clamp(min_translation.x, max_translation.x);
        } else {
            self.translation.x = 0.0;
        }
        if min_translation.y <= max_translation.y {
            self.translation.y = self
                .translation
                .y
                .clamp(min_translation.y, max_translation.y);
        } else {
            self.translation.y = 0.0;
        }
    }

    fn load_layout(&mut self, ctx: &Context) {
        // Load layout from server if needed
        if !self.layout.version.is_empty() {
            return;
        }
        // If on github use template instead of loading from server
        if self.host.contains("github.io") {
            self.layout = layout::Home::template();
            self.layout_server = layout::Home::template();
            return;
        }
        let download_store = self.download_data.clone();
        let mut download_data_guard = download_store.lock();
        match &download_data_guard.layout {
            DownloadLayout::None => {
                log::info!("Loading layout from server");
                download_data_guard.layout = DownloadLayout::InProgress;
                drop(download_data_guard);
                ehttp::fetch(
                    ehttp::Request::get(format!("http://{}/load_layout", self.host)),
                    move |response| {
                        download_store.lock().layout = DownloadLayout::Done(response);
                    },
                );
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
                let layout = response
                    .as_ref()
                    .ok()
                    .and_then(|res| res.text())
                    .and_then(|text| serde_json::from_str::<layout::Home>(text).ok());
                if let Some(layout) = layout {
                    self.layout_server = layout.clone();
                    self.layout = layout;
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
        eframe::set_value(storage, eframe::APP_KEY, &self.stored_data);
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
                fill: Color32::from_rgb(35, 35, 50),
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
                self.mouse_pos_world = self.pixels_to_world(mouse_pos.x, mouse_pos.y);

                let edit_mode_response = self.run_edit_mode(&response, ctx, ui);
                if !edit_mode_response.used_dragged {
                    self.handle_pan_zoom(&response, ui);
                }

                self.render_grid(&painter, &response.rect);

                self.render_layout(&painter, ctx);

                if self.edit_mode.enabled {
                    self.paint_edit_mode(&painter, &edit_mode_response, ctx);
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
