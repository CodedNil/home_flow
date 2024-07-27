use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct Color(pub [u8; 4]);

impl std::ops::Index<usize> for Color {
    type Output = u8;

    #[inline]
    fn index(&self, index: usize) -> &u8 {
        &self.0[index]
    }
}

impl std::ops::IndexMut<usize> for Color {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut u8 {
        &mut self.0[index]
    }
}

#[allow(dead_code)]
impl Color {
    pub const TRANSPARENT: Self = Self::from_rgba_premultiplied(0, 0, 0, 0);
    pub const BLACK: Self = Self::from_rgb(0, 0, 0);
    pub const GRAY: Self = Self::from_rgb(160, 160, 160);
    pub const WHITE: Self = Self::from_rgb(255, 255, 255);

    pub const RED: Self = Self::from_rgb(255, 0, 0);
    pub const GREEN: Self = Self::from_rgb(0, 255, 0);
    pub const BLUE: Self = Self::from_rgb(0, 0, 255);
    pub const YELLOW: Self = Self::from_rgb(255, 255, 0);

    #[inline]
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self([r, g, b, 255])
    }

    #[inline]
    pub fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        if a == 255 {
            Self::from_rgb(r, g, b)
        } else if a == 0 {
            Self::TRANSPARENT
        } else {
            let r_lin = linear_f32_from_gamma_u8(r);
            let g_lin = linear_f32_from_gamma_u8(g);
            let b_lin = linear_f32_from_gamma_u8(b);
            let a_lin = linear_f32_from_linear_u8(a);

            let r = gamma_u8_from_linear_f32(r_lin * a_lin);
            let g = gamma_u8_from_linear_f32(g_lin * a_lin);
            let b = gamma_u8_from_linear_f32(b_lin * a_lin);

            Self::from_rgba_premultiplied(r, g, b, a)
        }
    }

    #[inline]
    pub const fn from_rgba_premultiplied(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self([r, g, b, a])
    }

    #[inline]
    pub const fn from_gray(l: u8) -> Self {
        Self([l, l, l, 255])
    }

    #[inline]
    pub const fn from_alpha(a: u8) -> Self {
        Self([0, 0, 0, a])
    }

    #[inline]
    pub const fn is_opaque(self) -> bool {
        self.a() == 255
    }

    #[inline]
    pub const fn r(self) -> u8 {
        self.0[0]
    }

    #[inline]
    pub const fn g(self) -> u8 {
        self.0[1]
    }

    #[inline]
    pub const fn b(self) -> u8 {
        self.0[2]
    }

    #[inline]
    pub const fn a(self) -> u8 {
        self.0[3]
    }

    #[inline]
    pub const fn to_opaque(self) -> Self {
        let Self([r, g, b, _]) = self;
        Self([r, g, b, 255])
    }

    #[inline]
    pub const fn to_array(self) -> [u8; 4] {
        [self.r(), self.g(), self.b(), self.a()]
    }

    #[inline]
    pub fn mut_array(&mut self) -> &mut [u8; 4] {
        &mut self.0
    }

    #[inline]
    pub const fn to_tuple(self) -> (u8, u8, u8, u8) {
        (self.r(), self.g(), self.b(), self.a())
    }

    /// Multiply with 0.5 to make color half as opaque, perceptually.
    #[inline]
    pub fn gamma_multiply(self, factor: f32) -> Self {
        if (factor - 1.0).abs() < f32::EPSILON {
            return self;
        } else if factor == 0.0 {
            return Self::TRANSPARENT;
        }
        let Self([r, g, b, a]) = self;
        Self([
            (f32::from(r) * factor + 0.5) as u8,
            (f32::from(g) * factor + 0.5) as u8,
            (f32::from(b) * factor + 0.5) as u8,
            (f32::from(a) * factor + 0.5) as u8,
        ])
    }

    #[inline]
    pub fn saturate(self, factor: f64) -> Self {
        let mut new_color = self.0;
        let avg =
            (f64::from(new_color[0]) + f64::from(new_color[1]) + f64::from(new_color[2])) / 3.0;

        for c in new_color.iter_mut().take(3) {
            let difference = (f64::from(*c) - avg) * (1.0 + factor);
            *c = (avg + difference).clamp(0.0, 255.0) as u8;
        }

        Self(new_color)
    }

    #[inline]
    pub fn lighten(self, factor: f64) -> Self {
        let mut new_color = self.0;
        for c in new_color.iter_mut().take(3) {
            *c = (f64::from(*c) * (1.0 + factor * 2.0)).clamp(0.0, 255.0) as u8;
        }
        Self(new_color)
    }

    #[cfg(feature = "gui")]
    pub const fn to_egui(self) -> egui::Color32 {
        egui::Color32::from_rgba_premultiplied(self.r(), self.g(), self.b(), self.a())
    }
}

fn linear_f32_from_gamma_u8(s: u8) -> f32 {
    if s <= 10 {
        f32::from(s) / 3294.6
    } else {
        ((f32::from(s) + 14.025) / 269.025).powf(2.4)
    }
}

fn gamma_u8_from_linear_f32(l: f32) -> u8 {
    if l <= 0.0 {
        0
    } else if l <= 0.003_130_8 {
        fast_round(3294.6 * l)
    } else if l <= 1.0 {
        fast_round(269.025 * l.powf(1.0 / 2.4) - 14.025)
    } else {
        255
    }
}

fn fast_round(r: f32) -> u8 {
    (r + 0.5) as _
}

#[inline]
pub fn linear_f32_from_linear_u8(a: u8) -> f32 {
    f32::from(a) / 255.0
}
