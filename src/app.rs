mod edit_mode;
mod edit_mode_render;
mod edit_mode_utils;
mod interaction;
mod presence_sensor;
mod render;

use self::{
    edit_mode::{EditDetails, EditResponse},
    interaction::IState,
};
use crate::{
    common::{
        layout::Home,
        utils::{rotate_point, rotate_point_pivot},
    },
    server::{
        common_api::{get_layout, get_states, login, post_state},
        PostServicesData, StatesPacket,
    },
};
use anyhow::Result;
use egui::{Align2, CentralPanel, Color32, Context, Frame, Sense, TextEdit, TextureHandle, Window};
use egui_notify::Toasts;
use glam::{dvec2 as vec2, DVec2 as Vec2};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc, time::Duration};

static HOME_ASSISTANT_STATE_REFRESH: f64 = 1.0;
static HOME_ASSISTANT_STATE_LOCAL_OVERRIDE: f64 = 5.0;
static HOME_ASSISTANT_STATE_POST_EVERY: f64 = 0.1;

nestify::nest! {
    pub struct HomeFlow {
        time: f64,
        frame_time: f64,

        canvas_center: Vec2,
        mouse_pos: Vec2,
        mouse_pos_world: Vec2,
        is_mobile: bool,

        layout_server: Home,
        layout: Home,
        textures: HashMap<String, TextureHandle>,
        light_data: Option<(u64, TextureHandle)>,
        bounds: (Vec2, Vec2),
        rotate_key_down: bool,
        rotate_speed: f64,
        rotate_target: f64,
        interaction_state: IState,

        toasts: Arc<Mutex<Toasts>>,
        edit_mode: EditDetails,
        host: String,

        #>[derive(Deserialize, Serialize, Debug)]
        #>[serde(default)]
        stored: pub struct StoredData {
            auth_token: String,
            translation: Vec2,
            zoom: f64, // Zoom is meter to pixels
            rotation: f64,
        },

        login_form: struct LoginForm {
            username: String,
            password: String,
        },

        #>[derive(Default)]*
        network_data: Arc<Mutex<struct DownloadData {
            layout: enum DownloadLayout {
                #[default]
                None,
                InProgress,
                Done(Result<Home>),
            },
            hass_states: enum DownloadStates {
                #[default]
                None,
                Waiting(f64),
                InProgress,
                Done(Result<StatesPacket>),
            },
            hass_post: enum UploadStates {
                #[default]
                None,
                Waiting(f64),
                InProgress,
            },
            login: enum LoginState {
                #[default]
                None,
                InProgress,
                Done(Result<String>),
            },
        }>>,

        post_queue: Vec<PostServicesData>,
    }
}

impl Default for StoredData {
    fn default() -> Self {
        Self {
            auth_token: String::new(),
            translation: Vec2::ZERO,
            zoom: 100.0,
            rotation: 0.0,
        }
    }
}

impl HomeFlow {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let stored = cc.storage.map_or_else(StoredData::default, |storage| {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        });

        let rotation = ((stored.rotation / 90.0).round() * 90.0).rem_euclid(360.0);
        Self {
            time: 0.0,
            frame_time: 0.0,
            canvas_center: Vec2::ZERO,
            mouse_pos: Vec2::ZERO,
            mouse_pos_world: Vec2::ZERO,
            is_mobile: false,

            layout_server: Home::empty(),
            layout: Home::empty(),
            textures: HashMap::new(),
            light_data: None,
            bounds: (Vec2::ZERO, Vec2::ZERO),
            rotate_key_down: false,
            rotate_speed: 0.0,
            rotate_target: rotation,
            interaction_state: IState::default(),

            toasts: Arc::new(Mutex::new(Toasts::default())),
            edit_mode: EditDetails::default(),
            host: "localhost:8127".to_string(),
            stored: StoredData { rotation, ..stored },
            login_form: LoginForm {
                username: String::new(),
                password: String::new(),
            },
            network_data: Arc::new(Mutex::new(DownloadData::default())),
            post_queue: Vec::new(),
        }
    }

    fn screen_to_world(&self, v: Vec2) -> Vec2 {
        let pivot = vec2(-self.stored.translation.x, self.stored.translation.y);
        rotate_point_pivot(
            vec2(
                (v.x - self.canvas_center.x) / self.stored.zoom - self.stored.translation.x,
                (self.canvas_center.y - v.y) / self.stored.zoom + self.stored.translation.y,
            ),
            pivot,
            -self.stored.rotation,
        )
    }

    fn world_to_screen(&self, v: Vec2) -> Vec2 {
        let pivot = vec2(-self.stored.translation.x, self.stored.translation.y);
        let v = rotate_point_pivot(v, pivot, self.stored.rotation);
        vec2(
            (v.x + self.stored.translation.x) * self.stored.zoom + self.canvas_center.x,
            (self.stored.translation.y - v.y) * self.stored.zoom + self.canvas_center.y,
        )
    }
    fn world_to_screen_pos(&self, v: Vec2) -> egui::Pos2 {
        let v = self.world_to_screen(v);
        egui::pos2(v.x as f32, v.y as f32)
    }

    fn handle_pan_zoom(&mut self, response: &egui::Response, ui: &egui::Ui) {
        if !(self.bounds.0.is_finite()
            && self.bounds.1.is_finite()
            && self.bounds.0.length() > 0.0
            && self.bounds.1.length() > 0.0)
        {
            return;
        }

        // Drag
        let pointer_button = if self.edit_mode.enabled {
            egui::PointerButton::Secondary
        } else {
            egui::PointerButton::Primary
        };
        let mut translation_delta = if response.dragged_by(pointer_button) {
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
        let mut interaction_rotated = false;
        let mut multi_touch_rotation = 0.0;
        if let Some(multi_touch) = ui.ctx().multi_touch() {
            is_multi_touch = true;
            interaction_rotated = true;
            scroll_delta = (f64::from(multi_touch.zoom_delta) - 1.0) * 80.0;
            translation_delta = egui_to_vec2(multi_touch.translation_delta) * 0.01;
            multi_touch_rotation = f64::from(multi_touch.rotation_delta);
        }
        if scroll_delta.abs() > 0.0 {
            let zoom_amount = scroll_delta * (self.stored.zoom / 100.0);
            let mouse_world_before_zoom = self.screen_to_world(self.mouse_pos);
            self.stored.zoom = (self.stored.zoom + zoom_amount).clamp(40.0, 300.0);
            let mouse_world_after_zoom = self.screen_to_world(self.mouse_pos);
            let difference = mouse_world_after_zoom - mouse_world_before_zoom;
            self.stored.translation += Vec2::new(difference.x, -difference.y);
        }

        if translation_delta.length() > 0.0 {
            let rotated = rotate_point(translation_delta, self.stored.rotation);
            self.stored.translation += rotated / (self.stored.zoom / 100.0);
        }

        let (q_down, e_down) = ui.input(|i| (i.key_down(egui::Key::Q), i.key_down(egui::Key::E)));
        let max_speed = 800.0;
        if q_down || e_down {
            let rotation_delta = if q_down { 1.0 } else { -1.0 };
            self.rotate_speed = (self.rotate_speed + rotation_delta * 400.0 * self.frame_time)
                .clamp(-max_speed, max_speed);
            interaction_rotated = true;
        } else if is_multi_touch {
            self.stored.rotation -= multi_touch_rotation.to_degrees();
            self.rotate_speed = 0.0;
        }
        if interaction_rotated && !self.rotate_key_down {
            self.rotate_key_down = true;
            self.rotate_target = 0.0;
        } else if !interaction_rotated && self.rotate_key_down {
            self.rotate_key_down = false;
            // Determine the nearest 90 degree snap target based on current rotation
            let inertia = (self.rotate_speed * 0.25).clamp(-max_speed * 0.1, max_speed * 0.1);
            self.rotate_target = ((self.stored.rotation + inertia) / 90.0).round() * 90.0;
        }
        if !(q_down || e_down || is_multi_touch) {
            let rotation_diff = self.rotate_target - self.stored.rotation;

            // Adjust rotate speed towards the needed speed for snapping, within the max speed limit
            let needed_speed = rotation_diff * self.frame_time * 500.0;
            self.rotate_speed = if rotation_diff.abs() > 0.1 {
                needed_speed.clamp(-max_speed, max_speed)
            } else {
                self.stored.rotation = self.rotate_target.rem_euclid(360.0);
                self.rotate_target = self.stored.rotation;
                0.0
            };
        }

        // Apply rotation if there's any rotate speed
        if self.rotate_speed.abs() > 0.0 {
            self.stored.rotation += self.rotate_speed * self.frame_time;
        }

        // Clamp translation to bounds
        self.stored.translation = self.stored.translation.clamp(self.bounds.0, self.bounds.1);
    }

    fn load_layout(&mut self) {
        // Load layout from server if needed
        if !self.layout.version.is_empty() {
            return;
        }
        let network_store = self.network_data.clone();
        let mut network_data_guard = network_store.lock();
        match &network_data_guard.layout {
            DownloadLayout::None => {
                log::info!("Loading layout from server");
                network_data_guard.layout = DownloadLayout::InProgress;
                drop(network_data_guard);
                get_layout(&self.host, &self.stored.auth_token, move |res| {
                    network_store.lock().layout = DownloadLayout::Done(res);
                });
            }
            DownloadLayout::InProgress => {}
            DownloadLayout::Done(ref response) => {
                match response {
                    Ok(layout) => {
                        log::info!("Loaded layout from server");
                        self.layout_server = layout.clone();
                        self.layout = layout.clone();
                    }
                    Err(e) => {
                        // If unauthorised, clear auth token and show login screen
                        if e.to_string().contains("status code: 401") {
                            self.stored.auth_token.clear();
                        }
                        log::error!("Failed to fetch layout: {:?}", e);
                    }
                }
                network_data_guard.layout = DownloadLayout::None;
            }
        }
    }

    fn get_states(&mut self) {
        let network_store = self.network_data.clone();
        let mut network_data_guard = network_store.lock();
        match &network_data_guard.hass_states {
            DownloadStates::None => {
                network_data_guard.hass_states = DownloadStates::InProgress;
                drop(network_data_guard);

                // Get list of sensors to fetch
                let mut sensors = Vec::new();
                for room in &self.layout.rooms {
                    for furniture in &room.furniture {
                        let wanted = furniture.wanted_sensors();
                        if !wanted.is_empty() {
                            sensors.extend(wanted);
                        }
                    }
                }

                get_states(&self.host, &self.stored.auth_token, &sensors, move |res| {
                    network_store.lock().hass_states = DownloadStates::Done(res);
                });
            }
            DownloadStates::Waiting(time) => {
                if self.time > *time {
                    network_data_guard.hass_states = DownloadStates::None;
                }
            }
            DownloadStates::InProgress => {}
            DownloadStates::Done(ref response) => {
                match response {
                    Ok(states) => {
                        // Update all data with the new state
                        for room in &mut self.layout.rooms {
                            for light in &mut room.lights {
                                // Update light if it hasn't been locally edited recently
                                if light.last_manual == 0.0
                                    || self.time
                                        > light.last_manual + HOME_ASSISTANT_STATE_LOCAL_OVERRIDE
                                {
                                    for light_packet in &states.lights {
                                        if light.entity_id == light_packet.entity_id {
                                            light.state = light_packet.state;
                                        }
                                    }
                                }
                            }
                            for furniture in &mut room.furniture {
                                for sensor in &mut furniture.wanted_sensors() {
                                    for sensor_packet in &states.sensors {
                                        if sensor == &sensor_packet.entity_id {
                                            furniture.hass_data.insert(
                                                sensor.clone(),
                                                sensor_packet.state.clone(),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        // If unauthorised, clear auth token and show login screen
                        if e.to_string().contains("status code: 401") {
                            self.stored.auth_token.clear();
                        }
                        log::error!("Failed to fetch states: {:?}", e);
                    }
                }
                network_data_guard.hass_states =
                    DownloadStates::Waiting(self.time + HOME_ASSISTANT_STATE_REFRESH);
            }
        }
    }

    fn post_states(&mut self) {
        if self.post_queue.is_empty() {
            return;
        }
        let network_store = self.network_data.clone();
        let mut network_data_guard = network_store.lock();
        match &network_data_guard.hass_post {
            UploadStates::None => {
                network_data_guard.hass_post = UploadStates::InProgress;
                drop(network_data_guard);
                let next_post = self.time;
                post_state(
                    &self.host,
                    &self.stored.auth_token,
                    &self.post_queue,
                    move |_| {
                        network_store.lock().hass_post =
                            UploadStates::Waiting(next_post + HOME_ASSISTANT_STATE_POST_EVERY);
                    },
                );
                self.post_queue.clear();
            }
            UploadStates::Waiting(time) => {
                if self.time > *time {
                    network_data_guard.hass_post = UploadStates::None;
                }
            }
            UploadStates::InProgress => {}
        }
    }
}

impl eframe::App for HomeFlow {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.stored);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();

        #[cfg(target_arch = "wasm32")]
        {
            let web_info = &_frame.info().web_info;
            self.host = web_info.location.host.clone();
        }

        // Styling
        ctx.style_mut(|style| {
            style.visuals.window_shadow = egui::epaint::Shadow::NONE;
        });

        // If no auth token, show login screen
        if self.stored.auth_token.is_empty() {
            CentralPanel::default()
                .frame(Frame {
                    fill: Color32::from_rgb(25, 25, 35),
                    ..Default::default()
                })
                .show(ctx, |ui| {
                    let (response, _painter) =
                        ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
                    let canvas_center = egui_pos_to_vec2(response.rect.center());

                    Window::new("Login Form".to_string())
                        .fixed_pos(vec2_to_egui_pos(vec2(canvas_center.x, canvas_center.y)))
                        .fixed_size([300.0, 0.0])
                        .pivot(Align2::CENTER_CENTER)
                        .title_bar(false)
                        .resizable(false)
                        .show(ctx, |ui| {
                            ui.vertical_centered(|ui| {
                                let network_store = self.network_data.clone();
                                let mut network_data_guard = network_store.lock();
                                match &network_data_guard.login {
                                    LoginState::None => {
                                        ui.horizontal(|ui| {
                                            ui.label("Username:");
                                            TextEdit::singleline(&mut self.login_form.username)
                                                .show(ui);
                                        });
                                        ui.horizontal(|ui| {
                                            ui.label("Password:");
                                            TextEdit::singleline(&mut self.login_form.password)
                                                .password(true)
                                                .show(ui);
                                        });
                                        if ui.button("Login").clicked() {
                                            network_data_guard.login = LoginState::InProgress;
                                            drop(network_data_guard);
                                            login(
                                                &self.host,
                                                &self.login_form.username,
                                                &self.login_form.password,
                                                move |res| {
                                                    network_store.lock().login =
                                                        LoginState::Done(res);
                                                },
                                            );
                                        }
                                    }
                                    LoginState::InProgress => {
                                        ui.label("Logging in...");
                                        ui.add(egui::Spinner::new());
                                    }
                                    LoginState::Done(ref response) => {
                                        match response {
                                            Ok(response) => {
                                                if response.contains('|') {
                                                    // Split token on | to get message and token separately
                                                    let split: Vec<&str> =
                                                        response.split('|').collect();
                                                    let message = split[0];
                                                    let token = split[1];

                                                    let toasts_store = self.toasts.clone();
                                                    toasts_store
                                                        .lock()
                                                        .info(message)
                                                        .set_duration(Some(Duration::from_secs(3)));

                                                    self.stored.auth_token = token.to_string();
                                                } else {
                                                    // If no | is found, treat the entire response as the token
                                                    self.stored.auth_token.clone_from(response);
                                                }
                                            }
                                            Err(e) => {
                                                let toasts_store = self.toasts.clone();
                                                toasts_store
                                                    .lock()
                                                    .error(e.to_string())
                                                    .set_duration(Some(Duration::from_secs(3)));
                                            }
                                        }
                                        network_data_guard.login = LoginState::None;
                                    }
                                }
                            });
                        });

                    self.toasts.lock().show(ctx);
                });
            return;
        }

        self.load_layout();
        if self.layout.version.is_empty() {
            return;
        }
        self.get_states();
        self.post_states();

        CentralPanel::default()
            .frame(Frame {
                fill: Color32::from_rgb(25, 25, 35),
                ..Default::default()
            })
            .show(ctx, |ui| {
                self.time = ctx.input(|i| i.time);
                self.frame_time = f64::from(ui.input(|i| i.unstable_dt));

                let (response, painter) =
                    ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
                self.canvas_center = egui_pos_to_vec2(response.rect.center());

                let mouse_pos = ui
                    .input(|i| i.pointer.interact_pos())
                    .map_or(self.mouse_pos, egui_pos_to_vec2);
                self.mouse_pos = mouse_pos;
                self.mouse_pos_world = self.screen_to_world(mouse_pos);

                self.is_mobile = ctx.screen_rect().size().x < 550.0;

                let edit_mode_response = if self.is_mobile {
                    EditResponse {
                        used_dragged: false,
                        hovered_id: None,
                        snap_line_x: None,
                        snap_line_y: None,
                    }
                } else {
                    self.run_edit_mode(&response, ctx, ui)
                };
                if !edit_mode_response.used_dragged
                    && (self.interaction_state.light_drag.is_none()
                        || !self.interaction_state.light_drag.as_ref().unwrap().active)
                {
                    self.handle_pan_zoom(&response, ui);
                }

                self.render_layout(&painter, ctx);

                if !self.is_mobile && self.edit_mode.enabled {
                    self.paint_edit_mode(&painter, &edit_mode_response, ctx);
                } else {
                    self.interact_with_layout(&response, &painter);
                }

                if !self.is_mobile {
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
                }

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
