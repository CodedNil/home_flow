use egui::{
    epaint::{Shadow, Vertex},
    CentralPanel, Color32, Context, Frame, Mesh, Painter, Pos2, Rect, Sense, Shape, Stroke, TextureId, Vec2, Window,
};
use egui_plot::{CoordinatesFormatter, Corner, Legend, Line, LineStyle, Plot, PlotPoints};
use std::collections::HashSet;

mod layout;

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
}

impl Default for HomeFlow {
    fn default() -> Self {
        Self {
            time: 0.0,
            translation: Vec2::ZERO,
            zoom: 100.0,
            layout: layout::Home::load(),
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
                Stroke::new(1.5, Color32::from_rgb(85, 85, 100)),
            ),
            (
                if self.zoom <= 40.0 {
                    1.0
                } else if self.zoom <= 80.0 {
                    0.5
                } else {
                    0.25
                },
                Stroke::new(1.5, Color32::from_rgb(55, 55, 70)),
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

        let mut rendered_vertical = HashSet::new();
        let mut rendered_horizontal = HashSet::new();
        let mut lines = Vec::new();
        for (grid_interval, stroke) in grid_intervals {
            // Draw vertical grid lines
            for x in ((left_edge_world / grid_interval).ceil() as i32)..=((right_edge_world / grid_interval).floor() as i32) {
                let grid_line_pixel = self.world_to_pixels_x(canvas_center, x as f32 * grid_interval);
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
            for y in ((top_edge_world / grid_interval).ceil() as i32)..=((bottom_edge_world / grid_interval).floor() as i32) {
                let grid_line_pixel = self.world_to_pixels_y(canvas_center, y as f32 * grid_interval);
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

    fn render_box(&self, painter: &Painter, canvas_center: Pos2, pos: Pos2, size: Vec2, color: Color32) {
        let box_size = Vec2::new(size.x * self.zoom, size.y * self.zoom);
        painter.rect_filled(
            Rect::from_center_size(self.world_to_pixels(canvas_center, pos), box_size),
            0.0,
            color,
        );
    }

    fn render_mesh(&self, painter: &Painter, canvas_center: Pos2, vertices: Vec<Pos2>, color: Color32) {
        let mut indices = Vec::new();
        for i in 0..vertices.len() {
            indices.push(i as u32);
        }
        let mut vertices_pixels = Vec::new();
        for vertex in vertices {
            vertices_pixels.push(Vertex {
                pos: self.world_to_pixels(canvas_center, vertex),
                color,
                ..Default::default()
            });
        }

        let mesh = Mesh {
            indices,
            vertices: vertices_pixels,
            texture_id: TextureId::Managed(0),
        };
        painter.add(Shape::mesh(mesh));
    }
}

impl eframe::App for HomeFlow {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    #[allow(clippy::too_many_lines)]
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
                // ui.ctx().request_repaint();
                self.time += ui.input(|i| i.unstable_dt) as f64;

                let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::drag());
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

                // Clamp translation to bounds
                let bounds = [-10.0, 10.0, -10.0, 10.0];
                let window_size_meters = ui.ctx().available_rect().size() / self.zoom / 2.0;

                // Calculate the minimum and maximum translation values
                let min_translation = Vec2::new(bounds[0] + window_size_meters.x, bounds[2] + window_size_meters.y);
                let max_translation = Vec2::new(bounds[1] - window_size_meters.x, bounds[3] - window_size_meters.y);

                // Check if minimum is less than or equal to maximum
                if min_translation.x <= max_translation.x {
                    self.translation.x = self.translation.x.clamp(min_translation.x, max_translation.x);
                } else {
                    self.translation.x = 0.0;
                }
                if min_translation.y <= max_translation.y {
                    self.translation.y = self.translation.y.clamp(min_translation.y, max_translation.y);
                } else {
                    self.translation.y = 0.0;
                }
                self.render_box(
                    &painter,
                    canvas_center,
                    Pos2::new((bounds[0] + bounds[1]) / 2.0, (bounds[2] + bounds[3]) / 2.0),
                    Vec2::new(bounds[1] - bounds[0], bounds[3] - bounds[2]),
                    Color32::from_rgb(0, 0, 100),
                );

                self.render_grid(&painter, &response.rect, canvas_center);

                for room in &self.layout.rooms {
                    let pos = room.pos;
                    let size = Vec2::new(room.size.x, room.size.y);
                    let size_half = size / 2.0;
                    let vertices = vec![
                        Pos2::new(pos.x - size_half.x, pos.y + size_half.y), // Bottom-left
                        Pos2::new(pos.x + size_half.x, pos.y + size_half.y), // Bottom-right
                        Pos2::new(pos.x + size_half.x, pos.y - size_half.y), // Top-right
                        Pos2::new(pos.x - size_half.x, pos.y - size_half.y), // Top-left
                    ];
                    self.render_mesh(&painter, canvas_center, vertices, Color32::from_rgb(255, 0, 0));
                }

                let plot_location = self.world_to_pixels(canvas_center, Pos2::new(3.0, 2.0));
                let plot_end_location = self.world_to_pixels(canvas_center, Pos2::new(8.0, 6.0));
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
                            .coordinates_formatter(Corner::LeftBottom, CoordinatesFormatter::default())
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
