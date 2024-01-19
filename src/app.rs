use egui::{CentralPanel, Color32, Context, Painter, Pos2, Rect, Stroke, Vec2};

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct HomeFlow {
    #[serde(skip)]
    time: f64,

    #[serde(skip)]
    translation: Vec2,
    #[serde(skip)]
    zoom: f32, // Zoom is meter to pixels
}

impl Default for HomeFlow {
    fn default() -> Self {
        Self {
            time: 0.0,
            translation: Vec2::ZERO,
            zoom: 100.0,
        }
    }
}

impl HomeFlow {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Self::default()
    }

    fn pixels_to_world(&self, canvas_center: Pos2, pixels: Pos2) -> Vec2 {
        (pixels - canvas_center) / self.zoom - self.translation
    }

    fn pixels_to_world_x(&self, canvas_center: Pos2, pixels: f32) -> f32 {
        (pixels - canvas_center.x) / self.zoom - self.translation.x
    }

    fn pixels_to_world_y(&self, canvas_center: Pos2, pixels: f32) -> f32 {
        (pixels - canvas_center.y) / self.zoom - self.translation.y
    }

    fn world_to_pixels(&self, canvas_center: Pos2, world: Pos2) -> Pos2 {
        (world + self.translation) * self.zoom + Vec2::new(canvas_center.x, canvas_center.y)
    }

    fn world_to_pixels_x(&self, canvas_center: Pos2, world: f32) -> f32 {
        (world + self.translation.x) * self.zoom + canvas_center.x
    }

    fn world_to_pixels_y(&self, canvas_center: Pos2, world: f32) -> f32 {
        (world + self.translation.y) * self.zoom + canvas_center.y
    }

    fn render_grid(&self, painter: &Painter, visible_rect: &Rect, canvas_center: Pos2) {
        let grid_intervals = [
            (
                if self.zoom <= 40.0 {
                    4.0
                } else if self.zoom <= 80.0 {
                    2.0
                } else {
                    1.0
                },
                Color32::from_gray(160),
            ),
            (
                if self.zoom <= 40.0 {
                    1.0
                } else if self.zoom <= 80.0 {
                    0.5
                } else {
                    0.25
                },
                Color32::from_gray(50),
            ),
        ];

        let (top_edge_world, bottom_edge_world) = (
            self.pixels_to_world_y(canvas_center, visible_rect.top()),
            self.pixels_to_world_y(canvas_center, visible_rect.bottom()),
        );
        let (left_edge_world, right_edge_world) = (
            self.pixels_to_world_x(canvas_center, visible_rect.left()),
            self.pixels_to_world_x(canvas_center, visible_rect.right()),
        );

        for (grid_interval, color) in grid_intervals {
            // Draw vertical grid lines
            for x in ((left_edge_world / grid_interval).ceil() as i32)..=((right_edge_world / grid_interval).floor() as i32) {
                let grid_line_pixel = self.world_to_pixels_x(canvas_center, x as f32 * grid_interval);
                painter.line_segment(
                    [
                        Pos2::new(grid_line_pixel, visible_rect.top()),
                        Pos2::new(grid_line_pixel, visible_rect.bottom()),
                    ],
                    Stroke::new(1.5, color),
                );
            }

            // Draw horizontal grid lines
            for y in ((top_edge_world / grid_interval).ceil() as i32)..=((bottom_edge_world / grid_interval).floor() as i32) {
                let grid_line_pixel = self.world_to_pixels_y(canvas_center, y as f32 * grid_interval);
                painter.line_segment(
                    [
                        Pos2::new(visible_rect.left(), grid_line_pixel),
                        Pos2::new(visible_rect.right(), grid_line_pixel),
                    ],
                    Stroke::new(1.5, color),
                );
            }
        }
    }

    fn render_box(&self, painter: &Painter, canvas_center: Pos2) {
        let box_size = Vec2::new(2.0 * self.zoom, 2.0 * self.zoom);
        painter.rect_filled(
            Rect::from_center_size(self.world_to_pixels(canvas_center, Pos2::new(3.0, 1.0)), box_size),
            0.0,
            Color32::from_rgb(255, 0, 0),
        );
    }
}

impl eframe::App for HomeFlow {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        CentralPanel::default().show(ctx, |ui| {
            // ui.ctx().request_repaint();
            self.time += ui.input(|i| i.unstable_dt) as f64;

            let (response, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::drag());
            let canvas_center = response.rect.center();

            if response.dragged() {
                self.translation += response.drag_delta() * 0.01 / (self.zoom / 100.0);
            }

            let scroll_delta = ui.input(|i| i.scroll_delta);
            if scroll_delta != Vec2::ZERO {
                let zoom_amount = (scroll_delta.y.signum() * 15.0) * (self.zoom / 100.0);
                if let Some(mouse_pos) = ui.input(|i| i.pointer.latest_pos()) {
                    let mouse_world_before_zoom = self.pixels_to_world(canvas_center, mouse_pos);
                    self.zoom = (self.zoom + zoom_amount).clamp(20.0, 300.0);
                    let mouse_world_after_zoom = self.pixels_to_world(canvas_center, mouse_pos);
                    self.translation += mouse_world_after_zoom - mouse_world_before_zoom;
                } else {
                    self.zoom = (self.zoom + zoom_amount).clamp(20.0, 300.0);
                }
            }

            self.render_grid(&painter, &response.rect, canvas_center);
            self.render_box(&painter, canvas_center);
        });
    }
}
