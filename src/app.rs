use egui::{
    epaint::Shadow, Align2, CentralPanel, Color32, ColorImage, Context, Frame, Painter, Pos2, Rect,
    Sense, Shape, Stroke, TextureOptions, Vec2, Window,
};
use egui_plot::{CoordinatesFormatter, Corner, Legend, Line, LineStyle, Plot, PlotPoints};
use std::collections::{HashMap, HashSet};

mod layout;
mod shape;
mod wall_render;

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct HomeFlow {
    #[serde(skip)]
    time: f64,

    #[serde(skip)]
    translation: Vec2,
    #[serde(skip)]
    zoom: f32, // Zoom is meter to pixels

    #[serde(skip)]
    layout: layout::Home,

    #[serde(skip)]
    edit_mode: bool,
}

impl Default for HomeFlow {
    fn default() -> Self {
        Self {
            time: 0.0,
            translation: Vec2::ZERO,
            zoom: 100.0,
            layout: layout::Home::load(),
            edit_mode: false,
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

    fn pixels_to_world(&self, canvas_center: Pos2, x: f32, y: f32) -> Pos2 {
        Pos2::new(x - canvas_center.x, canvas_center.y - y) / self.zoom
            - Vec2::new(self.translation.x, -self.translation.y)
    }

    fn world_to_pixels(&self, canvas_center: Pos2, x: f32, y: f32) -> Pos2 {
        Pos2::new(x + self.translation.x, self.translation.y - y) * self.zoom
            + Vec2::new(canvas_center.x, canvas_center.y)
    }

    fn render_grid(&self, painter: &Painter, visible_rect: &Rect, canvas_center: Pos2) {
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
            self.pixels_to_world(canvas_center, 0.0, visible_rect.bottom())
                .y,
            self.pixels_to_world(canvas_center, 0.0, visible_rect.top())
                .y,
        );
        let (left_edge_world, right_edge_world) = (
            self.pixels_to_world(canvas_center, visible_rect.left(), 0.0)
                .x,
            self.pixels_to_world(canvas_center, visible_rect.right(), 0.0)
                .x,
        );

        let mut rendered_vertical = HashSet::new();
        let mut rendered_horizontal = HashSet::new();
        let mut lines = Vec::new();
        for (grid_interval, stroke) in grid_intervals {
            // Draw vertical grid lines
            for x in ((left_edge_world / grid_interval).ceil() as i32)
                ..=((right_edge_world / grid_interval).floor() as i32)
            {
                let grid_line_pixel = self
                    .world_to_pixels(canvas_center, x as f32 * grid_interval, 0.0)
                    .x;
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
                let grid_line_pixel = self
                    .world_to_pixels(canvas_center, 0.0, y as f32 * grid_interval)
                    .y;
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
}

impl eframe::App for HomeFlow {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        let inner_frame = Frame {
            shadow: Shadow::small_dark(),
            stroke: Stroke::new(4.0, Color32::from_rgb(60, 60, 60)),
            fill: Color32::from_rgb(27, 27, 27),
            ..Default::default()
        };
        CentralPanel::default()
            .frame(Frame {
                fill: Color32::from_rgb(35, 35, 50),
                ..Default::default()
            })
            .show(ctx, |ui| {
                ui.ctx().request_repaint();
                self.time += ui.input(|i| i.unstable_dt) as f64;

                let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::drag());
                let canvas_center = response.rect.center();

                // Drag
                if response.dragged() {
                    self.translation += response.drag_delta() * 0.01 / (self.zoom / 100.0);
                }

                // Zoom
                let scroll_delta = ui.input(|i| i.scroll_delta);
                if scroll_delta != Vec2::ZERO {
                    let zoom_amount = (scroll_delta.y.signum() * 15.0) * (self.zoom / 100.0);
                    if let Some(mouse_pos) = ui.input(|i| i.pointer.latest_pos()) {
                        let mouse_world_before_zoom =
                            self.pixels_to_world(canvas_center, mouse_pos.x, mouse_pos.y);
                        self.zoom = (self.zoom + zoom_amount).clamp(20.0, 300.0);
                        let mouse_world_after_zoom =
                            self.pixels_to_world(canvas_center, mouse_pos.x, mouse_pos.y);
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

                let mouse_pos = ui
                    .input(|i| i.pointer.latest_pos())
                    .map_or(Pos2::ZERO, |mouse_pos| mouse_pos);
                let mouse_pos_world = self.pixels_to_world(canvas_center, mouse_pos.x, mouse_pos.y);

                // self.render_grid(&painter, &response.rect, canvas_center);

                let mut update_rooms_render = HashMap::new();
                for room in &self.layout.rooms {
                    // Retrieve render from cache or render fresh and store in cache
                    let room_render = room.render.as_ref().map_or_else(
                        || {
                            let render = room.render();
                            update_rooms_render.insert(room.name.clone(), render);
                            update_rooms_render.get(&room.name).unwrap()
                        },
                        |render| render,
                    );

                    let canvas_size = room_render.texture.dimensions();
                    let egui_image = ColorImage::from_rgba_unmultiplied(
                        [canvas_size.0 as usize, canvas_size.1 as usize],
                        &room_render.texture,
                    );
                    let canvas_texture_id = ctx
                        .load_texture("noise", egui_image, TextureOptions::NEAREST)
                        .id();
                    let rect = Rect::from_center_size(
                        self.world_to_pixels(
                            canvas_center,
                            room_render.center.x,
                            room_render.center.y,
                        ),
                        Vec2::new(
                            room_render.size.x * self.zoom,
                            room_render.size.y * self.zoom,
                        ),
                    );
                    painter.image(
                        canvas_texture_id,
                        rect,
                        Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
                // Update cache if needed
                if !update_rooms_render.is_empty() {
                    for room in &mut self.layout.rooms {
                        if let Some(render) = update_rooms_render.get(&room.name) {
                            room.render = Some(render.clone());
                        }
                    }
                    self.layout.save_memory();
                }

                self.render_walls(&painter, canvas_center);

                // Edit mode logic
                if self.edit_mode {
                    for room in &self.layout.rooms {
                        let room_render = room.render.as_ref().unwrap();
                        // Render outline if mouse within the shape and in edit mode
                        if room.contains(mouse_pos_world.x, mouse_pos_world.y) {
                            let points = room_render
                                .vertices
                                .iter()
                                .map(|v| self.world_to_pixels(canvas_center, v.x, v.y))
                                .collect::<Vec<_>>();
                            painter.add(Shape::closed_line(
                                points,
                                Stroke::new(10.0, Color32::from_rgb(255, 255, 255)),
                            ));
                        }
                    }
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
                    .frame(inner_frame)
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut self.edit_mode, "Edit Mode");
                        });
                    });

                let plot_location = self.world_to_pixels(canvas_center, 4.0, -3.0);
                let plot_end_location = self.world_to_pixels(canvas_center, 8.0, -6.0);
                Window::new("Plot Window")
                    .fixed_pos(plot_location)
                    .fixed_size(plot_end_location - plot_location)
                    .title_bar(false)
                    .resizable(false)
                    .constrain(false)
                    .frame(inner_frame)
                    .show(ctx, |ui| {
                        Plot::new("lines_demo")
                            .legend(Legend::default())
                            .show_axes(false)
                            .data_aspect(1.0)
                            .allow_scroll(false)
                            .coordinates_formatter(
                                Corner::LeftBottom,
                                CoordinatesFormatter::default(),
                            )
                            .show(ui, |plot_ui| {
                                plot_ui.line(sin(self.time));
                            });
                    });
            });
    }
}

fn sin(time: f64) -> Line {
    Line::new(PlotPoints::from_explicit_callback(
        move |x| 0.5 * (2.0 * x).sin() * time.sin(),
        ..,
        512,
    ))
    .color(Color32::from_rgb(200, 100, 100))
    .style(LineStyle::Solid)
    .name("wave")
}
