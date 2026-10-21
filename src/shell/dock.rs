//! Left/right docks: free-docking panel containers with drag resizing.
//! Content never reflows when a dock shrinks — it simply crops, like Zed.
//! The resize handle is an invisible overlay straddling the dock's inner
//! edge, so the tab bar meets the dock with no gap.

use gpui::{
    AnyElement, App, CursorStyle, MouseButton, MouseDownEvent, WeakEntity, div, prelude::*, px,
    rgba,
};

use super::panels::{DockSide, PanelKind};
use super::actions::OpenFolder;
use super::root::{ProjectEntry, RootView};
use crate::theme::WEIGHT_MEDIUM;

pub const DOCK_DEFAULT_WIDTH: f32 = 240.0;
pub const DOCK_HANDLE_WIDTH: f32 = 10.0;

#[derive(IntoElement)]
pub struct DockView {
    pub root: WeakEntity<RootView>,
    pub side: DockSide,
    pub visible: PanelKind,
    pub width: f32,
}

impl DockView {
    fn divider_id(&self) -> &'static str {
        match self.side {
            DockSide::Left => "dock-divider-left",
            DockSide::Right => "dock-divider-right",
        }
    }

    fn render_body(&self, cx: &mut App) -> AnyElement {
        match self.visible {
            PanelKind::Files => self.render_project(cx),
        }
    }

    fn render_project(&self, cx: &mut App) -> AnyElement {
        let Some(entity) = self.root.upgrade() else {
            return div().into_any_element();
        };
        let (root_name, entries) = {
            let view = entity.read(cx);
            (view.project_root_name(), view.project_entries())
        };
        let weak = self.root.clone();
        let t = *crate::theme::active(cx);
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .overflow_hidden()
            .when_some(root_name, |panel, name| panel.child(
                div()
                    .px(px(8.))
                    .pb(px(4.))
                    .text_sm()
                    .font_weight(WEIGHT_MEDIUM)
                    .text_color(rgba(t.text_primary))
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .child(name),
            ))
            .when(entries.is_empty(), |panel| {
                panel.child(
                    div()
                        .id("project-open-folder")
                        .mx(px(8.))
                        .py(px(5.))
                        .rounded(px(6.))
                        .cursor_pointer()
                        .text_sm()
                        .font_weight(WEIGHT_MEDIUM)
                        .text_color(rgba(t.text_secondary))
                        .hover(|s| s.bg(rgba(t.menu_hover_bg)))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            // Mouse handlers run while the view tree is still
                            // borrowed. Queue the action so the native picker
                            // cannot re-enter GPUI from inside that borrow.
                            cx.defer(|cx| cx.dispatch_action(&OpenFolder));
                            cx.stop_propagation();
                        })
                        .child("Open Folder…"),
                )
            })
            .children(entries.into_iter().map(move |entry| project_row(weak.clone(), entry, t)))
            .into_any_element()
    }
}

fn project_row(root: WeakEntity<RootView>, entry: ProjectEntry, t: crate::theme::Theme) -> AnyElement {
    let path = entry.path.clone();
    let is_dir = entry.is_dir;
    let marker = if is_dir { "▸" } else { "·" };
    div()
        .id(format!("project-entry-{}", entry.path.display()))
        .h(px(24.))
        .pl(px(8. + entry.depth as f32 * 14.))
        .pr(px(6.))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(5.))
        .rounded(px(6.))
        .cursor_pointer()
        .text_sm()
        .font_weight(WEIGHT_MEDIUM)
        .text_color(rgba(t.text_secondary))
        .whitespace_nowrap()
        .overflow_hidden()
        .hover(|s| s.bg(rgba(t.menu_hover_bg)))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            let _ = root.update(cx, |root, cx| {
                if is_dir {
                    root.toggle_project_folder(path.clone(), cx);
                } else {
                    root.open_file_path(path.to_string_lossy().into_owned(), cx);
                }
            });
            cx.stop_propagation();
        })
        .child(div().w(px(10.)).text_color(rgba(t.empty_text_secondary)).child(marker))
        .child(entry.name)
        .into_any_element()
}

impl gpui::RenderOnce for DockView {
    fn render(self, _window: &mut gpui::Window, cx: &mut App) -> impl gpui::IntoElement {
        let t = *crate::theme::active(cx);
        let root = self.root.clone();
        let side = self.side;

        // Invisible overlay straddling the dock's inner edge: 2px over the
        // dock, 3px over the manuscript. No hover effect — the cursor
        // change is the only affordance, and the tab bar meets the dock
        // edge with zero gap.
        let handle = div()
            .id(self.divider_id())
            .absolute()
            .top(px(0.))
            .h_full()
            .w(px(DOCK_HANDLE_WIDTH))
            .cursor(CursorStyle::ResizeLeftRight)
            .on_mouse_down(MouseButton::Left, move |ev: &MouseDownEvent, window, cx| {
                let max = window.viewport_size().width.as_f32();
                let _ = root.update(cx, |root, cx| {
                    root.begin_dock_drag(side, ev.position.x.as_f32(), max, cx);
                });
            });
        let handle = match side {
            DockSide::Left => handle.right(px(-4.)),
            // Keep the right-dock resize hitbox inside the dock. The
            // manuscript scrollbar owns the opposite edge of the seam.
            DockSide::Right => handle.left(px(0.)),
        };

        let panel = div()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(rgba(t.bg_secondary))
            // 1px separator where the dock meets the tab bar / manuscript.
            .border_color(rgba(t.chrome_border_color))
            .when(side == DockSide::Left, |p| p.border_r_1())
            .when(side == DockSide::Right, |p| p.border_l_1())
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .pt(px(8.))
                    .pb(px(8.))
                    .overflow_hidden()
                    .child(self.render_body(cx)),
            );

        // Fixed-width slot; the handle floats above the seam and adds no
        // layout width, so dock + tab bar always touch exactly.
        div()
            .relative()
            .w(px(self.width.max(0.0)))
            .h_full()
            .child(panel)
            .child(handle)
            .into_any_element()
    }
}
