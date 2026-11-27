use super::color::{self, Palette};
use gpui::{BoxShadow, FontWeight, Point, px, rgba};

pub const FONT_UI: &str = "Google Sans Flex";
pub const FONT_MONO: &str = "Geist Mono";
pub const FONT_SERIF: &str = "Libre Baskerville";
// Default face for previewed text (overridable per file type).
pub const FONT_TEXT: &str = "Google Sans Flex";

/// Standard UI label weight for tabs and panel chrome (Google Sans Flex
/// renders cleanly at regular weight; use SEMIBOLD for headings/emphasis).
pub const WEIGHT_MEDIUM: FontWeight = FontWeight::NORMAL;

// Fades an RGBA color's alpha byte in from transparent (0xRRGGBBAA colors).
pub fn fade_in(color: u32, k: f32) -> u32 {
    let a = ((color & 0xFF) as f32 * k).round() as u32;
    (color & 0xFFFFFF00) | a
}

// Replaces an RGBA color's alpha byte with `alpha` in [0.0, 1.0].
pub fn with_alpha(color: u32, alpha: f32) -> u32 {
    let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u32;
    (color & 0xFFFFFF00) | a
}

// Linear blend of two 0xRRGGBBAA colors (all four channels, including
// the alpha byte). Our palette stores alpha in the low byte, so the
// 6-digit 0xRRGGBB math used elsewhere does NOT apply here.
pub fn lerp_rgba(from: u32, to: u32, k: f32) -> u32 {
    let ch = |c: u32, shift: u32| ((c >> shift) & 0xFF) as f32;
    let mix = |shift: u32| (ch(from, shift) + (ch(to, shift) - ch(from, shift)) * k).round() as u32;
    (mix(24) << 24) | (mix(16) << 16) | (mix(8) << 8) | mix(0)
}

// Same as lerp_rgba; kept under the familiar name for call-site parity.
pub fn lerp_rgb(from: u32, to: u32, k: f32) -> u32 {
    lerp_rgba(from, to, k)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
}

#[derive(Clone, Copy)]
pub struct Theme {
    // Surfaces
    pub bg_sunken: u32,
    pub bg_canvas: u32,
    pub bg_elevated: u32,
    pub bg_hover: u32,
    pub bg_active: u32,

    // Borders
    pub border_subtle: u32,
    pub border_color: u32,
    pub border_strong: u32,

    // Text
    pub text_primary: u32,
    pub text_secondary: u32,
    pub text_muted: u32,
    pub text_faint: u32,

    // Accent & selection
    pub accent: u32,
    pub accent_text: u32,
    pub selection: u32,

    // Status
    pub success: u32,
    pub warning: u32,
    pub error: u32,

    // Shadows
    pub shadow: u32,
    pub shadow_subtle: u32,
}

impl Theme {
    const fn from_palette(p: Palette) -> Self {
        Self {
            bg_sunken: p.bg_sunken,
            bg_canvas: p.bg_canvas,
            bg_elevated: p.bg_elevated,
            bg_hover: p.bg_hover,
            bg_active: p.bg_active,

            border_subtle: p.border_subtle,
            border_color: p.border_color,
            border_strong: p.border_strong,

            text_primary: p.text_primary,
            text_secondary: p.text_secondary,
            text_muted: p.text_muted,
            text_faint: p.text_faint,

            accent: p.accent,
            accent_text: p.accent_text,
            selection: p.selection,

            success: p.success,
            warning: p.warning,
            error: p.error,

            shadow: p.shadow,
            shadow_subtle: p.shadow_subtle,
        }
    }

    // Super thin, subtle shadow for popups and floating surfaces.
    pub fn shadow_md(&self) -> BoxShadow {
        BoxShadow {
            color: rgba(self.shadow).into(),
            offset: Point {
                x: px(0.),
                y: px(0.75),
            },
            blur_radius: px(3.),
            spread_radius: px(0.),
            inset: false,
        }
    }

    // Barely-there shadow for hovered / active items.
    pub fn shadow_sm(&self) -> BoxShadow {
        BoxShadow {
            color: rgba(self.shadow_subtle).into(),
            offset: Point {
                x: px(0.),
                y: px(0.75),
            },
            blur_radius: px(1.5),
            spread_radius: px(0.),
            inset: false,
        }
    }

    // Large floating shadow for menus and popups.
    pub fn shadow_lg(&self) -> BoxShadow {
        BoxShadow {
            color: rgba(self.shadow).into(),
            offset: Point { x: px(0.), y: px(4.) },
            blur_radius: px(16.),
            spread_radius: px(0.),
            inset: false,
        }
    }

    pub fn light() -> Self {
        Self::from_palette(color::LIGHT)
    }

    pub fn dark() -> Self {
        Self::from_palette(color::DARK)
    }
}
