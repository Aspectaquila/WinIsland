//! Colours and fonts.

use gpui::{hsla, rgb, rgba, Hsla, Rgba};

pub const FONT_TEXT: &str = "Segoe UI Variable Text";
pub const FONT_DISPLAY: &str = "Segoe UI Variable Display";
pub const FONT_MONO: &str = "Cascadia Mono";

pub fn hex(c: u32) -> Rgba {
    rgb(c)
}
pub fn hexa(c: u32) -> Rgba {
    rgba(c)
}
/// White with alpha
pub fn wa(a: f32) -> Hsla {
    hsla(0.0, 0.0, 1.0, a)
}
pub fn ba(a: f32) -> Hsla {
    hsla(0.0, 0.0, 0.0, a)
}

// iOS system colours (dark)
pub const GREEN: u32 = 0x30D158;
pub const ORANGE: u32 = 0xFF9F0A;
pub const BLUE: u32 = 0x0A84FF;
pub const PINK: u32 = 0xFF375F;
pub const RED: u32 = 0xFF453A;
pub const INDIGO: u32 = 0x5E5CE6;
pub const PURPLE: u32 = 0xBF5AF2;
pub const TEAL: u32 = 0x40C8E0;
pub const GREY: u32 = 0x8E8E93;
pub const GRAPHITE: u32 = 0x48484A;

pub const TEXT: u32 = 0xF5F5F7;
pub const DIM: u32 = 0x8E8E93;
pub const DIM_IOS: u32 = 0x98989F;
pub const DIM2: u32 = 0x636366;
pub const ACCENT: u32 = 0x8FD3FF;

pub fn tint_color(t: Option<(f32, f32, f32)>) -> Hsla {
    match t {
        Some((h, s, l)) => hsla(h, s, l, 1.0),
        None => rgb(ACCENT).into(),
    }
}

pub fn with_alpha(c: impl Into<Hsla>, a: f32) -> Hsla {
    let mut c = c.into();
    c.a = a;
    c
}
