use self::edit_mode::EditDetails;
use crate::common::layout;
use egui::{
    util::History, Align2, CentralPanel, Color32, ColorImage, Context, Frame, Painter, Pos2, Rect,
    Sense, Stroke, TextureOptions, Vec2, Window,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

mod edit_mode;
pub struct HomeFlow {
    time: f64,

    translation: Vec2,
    zoom: f32, // Zoom is meter to pixels
    canvas_center: Pos2,
    mouse_pos: Pos2,
    mouse_pos_world: Pos2,

    layout_server: layout::Home,
    layout: layout::Home,

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
        let max_age: f32 = 1.0;
        let max_len = (max_age * 300.0).round() as usize;
        Self {
            time: 0.0,
            translation: Vec2::ZERO,
            zoom: 100.0,
            canvas_center: Pos2::ZERO,
            mouse_pos: Pos2::ZERO,
            mouse_pos_world: Pos2::ZERO,

            layout_server: layout::Home::empty(),
            layout: layout::Home::empty(),

            edit_mode: EditDetails::default(),
            frame_times: History::new(0..max_len, max_age),
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

    fn pixels_to_world(&self, x: f32, y: f32) -> Pos2 {
        Pos2::new(x - self.canvas_center.x, self.canvas_center.y - y) / self.zoom
            - Vec2::new(self.translation.x, -self.translation.y)
    }

    fn world_to_pixels(&self, x: f32, y: f32) -> Pos2 {
        Pos2::new(x + self.translation.x, self.translation.y - y) * self.zoom
            + Vec2::new(self.canvas_center.x, self.canvas_center.y)
    }

    fn handle_pan_zoom(&mut self, response: &egui::Response, ui: &egui::Ui) {
        // Drag
        if response.dragged() {
            self.translation += response.drag_delta() * 0.01 / (self.zoom / 100.0);
        }

        // Zoom
        let scroll_delta = ui.input(|i| i.scroll_delta);
        if scroll_delta != Vec2::ZERO {
            let zoom_amount = (scroll_delta.y.signum() * 15.0) * (self.zoom / 100.0);
            if let Some(mouse_pos) = ui.input(|i| i.pointer.latest_pos()) {
                let mouse_world_before_zoom = self.pixels_to_world(mouse_pos.x, mouse_pos.y);
                self.zoom = (self.zoom + zoom_amount).clamp(20.0, 300.0);
                let mouse_world_after_zoom = self.pixels_to_world(mouse_pos.x, mouse_pos.y);
                let difference = mouse_world_after_zoom - mouse_world_before_zoom;
                self.translation += Vec2::new(difference.x, -difference.y);
            } else {
                self.zoom = (self.zoom + zoom_amount).clamp(20.0, 300.0);
            }
        }

        // Clamp translation to bounds
        let bounds = [-30.0, 30.0, -30.0, 30.0];
        let window_size_meters = ui.ctx().available_rect().size() / self.zoom / 2.0;
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

    fn render_grid(&self, painter: &Painter, visible_rect: &Rect) {
        let grid_interval = 2.0_f32.powf((160.0 / self.zoom).abs().log2().round());
        let grid_intervals = [
            (
                grid_interval,
                Stroke::new(1.5, Color32::from_rgb(85, 85, 100)),
            ),
            (
                grid_interval / 4.0,
                Stroke::new(1.5, Color32::from_rgb(55, 55, 70)),
            ),
        ];

        let (bottom_edge_world, top_edge_world) = (
            self.pixels_to_world(0.0, visible_rect.bottom()).y,
            self.pixels_to_world(0.0, visible_rect.top()).y,
        );
        let (left_edge_world, right_edge_world) = (
            self.pixels_to_world(visible_rect.left(), 0.0).x,
            self.pixels_to_world(visible_rect.right(), 0.0).x,
        );

        let mut rendered_vertical = HashSet::new();
        let mut rendered_horizontal = HashSet::new();
        let mut lines = Vec::new();
        for (grid_interval, stroke) in grid_intervals {
            // Draw vertical grid lines
            for x in ((left_edge_world / grid_interval).ceil() as i32)
                ..=((right_edge_world / grid_interval).floor() as i32)
            {
                let grid_line_pixel = self.world_to_pixels(x as f32 * grid_interval, 0.0).x;
                let grid_line_pixel_int = (grid_line_pixel * 100.0).round() as i32;
                if rendered_vertical.contains(&grid_line_pixel_int) {
                    continue;
                }
                rendered_vertical.insert(grid_line_pixel_int);
                lines.push((
                    Pos2::new(grid_line_pixel, visible_rect.top()),
                    Pos2::new(grid_line_pixel, visible_rect.bottom()),
                    stroke,
                ));
            }

            // Draw horizontal grid lines
            for y in ((bottom_edge_world / grid_interval).ceil() as i32)
                ..=((top_edge_world / grid_interval).floor() as i32)
            {
                let grid_line_pixel = self.world_to_pixels(0.0, y as f32 * grid_interval).y;
                let grid_line_pixel_int = (grid_line_pixel * 100.0).round() as i32;
                if rendered_horizontal.contains(&grid_line_pixel_int) {
                    continue;
                }
                rendered_horizontal.insert(grid_line_pixel_int);
                lines.push((
                    Pos2::new(visible_rect.left(), grid_line_pixel),
                    Pos2::new(visible_rect.right(), grid_line_pixel),
                    stroke,
                ));
            }
        }
        for line in lines.iter().rev() {
            painter.line_segment([line.0, line.1], line.2);
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
        let mut download_data_guard = download_store.lock().unwrap();
        match &download_data_guard.layout {
            DownloadLayout::None => {
                log::info!("Loading layout from server");
                download_data_guard.layout = DownloadLayout::InProgress;
                drop(download_data_guard);
                ehttp::fetch(
                    ehttp::Request::get(format!("http://{}/load_layout", self.host)),
                    move |response| {
                        download_store.lock().unwrap().layout = DownloadLayout::Done(response);
                    },
                );
            }
            DownloadLayout::InProgress => {
                Window::new("Layout Download")
                    .fixed_pos(Pos2::new(
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
            .fixed_pos(Pos2::new(20.0, 20.0))
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

        self.load_layout(ctx);

        CentralPanel::default()
            .frame(Frame {
                fill: Color32::from_rgb(35, 35, 50),
                ..Default::default()
            })
            .show(ctx, |ui| {
                self.time += ui.input(|i| i.unstable_dt) as f64;

                let (response, painter) =
                    ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
                let canvas_center = response.rect.center();
                self.canvas_center = canvas_center;

                let mouse_pos = ui
                    .input(|i| i.pointer.interact_pos())
                    .map_or(self.mouse_pos, |mouse_pos| mouse_pos);
                self.mouse_pos = mouse_pos;
                self.mouse_pos_world = self.pixels_to_world(mouse_pos.x, mouse_pos.y);

                let edit_mode_response = self.run_edit_mode(&response, ctx);
                if !edit_mode_response.used_dragged {
                    self.handle_pan_zoom(&response, ui);
                }

                self.render_grid(&painter, &response.rect);

                self.layout.render();
                let rendered_data = self.layout.rendered_data.as_ref().unwrap();

                let canvas_size = rendered_data.texture.dimensions();
                let egui_image = ColorImage::from_rgba_unmultiplied(
                    [canvas_size.0 as usize, canvas_size.1 as usize],
                    &rendered_data.texture,
                );
                let canvas_texture_id = ctx
                    .load_texture("home_texture", egui_image, TextureOptions::NEAREST)
                    .id();

                let (bounds_min, bounds_max) = self.layout.bounds_with_walls();
                let home_center = (bounds_min + bounds_max) / 2.0;
                let home_size = bounds_max - bounds_min;

                let rect = Rect::from_center_size(
                    self.world_to_pixels(home_center.x, home_center.y),
                    Vec2::new(home_size.x * self.zoom, home_size.y * self.zoom),
                );
                painter.image(
                    canvas_texture_id,
                    rect,
                    Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );

                if self.edit_mode.enabled {
                    self.paint_edit_mode(&painter, &edit_mode_response, ctx);
                }

                Window::new("Bottom Right")
                    .fixed_pos(Pos2::new(
                        response.rect.right() - 10.0,
                        response.rect.bottom() - 10.0,
                    ))
                    .auto_sized()
                    .pivot(Align2::RIGHT_BOTTOM)
                    .title_bar(false)
                    .resizable(false)
                    .constrain(false)
                    .show(ctx, |ui| {
                        self.edit_mode_settings(ctx, ui);
                    });
            });
    }
}
