use super::layout::{Furniture, Home, Room};

impl std::fmt::Display for Home {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut string = String::new();
        for room in &self.rooms {
            string.push_str(format!("{room}\n").as_str());
        }
        write!(f, "{string}")
    }
}

impl std::fmt::Display for Room {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut string = format!(
            "Room: {} - {}x{} @ {}x{}\n",
            self.name, self.size.x, self.size.y, self.pos.x, self.pos.y
        );
        for operation in &self.operations {
            // string.push_str(format!("    Operation: {}\n", operation).as_str());
        }

        // Walls
        string.push_str("    Walls: ");
        for (index, wall) in self.walls.iter().enumerate() {
            let side = match index {
                0 => "Left",
                1 => "Top",
                2 => "Right",
                3 => "Bottom",
                _ => "Unknown",
            };
            string.push_str(format!("{side}: {wall} ").as_str());
        }
        string.push('\n');

        write!(f, "{string}")
    }
}

impl std::fmt::Display for Furniture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut string = format!(
            "Furniture: {}x{} @ {}x{}",
            self.size.x, self.size.y, self.pos.x, self.pos.y
        );
        if self.rotation != 0.0 {
            string.push_str(format!(" - {}°", self.rotation).as_str());
        }
        string.push('\n');

        for child in &self.children {
            string.push_str(format!("    Child: {child}\n").as_str());
        }

        write!(f, "{string}")
    }
}
