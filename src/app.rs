use egui::{
    epaint::Vertex, CentralPanel, Color32, Context, Key, Mesh, Painter, Pos2, Rect, Shape, Stroke,
    TextureId, Vec2,
};

const PIXEL_TO_METER: f32 = 0.01;

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct HomeFlow {
    #[serde(skip)]
    translation: Vec2,
    #[serde(skip)]
    zoom: f32,
    #[serde(skip)]
    rotation: f32,
}

impl Default for HomeFlow {
    fn default() -> Self {
        Self {
            translation: Vec2::ZERO,
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
                self.translation += response.drag_delta();
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

            render_grid(
                &painter,
                &response.rect,
                self.translation,
                self.zoom,
                self.rotation,
            );
            render_box(
                &painter,
                &response.rect,
                self.translation,
                self.zoom,
                self.rotation,
            );
        });
    }
}

fn render_grid(
    painter: &Painter,
    visible_rect: &Rect,
    translation: Vec2,
    zoom: f32,
    rotation: f32,
) {
    let center_of_painter = Vec2::new(visible_rect.width() / 2.0, visible_rect.height() / 2.0);
    let grid_spacing = 1.0 * zoom / PIXEL_TO_METER;

    // Manually iterate over the range for both x and y
    let mut x = visible_rect.left() + center_of_painter.x + translation.x;
    while x < visible_rect.right() + center_of_painter.x + translation.x {
        let mut y = visible_rect.top() + center_of_painter.y + translation.y;
        while y < visible_rect.bottom() + center_of_painter.y + translation.y {
            // Calculate start and end points for horizontal and vertical lines
            let start_h = Pos2::new(x, visible_rect.top());
            let end_h = Pos2::new(x, visible_rect.bottom());
            let start_v = Pos2::new(visible_rect.left(), y);
            let end_v = Pos2::new(visible_rect.right(), y);

            // Apply rotation
            let rotated_h = rotate_points(start_h, end_h, rotation, center_of_painter);
            let rotated_v = rotate_points(start_v, end_v, rotation, center_of_painter);

            // Draw lines
            painter.line_segment(rotated_h.into(), Stroke::new(1.0, Color32::GRAY));
            painter.line_segment(rotated_v.into(), Stroke::new(1.0, Color32::GRAY));

            y += grid_spacing;
        }
        x += grid_spacing;
    }
}

// Helper function to rotate points around a pivot
fn rotate_points(start: Pos2, end: Pos2, angle: f32, pivot: Vec2) -> (Pos2, Pos2) {
    let angle_rad = angle.to_radians();
    let cos_angle = angle_rad.cos();
    let sin_angle = angle_rad.sin();

    let rotate = |point: Pos2| -> Pos2 {
        let translated_point = point - pivot;
        Pos2::new(
            cos_angle * translated_point.x - sin_angle * translated_point.y,
            sin_angle * translated_point.x + cos_angle * translated_point.y,
        ) + pivot
    };

    (rotate(start), rotate(end))
}

fn render_box(painter: &Painter, visible_rect: &Rect, translation: Vec2, zoom: f32, rotation: f32) {
    let center_of_painter = Vec2::new(visible_rect.width() / 2.0, visible_rect.height() / 2.0);
    let box_size = Vec2::new(2.0 * zoom / PIXEL_TO_METER, 2.0 * zoom / PIXEL_TO_METER);

    // Initial center of the box
    let initial_center =
        Pos2::new(3.0 * zoom / PIXEL_TO_METER, 0.0) + center_of_painter + translation;

    // Rotate the center around the pivot
    let rotated_center = rotate_point(
        initial_center,
        Pos2::new(center_of_painter.x, center_of_painter.y),
        rotation,
    );

    // Calculate the rotated corners of the box based on the rotated center
    let mut corners = [
        Pos2::new(
            rotated_center.x - box_size.x / 2.0,
            rotated_center.y - box_size.y / 2.0,
        ),
        Pos2::new(
            rotated_center.x + box_size.x / 2.0,
            rotated_center.y - box_size.y / 2.0,
        ),
        Pos2::new(
            rotated_center.x + box_size.x / 2.0,
            rotated_center.y + box_size.y / 2.0,
        ),
        Pos2::new(
            rotated_center.x - box_size.x / 2.0,
            rotated_center.y + box_size.y / 2.0,
        ),
    ];

    for corner in &mut corners {
        *corner = rotate_point(*corner, rotated_center, rotation);
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
fn rotate_point(point: Pos2, pivot: Pos2, angle: f32) -> Pos2 {
    let angle_rad = angle.to_radians();
    let cos_angle = angle_rad.cos();
    let sin_angle = angle_rad.sin();

    let translated_point = point - pivot;
    Pos2 {
        x: translated_point.x * cos_angle - translated_point.y * sin_angle + pivot.x,
        y: translated_point.x * sin_angle + translated_point.y * cos_angle + pivot.y,
    }
}
