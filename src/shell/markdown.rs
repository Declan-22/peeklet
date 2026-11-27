//! Markdown symbol highlighting and live preview rendering.
//!
//! Supports three view modes in the editor:
//! - Plain: raw Markdown with `accent` syntax highlighting on Markdown symbols.
//! - Split: editable raw Markdown on the left, rendered preview on the right.
//! - Stacked (live preview): rendered preview for every line except the active
//!   cursor line, which reveals its raw Markdown symbols for direct editing.

use std::ops::Range;

use gpui::{
    AnyElement, App, FontStyle, FontWeight, HighlightStyle, Hsla, MouseButton, StrikethroughStyle,
    StyledText, UnderlineStyle, Window, div, prelude::*, px, rgba, svg,
};

const TODO_CHECK_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="M5 13l4 4L19 7"/></svg>"#;

/// Parse a Markdown task list line (`- [ ] ...`, `* [x] ...`, `+ [X] ...`).
/// Returns `(check_char_byte_offset, is_checked, content_byte_offset)` if matched.
pub fn parse_task_line(line: &str) -> Option<(usize, bool, usize)> {
    let trimmed = line.trim_start();
    let ws_len = line.len() - trimmed.len();
    if trimmed.len() < 5
        || !matches!(trimmed.as_bytes()[0], b'-' | b'*' | b'+')
        || trimmed.as_bytes()[1] != b' '
    {
        return None;
    }
    let after = &trimmed[2..];
    if after.len() < 3 || after.as_bytes()[0] != b'[' || after.as_bytes()[2] != b']' {
        return None;
    }
    let checked = match after.as_bytes()[1] {
        b' ' => false,
        b'x' | b'X' => true,
        _ => return None,
    };
    let after_bracket = if after.len() >= 4 {
        if after.as_bytes()[3] != b' ' {
            return None;
        }
        4
    } else {
        3
    };
    Some((ws_len + 3, checked, ws_len + 2 + after_bracket))
}

use crate::theme::Theme;

/// Whether a file path is a Markdown document.
pub fn is_markdown_path(path: &str) -> bool {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(ext.as_str(), "md" | "markdown")
}

/// Whether a line starts or ends a fenced code block (```` ``` ```` or `~~~`).
pub fn is_fence_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("```") || trimmed.starts_with("~~~")
}

/// Whether a line is a Markdown table separator row (`|---|---|`, `|:---|---:|`, etc.).
pub fn is_table_separator(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.contains('|') {
        return false;
    }
    let inner = trimmed.strip_prefix('|').unwrap_or(trimmed);
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    let mut cell_count = 0usize;
    for raw_cell in inner.split('|') {
        let cell = raw_cell.trim();
        if cell.is_empty() {
            return false;
        }
        let body = cell.strip_prefix(':').unwrap_or(cell);
        let body = body.strip_suffix(':').unwrap_or(body);
        if body.is_empty() || !body.bytes().all(|b| b == b'-') {
            return false;
        }
        cell_count += 1;
    }
    cell_count >= 1
}

/// Split a Markdown table row into trimmed cell slices.
pub fn parse_table_cells(line: &str) -> Option<Vec<&str>> {
    let trimmed = line.trim();
    if !trimmed.contains('|') {
        return None;
    }
    let has_outer_pipes = trimmed.starts_with('|');
    let inner = trimmed.strip_prefix('|').unwrap_or(trimmed);
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    let cells: Vec<&str> = inner.split('|').map(|c| c.trim()).collect();
    if cells.is_empty() || (!has_outer_pipes && cells.len() < 2) {
        return None;
    }
    Some(cells)
}

/// Role of a buffer line within a Markdown table.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TableRowKind {
    None,
    Header,
    Separator,
    Body { is_last: bool },
}

/// Whether a line is a horizontal rule (`---`, `***`, `___`).
fn is_horizontal_rule(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.len() < 3 {
        return false;
    }
    let first = match trimmed.bytes().next() {
        Some(b @ (b'-' | b'*' | b'_')) => b,
        _ => return false,
    };
    let mut count = 0usize;
    for b in trimmed.bytes() {
        if b == first {
            count += 1;
        } else if b != b' ' && b != b'\t' {
            return false;
        }
    }
    count >= 3
}

/// Highlight all Markdown syntax symbols in `accent`, leaving text in
/// `default_color`.
pub fn markdown_symbol_spans(
    line: &str,
    accent: u32,
    default_color: u32,
) -> Vec<(u32, usize, usize)> {
    let n = line.len();
    if n == 0 {
        return Vec::new();
    }
    let bytes = line.as_bytes();
    let mut marks = vec![false; n];

    // Table separator row (`|---|---|`): highlight the whole separator.
    if is_table_separator(line) {
        return vec![(accent, 0, n)];
    }

    // Horizontal rule: highlight the entire rule.
    if is_horizontal_rule(line) {
        return vec![(accent, 0, n)];
    }

    // Leading whitespace.
    let mut pos = 0;
    while pos < n && matches!(bytes[pos], b' ' | b'\t') {
        pos += 1;
    }

    // Fenced code block delimiter (``` or ~~~).
    if line[pos..].starts_with("```") || line[pos..].starts_with("~~~") {
        let fence = bytes[pos];
        let mut end = pos;
        while end < n && bytes[end] == fence {
            marks[end] = true;
            end += 1;
        }
        return marks_to_spans(&marks, accent, default_color);
    }

    // Blockquote prefix (`> `).
    while pos < n && bytes[pos] == b'>' {
        marks[pos] = true;
        pos += 1;
        if pos < n && bytes[pos] == b' ' {
            pos += 1;
        }
    }

    // ATX heading prefix (`#`..`######` followed by space or end).
    if pos < n && bytes[pos] == b'#' {
        let mut h_end = pos;
        while h_end < n && bytes[h_end] == b'#' && h_end - pos < 6 {
            h_end += 1;
        }
        if h_end == n || bytes[h_end] == b' ' {
            for m in &mut marks[pos..h_end] {
                *m = true;
            }
            pos = h_end;
        }
    } else if pos < n {
        // Unordered list marker (`- `, `* `, `+ `) + optional task checkbox.
        if matches!(bytes[pos], b'-' | b'*' | b'+') && pos + 1 < n && bytes[pos + 1] == b' ' {
            marks[pos] = true;
            pos += 2;
            if pos + 3 <= n
                && bytes[pos] == b'['
                && matches!(bytes[pos + 1], b' ' | b'x' | b'X')
                && bytes[pos + 2] == b']'
                && (pos + 3 == n || bytes[pos + 3] == b' ')
            {
                marks[pos] = true;
                marks[pos + 1] = true;
                marks[pos + 2] = true;
                pos += 3;
            }
        } else if bytes[pos].is_ascii_digit() {
            // Ordered list marker (`1. ` or `1) `).
            let mut d_end = pos;
            while d_end < n && bytes[d_end].is_ascii_digit() {
                d_end += 1;
            }
            if d_end + 1 < n && matches!(bytes[d_end], b'.' | b')') && bytes[d_end + 1] == b' ' {
                for m in &mut marks[pos..=d_end] {
                    *m = true;
                }
                pos = d_end + 2;
            }
        }
    }

    // Inline Markdown symbols: backticks, *, _, ~, !, [, ], (, ).
    let mut i = pos;
    let mut in_code = false;
    while i < n {
        let b = bytes[i];
        if b == b'\\' && i + 1 < n {
            // Escaped symbol: highlight the backslash itself.
            marks[i] = true;
            i += 2;
            continue;
        }
        if b == b'`' {
            marks[i] = true;
            in_code = !in_code;
            i += 1;
            continue;
        }
        if in_code {
            i += 1;
            continue;
        }
        match b {
            b'*' | b'_' | b'|' => {
                marks[i] = true;
                i += 1;
            }
            b'~' if i + 1 < n && bytes[i + 1] == b'~' => {
                marks[i] = true;
                marks[i + 1] = true;
                i += 2;
            }
            b'!' if i + 1 < n && bytes[i + 1] == b'[' => {
                marks[i] = true;
                marks[i + 1] = true;
                i += 2;
            }
            b'[' | b']' => {
                marks[i] = true;
                i += 1;
            }
            b'(' if i > 0 && bytes[i - 1] == b']' => {
                // Highlight link URL parentheses `(` and matching `)`.
                marks[i] = true;
                if let Some(close) = bytes[i + 1..].iter().position(|&c| c == b')') {
                    marks[i + 1 + close] = true;
                }
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    marks_to_spans(&marks, accent, default_color)
}

fn marks_to_spans(marks: &[bool], accent: u32, default_color: u32) -> Vec<(u32, usize, usize)> {
    let mut out = Vec::new();
    let mut start = 0usize;
    while start < marks.len() {
        let is_sym = marks[start];
        let mut end = start + 1;
        while end < marks.len() && marks[end] == is_sym {
            end += 1;
        }
        out.push((if is_sym { accent } else { default_color }, start, end));
        start = end;
    }
    out
}

/// Inline parsed text + GPUI `HighlightStyle` ranges + clickable URL ranges.
struct InlineRendered {
    text: String,
    highlights: Vec<(Range<usize>, HighlightStyle)>,
    links: Vec<(Range<usize>, String)>,
}

/// Merge an inner `HighlightStyle` on top of an outer inherited style.
fn merge_style(base: HighlightStyle, overlay: HighlightStyle) -> HighlightStyle {
    HighlightStyle {
        color: overlay.color.or(base.color),
        font_weight: overlay.font_weight.or(base.font_weight),
        font_style: overlay.font_style.or(base.font_style),
        background_color: overlay.background_color.or(base.background_color),
        underline: overlay.underline.or(base.underline),
        strikethrough: overlay.strikethrough.or(base.strikethrough),
        fade_out: overlay.fade_out.or(base.fade_out),
    }
}

fn style_is_empty(s: &HighlightStyle) -> bool {
    s.color.is_none()
        && s.font_weight.is_none()
        && s.font_style.is_none()
        && s.background_color.is_none()
        && s.underline.is_none()
        && s.strikethrough.is_none()
        && s.fade_out.is_none()
}

/// Parse inline Markdown (`**bold**`, `*italic*`, `***both***`, `~~strike~~`,
/// `` `code` ``, `[label](url)`, `![alt](url)`), including nested combinations
/// such as ``**`POST /preview?device_id=X`**``, into clean display text and
/// sorted, non-overlapping GPUI highlight styles.
fn parse_inline(src: &str, t: &Theme) -> InlineRendered {
    let mut out = String::with_capacity(src.len());
    let mut highlights: Vec<(Range<usize>, HighlightStyle)> = Vec::new();
    let mut links: Vec<(Range<usize>, String)> = Vec::new();
    parse_inline_into(
        src,
        t,
        HighlightStyle::default(),
        &mut out,
        &mut highlights,
        &mut links,
    );
    InlineRendered {
        text: out,
        highlights,
        links,
    }
}

fn parse_inline_into(
    src: &str,
    t: &Theme,
    base: HighlightStyle,
    out: &mut String,
    highlights: &mut Vec<(Range<usize>, HighlightStyle)>,
    links: &mut Vec<(Range<usize>, String)>,
) {
    let bytes = src.as_bytes();
    let n = bytes.len();
    let has_base = !style_is_empty(&base);
    let mut plain_start = out.len();

    let flush_plain = |plain_start: &mut usize,
                       out: &String,
                       highlights: &mut Vec<(Range<usize>, HighlightStyle)>| {
        if has_base && *plain_start < out.len() {
            highlights.push((*plain_start..out.len(), base));
        }
        *plain_start = out.len();
    };

    let mut i = 0usize;
    while i < n {
        // Escaped character.
        if bytes[i] == b'\\' && i + 1 < n {
            let ch = src[i + 1..].chars().next().unwrap();
            out.push(ch);
            i += 1 + ch.len_utf8();
            continue;
        }

        // Inline code: `...` (leaf — no further Markdown parsing inside).
        if bytes[i] == b'`' {
            if let Some(end_rel) = src[i + 1..].find('`') {
                flush_plain(&mut plain_start, out, highlights);
                let start = out.len();
                out.push_str(&src[i + 1..i + 1 + end_rel]);
                let end = out.len();
                if start < end {
                    let style = merge_style(
                        base,
                        HighlightStyle {
                            color: Some(Hsla::from(rgba(t.accent))),
                            background_color: Some(Hsla::from(rgba(t.bg_sunken))),
                            ..Default::default()
                        },
                    );
                    highlights.push((start..end, style));
                }
                plain_start = out.len();
                i = i + 1 + end_rel + 1;
                continue;
            }
        }

        // Image `![alt](url)` or Link `[label](url)`
        let is_image = bytes[i] == b'!' && i + 1 < n && bytes[i + 1] == b'[';
        if is_image || bytes[i] == b'[' {
            let lb = if is_image { i + 1 } else { i };
            if let Some(rb_rel) = src[lb + 1..].find(']') {
                let rb = lb + 1 + rb_rel;
                if rb + 1 < n && bytes[rb + 1] == b'(' {
                    if let Some(rp_rel) = src[rb + 2..].find(')') {
                        flush_plain(&mut plain_start, out, highlights);
                        let label = &src[lb + 1..rb];
                        let url = src[rb + 2..rb + 2 + rp_rel].trim().to_string();
                        let link_style = merge_style(
                            base,
                            HighlightStyle {
                                color: Some(Hsla::from(rgba(if is_image {
                                    t.text_secondary
                                } else {
                                    t.accent
                                }))),
                                underline: if is_image {
                                    None
                                } else {
                                    Some(UnderlineStyle {
                                        thickness: px(1.),
                                        color: Some(Hsla::from(rgba(t.accent))),
                                        wavy: false,
                                    })
                                },
                                ..Default::default()
                            },
                        );
                        let link_start = out.len();
                        parse_inline_into(label, t, link_style, out, highlights, links);
                        let link_end = out.len();
                        if !is_image && link_start < link_end && !url.is_empty() {
                            links.push((link_start..link_end, url));
                        }
                        plain_start = out.len();
                        i = rb + 2 + rp_rel + 1;
                        continue;
                    }
                }
            }
        }

        // Bold + italic: ***...*** or ___...___
        if (src[i..].starts_with("***") || src[i..].starts_with("___")) && i + 3 < n {
            let delim = &src[i..i + 3];
            if let Some(end_rel) = src[i + 3..].find(delim) {
                if end_rel > 0 {
                    flush_plain(&mut plain_start, out, highlights);
                    let style = merge_style(
                        base,
                        HighlightStyle {
                            font_weight: Some(FontWeight::SEMIBOLD),
                            font_style: Some(FontStyle::Italic),
                            ..Default::default()
                        },
                    );
                    parse_inline_into(&src[i + 3..i + 3 + end_rel], t, style, out, highlights, links);
                    plain_start = out.len();
                    i = i + 3 + end_rel + 3;
                    continue;
                }
            }
        }

        // Bold: **...** or __...__
        if (src[i..].starts_with("**") || src[i..].starts_with("__")) && i + 2 < n {
            let delim = &src[i..i + 2];
            if let Some(end_rel) = src[i + 2..].find(delim) {
                if end_rel > 0 {
                    flush_plain(&mut plain_start, out, highlights);
                    let style = merge_style(
                        base,
                        HighlightStyle {
                            font_weight: Some(FontWeight::SEMIBOLD),
                            ..Default::default()
                        },
                    );
                    parse_inline_into(&src[i + 2..i + 2 + end_rel], t, style, out, highlights, links);
                    plain_start = out.len();
                    i = i + 2 + end_rel + 2;
                    continue;
                }
            }
        }

        // Strikethrough: ~~...~~
        if src[i..].starts_with("~~") && i + 2 < n {
            if let Some(end_rel) = src[i + 2..].find("~~") {
                if end_rel > 0 {
                    flush_plain(&mut plain_start, out, highlights);
                    let style = merge_style(
                        base,
                        HighlightStyle {
                            color: Some(Hsla::from(rgba(t.text_secondary))),
                            strikethrough: Some(StrikethroughStyle {
                                thickness: px(1.),
                                color: Some(Hsla::from(rgba(t.text_secondary))),
                            }),
                            ..Default::default()
                        },
                    );
                    parse_inline_into(&src[i + 2..i + 2 + end_rel], t, style, out, highlights, links);
                    plain_start = out.len();
                    i = i + 2 + end_rel + 2;
                    continue;
                }
            }
        }

        // Italic: *...* or _..._
        if matches!(bytes[i], b'*' | b'_') && i + 1 < n && bytes[i + 1] != b' ' {
            let delim = bytes[i] as char;
            if let Some(end_rel) = src[i + 1..].find(delim) {
                if end_rel > 0 && bytes[i + end_rel] != b' ' {
                    flush_plain(&mut plain_start, out, highlights);
                    let style = merge_style(
                        base,
                        HighlightStyle {
                            font_style: Some(FontStyle::Italic),
                            ..Default::default()
                        },
                    );
                    parse_inline_into(&src[i + 1..i + 1 + end_rel], t, style, out, highlights, links);
                    plain_start = out.len();
                    i = i + 1 + end_rel + 1;
                    continue;
                }
            }
        }

        // Bare autolink (`https://...` or `http://...`).
        if src[i..].starts_with("https://") || src[i..].starts_with("http://") {
            let mut end_i = i;
            while end_i < n
                && !bytes[end_i].is_ascii_whitespace()
                && !matches!(bytes[end_i], b')' | b']' | b'>' | b'"' | b'\'')
            {
                end_i += 1;
            }
            while end_i > i && matches!(bytes[end_i - 1], b'.' | b',' | b';' | b':') {
                end_i -= 1;
            }
            if end_i > i + 8 {
                flush_plain(&mut plain_start, out, highlights);
                let url = src[i..end_i].to_string();
                let start = out.len();
                out.push_str(&url);
                let end = out.len();
                let style = merge_style(
                    base,
                    HighlightStyle {
                        color: Some(Hsla::from(rgba(t.accent))),
                        underline: Some(UnderlineStyle {
                            thickness: px(1.),
                            color: Some(Hsla::from(rgba(t.accent))),
                            wavy: false,
                        }),
                        ..Default::default()
                    },
                );
                highlights.push((start..end, style));
                links.push((start..end, url));
                plain_start = out.len();
                i = end_i;
                continue;
            }
        }

        // Plain UTF-8 character.
        let ch = src[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }

    flush_plain(&mut plain_start, out, highlights);
}

/// Slice highlight styles overlapping `slice_range`, shifted relative to `slice_range.start`.
fn slice_highlights(
    highlights: &[(Range<usize>, HighlightStyle)],
    slice_range: Range<usize>,
) -> Vec<(Range<usize>, HighlightStyle)> {
    highlights
        .iter()
        .filter_map(|(r, style)| {
            let a = r.start.max(slice_range.start);
            let b = r.end.min(slice_range.end);
            (a < b).then(|| ((a - slice_range.start)..(b - slice_range.start), *style))
        })
        .collect()
}

/// Render parsed inline Markdown. When links are present, only the link text
/// itself gets `cursor_pointer()` and opens the URL on click; clicking anywhere
/// else on the line bubbles up to edit the line normally.
fn render_inline_element(parsed: InlineRendered) -> AnyElement {
    if parsed.links.is_empty() {
        return StyledText::new(parsed.text)
            .with_highlights(parsed.highlights)
            .into_any_element();
    }
    let mut row = div().flex().flex_row().items_center().whitespace_nowrap();
    let mut cursor = 0usize;
    for (range, url) in parsed.links {
        if range.start > cursor {
            let seg = parsed.text[cursor..range.start].to_string();
            let hi = slice_highlights(&parsed.highlights, cursor..range.start);
            row = row.child(StyledText::new(seg).with_highlights(hi));
        }
        let seg = parsed.text[range.start..range.end].to_string();
        let hi = slice_highlights(&parsed.highlights, range.start..range.end);
        row = row.child(
            div()
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    cx.open_url(&url);
                    cx.stop_propagation();
                })
                .child(StyledText::new(seg).with_highlights(hi)),
        );
        cursor = range.end;
    }
    if cursor < parsed.text.len() {
        let seg = parsed.text[cursor..].to_string();
        let hi = slice_highlights(&parsed.highlights, cursor..parsed.text.len());
        row = row.child(StyledText::new(seg).with_highlights(hi));
    }
    row.into_any_element()
}

/// Render a full Markdown table (header + body rows, with wrapping cell text)
/// for the Split-view preview pane.
pub fn render_markdown_table(
    header_cells: &[&str],
    body_rows: &[Vec<&str>],
    font_size: f32,
    font_family: &'static str,
    t: &Theme,
) -> AnyElement {
    let col_count = header_cells
        .len()
        .max(body_rows.iter().map(|r| r.len()).max().unwrap_or(0))
        .max(1);

    let make_row = |cells: &[&str], is_header: bool, is_last: bool| {
        let mut row = div()
            .w_full()
            .flex()
            .flex_row()
            .items_stretch()
            .bg(rgba(if is_header { t.bg_sunken } else { t.bg_canvas }));
        if !is_last {
            row = row.border_b_1().border_color(rgba(t.border_color));
        }
        for col in 0..col_count {
            let cell_text = cells.get(col).copied().unwrap_or("");
            let parsed = parse_inline(cell_text, t);
            let mut cell = div()
                .flex_1()
                .min_w(px(0.))
                .px(px(10.))
                .py(px(6.))
                .font_family(font_family)
                .text_size(px(font_size - 0.5))
                .font_weight(if is_header {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .text_color(rgba(t.text_primary))
                .child(render_inline_element(parsed));
            if col + 1 < col_count {
                cell = cell.border_r_1().border_color(rgba(t.border_color));
            }
            row = row.child(cell);
        }
        row
    };

    let mut table = div()
        .w_full()
        .my(px(4.))
        .flex()
        .flex_col()
        .border_1()
        .border_color(rgba(t.border_color))
        .rounded(px(6.))
        .overflow_hidden()
        .child(make_row(header_cells, true, body_rows.is_empty()));

    for (idx, cells) in body_rows.iter().enumerate() {
        table = table.child(make_row(cells, false, idx + 1 == body_rows.len()));
    }

    table.into_any_element()
}

/// Render a single Markdown table row at uniform `row_height` (for Stacked
/// live-preview mode where every buffer line keeps 1-to-1 vertical alignment).
pub fn render_markdown_table_line(
    full_line: &str,
    kind: TableRowKind,
    is_first_wrap_row: bool,
    row_height: f32,
    font_size: f32,
    font_family: &'static str,
    t: &Theme,
) -> AnyElement {
    if !is_first_wrap_row {
        return div().w_full().h(px(row_height)).into_any_element();
    }
    match kind {
        TableRowKind::Separator => {
            return div()
                .w_full()
                .h(px(row_height))
                .flex()
                .items_center()
                .child(div().w_full().h(px(1.)).bg(rgba(t.border_color)))
                .into_any_element();
        }
        TableRowKind::Header | TableRowKind::Body { .. } => {
            let is_header = kind == TableRowKind::Header;
            let is_last = matches!(kind, TableRowKind::Body { is_last: true });
            let cells = parse_table_cells(full_line).unwrap_or_default();
            let col_count = cells.len().max(1);
            let mut row = div()
                .w_full()
                .h(px(row_height))
                .flex()
                .flex_row()
                .items_stretch()
                .border_l_1()
                .border_r_1()
                .border_color(rgba(t.border_color))
                .bg(rgba(if is_header { t.bg_sunken } else { t.bg_canvas }));
            if is_header {
                row = row.border_t_1().rounded_t(px(6.));
            }
            if is_last {
                row = row.border_b_1().rounded_b(px(6.));
            } else if !is_header {
                row = row.border_b_1();
            }
            for col in 0..col_count {
                let cell_text = cells.get(col).copied().unwrap_or("");
                let parsed = parse_inline(cell_text, t);
                let mut cell = div()
                    .flex_1()
                    .min_w(px(0.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .font_family(font_family)
                    .text_size(px(font_size - 0.5))
                    .font_weight(if is_header {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .text_color(rgba(t.text_primary))
                    .child(render_inline_element(parsed));
                if col + 1 < col_count {
                    cell = cell.border_r_1().border_color(rgba(t.border_color));
                }
                row = row.child(cell);
            }
            row.into_any_element()
        }
        TableRowKind::None => div().w_full().h(px(row_height)).into_any_element(),
    }
}

/// Whether a line is a Markdown blockquote line (`> ...`).
pub fn is_quote_line(line: &str) -> bool {
    line.trim_start().starts_with('>')
}

/// Position of a row within a multi-line blockquote so its left bar renders
/// as one continuous vertical line across all lines of the quote.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct QuotePos {
    pub is_first: bool,
    pub is_last: bool,
}

/// Render one visual (possibly wrapped) row of a Markdown line at uniform
/// `row_height`.
///
/// Keeping uniform `row_height` ensures 1-to-1 vertical alignment in both
/// Split view (left raw source ↔ right preview) and Stacked live-preview view
/// (clicking any rendered line reveals its raw Markdown without jumping), while
/// wrapped continuation rows inherit the block's heading/quote/list styling.
#[allow(clippy::too_many_arguments)]
pub fn render_markdown_wrapped_row(
    row_text: &str,
    full_line: &str,
    is_first_wrap_row: bool,
    in_code_block: bool,
    quote_pos: QuotePos,
    row_height: f32,
    font_size: f32,
    font_family: &'static str,
    t: &Theme,
    on_toggle_task: Option<Box<dyn Fn(&mut Window, &mut App)>>,
) -> AnyElement {
    // Code fence delimiter line.
    if is_fence_line(full_line) {
        if !is_first_wrap_row {
            return div().w_full().h(px(row_height)).into_any_element();
        }
        let lang = full_line.trim().trim_start_matches(['`', '~']).trim();
        return div()
            .w_full()
            .h(px(row_height))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .child(div().flex_1().h(px(1.)).bg(rgba(t.border_color)))
            .when(!lang.is_empty(), |d| {
                d.child(
                    div()
                        .text_xs()
                        .font_family(crate::theme::FONT_MONO)
                        .text_color(rgba(t.text_secondary))
                        .child(lang.to_string()),
                )
            })
            .into_any_element();
    }

    // Line inside a fenced code block.
    if in_code_block {
        return div()
            .w_full()
            .h(px(row_height))
            .px(px(10.))
            .flex()
            .items_center()
            .whitespace_nowrap()
            .overflow_hidden()
            .bg(rgba(t.bg_sunken))
            .font_family(crate::theme::FONT_MONO)
            .text_size(px(font_size - 1.0))
            .text_color(rgba(t.text_primary))
            .child(row_text.to_string())
            .into_any_element();
    }

    // Horizontal rule.
    if is_horizontal_rule(full_line) {
        if !is_first_wrap_row {
            return div().w_full().h(px(row_height)).into_any_element();
        }
        return div()
            .w_full()
            .h(px(row_height))
            .flex()
            .items_center()
            .child(div().w_full().h(px(1.)).bg(rgba(t.border_subtle)))
            .into_any_element();
    }

    let trimmed = full_line.trim_start();
    let indent_spaces = full_line.len() - trimmed.len();

    // ATX Heading (`# ` through `###### `).
    if trimmed.starts_with('#') {
        let level = trimmed.bytes().take_while(|&b| b == b'#').count();
        if level <= 6 && (level == trimmed.len() || trimmed.as_bytes()[level] == b' ') {
            let content = if is_first_wrap_row {
                let t_row = row_text.trim_start();
                if t_row.starts_with('#') {
                    let l = t_row.bytes().take_while(|&b| b == b'#').count().min(6);
                    t_row[l..].trim_start()
                } else {
                    row_text
                }
            } else {
                row_text
            };
            let parsed = parse_inline(content, t);
            let size = match level {
                1 => 20.0,
                2 => 18.0,
                3 => 16.5,
                4 => 15.0,
                _ => font_size,
            };
            return div()
                .w_full()
                .h(px(row_height))
                .flex()
                .items_center()
                .whitespace_nowrap()
                .overflow_hidden()
                .font_family(font_family)
                .text_size(px(size))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgba(t.text_primary))
                .child(render_inline_element(parsed))
                .into_any_element();
        }
    }

    // Blockquote (`> ...`).
    if trimmed.starts_with('>') {
        let content = if is_first_wrap_row {
            let r = row_text.trim_start();
            r.strip_prefix('>')
                .map(|s| s.strip_prefix(' ').unwrap_or(s))
                .unwrap_or(row_text)
        } else {
            row_text
        };
        let parsed = parse_inline(content, t);
        let mut bar = div()
            .w(px(2.5))
            .h_full()
            .flex_shrink_0()
            .bg(rgba(t.accent));
        if quote_pos.is_first {
            bar = bar.rounded_t(px(1.5));
        }
        if quote_pos.is_last {
            bar = bar.rounded_b(px(1.5));
        }
        return div()
            .w_full()
            .h(px(row_height))
            .flex()
            .flex_row()
            .items_stretch()
            .gap(px(10.))
            .child(bar)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .font_family(font_family)
                    .text_size(px(font_size))
                    .text_color(rgba(t.text_secondary))
                    .child(render_inline_element(parsed)),
            )
            .into_any_element();
    }

    // Unordered list / task list (`- `, `* `, `+ `).
    if trimmed.len() >= 2
        && matches!(trimmed.as_bytes()[0], b'-' | b'*' | b'+')
        && trimmed.as_bytes()[1] == b' '
    {
        let indent_px = ((indent_spaces / 2) as f32 * 16.0).min(64.0);
        let task_info = parse_task_line(full_line);
        let (is_task, checked) = match task_info {
            Some((_, is_checked, _)) => (true, is_checked),
            None => (false, false),
        };
        let content = if is_first_wrap_row {
            let r = row_text.trim_start();
            let after = if r.len() >= 2 && r.as_bytes()[1] == b' ' {
                &r[2..]
            } else {
                r
            };
            if is_task {
                if after.len() >= 4 {
                    &after[4..]
                } else if after.len() >= 3 {
                    &after[3..]
                } else {
                    ""
                }
            } else {
                after
            }
        } else {
            row_text
        };
        let parsed = parse_inline(content, t);
        let marker = if !is_first_wrap_row {
            div()
                .w(px(if is_task { 15.0 } else { 12.0 }))
                .flex_shrink_0()
                .into_any_element()
        } else if is_task {
            let mut box_el = div()
                .w(px(15.))
                .h(px(15.))
                .rounded(px(4.))
                .border_1()
                .border_color(rgba(if checked {
                    t.accent
                } else {
                    t.border_strong
                }))
                .bg(rgba(if checked {
                    t.accent
                } else {
                    t.bg_sunken
                }))
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .cursor_pointer()
                .hover(|s| {
                    s.border_color(rgba(t.accent))
                });
            if checked {
                box_el = box_el.child(
                    svg()
                        .data(TODO_CHECK_SVG)
                        .w(px(10.))
                        .h(px(10.))
                        .text_color(rgba(t.accent_text)),
                );
            }
            if let Some(toggle) = on_toggle_task {
                box_el = box_el.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    toggle(window, cx);
                    cx.stop_propagation();
                });
            }
            box_el.into_any_element()
        } else {
            div()
                .w(px(12.))
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .text_size(px(font_size))
                .text_color(rgba(t.accent))
                .child("•")
                .into_any_element()
        };
        let mut text_div = div()
            .flex_1()
            .whitespace_nowrap()
            .overflow_hidden()
            .font_family(font_family)
            .text_size(px(font_size))
            .text_color(rgba(if checked {
                t.text_secondary
            } else {
                t.text_primary
            }));
        if checked {
            text_div = text_div.line_through();
        }
        return div()
            .w_full()
            .h(px(row_height))
            .pl(px(indent_px))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(if is_task { 8.0 } else { 6.0 }))
            .child(marker)
            .child(text_div.child(render_inline_element(parsed)))
            .into_any_element();
    }

    // Ordered list (`1. ` or `1) `).
    let digits = trimmed.bytes().take_while(|b| b.is_ascii_digit()).count();
    if digits > 0
        && digits + 1 < trimmed.len()
        && matches!(trimmed.as_bytes()[digits], b'.' | b')')
        && trimmed.as_bytes()[digits + 1] == b' '
    {
        let indent_px = ((indent_spaces / 2) as f32 * 16.0).min(64.0);
        let num_label = trimmed[..=digits].to_string();
        let content = if is_first_wrap_row {
            let r = row_text.trim_start();
            let d = r.bytes().take_while(|b| b.is_ascii_digit()).count();
            if d > 0 && d + 1 < r.len() && r.as_bytes()[d + 1] == b' ' {
                &r[d + 2..]
            } else {
                r
            }
        } else {
            row_text
        };
        let parsed = parse_inline(content, t);
        return div()
            .w_full()
            .h(px(row_height))
            .pl(px(indent_px))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.))
            .child(
                div()
                    .flex_shrink_0()
                    .font_family(font_family)
                    .text_size(px(font_size))
                    .text_color(rgba(if is_first_wrap_row { t.accent } else { 0x00000000 }))
                    .child(num_label),
            )
            .child(
                div()
                    .flex_1()
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .font_family(font_family)
                    .text_size(px(font_size))
                    .text_color(rgba(t.text_primary))
                    .child(render_inline_element(parsed)),
            )
            .into_any_element();
    }

    // Regular paragraph line with inline formatting.
    let parsed = parse_inline(row_text, t);
    div()
        .w_full()
        .h(px(row_height))
        .flex()
        .items_center()
        .whitespace_nowrap()
        .overflow_hidden()
        .font_family(font_family)
        .text_size(px(font_size))
        .font_weight(FontWeight::NORMAL)
        .text_color(rgba(t.text_primary))
        .child(render_inline_element(parsed))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_heading_and_inline_symbols_in_accent() {
        let accent = 0x5477b8ff;
        let fg = 0x30302fff;
        let spans = markdown_symbol_spans("## Hello **world** and `code`", accent, fg);
        // Leading `##` is accented.
        assert_eq!(spans[0], (accent, 0, 2));
        // `**` delimiters around `world` are accented.
        let accented: Vec<&str> = spans
            .iter()
            .filter(|&&(c, _, _)| c == accent)
            .map(|&(_, a, b)| &"## Hello **world** and `code`"[a..b])
            .collect();
        assert_eq!(accented, vec!["##", "**", "**", "`", "`"]);
    }

    #[test]
    fn inline_parser_strips_delimiters_and_records_styles() {
        let t = Theme::light();
        let parsed = parse_inline("Hello **bold** *ital* `code` [link](https://x.y)", &t);
        assert_eq!(parsed.text, "Hello bold ital code link");
        assert_eq!(parsed.highlights.len(), 4);
    }

    #[test]
    fn inline_parser_handles_bold_inline_code_combo() {
        let t = Theme::light();
        let parsed = parse_inline("**`POST /preview?device_id=X`**", &t);
        assert_eq!(parsed.text, "POST /preview?device_id=X");
        assert_eq!(parsed.highlights.len(), 1);
        let style = parsed.highlights[0].1;
        assert_eq!(style.font_weight, Some(FontWeight::SEMIBOLD));
        assert!(style.background_color.is_some());
        assert!(style.color.is_some());
    }

    #[test]
    fn detects_markdown_extensions() {
        assert!(is_markdown_path("README.md"));
        assert!(is_markdown_path("notes.MARKDOWN"));
        assert!(!is_markdown_path("main.rs"));
    }

    #[test]
    fn parses_markdown_table_rows_and_separators() {
        assert!(is_table_separator("|---|---|"));
        assert!(is_table_separator("| :--- | ---: | :---: |"));
        assert!(!is_table_separator("| Feature | Description |"));
        assert_eq!(
            parse_table_cells("| Feature | Description |"),
            Some(vec!["Feature", "Description"])
        );
        assert_eq!(
            parse_table_cells("| WiFi Sync (host + pull/push) | Full network repository sync |"),
            Some(vec![
                "WiFi Sync (host + pull/push)",
                "Full network repository sync"
            ])
        );
        let accent = 0x5477b8ff;
        let fg = 0x30302fff;
        let sep_spans = markdown_symbol_spans("|---|---|", accent, fg);
        assert_eq!(sep_spans, vec![(accent, 0, 9)]);
        let row_spans = markdown_symbol_spans("| A | B |", accent, fg);
        assert_eq!(
            row_spans.iter().filter(|&&(c, _, _)| c == accent).count(),
            3
        );
    }

    #[test]
    fn parses_task_list_lines() {
        assert_eq!(parse_task_line("- [ ] Buy milk"), Some((3, false, 6)));
        assert_eq!(parse_task_line("  * [x] Ship it"), Some((5, true, 8)));
        assert_eq!(parse_task_line("+ [X] Done"), Some((3, true, 6)));
        assert_eq!(parse_task_line("- [ ]"), Some((3, false, 5)));
        assert_eq!(parse_task_line("- plain bullet"), None);
        assert_eq!(parse_task_line("regular text"), None);
    }
}
