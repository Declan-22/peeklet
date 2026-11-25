//! Live SVG preview preparation and Split-View preview pane.

use std::sync::Arc;

use gpui::{AnyElement, Context, ImageSource, ObjectFit, div, img, prelude::*, px, rgba};

use crate::theme::Theme;

use super::root::RootView;

/// Whether a file path is an SVG vector document.
pub fn is_svg_path(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
}

/// Extract the value of an XML attribute (double- or single-quoted) inside an
/// opening tag slice.
fn find_attr<'a>(tag: &'a str, name: &str) -> Option<(usize, usize, &'a str)> {
    let bytes = tag.as_bytes();
    let name_bytes = name.as_bytes();
    let mut i = 0usize;
    while i + name_bytes.len() < bytes.len() {
        let prev_ok = i == 0 || bytes[i - 1].is_ascii_whitespace();
        if prev_ok
            && tag[i..].len() >= name_bytes.len()
            && tag[i..i + name_bytes.len()].eq_ignore_ascii_case(name)
        {
            let mut j = i + name_bytes.len();
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'=' {
                j += 1;
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < bytes.len() && (bytes[j] == b'"' || bytes[j] == b'\'') {
                    let quote = bytes[j];
                    let val_start = j + 1;
                    if let Some(rel_end) = bytes[val_start..].iter().position(|&b| b == quote) {
                        let val_end = val_start + rel_end;
                        return Some((i, val_end + 1, &tag[val_start..val_end]));
                    }
                }
            }
        }
        i += 1;
    }
    None
}

fn parse_numeric_px(raw: &str) -> Option<f32> {
    let s = raw.trim();
    if s.ends_with('%') || s.ends_with("em") || s.ends_with("rem") {
        return None;
    }
    let num = s.strip_suffix("px").unwrap_or(s).trim();
    let v: f32 = num.parse().ok()?;
    (v.is_finite() && v > 0.0).then_some(v)
}

fn parse_viewbox(raw: &str) -> Option<(f32, f32)> {
    let nums: Vec<f32> = raw
        .split(|c: char| c.is_ascii_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<f32>().ok())
        .collect();
    if nums.len() == 4 && nums[2] > 0.0 && nums[3] > 0.0 {
        Some((nums[2], nums[3]))
    } else {
        None
    }
}

/// Prepare raw SVG markup for crisp preview rendering:
/// - Replaces `currentColor` with the active theme's `text_primary` color so
///   monochrome icon SVGs stay visible in both Light and Dark themes.
/// - Normalizes missing, `em`/`%`, or small (`< 512px`) root `width`/`height`
///   attributes to a crisp `1024px` max-edge raster size preserving exact
///   `viewBox` aspect ratio.
///
/// Returns `(prepared_svg_bytes, intrinsic_dimensions)`.
pub fn prepare_svg_for_preview(
    raw_svg: &str,
    text_primary: u32,
) -> Option<(Vec<u8>, Option<(f32, f32)>)> {
    let lower = raw_svg.to_ascii_lowercase();
    let svg_start = lower.find("<svg")?;
    let rel_end = raw_svg[svg_start..].find('>')?;
    let tag_end = svg_start + rel_end + 1;
    let open_tag = &raw_svg[svg_start..tag_end];

    let viewbox_dims = find_attr(open_tag, "viewBox").and_then(|(_, _, v)| parse_viewbox(v));
    let attr_w = find_attr(open_tag, "width");
    let attr_h = find_attr(open_tag, "height");
    let parsed_w = attr_w.and_then(|(_, _, v)| parse_numeric_px(v));
    let parsed_h = attr_h.and_then(|(_, _, v)| parse_numeric_px(v));

    let intrinsic = match (parsed_w, parsed_h, viewbox_dims) {
        (Some(w), Some(h), _) => Some((w, h)),
        (_, _, Some((vw, vh))) => Some((vw, vh)),
        _ => None,
    };

    // Determine target raster dimensions (scale up small/em/missing sizes so
    // GPUI's 1.0x SVG frame render is sharp).
    let needs_resize = match (parsed_w, parsed_h) {
        (Some(w), Some(h)) => w.max(h) < 512.0,
        _ => true,
    };

    let mut prepared = if needs_resize {
        let (aspect_w, aspect_h) = intrinsic.unwrap_or((512.0, 512.0));
        let max_edge = aspect_w.max(aspect_h).max(1.0);
        let scale = 1024.0 / max_edge;
        let out_w = (aspect_w * scale).round().max(1.0) as u32;
        let out_h = (aspect_h * scale).round().max(1.0) as u32;

        // Strip existing width/height attributes from the opening tag.
        let mut removals: Vec<(usize, usize)> = Vec::new();
        if let Some((s, e, _)) = attr_w {
            removals.push((s, e));
        }
        if let Some((s, e, _)) = attr_h {
            removals.push((s, e));
        }
        removals.sort_by_key(|&(s, _)| s);

        let is_self_closing = open_tag.trim_end().ends_with("/>");
        let close_len = if is_self_closing { 2 } else { 1 };
        let body_end = open_tag.len().saturating_sub(close_len);

        let mut rebuilt_tag = String::with_capacity(open_tag.len() + 40);
        let mut cur = 0usize;
        for (s, e) in removals {
            if s >= cur && e <= body_end {
                rebuilt_tag.push_str(&open_tag[cur..s]);
                cur = e;
            }
        }
        rebuilt_tag.push_str(&open_tag[cur..body_end]);
        rebuilt_tag.push_str(&format!(" width=\"{out_w}\" height=\"{out_h}\""));
        rebuilt_tag.push_str(if is_self_closing { "/>" } else { ">" });

        let mut full = String::with_capacity(raw_svg.len() + 40);
        full.push_str(&raw_svg[..svg_start]);
        full.push_str(&rebuilt_tag);
        full.push_str(&raw_svg[tag_end..]);
        full
    } else {
        raw_svg.to_string()
    };

    // Replace `currentColor` (any case) with the theme's primary text hex.
    let hex = format!(
        "#{:02x}{:02x}{:02x}",
        (text_primary >> 24) & 0xff,
        (text_primary >> 16) & 0xff,
        (text_primary >> 8) & 0xff,
    );
    if prepared.contains("currentColor") || prepared.contains("currentcolor") {
        prepared = prepared
            .replace("currentColor", &hex)
            .replace("currentcolor", &hex);
    }

    Some((prepared.into_bytes(), intrinsic))
}

impl RootView {
    /// Whether the active tab is an `.svg` file.
    pub(crate) fn active_is_svg(&self) -> bool {
        self.tabs
            .get(self.active_tab)
            .and_then(|tab| tab.path.as_deref())
            .is_some_and(is_svg_path)
    }

    /// Toggle Split View (Code + live SVG preview) vs Code-only for `.svg` tabs.
    pub fn toggle_svg_split_view(&mut self, cx: &mut Context<Self>) {
        self.svg_split_view = !self.svg_split_view;
        self.scroll_x = 0.0;
        self.cached_wrap = None;
        cx.notify();
    }

    fn get_or_build_svg_preview(
        &mut self,
        tab_id: usize,
        raw_svg: &str,
        t: &Theme,
    ) -> Option<(Arc<gpui::Image>, Option<(f32, f32)>)> {
        let key = (tab_id, self.editor_version, t.text_primary);
        if let Some((cached_key, img_ref, dims)) = &self.svg_preview_cache {
            if *cached_key == key {
                return Some((img_ref.clone(), *dims));
            }
        }
        let (bytes, dims) = prepare_svg_for_preview(raw_svg, t.text_primary)?;
        let image = Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Svg, bytes));
        self.svg_preview_cache = Some((key, image.clone(), dims));
        Some((image, dims))
    }

    /// Format the active SVG's intrinsic dimensions (e.g. `24 × 24`) for the
    /// bottom status bar.
    pub(super) fn svg_dimensions_label(&self, tab_id: usize) -> Option<String> {
        let dims = self
            .svg_preview_cache
            .as_ref()
            .filter(|((id, _, _), _, _)| *id == tab_id)
            .and_then(|(_, _, d)| *d)
            .or_else(|| {
                let raw = self.editors.get(&tab_id)?.text_string();
                prepare_svg_for_preview(&raw, 0x30302fff).and_then(|(_, d)| d)
            })?;
        let (w, h) = dims;
        Some(
            if (w - w.round()).abs() < 0.01 && (h - h.round()).abs() < 0.01 {
                format!("{} × {}", w.round() as u32, h.round() as u32)
            } else {
                format!("{w:.1} × {h:.1}")
            },
        )
    }

    /// Render the right-hand live SVG preview pane in SVG Split View.
    pub(super) fn render_svg_preview_pane(
        &mut self,
        tab_id: usize,
        raw_svg: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = *crate::theme::active(cx);
        let prepared = self.get_or_build_svg_preview(tab_id, raw_svg, &t);

        let Some((image, _dims)) = prepared else {
            return div()
                .id("svg-split-preview")
                .flex_1()
                .min_w(px(0.))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(rgba(t.bg_primary))
                .child(
                    div()
                        .text_sm()
                        .text_color(rgba(t.text_secondary))
                        .child("No valid <svg> root element"),
                )
                .into_any_element();
        };

        let fallback_color = t.text_secondary;
        div()
            .id("svg-split-preview")
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .p(px(28.))
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .bg(rgba(t.bg_primary))
            .child(
                img(ImageSource::from(image))
                    .size_full()
                    .object_fit(ObjectFit::Contain)
                    .with_fallback(move || {
                        div()
                            .text_sm()
                            .text_color(rgba(fallback_color))
                            .child("Invalid SVG markup")
                            .into_any_element()
                    }),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_svg_paths() {
        assert!(is_svg_path("icon.svg"));
        assert!(is_svg_path("assets/LOGO.SVG"));
        assert!(!is_svg_path("README.md"));
    }

    #[test]
    fn prepares_svg_current_color_and_scales_small_icons() {
        let raw = r#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24"><path stroke="currentColor" d="M0 0h24v24H0z"/></svg>"#;
        let (bytes, dims) = prepare_svg_for_preview(raw, 0x30302fff).expect("valid svg");
        assert_eq!(dims, Some((24.0, 24.0)));
        let s = String::from_utf8(bytes).unwrap();
        assert!(s.contains(r##"stroke="#30302f""##));
        assert!(s.contains(r#"width="1024" height="1024""#));
    }

    #[test]
    fn preserves_aspect_ratio_from_viewbox() {
        let raw = r#"<svg viewBox="0 0 200 100"><rect width="200" height="100"/></svg>"#;
        let (bytes, dims) = prepare_svg_for_preview(raw, 0xd2d2ceff).expect("valid svg");
        assert_eq!(dims, Some((200.0, 100.0)));
        let s = String::from_utf8(bytes).unwrap();
        assert!(s.contains(r#"width="1024" height="512""#));
    }
}
