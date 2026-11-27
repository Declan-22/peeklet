//! Reusable pill-shaped tooltip with a compact keycap shortcut badge,
//! anchored directly below a toolbar control.

use gpui::{AnyElement, deferred, div, prelude::*, px, rgba};

use crate::theme::Theme;

/// Renders a pill tooltip centered directly underneath a control of width
/// `anchor_w` (at `top = 34px`).
pub fn render_anchored_tooltip(
    label: &'static str,
    shortcut: Option<&'static str>,
    anchor_w: f32,
    progress: f32,
    t: &Theme,
) -> AnyElement {
    const WRAP_W: f32 = 240.0;
    let is_dark = (t.bg_canvas >> 24) < 0x80;
    let keycap_bg = if is_dark { t.bg_sunken } else { t.bg_elevated };
    let pill = div()
        .opacity(progress.clamp(0.0, 1.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(5.))
        .pl(px(9.))
        .pr(px(if shortcut.is_some() { 9. } else { 12. }))
        .py(px(3.))
        .rounded(px(999.))
        .bg(rgba(t.bg_hover))
        .border_1()
        .border_color(rgba(t.border_subtle))
        .shadow(vec![t.shadow_md()])
        .font_family(crate::theme::FONT_UI)
        .text_size(px(14.))
        .text_color(rgba(t.text_primary))
        .whitespace_nowrap()
        .child(label)
        .when_some(shortcut, |el, key| {
            el.child(
                div()
                    .px(px(4.))
                    .h(px(16.))
                    .min_w(px(16.))
                    .rounded(px(5.))
                    .bg(rgba(keycap_bg))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(10.5))
                    .text_color(rgba(t.text_muted))
                    .child(key),
            )
        });

    deferred(
        div()
            .absolute()
            .top(px(34.))
            .left(px(anchor_w * 0.5 - WRAP_W * 0.5))
            .w(px(WRAP_W))
            .flex()
            .justify_center()
            .child(pill),
    )
    .with_priority(1100)
    .into_any_element()
}
