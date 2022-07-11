/// 0-360, 0-1, 0-1, 0-1
pub fn hsla(h: f32, s: f32, l: f32, a: f32) -> [f32; 4] {
    let converted = palette::rgb::Rgb::from_color(palette::Hsl::new(h, s, l)).into_components();
    [converted.0, converted.1, converted.2, a]
}

pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a]
}

mod basic_colors {
    pub type Color = [f32; 4];

    pub const RED: Color = [1.0, 0.0, 0.0, 1.0];
    pub const GREEN: Color = [0.0, 1.0, 0.0, 1.0];
    pub const BLUE: Color = [0.0, 0.0, 1.0, 1.0];
    pub const BLACK: Color = [0.0, 0.0, 0.0, 1.0];
    pub const WHITE: Color = [1.0; 4];
    pub const TRANSPARENT: Color = [0.0; 4];
}
mod colors {
    use super::basic_colors::Color;

    pub const LIGHT_GREY: Color = [0.8, 0.8, 0.8, 1.0];
    pub const DARK_GREY: Color = [0.2, 0.2, 0.2, 1.0];
    pub const LIGHT_TRANSPARENT_BLUE: Color = [107.0 / 255.0, 243.0 / 255.0, 243.0 / 255.0, 0.4];
}
pub use basic_colors::*;
pub use colors::*;
use palette::FromColor;
