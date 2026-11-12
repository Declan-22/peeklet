//! Vector ink layer for image annotation.
//!
//! Strokes are stored as **vectors in the source image's pixel space**, so
//! they stay glued to the image and scale with zoom.
//!
//! Rendering builds the outline of each stroke as a *ribbon* polygon (the
//! stroke's silhouette) and fills it. Filling polygons keeps the edges exact
//! at any zoom, costs a tiny amount of geometry, and — because every stroke
//! of a tool goes into a single filled path with the non-zero rule — the
//! highlighter's overlapping strokes merge into one region instead of
//! stacking their opacity.

use gpui::{Path, PathBuilder, PathStyle, Pixels, point, px};
use lyon::tessellation::{FillOptions, FillRule};

pub const TOOL_PEN: u8 = 1;
pub const TOOL_HIGHLIGHTER: u8 = 2;
pub const TOOL_ERASER: u8 = 3;

/// Pen width on screen, in pixels, at the fit-to-window zoom level.
pub const PEN_SCREEN_WIDTH: f32 = 2.5;
/// Highlighter width on screen, in pixels, at the fit-to-window zoom level.
pub const HIGHLIGHTER_SCREEN_WIDTH: f32 = 14.0;
/// Eraser radius on screen, in pixels, at the fit-to-window zoom level.
pub const ERASER_SCREEN_RADIUS: f32 = 14.0;

/// A single annotation stroke, in the source image's pixel space.
#[derive(Clone, Debug)]
pub struct InkStroke {
    pub tool: u8,
    /// Stroke width in source image pixels.
    pub width: f32,
    /// Points in source image pixels.
    pub points: Vec<(f32, f32)>,
}

impl InkStroke {
    pub fn new(tool: u8, width: f32, points: Vec<(f32, f32)>) -> Self {
        Self { tool, width, points }
    }
}

/// A cubic Bézier segment.
#[derive(Clone, Copy, Debug)]
struct Cubic {
    p0: (f32, f32),
    c1: (f32, f32),
    c2: (f32, f32),
    p1: (f32, f32),
}

/// Convert a polyline into a chain of cubic Béziers using the centripetal
/// Catmull-Rom spline. Centripetal parameterization keeps the curve free of
/// cusps and self-intersections even when the samples are unevenly spaced.
fn catmull_rom(points: &[(f32, f32)]) -> Vec<Cubic> {
    let n = points.len();
    if n < 2 {
        return Vec::new();
    }
    const ALPHA: f32 = 0.5;
    let knot = |a: (f32, f32), b: (f32, f32)| {
        let dx = b.0 - a.0;
        let dy = b.1 - a.1;
        (dx * dx + dy * dy).powf(ALPHA).max(1e-4)
    };

    let mut out = Vec::with_capacity(n - 1);
    for i in 0..n - 1 {
        let p0 = points[i.saturating_sub(1)];
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = points[(i + 2).min(n - 1)];

        let t0 = 0.0f32;
        let t1 = t0 + knot(p0, p1);
        let t2 = t1 + knot(p1, p2);
        let t3 = t2 + knot(p2, p3);

        let d20 = (t2 - t1) / (t2 - t0).max(1e-6);
        let d31 = (t2 - t1) / (t3 - t1).max(1e-6);
        let m1 = ((p2.0 - p0.0) * d20, (p2.1 - p0.1) * d20);
        let m2 = ((p3.0 - p1.0) * d31, (p3.1 - p1.1) * d31);

        out.push(Cubic {
            p0: p1,
            c1: (p1.0 + m1.0 / 3.0, p1.1 + m1.1 / 3.0),
            c2: (p2.0 - m2.0 / 3.0, p2.1 - m2.1 / 3.0),
            p1: p2,
        });
    }
    out
}

/// Distance of a point from the segment `a`..`b`.
fn point_line_dist(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let len2 = dx * dx + dy * dy;
    if len2 < 1e-12 {
        return ((p.0 - a.0).powi(2) + (p.1 - a.1).powi(2)).sqrt();
    }
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0);
    let cx = a.0 + dx * t;
    let cy = a.1 + dy * t;
    ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt()
}

/// Split a cubic Bézier in half.
fn split_cubic(c: &Cubic) -> (Cubic, Cubic) {
    let mid = |a: (f32, f32), b: (f32, f32)| ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5);
    let p01 = mid(c.p0, c.c1);
    let p12 = mid(c.c1, c.c2);
    let p23 = mid(c.c2, c.p1);
    let p012 = mid(p01, p12);
    let p123 = mid(p12, p23);
    let p = mid(p012, p123);
    (
        Cubic { p0: c.p0, c1: p01, c2: p012, p1: p },
        Cubic { p0: p, c1: p123, c2: p23, p1: c.p1 },
    )
}

/// Flatten a cubic Bézier, appending points until the curve deviates from
/// the chord by less than `tolerance`.
fn flatten_cubic(c: &Cubic, tolerance: f32, out: &mut Vec<(f32, f32)>, depth: u32) {
    let dev = point_line_dist(c.c1, c.p0, c.p1).max(point_line_dist(c.c2, c.p0, c.p1));
    if dev <= tolerance || depth >= 12 {
        out.push(c.p1);
        return;
    }
    let (l, r) = split_cubic(c);
    flatten_cubic(&l, tolerance, out, depth + 1);
    flatten_cubic(&r, tolerance, out, depth + 1);
}

/// Draw a smooth curve through the points, flattened so that it stays within
/// `tolerance` of the ideal shape.
///
/// The points are pointer samples, which are far apart when the mouse moves
/// quickly. Interpolating them with a spline and flattening the result at
/// screen resolution is what keeps the stroke smooth instead of a chain of
/// straight lines.
pub fn spline(points: &[(f32, f32)], tolerance: f32, max_points: usize) -> Vec<(f32, f32)> {
    if points.len() < 2 {
        return points.to_vec();
    }
    // The tessellator allocates per point, so a very long stroke would cost
    // a lot to draw. Keep the number of points bounded by raising the
    // flattening tolerance when the curve is long.
    let mut length = 0.0f32;
    for i in 0..points.len() - 1 {
        let a = points[i];
        let b = points[i + 1];
        length += ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    }
    let tolerance = tolerance.max(length / max_points.max(2) as f32);

    let segments = catmull_rom(points);
    let mut out: Vec<(f32, f32)> = Vec::with_capacity(points.len() * 4);
    out.push(segments[0].p0);
    for seg in &segments {
        flatten_cubic(seg, tolerance, &mut out, 0);
    }
    out
}

/// Smooth raw pointer samples with a binomial filter.
///
/// Pointer samples are noisy and often land in a straight line for a couple
/// of frames before the hand turns, which shows up as a sharp corner in the
/// finished stroke. Filtering the samples rounds those corners the way a
/// brush does. The filter runs over the sample list, so the amount of
/// smoothing follows the stroke's own sampling density.
pub fn smooth(points: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let n = points.len();
    if n < 3 {
        return points.to_vec();
    }
    // Two passes of a [1, 2, 1] filter.
    let mut a: Vec<(f32, f32)> = Vec::with_capacity(n);
    a.push(points[0]);
    for i in 1..n - 1 {
        let p = points[i];
        let q = points[i - 1];
        let r = points[i + 1];
        a.push((
            (q.0 + 2.0 * p.0 + r.0) * 0.25,
            (q.1 + 2.0 * p.1 + r.1) * 0.25,
        ));
    }
    a.push(points[n - 1]);

    let mut out: Vec<(f32, f32)> = Vec::with_capacity(n);
    out.push(a[0]);
    for i in 1..n - 1 {
        let p = a[i];
        let q = a[i - 1];
        let r = a[i + 1];
        out.push((
            (q.0 + 2.0 * p.0 + r.0) * 0.25,
            (q.1 + 2.0 * p.1 + r.1) * 0.25,
        ));
    }
    out.push(a[n - 1]);
    out
}

/// Build the silhouette of a stroke: a ribbon of the given half width that
/// follows the polyline.
///
/// Joins use the angle bisector (a miter), which for a smoothed curve is
/// indistinguishable from a round join but costs a single point per sample.
/// Caps are round, or square when `square_cap` is set (the chisel tip of a
/// marker).
pub fn ribbon(points: &[(f32, f32)], half: f32, square_cap: bool) -> Vec<(f32, f32)> {
    let n = points.len();
    if n < 2 || half <= 0.0 {
        return Vec::new();
    }

    // Unit direction of every segment.
    let mut dirs: Vec<(f32, f32)> = Vec::with_capacity(n - 1);
    for i in 0..n - 1 {
        let dx = points[i + 1].0 - points[i].0;
        let dy = points[i + 1].1 - points[i].1;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-9 {
            dirs.push(*dirs.last().unwrap_or(&(1.0, 0.0)));
        } else {
            dirs.push((dx / len, dy / len));
        }
    }

    // Left and right offsets of every sample.
    let mut left: Vec<(f32, f32)> = Vec::with_capacity(n);
    let mut right: Vec<(f32, f32)> = Vec::with_capacity(n);
    for i in 0..n {
        let (tx, ty, scale) = if i == 0 {
            (dirs[0].0, dirs[0].1, 1.0)
        } else if i == n - 1 {
            (dirs[n - 2].0, dirs[n - 2].1, 1.0)
        } else {
            let (d0, d1) = (dirs[i - 1], dirs[i]);
            let bx = d0.0 + d1.0;
            let by = d0.1 + d1.1;
            let bl = (bx * bx + by * by).sqrt();
            if bl < 1e-6 {
                (d1.0, d1.1, 1.0)
            } else {
                let tx = bx / bl;
                let ty = by / bl;
                // Widen the offset so the two edges meet at the corner.
                let cos_half = (tx * d1.0 + ty * d1.1).abs().max(0.2);
                (tx, ty, 1.0 / cos_half)
            }
        };
        let nx = -ty * half * scale;
        let ny = tx * half * scale;
        left.push((points[i].0 + nx, points[i].1 + ny));
        right.push((points[i].0 - nx, points[i].1 - ny));
    }

    let mut out: Vec<(f32, f32)> = Vec::with_capacity(n * 2 + 32);
    out.extend_from_slice(&left);

    // End cap.
    let end = points[n - 1];
    if square_cap {
        let d = dirs[n - 2];
        let ex = end.0 + d.0 * half;
        let ey = end.1 + d.1 * half;
        let ox = left[n - 1].0 - end.0;
        let oy = left[n - 1].1 - end.1;
        out.push((ex + ox, ey + oy));
        out.push((ex - ox, ey - oy));
    } else {
        let a0 = (left[n - 1].1 - end.1).atan2(left[n - 1].0 - end.0);
        cap_arc(&mut out, end, half, a0, -std::f32::consts::PI);
    }

    // Right side, back to the start.
    for i in (0..n).rev() {
        out.push(right[i]);
    }

    // Start cap.
    let start = points[0];
    if square_cap {
        let d = dirs[0];
        let sx = start.0 - d.0 * half;
        let sy = start.1 - d.1 * half;
        let ox = right[0].0 - start.0;
        let oy = right[0].1 - start.1;
        // The outline arrives here along the right side, so the corner on that
        // side comes first; pushing them the other way crosses the shape.
        out.push((sx + ox, sy + oy));
        out.push((sx - ox, sy - oy));
    } else {
        let a0 = (right[0].1 - start.1).atan2(right[0].0 - start.0);
        // Sweeping backwards makes the cap bulge away from the stroke, so
        // both ends of the stroke are rounded outwards.
        cap_arc(&mut out, start, half, a0, -std::f32::consts::PI);
    }

    subdivide(&mut out);
    out
}

/// Split long edges into shorter pieces.
///
/// The tessellator that fills a path allocates per vertex, so the number of
/// pieces is capped: past a few hundred the cost of the fill outweighs any
/// gain in smoothness.
fn subdivide(poly: &mut Vec<(f32, f32)>) {
    /// Largest number of points a single stroke's outline may have.
    const MAX_POINTS: usize = 220;
    /// Shortest edge we bother splitting to.
    const MIN_LEN: f32 = 1.0;

    let n = poly.len();
    if n < 3 {
        return;
    }
    let mut total = 0.0f32;
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        total += ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    }
    let max_len = (total / MAX_POINTS as f32).max(MIN_LEN);

    let mut out: Vec<(f32, f32)> = Vec::with_capacity(n * 2);
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        out.push(a);
        let dx = b.0 - a.0;
        let dy = b.1 - a.1;
        let len = (dx * dx + dy * dy).sqrt();
        if len > max_len {
            let parts = (len / max_len).ceil() as usize;
            for k in 1..parts {
                let t = k as f32 / parts as f32;
                out.push((a.0 + dx * t, a.1 + dy * t));
            }
        }
    }
    *poly = out;
}
/// Append a semicircular cap.
fn cap_arc(out: &mut Vec<(f32, f32)>, center: (f32, f32), r: f32, a0: f32, sweep: f32) {
    const STEPS: usize = 10;
    for i in 0..=STEPS {
        let a = a0 + sweep * (i as f32 / STEPS as f32);
        out.push((center.0 + r * a.cos(), center.1 + r * a.sin()));
    }
}

/// Fill a set of polygons into a single path.
///
/// All contours are wound the same way and filled with the non-zero rule, so
/// overlapping polygons merge into one region. That is what keeps the
/// highlighter from stacking its opacity where strokes cross.
pub fn fill(polygons: &[Vec<(f32, f32)>]) -> Option<Path<Pixels>> {
    if polygons.is_empty() {
        return None;
    }
    let mut opts = FillOptions::default();
    // The default is even-odd, which turns overlapping shapes into holes.
    opts.fill_rule = FillRule::NonZero;
    let mut pb = PathBuilder::default().with_style(PathStyle::Fill(opts));

    for poly in polygons {
        if poly.len() < 3 {
            continue;
        }
        let mut pts = poly.clone();
        if signed_area(&pts) < 0.0 {
            pts.reverse();
        }
        pb.move_to(point(px(pts[0].0), px(pts[0].1)));
        for p in &pts[1..] {
            pb.line_to(point(px(p.0), px(p.1)));
        }
        pb.close();
    }

    pb.build().ok()
}

/// Signed area of a polygon; positive means counter-clockwise in a y-down
/// space.
fn signed_area(points: &[(f32, f32)]) -> f32 {
    let mut area = 0.0;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        area += a.0 * b.1 - b.0 * a.1;
    }
    area * 0.5
}

/// Remove every stroke the eraser touches.
///
/// The eraser takes whole strokes away rather than nibbling at them: a
/// stroke is removed as soon as the eraser reaches any part of it.
pub fn erase(strokes: &mut Vec<InkStroke>, x: f32, y: f32, radius: f32) {
    strokes.retain(|stroke| !stroke_touches(stroke, x, y, radius + stroke.width * 0.5));
}

/// Whether the eraser at `(x, y)` reaches any part of the stroke.
fn stroke_touches(stroke: &InkStroke, x: f32, y: f32, reach: f32) -> bool {
    let r2 = reach * reach;
    match stroke.points.len() {
        0 => false,
        1 => {
            let dx = stroke.points[0].0 - x;
            let dy = stroke.points[0].1 - y;
            dx * dx + dy * dy <= r2
        }
        _ => stroke
            .points
            .windows(2)
            .any(|w| dist_to_segment((x, y), w[0], w[1]) <= reach),
    }
}

/// Distance from a point to a line segment.
fn dist_to_segment(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let len2 = dx * dx + dy * dy;
    if len2 < 1e-9 {
        return ((p.0 - a.0).powi(2) + (p.1 - a.1).powi(2)).sqrt();
    }
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0);
    let cx = a.0 + dx * t;
    let cy = a.1 + dy * t;
    ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt()
}
