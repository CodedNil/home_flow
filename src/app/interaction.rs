use super::HomeFlow;

impl HomeFlow {
    pub fn interact_with_layout(&mut self, ctx: &egui::Context) {
        let mouse_click = ctx.input(|i| i.pointer.primary_clicked());
        if mouse_click {
            self.interact_with_layout_click();
        }
    }

    fn interact_with_layout_click(&mut self) {
        let mut light_clicked = None;
        for room in &self.layout.rooms {
            for light in &room.lights {
                let pos_world = room.pos + light.pos;
                let mouse_dist = self.mouse_pos_world.distance(pos_world) as f32;
                if mouse_dist < 0.2 {
                    light_clicked = Some(light.name.clone());
                }
            }
        }
        if let Some(light_name) = light_clicked {
            for room in &mut self.layout.rooms {
                for light in &mut room.lights {
                    if light.name == light_name {
                        light.state = if light.state < 130 { 255 } else { 0 };
                    }
                }
            }
        }
    }
}
