// OKLCH warm-paper color system (light + dark).
// Stored as 0xRRGGBBAA u32 values; convert at the use site with `gpui::rgba(...)`.
//
// Each mode is built from a 12-step neutral ramp on warm hue 95° where step
// number encodes contrast role (identical semantics in both modes):
//   N1  sunken    — recessed chrome (sidebars, titlebar, panels)
//   N2  canvas    — main app / document background
//   N3  elevated  — floating surfaces (menus, popovers, cards)
//   N4  hover     — hovered items & controls
//   N5  active    — pressed / selected neutral surfaces
//   N6  b_subtle  — hairline structural dividers (chrome, menu edges)
//   N7  b_color   — default borders & splits
//   N8  b_strong  — interactive control & input borders
//   N9  faint     — disabled / quietest hint text
//   N10 muted     — placeholders, empty states, line numbers
//   N11 secondary — labels, metadata, captions
//   N12 primary   — main body & heading text

#[derive(Clone, Copy, Debug)]
pub struct Palette {
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
    /// High-contrast foreground for text/icons rendered on top of `accent`.
    pub accent_text: u32,
    pub selection: u32,

    // Status
    pub success: u32,
    pub warning: u32,
    pub error: u32,

    // Shadows (semi-transparent black)
    pub shadow: u32,
    pub shadow_subtle: u32,
}

// ========================================
// LIGHT — "Warm Paper Light"
// ========================================
pub const LIGHT: Palette = {
    const N1: u32 = 0xf2f2efff; // oklch(0.960 0.004 95) — sunken
    const N2: u32 = 0xf9f8f6ff; // oklch(0.980 0.003 95) — canvas
    const N3: u32 = 0xfdfdfcff; // oklch(0.994 0.002 95) — elevated
    const N4: u32 = 0xf1f0edff; // oklch(0.956 0.004 95) — hover
    const N5: u32 = 0xecece9ff; // oklch(0.942 0.004 95) — active
    const N6: u32 = 0xe6e5e1ff; // oklch(0.922 0.005 95) — border_subtle
    const N7: u32 = 0xdad9d5ff; // oklch(0.885 0.006 95) — border_color
    const N8: u32 = 0xc4c2bdff; // oklch(0.815 0.007 95) — border_strong
    const N9: u32 = 0xa6a5a0ff; // oklch(0.720 0.006 95) — text_faint
    const N10: u32 = 0x868581ff; // oklch(0.615 0.006 95) — text_muted
    const N11: u32 = 0x686764ff; // oklch(0.515 0.006 95) — text_secondary
    const N12: u32 = 0x201f1cff; // oklch(0.240 0.006 95) — text_primary

    Palette {
        bg_sunken: N1,
        bg_canvas: N2,
        bg_elevated: N3,
        bg_hover: N4,
        bg_active: N5,

        border_subtle: N6,
        border_color: N7,
        border_strong: N8,

        text_primary: N12,
        text_secondary: N11,
        text_muted: N10,
        text_faint: N9,

        accent: 0xa9d0fbff,      // oklch(0.844 0.073 251) — #A9D0FB
        accent_text: 0x071c47ff, // oklch(0.241 0.085 262) — #071C47
        selection: 0xa9d0fb55,

        success: 0x5d8758ff, // oklch(0.580 0.085 142)
        warning: 0xb58531ff, // oklch(0.650 0.115 78)
        error: 0xb95954ff,   // oklch(0.580 0.125 25)

        shadow: 0x00000012,
        shadow_subtle: 0x00000008,
    }
};

// ========================================
// DARK — "Warm Paper Dark"
// ========================================
pub const DARK: Palette = {
    const N1: u32 = 0x161512ff; // oklch(0.195 0.005 95) — sunken
    const N2: u32 = 0x1c1c19ff; // oklch(0.225 0.005 95) — canvas
    const N3: u32 = 0x232320ff; // oklch(0.255 0.005 95) — elevated
    const N4: u32 = 0x2b2a27ff; // oklch(0.285 0.005 95) — hover
    const N5: u32 = 0x343330ff; // oklch(0.320 0.005 95) — active
    const N6: u32 = 0x2c2b29ff; // oklch(0.290 0.005 95) — border_subtle
    const N7: u32 = 0x393834ff; // oklch(0.340 0.006 95) — border_color
    const N8: u32 = 0x4d4c47ff; // oklch(0.415 0.007 95) — border_strong
    const N9: u32 = 0x605f5bff; // oklch(0.485 0.006 95) — text_faint
    const N10: u32 = 0x7d7c78ff; // oklch(0.585 0.006 95) — text_muted
    const N11: u32 = 0x9e9d99ff; // oklch(0.695 0.006 95) — text_secondary
    const N12: u32 = 0xe2e1ddff; // oklch(0.910 0.006 95) — text_primary

    Palette {
        bg_sunken: N1,
        bg_canvas: N2,
        bg_elevated: N3,
        bg_hover: N4,
        bg_active: N5,

        border_subtle: N6,
        border_color: N7,
        border_strong: N8,

        text_primary: N12,
        text_secondary: N11,
        text_muted: N10,
        text_faint: N9,

        accent: 0x8fbff3ff,      // oklch(0.790 0.090 251)
        accent_text: 0x03163aff, // oklch(0.210 0.075 261)
        selection: 0x8fbff338,

        success: 0x86b281ff, // oklch(0.720 0.085 142)
        warning: 0xd7a85bff, // oklch(0.760 0.110 78)
        error: 0xdf7f78ff,   // oklch(0.700 0.120 25)

        shadow: 0x00000040,
        shadow_subtle: 0x00000024,
    }
};
