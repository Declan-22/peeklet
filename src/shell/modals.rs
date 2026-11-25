//! Shared UI widgets for Settings and confirmation modals.

use gpui::{AnyElement, App, FontWeight, MouseButton, WeakEntity, div, prelude::*, px, rgba};

use crate::theme::{Theme, WEIGHT_MEDIUM};

use super::root::RootView;

pub(super) fn settings_tab_button(
    t: Theme,
    shell: WeakEntity<RootView>,
    index: usize,
    label: &'static str,
    active: bool,
) -> AnyElement {
    div()
        .id(format!("settings-tab-{}", index))
        .w_full()
        .px(px(10.))
        .py(px(6.))
        .rounded(px(7.))
        .cursor_pointer()
        .text_sm()
        .font_weight(if active {
            WEIGHT_MEDIUM
        } else {
            FontWeight::NORMAL
        })
        .text_color(rgba(if active {
            t.text_primary
        } else {
            t.text_secondary
        }))
        .border_1()
        .border_color(rgba(if active { t.border_color } else { 0x00000000 }))
        .bg(rgba(if active { t.bg_darker } else { 0x00000000 }))
        .hover(|s| s.bg(rgba(t.menu_hover_bg)))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            let _ = shell.update(cx, |root, cx| {
                root.settings_tab = index;
                cx.notify();
            });
        })
        .child(label)
        .into_any_element()
}

pub(super) fn settings_toggle(
    t: Theme,
    shell: WeakEntity<RootView>,
    label: &'static str,
    description: &'static str,
    value: bool,
    kind: &'static str,
    divider: bool,
) -> AnyElement {
    let shell2 = shell.clone();
    let mut row = div()
        .w_full()
        .py(px(12.))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(16.));
    if divider {
        row = row.border_b_1().border_color(rgba(t.border_color));
    }
    row.child(
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(
                div()
                    .text_sm()
                    .font_weight(WEIGHT_MEDIUM)
                    .text_color(rgba(t.text_primary))
                    .child(label),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgba(t.text_secondary))
                    .child(description),
            ),
    )
    .child(
        div()
            .id(format!("settings-toggle-{}", label))
            .relative()
            .flex_shrink_0()
            .w(px(36.))
            .h(px(20.))
            .rounded(px(10.))
            .cursor_pointer()
            .border_1()
            .border_color(rgba(if value {
                t.accent
            } else {
                t.component_border_color
            }))
            .bg(rgba(if value { t.accent } else { t.bg_darker }))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                let _ = shell2.update(cx, |root, cx| match kind {
                    "wrap_text" => root.set_wrap_text_files(!value, cx),
                    "wrap_code" => root.set_wrap_code_files(!value, cx),
                    "line_numbers" => root.set_line_numbers(!value, cx),
                    "syntax_highlighting" => root.set_syntax_highlighting(!value, cx),
                    "windows_open_with" => root.set_windows_open_with(!value, cx),
                    _ => {}
                });
            })
            .child(
                div()
                    .absolute()
                    .top(px(2.))
                    .left(px(if value { 18.0 } else { 2.0 }))
                    .w(px(14.))
                    .h(px(14.))
                    .rounded(px(7.))
                    .bg(rgba(if value { 0xffffffff } else { t.text_secondary })),
            ),
    )
    .into_any_element()
}

pub(super) fn modal_button(
    t: Theme,
    label: &'static str,
    _primary: bool,
    on_click: impl Fn(&mut App) + 'static + Clone,
) -> AnyElement {
    let cb = on_click.clone();
    div()
        .id(format!("modal-{}", label))
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .px(px(12.))
        .py(px(4.))
        .rounded(px(10.))
        .cursor_pointer()
        .border_1()
        .border_color(rgba(t.chrome_border_color))
        .bg(rgba(t.menu_hover_bg))
        .hover(|s| s.bg(rgba(t.bg_secondary)))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cb(cx);
        })
        .child(label)
        .into_any_element()
}
