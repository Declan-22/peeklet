//! Welcome tab view (Get Started actions + recent files list).

use gpui::{AnyElement, App, FontWeight, MouseButton, WeakEntity, div, prelude::*, px, rgba, svg};

use crate::theme::WEIGHT_MEDIUM;

use super::root::{ACCEPTED_FILE_EXTENSIONS, RootView};

const ICON_OPEN_FILE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<g fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5">
		<path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
	</g>
</svg>"#;

impl RootView {
    /// Zed-style welcome tab: big centered greeting, then a narrow
    /// left-aligned column of Get Started actions and recent files.
    pub(super) fn render_welcome(&self, root: WeakEntity<RootView>, cx: &mut App) -> AnyElement {
        let t = *crate::theme::active(cx);

        let section = |label: &'static str| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.))
                .mb(px(8.))
                .child(
                    div()
                        .text_xs()
                        .font_family(crate::theme::FONT_MONO)
                        .text_color(rgba(t.text_secondary))
                        .child(label),
                )
                .child(div().flex_1().h(px(1.)).bg(rgba(t.border_subtle)))
        };

        let action_button = |id: &'static str, icon: &'static [u8], label: &'static str| {
            let root = root.clone();
            div()
                .id(id)
                .flex()
                .flex_row()
                .items_center()
                .gap(px(10.))
                .h(px(30.))
                .px(px(6.))
                .rounded(px(6.))
                .cursor_pointer()
                .hover(|s| s.bg(rgba(t.bg_hover)))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    let root = root.clone();
                    cx.spawn(async move |cx| {
                        let path = cx
                            .background_executor()
                            .spawn(async {
                                rfd::FileDialog::new()
                                    .add_filter("Supported files", ACCEPTED_FILE_EXTENSIONS)
                                    .pick_file()
                            })
                            .await;
                        if let Some(path) = path {
                            let _ = root.update(cx, |root, cx| {
                                root.open_file_path(path.to_string_lossy().into_owned(), cx);
                            });
                        }
                    })
                    .detach();
                })
                .child(
                    svg()
                        .data(icon)
                        .w(px(14.))
                        .h(px(14.))
                        .text_color(rgba(t.text_secondary)),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(WEIGHT_MEDIUM)
                        .text_color(rgba(t.text_primary))
                        .child(label),
                )
        };

        div()
            .flex_1()
            .min_w(px(0.))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .bg(rgba(t.bg_canvas))
            .child(
                div()
                    .w_full()
                    .max_w(px(560.))
                    .px(px(48.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .w_full()
                            .text_center()
                            .text_2xl()
                            .font_weight(FontWeight(500.0))
                            .text_color(rgba(t.text_primary))
                            .mb(px(28.))
                            .child("Welcome to Peeklet"),
                    )
                    .child(section("OPEN A FILE"))
                    .child(action_button("welcome-open", ICON_OPEN_FILE, "Open File…"))
                    .child(div().h(px(20.)))
                    .when(!self.recent_files.is_empty(), |col| {
                        col.child(section("RECENT FILES")).children(
                            self.recent_files.iter().map(|path| {
                                let root = root.clone();
                                let path = path.clone();
                                let name = std::path::Path::new(&path)
                                    .file_name()
                                    .map(|n| n.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| path.clone());
                                div()
                                    .id(format!("recent-{path}"))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .h(px(28.))
                                    .px(px(6.))
                                    .rounded(px(6.))
                                    .cursor_pointer()
                                    .overflow_hidden()
                                    .hover(|s| s.bg(rgba(t.bg_hover)))
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        let _ = root.update(cx, |root, cx| {
                                            root.open_file_path(path.clone(), cx);
                                        });
                                    })
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(WEIGHT_MEDIUM)
                                            .text_color(rgba(t.text_primary))
                                            .whitespace_nowrap()
                                            .overflow_hidden()
                                            .child(name),
                                    )
                            }),
                        )
                    }),
            )
            .into_any_element()
    }
}
