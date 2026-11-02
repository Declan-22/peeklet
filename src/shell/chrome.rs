//! Window chrome: top bar (hamburger menu + history + tabs + window controls), bottom bar
//! (placeholder for per-file-type controls).

use gpui::{
    AnyElement, App, MouseButton, WeakEntity, Window, WindowControlArea, div, prelude::*, px, rgba,
    svg,
};

use super::panels::{ICON_INFO, PanelKind};
use super::root::{RootView, Tab};
use crate::theme::WEIGHT_MEDIUM;

pub const TOP_BAR_HEIGHT: f32 = 36.0;
pub const BOTTOM_BAR_HEIGHT: f32 = 30.0;

const MENU_ICON: &[u8] =
    br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<g fill="none" stroke="currentColor" stroke-linecap="round" stroke-width="1.5">
		<path d="M20 7L4 7" />
		<path d="M20 12L4 12" />
		<path d="M20 17L4 17" />
	</g>
</svg>
"#;

const TAB_CLOSE_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<path fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="m18 6l-6 6m0 0l-6 6m6-6l6 6m-6-6L6 6" />
</svg>
"#;

const TAB_PLUS_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M12 5v14M5 12h14" fill="none" stroke="currentColor" stroke-linecap="round" stroke-width="1.5"/>
</svg>"#;

const TAB_MAX_WIDTH: f32 = 220.0;

const TAB_TEXT_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M0 0h24v24H0z" fill="none"/><path fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M8 18h8m-8-4h8m-8-4h2m3-7.5V3c0 2.828 0 4.243.879 5.121C14.757 9 16.172 9 19 9h.5m.5 1.657V14c0 3.771 0 5.657-1.172 6.828S15.771 22 12 22s-5.657 0-6.828-1.172S4 17.771 4 14V9.456c0-3.245 0-4.868.886-5.967a4 4 0 0 1 .603-.603C6.59 2 8.211 2 11.456 2c.705 0 1.058 0 1.381.114q.1.036.197.082c.31.148.559.397 1.058.896l4.736 4.736c.579.578.867.868 1.02 1.235c.152.368.152.776.152 1.594"/>
</svg>"#;

const TAB_IMAGE_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M0 0h24v24H0z" fill="none"/><g fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"><circle cx="9.5" cy="12.5" r="1.5"/><path d="m7.5 21.5l6.793-6.793a2.414 2.414 0 0 1 3.414 0l2.107 2.107"/><path d="M13 2.5V3c0 2.828 0 4.243.879 5.121C14.757 9 16.172 9 19 9h.5m.5 1.657V14c0 3.771 0 5.657-1.172 6.828S15.771 22 12 22s-5.657-1.172-6.828-1.172S4 17.771 4 14V9.456c0-3.245 0-4.868.886-5.967a4 4 0 0 1 .603-.603C6.59 2 8.211 2 11.456 2c.705 0 1.058 0 1.381.114q.1.036.197.082c.31.148.559.397 1.058.896l4.736 4.736c.579.578.867.868 1.02 1.235c.152.368.152.776.152 1.594"/></g>
</svg>"#;

const TAB_PDF_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M0 0h24v24H0z" fill="none"/><g fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"><path d="M19 11c0-.818 0-1.57-.152-1.937s-.441-.657-1.02-1.235l-4.736-4.736c-.499-.499-.748-.748-1.058-.896a2 2 0 0 0-.197-.082C11.514 2 11.161 2 10.456 2c-3.245 0-4.868 0-5.967.886a4 4 0 0 0-.603.603C3 4.59 3 6.211 3 9.456V14c0 3.771 0 5.657 1.172 6.828S7.229 22 11 22h8M12 2.5V3c0 2.828 0 4.243.879 5.121C13.757 9 15.172 9 18 9h.5"/><path d="M21 14h-2a1 1 0 0 0-1 1v1.5m0 0V19m0-2.5h2.5M7 19v-2m0 0v-3h1.5a1.5 1.5 0 0 1 0 3zm5.5-3h1.286c.947 0 1.714.746 1.714 1.667v1.666c0 .92-.768 1.667-1.714 1.667H12.5z"/></g>
</svg>"#;

const TAB_AUDIO_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M0 0h24v24H0z" fill="none"/><g fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"><path d="M20 11s0-1.57-.152-1.937s-.441-.657-1.02-1.235l-4.736-4.736c-.499-.499-.748-.748-1.058-.896a2 2 0 0 0-.197-.082C12.514 2 12.161 2 11.456 2c-3.245 0-4.868 0-5.967.886a4 4 0 0 0-.603.603C4 4.59 4 6.211 4 9.456V14c0 3.771 0 5.657 1.172 6.828S8.229 22 12 22m1-19.5V3c0 2.828 0 4.243.879 5.121C14.757 9 16.172 9 19 9h.5"/><path d="M20 19.407v-2.814c0-1.572 0-2.359-.54-2.554c-.54-.196-1.175.36-2.446 1.472L16.5 16h-1.496c-.943 0-1.414 0-1.707.293s-.293.764-.293 1.707s0 1.414.293 1.707s.764.293 1.707.293H16.5l.514.489c1.27 1.112 1.906 1.668 2.446 1.472s.54-.982.54-2.554"/></g>
</svg>"#;

const TAB_VIDEO_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M0 0h24v24H0z" fill="none"/><g fill="none" stroke="currentColor" stroke-linejoin="round" stroke-width="1.5"><path stroke-linecap="round" d="M19 14.005v-3.344c0-.818 0-1.227-.152-1.595s-.441-.657-1.02-1.235l-4.736-4.739c-.499-.499-.748-.748-1.058-.896a2 2 0 0 0-.197-.082C11.514 2 11.161 2 10.456 2c-3.245 0-4.868 0-5.967.886a4 4 0 0 0-.603.604C3 4.59 3 6.213 3 9.46v4.545c0 3.773 0 5.66 1.172 6.832C5.115 21.78 6.52 21.964 9 22m3-19.5V3c0 2.83 0 4.245.879 5.124c.878.879 2.293.879 5.121.879h.5"/><path d="m18 19.5l1.445.963c.685.457 1.027.685 1.29.544c.265-.142.265-.553.265-1.376V18.37c0-.823 0-1.234-.264-1.376c-.264-.14-.606.087-1.29.544L18 18.5m0 1v-1m0 1c0 .935 0 1.402-.201 1.75a1.5 1.5 0 0 1-.549.549c-.348.201-.815.201-1.75.201H15c-1.414 0-2.121 0-2.56-.44C12 21.122 12 20.415 12 19s0-2.121.44-2.56C12.878 16 13.585 16 15 16h.5c.935 0 1.402 0 1.75.201a1.5 1.5 0 0 1 .549.549c.201.348.201.815.201 1.75"/></g>
</svg>"#;

const TAB_CSV_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M0 0h24v24H0z" fill="none"/><path fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M8 14h2m4 0h2m-8 4h2m4 0h2M13 2.5V3c0 2.828 0 4.243.879 5.121C14.757 9 16.172 9 19 9h.5m.5 1.657V14c0 3.771 0 5.657-1.172 6.828S15.771 22 12 22s-5.657-1.172-6.828-1.172S4 17.771 4 14V9.456c0-3.245 0-4.868.886-5.967a4 4 0 0 1 .603-.603C6.59 2 8.211 2 11.456 2c.705 0 1.058 0 1.381.114q.1.036.197.082c.31.148.559.397 1.058.896l4.736 4.736c.579.578.867.868 1.02 1.235c.152.368.152.776.152 1.594"/>
</svg>"#;

const TAB_3D_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M0 0h24v24H0z" fill="none"/><path fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M12 2v20m2.87-8.324l7.119 3.91m0 0l.005.004m-.005-.004c.075-.437-.232-.928-.802-1.839L13.784 3.92C12.984 2.64 12.584 2 12 2c-.583 0-.983.64-1.784 1.92L2.813 15.746c-.57.911-.877 1.402-.802 1.84m19.978 0a1 1 0 0 1-.028.109c-.152.467-.694.683-1.777 1.116l-7.403 2.957c-.385.154-.578.231-.781.231s-.396-.077-.781-.231l-7.403-2.957c-1.083-.433-1.625-.65-1.777-1.116a1 1 0 0 1-.028-.11m-.005.004l.005-.004m0 0l7.12-3.91"/>
</svg>"#;

const TAB_SVG_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M0 0h24v24H0z" fill="none"/><g fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"><path d="M19 11c0-.818 0-1.57-.152-1.937s-.441-.657-1.02-1.235l-4.736-4.736c-.499-.499-.748-.748-1.058-.896a2 2 0 0 0-.197-.082C11.514 2 11.161 2 10.456 2c-3.245 0-4.868 0-5.967.886a4 4 0 0 0-.603.603C3 4.59 3 6.211 3 9.456V14c0 3.771 0 5.657 1.172 6.828S7.229 22 11 22h8M12 2.5V3c0 2.828 0 4.243.879 5.121C13.757 9 15.172 9 18 9h.5"/><path d="M21 15a1 1 0 0 0-1-1h-1.5a1 1 0 0 0-1 1v3a1 1 0 0 0 1 1H20a1 1 0 0 0 1-1v-1h-1m-10-3H8.5a1.5 1.5 0 0 0 0 3H9a1 1 0 0 1 1 1v.5a1.5 1.5 0 0 1-1.5 1.5H7.5m4.5-5l1.75 5l1.75-5"/></g>
</svg>"#;

fn tab_icon(tab: &Tab) -> &'static [u8] {
    let Some(path) = tab.path.as_deref() else {
        return TAB_TEXT_ICON;
    };
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tif" | "tiff" | "ico" | "heic"
        | "avif" => TAB_IMAGE_ICON,
        "pdf" => TAB_PDF_ICON,
        "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" => TAB_AUDIO_ICON,
        "mp4" | "mov" | "avi" | "mkv" | "webm" | "m4v" => TAB_VIDEO_ICON,
        "csv" | "tsv" => TAB_CSV_ICON,
        "obj" | "fbx" | "gltf" | "glb" | "stl" | "ply" | "3ds" | "dae" | "blend" => TAB_3D_ICON,
        "svg" => TAB_SVG_ICON,
        _ => TAB_TEXT_ICON,
    }
}

// ---------- Top bar ----------

#[derive(IntoElement)]
pub struct TopBar {
    pub root: WeakEntity<RootView>,
    pub menu_open: bool,
    pub icon_animation: f32,
    pub tabs: Vec<Tab>,
    pub active: usize,
    pub hovered: Option<usize>,
    pub can_undo: bool,
    pub can_redo: bool,
}

impl gpui::RenderOnce for TopBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl gpui::IntoElement {
        let t = *crate::theme::active(cx);

        div()
            .id("top-bar")
            .w_full()
            .h(px(TOP_BAR_HEIGHT))
            .flex()
            .flex_row()
            .items_center()
            .bg(rgba(t.bg_secondary))
            // Clicking anywhere else on the title bar dismisses the menu.
            .on_mouse_down(MouseButton::Left, {
                let root = self.root.clone();
                move |_, _, cx| {
                    let _ = root.update(cx, |root, cx| root.close_menu(cx));
                }
            })
            .child(render_menu_button(
                self.menu_open,
                self.icon_animation,
                t,
                self.root.clone(),
                cx,
            ))
            .child(divider(t.chrome_border_color))
            .child(TabBar {
                root: self.root.clone(),
                tabs: self.tabs,
                active: self.active,
                hovered: self.hovered,
                can_undo: self.can_undo,
                can_redo: self.can_redo,
            })
            // Spacer — only this region is draggable so it doesn't
            // swallow the menu or close / min / max hitboxes.
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .window_control_area(WindowControlArea::Drag),
            )
            .child(render_window_controls(t, cx))
    }
}

fn divider(color: u32) -> AnyElement {
    div()
        .w(px(1.5))
        .h(px(TOP_BAR_HEIGHT - 8.))
        .my(px(4.))
        .mx(px(6.))
        .rounded(px(2.))
        .bg(rgba(color))
        .into_any_element()
}

fn render_menu_button(
    _menu_open: bool,
    animation: f32,
    t: crate::theme::Theme,
    root: WeakEntity<RootView>,
    cx: &App,
) -> AnyElement {
    // Active fade follows the open/close animation; hover fade is a tween.
    let act = animation.clamp(0., 1.);
    let hov = shell_fade(&root, cx, "tb-menu");
    let k = act.max(hov);
    let bg = crate::theme::lerp_rgb(t.bg_secondary, t.bg_tertiary, k);
    let icon_color = crate::theme::lerp_rgb(t.text_secondary, t.text_primary, act);
    let border = crate::theme::fade_in(t.border_color, k);
    let mut shadow = t.shadow_sm();
    shadow.color = rgba(crate::theme::fade_in(t.item_shadow_color, k)).into();

    div()
        .id("app-menu-button")
        .flex()
        .items_center()
        .justify_center()
        .w(px(TOP_BAR_HEIGHT - 8.))
        .h(px(TOP_BAR_HEIGHT - 8.))
        .ml(px(3.))
        .rounded(px(6.))
        .cursor_pointer()
        // Constant geometry; only colors tween.
        .border_1()
        .border_color(rgba(border))
        .bg(rgba(bg))
        .shadow(vec![shadow])
        .on_hover({
            let root = root.clone();
            move |hovered, _, cx| {
                let _ = root.update(cx, |root, cx| {
                    root.animate_fade("tb-menu", if *hovered { 1.0 } else { 0.0 }, cx);
                });
            }
        })
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            let _ = root.update(cx, |root, cx| {
                root.toggle_menu(cx);
                cx.stop_propagation();
            });
        })
        .child(
            svg()
                .data(MENU_ICON)
                .w(px(20.))
                .h(px(20.))
                .text_color(rgba(icon_color)),
        )
        .into_any_element()
}

fn render_window_controls(t: crate::theme::Theme, _cx: &App) -> AnyElement {
    let control = |id: &'static str, area: WindowControlArea, content: AnyElement| -> AnyElement {
        div()
            .id(id)
            .w(px(46.))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .window_control_area(area)
            .hover(|s| s.bg(rgba(t.menu_hover_bg)))
            .child(content)
            .into_any_element()
    };

    div()
        .flex()
        .flex_row()
        .items_stretch()
        .h_full()
        .child(control(
            "window-minimize",
            WindowControlArea::Min,
            div()
                .w(px(10.))
                .h(px(1.))
                .bg(rgba(t.text_secondary))
                .into_any_element(),
        ))
        .child(control(
            "window-maximize",
            WindowControlArea::Max,
            div()
                .size(px(9.))
                .border_1()
                .border_color(rgba(t.text_secondary))
                .into_any_element(),
        ))
        .child(control(
            "window-close",
            WindowControlArea::Close,
            div()
                .text_sm()
                .text_color(rgba(t.text_primary))
                .child("✕")
                .into_any_element(),
        ))
        .into_any_element()
}

// Reads a fade value from the root without needing a full upgrade.
fn shell_fade(root: &WeakEntity<RootView>, cx: &App, key: &str) -> f32 {
    root.upgrade().map(|s| s.read(cx).fade(key)).unwrap_or(0.0)
}

// ---------- Tabs in the top bar ----------

#[derive(IntoElement)]
pub struct TabBar {
    pub root: WeakEntity<RootView>,
    pub tabs: Vec<Tab>,
    pub active: usize,
    pub hovered: Option<usize>,
    pub can_undo: bool,
    pub can_redo: bool,
}

impl gpui::RenderOnce for TabBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl gpui::IntoElement {
        let t = *crate::theme::active(cx);
        let undo = |id: &'static str, label: &'static str, enabled: bool| {
            let root = self.root.clone();
            div()
                .id(id)
                .w(px(30.))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(rgba(if enabled {
                    t.text_secondary
                } else {
                    t.empty_text_secondary
                }))
                .hover(|s| {
                    if enabled {
                        s.bg(rgba(t.menu_hover_bg))
                    } else {
                        s
                    }
                })
                .on_mouse_down(MouseButton::Left, {
                    let root = root.clone();
                    move |_, _, cx| {
                        let _ = root.update(cx, |root, cx| {
                            if id == "tab-undo" {
                                root.undo(cx);
                            } else {
                                root.redo(cx);
                            }
                        });
                    }
                })
                .child(label)
        };

        let root_for_scroll = self.root.clone();
        let root_for_move = self.root.clone();
        let root_for_up = self.root.clone();
        div()
            .id("tab-bar")
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_row()
            .items_stretch()
            .bg(rgba(t.bg_secondary))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_stretch()
                    .child(undo("tab-undo", "↩", self.can_undo))
                    .child(undo("tab-redo", "↪", self.can_redo))
                    .child(divider(t.chrome_border_color)),
            )
            .child(
                div()
                    .id("tab-viewport")
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .overflow_hidden()
                    .on_mouse_down(
                        MouseButton::Middle,
                        move |event: &gpui::MouseDownEvent, _, cx| {
                            let _ = root_for_scroll.update(cx, |root, _| {
                                root.begin_tab_scroll(event.position.x.as_f32())
                            });
                            cx.stop_propagation();
                        },
                    )
                    .on_mouse_move(move |event: &gpui::MouseMoveEvent, _, cx| {
                        let _ = root_for_move.update(cx, |root, cx| {
                            root.update_tab_scroll(event.position.x.as_f32(), cx)
                        });
                    })
                    .on_mouse_up(MouseButton::Middle, move |_, _, cx| {
                        let _ = root_for_up.update(cx, |root, cx| root.end_tab_scroll(cx));
                    })
                    .child(
                        div()
                            .id("tab-strip")
                            .absolute()
                            .left(px(-self
                                .root
                                .upgrade()
                                .map(|r| r.read(cx).tab_scroll_offset)
                                .unwrap_or(0.)))
                            .top(px(0.))
                            .h_full()
                            .flex()
                            .flex_row()
                            .items_end()
                            .pl(px(8.))
                            .children(self.tabs.iter().enumerate().map(|(i, tab)| {
                                let root = self.root.clone();
                                let tab_id = tab.id;
                                let is_active = i == self.active;
                                let is_hovered = self.hovered == Some(tab_id);
                                // Constant geometry in both states: the outer shell always
                                // carries side borders, the inner row always carries the
                                // bottom border — only colors change. Inactive sides are
                                // transparent and the active bottom edge is manuscript-toned,
                                // so switching tabs never moves anything by a pixel.
                                div()
                                    .id(format!("tab-{tab_id}"))
                                    .h(px(TOP_BAR_HEIGHT - 6.))
                                    .mt(px(6.))
                                    .mb(px(0.))
                                    .min_w(px(84.))
                                    .max_w(px(TAB_MAX_WIDTH))
                                    .flex_shrink_0()
                                    .flex()
                                    .flex_row()
                                    .items_stretch()
                                    .cursor_pointer()
                                    .rounded_tl(px(9.))
                                    .rounded_tr(px(9.))
                                    .border_l_1()
                                    .border_r_1()
                                    .border_t_1()
                                    .border_color(rgba(if is_active {
                                        t.chrome_border_color
                                    } else {
                                        0x00000000
                                    }))
                                    .bg(rgba(if is_active {
                                        0x00000000
                                    } else {
                                        t.bg_secondary
                                    }))
                                    .hover(|s| {
                                        if is_active {
                                            s
                                        } else {
                                            s.bg(rgba(t.menu_hover_bg))
                                        }
                                    })
                                    .on_hover({
                                        let root = root.clone();
                                        move |hovered, _, cx| {
                                            let _ = root.update(cx, |root, cx| {
                                                root.set_hovered_tab(
                                                    if *hovered { Some(tab_id) } else { None },
                                                    cx,
                                                );
                                            });
                                        }
                                    })
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        let _ = root
                                            .update(cx, |root, cx| root.activate_tab(tab_id, cx));
                                    })
                                    .when(is_active, |tab| {
                                        tab.child(
                                            div()
                                                .absolute()
                                                .left(px(5.))
                                                .right(px(5.))
                                                .top(px(0.))
                                                .bottom(px(0.))
                                                .rounded_tl(px(8.))
                                                .rounded_tr(px(8.))
                                                .bg(rgba(t.bg_primary)),
                                        )
                                        .child(
                                            div()
                                                .absolute()
                                                .left(px(0.))
                                                .bottom(px(-1.))
                                                .w(px(12.))
                                                .h(px(12.))
                                                .rounded(px(12.))
                                                .bg(rgba(t.bg_primary)),
                                        )
                                        .child(
                                            div()
                                                .absolute()
                                                .right(px(0.))
                                                .bottom(px(-1.))
                                                .w(px(12.))
                                                .h(px(12.))
                                                .rounded(px(12.))
                                                .bg(rgba(t.bg_primary)),
                                        )
                                    })
                                    .child(
                                        div()
                                            .relative()
                                            .id(format!("tab-surface-{tab_id}"))
                                            .min_w(px(0.))
                                            .flex_1()
                                            .h_full()
                                            .flex_shrink_0()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .pl(px(9.))
                                            .pr(px(7.))
                                            .on_hover({
                                                let root = self.root.clone();
                                                move |hovered, _, cx| {
                                                    let _ = root.update(cx, |root, cx| {
                                                        root.set_hovered_tab(
                                                            if *hovered {
                                                                Some(tab_id)
                                                            } else {
                                                                None
                                                            },
                                                            cx,
                                                        );
                                                    });
                                                }
                                            })
                                            .child(
                                                div()
                                                    .w(px(14.))
                                                    .h(px(14.))
                                                    .mt(px(1.))
                                                    .mr(px(6.))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(
                                                        svg()
                                                            .data(tab_icon(tab))
                                                            .w(px(14.))
                                                            .h(px(14.))
                                                            .text_color(rgba(if is_active {
                                                                t.accent
                                                            } else {
                                                                t.text_secondary
                                                            })),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .min_w(px(0.))
                                                    .flex_1()
                                                    .h_full()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(
                                                        div()
                                                            .w_full()
                                                            .min_w(px(0.))
                                                            .text_left()
                                                            .text_sm()
                                                            .font_weight(WEIGHT_MEDIUM)
                                                            .text_color(rgba(if is_active {
                                                                t.text_primary
                                                            } else {
                                                                t.text_secondary
                                                            }))
                                                            .whitespace_nowrap()
                                                            .overflow_hidden()
                                                            .text_ellipsis()
                                                            .child(tab.title.clone()),
                                                    ),
                                            )
                                            .child({
                                                let root = self.root.clone();
                                                let close_hovered = root
                                                    .upgrade()
                                                    .map(|view| {
                                                        view.read(cx).hovered_close_tab
                                                            == Some(tab_id)
                                                    })
                                                    .unwrap_or(false);
                                                // Fixed slot so the title never shifts. The close control
                                                // is always mounted so hover timing cannot hide it.
                                                div()
                                                    .absolute()
                                                    .right(px(2.))
                                                    .top(px(0.))
                                                    .bottom(px(0.))
                                                    .w(px(16.))
                                                    .opacity(if is_hovered { 1.0 } else { 0.0 })
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(
                                                        div()
                                                            .id(format!("tab-close-{tab_id}"))
                                                            .w(px(16.))
                                                            .min_w(px(16.))
                                                            .h(px(16.))
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .rounded(px(4.))
                                                            .text_color(rgba(if close_hovered {
                                                                t.text_primary
                                                            } else {
                                                                t.text_secondary
                                                            }))
                                                            .when(close_hovered, |button| {
                                                                button.bg(rgba(t.bg_secondary))
                                                            })
                                                            .on_hover({
                                                                let root = root.clone();
                                                                move |hovered, _, cx| {
                                                                    let _ =
                                                                        root.update(
                                                                            cx,
                                                                            |root, cx| {
                                                                                if *hovered {
                                                    root.hovered_close_tab = Some(tab_id);
                                                } else if root.hovered_close_tab == Some(tab_id) {
                                                    root.hovered_close_tab = None;
                                                }
                                                                                cx.notify();
                                                                            },
                                                                        );
                                                                }
                                                            })
                                                            .on_mouse_down(
                                                                MouseButton::Left,
                                                                move |_, _, cx| {
                                                                    let _ = root.update(
                                                                        cx,
                                                                        |root, cx| {
                                                                            root.close_tab(
                                                                                tab_id, cx,
                                                                            )
                                                                        },
                                                                    );
                                                                    cx.stop_propagation();
                                                                },
                                                            )
                                                            .child(
                                                                svg()
                                                                    .data(TAB_CLOSE_ICON)
                                                                    .w(px(12.))
                                                                    .h(px(12.))
                                                                    .text_color(rgba(
                                                                        if close_hovered {
                                                                            t.text_primary
                                                                        } else {
                                                                            t.text_secondary
                                                                        },
                                                                    )),
                                                            ),
                                                    )
                                            }),
                                    )
                            }))
                            .child(
                                div()
                                    .id("new-tab")
                                    .w(px(32.))
                                    .h_full()
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(rgba(t.text_secondary))
                                    .hover(|s| s.bg(rgba(t.menu_hover_bg)))
                                    .on_mouse_down(MouseButton::Left, {
                                        let root = self.root.clone();
                                        move |_, _, cx| {
                                            let _ =
                                                root.update(cx, |root, cx| root.open_welcome(cx));
                                        }
                                    })
                                    .child(svg().data(TAB_PLUS_ICON).w(px(15.)).h(px(15.))),
                            ),
                    ),
            )
    }
}

// ---------- Bottom bar ----------

#[derive(IntoElement)]
pub struct BottomBar {
    pub root: WeakEntity<RootView>,
}

impl gpui::RenderOnce for BottomBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl gpui::IntoElement {
        let t = *crate::theme::active(cx);
        let info_active = self
            .root
            .upgrade()
            .is_some_and(|root| root.read(cx).panel_visible(PanelKind::Info));
        let root = self.root.clone();

        div()
            .id("bottom-bar")
            .w_full()
            .h(px(BOTTOM_BAR_HEIGHT))
            .flex()
            .flex_row()
            .items_center()
            .px(px(6.))
            .bg(rgba(t.bg_secondary))
            .border_t_1()
            .border_color(rgba(t.chrome_border_color))
            .child(div().flex_1())
            .child(
                div()
                    .id("bottom-info-toggle")
                    .w(px(24.))
                    .h(px(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(5.))
                    .cursor_pointer()
                    .when(info_active, |button| button.bg(rgba(t.bg_tertiary)))
                    .hover(|button| button.bg(rgba(t.bg_tertiary)))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        let _ = root.update(cx, |root, cx| {
                            root.toggle_panel(PanelKind::Info, cx);
                        });
                        cx.stop_propagation();
                    })
                    .child(
                        svg()
                            .data(ICON_INFO)
                            .w(px(15.))
                            .h(px(15.))
                            .text_color(rgba(t.text_secondary)),
                    ),
            )
    }
}
