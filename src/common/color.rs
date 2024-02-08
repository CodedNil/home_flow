use palette::{Hsla, IntoColor, Srgba};
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
    pub const DARK_GRAY: Self = Self::from_rgb(96, 96, 96);
    pub const GRAY: Self = Self::from_rgb(160, 160, 160);
    pub const LIGHT_GRAY: Self = Self::from_rgb(220, 220, 220);
    pub const WHITE: Self = Self::from_rgb(255, 255, 255);

    pub const BROWN: Self = Self::from_rgb(165, 42, 42);
    pub const DARK_RED: Self = Self::from_rgb(0x8B, 0, 0);
    pub const RED: Self = Self::from_rgb(255, 0, 0);
    pub const LIGHT_RED: Self = Self::from_rgb(255, 128, 128);

    pub const YELLOW: Self = Self::from_rgb(255, 255, 0);
    pub const LIGHT_YELLOW: Self = Self::from_rgb(255, 255, 0xE0);
    pub const KHAKI: Self = Self::from_rgb(240, 230, 140);

    pub const DARK_GREEN: Self = Self::from_rgb(0, 0x64, 0);
    pub const GREEN: Self = Self::from_rgb(0, 255, 0);
    pub const LIGHT_GREEN: Self = Self::from_rgb(0x90, 0xEE, 0x90);

    pub const DARK_BLUE: Self = Self::from_rgb(0, 0, 0x8B);
    pub const BLUE: Self = Self::from_rgb(0, 0, 255);
    pub const LIGHT_BLUE: Self = Self::from_rgb(0xAD, 0xD8, 0xE6);

    pub const GOLD: Self = Self::from_rgb(255, 215, 0);

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
        let Self([r, g, b, a]) = self;
        Self([
            (r as f32 * factor + 0.5) as u8,
            (g as f32 * factor + 0.5) as u8,
            (b as f32 * factor + 0.5) as u8,
            (a as f32 * factor + 0.5) as u8,
        ])
    }

    #[inline]
    pub fn saturate(self, factor: f64) -> Self {
        let mut hsl: Hsla = Srgba::new(
            self.0[0] as f32 / 255.0,
            self.0[1] as f32 / 255.0,
            self.0[2] as f32 / 255.0,
            self.0[3] as f32 / 255.0,
        )
        .into_color();
        hsl.saturation = (hsl.saturation + factor as f32).clamp(0.0, 1.0);
        let new_color: Srgba = Hsla::into_color(hsl);
        Self([
            (new_color.red * 255.0) as u8,
            (new_color.green * 255.0) as u8,
            (new_color.blue * 255.0) as u8,
            (new_color.alpha * 255.0) as u8,
        ])
    }

    #[inline]
    pub fn lighten(self, factor: f64) -> Self {
        let mut hsl: Hsla = Srgba::new(
            self.0[0] as f32 / 255.0,
            self.0[1] as f32 / 255.0,
            self.0[2] as f32 / 255.0,
            self.0[3] as f32 / 255.0,
        )
        .into_color();
        hsl.lightness = (hsl.lightness + factor as f32).clamp(0.0, 1.0);
        let new_color: Srgba = Hsla::into_color(hsl);
        Self([
            (new_color.red * 255.0) as u8,
            (new_color.green * 255.0) as u8,
            (new_color.blue * 255.0) as u8,
            (new_color.alpha * 255.0) as u8,
        ])
    }

    #[cfg(feature = "gui")]
    pub const fn to_egui(self) -> egui::Color32 {
        egui::Color32::from_rgba_premultiplied(self.r(), self.g(), self.b(), self.a())
    }
}

fn linear_f32_from_gamma_u8(s: u8) -> f32 {
    if s <= 10 {
        s as f32 / 3294.6
    } else {
        ((s as f32 + 14.025) / 269.025).powf(2.4)
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
    a as f32 / 255.0
}
