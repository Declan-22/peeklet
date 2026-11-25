//! Image and PDF preview rendering + ink annotation canvas layer.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, ClipboardItem, Context, CursorStyle, ImageSource, MouseButton, MouseDownEvent,
    MouseExitEvent, MouseUpEvent, ObjectFit, ScrollWheelEvent, WeakEntity, Window, div, img,
    prelude::*, px, rgba, svg,
};

use crate::theme::WEIGHT_MEDIUM;

use super::chrome::{TOOLBAR_HEIGHT, TOP_BAR_HEIGHT};
use super::ink::{self, InkStroke};
use super::media_io::{
    InkRenderCache, MAX_CACHED_INK_VERTICES, build_ink_groups, copy_image_to_system_clipboard,
    crop_and_bake_image, file_name_of, fill_ink_groups, image_crop_bounds, load_image_asset,
    load_pdf_assets, save_image_copy_as,
};
use super::root::RootView;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum CropHandle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
    Move,
}

const ICON_COPY_IMAGE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<g fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5">
		<path d="M7.5 14.5c0-3.3 0-4.95 1.025-5.975S11.2 7.5 14.5 7.5s4.95 0 5.975 1.025S21.5 11.2 21.5 14.5s0 4.95-1.025 5.975S17.8 21.5 14.5 21.5s-4.95 0-5.975-1.025S7.5 17.8 7.5 14.5" />
		<path d="M7.5 16.5c-1.396 0-2.095 0-2.656-.196a3.5 3.5 0 0 1-2.148-2.148C2.5 13.595 2.5 12.896 2.5 11.5v-2c0-3.3 0-4.95 1.025-5.975S6.2 2.5 9.5 2.5h2c1.396 0 2.095 0 2.656.196a3.5 3.5 0 0 1 2.148 2.148c.196.561.196 1.26.196 2.656" />
	</g>
</svg>"#;

const ICON_SAVE_COPY: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<g fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5">
		<path d="m3 16l4.47-4.47a1.81 1.81 0 0 1 2.56 0L14 15.5m1.5 1.5L14 15.5m7 .5l-2.47-2.47a1.81 1.81 0 0 0-2.56 0L14 15.5" />
		<path d="M21.5 12c0 4.23 0 6.345-1.198 7.747q-.256.3-.555.555C18.345 21.5 16.23 21.5 12 21.5s-6.345 0-7.747-1.198q-.256-.3-.555-.555C2.5 18.345 2.5 16.23 2.5 12s0-6.345 1.198-7.747q.256-.3.555-.555C5.655 2.5 7.77 2.5 12 2.5h1.5" />
		<path d="M21.5 6.5c-.59.607-2.16 3-3 3s-2.41-2.393-3-3m3 2.5V2.5" />
	</g>
</svg>"#;

impl RootView {
    pub(super) fn start_pdf_load(&mut self, path: &str, cx: &mut Context<Self>) {
        if self.pdf_loading.contains(path) || self.pdf_assets.contains_key(path) {
            return;
        }
        let key = path.to_string();
        self.pdf_loading.insert(key.clone());
        cx.spawn(async move |this, cx| {
            let load_key = key.clone();
            let loaded = cx
                .background_executor()
                .spawn(async move { load_pdf_assets(&load_key) })
                .await;
            let _ = this.update(cx, |root, cx| {
                root.pdf_loading.remove(&key);
                if let Some(loaded) = loaded {
                    root.pdf_assets.insert(key.clone(), loaded.pages);
                } else {
                    root.pdf_load_errors.insert(key.clone());
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn render_pdf_preview(
        &mut self,
        path: &str,
        _root: WeakEntity<RootView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = *crate::theme::active(cx);
        if !self.pdf_loading.contains(path)
            && !self.pdf_assets.contains_key(path)
            && !self.pdf_load_errors.contains(path)
        {
            self.start_pdf_load(path, cx);
        }
        let Some(pages) = self.pdf_assets.get(path) else {
            return div()
                .id("pdf-preview")
                .w_full()
                .flex_1()
                .min_h(px(0.))
                .flex()
                .items_center()
                .justify_center()
                .bg(rgba(t.bg_primary))
                .child(div().text_sm().text_color(rgba(t.text_secondary)).child(
                    if self.pdf_load_errors.contains(path) {
                        "Unable to preview this PDF"
                    } else {
                        "Loading PDF…"
                    },
                ))
                .into_any_element();
        };
        let view_w = window.viewport_size().width.as_f32().max(1.0);
        let max_w = (view_w - self.dock_occupancy().0 - 48.0).max(120.0);
        let children = pages.iter().enumerate().map(|(index, page)| {
            let scale = (max_w / page.width as f32).min(1.0);
            div()
                .id(format!("pdf-page-{index}"))
                .w(px(page.width as f32 * scale))
                .h(px(page.height as f32 * scale))
                .bg(rgba(0xffffffff))
                .child(
                    img(ImageSource::from(page.image.clone()))
                        .size_full()
                        .object_fit(ObjectFit::Fill),
                )
        });
        div()
            .id("pdf-preview")
            .w_full()
            .flex_1()
            .min_h(px(0.))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(16.))
            .py(px(16.))
            .overflow_y_scroll()
            .bg(rgba(t.bg_primary))
            .children(children)
            .into_any_element()
    }

    pub(super) fn start_image_load(&mut self, path: &str, cx: &mut Context<Self>) {
        if self.image_loading.contains(path) || self.image_assets.contains_key(path) {
            return;
        }

        let key = path.to_string();
        self.image_loading.insert(key.clone());
        cx.spawn(async move |this, cx| {
            let load_key = key.clone();
            let loaded = cx
                .background_executor()
                .spawn(async move { load_image_asset(&load_key) })
                .await;
            let _ = this.update(cx, |root, cx| {
                root.image_loading.remove(&key);
                if let Some(loaded) = loaded {
                    root.image_assets
                        .insert(key.clone(), (loaded.image, loaded.width, loaded.height));
                    root.prune_image_cache(&key);
                } else {
                    root.image_load_errors.insert(key.clone());
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn prune_image_cache(&mut self, keep: &str) {
        const IMAGE_CACHE_BUDGET: usize = 256 * 1024 * 1024;
        let total_bytes: usize = self
            .image_assets
            .values()
            .map(|(image, _, _)| image.bytes.len())
            .sum();
        if total_bytes <= IMAGE_CACHE_BUDGET {
            return;
        }

        let current = self.image_assets.remove(keep);
        self.image_assets.clear();
        if let Some(current) = current {
            self.image_assets.insert(keep.to_string(), current);
        }
    }

    pub(super) fn render_image_preview(
        &mut self,
        path: &str,
        root: WeakEntity<RootView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = *crate::theme::active(cx);
        let image = self.image_assets.get(path).cloned();

        let Some((image, image_w, image_h)) = image else {
            if !self.image_loading.contains(path) && !self.image_load_errors.contains(path) {
                self.start_image_load(path, cx);
            }
            return div()
                .id("image-preview")
                .w_full()
                .flex_1()
                .min_h(px(0.))
                .flex()
                .items_center()
                .justify_center()
                .bg(rgba(t.bg_primary))
                .child(
                    div()
                        .text_sm()
                        .text_color(rgba(t.text_secondary))
                        .child(if self.image_load_errors.contains(path) {
                            "Unable to preview this image"
                        } else {
                            "Loading image…"
                        }),
                )
                .into_any_element();
        };

        let viewport = window.viewport_size();
        let (left_occ, right_occ) = self.dock_occupancy();
        let view_w = (viewport.width.as_f32() - left_occ - right_occ).max(1.0);
        let view_h = Self::viewport_h(window).max(1.0);
        let image_w = image_w as f32;
        let image_h = image_h as f32;
        const CANVAS_MARGIN: f32 = 16.0;
        let fit_w = (view_w - CANVAS_MARGIN * 2.0).max(1.0);
        let fit_h = (view_h - CANVAS_MARGIN * 2.0).max(1.0);
        let fit = (fit_w / image_w).min(fit_h / image_h);
        let draw_w = image_w * fit * self.image_zoom;
        let draw_h = image_h * fit * self.image_zoom;
        let max_pan_x = if self.image_zoom <= 1.0 {
            0.0
        } else {
            (draw_w - view_w).abs() / 2.0
        };
        let max_pan_y = if self.image_zoom <= 1.0 {
            0.0
        } else {
            (draw_h - view_h).abs() / 2.0
        };
        let left = left_occ + (view_w - draw_w) / 2.0 + self.image_pan.0;
        let top = (view_h - draw_h) / 2.0 + self.image_pan.1;

        let scale_x = if image_w > 0.0 { draw_w / image_w } else { 1.0 };
        let scale_y = if image_h > 0.0 { draw_h / image_h } else { 1.0 };

        self.image_fit = fit;
        let pen_width = (self.ink_pen_size / fit).max(0.5);
        let hl_width = (self.ink_highlighter_size / fit).max(1.0);
        let eraser_radius = ink::ERASER_SCREEN_RADIUS / scale_x;

        let ink_origin: Rc<Cell<(f32, f32)>> = Rc::new(Cell::new((left, top)));

        let mut preview = div()
            .id("image-preview")
            .w_full()
            .flex_1()
            .min_h(px(0.))
            .relative()
            .overflow_hidden()
            .bg(rgba(t.bg_primary))
            .on_mouse_down(MouseButton::Right, {
                let root = root.clone();
                move |event: &MouseDownEvent, _, cx| {
                    let _ = root.update(cx, |root, cx| {
                        let x = event.position.x.as_f32();
                        let y = event.position.y.as_f32();
                        root.image_context_menu = None;
                        root.image_right_down_pos = Some((x, y));
                        root.image_zoom_target = root.image_zoom;
                        root.image_pan_target = root.image_pan;
                        root.image_drag = Some((
                            MouseButton::Right,
                            x,
                            y,
                            root.image_pan.0,
                            root.image_pan.1,
                        ));
                        cx.notify();
                    });
                    cx.stop_propagation();
                }
            })
            .on_mouse_down(MouseButton::Middle, {
                let root = root.clone();
                move |event: &MouseDownEvent, _, cx| {
                    let _ = root.update(cx, |root, _| {
                        root.image_zoom_target = root.image_zoom;
                        root.image_pan_target = root.image_pan;
                        root.image_drag = Some((
                            MouseButton::Middle,
                            event.position.x.as_f32(),
                            event.position.y.as_f32(),
                            root.image_pan.0,
                            root.image_pan.1,
                        ));
                    });
                    cx.stop_propagation();
                }
            })
            .on_mouse_move({
                let root = root.clone();
                move |event: &gpui::MouseMoveEvent, _, cx| {
                    let _ = root.update(cx, |root, cx| {
                        if let Some((_, start_x, start_y, pan_x, pan_y)) = root.image_drag {
                            root.image_pan = (
                                (pan_x + event.position.x.as_f32() - start_x)
                                    .clamp(-max_pan_x, max_pan_x),
                                (pan_y + event.position.y.as_f32() - start_y)
                                    .clamp(-max_pan_y, max_pan_y),
                            );
                            root.image_pan_target = root.image_pan;
                            cx.notify();
                        }
                    });
                }
            })
            .on_mouse_exit({
                let root = root.clone();
                move |_event: &MouseExitEvent, _, cx| {
                    let _ = root.update(cx, |root, _| root.image_drag = None);
                }
            })
            .on_mouse_up(MouseButton::Right, {
                let root = root.clone();
                let path_up = path.to_string();
                move |event: &MouseUpEvent, _, cx| {
                    let _ = root.update(cx, |root, cx| {
                        root.image_drag = None;
                        if let Some((dx0, dy0)) = root.image_right_down_pos.take() {
                            let x = event.position.x.as_f32();
                            let y = event.position.y.as_f32();
                            let dist2 = (x - dx0).powi(2) + (y - dy0).powi(2);
                            if dist2 <= 16.0 {
                                root.open_image_context_menu(path_up.clone(), x, y, cx);
                            }
                        }
                    });
                }
            })
            .on_mouse_up(MouseButton::Middle, {
                let root = root.clone();
                move |_, _, cx| {
                    let _ = root.update(cx, |root, _| root.image_drag = None);
                }
            })
            .on_scroll_wheel({
                let root = root.clone();
                move |event: &ScrollWheelEvent, _, cx| {
                    let delta = event.delta.pixel_delta(px(20.)).y.as_f32();
                    if delta == 0.0 {
                        return;
                    }
                    let _ = root.update(cx, |root, cx| {
                        let old_zoom = root.image_zoom;
                        let step = (delta.abs() * 0.005).clamp(0.08, 0.5);
                        let factor = if delta >= 0.0 {
                            1.0 + step
                        } else {
                            1.0 / (1.0 + step)
                        };
                        let new_zoom = (root.image_zoom_target * factor).clamp(1.0, 16.0);
                        let cursor_x = event.position.x.as_f32() - left_occ - view_w / 2.0;
                        let cursor_y = event.position.y.as_f32()
                            - TOP_BAR_HEIGHT
                            - TOOLBAR_HEIGHT
                            - view_h / 2.0;
                        root.image_zoom_target = new_zoom;
                        if new_zoom <= 1.0 {
                            root.image_pan_target = (0.0, 0.0);
                        } else if new_zoom != old_zoom {
                            let scale = new_zoom / root.image_zoom;
                            root.image_pan_target.0 =
                                cursor_x - (cursor_x - root.image_pan.0) * scale;
                            root.image_pan_target.1 =
                                cursor_y - (cursor_y - root.image_pan.1) * scale;
                        }
                        root.start_image_zoom_animation(cx);
                        cx.notify();
                    });
                }
            })
            .child(
                div()
                    .id("image-preview-content")
                    .absolute()
                    .left(px(left))
                    .top(px(top))
                    .w(px(draw_w))
                    .h(px(draw_h))
                    .child(
                        img(ImageSource::from(image))
                            .size_full()
                            .object_fit(ObjectFit::Fill),
                    ),
            );

        let tool = self.image_toolbar_tool as u8;
        if matches!(
            tool,
            ink::TOOL_PEN | ink::TOOL_HIGHLIGHTER | ink::TOOL_ERASER
        ) {
            let root_down = root.clone();
            let path_down = path.to_string();
            let origin_down = ink_origin.clone();
            preview = preview.on_mouse_down(
                MouseButton::Left,
                move |ev: &MouseDownEvent, _w, cx| {
                    let (ox, oy) = origin_down.get();
                    let ix = (ev.position.x.as_f32() - ox) / scale_x;
                    let iy = (ev.position.y.as_f32() - oy) / scale_y;
                    let _ = root_down.update(cx, |root, cx| {
                        root.begin_ink_stroke(
                            &path_down,
                            ix,
                            iy,
                            pen_width,
                            hl_width,
                            eraser_radius,
                        );
                        root.set_ink_popup(None, cx);
                        cx.notify();
                    });
                    cx.stop_propagation();
                },
            );

            let root_move = root.clone();
            let origin_move = ink_origin.clone();
            preview = preview.on_mouse_move(move |ev: &gpui::MouseMoveEvent, _w, cx| {
                let (ox, oy) = origin_move.get();
                let ix = (ev.position.x.as_f32() - ox) / scale_x;
                let iy = (ev.position.y.as_f32() - oy) / scale_y;
                let _ = root_move.update(cx, |root, cx| {
                    if root.extend_ink_stroke(ix, iy, eraser_radius)
                        && root.image_last_ink_notify.elapsed() >= Duration::from_millis(8)
                    {
                        root.image_last_ink_notify = Instant::now();
                        cx.notify();
                    }
                });
            });

            let root_up = root.clone();
            preview = preview.on_mouse_up(MouseButton::Left, move |_ev, _w, cx| {
                let _ = root_up.update(cx, |root, cx| {
                    root.end_ink_stroke();
                    cx.notify();
                });
            });
        }

        let mut preview = preview.child(self.render_image_ink_layer(
            path,
            left,
            top,
            window.viewport_size().width.as_f32(),
            view_h,
            scale_x,
            scale_y,
            ink_origin.clone(),
            cx,
        ));

        if self.image_toolbar_tool == 5 {
            if let Some(crop_overlay) = self.render_image_crop_overlay(
                path,
                image_w,
                image_h,
                left,
                top,
                scale_x,
                scale_y,
                root,
                cx,
            ) {
                preview = preview.child(crop_overlay);
            }
        }

        preview.into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn render_image_ink_layer(
        &self,
        path: &str,
        left: f32,
        top: f32,
        view_w: f32,
        view_h: f32,
        scale_x: f32,
        scale_y: f32,
        origin: Rc<Cell<(f32, f32)>>,
        _cx: &mut Context<Self>,
    ) -> AnyElement {
        let committed: Rc<Vec<InkStroke>> =
            self.image_strokes.get(path).cloned().unwrap_or_default();
        let current_stroke = self.image_current_stroke.clone();
        let cache = self.image_ink_cache.clone();
        let revision = self.image_ink_revision;
        let cache_path = path.to_string();

        let origin_prepaint = origin.clone();
        let canvas = gpui::canvas(
            move |bounds: gpui::Bounds<gpui::Pixels>, _window, _app| {
                origin_prepaint.set((
                    bounds.origin.x.as_f32() + left,
                    bounds.origin.y.as_f32() + top,
                ));
            },
            move |bounds: gpui::Bounds<gpui::Pixels>, _unit, window, _app| {
                let ox = bounds.origin.x.as_f32();
                let oy = bounds.origin.y.as_f32();
                let vw = bounds.size.width.as_f32();
                let vh = bounds.size.height.as_f32();
                let ix0 = ox + left;
                let iy0 = oy + top;

                let key = (
                    revision,
                    scale_x.to_bits(),
                    scale_y.to_bits(),
                    ix0.to_bits(),
                    iy0.to_bits(),
                    vw.to_bits(),
                    vh.to_bits(),
                );
                let cached = cache
                    .borrow()
                    .as_ref()
                    .filter(|c| c.path == cache_path && c.key == key)
                    .map(|c| (c.pen.clone(), c.highlighter.clone()));
                let (pen_paths, hl_paths) = if let Some(paths) = cached {
                    paths
                } else {
                    *cache.borrow_mut() = None;
                    let (pen_groups, hl_groups) =
                        build_ink_groups(&committed, ix0, iy0, ox, oy, vw, vh, scale_x, scale_y);
                    let pen = fill_ink_groups(pen_groups);
                    let highlighter = fill_ink_groups(hl_groups);
                    let vertices = pen
                        .iter()
                        .chain(highlighter.iter())
                        .map(|(_, path)| path.vertices.len())
                        .sum::<usize>();
                    if vertices <= MAX_CACHED_INK_VERTICES {
                        *cache.borrow_mut() = Some(InkRenderCache {
                            path: cache_path.clone(),
                            key,
                            pen: pen.clone(),
                            highlighter: highlighter.clone(),
                        });
                    }
                    (pen, highlighter)
                };

                for (color, path) in pen_paths {
                    window.paint_path(path, rgba((color << 8) | 0xff));
                }
                for (color, path) in hl_paths {
                    window.paint_path(path, rgba((color << 8) | 0x78));
                }
                if let Some(current) = &current_stroke {
                    let (pen_groups, hl_groups) = build_ink_groups(
                        std::slice::from_ref(current),
                        ix0,
                        iy0,
                        ox,
                        oy,
                        vw,
                        vh,
                        scale_x,
                        scale_y,
                    );
                    for (color, group) in pen_groups {
                        if let Some(p) = ink::fill(&group) {
                            window.paint_path(p, rgba((color << 8) | 0xff));
                        }
                    }
                    for (color, group) in hl_groups {
                        if let Some(p) = ink::fill(&group) {
                            window.paint_path(p, rgba((color << 8) | 0x78));
                        }
                    }
                }
            },
        )
        .absolute()
        .left(px(0.))
        .top(px(0.))
        .w(px(view_w))
        .h(px(view_h));

        canvas.into_any_element()
    }

    /// Copy the image (with any unbaked annotations auto-baked in) to the
    /// system clipboard.
    pub(super) fn copy_image_to_clipboard(&mut self, path: String, cx: &mut Context<Self>) {
        self.image_context_menu = None;
        let strokes: Vec<InkStroke> = self
            .image_strokes
            .get(&path)
            .map(|s| (**s).clone())
            .unwrap_or_default();
        self.set_status_message("Copying image…");
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { copy_image_to_system_clipboard(&path, &strokes) })
                .await;
            let _ = this.update(cx, |root, cx| {
                match result {
                    Ok(Some(image)) => {
                        cx.write_to_clipboard(ClipboardItem::new_image(&image));
                        root.set_status_message("Copied image to clipboard");
                    }
                    Ok(None) => {
                        root.set_status_message("Copied image to clipboard");
                    }
                    Err(err) => {
                        root.set_status_message(err);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Open a Save As dialog and write a copy of the image (with any unbaked
    /// annotations auto-baked in) to the chosen path without modifying the
    /// original file.
    pub(super) fn save_image_copy_as_dialog(&mut self, path: String, cx: &mut Context<Self>) {
        self.image_context_menu = None;
        let strokes: Vec<InkStroke> = self
            .image_strokes
            .get(&path)
            .map(|s| (**s).clone())
            .unwrap_or_default();
        let src_path = std::path::PathBuf::from(&path);
        let stem = src_path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "image".to_string());
        let ext = src_path
            .extension()
            .map(|e| e.to_string_lossy().into_owned())
            .unwrap_or_else(|| "png".to_string());
        let suggested = format!("{stem}-copy.{ext}");
        let start_dir = src_path.parent().map(|p| p.to_path_buf());
        let ext_lower = ext.to_ascii_lowercase();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let dest = cx
                .background_executor()
                .spawn(async move {
                    let mut dialog = rfd::FileDialog::new().set_file_name(&suggested);
                    if let Some(dir) = start_dir {
                        dialog = dialog.set_directory(dir);
                    }
                    // Put the source image's format first so it is the default
                    // filter, followed by other common image formats.
                    let formats: &[(&str, &[&str])] = &[
                        ("PNG Image", &["png"]),
                        ("JPEG Image", &["jpg", "jpeg"]),
                        ("WebP Image", &["webp"]),
                        ("BMP Image", &["bmp"]),
                        ("TIFF Image", &["tif", "tiff"]),
                        ("GIF Image", &["gif"]),
                    ];
                    if let Some(&(name, exts)) = formats
                        .iter()
                        .find(|(_, exts)| exts.iter().any(|e| *e == ext_lower))
                    {
                        dialog = dialog.add_filter(name, exts);
                    }
                    for &(name, exts) in formats {
                        if !exts.iter().any(|e| *e == ext_lower) {
                            dialog = dialog.add_filter(name, exts);
                        }
                    }
                    dialog.save_file()
                })
                .await;
            let Some(dest) = dest else {
                return;
            };
            let _ = this.update(cx, |root, cx| {
                root.set_status_message("Saving copy…");
                cx.notify();
            });
            let result = cx
                .background_executor()
                .spawn(async move { save_image_copy_as(&path, &dest, &strokes) })
                .await;
            let _ = this.update(cx, |root, cx| {
                match result {
                    Ok(()) => root.set_status_message("Saved image copy"),
                    Err(err) => root.set_status_message(err),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Open the image right-click context menu at `(x, y)` with a smooth
    /// ease-out pop-in animation.
    pub(super) fn open_image_context_menu(
        &mut self,
        path: String,
        x: f32,
        y: f32,
        cx: &mut Context<Self>,
    ) {
        self.image_context_menu = Some((path, x, y));
        self.image_context_menu_progress = 0.0;
        self.set_ink_popup(None, cx);
        cx.notify();
        let start = Instant::now();
        const DURATION_MS: f32 = 120.0;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(8))
                    .await;
                let t = (start.elapsed().as_secs_f32() * 1000.0 / DURATION_MS).clamp(0.0, 1.0);
                // Ease-out cubic pop-in.
                let eased = 1.0 - (1.0 - t).powi(3);
                let mut done = t >= 1.0;
                let alive = this
                    .update(cx, |root, cx| {
                        if root.image_context_menu.is_none() {
                            done = true;
                            return;
                        }
                        root.image_context_menu_progress = eased;
                        cx.notify();
                    })
                    .is_ok();
                if !alive || done {
                    break;
                }
            }
        })
        .detach();
    }

    /// Dropdown-styled right-click context menu for images (`Copy Image` and
    /// `Save Copy As...`), positioned at the cursor.
    pub(super) fn render_image_context_menu(
        &self,
        root: WeakEntity<RootView>,
        window: &mut Window,
        cx: &App,
    ) -> Option<AnyElement> {
        let (path, raw_x, raw_y) = self.image_context_menu.as_ref()?;
        let t = *crate::theme::active(cx);
        let progress = self.image_context_menu_progress.clamp(0.0, 1.0);

        const MENU_W: f32 = 176.0;
        const MENU_H: f32 = 62.0;
        let vp = window.viewport_size();
        let max_x = (vp.width.as_f32() - MENU_W - 8.0).max(8.0);
        let max_y = (vp.height.as_f32() - MENU_H - 8.0).max(8.0);
        let x = raw_x.clamp(8.0, max_x);
        let y = raw_y.clamp(8.0, max_y) + (1.0 - progress) * 4.0;

        let root_bg_left = root.clone();
        let root_bg_right = root.clone();
        let root_copy = root.clone();
        let root_save = root.clone();
        let path_copy = path.clone();
        let path_save = path.clone();

        let item = |id: &'static str,
                    icon: &'static [u8],
                    label: &'static str,
                    on_click: Box<dyn Fn(&mut Window, &mut App)>| {
            div()
                .id(id)
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .h(px(26.))
                .px(px(8.))
                .rounded(px(8.))
                .text_sm()
                .font_weight(WEIGHT_MEDIUM)
                .text_color(rgba(t.text_primary))
                .cursor_pointer()
                .hover(|s| s.bg(rgba(t.menu_hover_bg)))
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    on_click(window, cx);
                    cx.stop_propagation();
                })
                .child(
                    svg()
                        .data(icon)
                        .w(px(15.))
                        .h(px(15.))
                        .flex_shrink_0()
                        .text_color(rgba(t.text_secondary)),
                )
                .child(label)
        };

        let panel = div()
            .occlude()
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(MENU_W))
            .flex()
            .flex_col()
            .px(px(4.))
            .py(px(4.))
            .gap(px(2.))
            .bg(rgba(t.bg_primary))
            .border_1()
            .border_color(rgba(t.border_color))
            .rounded(px(10.))
            .shadow(vec![t.shadow_lg()])
            .opacity(progress)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .child(item(
                "image-ctx-copy",
                ICON_COPY_IMAGE,
                "Copy Image",
                Box::new(move |_, cx| {
                    let _ = root_copy.update(cx, |root, cx| {
                        root.copy_image_to_clipboard(path_copy.clone(), cx);
                    });
                }),
            ))
            .child(item(
                "image-ctx-save-copy",
                ICON_SAVE_COPY,
                "Save Copy As...",
                Box::new(move |_, cx| {
                    let _ = root_save.update(cx, |root, cx| {
                        root.save_image_copy_as_dialog(path_save.clone(), cx);
                    });
                }),
            ));

        Some(
            div()
                .absolute()
                .top(px(0.))
                .bottom(px(0.))
                .left(px(0.))
                .right(px(0.))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    let _ = root_bg_left.update(cx, |root, cx| {
                        root.image_context_menu = None;
                        cx.notify();
                    });
                })
                .on_mouse_down(MouseButton::Right, move |_, _, cx| {
                    let _ = root_bg_right.update(cx, |root, cx| {
                        root.image_context_menu = None;
                        cx.notify();
                    });
                })
                .child(panel)
                .into_any_element(),
        )
    }

    // ----- image crop -----

    /// Enter crop mode for the active image, initializing the crop rectangle
    /// to the maximum bounds covering the image and any unbaked annotations.
    pub fn begin_image_crop(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.active_image_path() else {
            return;
        };
        let Some(&(_, w, h)) = self.image_assets.get(&path) else {
            return;
        };
        let bounds = {
            let strokes = self
                .image_strokes
                .get(&path)
                .map(|s| s.as_slice())
                .unwrap_or(&[]);
            image_crop_bounds(w as f32, h as f32, strokes)
        };
        self.image_toolbar_tool = 5;
        self.set_ink_popup(None, cx);
        self.image_shapes_dropdown_open = false;
        self.image_context_menu = None;
        self.image_crop_drag = None;
        self.image_crop_rect = Some(bounds);
        cx.notify();
    }

    /// Cancel crop mode without modifying the image.
    pub fn cancel_image_crop(&mut self, cx: &mut Context<Self>) {
        self.image_crop_rect = None;
        self.image_crop_drag = None;
        if self.image_toolbar_tool == 5 {
            self.image_toolbar_tool = 0;
        }
        cx.notify();
    }

    /// Update the active crop rectangle while dragging one of its 8 handles
    /// (or moving the whole crop box).
    pub(super) fn drag_image_crop_to(&mut self, win_x: f32, win_y: f32, cx: &mut Context<Self>) {
        let Some((handle, start_x, start_y, r, b, scale_x, scale_y)) = self.image_crop_drag else {
            return;
        };
        const MIN_CROP: f32 = 8.0;
        let dx = (win_x - start_x) / scale_x.max(1e-4);
        let dy = (win_y - start_y) / scale_y.max(1e-4);
        let mut next = r;
        match handle {
            CropHandle::TopLeft => {
                next.0 = (r.0 + dx).clamp(b.0, (r.2 - MIN_CROP).max(b.0));
                next.1 = (r.1 + dy).clamp(b.1, (r.3 - MIN_CROP).max(b.1));
            }
            CropHandle::Top => {
                next.1 = (r.1 + dy).clamp(b.1, (r.3 - MIN_CROP).max(b.1));
            }
            CropHandle::TopRight => {
                next.2 = (r.2 + dx).clamp((r.0 + MIN_CROP).min(b.2), b.2);
                next.1 = (r.1 + dy).clamp(b.1, (r.3 - MIN_CROP).max(b.1));
            }
            CropHandle::Right => {
                next.2 = (r.2 + dx).clamp((r.0 + MIN_CROP).min(b.2), b.2);
            }
            CropHandle::BottomRight => {
                next.2 = (r.2 + dx).clamp((r.0 + MIN_CROP).min(b.2), b.2);
                next.3 = (r.3 + dy).clamp((r.1 + MIN_CROP).min(b.3), b.3);
            }
            CropHandle::Bottom => {
                next.3 = (r.3 + dy).clamp((r.1 + MIN_CROP).min(b.3), b.3);
            }
            CropHandle::BottomLeft => {
                next.0 = (r.0 + dx).clamp(b.0, (r.2 - MIN_CROP).max(b.0));
                next.3 = (r.3 + dy).clamp((r.1 + MIN_CROP).min(b.3), b.3);
            }
            CropHandle::Left => {
                next.0 = (r.0 + dx).clamp(b.0, (r.2 - MIN_CROP).max(b.0));
            }
            CropHandle::Move => {
                let clamped_dx = dx.clamp(b.0 - r.0, b.2 - r.2);
                let clamped_dy = dy.clamp(b.1 - r.1, b.3 - r.3);
                next = (
                    r.0 + clamped_dx,
                    r.1 + clamped_dy,
                    r.2 + clamped_dx,
                    r.3 + clamped_dy,
                );
            }
        }
        self.image_crop_rect = Some(next);
        cx.notify();
    }

    /// Apply the active crop rectangle to the image file on disk without
    /// baking unbaked annotations; any existing unbaked strokes are translated
    /// into the cropped image's coordinate space so they stay aligned and
    /// editable.
    pub fn save_image_crop(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.active_image_path() else {
            return;
        };
        let Some(crop_rect) = self.image_crop_rect else {
            return;
        };
        let name = file_name_of(&path);
        let shift_x = -crop_rect.0.floor();
        let shift_y = -crop_rect.1.floor();
        self.image_crop_rect = None;
        self.image_crop_drag = None;
        self.image_toolbar_tool = 0;
        self.set_status_message(format!("Cropping {name}…"));
        cx.notify();

        cx.spawn(async move |this, cx| {
            let job_path = path.clone();
            let result = cx
                .background_executor()
                .spawn(async move { crop_and_bake_image(&job_path, &[], crop_rect) })
                .await;
            let _ = this.update(cx, |root, cx| {
                match result {
                    Ok(loaded) => {
                        if let Some(strokes) = root.image_strokes.get_mut(&path) {
                            for stroke in Rc::make_mut(strokes) {
                                for p in &mut stroke.points {
                                    p.0 += shift_x;
                                    p.1 += shift_y;
                                }
                            }
                        }
                        if let Some(history) = root.image_history.get_mut(&path) {
                            history.translate(shift_x, shift_y);
                        }
                        root.invalidate_image_ink_cache();
                        root.image_zoom = 1.0;
                        root.image_zoom_target = 1.0;
                        root.image_pan = (0.0, 0.0);
                        root.image_pan_target = (0.0, 0.0);
                        root.image_load_errors.remove(&path);
                        if let Some(loaded) = loaded {
                            root.image_assets
                                .insert(path.clone(), (loaded.image, loaded.width, loaded.height));
                            root.prune_image_cache(&path);
                        } else {
                            root.image_assets.remove(&path);
                            root.start_image_load(&path, cx);
                        }
                        root.sync_image_dirty(&path);
                        root.set_status_message(format!("Cropped {name}"));
                    }
                    Err(err) => {
                        root.set_status_message(err);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    #[allow(clippy::too_many_arguments)]
    fn render_image_crop_overlay(
        &self,
        path: &str,
        image_w: f32,
        image_h: f32,
        left: f32,
        top: f32,
        scale_x: f32,
        scale_y: f32,
        root: WeakEntity<RootView>,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let strokes = self
            .image_strokes
            .get(path)
            .map(|s| s.as_slice())
            .unwrap_or(&[]);
        let max_bounds = image_crop_bounds(image_w, image_h, strokes);
        let rect = self.image_crop_rect.unwrap_or(max_bounds);
        let t = *crate::theme::active(cx);

        // Screen-space crop rectangle inside `image-preview`.
        let sx0 = left + rect.0 * scale_x;
        let sy0 = top + rect.1 * scale_y;
        let sx1 = left + rect.2 * scale_x;
        let sy1 = top + rect.3 * scale_y;
        let cw = (sx1 - sx0).max(1.0);
        let ch = (sy1 - sy0).max(1.0);

        const DARK: u32 = 0x00000088;
        const ARM: f32 = 12.0; // Corner L-handle arm length
        const SIDE: f32 = 20.0; // Mid-side handle bar length
        const THICK: f32 = 3.0; // Handle thickness
        const HIT: f32 = 16.0; // Generous invisible grab zone

        let start_drag = |handle: CropHandle| {
            let root = root.clone();
            move |ev: &MouseDownEvent, _: &mut Window, cx: &mut App| {
                let _ = root.update(cx, |root, _| {
                    root.image_crop_drag = Some((
                        handle,
                        ev.position.x.as_f32(),
                        ev.position.y.as_f32(),
                        rect,
                        max_bounds,
                        scale_x,
                        scale_y,
                    ));
                });
                cx.stop_propagation();
            }
        };

        let mid_y_top = sy0.max(0.0);
        let mid_h = (sy1 - mid_y_top).max(0.0);

        // Build each corner L-bracket inside a HIT×HIT grab target.
        let corner_handle = |id: &'static str,
                             handle: CropHandle,
                             cursor: CursorStyle,
                             hx: f32,
                             hy: f32,
                             h_left: f32,
                             h_top: f32,
                             v_left: f32,
                             v_top: f32| {
            div()
                .id(id)
                .absolute()
                .left(px(hx))
                .top(px(hy))
                .w(px(HIT))
                .h(px(HIT))
                .cursor(cursor)
                .on_mouse_down(MouseButton::Left, start_drag(handle))
                .child(
                    div()
                        .absolute()
                        .left(px(h_left))
                        .top(px(h_top))
                        .w(px(ARM))
                        .h(px(THICK))
                        .rounded(px(1.))
                        .bg(rgba(t.accent)),
                )
                .child(
                    div()
                        .absolute()
                        .left(px(v_left))
                        .top(px(v_top))
                        .w(px(THICK))
                        .h(px(ARM))
                        .rounded(px(1.))
                        .bg(rgba(t.accent)),
                )
        };

        let h_side_w = SIDE.min((cw - ARM * 2.0 - 4.0).max(8.0));
        let v_side_h = SIDE.min((ch - ARM * 2.0 - 4.0).max(8.0));

        Some(
            div()
                .id("image-crop-overlay")
                .absolute()
                .inset_0()
                // 4 surrounding darkened masks (everything outside the crop rect).
                .child(
                    div()
                        .absolute()
                        .left(px(0.))
                        .right(px(0.))
                        .top(px(0.))
                        .h(px(mid_y_top))
                        .bg(rgba(DARK)),
                )
                .child(
                    div()
                        .absolute()
                        .left(px(0.))
                        .right(px(0.))
                        .top(px(sy1))
                        .bottom(px(0.))
                        .bg(rgba(DARK)),
                )
                .child(
                    div()
                        .absolute()
                        .left(px(0.))
                        .top(px(mid_y_top))
                        .w(px(sx0.max(0.0)))
                        .h(px(mid_h))
                        .bg(rgba(DARK)),
                )
                .child(
                    div()
                        .absolute()
                        .left(px(sx1))
                        .right(px(0.))
                        .top(px(mid_y_top))
                        .h(px(mid_h))
                        .bg(rgba(DARK)),
                )
                // Crop container: 1px text_primary border + drag-to-move body.
                .child(
                    div()
                        .id("crop-move-body")
                        .absolute()
                        .left(px(sx0))
                        .top(px(sy0))
                        .w(px(cw))
                        .h(px(ch))
                        .border_1()
                        .border_color(rgba(t.text_primary))
                        .cursor(CursorStyle::OpenHand)
                        .on_mouse_down(MouseButton::Left, start_drag(CropHandle::Move)),
                )
                // 4 Corner L-brackets (accent_color, 3px thick, move on both axes).
                .child(corner_handle(
                    "crop-handle-tl",
                    CropHandle::TopLeft,
                    CursorStyle::ResizeUpLeftDownRight,
                    sx0 - 4.0,
                    sy0 - 4.0,
                    3.0,
                    3.0,
                    3.0,
                    3.0,
                ))
                .child(corner_handle(
                    "crop-handle-tr",
                    CropHandle::TopRight,
                    CursorStyle::ResizeUpRightDownLeft,
                    sx1 - HIT + 4.0,
                    sy0 - 4.0,
                    HIT - 3.0 - ARM,
                    3.0,
                    HIT - 3.0 - THICK,
                    3.0,
                ))
                .child(corner_handle(
                    "crop-handle-bl",
                    CropHandle::BottomLeft,
                    CursorStyle::ResizeUpRightDownLeft,
                    sx0 - 4.0,
                    sy1 - HIT + 4.0,
                    3.0,
                    HIT - 3.0 - THICK,
                    3.0,
                    HIT - 3.0 - ARM,
                ))
                .child(corner_handle(
                    "crop-handle-br",
                    CropHandle::BottomRight,
                    CursorStyle::ResizeUpLeftDownRight,
                    sx1 - HIT + 4.0,
                    sy1 - HIT + 4.0,
                    HIT - 3.0 - ARM,
                    HIT - 3.0 - THICK,
                    HIT - 3.0 - THICK,
                    HIT - 3.0 - ARM,
                ))
                // 4 Mid-side handles (accent_color, 3px thick, move on 1 axis).
                .child(
                    div()
                        .id("crop-handle-top")
                        .absolute()
                        .left(px(sx0 + (cw - h_side_w) * 0.5))
                        .top(px(sy0 - HIT * 0.5))
                        .w(px(h_side_w))
                        .h(px(HIT))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor(CursorStyle::ResizeUpDown)
                        .on_mouse_down(MouseButton::Left, start_drag(CropHandle::Top))
                        .child(
                            div()
                                .w_full()
                                .h(px(THICK))
                                .rounded(px(1.))
                                .bg(rgba(t.accent)),
                        ),
                )
                .child(
                    div()
                        .id("crop-handle-bottom")
                        .absolute()
                        .left(px(sx0 + (cw - h_side_w) * 0.5))
                        .top(px(sy1 - HIT * 0.5))
                        .w(px(h_side_w))
                        .h(px(HIT))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor(CursorStyle::ResizeUpDown)
                        .on_mouse_down(MouseButton::Left, start_drag(CropHandle::Bottom))
                        .child(
                            div()
                                .w_full()
                                .h(px(THICK))
                                .rounded(px(1.))
                                .bg(rgba(t.accent)),
                        ),
                )
                .child(
                    div()
                        .id("crop-handle-left")
                        .absolute()
                        .left(px(sx0 - HIT * 0.5))
                        .top(px(sy0 + (ch - v_side_h) * 0.5))
                        .w(px(HIT))
                        .h(px(v_side_h))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor(CursorStyle::ResizeLeftRight)
                        .on_mouse_down(MouseButton::Left, start_drag(CropHandle::Left))
                        .child(
                            div()
                                .w(px(THICK))
                                .h_full()
                                .rounded(px(1.))
                                .bg(rgba(t.accent)),
                        ),
                )
                .child(
                    div()
                        .id("crop-handle-right")
                        .absolute()
                        .left(px(sx1 - HIT * 0.5))
                        .top(px(sy0 + (ch - v_side_h) * 0.5))
                        .w(px(HIT))
                        .h(px(v_side_h))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor(CursorStyle::ResizeLeftRight)
                        .on_mouse_down(MouseButton::Left, start_drag(CropHandle::Right))
                        .child(
                            div()
                                .w(px(THICK))
                                .h_full()
                                .rounded(px(1.))
                                .bg(rgba(t.accent)),
                        ),
                )
                .into_any_element(),
        )
    }
}
