use egui::{remap, CentralPanel, Color32, Context, NumExt, Response, Slider, Ui};
use egui_plot::{Legend, Line, LineStyle, Plot, PlotPoints};
use std::f64::consts::TAU;

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct HomeFlow {
    label: String,

    #[serde(skip)]
    value: f32,

    #[serde(skip)]
    line_demo: LineDemo,
}

impl Default for HomeFlow {
    fn default() -> Self {
        Self {
            label: "Hello World!".to_owned(),
            value: 2.7,
            line_demo: LineDemo::default(),
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
            // The central panel the region left after adding TopPanel's and SidePanel's
            ui.heading("eframe template");

            ui.horizontal(|ui| {
                ui.label("Write something: ");
                ui.text_edit_singleline(&mut self.label);
            });

            ui.add(Slider::new(&mut self.value, 0.0..=10.0).text("value"));
            if ui.button("Increment").clicked() {
                self.value += 1.0;
            }

            ui.separator();

            self.line_demo.ui(ui);
        });
    }
}

#[derive(Copy, Clone, PartialEq)]
struct LineDemo {
    time: f64,
}

impl Default for LineDemo {
    fn default() -> Self {
        Self { time: 0.0 }
    }
}

impl LineDemo {
    fn circle(&self) -> Line {
        let n = 512;
        let circle_points: PlotPoints = (0..=n)
            .map(|i| {
                let t = remap(i as f64, 0.0..=(n as f64), 0.0..=TAU);
                let r = 1.5;
                [r * t.cos(), r * t.sin()]
            })
            .collect();
        Line::new(circle_points)
            .color(Color32::from_rgb(100, 200, 100))
            .style(LineStyle::Solid)
            .name("circle")
    }

    fn sin(&self) -> Line {
        let time = self.time;
        Line::new(PlotPoints::from_explicit_callback(
            move |x| 0.5 * (2.0 * x).sin() * time.sin(),
            ..,
            512,
        ))
        .color(Color32::from_rgb(200, 100, 100))
        .style(LineStyle::Solid)
        .name("wave")
    }

    fn thingy(&self) -> Line {
        let time = self.time;
        Line::new(PlotPoints::from_parametric_callback(
            move |t| ((2.0 * t + time).sin(), (3.0 * t).sin()),
            0.0..=TAU,
            256,
        ))
        .color(Color32::from_rgb(100, 150, 250))
        .style(LineStyle::Solid)
        .name("x = sin(2t), y = sin(3t)")
    }
}

impl LineDemo {
    fn ui(&mut self, ui: &mut Ui) -> Response {
        ui.ctx().request_repaint();
        self.time += ui.input(|i| i.unstable_dt).at_most(1.0 / 30.0) as f64;
        let plot = Plot::new("lines_demo")
            .show_axes(false)
            .show_grid(true)
            .data_aspect(1.0);
        plot.show(ui, |plot_ui| {
            plot_ui.line(self.circle());
            plot_ui.line(self.sin());
            plot_ui.line(self.thingy());
        })
        .response
    }
}
