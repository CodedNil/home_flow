use egui::{
    epaint::Vertex, CentralPanel, Color32, Context, Key, Mesh, Painter, Pos2, Rect, Shape, Stroke,
    TextureId, Vec2,
};

const PIXEL_TO_METER: f32 = 0.01;

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct HomeFlow {
    #[serde(skip)]
    pan: Vec2,
    #[serde(skip)]
    zoom: f32,
    #[serde(skip)]
    rotation: f32,
}

impl Default for HomeFlow {
    fn default() -> Self {
        Self {
            pan: Vec2::ZERO,
            zoom: 1.0,
            rotation: 0.0,
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
}

impl eframe::App for HomeFlow {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        CentralPanel::default().show(ctx, |ui| {
            let (response, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::drag());

            if response.dragged() {
                self.pan += response.drag_delta();
            }

            let scroll_delta = ui.input(|i| i.scroll_delta);
            if scroll_delta != Vec2::ZERO {
                self.zoom = (self.zoom + scroll_delta.y / 100.0).clamp(0.1, 10.0);
            }

            if ui.input(|i| i.key_pressed(Key::Q)) {
                self.rotation -= 5.0; // Rotate counter-clockwise
            }
            if ui.input(|i| i.key_pressed(Key::E)) {
                self.rotation += 5.0; // Rotate clockwise
            }

            render_grid(&painter, &response.rect, self.pan, self.zoom, self.rotation);
            render_box(&painter, &response.rect, self.pan, self.zoom, self.rotation);
        });
    }
}

fn render_grid(painter: &Painter, visible_rect: &Rect, pan: Vec2, zoom: f32, rotation: f32) {
    let center_of_painter = Vec2::new(visible_rect.width() / 2.0, visible_rect.height() / 2.0);
    let grid_spacing = 1.0 * zoom / PIXEL_TO_METER;

    // Start and end points for grid lines
    let left = visible_rect.left() - pan.x;
    let right = visible_rect.right() - pan.x;
    let top = visible_rect.top() - pan.y;
    let bottom = visible_rect.bottom() - pan.y;

    // Horizontal lines
    let mut y = top - top % grid_spacing - center_of_painter.y;
    while y <= bottom {
        let line_start = Pos2::new(left, y);
        let line_end = Pos2::new(right, y);
        painter.line_segment(
            [line_start + pan, line_end + pan],
            Stroke::new(1.0, Color32::GRAY),
        );
        y += grid_spacing;
    }

    // Vertical lines
    let mut x = left - left % grid_spacing - center_of_painter.x;
    while x <= right {
        let line_start = Pos2::new(x, top);
        let line_end = Pos2::new(x, bottom);
        painter.line_segment(
            [line_start + pan, line_end + pan],
            Stroke::new(1.0, Color32::GRAY),
        );
        x += grid_spacing;
    }
}

fn render_box(painter: &Painter, visible_rect: &Rect, pan: Vec2, zoom: f32, rotation: f32) {
    let center_of_painter = Vec2::new(visible_rect.width() / 2.0, visible_rect.height() / 2.0);
    let box_size = Vec2::new(2.0 * zoom / PIXEL_TO_METER, 2.0 * zoom / PIXEL_TO_METER);
    let center = Pos2::new(0.0, 0.0) + center_of_painter + pan;

    // Calculate the rotated corners of the box
    let mut corners = [
        Pos2::new(center.x - box_size.x / 2.0, center.y - box_size.y / 2.0),
        Pos2::new(center.x + box_size.x / 2.0, center.y - box_size.y / 2.0),
        Pos2::new(center.x + box_size.x / 2.0, center.y + box_size.y / 2.0),
        Pos2::new(center.x - box_size.x / 2.0, center.y + box_size.y / 2.0),
    ];

    let rotation_rad = rotation.to_radians();
    for corner in &mut corners {
        *corner = rotate_point(*corner, center, rotation_rad);
    }

    // Create vertices for the mesh
    let vertices = corners
        .iter()
        .map(|&pos| Vertex {
            pos,
            uv: Pos2::ZERO,
            color: Color32::RED,
        })
        .collect::<Vec<_>>();

    // Define indices for two triangles that make up the box
    let indices = vec![0, 1, 2, 0, 2, 3]; // Two triangles covering the box area

    // Create the mesh
    let mesh = Mesh {
        indices,
        vertices,
        texture_id: TextureId::Managed(0), // Default texture, since we're only using colors
    };

    // Add the mesh to the painter
    painter.add(Shape::mesh(mesh));
}

// Function to rotate a point around a pivot
fn rotate_point(point: Pos2, pivot: Pos2, angle_rad: f32) -> Pos2 {
    let cos_angle = angle_rad.cos();
    let sin_angle = angle_rad.sin();

    let translated_point = point - pivot;
    Pos2 {
        x: translated_point.x * cos_angle - translated_point.y * sin_angle + pivot.x,
        y: translated_point.x * sin_angle + translated_point.y * cos_angle + pivot.y,
    }
}
