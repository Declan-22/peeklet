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
use std::sync::atomic::{AtomicU64, Ordering};

pub const TOOL_PEN: u8 = 1;
pub const TOOL_HIGHLIGHTER: u8 = 2;
pub const TOOL_ERASER: u8 = 3;

/// Pen width on screen, in pixels, at the fit-to-window zoom level.
pub const PEN_SCREEN_WIDTH: f32 = 2.5;
/// Highlighter width on screen, in pixels, at the fit-to-window zoom level.
pub const HIGHLIGHTER_SCREEN_WIDTH: f32 = 14.0;
/// Eraser radius on screen, in pixels, at the fit-to-window zoom level.
pub const ERASER_SCREEN_RADIUS: f32 = 14.0;

/// Gives every stroke a unique id, so history entries can find their stroke
/// again even after the list has been reordered.
static NEXT_STROKE_ID: AtomicU64 = AtomicU64::new(1);

/// A single annotation stroke, in the source image's pixel space.
#[derive(Clone, Debug)]
pub struct InkStroke {
    pub id: u64,
    pub tool: u8,
    /// The colour the stroke was drawn in, as 0xRRGGBB. Keeping it on the
    /// stroke means changing the colour later leaves earlier strokes alone.
    pub color: u32,
    /// Stroke width in source image pixels.
    pub width: f32,
    /// Points in source image pixels.
    pub points: Vec<(f32, f32)>,
}

impl InkStroke {
    pub fn new(tool: u8, color: u32, width: f32, points: Vec<(f32, f32)>) -> Self {
        Self {
            id: NEXT_STROKE_ID.fetch_add(1, Ordering::Relaxed),
            tool,
            color,
            width,
            points,
        }
    }
}

/// One change to an image's annotations.
#[derive(Clone, Debug)]
enum InkEdit {
    /// A stroke was drawn at `index` in the stroke list.
    Added { index: usize, stroke: InkStroke },
    /// The eraser took these strokes away.
    Removed(Vec<InkStroke>),
}

/// Undo/redo history for one image's annotations.
#[derive(Clone, Debug)]
pub struct InkHistory {
    edits: Vec<InkEdit>,
    /// How many edits are currently applied.
    cursor: usize,
    /// The cursor position that matches the file on disk.
    saved_cursor: Option<usize>,
}

impl Default for InkHistory {
    fn default() -> Self {
        Self {
            edits: Vec::new(),
            cursor: 0,
            saved_cursor: Some(0),
        }
    }
}

impl InkHistory {
    /// Record a stroke that was drawn.
    pub fn push_added(&mut self, index: usize, stroke: InkStroke) {
        self.edits.truncate(self.cursor);
        self.edits.push(InkEdit::Added { index, stroke });
        self.cursor = self.edits.len();
    }

    /// Record strokes that the eraser removed.
    pub fn push_removed(&mut self, strokes: Vec<InkStroke>) {
        if strokes.is_empty() {
            return;
        }
        self.edits.truncate(self.cursor);
        self.edits.push(InkEdit::Removed(strokes));
        self.cursor = self.edits.len();
    }

    /// Step back one edit, applying it to `strokes`.
    pub fn undo(&mut self, strokes: &mut Vec<InkStroke>) -> bool {
        if self.cursor == 0 {
            return false;
        }
        self.cursor -= 1;
        match &self.edits[self.cursor] {
            InkEdit::Added { index, stroke } => {
                if let Some(pos) = strokes.iter().position(|s| s.id == stroke.id) {
                    strokes.remove(pos);
                } else if *index < strokes.len() {
                    strokes.remove(*index);
                }
            }
            InkEdit::Removed(removed) => {
                for stroke in removed {
                    strokes.push(stroke.clone());
                }
            }
        }
        true
    }

    /// Step forward one edit, applying it to `strokes`.
    pub fn redo(&mut self, strokes: &mut Vec<InkStroke>) -> bool {
        if self.cursor >= self.edits.len() {
            return false;
        }
        match &self.edits[self.cursor] {
            InkEdit::Added { index, stroke } => {
                let at = (*index).min(strokes.len());
                strokes.insert(at, stroke.clone());
            }
            InkEdit::Removed(removed) => {
                let ids: Vec<u64> = removed.iter().map(|s| s.id).collect();
                strokes.retain(|s| !ids.contains(&s.id));
            }
        }
        self.cursor += 1;
        true
    }

    /// Shift every recorded stroke by `(dx, dy)` so undo/redo stays aligned
    /// after the underlying image is cropped.
    pub fn translate(&mut self, dx: f32, dy: f32) {
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        for edit in &mut self.edits {
            match edit {
                InkEdit::Added { stroke, .. } => {
                    for p in &mut stroke.points {
                        p.0 += dx;
                        p.1 += dy;
                    }
                }
                InkEdit::Removed(strokes) => {
                    for stroke in strokes {
                        for p in &mut stroke.points {
                            p.0 += dx;
                            p.1 += dy;
                        }
                    }
                }
            }
        }
    }

    /// Whether the annotations differ from what is on disk.
    pub fn is_dirty(&self) -> bool {
        self.saved_cursor != Some(self.cursor)
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
    let smooth_pass = |src: &[(f32, f32)]| -> Vec<(f32, f32)> {
        let mut dst: Vec<(f32, f32)> = Vec::with_capacity(src.len());
        dst.push(src[0]);
        for i in 1..src.len() - 1 {
            let q = src[i - 1];
            let p = src[i];
            let r = src[i + 1];
            // Preserve sharp direction reversals so reversing the stroke never
            // pulls the turnaround point backwards or eats the end of the line.
            let ax = p.0 - q.0;
            let ay = p.1 - q.1;
            let bx = r.0 - p.0;
            let by = r.1 - p.1;
            let al = (ax * ax + ay * ay).sqrt();
            let bl = (bx * bx + by * by).sqrt();
            let is_reversal = al > 1e-4 && bl > 1e-4 && (ax * bx + ay * by) / (al * bl) < 0.2;
            if is_reversal {
                dst.push(p);
            } else {
                dst.push((
                    (q.0 + 2.0 * p.0 + r.0) * 0.25,
                    (q.1 + 2.0 * p.1 + r.1) * 0.25,
                ));
            }
        }
        dst.push(src[src.len() - 1]);
        dst
    };
    let a = smooth_pass(points);
    smooth_pass(&a)
}

/// Build the silhouette of a stroke: a ribbon of the given half width that
/// follows the polyline.
///
/// When `square_cap` is false (pen), caps and outer joins are rounded with
/// circular arcs. When `square_cap` is true (highlighter), caps are square
/// and outer joins are flat bevels so sharp zig-zags keep a crisp, chiseled
/// edge without miter spikes.
pub fn ribbon(points: &[(f32, f32)], half: f32, square_cap: bool) -> Vec<(f32, f32)> {
    if points.len() < 2 || half <= 0.0 {
        return Vec::new();
    }

    // Drop consecutive coincident points so every segment has a well-defined
    // direction and length.
    let mut pts: Vec<(f32, f32)> = Vec::with_capacity(points.len());
    pts.push(points[0]);
    for &p in &points[1..] {
        let last = *pts.last().unwrap();
        let dx = p.0 - last.0;
        let dy = p.1 - last.1;
        if dx * dx + dy * dy >= 1e-8 {
            pts.push(p);
        }
    }
    let n = pts.len();
    if n < 2 {
        return Vec::new();
    }

    // Unit direction and length of every segment.
    let mut dirs: Vec<(f32, f32)> = Vec::with_capacity(n - 1);
    let mut lens: Vec<f32> = Vec::with_capacity(n - 1);
    for i in 0..n - 1 {
        let dx = pts[i + 1].0 - pts[i].0;
        let dy = pts[i + 1].1 - pts[i].1;
        let len = (dx * dx + dy * dy).sqrt();
        dirs.push((dx / len, dy / len));
        lens.push(len);
    }

    // Left and right offsets of every sample, with round (pen) or flat bevel
    // (highlighter) joins at corners.
    let mut left: Vec<(f32, f32)> = Vec::with_capacity(n * 2);
    let mut right: Vec<(f32, f32)> = Vec::with_capacity(n * 2);

    // Start sample.
    let d_start = dirs[0];
    let nx0 = -d_start.1 * half;
    let ny0 = d_start.0 * half;
    left.push((pts[0].0 + nx0, pts[0].1 + ny0));
    right.push((pts[0].0 - nx0, pts[0].1 - ny0));

    // Interior joins.
    const STRAIGHT_ANGLE: f32 = 0.05;
    const ARC_STEP_ANGLE: f32 = std::f32::consts::PI / 10.0;
    for i in 1..n - 1 {
        let p = pts[i];
        let d0 = dirs[i - 1];
        let d1 = dirs[i];
        let dot = (d0.0 * d1.0 + d0.1 * d1.1).clamp(-1.0, 1.0);
        let angle = dot.acos();

        if angle < STRAIGHT_ANGLE {
            let bx = d0.0 + d1.0;
            let by = d0.1 + d1.1;
            let bl = (bx * bx + by * by).sqrt();
            let (tx, ty, scale) = if bl < 1e-6 {
                (d1.0, d1.1, 1.0)
            } else {
                let tx = bx / bl;
                let ty = by / bl;
                let cos_half = (tx * d1.0 + ty * d1.1).abs().max(0.5);
                (tx, ty, 1.0 / cos_half)
            };
            let nx = -ty * half * scale;
            let ny = tx * half * scale;
            left.push((p.0 + nx, p.1 + ny));
            right.push((p.0 - nx, p.1 - ny));
            continue;
        }

        let n0 = (-d0.1, d0.0);
        let n1 = (-d1.1, d1.0);
        let l0 = (p.0 + n0.0 * half, p.1 + n0.1 * half);
        let l1 = (p.0 + n1.0 * half, p.1 + n1.1 * half);
        let r0 = (p.0 - n0.0 * half, p.1 - n0.1 * half);
        let r1 = (p.0 - n1.0 * half, p.1 - n1.1 * half);

        let cross = d0.0 * d1.1 - d0.1 * d1.0;

        // On the inside of a gentle corner where both adjacent segments are
        // long enough, the miter vertex is the exact clean intersection. On
        // sharp turns or short segments, route through the corner center `p`
        // so the inside never shoots out a spike or inverts winding.
        let inner_miter_ok = angle < std::f32::consts::PI * 0.75
            && half * (angle * 0.5).tan() < lens[i - 1].min(lens[i]);
        let inner_miter = if inner_miter_ok {
            let bx = d0.0 + d1.0;
            let by = d0.1 + d1.1;
            let bl = (bx * bx + by * by).sqrt();
            if bl >= 1e-6 {
                let tx = bx / bl;
                let ty = by / bl;
                let cos_half = (tx * d1.0 + ty * d1.1).abs().max(0.2);
                let scale = 1.0 / cos_half;
                Some((-ty * half * scale, tx * half * scale))
            } else {
                None
            }
        } else {
            None
        };

        if cross >= 0.0 {
            // Left is the inside of the turn; right is the outside.
            if let Some((nx, ny)) = inner_miter {
                left.push((p.0 + nx, p.1 + ny));
            } else {
                left.push(l0);
                left.push(p);
                left.push(l1);
            }
            right.push(r0);
            if !square_cap {
                let steps = (angle / ARC_STEP_ANGLE).ceil().max(1.0) as usize;
                let a0 = (-n0.1).atan2(-n0.0);
                for k in 1..steps {
                    let a = a0 + angle * (k as f32 / steps as f32);
                    right.push((p.0 + half * a.cos(), p.1 + half * a.sin()));
                }
            }
            right.push(r1);
        } else {
            // Left is the outside of the turn; right is the inside.
            left.push(l0);
            if !square_cap {
                let steps = (angle / ARC_STEP_ANGLE).ceil().max(1.0) as usize;
                let a0 = n0.1.atan2(n0.0);
                for k in 1..steps {
                    let a = a0 - angle * (k as f32 / steps as f32);
                    left.push((p.0 + half * a.cos(), p.1 + half * a.sin()));
                }
            }
            left.push(l1);
            if let Some((nx, ny)) = inner_miter {
                right.push((p.0 - nx, p.1 - ny));
            } else {
                right.push(r0);
                right.push(p);
                right.push(r1);
            }
        }
    }

    // End sample.
    let d_end = dirs[n - 2];
    let nxe = -d_end.1 * half;
    let nye = d_end.0 * half;
    let end = pts[n - 1];
    let left_end = (end.0 + nxe, end.1 + nye);
    let right_end = (end.0 - nxe, end.1 - nye);
    left.push(left_end);
    right.push(right_end);

    let mut out: Vec<(f32, f32)> = Vec::with_capacity(left.len() + right.len() + 32);
    out.extend_from_slice(&left);

    // End cap.
    // For the highlighter (`square_cap`), end flush at the cursor (butt cap)
    // so the tip never overhangs past the cursor and never folds/flips when
    // quickly reversing direction.
    if !square_cap {
        let a0 = (left_end.1 - end.1).atan2(left_end.0 - end.0);
        cap_arc(&mut out, end, half, a0, -std::f32::consts::PI);
    }

    // Right side, back to the start.
    for &p in right.iter().rev() {
        out.push(p);
    }

    // Start cap (flush butt cap for highlighter; semicircular arc for pen).
    let start = pts[0];
    if !square_cap {
        let a0 = (right[0].1 - start.1).atan2(right[0].0 - start.0);
        // Sweeping backwards makes the cap bulge away from the stroke, so
        // both ends of the stroke are rounded outwards.
        cap_arc(&mut out, start, half, a0, -std::f32::consts::PI);
    }

    out
}

/// Append the interior points of a semicircular cap.
fn cap_arc(out: &mut Vec<(f32, f32)>, center: (f32, f32), r: f32, a0: f32, sweep: f32) {
    const STEPS: usize = 10;
    for i in 1..STEPS {
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
        if signed_area(poly) < 0.0 {
            let mut it = poly.iter().rev();
            if let Some(&first) = it.next() {
                pb.move_to(point(px(first.0), px(first.1)));
                for &p in it {
                    pb.line_to(point(px(p.0), px(p.1)));
                }
                pb.close();
            }
        } else {
            pb.move_to(point(px(poly[0].0), px(poly[0].1)));
            for &p in &poly[1..] {
                pb.line_to(point(px(p.0), px(p.1)));
            }
            pb.close();
        }
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
///
/// Returns the strokes that were removed.
pub fn erase(strokes: &mut Vec<InkStroke>, x: f32, y: f32, radius: f32) -> Vec<InkStroke> {
    let mut removed = Vec::new();
    let mut kept = Vec::with_capacity(strokes.len());
    for stroke in strokes.drain(..) {
        if stroke_touches(&stroke, x, y, radius + stroke.width * 0.5) {
            removed.push(stroke);
        } else {
            kept.push(stroke);
        }
    }
    *strokes = kept;
    removed
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

#[cfg(test)]
mod tests {
    use super::*;

    fn dist_to_polyline(p: (f32, f32), line: &[(f32, f32)]) -> f32 {
        line.windows(2)
            .map(|w| dist_to_segment(p, w[0], w[1]))
            .fold(f32::INFINITY, f32::min)
    }

    #[test]
    fn sharp_zigzag_has_no_miter_spikes_round_cap() {
        let half = 7.0;
        let cases: &[&[(f32, f32)]] = &[
            // 90-degree turns
            &[(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (200.0, 100.0)],
            // Sharp zig-zag (~165-degree turns)
            &[
                (0.0, 0.0),
                (120.0, 0.0),
                (10.0, 20.0),
                (130.0, 40.0),
                (20.0, 60.0),
            ],
            // Extreme hairpin (~178-degree turn)
            &[(0.0, 0.0), (200.0, 0.0), (0.0, 4.0)],
            // Exact 180-degree reversal
            &[(0.0, 0.0), (100.0, 0.0), (0.0, 0.0)],
            // Short segments around a sharp turn
            &[(0.0, 0.0), (3.0, 0.0), (0.5, 1.0), (4.0, 2.0)],
        ];

        for &pts in cases {
            let poly = ribbon(pts, half, false);
            assert!(poly.len() >= 3);
            for &v in &poly {
                assert!(v.0.is_finite() && v.1.is_finite());
                let d = dist_to_polyline(v, pts);
                assert!(
                    d <= half + 1e-2,
                    "vertex {:?} spiked to distance {} (half={}) for {:?}",
                    v,
                    d,
                    half,
                    pts
                );
            }
            assert!(fill(&[poly]).is_some());
        }
    }

    #[test]
    fn sharp_zigzag_has_no_miter_spikes_square_cap() {
        let half = 10.0;
        let pts = &[
            (0.0, 0.0),
            (150.0, 0.0),
            (10.0, 15.0),
            (160.0, 30.0),
            (20.0, 45.0),
        ];
        let poly = ribbon(pts, half, true);
        assert!(poly.len() >= 3);

        // Square caps at the start/end extend up to half * sqrt(2) from the
        // endpoints, while every interior turn must stay within `half`.
        let max_cap_dist = half * std::f32::consts::SQRT_2 + 1e-2;
        for &v in &poly {
            assert!(v.0.is_finite() && v.1.is_finite());
            let d = dist_to_polyline(v, pts);
            assert!(
                d <= max_cap_dist,
                "vertex {:?} exceeded max cap distance {} (got {})",
                v,
                max_cap_dist,
                d
            );
            // At the sharp interior apexes (x > 100), vertices must stay
            // within the round-join radius `half`.
            if v.0 > 100.0 {
                assert!(
                    d <= half + 1e-2,
                    "interior turn vertex {:?} spiked to distance {} (half={})",
                    v,
                    d,
                    half
                );
            }
        }
        assert!(fill(&[poly]).is_some());
    }

    #[test]
    fn full_pipeline_fast_zigzag_stays_smooth() {
        let raw = vec![
            (0.0, 0.0),
            (300.0, 10.0),
            (10.0, 35.0),
            (310.0, 50.0),
            (20.0, 75.0),
            (320.0, 90.0),
        ];
        let smoothed = smooth(&raw);

        for &(tol, half, square_cap) in &[(0.4_f32, 2.5_f32, false), (1.0_f32, 14.0_f32, true)] {
            let curve = spline(&smoothed, tol, 220);
            let poly = ribbon(&curve, half, square_cap);
            assert!(poly.len() >= 3);
            let limit = if square_cap {
                half * std::f32::consts::SQRT_2 + 1e-2
            } else {
                half + 1e-2
            };
            for &v in &poly {
                assert!(v.0.is_finite() && v.1.is_finite());
                let d = dist_to_polyline(v, &curve);
                assert!(
                    d <= limit,
                    "pipeline vertex {:?} spiked to {} (limit={})",
                    v,
                    d,
                    limit
                );
            }
            assert!(fill(&[poly]).is_some());
        }
    }

    #[test]
    fn highlighter_uses_flat_bevel_joins() {
        // 90-degree corner at (100, 0), turning down to (100, 100).
        // Outside bevel connects (100, -10) straight to (110, 0) with no arc
        // bulge past the bevel line x - y = 110.
        let pts = &[(0.0, 0.0), (100.0, 0.0), (100.0, 100.0)];
        let poly = ribbon(pts, 10.0, true);
        for &v in &poly {
            if v.0 > 100.0 && v.1 < 0.0 {
                panic!("unexpected rounded arc vertex {:?} on beveled highlighter corner", v);
            }
        }
        assert!(fill(&[poly]).is_some());
    }

    #[test]
    fn ribbon_handles_duplicate_points() {
        let pts = &[(10.0, 10.0), (10.0, 10.0), (50.0, 10.0), (50.0, 10.0), (10.0, 12.0)];
        let poly = ribbon(pts, 5.0, false);
        assert!(poly.len() >= 3);
        for &v in &poly {
            assert!(v.0.is_finite() && v.1.is_finite());
        }
        assert!(ribbon(&[(5.0, 5.0), (5.0, 5.0)], 5.0, false).is_empty());
    }

    #[test]
    fn highlighter_ends_flush_and_keeps_reach_on_reversal() {
        // Horizontal highlighter stroke should end flush at x=0 and x=100
        // (cursor is at the edge, not half-width behind the tip).
        let straight = ribbon(&[(0.0, 0.0), (100.0, 0.0)], 10.0, true);
        let max_x = straight.iter().map(|v| v.0).fold(f32::NEG_INFINITY, f32::max);
        let min_x = straight.iter().map(|v| v.0).fold(f32::INFINITY, f32::min);
        assert!((max_x - 100.0).abs() < 1e-3, "expected flush end at 100, got {max_x}");
        assert!(min_x.abs() < 1e-3, "expected flush start at 0, got {min_x}");

        // Reversing direction at x=100 should preserve the turnaround reach.
        let reversed = smooth(&[(0.0, 0.0), (50.0, 0.0), (100.0, 0.0), (60.0, 0.0)]);
        let peak_x = reversed.iter().map(|v| v.0).fold(f32::NEG_INFINITY, f32::max);
        assert!((peak_x - 100.0).abs() < 1e-3, "reversal should preserve x=100, got {peak_x}");
    }
}
