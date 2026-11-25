//! Image/PDF asset loading and ink annotation rasterization/culling helpers.

use std::io::Cursor;
use std::sync::Arc;

use gpui::Path;
use image::{ImageDecoder, ImageEncoder};

use super::ink::{self, InkStroke};

pub(super) type InkPath = (u32, Path<gpui::Pixels>);
pub(super) const MAX_CACHED_INK_VERTICES: usize = 50_000;

pub(super) struct InkRenderCache {
    pub path: String,
    pub key: (u64, u32, u32, u32, u32, u32, u32),
    pub pen: Vec<InkPath>,
    pub highlighter: Vec<InkPath>,
}

pub(super) struct LoadedImage {
    pub image: Arc<gpui::Image>,
    pub width: u32,
    pub height: u32,
}

pub(super) struct LoadedPdf {
    pub pages: Vec<LoadedImage>,
}

pub(super) fn load_pdf_assets(path: &str) -> Option<LoadedPdf> {
    let root = std::env::temp_dir().join(format!(
        "peeklet-pdf-{}-{}",
        std::process::id(),
        path.bytes()
            .fold(0u64, |hash, byte| hash.wrapping_mul(31).wrapping_add(byte as u64))
    ));
    let _ = std::fs::create_dir_all(&root);
    let prefix = root.join("page");
    let status = std::process::Command::new("pdftoppm")
        .args(["-png", "-r", "120", "-f", "1", "-l", "64"])
        .arg(path)
        .arg(&prefix)
        .status()
        .ok()?;
    if !status.success() {
        let _ = std::fs::remove_dir_all(&root);
        return None;
    }
    let mut pages = Vec::new();
    for page_number in 1..=64 {
        let page_path = root.join(format!("page-{page_number}.png"));
        let Ok(bytes) = std::fs::read(&page_path) else {
            break;
        };
        let reader = image::ImageReader::new(Cursor::new(&bytes))
            .with_guessed_format()
            .ok()?;
        let (width, height) = reader.into_dimensions().ok()?;
        pages.push(LoadedImage {
            image: Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png, bytes)),
            width,
            height,
        });
    }
    let _ = std::fs::remove_dir_all(&root);
    (!pages.is_empty()).then_some(LoadedPdf { pages })
}

/// Read image bytes and dimensions away from the UI thread. Keep normal-sized
/// source bytes intact so JPEG/WebP/PNG files are not needlessly decoded and
/// re-encoded before GPUI gets them. Very large images are reduced once, in
/// the background, to keep the GPU/cache footprint bounded.
pub(super) fn load_image_asset(path: &str) -> Option<LoadedImage> {
    const MAX_IMAGE_EDGE: u32 = 8192;

    let bytes = std::fs::read(path).ok()?;
    let reader = image::ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .ok()?;
    let source_format = reader.format()?;
    let mut decoder = reader.into_decoder().ok()?;
    let (width, height) = decoder.dimensions();
    let orientation = decoder.orientation().ok();
    let (width, height) = match orientation {
        Some(o) if orientation_swaps_axes(o) => (height, width),
        _ => (width, height),
    };
    let gpui_format = to_gpui_image_format(source_format)?;

    if width.max(height) <= MAX_IMAGE_EDGE {
        drop(decoder);
        return Some(LoadedImage {
            image: Arc::new(gpui::Image::from_bytes(gpui_format, bytes)),
            width,
            height,
        });
    }

    let orientation = decoder.orientation().ok()?;
    let mut decoded = image::DynamicImage::from_decoder(decoder).ok()?;
    decoded.apply_orientation(orientation);
    let scale = MAX_IMAGE_EDGE as f32 / decoded.width().max(decoded.height()) as f32;
    let resized = decoded.resize(
        (decoded.width() as f32 * scale).round() as u32,
        (decoded.height() as f32 * scale).round() as u32,
        image::imageops::FilterType::Lanczos3,
    );
    let width = resized.width();
    let height = resized.height();
    let mut encoded = Cursor::new(Vec::new());
    resized
        .write_to(&mut encoded, image::ImageFormat::Png)
        .ok()?;
    Some(LoadedImage {
        image: Arc::new(gpui::Image::from_bytes(
            gpui::ImageFormat::Png,
            encoded.into_inner(),
        )),
        width,
        height,
    })
}

/// Build a raster path for a stroke, offset into the output canvas.
fn stroke_path(stroke: &InkStroke, off_x: f32, off_y: f32) -> Option<tiny_skia::Path> {
    if stroke.points.len() < 2 {
        return None;
    }
    let mut pb = tiny_skia::PathBuilder::new();
    let first = stroke.points[0];
    pb.move_to(first.0 + off_x, first.1 + off_y);
    for p in &stroke.points[1..] {
        pb.line_to(p.0 + off_x, p.1 + off_y);
    }
    pb.finish()
}

/// Rasterize a same-colour group of highlighter strokes into a tight
/// bounding-box coverage mask and blend it onto `canvas`.
fn blend_highlighter_group(
    canvas: &mut tiny_skia::Pixmap,
    strokes: &[&InkStroke],
    color: u32,
    off_x: f32,
    off_y: f32,
) {
    let canvas_w = canvas.width();
    let canvas_h = canvas.height();
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for stroke in strokes {
        let pad = stroke.width + 2.0;
        for &(px, py) in &stroke.points {
            let x = px + off_x;
            let y = py + off_y;
            min_x = min_x.min(x - pad);
            min_y = min_y.min(y - pad);
            max_x = max_x.max(x + pad);
            max_y = max_y.max(y + pad);
        }
    }
    if !min_x.is_finite() {
        return;
    }
    let bx0 = (min_x.floor() as i64).clamp(0, canvas_w as i64) as u32;
    let by0 = (min_y.floor() as i64).clamp(0, canvas_h as i64) as u32;
    let bx1 = (max_x.ceil() as i64).clamp(0, canvas_w as i64) as u32;
    let by1 = (max_y.ceil() as i64).clamp(0, canvas_h as i64) as u32;
    let bw = bx1.saturating_sub(bx0);
    let bh = by1.saturating_sub(by0);
    if bw == 0 || bh == 0 {
        return;
    }

    let Some(mut mask) = tiny_skia::Pixmap::new(bw, bh) else {
        return;
    };
    let mut paint = tiny_skia::Paint::default();
    paint.anti_alias = true;
    paint.set_color(tiny_skia::Color::WHITE);
    paint.blend_mode = tiny_skia::BlendMode::Lighten;

    let mask_off_x = off_x - bx0 as f32;
    let mask_off_y = off_y - by0 as f32;
    for stroke in strokes {
        let Some(path) = stroke_path(stroke, mask_off_x, mask_off_y) else {
            continue;
        };
        let mut style = tiny_skia::Stroke::default();
        style.width = stroke.width;
        style.line_cap = tiny_skia::LineCap::Butt;
        style.line_join = tiny_skia::LineJoin::Bevel;
        mask.stroke_path(
            &path,
            &paint,
            &style,
            tiny_skia::Transform::identity(),
            None,
        );
    }

    let hr = (color >> 16) & 0xff;
    let hg = (color >> 8) & 0xff;
    let hb = color & 0xff;
    let mask_pixels = mask.pixels();
    let dst_stride = canvas_w as usize * 4;
    let bw_usize = bw as usize;
    let data = canvas.data_mut();
    for y in 0..bh as usize {
        let mask_row = &mask_pixels[y * bw_usize..(y + 1) * bw_usize];
        let dst_start = (by0 as usize + y) * dst_stride + bx0 as usize * 4;
        let dst_row = &mut data[dst_start..dst_start + bw_usize * 4];
        for (mp, p) in mask_row.iter().zip(dst_row.chunks_exact_mut(4)) {
            let a = mp.alpha();
            if a == 0 {
                continue;
            }
            let alpha = (a as u32 * 120 + 127) / 255;
            let inv = 255 - alpha;
            p[0] = ((hr * alpha + p[0] as u32 * inv + 127) / 255) as u8;
            p[1] = ((hg * alpha + p[1] as u32 * inv + 127) / 255) as u8;
            p[2] = ((hb * alpha + p[2] as u32 * inv + 127) / 255) as u8;
            p[3] = (alpha + (p[3] as u32 * inv + 127) / 255).min(255) as u8;
        }
    }
}

/// Undo premultiplication in place on an RGBA8 buffer.
fn unpremultiply_in_place(data: &mut [u8]) {
    for px in data.chunks_exact_mut(4) {
        let a = px[3];
        if a == 255 || a == 0 {
            continue;
        }
        let a16 = a as u16;
        px[0] = ((px[0] as u16 * 255) / a16).min(255) as u8;
        px[1] = ((px[1] as u16 * 255) / a16).min(255) as u8;
        px[2] = ((px[2] as u16 * 255) / a16).min(255) as u8;
    }
}

/// Map an `image::ImageFormat` to the `gpui::ImageFormat` used by the viewer.
fn to_gpui_image_format(format: image::ImageFormat) -> Option<gpui::ImageFormat> {
    match format {
        image::ImageFormat::Png => Some(gpui::ImageFormat::Png),
        image::ImageFormat::Jpeg => Some(gpui::ImageFormat::Jpeg),
        image::ImageFormat::WebP => Some(gpui::ImageFormat::Webp),
        image::ImageFormat::Gif => Some(gpui::ImageFormat::Gif),
        image::ImageFormat::Bmp => Some(gpui::ImageFormat::Bmp),
        image::ImageFormat::Tiff => Some(gpui::ImageFormat::Tiff),
        image::ImageFormat::Ico => Some(gpui::ImageFormat::Ico),
        image::ImageFormat::Pnm => Some(gpui::ImageFormat::Pnm),
        _ => None,
    }
}

/// The file name of a path, for messages.
pub(super) fn file_name_of(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

/// Whether an orientation swaps the width and the height.
fn orientation_swaps_axes(orientation: image::metadata::Orientation) -> bool {
    matches!(
        orientation,
        image::metadata::Orientation::Rotate90
            | image::metadata::Orientation::Rotate270
            | image::metadata::Orientation::Rotate90FlipH
            | image::metadata::Orientation::Rotate270FlipH
    )
}

/// Whether an image format can store an alpha channel.
fn format_has_alpha(format: image::ImageFormat) -> bool {
    !matches!(format, image::ImageFormat::Jpeg | image::ImageFormat::Pnm)
}

/// Maximum crop bounds in source-image pixel space, covering both the image
/// `(0..img_w, 0..img_h)` and any annotation strokes extending outside it.
pub(super) fn image_crop_bounds(
    img_w: f32,
    img_h: f32,
    strokes: &[InkStroke],
) -> (f32, f32, f32, f32) {
    let mut min_x = 0.0f32;
    let mut min_y = 0.0f32;
    let mut max_x = img_w;
    let mut max_y = img_h;
    for stroke in strokes {
        let pad = stroke.width;
        for p in &stroke.points {
            min_x = min_x.min(p.0 - pad);
            min_y = min_y.min(p.1 - pad);
            max_x = max_x.max(p.0 + pad);
            max_y = max_y.max(p.1 + pad);
        }
    }
    (min_x, min_y, max_x, max_y)
}

/// Rasterize the annotation strokes (if any) onto the image and return the
/// encoded bytes, output format, and dimensions without modifying disk.
fn bake_annotated_image_bytes(
    path: &str,
    strokes: &[InkStroke],
    target_format: Option<image::ImageFormat>,
    crop_rect: Option<(f32, f32, f32, f32)>,
) -> Result<(Vec<u8>, image::ImageFormat, u32, u32), String> {
    let name = file_name_of(path);

    let reader = image::ImageReader::open(path)
        .map_err(|e| format!("Could not read {name}: {e}"))?
        .with_guessed_format()
        .map_err(|e| format!("Could not read {name}: {e}"))?;
    let source_format = reader.format().unwrap_or(image::ImageFormat::Png);
    let format = target_format.unwrap_or(source_format);
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| format!("Could not decode {name}: {e}"))?;
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut source = image::DynamicImage::from_decoder(decoder)
        .map_err(|e| format!("Could not decode {name}: {e}"))?;
    source.apply_orientation(orientation);

    let (img_w, img_h) = (source.width() as f32, source.height() as f32);

    let (min_x, min_y, max_x, max_y) = crop_rect.unwrap_or_else(|| {
        image_crop_bounds(img_w, img_h, strokes)
    });
    let ix0 = min_x.floor() as i64;
    let iy0 = min_y.floor() as i64;
    let ix1 = max_x.ceil() as i64;
    let iy1 = max_y.ceil() as i64;
    let off_x = -ix0;
    let off_y = -iy0;
    let canvas_w = (ix1 - ix0).max(1) as u32;
    let canvas_h = (iy1 - iy0).max(1) as u32;

    let Some(mut canvas) = tiny_skia::Pixmap::new(canvas_w, canvas_h) else {
        return Err(format!("Could not make room for the annotations in {name}"));
    };
    let has_alpha = format_has_alpha(format);
    if !has_alpha {
        canvas.fill(tiny_skia::Color::WHITE);
    }

    let canvas_covered_by_source = off_x <= 0
        && off_y <= 0
        && (canvas_w as i64 - off_x) <= source.width() as i64
        && (canvas_h as i64 - off_y) <= source.height() as i64;
    let mut all_opaque = !has_alpha;

    {
        let src_w = source.width() as usize;
        let src_h = source.height() as usize;
        let data = canvas.data_mut();
        let dst_stride = canvas_w as usize * 4;
        // Clip source rectangle to the intersection with the output canvas.
        let src_x0 = (-off_x).clamp(0, src_w as i64) as usize;
        let src_x1 = (canvas_w as i64 - off_x).clamp(0, src_w as i64) as usize;
        let src_y0 = (-off_y).clamp(0, src_h as i64) as usize;
        let src_y1 = (canvas_h as i64 - off_y).clamp(0, src_h as i64) as usize;
        let copy_w = src_x1.saturating_sub(src_x0);
        let dst_x0 = (src_x0 as i64 + off_x).max(0) as usize;
        let dst_y0 = (src_y0 as i64 + off_y).max(0) as usize;

        if copy_w > 0 && src_y1 > src_y0 {
            match source.color() {
                image::ColorType::Rgb8 => {
                    if canvas_covered_by_source {
                        all_opaque = true;
                    }
                    let src = source.as_bytes();
                    for y in src_y0..src_y1 {
                        let row_start = y * src_w * 3 + src_x0 * 3;
                        let src_row = &src[row_start..row_start + copy_w * 3];
                        let dst_start = (dst_y0 + (y - src_y0)) * dst_stride + dst_x0 * 4;
                        let dst_row = &mut data[dst_start..dst_start + copy_w * 4];
                        for (s, d) in src_row.chunks_exact(3).zip(dst_row.chunks_exact_mut(4)) {
                            d[0] = s[0];
                            d[1] = s[1];
                            d[2] = s[2];
                            d[3] = 255;
                        }
                    }
                }
                image::ColorType::Rgba8 => {
                    let src = source.as_bytes();
                    let mut src_all_opaque = canvas_covered_by_source;
                    for y in src_y0..src_y1 {
                        let row_start = y * src_w * 4 + src_x0 * 4;
                        let src_row = &src[row_start..row_start + copy_w * 4];
                        let dst_start = (dst_y0 + (y - src_y0)) * dst_stride + dst_x0 * 4;
                        let dst_row = &mut data[dst_start..dst_start + copy_w * 4];
                        if src_row.chunks_exact(4).all(|p| p[3] == 255) {
                            dst_row.copy_from_slice(src_row);
                        } else {
                            src_all_opaque = false;
                            for (s, d) in src_row.chunks_exact(4).zip(dst_row.chunks_exact_mut(4)) {
                                let a = s[3] as u16;
                                d[0] = ((s[0] as u16 * a) / 255) as u8;
                                d[1] = ((s[1] as u16 * a) / 255) as u8;
                                d[2] = ((s[2] as u16 * a) / 255) as u8;
                                d[3] = a as u8;
                            }
                        }
                    }
                    if src_all_opaque {
                        all_opaque = true;
                    }
                }
                _ => {
                    let rgba = source.to_rgba8();
                    let src = rgba.as_raw();
                    let stride = rgba.width() as usize * 4;
                    for y in src_y0..src_y1 {
                        let row_start = y * stride + src_x0 * 4;
                        let src_row = &src[row_start..row_start + copy_w * 4];
                        let dst_start = (dst_y0 + (y - src_y0)) * dst_stride + dst_x0 * 4;
                        let dst_row = &mut data[dst_start..dst_start + copy_w * 4];
                        if src_row.chunks_exact(4).all(|p| p[3] == 255) {
                            dst_row.copy_from_slice(src_row);
                        } else {
                            for (s, d) in src_row.chunks_exact(4).zip(dst_row.chunks_exact_mut(4)) {
                                let a = s[3] as u16;
                                d[0] = ((s[0] as u16 * a) / 255) as u8;
                                d[1] = ((s[1] as u16 * a) / 255) as u8;
                                d[2] = ((s[2] as u16 * a) / 255) as u8;
                                d[3] = a as u8;
                            }
                        }
                    }
                }
            }
        }
    }

    drop(source);

    for stroke in strokes.iter().filter(|s| s.tool == ink::TOOL_PEN) {
        let Some(path) = stroke_path(stroke, off_x as f32, off_y as f32) else {
            continue;
        };
        let mut paint = tiny_skia::Paint::default();
        paint.anti_alias = true;
        let pen_color = stroke.color;
        paint.set_color(
            tiny_skia::Color::from_rgba(
                ((pen_color >> 16) & 0xff) as f32 / 255.0,
                ((pen_color >> 8) & 0xff) as f32 / 255.0,
                (pen_color & 0xff) as f32 / 255.0,
                1.0,
            )
            .unwrap_or(tiny_skia::Color::BLACK),
        );
        let mut style = tiny_skia::Stroke::default();
        style.width = stroke.width;
        style.line_cap = tiny_skia::LineCap::Round;
        style.line_join = tiny_skia::LineJoin::Round;
        canvas.stroke_path(
            &path,
            &paint,
            &style,
            tiny_skia::Transform::identity(),
            None,
        );
    }

    let mut hl_groups: Vec<(u32, Vec<&InkStroke>)> = Vec::new();
    for stroke in strokes.iter().filter(|s| s.tool == ink::TOOL_HIGHLIGHTER) {
        match hl_groups.iter_mut().find(|(c, _)| *c == stroke.color) {
            Some((_, group)) => group.push(stroke),
            None => hl_groups.push((stroke.color, vec![stroke])),
        }
    }
    for (color, group) in hl_groups {
        blend_highlighter_group(&mut canvas, &group, color, off_x as f32, off_y as f32);
    }

    let mut encoded: Vec<u8> = Vec::with_capacity((canvas_w as usize * canvas_h as usize) / 2);
    {
        let mut writer = Cursor::new(&mut encoded);
        match format {
            image::ImageFormat::Png => {
                let encoder = image::codecs::png::PngEncoder::new_with_quality(
                    &mut writer,
                    image::codecs::png::CompressionType::Fast,
                    image::codecs::png::FilterType::Sub,
                );
                if all_opaque {
                    let mut rgb: Vec<u8> =
                        Vec::with_capacity(canvas_w as usize * canvas_h as usize * 3);
                    for px in canvas.data().chunks_exact(4) {
                        rgb.extend_from_slice(&px[0..3]);
                    }
                    encoder
                        .write_image(&rgb, canvas_w, canvas_h, image::ExtendedColorType::Rgb8)
                        .map_err(|e| format!("Could not encode {name}: {e}"))?;
                } else {
                    unpremultiply_in_place(canvas.data_mut());
                    encoder
                        .write_image(
                            canvas.data(),
                            canvas_w,
                            canvas_h,
                            image::ExtendedColorType::Rgba8,
                        )
                        .map_err(|e| format!("Could not encode {name}: {e}"))?;
                }
            }
            image::ImageFormat::Jpeg => {
                let mut rgb: Vec<u8> =
                    Vec::with_capacity(canvas_w as usize * canvas_h as usize * 3);
                for px in canvas.data().chunks_exact(4) {
                    rgb.extend_from_slice(&px[0..3]);
                }
                let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, 85);
                encoder
                    .write_image(&rgb, canvas_w, canvas_h, image::ExtendedColorType::Rgb8)
                    .map_err(|e| format!("Could not encode {name}: {e}"))?;
            }
            _ => {
                if !all_opaque {
                    unpremultiply_in_place(canvas.data_mut());
                }
                let out = image::RgbaImage::from_raw(canvas_w, canvas_h, canvas.take())
                    .ok_or_else(|| format!("Could not build the picture data for {name}"))?;
                image::DynamicImage::ImageRgba8(out)
                    .write_to(&mut writer, format)
                    .map_err(|e| format!("Could not encode {name}: {e}"))?;
            }
        }
    }
    Ok((encoded, format, canvas_w, canvas_h))
}

/// Rasterize the annotation strokes onto the image and write the result back.
pub(super) fn bake_annotated_image(
    path: &str,
    strokes: &[InkStroke],
) -> Result<Option<LoadedImage>, String> {
    const MAX_IMAGE_EDGE: u32 = 8192;
    let name = file_name_of(path);
    let (encoded, format, canvas_w, canvas_h) =
        bake_annotated_image_bytes(path, strokes, None, None)?;
    std::fs::write(path, &encoded).map_err(|e| format!("Could not save {name}: {e}"))?;

    let loaded = if canvas_w.max(canvas_h) <= MAX_IMAGE_EDGE {
        to_gpui_image_format(format).map(|gpui_format| LoadedImage {
            image: Arc::new(gpui::Image::from_bytes(gpui_format, encoded)),
            width: canvas_w,
            height: canvas_h,
        })
    } else {
        None
    };
    Ok(loaded)
}

/// Crop the image (with any unbaked annotations included) to `crop_rect` in
/// source-image pixel space and write the result back to `path`.
pub(super) fn crop_and_bake_image(
    path: &str,
    strokes: &[InkStroke],
    crop_rect: (f32, f32, f32, f32),
) -> Result<Option<LoadedImage>, String> {
    const MAX_IMAGE_EDGE: u32 = 8192;
    let name = file_name_of(path);
    let (encoded, format, canvas_w, canvas_h) =
        bake_annotated_image_bytes(path, strokes, None, Some(crop_rect))?;
    std::fs::write(path, &encoded).map_err(|e| format!("Could not save {name}: {e}"))?;

    let loaded = if canvas_w.max(canvas_h) <= MAX_IMAGE_EDGE {
        to_gpui_image_format(format).map(|gpui_format| LoadedImage {
            image: Arc::new(gpui::Image::from_bytes(gpui_format, encoded)),
            width: canvas_w,
            height: canvas_h,
        })
    } else {
        None
    };
    Ok(loaded)
}

/// Copy the image (with any unbaked annotations auto-baked in) to the system
/// clipboard.
///
/// On Windows, writes both `CF_DIB` (universal paste support in Paint, Word,
/// Discord, browsers, etc.) and the registered `"PNG"` format (preserving full
/// alpha in apps that support it). On other platforms, returns a `gpui::Image`
/// for `cx.write_to_clipboard`.
pub(super) fn copy_image_to_system_clipboard(
    path: &str,
    strokes: &[InkStroke],
) -> Result<Option<gpui::Image>, String> {
    let (png_bytes, _, _, _) =
        bake_annotated_image_bytes(path, strokes, Some(image::ImageFormat::Png), None)?;
    #[cfg(target_os = "windows")]
    {
        write_windows_clipboard_image(&png_bytes)?;
        Ok(None)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(Some(gpui::Image::from_bytes(
            gpui::ImageFormat::Png,
            png_bytes,
        )))
    }
}

/// Build a 32-bit bottom-up `CF_DIB` (`BITMAPINFOHEADER` + BGRA pixels) buffer
/// from PNG bytes for the Windows clipboard.
#[cfg(target_os = "windows")]
fn png_to_cf_dib(png_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let decoded = image::load_from_memory_with_format(png_bytes, image::ImageFormat::Png)
        .map_err(|e| format!("Could not decode PNG for clipboard: {e}"))?
        .to_rgba8();
    let w = decoded.width();
    let h = decoded.height();
    let row_bytes = (w as usize) * 4;
    let img_bytes = row_bytes * (h as usize);
    let mut dib = Vec::with_capacity(40 + img_bytes);

    // BITMAPINFOHEADER (40 bytes, little-endian)
    dib.extend_from_slice(&40u32.to_le_bytes()); // biSize
    dib.extend_from_slice(&(w as i32).to_le_bytes()); // biWidth
    dib.extend_from_slice(&(h as i32).to_le_bytes()); // biHeight (positive = bottom-up)
    dib.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    dib.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    dib.extend_from_slice(&0u32.to_le_bytes()); // biCompression = BI_RGB
    dib.extend_from_slice(&(img_bytes as u32).to_le_bytes()); // biSizeImage
    dib.extend_from_slice(&0i32.to_le_bytes()); // biXPelsPerMeter
    dib.extend_from_slice(&0i32.to_le_bytes()); // biYPelsPerMeter
    dib.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
    dib.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant

    let raw = decoded.as_raw();
    for y in (0..h as usize).rev() {
        let row = &raw[y * row_bytes..(y + 1) * row_bytes];
        for px in row.chunks_exact(4) {
            let a = px[3] as u16;
            let inv = 255 - a;
            // Composite over white for CF_DIB (BI_RGB) so apps that ignore the
            // alpha byte don't show black fringes; full alpha is preserved in
            // the companion "PNG" clipboard format.
            let r = ((px[0] as u16 * a + 255 * inv + 127) / 255) as u8;
            let g = ((px[1] as u16 * a + 255 * inv + 127) / 255) as u8;
            let b = ((px[2] as u16 * a + 255 * inv + 127) / 255) as u8;
            dib.push(b);
            dib.push(g);
            dib.push(r);
            dib.push(255);
        }
    }
    Ok(dib)
}

#[cfg(target_os = "windows")]
fn write_windows_clipboard_image(png_bytes: &[u8]) -> Result<(), String> {
    use std::ffi::c_void;
    use std::ptr::null_mut;

    const CF_DIB: u32 = 8;
    const GMEM_MOVEABLE: u32 = 0x0002;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn OpenClipboard(hwnd: *mut c_void) -> i32;
        fn EmptyClipboard() -> i32;
        fn SetClipboardData(format: u32, mem: *mut c_void) -> *mut c_void;
        fn CloseClipboard() -> i32;
        fn RegisterClipboardFormatW(name: *const u16) -> u32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GlobalAlloc(flags: u32, bytes: usize) -> *mut c_void;
        fn GlobalLock(mem: *mut c_void) -> *mut c_void;
        fn GlobalUnlock(mem: *mut c_void) -> i32;
        fn GlobalFree(mem: *mut c_void) -> *mut c_void;
    }

    let dib_bytes = png_to_cf_dib(png_bytes)?;

    unsafe fn set_format(format: u32, data: &[u8]) -> Result<(), String> {
        let hmem = unsafe { GlobalAlloc(GMEM_MOVEABLE, data.len()) };
        if hmem.is_null() {
            return Err("Could not allocate clipboard memory".into());
        }
        let dst = unsafe { GlobalLock(hmem) } as *mut u8;
        if dst.is_null() {
            unsafe { GlobalFree(hmem) };
            return Err("Could not lock clipboard memory".into());
        }
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), dst, data.len());
            GlobalUnlock(hmem);
            if SetClipboardData(format, hmem).is_null() {
                GlobalFree(hmem);
                return Err("Could not write image to clipboard".into());
            }
        }
        Ok(())
    }

    let mut opened = false;
    for _ in 0..5 {
        if unsafe { OpenClipboard(null_mut()) } != 0 {
            opened = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    if !opened {
        return Err("Clipboard is busy".into());
    }

    let result = (|| -> Result<(), String> {
        if unsafe { EmptyClipboard() } == 0 {
            return Err("Could not clear clipboard".into());
        }
        let png_name: [u16; 4] = [b'P' as u16, b'N' as u16, b'G' as u16, 0];
        let png_format = unsafe { RegisterClipboardFormatW(png_name.as_ptr()) };
        if png_format != 0 {
            unsafe { set_format(png_format, png_bytes)? };
        }
        unsafe { set_format(CF_DIB, &dib_bytes)? };
        Ok(())
    })();

    unsafe { CloseClipboard() };
    result
}

/// Write a copy of the image (with any unbaked annotations auto-baked in) to
/// `dest_path` without modifying the original file.
pub(super) fn save_image_copy_as(
    source_path: &str,
    dest_path: &std::path::Path,
    strokes: &[InkStroke],
) -> Result<(), String> {
    let dest_name = dest_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| dest_path.display().to_string());
    let target_format = image::ImageFormat::from_path(dest_path).ok();
    let (encoded, _, _, _) =
        bake_annotated_image_bytes(source_path, strokes, target_format, None)?;
    std::fs::write(dest_path, &encoded).map_err(|e| format!("Could not save {dest_name}: {e}"))?;
    Ok(())
}

pub(super) fn build_ink_groups(
    strokes: &[InkStroke],
    ix0: f32,
    iy0: f32,
    ox: f32,
    oy: f32,
    vw: f32,
    vh: f32,
    scale_x: f32,
    scale_y: f32,
) -> (
    Vec<(u32, Vec<Vec<(f32, f32)>>)>,
    Vec<(u32, Vec<Vec<(f32, f32)>>)>,
) {
    let mut pen_groups: Vec<(u32, Vec<Vec<(f32, f32)>>)> = Vec::new();
    let mut hl_groups: Vec<(u32, Vec<Vec<(f32, f32)>>)> = Vec::new();
    for stroke in strokes {
        if stroke.points.len() < 2 {
            continue;
        }
        let pts = ink::smooth(&stroke.points);
        let win: Vec<(f32, f32)> = pts
            .iter()
            .map(|p| (ix0 + p.0 * scale_x, iy0 + p.1 * scale_y))
            .collect();
        if !visible(&win, ox, oy, vw, vh) {
            continue;
        }
        let w = stroke.width * scale_x;
        match stroke.tool {
            ink::TOOL_PEN => {
                let curve = ink::spline(&win, 0.4, 220);
                let poly = ink::ribbon(&curve, w * 0.5, false);
                if poly.len() >= 3 {
                    match pen_groups.iter_mut().find(|(c, _)| *c == stroke.color) {
                        Some((_, group)) => group.push(poly),
                        None => pen_groups.push((stroke.color, vec![poly])),
                    }
                }
            }
            ink::TOOL_HIGHLIGHTER => {
                let curve = ink::spline(&win, 1.0, 220);
                let poly = ink::ribbon(&curve, w * 0.5, true);
                if poly.len() >= 3 {
                    match hl_groups.iter_mut().find(|(c, _)| *c == stroke.color) {
                        Some((_, group)) => group.push(poly),
                        None => hl_groups.push((stroke.color, vec![poly])),
                    }
                }
            }
            _ => {}
        }
    }
    (pen_groups, hl_groups)
}

pub(super) fn fill_ink_groups(groups: Vec<(u32, Vec<Vec<(f32, f32)>>)>) -> Vec<InkPath> {
    groups
        .into_iter()
        .filter_map(|(color, group)| ink::fill(&group).map(|path| (color, path)))
        .collect()
}

fn visible(points: &[(f32, f32)], x: f32, y: f32, w: f32, h: f32) -> bool {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for p in points {
        min_x = min_x.min(p.0);
        min_y = min_y.min(p.1);
        max_x = max_x.max(p.0);
        max_y = max_y.max(p.1);
    }
    min_x <= x + w && max_x >= x && min_y <= y + h && max_y >= y
}
