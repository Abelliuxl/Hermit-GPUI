use gpui::{hsla, rgb, Hsla};

/// Fixed dark theme for Hermit GPUI (v1). Derived from the SwiftUI app's
/// visual language but tuned for a GPUI-rendered dark surface.
pub struct Theme;

impl Theme {
    pub fn window_bg() -> Hsla {
        rgb(0x141416).into()
    }
    pub fn sidebar_bg() -> Hsla {
        rgb(0x1a1a1e).into()
    }
    pub fn surface() -> Hsla {
        rgb(0x202024).into()
    }
    pub fn surface_hover() -> Hsla {
        rgb(0x28282e).into()
    }
    pub fn input_bg() -> Hsla {
        rgb(0x1c1c21).into()
    }
    pub fn border() -> Hsla {
        rgb(0x2c2c33).into()
    }
    pub fn border_strong() -> Hsla {
        rgb(0x3a3a44).into()
    }
    pub fn text() -> Hsla {
        rgb(0xe8e8ec).into()
    }
    pub fn text_secondary() -> Hsla {
        rgb(0x9b9ba6).into()
    }
    pub fn text_tertiary() -> Hsla {
        rgb(0x6d6d78).into()
    }
    pub fn accent() -> Hsla {
        rgb(0x4f8cff).into()
    }
    pub fn accent_soft() -> Hsla {
        rgba_hex(0x4f8cff, 0.16)
    }
    pub fn user_bubble() -> Hsla {
        rgb(0x2c3b58).into()
    }
    pub fn tool_bg() -> Hsla {
        rgba_hex(0xffffff, 0.05)
    }
    pub fn clarify_bg() -> Hsla {
        rgba_hex(0x4f8cff, 0.10)
    }
    pub fn danger() -> Hsla {
        rgb(0xff5f57).into()
    }
    pub fn warn() -> Hsla {
        rgb(0xf5a623).into()
    }
    pub fn ok() -> Hsla {
        rgb(0x34c759).into()
    }
    pub fn code_bg() -> Hsla {
        rgb(0x1b1b20).into()
    }
    pub fn quote_bar() -> Hsla {
        rgba_hex(0xffffff, 0.25)
    }
}

pub fn rgba_hex(value: u32, alpha: f32) -> Hsla {
    let r = ((value >> 16) & 0xff) as f32 / 255.0;
    let g = ((value >> 8) & 0xff) as f32 / 255.0;
    let b = (value & 0xff) as f32 / 255.0;
    hsla(r, g, b, alpha)
}
