//! Root view: top bar, middle row (left dock + tab bar/preview +
//! right dock), bottom bar, menu overlay. Owns tabs, dock layout, menus,
//! hover tweens, and drag-resize state.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use chrono::Datelike;
use image::ImageDecoder;
use std::time::{Duration, Instant};

use gpui::{
    App, ClipboardItem, Context, FocusHandle, Font, FontFeatures, FontStyle, FontWeight,
    HighlightStyle, Hsla, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, Point, Render,
    ScrollDelta, ScrollWheelEvent, ShapedLine, StyledText, TextRun, WeakEntity, Window, deferred,
    div, prelude::*, px, rgba,
};

use super::chrome::{
    BOTTOM_BAR_HEIGHT, BottomBar, SidebarBottomBar, TOOLBAR_HEIGHT, Toolbar, TOP_BAR_HEIGHT, TopBar,
};
use super::dock::{DOCK_DEFAULT_WIDTH, DockView};
use super::dropdown::AppMenu;
use super::ink::{self, InkHistory, InkStroke};
use super::markdown;
use super::md_actions::{
    ListEnterAction, list_enter_action, quote_pos_for_row, smart_backspace_pair, smart_type_char,
    table_row_kind,
};
use super::media_io::{InkRenderCache, LoadedImage, bake_annotated_image, file_name_of};
use super::media_view::CropHandle;
use super::modals::{modal_button, settings_tab_button, settings_toggle};
use super::panels::{DockSide, PanelKind};
use super::svg_view;
use super::syntax::{highlight_line_spans, is_code_path};
use crate::editor::EditorState;
use crate::persist::{Store, TabRecord};
use crate::theme::{ThemeMode, WEIGHT_MEDIUM};

#[derive(Clone)]
pub struct Tab {
    pub id: usize,
    pub title: String,
    /// Backing file path, if any. Write-through keeps it in sync.
    pub path: Option<String>,
}

#[derive(Clone)]
pub(crate) struct FileInfo {
    pub name: String,
    pub size: String,
    pub dimensions: Option<String>,
    pub dpi: Option<String>,
    pub modified: String,
}

impl FileInfo {
    fn empty() -> Self {
        Self {
            name: "No file selected".into(),
            size: "—".into(),
            dimensions: None,
            dpi: None,
            modified: "—".into(),
        }
    }
}

/// The colours offered for the pen and the highlighter.
pub const INK_PALETTE: [u32; 16] = [
    0xFFD200, 0xFFA000, 0xF4511E, 0xE53935, 0x8E24AA, 0x7B1FA2, 0xD81B60, 0xEC407A,
    0x29B6F6, 0x1E88E5, 0x1565C0, 0x7CB342, 0x43A047, 0xFFFFFF, 0xE0E0E0, 0x212121,
];

/// Smallest and largest stroke size, in screen pixels at the fit zoom.
pub const INK_SIZE_MIN: f32 = 1.0;
pub const INK_SIZE_MAX: f32 = 40.0;

pub(crate) const ACCEPTED_FILE_EXTENSIONS: &[&str] = &[
    "txt", "md", "markdown", "rs", "toml", "json", "yaml", "yml", "xml", "html", "css",
    "js", "ts", "jsx", "tsx", "py", "go", "java", "c", "h", "cpp", "hpp", "sql", "ini",
    "conf", "csv", "tsv", "png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff", "ico",
    "svg", "pdf", "mp3", "wav", "flac", "ogg", "m4a", "aac", "mp4", "mov",
    "avi", "mkv", "webm", "m4v", "obj", "fbx", "gltf", "glb", "stl", "ply", "3ds", "dae",
    "blend",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MarkdownViewMode {
    /// Neither toolbar toggle active: plain editable Markdown with accented symbols.
    Plain,
    /// Left: editable raw Markdown; Right: live rendered Markdown preview.
    Split,
    /// Live preview: every line rendered except the active cursor line.
    Stacked,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewerKind {
    Text,
    Image,
    Pdf,
    Audio,
    Video,
    ThreeD,
    Svg,
    Csv,
    Unknown,
}

fn viewer_kind(path: Option<&str>) -> ViewerKind {
    let Some(path) = path else {
        return ViewerKind::Text;
    };
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tif" | "tiff" | "ico" => {
            ViewerKind::Image
        }
        "pdf" => ViewerKind::Pdf,
        "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" => ViewerKind::Audio,
        "mp4" | "mov" | "avi" | "mkv" | "webm" | "m4v" => ViewerKind::Video,
        "obj" | "fbx" | "gltf" | "glb" | "stl" | "ply" | "3ds" | "dae" | "blend" => {
            ViewerKind::ThreeD
        }
        "svg" => ViewerKind::Svg,
        "csv" | "tsv" => ViewerKind::Csv,
        "txt" | "md" | "markdown" | "rs" | "toml" | "json" | "yaml" | "yml" | "xml"
        | "html" | "css" | "js" | "ts" | "jsx" | "tsx" | "py" | "go" | "java"
        | "c" | "h" | "cpp" | "hpp" | "sql" | "ini" | "conf" => ViewerKind::Text,
        _ => ViewerKind::Unknown,
    }
}

fn format_file_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[derive(Clone)]
pub struct ProjectEntry {
    pub path: std::path::PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub depth: usize,
}

const PREVIEW_FONT_SIZE: f32 = 15.0;
const PREVIEW_LINE_HEIGHT: f32 = 24.0;
const PREVIEW_TOP_PAD: f32 = 24.0;
const CARET_HEIGHT: f32 = 17.0;
/// Widest the text column ever gets; it stays centered in the viewport.
const MAX_TEXT_WIDTH: f32 = 720.0;
/// Smallest margin kept between the text column and the viewport edge.
const MIN_COLUMN_MARGIN: f32 = 24.0;

/// Resolved preview layout in center-column coordinates. The text column is
/// centered: it grows to `MAX_TEXT_WIDTH`, then the leftover space becomes
/// equal margins.
struct PreviewGeometry {
    /// Center-column width (window minus open docks).
    col_w: f32,
    /// Left occupancy: sidebar width (so text column starts after it).
    left_occ: f32,
    /// Left margin (static).
    margin: f32,
    /// Wrap width (f32::MAX disables wrapping).
    measure: f32,
    /// Rendered text block width.
    text_width: f32,
    /// Whether text wraps.
    wrap: bool,
}

const LINE_NUMBER_GUTTER_WIDTH: f32 = 76.0;

fn preview_geometry(
    view_w: f32,
    left_occ: f32,
    right_occ: f32,
    wrap: bool,
    gutter: f32,
) -> PreviewGeometry {
    let col = (view_w - left_occ - right_occ).max(0.0);
    let left_margin = if gutter > 0.0 { gutter } else { MIN_COLUMN_MARGIN };
    let right_margin = MIN_COLUMN_MARGIN;
    let (measure, text_width) = if wrap {
        let m = (col - left_margin - right_margin).max(0.0);
        (m, m)
    } else {
        (f32::MAX, (col - left_margin).max(0.0))
    };
    PreviewGeometry { col_w: col, left_occ, margin: left_margin, measure, text_width, wrap }
}

fn preview_text_left(geo: &PreviewGeometry) -> f32 {
    geo.left_occ + geo.margin
}

/// Cached visual wrap map for a single text buffer.
///
/// Only built when wrapping is enabled (`measure != f32::MAX`). When wrapping
/// is off, visual row `i` is always buffer line `i` so no wrap map is needed.
/// Moving the cursor or dragging a selection never invalidates this cache.
pub(super) struct EditorWrapCache {
    /// Visual rows: `(buffer_line, start_byte, end_byte)`.
    rows: Vec<(usize, usize, usize)>,
}

/// Render-ready visible row computed only for rows currently on screen.
/// Rendered as a single `StyledText` run so glyph positions match `shape_row`
/// to the exact subpixel with zero flexbox rounding drift.
struct VisibleRow {
    text: String,
    highlights: Vec<(std::ops::Range<usize>, HighlightStyle)>,
    buffer_line: usize,
    is_first_wrap_row: bool,
    is_last_wrap_row: bool,
    in_code_block: bool,
    /// Caret (x, height) within the row, if the cursor is here.
    caret: Option<(f32, f32)>,
    /// Selection x-range (px) within the row, if any overlaps it.
    sel_px: Option<(f32, f32)>,
}

/// Shape-measured width of a preview string via the window text system.
fn measure_text(
    window: &mut Window,
    text: &str,
    color: Hsla,
    size: f32,
    weight: FontWeight,
    mono: bool,
) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    shape_row(window, text, color, size, weight, mono).width().as_f32()
}

/// Shape one preview run. Callers split edges from the returned layout
/// so positions match rendered text exactly.
fn shape_row(
    window: &mut Window,
    text: &str,
    color: Hsla,
    size: f32,
    weight: FontWeight,
    mono: bool,
) -> ShapedLine {
    let font = Font {
        family: (if mono { crate::theme::FONT_MONO } else { crate::theme::FONT_TEXT }).into(),
        features: FontFeatures::default(),
        fallbacks: None,
        weight,
        style: FontStyle::Normal,
    };
    let run = TextRun {
        len: text.len(),
        font,
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window
        .text_system()
        .shape_line(text.to_string().into(), px(size), &[run], None)
}

/// Convert syntax spans on `line_text` into `StyledText` highlight ranges
/// relative to the wrapped row slice `rs..re`.
fn row_highlights(
    rs: usize,
    re: usize,
    default_color: u32,
    spans: &[(u32, usize, usize)],
) -> Vec<(std::ops::Range<usize>, HighlightStyle)> {
    let mut out = Vec::new();
    for &(c, s, e) in spans {
        if c == default_color {
            continue;
        }
        let a = s.max(rs);
        let b = e.min(re);
        if a < b {
            out.push((
                (a - rs)..(b - rs),
                HighlightStyle {
                    color: Some(Hsla::from(rgba(c))),
                    ..Default::default()
                },
            ));
        }
    }
    out
}

pub struct RootView {
    pub(super) tabs: Vec<Tab>,
    pub(super) active_tab: usize,
    next_tab_id: usize,
    visible_left: Option<PanelKind>,
    visible_right: Option<PanelKind>,
    left_width: f32,
    left_sidebar_progress: f32,
    left_sidebar_target: f32,
    left_sidebar_animating: bool,
    right_width: f32,
    /// Active dock drag: (side being resized, last cursor x, window width).
    /// Widths are unclamped; the only rule is the docks never overlap.
    drag: Option<(DockSide, f32, f32)>,
    pub(super) recent_files: Vec<String>,
    project_root: Option<std::path::PathBuf>,
    project_expanded: HashSet<std::path::PathBuf>,
    hovered_tab: Option<usize>,
    pub(crate) tab_scroll_offset: f32,
    tab_scroll_drag: Option<(f32, f32)>,
    pub(crate) hovered_close_tab: Option<usize>,
    pub(super) store: Store,
    pub(super) image_assets: HashMap<String, (Arc<gpui::Image>, u32, u32)>,
    pub(super) image_loading: HashSet<String>,
    pub(super) image_load_errors: HashSet<String>,
    pub(super) pdf_assets: HashMap<String, Vec<LoadedImage>>,
    pub(super) pdf_loading: HashSet<String>,
    pub(super) pdf_load_errors: HashSet<String>,
    pub(super) image_zoom: f32,
    pub(super) image_zoom_target: f32,
    pub(super) image_pan: (f32, f32),
    pub(super) image_pan_target: (f32, f32),
    image_zoom_tween_active: bool,
    pub(super) image_drag: Option<(MouseButton, f32, f32, f32, f32)>,
    /// Right-click start position on image preview to distinguish click vs pan drag.
    pub(super) image_right_down_pos: Option<(f32, f32)>,
    /// Active image right-click context menu: `(image_path, window_x, window_y)`.
    pub(super) image_context_menu: Option<(String, f32, f32)>,
    /// Pop-in animation progress (`0.0..=1.0`) for `image_context_menu`.
    pub(super) image_context_menu_progress: f32,
    /// Active crop rectangle in source-image pixel space `(x0, y0, x1, y1)`.
    pub(super) image_crop_rect: Option<(f32, f32, f32, f32)>,
    /// Active crop handle drag: `(handle, start_win_x, start_win_y, start_rect, max_bounds, scale_x, scale_y)`.
    pub(super) image_crop_drag: Option<(
        CropHandle,
        f32,
        f32,
        (f32, f32, f32, f32),
        (f32, f32, f32, f32),
        f32,
        f32,
    )>,
    /// Annotation strokes per image path, in source-image pixel space.
    pub(super) image_strokes: HashMap<String, Rc<Vec<InkStroke>>>,
    /// Undo/redo history for the annotations of each image.
    pub(super) image_history: HashMap<String, InkHistory>,
    /// Strokes removed by the eraser during the current drag, so the whole
    /// drag counts as one undo step.
    image_erased: Vec<InkStroke>,
    /// The stroke currently being drawn, if any.
    pub(super) image_current_stroke: Option<InkStroke>,
    /// True while the mouse button is held down on the canvas.
    image_drawing: bool,
    /// Scale of the image at the fit-to-window zoom level, used to keep the
    /// sampling density of new strokes constant.
    pub(super) image_fit: f32,
    /// Path of the image that is currently being annotated.
    image_ink_path: String,
    /// Last time an in-progress annotation caused a view repaint.
    pub(super) image_last_ink_notify: Instant,
    pub(super) image_ink_revision: u64,
    pub(super) image_ink_cache: Rc<RefCell<Option<InkRenderCache>>>,
    /// A transient message shown in the status bar.
    status_message: Option<(String, Instant)>,
    /// Colour of the pen strokes, as 0xRRGGBB.
    pub ink_pen_color: u32,
    /// Width of the pen strokes, in screen pixels at the fit zoom.
    pub ink_pen_size: f32,
    /// Colour of the highlighter strokes, as 0xRRGGBB.
    pub ink_highlighter_color: u32,
    /// Width of the highlighter strokes, in screen pixels at the fit zoom.
    pub ink_highlighter_size: f32,
    /// The ink tool whose settings popup is logically open, if any.
    pub ink_popup: Option<u8>,
    /// The ink tool whose popup is currently rendered (kept alive while closing).
    pub ink_popup_display: Option<u8>,
    /// Open/close transition progress in `0.0..=1.0`.
    pub ink_popup_progress: f32,
    ink_popup_animating: bool,
    /// True while the size slider is being dragged.
    pub ink_slider_dragging: bool,
    /// The thickness the handle is currently showing, in pixels.
    pub ink_handle: f32,
    /// The thickness the handle is heading for.
    pub ink_handle_target: f32,
    /// True while the handle is sliding.
    pub ink_handle_active: bool,
    /// Currently selected image-toolbar tool (0 = cursor, ...).
    pub image_toolbar_tool: usize,
    /// Whether the shapes dropdown is open.
    pub image_shapes_dropdown_open: bool,
    /// Currently selected shape in the shapes dropdown (0 = rectangle).
    pub image_shapes_selected: usize,
    /// Buffers keyed by tab id, created lazily.
    pub(super) editors: HashMap<usize, EditorState>,
    /// Tab ids with unsaved edits (dirty).
    pub(super) dirty_tabs: HashSet<usize>,
    /// `(len_bytes, hash)` of each text/code/markdown/svg tab as last loaded
    /// from or saved to disk, so undoing or reverting edits clears the dot.
    saved_text_hash: HashMap<usize, (usize, u64)>,
    /// Bumped only when text buffer content changes (never on cursor/selection
    /// moves) so the wrap cache stays valid across clicks, drags, and arrows.
    pub(super) editor_version: u64,
    pub(super) cached_wrap: Option<Rc<EditorWrapCache>>,
    cached_wrap_key: (usize, u64, u32),
    /// Preview focus follows the active tab.
    pub focus: FocusHandle,
    /// Instant of the last cursor move or edit; resets the 530ms blink phase
    /// so clicks and cursor moves never flash off prematurely.
    caret_blink_epoch: Instant,
    last_caret_visible: bool,
    blink_started: bool,
    /// Vertical scroll offset (px) of the preview viewport.
    scroll_offset: f32,
    /// Scrollbar drag: (pointer start y, scroll offset at start).
    scroll_drag: Option<(f32, f32)>,
    /// Number of rendered preview rows from the most recent frame.
    scroll_content_rows: usize,
    scrollbar_hovered: bool,
    /// Horizontal scroll offset (px) for unwrapped text/code files.
    pub(super) scroll_x: f32,
    /// Horizontal scrollbar drag: (pointer start x, scroll_x at start, track_w, thumb_w, max_x).
    scroll_x_drag: Option<(f32, f32, f32, f32, f32)>,
    /// Cached max pixel width across all lines when unwrapped.
    scroll_x_content_w: f32,
    scrollbar_x_hovered: bool,
    cached_max_line_w: f32,
    cached_max_line_key: (usize, u64),
    /// True while the left button is held for mouse selection.
    mouse_select: bool,
    // -- hamburger menu state (Parametric behavior) --
    pub menu_open: bool,
    pub settings_open: bool,
    pub close_confirm: Option<usize>,
    pub settings_tab: usize,
    pub wrap_text_files: bool,
    pub wrap_code_files: bool,
    pub line_numbers: bool,
    pub syntax_highlighting: bool,
    pub windows_open_with: bool,
    /// Active view mode for Markdown files (Plain, Split, or Stacked).
    pub markdown_mode: MarkdownViewMode,
    /// Whether SVG Split View (code + live SVG preview) is enabled.
    pub svg_split_view: bool,
    /// Cached prepared SVG preview keyed by `(tab_id, editor_version, text_primary)`.
    pub(super) svg_preview_cache: Option<((usize, u64, u32), Arc<gpui::Image>, Option<(f32, f32)>)>,
    /// Horizontal split ratio (0.2..=0.8) for Split view; snaps to 0.5.
    md_split_ratio: f32,
    /// True while dragging the Markdown Split view divider.
    md_split_dragging: bool,
    /// True while the Markdown heading dropdown menu in the toolbar is open.
    pub md_heading_dropdown_open: bool,
    /// Cached printable-ASCII glyph widths for O(N) zero-shaping line wrapping
    /// while resizing the sidebar or split divider on large wrapped files.
    ascii_char_widths: Option<[f32; 128]>,
    ascii_max_width: f32,
    pub active_menu: Option<usize>,
    pub menu_animation: f32,
    pub icon_animation: f32,
    pub hovered_entry: Option<usize>,
    fades: HashMap<String, f32>,
    fade_pending: HashMap<String, f32>,
    fade_tween_active: HashSet<String>,
    cursor_trail: Vec<(Point<gpui::Pixels>, Instant)>,
}

/// What `save_step` decided, so the app can run dialogs borrow-free.
#[derive(Clone)]
pub enum SaveStep {
    Nothing,
    WriteActive,
    AskDialog(String),
}

impl RootView {
    pub fn new(cx: &mut Context<Self>, store: Store) -> Self {
        // Restore tabs: a tab with a path reopens its file (content
        // preloaded), a tab without one is an empty placeholder.
        let mut tabs: Vec<Tab> = Vec::new();
        let mut next_tab_id = 0;
        let mut preloaded: HashMap<usize, String> = HashMap::new();
        let mut saved_hashes: HashMap<usize, (usize, u64)> = HashMap::new();
        let mut dirty_restored: HashSet<usize> = HashSet::new();
        for record in store.tabs() {
            if let Some(path) = record.path.as_ref() {
                let title = std::path::Path::new(path)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.to_string());
                let dirty_key = format!("dirty:{}", path);
                let buffer_key = format!("buffer:{}", path);
                let disk_content = match viewer_kind(Some(path)) {
                    ViewerKind::Text | ViewerKind::Csv | ViewerKind::Svg => {
                        std::fs::read_to_string(path).unwrap_or_default()
                    }
                    _ => String::new(),
                };
                let disk_fp = crate::editor::fingerprint_str(&disk_content);
                saved_hashes.insert(next_tab_id, disk_fp);
                let content = if store.get(&dirty_key).as_deref() == Some("1") {
                    let restored = store.get(&buffer_key).unwrap_or_default();
                    if crate::editor::fingerprint_str(&restored) != disk_fp {
                        dirty_restored.insert(next_tab_id);
                    }
                    restored
                } else {
                    disk_content
                };
                preloaded.insert(next_tab_id, content);
                tabs.push(Tab { id: next_tab_id, title, path: Some(path.clone()) });
            } else {
                tabs.push(Tab { id: next_tab_id, title: record.title, path: None });
            }
            next_tab_id += 1;
        }
        if tabs.is_empty() {
            tabs.push(Tab { id: 0, title: "Welcome".into(), path: None });
            next_tab_id = 1;
        }
        let active_tab = store
            .get("tabs.active")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(tabs.len() - 1)
            .min(tabs.len() - 1);

        let width = |key: &str, fallback: f32| {
            store.get(key).and_then(|v| v.parse::<f32>().ok()).unwrap_or(fallback)
        };
        let visible = |key: &str| store.get(key).and_then(|v| PanelKind::from_key(&v));
        let visible_left = visible("panels.left.visible");
        let visible_right = visible("panels.right.visible");

        let recent_files: Vec<String> = store
            .get("recents")
            .map(|v| v.split('\n').filter(|s| !s.is_empty()).map(str::to_string).collect())
            .unwrap_or_default();
        let wrap_text_files = store.get("editor.wrap_text_files").map(|v| v == "true").unwrap_or(true);
        let wrap_code_files = store.get("editor.wrap_code_files").map(|v| v == "true").unwrap_or(false);
        let line_numbers = store.get("editor.line_numbers").map(|v| v == "true").unwrap_or(true);
        let syntax_highlighting = store.get("editor.syntax_highlighting").map(|v| v == "true").unwrap_or(true);
        let windows_open_with = store.get("windows.open_with").as_deref() != Some("disabled");
        let project_root = None;
        let project_expanded = HashSet::new();

        let mut view = Self {
            tabs,
            active_tab,
            next_tab_id,
            visible_left,
            visible_right,
            left_width: width("panels.left.width", DOCK_DEFAULT_WIDTH),
            left_sidebar_progress: if visible_left.is_some() { 1.0 } else { 0.0 },
            left_sidebar_target: if visible_left.is_some() { 1.0 } else { 0.0 },
            left_sidebar_animating: false,
            right_width: width("panels.right.width", DOCK_DEFAULT_WIDTH),
            drag: None,
            recent_files,
            project_root,
            project_expanded,
            hovered_tab: None,
            tab_scroll_offset: 0.0,
            tab_scroll_drag: None,
            hovered_close_tab: None,
            store,
            image_assets: HashMap::new(),
            image_loading: HashSet::new(),
            image_load_errors: HashSet::new(),
            pdf_assets: HashMap::new(),
            pdf_loading: HashSet::new(),
            pdf_load_errors: HashSet::new(),
            image_zoom: 1.0,
            image_zoom_target: 1.0,
            image_pan: (0.0, 0.0),
            image_pan_target: (0.0, 0.0),
            image_zoom_tween_active: false,
            image_drag: None,
            image_right_down_pos: None,
            image_context_menu: None,
            image_context_menu_progress: 0.0,
            image_crop_rect: None,
            image_crop_drag: None,
            image_strokes: HashMap::new(),
            image_history: HashMap::new(),
            image_erased: Vec::new(),
            image_current_stroke: None,
            image_drawing: false,
            image_fit: 1.0,
            image_ink_path: String::new(),
            image_last_ink_notify: Instant::now(),
            image_ink_revision: 0,
            image_ink_cache: Rc::new(RefCell::new(None)),
            status_message: None,
            ink_pen_color: INK_PALETTE[15],
            ink_pen_size: 4.0,
            ink_highlighter_color: INK_PALETTE[0],
            ink_highlighter_size: 20.0,
            ink_popup: None,
            ink_popup_display: None,
            ink_popup_progress: 0.0,
            ink_popup_animating: false,
            ink_slider_dragging: false,
            ink_handle: 4.0,
            ink_handle_target: 4.0,
            ink_handle_active: false,
            image_toolbar_tool: 0,
            image_shapes_dropdown_open: false,
            image_shapes_selected: 0,
            editors: HashMap::new(),
            dirty_tabs: HashSet::new(),
            saved_text_hash: saved_hashes,
            editor_version: 0,
            cached_wrap: None,
            cached_wrap_key: (usize::MAX, 0, 0),
            focus: cx.focus_handle(),
            caret_blink_epoch: Instant::now(),
            last_caret_visible: true,
            blink_started: false,
            scroll_offset: 0.0,
            scroll_drag: None,
            scroll_content_rows: 0,
            scrollbar_hovered: false,
            scroll_x: 0.0,
            scroll_x_drag: None,
            scroll_x_content_w: 0.0,
            scrollbar_x_hovered: false,
            cached_max_line_w: 0.0,
            cached_max_line_key: (usize::MAX, 0),
            mouse_select: false,
            menu_open: false,
            settings_open: false,
            close_confirm: None,
            settings_tab: 0,
            wrap_text_files,
            wrap_code_files,
            line_numbers,
            syntax_highlighting,
            windows_open_with,
            markdown_mode: MarkdownViewMode::Plain,
            svg_split_view: true,
            svg_preview_cache: None,
            md_split_ratio: 0.5,
            md_split_dragging: false,
            md_heading_dropdown_open: false,
            ascii_char_widths: None,
            ascii_max_width: 0.0,
            active_menu: None,
            menu_animation: 0.0,
            icon_animation: 0.0,
            hovered_entry: None,
            fades: HashMap::new(),
            fade_pending: HashMap::new(),
            fade_tween_active: HashSet::new(),
            cursor_trail: Vec::new(),
        };
        view.dirty_tabs = dirty_restored;
        // Restore the ink colour and size settings.
        if let Some(v) = view.store.get("ink.pen.color").and_then(|v| v.parse::<u32>().ok()) {
            view.ink_pen_color = v;
        }
        if let Some(v) = view.store.get("ink.pen.size").and_then(|v| v.parse::<f32>().ok()) {
            view.ink_pen_size = v;
        }
        if let Some(v) = view
            .store
            .get("ink.highlighter.color")
            .and_then(|v| v.parse::<u32>().ok())
        {
            view.ink_highlighter_color = v;
        }
        if let Some(v) = view
            .store
            .get("ink.highlighter.size")
            .and_then(|v| v.parse::<f32>().ok())
        {
            view.ink_highlighter_size = v;
        }
        // Preload reopened files into their buffers.
        for (id, content) in preloaded {
            view.editors
                .entry(id)
                .or_insert_with(EditorState::new)
                .set_text(&content);
        }
        view
    }

    /// Which dock a panel opens in.
    fn dock_of(kind: PanelKind) -> DockSide {
        match kind {
            PanelKind::Files | PanelKind::Info | PanelKind::PdfPages => DockSide::Left,
        }
    }

    pub(crate) fn active_file_info(&self) -> FileInfo {
        let Some(path) = self
            .tabs
            .get(self.active_tab)
            .and_then(|tab| tab.path.as_deref())
        else {
            return FileInfo::empty();
        };
        let path_obj = std::path::Path::new(path);
        let name = path_obj
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string());
        let Ok(metadata) = std::fs::metadata(path_obj) else {
            return FileInfo { name, ..FileInfo::empty() };
        };
        let size = format_file_size(metadata.len());
        let modified = metadata
            .modified()
            .ok()
            .map(|time| {
                let date = chrono::DateTime::<chrono::Local>::from(time);
                let month = date.format("%B");
                let clock = date.format("%I:%M %p").to_string();
                format!(
                    "{} {}, {} Â· {}",
                    month,
                    date.day(),
                    date.year(),
                    clock.trim_start_matches('0')
                )
            })
            .unwrap_or_else(|| "—".into());
        let (dimensions, dpi) = if viewer_kind(Some(path)) == ViewerKind::Image {
            image::ImageReader::open(path)
                .ok()
                .and_then(|reader| reader.with_guessed_format().ok())
                .and_then(|reader| reader.into_decoder().ok())
                .map(|decoder| {
                    let (width, height) = decoder.dimensions();
                    (Some(format!("{width} × {height} px")), Some("—".into()))
                })
                .unwrap_or((None, None))
        } else {
            (None, None)
        };
        FileInfo { name, size, dimensions, dpi, modified }
    }

    fn save_layout(&self) {
        let opt = |v: Option<PanelKind>| v.map(|k| k.key()).unwrap_or("");
        self.store.set("panels.left.visible", opt(self.visible_left));
        self.store.set("panels.right.visible", opt(self.visible_right));
        self.store.set("panels.left.width", &self.left_width.to_string());
        self.store.set("panels.right.width", &self.right_width.to_string());
        self.store.set("tabs.active", &self.active_tab.to_string());
        self.store.set("recents", &self.recent_files.join("\n"));
        let records: Vec<TabRecord> = self
            .tabs
            .iter()
            .map(|t| TabRecord { title: t.title.clone(), path: t.path.clone() })
            .collect();
        self.store.set_tabs(&records);
    }

    // ----- tabs -----

    pub fn activate_tab(&mut self, id: usize, cx: &mut Context<Self>) {
        if let Some(i) = self.tabs.iter().position(|t| t.id == id) {
            self.active_tab = i;
            self.image_zoom = 1.0;
            self.image_zoom_target = 1.0;
            self.image_pan = (0.0, 0.0);
            self.image_pan_target = (0.0, 0.0);
            self.image_drag = None;
            self.image_context_menu = None;
            self.image_crop_rect = None;
            self.image_crop_drag = None;
            if self.image_toolbar_tool == 5 {
                self.image_toolbar_tool = 0;
            }
            self.save_layout();
            cx.notify();
        }
    }

    pub fn close_tab(&mut self, id: usize, cx: &mut Context<Self>) {
        if self.dirty_tabs.contains(&id) {
            self.close_confirm = Some(id);
            cx.notify();
            return;
        }
        self.close_tab_force(id, cx);
    }

    fn close_tab_force(&mut self, id: usize, cx: &mut Context<Self>) {
        // Persist unsaved buffer before discarding, so reopen can restore.
        let closed_path = self
            .tabs
            .iter()
            .find(|t| t.id == id)
            .and_then(|t| t.path.clone());
        if let Some(path) = closed_path.as_deref() {
            if viewer_kind(Some(path)) == ViewerKind::Image {
                // Image annotations cannot be restored later, so closing the
                // tab throws them away instead of persisting a stale buffer.
                self.image_strokes.remove(path);
                self.image_history.remove(path);
                self.store.set(&format!("dirty:{}", path), "0");
            } else if self.dirty_tabs.contains(&id) {
                if let Some(ed) = self.editors.get(&id) {
                    self.store.set(&format!("buffer:{}", path), &ed.text_string());
                    self.store.set(&format!("dirty:{}", path), "1");
                }
            } else {
                self.store.set(&format!("dirty:{}", path), "0");
            }
        }
        self.dirty_tabs.remove(&id);
        self.saved_text_hash.remove(&id);
        self.editors.remove(&id);
        if let Some(i) = self.tabs.iter().position(|t| t.id == id) {
            self.tabs.remove(i);
            if self.tabs.is_empty() {
                self.tabs.push(Tab {
                    id: self.next_tab_id,
                    title: "Welcome".into(),
                    path: None,
                });
                self.next_tab_id += 1;
            }
            self.active_tab = self.active_tab.min(self.tabs.len() - 1);
            self.save_layout();
            cx.notify();
        }
    }

    pub fn close_active_tab(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get(self.active_tab) {
            let id = tab.id;
            self.close_tab(id, cx);
        }
    }
    pub fn open_welcome(&mut self, cx: &mut Context<Self>) {
        if let Some(i) = self.tabs.iter().position(|t| t.title == "Welcome") {
            self.active_tab = i;
        } else {
            let id = self.next_tab_id;
            self.next_tab_id += 1;
            self.tabs.push(Tab { id, title: "Welcome".into(), path: None });
            self.active_tab = self.tabs.len() - 1;
        }
        self.save_layout();
        cx.notify();
    }

    // ----- preview geometry -----

    pub fn left_occupancy(&self) -> f32 {
        if self.visible_left.is_some() {
            self.left_width * self.left_sidebar_progress
        } else {
            0.0
        }
    }

    pub fn right_occupancy(&self) -> f32 {
        if self.visible_right.is_some() {
            self.right_width
        } else {
            0.0
        }
    }

    pub(super) fn dock_occupancy(&self) -> (f32, f32) {
        let left = if self.visible_left.is_some() {
            self.left_width * self.left_sidebar_progress
        } else {
            0.0
        };
        let right = if self.visible_right.is_some() {
            self.right_width
        } else {
            0.0
        };
        (left, right)
    }

    fn geometry(&self, window: &mut Window) -> PreviewGeometry {
        let view_w = window.viewport_size().width.as_f32();
        let (left_occ, right_occ) = self.dock_occupancy();
        let active_path = self.tabs.get(self.active_tab).and_then(|t| t.path.clone());
        let is_code = active_path.as_deref().map(is_code_path).unwrap_or(false);
        let is_markdown = active_path
            .as_deref()
            .map(markdown::is_markdown_path)
            .unwrap_or(false);
        let is_svg = active_path
            .as_deref()
            .map(svg_view::is_svg_path)
            .unwrap_or(false);
        let wrap = if is_code { self.wrap_code_files } else { self.wrap_text_files };
        let gutter = if is_code && self.line_numbers {
            LINE_NUMBER_GUTTER_WIDTH
        } else {
            0.0
        };
        let is_split = (is_markdown && self.markdown_mode == MarkdownViewMode::Split)
            || (is_svg && self.svg_split_view);
        let effective_right_occ = if is_split {
            let full_col = (view_w - left_occ - right_occ).max(0.0);
            right_occ + full_col * (1.0 - self.md_split_ratio)
        } else {
            right_occ
        };
        preview_geometry(view_w, left_occ, effective_right_occ, wrap, gutter)
    }

    // ----- undo / redo (active tab's buffer) -----

    pub(super) fn active_editor_mut(&mut self) -> Option<&mut EditorState> {
        let tab = self.tabs.get(self.active_tab)?;
        if tab.title == "Welcome" {
            return None;
        }
        Some(self.editors.entry(tab.id).or_insert_with(EditorState::new))
    }

    /// Reset the caret blink phase so it stays visible for a full 530ms half-
    /// period after any click, cursor move, or edit instead of flashing off.
    fn reset_caret_blink(&mut self) {
        self.caret_blink_epoch = Instant::now();
        self.last_caret_visible = true;
    }

    /// Crisp on/off caret visibility derived from `caret_blink_epoch`.
    fn caret_visible(&self) -> bool {
        const HALF_PERIOD_MS: u128 = 530;
        (self.caret_blink_epoch.elapsed().as_millis() / HALF_PERIOD_MS) % 2 == 0
    }

    fn handle_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let mods = &ev.keystroke.modifiers;
        // Clipboard + select-all chords bypass the buffer entirely.
        if (mods.control || mods.platform) && !mods.alt {
            match ev.keystroke.key.as_str() {
                "c" => {
                    if let Some(ed) = self.active_editor_mut() {
                        if let Some(text) = ed.selected_text() {
                            cx.write_to_clipboard(ClipboardItem::new_string(text));
                        }
                    }
                    return;
                }
                "x" if !mods.shift => {
                    if let Some(ed) = self.active_editor_mut() {
                        if let Some(text) = ed.selected_text() {
                            cx.write_to_clipboard(ClipboardItem::new_string(text));
                            ed.delete_selection();
                            self.reset_caret_blink();
                            self.editor_version = self.editor_version.wrapping_add(1);
                            if let Some(tab) = self.tabs.get(self.active_tab) {
                                self.mark_tab_dirty(tab.id);
                            }
                            cx.notify();
                        }
                    }
                    return;
                }
                "v" => {
                    if let Some(ed) = self.active_editor_mut() {
                        if let Some(item) = cx.read_from_clipboard() {
                            if let Some(text) = item.text() {
                                ed.delete_selection();
                                ed.insert_str(&text);
                                self.reset_caret_blink();
                                self.editor_version = self.editor_version.wrapping_add(1);
                                if let Some(tab) = self.tabs.get(self.active_tab) {
                                    self.mark_tab_dirty(tab.id);
                                }
                                cx.notify();
                            }
                        }
                    }
                    return;
                }
                "a" => {
                    if let Some(ed) = self.active_editor_mut() {
                        ed.select_all();
                        self.reset_caret_blink();
                        cx.notify();
                    }
                    return;
                }
                // Markdown formatting & todo shortcuts.
                "b" if self.active_is_markdown() && !mods.shift => {
                    self.md_wrap_inline("**", "**", cx);
                    return;
                }
                "i" if self.active_is_markdown() && !mods.shift => {
                    self.md_wrap_inline("*", "*", cx);
                    return;
                }
                "k" if self.active_is_markdown() && !mods.shift => {
                    self.md_insert_link(cx);
                    return;
                }
                "e" if self.active_is_markdown() && !mods.shift => {
                    self.md_toggle_code(cx);
                    return;
                }
                "x" if self.active_is_markdown() && mods.shift => {
                    self.md_wrap_inline("~~", "~~", cx);
                    return;
                }
                "l" if self.active_is_markdown() && !mods.shift => {
                    self.md_toggle_task_list(cx);
                    return;
                }
                "enter" if self.active_is_markdown() && !mods.shift => {
                    if let Some(line_no) = self.active_editor_mut().map(|ed| ed.cursor_line_col().0)
                    {
                        self.md_toggle_task_at_line(line_no, cx);
                    }
                    return;
                }
                _ => return,
            }
        }
        let is_markdown = self.active_is_markdown();
        let Some(ed) = self.active_editor_mut() else {
            return;
        };
        let key = ev.keystroke.key.as_str();
        let clean = !mods.control && !mods.platform && !mods.alt;
        let extend = mods.shift;
        let mut mutated_text = false;
        match key {
            "left" if clean => ed.move_left(extend),
            "right" if clean => ed.move_right(extend),
            "home" if clean => ed.line_home(extend),
            "end" if clean => ed.line_end(extend),
            "up" if clean => self.move_visual(-1, extend, window, cx),
            "down" if clean => self.move_visual(1, extend, window, cx),
            "backspace" if clean => {
                if !smart_backspace_pair(ed) {
                    ed.backspace();
                }
                mutated_text = true;
            }
            "delete" if clean => {
                ed.delete_forward();
                mutated_text = true;
            }
            "enter" if clean => {
                match list_enter_action(ed) {
                    ListEnterAction::Insert(text) => ed.insert_str(&text),
                    ListEnterAction::ClearLine { start, end } => {
                        ed.replace_char_range(start, end, "", start, None);
                    }
                }
                mutated_text = true;
            }
            "tab" => return,
            _ => {
                if !clean {
                    return;
                }
                match &ev.keystroke.key_char {
                    Some(s) => {
                        let filtered: String = s.chars().filter(|c| *c != '\r').collect();
                        if filtered.is_empty() {
                            return;
                        }
                        let mut chars = filtered.chars();
                        if let (Some(ch), None) = (chars.next(), chars.next()) {
                            if let Some(did_mutate) = smart_type_char(ed, ch, is_markdown) {
                                mutated_text = did_mutate;
                            } else {
                                ed.insert_str(&filtered);
                                mutated_text = true;
                            }
                        } else {
                            ed.insert_str(&filtered);
                            mutated_text = true;
                        }
                    }
                    None => return,
                }
            }
        }
        self.reset_caret_blink();
        if mutated_text {
            self.editor_version = self.editor_version.wrapping_add(1);
            if let Some(tab) = self.tabs.get(self.active_tab) {
                self.mark_tab_dirty(tab.id);
            }
        }
        cx.notify();
    }

    /// Vertical movement by visual (wrapped) row, preserving pixel x.
    fn move_visual(&mut self, dir: i32, extend: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab_id) = self
            .tabs
            .get(self.active_tab)
            .filter(|tab| tab.title != "Welcome")
            .map(|tab| tab.id)
        else {
            return;
        };
        let color = Hsla::from(rgba(crate::theme::active(cx).text_primary));
        let geo = self.geometry(window);
        let ed = self.editors.entry(tab_id).or_insert_with(EditorState::new);
        let (cur_line, cur_byte) = ed.cursor_line_byte();
        // Rows for the cursor line and its neighbors, all sharing the
        // preview's uniform metrics.
        let mut rows: Vec<(crate::editor::VisualRow, f32, FontWeight)> = Vec::new();
        for line in cur_line.saturating_sub(1)..=(cur_line + 1).min(ed.len_lines().saturating_sub(1)) {
            let text = ed.line_text(line);
            let size = PREVIEW_FONT_SIZE;
            let weight = FontWeight::NORMAL;
            let mut w = |s: &str| measure_text(window, s, color, size, weight, false);
            rows.extend(
                crate::editor::wrap_line(line, &text, geo.measure, &mut w)
                    .into_iter()
                    .map(|r| (r, size, weight)),
            );
        }
        let mut current = None;
        for (i, (row, _, _)) in rows.iter().enumerate() {
            if row.buffer_line == cur_line && row.start_byte <= cur_byte && cur_byte <= row.end_byte
            {
                current = Some(i);
                break;
            }
        }
        let Some(current) = current else { return };
        let target = current as i32 + dir;
        if target < 0 || target >= rows.len() as i32 {
            return;
        }
        let (cur_row, size, weight) = rows[current].clone();
        let (target_row, target_size, target_weight) = rows[target as usize].clone();
        // Caret x on the current row, then the closest byte on the target row
        // using the exact same single-run shaping as rendered text.
        let rel = cur_byte.min(cur_row.end_byte).saturating_sub(cur_row.start_byte);
        let row_text = ed.line_text(cur_row.buffer_line);
        let row_slice = &row_text[cur_row.start_byte..cur_row.end_byte.min(row_text.len())];
        let x = if row_slice.is_empty() {
            0.0
        } else {
            shape_row(window, row_slice, color, size, weight, false)
                .x_for_index(rel.min(row_slice.len()))
                .as_f32()
        };
        let target_text = ed.line_text(target_row.buffer_line);
        let target_slice =
            &target_text[target_row.start_byte..target_row.end_byte.min(target_text.len())];
        let lo = if target_slice.is_empty() || x <= 0.0 {
            0
        } else {
            shape_row(window, target_slice, color, target_size, target_weight, false)
                .closest_index_for_x(px(x))
        };
        ed.set_cursor_line_byte(target_row.buffer_line, target_row.start_byte + lo, extend);
    }

    /// Wheel scrolling over the preview viewport (vertical + horizontal).
    fn handle_wheel(&mut self, ev: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
        let (raw_dx, raw_dy) = match ev.delta {
            ScrollDelta::Pixels(p) => (p.x.as_f32(), p.y.as_f32()),
            ScrollDelta::Lines(p) => (p.x * 24.0, p.y * PREVIEW_LINE_HEIGHT),
        };
        let (dx, dy) = if ev.modifiers.shift && raw_dx == 0.0 {
            (raw_dy, 0.0)
        } else {
            (raw_dx, raw_dy)
        };
        let mut changed = false;
        if dy != 0.0 {
            let max = self.scroll_max_for_geometry(Self::viewport_h(window), 0.0);
            let next = (self.scroll_offset - dy).clamp(0.0, max);
            if (next - self.scroll_offset).abs() > 1e-3 {
                self.scroll_offset = next;
                changed = true;
            }
        }
        if dx != 0.0 {
            let geo = self.geometry(window);
            if geo.measure == f32::MAX {
                let max_x = (self.scroll_x_content_w - geo.text_width).max(0.0);
                let next_x = (self.scroll_x - dx).clamp(0.0, max_x);
                if (next_x - self.scroll_x).abs() > 1e-3 {
                    self.scroll_x = next_x;
                    changed = true;
                }
            }
        }
        if changed {
            cx.notify();
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn begin_scroll_x_drag(
        &mut self,
        pointer_x: f32,
        track_left: f32,
        thumb_left: f32,
        thumb_w: f32,
        track_w: f32,
        max_x: f32,
        cx: &mut Context<Self>,
    ) {
        let local_x = pointer_x - track_left;
        if local_x >= thumb_left && local_x <= thumb_left + thumb_w {
            self.scroll_x_drag = Some((pointer_x, self.scroll_x, track_w, thumb_w, max_x));
        } else if max_x > 0.0 {
            let target = (local_x - thumb_w / 2.0).clamp(0.0, (track_w - thumb_w).max(0.0));
            self.scroll_x = target / (track_w - thumb_w).max(1.0) * max_x;
            self.scroll_x_drag = Some((pointer_x, self.scroll_x, track_w, thumb_w, max_x));
        }
        cx.notify();
    }

    fn scroll_x_drag_to(&mut self, pointer_x: f32, cx: &mut Context<Self>) {
        let Some((start_x, start_offset, track_w, thumb_w, max_x)) = self.scroll_x_drag else {
            return;
        };
        let travel = (track_w - thumb_w).max(1.0);
        let delta = (pointer_x - start_x) / travel * max_x;
        self.scroll_x = (start_offset + delta).clamp(0.0, max_x);
        cx.notify();
    }

    fn begin_scroll_drag(&mut self, pointer_y: f32, thumb_top: f32, thumb_height: f32, track_h: f32, view_h: f32, cx: &mut Context<Self>) {
        let track_y = TOP_BAR_HEIGHT;
        let local_y = pointer_y - track_y;
        let max = self.scroll_max_for_geometry(view_h, thumb_height);
        if local_y >= thumb_top && local_y <= thumb_top + thumb_height {
            self.scroll_drag = Some((pointer_y, self.scroll_offset));
        } else if max > 0.0 {
            let target = (local_y - thumb_height / 2.0)
                .clamp(0.0, (track_h - thumb_height).max(0.0));
            self.scroll_offset = target / (track_h - thumb_height).max(1.0) * max;
            self.scroll_drag = Some((pointer_y, self.scroll_offset));
        }
        cx.notify();
    }

    fn scroll_drag_to(&mut self, pointer_y: f32, track_h: f32, view_h: f32, thumb_height: f32, cx: &mut Context<Self>) {
        let Some((start_y, start_offset)) = self.scroll_drag else { return };
        let max = self.scroll_max_for_geometry(view_h, thumb_height);
        let travel = (track_h - thumb_height).max(1.0);
        let delta = (pointer_y - start_y) / travel * max;
        self.scroll_offset = (start_offset + delta).clamp(0.0, max);
        cx.notify();
    }

    fn scroll_max_for_geometry(&self, view_h: f32, thumb_height: f32) -> f32 {
        let total_h = self.scroll_content_rows as f32 * PREVIEW_LINE_HEIGHT;
        let _ = thumb_height;
        (total_h - view_h).max(0.0)
    }

    /// Move the cursor to a window point (click / drag). Coordinates are
    /// mapped through scroll offset, top padding, margins, and the wrap
    /// map, so the cursor lands on the exact character under the pointer.
    fn position_from_point(
        &mut self,
        x_win: f32,
        y_win: f32,
        window: &mut Window,
        extend: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(tab_id) = self
            .tabs
            .get(self.active_tab)
            .filter(|tab| tab.title != "Welcome")
            .map(|tab| tab.id)
        else {
            return;
        };
        let color = Hsla::from(rgba(crate::theme::active(cx).text_primary));
        let geo = self.geometry(window);
        let (left_occ, _) = self.dock_occupancy();
        let x_col = x_win - left_occ;
        // Ignore clicks landing on the scrollbar gutter.
        if x_col > geo.col_w - 18.0 {
            return;
        }
        // The text viewport starts below both the top bar and the image/text
        // toolbar. Omitting the toolbar offset makes clicks resolve one or
        // more rows below the pointer.
        let y_view = y_win - TOP_BAR_HEIGHT - TOOLBAR_HEIGHT;
        let view_h = Self::viewport_h(window);
        let ed = self.editors.entry(tab_id).or_insert_with(EditorState::new);
        let (buffer_line, row_start, row_end) = if geo.measure == f32::MAX {
            // Unwrapped: visual rows map 1-to-1 to buffer lines in O(1).
            let total_rows = ed.len_lines().max(1);
            let total_h = total_rows as f32 * PREVIEW_LINE_HEIGHT;
            let max = (total_h - view_h).max(0.0);
            let offset = self.scroll_offset.min(max);
            let content_y = y_view + offset;
            let idx = ((content_y - PREVIEW_TOP_PAD) / PREVIEW_LINE_HEIGHT)
                .floor()
                .max(0.0) as usize;
            let line = idx.min(total_rows - 1);
            (line, 0, usize::MAX)
        } else if let Some(wrap) = self.cached_wrap.as_ref() {
            if wrap.rows.is_empty() {
                return;
            }
            let total_h = wrap.rows.len() as f32 * PREVIEW_LINE_HEIGHT;
            let max = (total_h - view_h).max(0.0);
            let offset = self.scroll_offset.min(max);
            let content_y = y_view + offset;
            let idx = ((content_y - PREVIEW_TOP_PAD) / PREVIEW_LINE_HEIGHT)
                .floor()
                .max(0.0) as usize;
            wrap.rows[idx.min(wrap.rows.len() - 1)]
        } else {
            return;
        };
        let text_left = preview_text_left(&geo);
        let max_x = (self.scroll_x_content_w - geo.text_width).max(0.0);
        let offset_x = if geo.measure == f32::MAX {
            self.scroll_x.clamp(0.0, max_x)
        } else {
            0.0
        };
        let x_text = x_win - text_left + offset_x;
        let row_text = ed.line_text(buffer_line);
        let start = row_start.min(row_text.len());
        let end = row_end.min(row_text.len());
        let row_slice = &row_text[start..end];
        let byte = if row_slice.is_empty() || x_text <= 0.0 {
            0
        } else {
            shape_row(window, row_slice, color, PREVIEW_FONT_SIZE, FontWeight::NORMAL, false)
                .closest_index_for_x(px(x_text))
        };
        let old_cursor = ed.cursor();
        let old_sel = ed.selection_range();
        ed.set_cursor_line_byte(buffer_line, start + byte, extend);
        let changed = ed.cursor() != old_cursor || ed.selection_range() != old_sel;
        if changed || !extend {
            self.reset_caret_blink();
            cx.notify();
        }
    }

    pub fn undo(&mut self, cx: &mut Context<Self>) {
        if self.undo_annotations() {
            cx.notify();
            return;
        }
        if let Some(ed) = self.active_editor_mut() {
            ed.undo();
        }
        self.reset_caret_blink();
        if let Some(tab) = self.tabs.get(self.active_tab) {
            self.mark_tab_dirty(tab.id);
        } else {
            self.editor_version = self.editor_version.wrapping_add(1);
        }
        cx.notify();
    }

    pub fn redo(&mut self, cx: &mut Context<Self>) {
        if self.redo_annotations() {
            cx.notify();
            return;
        }
        if let Some(ed) = self.active_editor_mut() {
            ed.redo();
        }
        self.reset_caret_blink();
        if let Some(tab) = self.tabs.get(self.active_tab) {
            self.mark_tab_dirty(tab.id);
        } else {
            self.editor_version = self.editor_version.wrapping_add(1);
        }
        cx.notify();
    }

    /// Step back through the active image's annotation history.
    fn undo_annotations(&mut self) -> bool {
        let Some(path) = self.active_image_path() else {
            return false;
        };
        let Some(history) = self.image_history.get_mut(&path) else {
            return false;
        };
        let strokes = self.image_strokes.entry(path.clone()).or_default();
        if !history.undo(Rc::make_mut(strokes)) {
            return false;
        }
        self.invalidate_image_ink_cache();
        self.sync_image_dirty(&path);
        true
    }

    /// Step forward through the active image's annotation history.
    fn redo_annotations(&mut self) -> bool {
        let Some(path) = self.active_image_path() else {
            return false;
        };
        let Some(history) = self.image_history.get_mut(&path) else {
            return false;
        };
        let strokes = self.image_strokes.entry(path.clone()).or_default();
        if !history.redo(Rc::make_mut(strokes)) {
            return false;
        }
        self.invalidate_image_ink_cache();
        self.sync_image_dirty(&path);
        true
    }

    pub(super) fn invalidate_image_ink_cache(&mut self) {
        self.image_ink_revision = self.image_ink_revision.wrapping_add(1);
        *self.image_ink_cache.borrow_mut() = None;
    }

    /// The path of the image in the active tab, if there is one.
    pub(super) fn active_image_path(&self) -> Option<String> {
        let tab = self.tabs.get(self.active_tab)?;
        let path = tab.path.clone()?;
        (viewer_kind(Some(&path)) == ViewerKind::Image).then_some(path)
    }

    /// Update the dirty mark of an image's tab from whether any unbaked
    /// annotation strokes remain. Drawing and then erasing (or undoing) every
    /// stroke clears the dot immediately.
    pub(super) fn sync_image_dirty(&mut self, path: &str) {
        let dirty = self
            .image_strokes
            .get(path)
            .is_some_and(|strokes| !strokes.is_empty());
        let Some(tab) = self.tabs.iter().find(|t| t.path.as_deref() == Some(path)) else {
            return;
        };
        let id = tab.id;
        if dirty {
            self.dirty_tabs.insert(id);
        } else {
            self.dirty_tabs.remove(&id);
        }
    }

    // ----- hamburger menu state -----

    pub fn record_cursor(&mut self, pos: Point<gpui::Pixels>) {
        let now = Instant::now();
        self.cursor_trail.push((pos, now));
        self.cursor_trail
            .retain(|(_, t)| now.duration_since(*t).as_millis() <= 120);
    }

    // Velocity in px over the recent trail.
    pub fn cursor_velocity(&self) -> (f32, f32) {
        let Some((first, t0)) = self.cursor_trail.first() else {
            return (0., 0.);
        };
        let Some((last, t1)) = self.cursor_trail.last() else {
            return (0., 0.);
        };
        let dt = t1.duration_since(*t0).as_secs_f32();
        if dt < f32::EPSILON {
            return (0., 0.);
        }
        (
            (last.x - first.x).as_f32() / dt,
            (last.y - first.y).as_f32() / dt,
        )
    }

    pub fn toggle_menu(&mut self, cx: &mut Context<Self>) {
        self.menu_open = !self.menu_open;
        // Every transition — open or close — starts from cold state. Hover
        // looks must come from live hover events only, never leftovers.
        self.reset_menu_state();
        self.menu_animation = 0.0;
        self.animate_icon(self.menu_open, cx);
        if self.menu_open {
            self.start_menu_animation(cx);
        }
        cx.notify();
    }

    pub fn toggle_settings(&mut self, cx: &mut Context<Self>) {
        self.settings_open = !self.settings_open;
        cx.notify();
    }

    pub fn set_wrap_text_files(&mut self, value: bool, cx: &mut Context<Self>) {
        self.wrap_text_files = value;
        self.store.set("editor.wrap_text_files", if value { "true" } else { "false" });
        cx.notify();
    }

    pub fn set_wrap_code_files(&mut self, value: bool, cx: &mut Context<Self>) {
        self.wrap_code_files = value;
        self.store.set("editor.wrap_code_files", if value { "true" } else { "false" });
        cx.notify();
    }

    pub fn set_line_numbers(&mut self, value: bool, cx: &mut Context<Self>) {
        self.line_numbers = value;
        self.store.set("editor.line_numbers", if value { "true" } else { "false" });
        cx.notify();
    }

    pub fn set_syntax_highlighting(&mut self, value: bool, cx: &mut Context<Self>) {
        self.syntax_highlighting = value;
        self.store.set("editor.syntax_highlighting", if value { "true" } else { "false" });
        cx.notify();
    }

    pub fn set_windows_open_with(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.windows_open_with = enabled;
        self.store.set("windows.open_with", if enabled { "enabled" } else { "disabled" });
        let result = if enabled {
            crate::win_register::register_windows_open_with()
        } else {
            crate::win_register::unregister_windows_open_with()
        };
        match result {
            Ok(()) => self.set_status_message(if enabled {
                "Registered Peeklet in Windows \"Open with\""
            } else {
                "Removed Peeklet from Windows \"Open with\""
            }),
            Err(err) => self.set_status_message(err),
        }
        cx.notify();
    }

    /// Clear submenu, hover tracking, and hover tweens.
    fn reset_menu_state(&mut self) {
        self.active_menu = None;
        self.hovered_entry = None;
        self.fades.retain(|k, _| {
            !k.starts_with("menu-entry-") && !k.starts_with("submenu-")
        });
        self.fade_pending
            .retain(|k, _| !k.starts_with("menu-entry-") && !k.starts_with("submenu-"));
        self.fade_tween_active
            .retain(|k| !k.starts_with("menu-entry-") && !k.starts_with("submenu-"));
    }

    // Fades the menu icon between its idle and active styling.
    pub fn animate_icon(&mut self, opening: bool, cx: &mut Context<Self>) {
        let start = self.icon_animation;
        let end = if opening { 1.0 } else { 0.0 };
        cx.spawn(async move |this, cx| {
            let steps = 6;
            for i in 1..=steps {
                cx.background_executor()
                    .timer(Duration::from_millis(12))
                    .await;
                let _ = this.update(cx, |root, cx| {
                    root.icon_animation = start + (end - start) * (i as f32 / steps as f32);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub(super) fn start_image_zoom_animation(&mut self, cx: &mut Context<Self>) {
        if self.image_zoom_tween_active {
            return;
        }
        self.image_zoom_tween_active = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(12))
                    .await;
                let mut done = false;
                let _ = this.update(cx, |root, cx| {
                    let zoom_delta = root.image_zoom_target - root.image_zoom;
                    let pan_x_delta = root.image_pan_target.0 - root.image_pan.0;
                    let pan_y_delta = root.image_pan_target.1 - root.image_pan.1;
                    if zoom_delta.abs() < 0.001
                        && pan_x_delta.abs() < 0.05
                        && pan_y_delta.abs() < 0.05
                    {
                        root.image_zoom = root.image_zoom_target;
                        root.image_pan = root.image_pan_target;
                        root.image_zoom_tween_active = false;
                        done = true;
                    } else {
                        // Exponential easing keeps the motion smooth while
                        // still allowing rapid wheel input to retarget it.
                        root.image_zoom += zoom_delta * 0.24;
                        root.image_pan.0 += pan_x_delta * 0.24;
                        root.image_pan.1 += pan_y_delta * 0.24;
                    }
                    cx.notify();
                });
                if done {
                    break;
                }
            }
        })
        .detach();
    }

    pub fn start_menu_animation(&mut self, cx: &mut Context<Self>) {
        self.menu_animation = 0.0;
        cx.spawn(async move |this, cx| {
            let steps = 6;
            for i in 1..=steps {
                cx.background_executor()
                    .timer(Duration::from_millis(12))
                    .await;
                let _ = this.update(cx, |root, cx| {
                    root.menu_animation = i as f32 / steps as f32;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn close_menu(&mut self, cx: &mut Context<Self>) {
        if !self.menu_open {
            return;
        }
        self.menu_open = false;
        self.reset_menu_state();
        self.menu_animation = 0.0;
        self.animate_icon(false, cx);
        cx.notify();
    }

    pub fn set_active_menu(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        if self.active_menu == index {
            return;
        }
        self.active_menu = index;
        cx.notify();
    }

    // ----- hover fades -----

    pub fn fade(&self, key: &str) -> f32 {
        self.fades.get(key).copied().unwrap_or(0.0)
    }

    // Tweens a named fade toward the target (0 or 1). A single ticker per
    // key eases exponentially toward whatever the pending target is, so
    // rapid hover in/out never fights itself.
    pub fn animate_fade(&mut self, key: &str, target: f32, cx: &mut Context<Self>) {
        self.fade_pending.insert(key.to_string(), target);
        if !self.fade_tween_active.insert(key.to_string()) {
            return;
        }
        let key_owned = key.to_string();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(12))
                    .await;
                let mut done = false;
                let _ = this.update(cx, |root, cx| {
                    if let Some(&target) = root.fade_pending.get(&key_owned) {
                        let cur = root.fade(&key_owned);
                        let next = cur + (target - cur) * 0.4;
                        if (next - target).abs() < 0.01 {
                            root.fades.insert(key_owned.clone(), target);
                            done = true;
                        } else {
                            root.fades.insert(key_owned.clone(), next);
                        }
                        cx.notify();
                    } else {
                        done = true;
                    }
                });
                if !done {
                    continue;
                }
                let _ = this.update(cx, |root, _| {
                    root.fade_pending.remove(&key_owned);
                    root.fade_tween_active.remove(&key_owned);
                });
                break;
            }
        })
        .detach();
    }

    pub fn set_hovered_tab(&mut self, id: Option<usize>, cx: &mut Context<Self>) {
        if self.hovered_tab != id {
            self.hovered_tab = id;
            cx.notify();
        }
    }

    pub fn clear_hovered_tab(&mut self, id: usize, cx: &mut Context<Self>) {
        if self.hovered_tab == Some(id) {
            self.hovered_tab = None;
            cx.notify();
        }
    }

    pub fn begin_tab_scroll(&mut self, x: f32) {
        self.tab_scroll_drag = Some((x, self.tab_scroll_offset));
    }

    pub fn update_tab_scroll(&mut self, x: f32, cx: &mut Context<Self>) {
        let Some((start_x, start_offset)) = self.tab_scroll_drag else { return };
        self.tab_scroll_offset = (start_offset + start_x - x).max(0.0);
        cx.notify();
    }

    pub fn end_tab_scroll(&mut self, cx: &mut Context<Self>) {
        if self.tab_scroll_drag.take().is_some() {
            cx.notify();
        }
    }

    // ----- panels -----

    fn animate_left_sidebar(&mut self, target: f32, cx: &mut Context<Self>) {
        self.left_sidebar_target = target;
        if self.left_sidebar_animating {
            return;
        }
        self.left_sidebar_animating = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(12))
                    .await;
                let mut done = false;
                let _ = this.update(cx, |root, cx| {
                    let target = root.left_sidebar_target;
                    let delta = target - root.left_sidebar_progress;
                    if delta.abs() < 0.01 {
                        root.left_sidebar_progress = target;
                        root.left_sidebar_animating = false;
                        if target == 0.0 {
                            root.visible_left = None;
                        }
                        done = true;
                    } else {
                        root.left_sidebar_progress += delta * 0.28;
                    }
                    cx.notify();
                });
                if done {
                    break;
                }
            }
        })
        .detach();
    }

    pub fn toggle_panel(&mut self, kind: PanelKind, cx: &mut Context<Self>) {
        match Self::dock_of(kind) {
            DockSide::Left => {
                if self.visible_left == Some(kind) {
                    self.animate_left_sidebar(0.0, cx);
                } else {
                    self.visible_left = Some(kind);
                    self.animate_left_sidebar(1.0, cx);
                }
            }
            DockSide::Right => {
                self.visible_right =
                    if self.visible_right == Some(kind) { None } else { Some(kind) };
            }
        }
        self.save_layout();
        cx.notify();
    }

    pub(crate) fn panel_visible(&self, kind: PanelKind) -> bool {
        match Self::dock_of(kind) {
            DockSide::Left => self.visible_left == Some(kind),
            DockSide::Right => self.visible_right == Some(kind),
        }
    }

    pub(crate) fn active_is_pdf(&self) -> bool {
        self.tabs
            .get(self.active_tab)
            .and_then(|tab| tab.path.as_deref())
            .is_some_and(|path| viewer_kind(Some(path)) == ViewerKind::Pdf)
    }

    pub(crate) fn active_is_image(&self) -> bool {
        self.tabs
            .get(self.active_tab)
            .and_then(|tab| tab.path.as_deref())
            .is_some_and(|path| viewer_kind(Some(path)) == ViewerKind::Image)
    }

    pub(crate) fn active_is_markdown(&self) -> bool {
        self.tabs
            .get(self.active_tab)
            .and_then(|tab| tab.path.as_deref())
            .is_some_and(markdown::is_markdown_path)
    }

    pub fn toggle_markdown_mode(&mut self, target: MarkdownViewMode, cx: &mut Context<Self>) {
        self.markdown_mode = if self.markdown_mode == target {
            MarkdownViewMode::Plain
        } else {
            target
        };
        self.md_heading_dropdown_open = false;
        self.cached_wrap_key = (usize::MAX, 0, 0);
        cx.notify();
    }

    /// Heading level (`1..=6`) on the active cursor line, or `0` for paragraph.
    pub(crate) fn active_markdown_heading_level(&self) -> usize {
        let Some(tab) = self.tabs.get(self.active_tab) else {
            return 0;
        };
        let Some(ed) = self.editors.get(&tab.id) else {
            return 0;
        };
        let (line_no, _) = ed.cursor_line_col();
        if line_no >= ed.len_lines() {
            return 0;
        }
        let line = ed.line_text(line_no);
        let trimmed = line.trim_start();
        let level = trimmed.bytes().take_while(|&b| b == b'#').count();
        if (1..=6).contains(&level)
            && (level == trimmed.len() || trimmed.as_bytes()[level] == b' ')
        {
            level
        } else {
            0
        }
    }

    pub(super) fn finish_md_edit(&mut self, cx: &mut Context<Self>) {
        self.reset_caret_blink();
        self.editor_version = self.editor_version.wrapping_add(1);
        if let Some(tab) = self.tabs.get(self.active_tab) {
            self.mark_tab_dirty(tab.id);
        }
        cx.notify();
    }

    /// Remove strokes (or parts of them) near the given image-space point.
    pub fn erase_image_strokes(&mut self, path: &str, point: (f32, f32), radius: f32) {
        let removed = match self.image_strokes.get_mut(path) {
            Some(strokes) => ink::erase(Rc::make_mut(strokes), point.0, point.1, radius),
            None => Vec::new(),
        };
        if removed.is_empty() {
            return;
        }
        // The whole drag is one undo step, so hold on to what it removed.
        self.image_erased.extend(removed);
        self.invalidate_image_ink_cache();
        self.sync_image_dirty(path);
    }

    /// Start an annotation stroke (or an eraser pass) at an image-space point.
    pub fn begin_ink_stroke(
        &mut self,
        path: &str,
        x: f32,
        y: f32,
        pen_width: f32,
        hl_width: f32,
        eraser_radius: f32,
    ) {
        // A previous drag that never got its mouse-up still has to be
        // recorded, otherwise its change would be lost from the history.
        self.flush_erased(path);
        let tool = self.image_toolbar_tool as u8;
        match tool {
            ink::TOOL_PEN | ink::TOOL_HIGHLIGHTER => {
                let width = if tool == ink::TOOL_PEN {
                    pen_width
                } else {
                    hl_width
                };
                self.image_drawing = true;
                self.image_ink_path = path.to_string();
                let color = self.ink_color(tool);
                self.image_current_stroke =
                    Some(InkStroke::new(tool, color, width, vec![(x, y)]));
            }
            ink::TOOL_ERASER => {
                self.image_drawing = true;
                self.image_ink_path = path.to_string();
                self.image_current_stroke = None;
                self.erase_image_strokes(path, (x, y), eraser_radius);
            }
            _ => {}
        }
    }

    /// Extend the stroke in progress to an image-space point.
    pub fn extend_ink_stroke(&mut self, x: f32, y: f32, eraser_radius: f32) -> bool {
        if !self.image_drawing {
            return false;
        }
        if self.image_toolbar_tool as u8 == ink::TOOL_ERASER {
            let path = self.image_ink_path.clone();
            let before = self.image_erased.len();
            self.erase_image_strokes(&path, (x, y), eraser_radius);
            return self.image_erased.len() != before;
        }
        if let Some(stroke) = &mut self.image_current_stroke {
            // Ignore samples that are too close together; they add nothing but
            // path vertices. The spacing is fixed in screen terms so the
            // smoothing is the same at every zoom level.
            let min_dist = (1.5 / self.image_fit).max(0.05);
            let too_close = match stroke.points.last() {
                Some((lx, ly)) => {
                    let dx = x - lx;
                    let dy = y - ly;
                    (dx * dx + dy * dy).sqrt() < min_dist
                }
                None => false,
            };
            if !too_close {
                stroke.points.push((x, y));
                return true;
            }
        }
        false
    }

    /// Finish the stroke in progress and store it.
    pub fn end_ink_stroke(&mut self) {
        self.image_drawing = false;
        let path = std::mem::take(&mut self.image_ink_path);
        if let Some(stroke) = self.image_current_stroke.take() {
            if stroke.points.len() >= 2 {
                let strokes = Rc::make_mut(self.image_strokes.entry(path.clone()).or_default());
                let index = strokes.len();
                strokes.push(stroke.clone());
                self.invalidate_image_ink_cache();
                self.image_history
                    .entry(path.clone())
                    .or_default()
                    .push_added(index, stroke);
                // Annotations are unsaved work, so the tab gets a dirty mark.
                self.mark_path_dirty(&path);
            }
        }
        // The eraser collects what it removed over the whole drag; that is
        // one undo step.
        self.flush_erased(&path);
    }

    /// Record the strokes the eraser removed during the current drag.
    fn flush_erased(&mut self, path: &str) {
        if self.image_erased.is_empty() {
            return;
        }
        let removed = std::mem::take(&mut self.image_erased);
        self.image_history
            .entry(path.to_string())
            .or_default()
            .push_removed(removed);
        self.sync_image_dirty(path);
    }

    /// Mark the tab that shows the file at `path` as having unsaved changes.
    fn mark_path_dirty(&mut self, path: &str) {
        if let Some(tab) = self.tabs.iter().find(|t| t.path.as_deref() == Some(path)) {
            let id = tab.id;
            self.dirty_tabs.insert(id);
        }
    }

    pub(crate) fn pdf_page_assets(&self) -> Vec<(Arc<gpui::Image>, u32, u32)> {
        self.tabs
            .get(self.active_tab)
            .and_then(|tab| tab.path.as_deref())
            .and_then(|path| self.pdf_assets.get(path))
            .map(|pages| pages.iter().map(|page| (page.image.clone(), page.width, page.height)).collect())
            .unwrap_or_default()
    }

    pub fn project_entries(&self) -> Vec<ProjectEntry> {
        fn collect(root: &RootView, dir: &std::path::Path, depth: usize, out: &mut Vec<ProjectEntry>) {
            if depth >= 5 { return; }
            let Ok(read_dir) = std::fs::read_dir(dir) else { return; };
            let mut entries: Vec<_> = read_dir
                .filter_map(Result::ok)
                .filter(|entry| {
                    let name = entry.file_name();
                    let name = name.to_string_lossy();
                    !name.starts_with('.') && !matches!(name.as_ref(), "target" | "node_modules")
                })
                .collect();
            entries.sort_by_key(|entry| {
                let is_file = entry.file_type().map(|kind| !kind.is_dir()).unwrap_or(true);
                (is_file, entry.file_name())
            });
            for entry in entries {
                let path = entry.path();
                let is_dir = entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
                out.push(ProjectEntry {
                    name: entry.file_name().to_string_lossy().into_owned(),
                    path: path.clone(),
                    is_dir,
                    depth,
                });
                if is_dir && root.project_expanded.contains(&path) {
                    collect(root, &path, depth + 1, out);
                }
            }
        }
        let mut entries = Vec::new();
        if let Some(project_root) = &self.project_root {
            collect(self, project_root, 0, &mut entries);
        }
        entries
    }

    pub fn project_root_name(&self) -> Option<String> {
        self.project_root.as_ref()?.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .or_else(|| self.project_root.as_ref().map(|path| path.display().to_string()))
    }

    pub fn set_project_folder(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        self.project_root = Some(path.clone());
        self.project_expanded.clear();
        self.project_expanded.insert(path);
        cx.notify();
    }

    pub fn toggle_project_folder(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        if !self.project_expanded.insert(path.clone()) {
            self.project_expanded.remove(&path);
        }
        cx.notify();
    }

    pub fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        crate::theme::toggle(cx);
        let mode = match crate::theme::mode(cx) {
            crate::theme::ThemeMode::Light => "light",
            crate::theme::ThemeMode::Dark => "dark",
        };
        self.store.set("theme.mode", mode);
        cx.notify();
    }

    // ----- files -----

    /// Open a path in a tab (or activate its existing tab if already open).
    pub fn open_file_path(&mut self, path: String, cx: &mut Context<Self>) {
        // Deduplicate: if this path is already open in a tab, activate it.
        if let Some(existing) = self.tabs.iter().find(|t| {
            t.path.as_deref().is_some_and(|p| {
                if cfg!(target_os = "windows") {
                    p.eq_ignore_ascii_case(&path)
                } else {
                    p == path
                }
            })
        }) {
            let id = existing.id;
            self.activate_tab(id, cx);
            return;
        }
        // Replace a lone Welcome placeholder tab cleanly.
        if self.tabs.len() == 1 && self.tabs[0].title == "Welcome" && self.tabs[0].path.is_none() {
            self.tabs.clear();
        }
        let title = std::path::Path::new(&path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone());
        let content = match viewer_kind(Some(&path)) {
            ViewerKind::Text | ViewerKind::Csv | ViewerKind::Svg => {
                std::fs::read_to_string(&path).unwrap_or_default()
            }
            _ => String::new(),
        };
        self.recent_files.retain(|p| p != &path);
        self.recent_files.insert(0, path.clone());
        self.recent_files.truncate(12);
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        self.tabs.push(Tab { id, title, path: Some(path) });
        self.active_tab = self.tabs.len() - 1;
        self.image_zoom = 1.0;
        self.image_zoom_target = 1.0;
        self.image_pan = (0.0, 0.0);
        self.image_pan_target = (0.0, 0.0);
        self.image_drag = None;
        self.editors
            .entry(id)
            .or_insert_with(EditorState::new)
            .set_text(&content);
        self.saved_text_hash
            .insert(id, crate::editor::fingerprint_str(&content));
        self.scroll_offset = 0.0;
        self.save_layout();
        cx.notify();
    }

    fn mark_tab_dirty(&mut self, tab_id: usize) {
        self.editor_version = self.editor_version.wrapping_add(1);
        let Some(tab) = self.tabs.iter().find(|t| t.id == tab_id) else {
            return;
        };
        if tab.path.is_none() {
            return;
        }
        let is_dirty = match (self.editors.get(&tab_id), self.saved_text_hash.get(&tab_id)) {
            (Some(ed), Some(&saved)) => ed.content_fingerprint() != saved,
            _ => true,
        };
        if is_dirty {
            self.dirty_tabs.insert(tab_id);
        } else {
            self.dirty_tabs.remove(&tab_id);
        }
    }

    /// The colour an ink tool draws with.
    pub fn ink_color(&self, tool: u8) -> u32 {
        match tool {
            ink::TOOL_HIGHLIGHTER => self.ink_highlighter_color,
            _ => self.ink_pen_color,
        }
    }

    /// The width an ink tool draws with, in screen pixels at the fit zoom.
    pub fn ink_size(&self, tool: u8) -> f32 {
        match tool {
            ink::TOOL_HIGHLIGHTER => self.ink_highlighter_size,
            _ => self.ink_pen_size,
        }
    }

    /// Pick the colour of an ink tool and remember it.
    pub fn set_ink_color(&mut self, tool: u8, color: u32) {
        match tool {
            ink::TOOL_HIGHLIGHTER => {
                self.ink_highlighter_color = color;
                self.store.set("ink.highlighter.color", &color.to_string());
            }
            ink::TOOL_PEN => {
                self.ink_pen_color = color;
                self.store.set("ink.pen.color", &color.to_string());
            }
            _ => {}
        }
    }

    /// Open or close the ink colour/thickness popup with a smooth transition.
    pub fn set_ink_popup(&mut self, next: Option<u8>, cx: &mut Context<Self>) {
        if self.ink_popup == next {
            return;
        }
        self.ink_popup = next;
        if let Some(tool) = next {
            let current_size = self.ink_size(tool);
            self.ink_handle = current_size;
            self.ink_handle_target = current_size;
            if self.ink_popup_display != Some(tool) {
                self.ink_popup_display = Some(tool);
                self.ink_popup_progress = 0.0;
            }
        } else {
            self.ink_slider_dragging = false;
        }
        if self.ink_popup_animating {
            cx.notify();
            return;
        }
        self.ink_popup_animating = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(12))
                    .await;
                let mut done = false;
                let alive = this
                    .update(cx, |root, cx| {
                        let target = if root.ink_popup.is_some() { 1.0 } else { 0.0 };
                        let delta = target - root.ink_popup_progress;
                        if delta.abs() < 0.01 {
                            root.ink_popup_progress = target;
                            if target == 0.0 {
                                root.ink_popup_display = None;
                            }
                            root.ink_popup_animating = false;
                            done = true;
                        } else {
                            root.ink_popup_progress += delta * 0.30;
                        }
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

    /// Update the thickness while actively dragging the slider (tracks the
    /// magnetically-pulled pointer position directly without lag).
    pub fn drag_ink_size(&mut self, tool: u8, size: f32) {
        let size = size.clamp(INK_SIZE_MIN, INK_SIZE_MAX);
        self.ink_handle = size;
        self.ink_handle_target = size;
        match tool {
            ink::TOOL_HIGHLIGHTER => self.ink_highlighter_size = size,
            ink::TOOL_PEN => self.ink_pen_size = size,
            _ => {}
        }
    }

    /// Release the thickness slider and smoothly ease it to the nearest
    /// integer pixel tick.
    pub fn release_ink_slider(&mut self, cx: &mut Context<Self>) {
        if !self.ink_slider_dragging {
            return;
        }
        self.ink_slider_dragging = false;
        if let Some(tool) = self.ink_popup {
            let max_step = if tool == ink::TOOL_HIGHLIGHTER { 20.0 } else { 10.0 };
            let snapped = self.ink_size(tool).round().clamp(1.0, max_step);
            self.set_ink_size(tool, snapped, cx);
            cx.notify();
        }
    }

    /// Pick the width of an ink tool and remember it.
    pub fn set_ink_size(&mut self, tool: u8, size: f32, cx: &mut Context<Self>) {
        let size = size.clamp(INK_SIZE_MIN, INK_SIZE_MAX);
        self.slide_ink_handle(size, cx);
        match tool {
            ink::TOOL_HIGHLIGHTER => {
                self.ink_highlighter_size = size;
                self.store.set("ink.highlighter.size", &size.to_string());
            }
            ink::TOOL_PEN => {
                self.ink_pen_size = size;
                self.store.set("ink.pen.size", &size.to_string());
            }
            _ => {}
        }
    }

    /// Slide the thickness handle towards a thickness.
    ///
    /// The handle moves smoothly instead of jumping, so picking a thickness
    /// reads as the slider gliding to the mark.
    fn slide_ink_handle(&mut self, size: f32, cx: &mut Context<Self>) {
        if (self.ink_handle_target - size).abs() < 0.001 {
            return;
        }
        self.ink_handle_target = size;
        if self.ink_handle_active {
            return;
        }
        self.ink_handle_active = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(12))
                    .await;
                let mut done = false;
                let _ = this.update(cx, |root, cx| {
                    let delta = root.ink_handle_target - root.ink_handle;
                    if delta.abs() < 0.01 {
                        root.ink_handle = root.ink_handle_target;
                        root.ink_handle_active = false;
                        done = true;
                    } else {
                        // Exponential easing, so the motion is smooth.
                        root.ink_handle += delta * 0.25;
                    }
                    cx.notify();
                });
                if done {
                    break;
                }
            }
        })
        .detach();
    }

    /// Show a transient message in the status bar.
    pub fn set_status_message(&mut self, message: impl Into<String>) {
        self.status_message = Some((message.into(), Instant::now()));
    }

    pub fn bottom_status(&self) -> Option<Vec<String>> {
        if let Some((message, at)) = &self.status_message {
            if at.elapsed() < Duration::from_secs(8) {
                return Some(vec![message.clone()]);
            }
        }
        let tab = self.tabs.get(self.active_tab)?;
        if tab.title == "Welcome" {
            return None;
        }
        let path = tab.path.as_deref();
        let size_bytes = tab
            .path
            .as_ref()
            .and_then(|p| std::fs::metadata(p).ok().map(|m| m.len()))
            .unwrap_or(0);
        let size_str = if size_bytes < 1024 {
            format!("{} B", size_bytes)
        } else if size_bytes < 1024 * 1024 {
            format!("{:.1} KB", size_bytes as f64 / 1024.0)
        } else {
            format!("{:.1} MB", size_bytes as f64 / (1024.0 * 1024.0))
        };

        if viewer_kind(path) == ViewerKind::Image {
            let info = self.active_file_info();
            let dims = info.dimensions.unwrap_or_else(|| "—".into());
            let kind = tab
                .path
                .as_ref()
                .and_then(|p| std::path::Path::new(p).extension().and_then(|e| e.to_str()).map(|s| s.to_uppercase()))
                .unwrap_or_else(|| "IMAGE".to_string());
            return Some(vec![dims, kind, size_str]);
        }

        let id = tab.id;
        let ed = self.editors.get(&id)?;
        let (line, byte) = ed.cursor_line_byte();
        let line_text = ed.line_text(line);
        let col = line_text[..byte.min(line_text.len())].chars().count() + 1;
        let chars = ed.text_string().chars().count();
        let file_type = tab
            .path
            .as_ref()
            .and_then(|p| std::path::Path::new(p).extension().and_then(|e| e.to_str()).map(|s| s.to_string()))
            .unwrap_or_else(|| "Text".to_string());
        let mut parts = vec![
            format!("Ln {}, Col {}", line + 1, col),
            format!("{}", chars),
        ];
        if path.is_some_and(svg_view::is_svg_path) {
            if let Some(dims) = self.svg_dimensions_label(id) {
                parts.push(dims);
            }
        }
        parts.push(file_type);
        parts.push(size_str);
        Some(parts)
    }

    /// Write the active tab's buffer to its file. No-op without a path.
    fn save_active(&mut self, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(self.active_tab) else {
            return;
        };
        let Some(path) = tab.path.clone() else {
            return;
        };
        if viewer_kind(Some(&path)) == ViewerKind::Image {
            self.save_annotated_image(&path, cx);
            return;
        }
        if let Some(ed) = self.editors.get(&tab.id) {
            let _ = std::fs::write(&path, ed.text_string());
        }
    }

    /// Write the image back with the annotation strokes baked in.
    ///
    /// Strokes are stored in the source image's pixel space, so they are
    /// rasterized at the image's own resolution. A stroke that reaches
    /// outside the image grows the canvas instead of being clipped.
    ///
    /// The work runs on a background thread: decoding and re-encoding a large
    /// photo takes a while and the window has to stay usable meanwhile.
    fn save_annotated_image(&mut self, path: &str, cx: &mut Context<Self>) {
        let Some(strokes) = self.image_strokes.get(path) else {
            return;
        };
        if strokes.is_empty() {
            return;
        }
        let strokes = (**strokes).clone();
        let path = path.to_string();
        let name = file_name_of(&path);
        self.set_status_message(format!("Saving {name}..."));
        cx.spawn(async move |this, cx| {
            let job_path = path.clone();
            let result = cx
                .background_executor()
                .spawn(async move { bake_annotated_image(&job_path, &strokes) })
                .await;
            let _ = this.update(cx, |root, cx| {
                match result {
                    Ok(loaded) => {
                        // The strokes are now part of the file, so drop them.
                        // Reuse the encoded bytes we just wrote instead of
                        // re-reading and re-decoding the file from disk.
                        root.image_strokes.remove(&path);
                        root.image_history.remove(&path);
                        root.image_erased.clear();
                        root.invalidate_image_ink_cache();
                        root.image_loading.remove(&path);
                        root.image_load_errors.remove(&path);
                        if let Some(loaded) = loaded {
                            root.image_assets.insert(
                                path.clone(),
                                (loaded.image, loaded.width, loaded.height),
                            );
                            root.prune_image_cache(&path);
                        } else {
                            root.image_assets.remove(&path);
                            root.start_image_load(&path, cx);
                        }
                        root.set_status_message(format!("Saved {name}"));
                    }
                    Err(message) => root.set_status_message(message),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Strokes are stored in the source image's pixel space, so they are
    /// rasterized at the image's own resolution. A stroke that reaches
    /// outside the image grows the canvas instead of being clipped.
    /// Save the active tab, asking for a path when it has none.
    /// Never shows a dialog itself: callers run the dialog with no entity
    /// borrow held (a pumping native modal re-enters the app and panics
    /// against a held borrow), then apply the picked path here.
    pub fn save_step(&self) -> SaveStep {
        let Some(tab) = self.tabs.get(self.active_tab) else {
            return SaveStep::Nothing;
        };
        if tab.path.is_some() {
            return SaveStep::WriteActive;
        }
        if tab.title == "Welcome" || tab.title.is_empty() {
            return SaveStep::Nothing;
        }
        SaveStep::AskDialog(format!("{}.txt", tab.title))
    }

    /// Apply a save: write the active buffer, adopting `path` first when
    /// the tab had none.
    pub fn save_to(&mut self, path: Option<String>, cx: &mut Context<Self>) {
        if let Some(path) = path {
            let title = std::path::Path::new(&path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or(path.clone());
            if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                tab.path = Some(path);
                tab.title = title;
            }
        }
        self.save_active(cx);
        // Mark clean: update saved fingerprint, remove dirty state and persisted buffer.
        if let Some(tab) = self.tabs.get(self.active_tab) {
            let id = tab.id;
            if let Some(ed) = self.editors.get(&id) {
                self.saved_text_hash.insert(id, ed.content_fingerprint());
            }
            self.dirty_tabs.remove(&id);
            if let Some(path) = tab.path.clone() {
                self.store.set(&format!("dirty:{}", path), "0");
            }
        }
        self.save_layout();
        cx.notify();
    }

    // ----- dock drag-resize (unclamped; docks just can't overlap) -----

    pub fn begin_dock_drag(&mut self, side: DockSide, x: f32, max_total: f32, cx: &mut Context<Self>) {
        self.drag = Some((side, x, max_total));
        cx.notify();
    }

    fn drag_to(&mut self, x: f32, cx: &mut Context<Self>) {
        if let Some((side, last, max_total)) = self.drag {
            let dx = x - last;
            match side {
                DockSide::Left => {
                    let cap = (max_total - self.right_width).max(0.0);
                    self.left_width = (self.left_width + dx).clamp(0.0, cap);
                }
                DockSide::Right => {
                    let cap = (max_total - self.left_width).max(0.0);
                    self.right_width = (self.right_width - dx).clamp(0.0, cap);
                }
            }
            self.drag = Some((side, x, max_total));
            cx.notify();
        }
    }

    pub fn begin_md_split_drag(&mut self, cx: &mut Context<Self>) {
        self.md_split_dragging = true;
        cx.notify();
    }

    fn drag_md_split_to(&mut self, x: f32, view_w: f32, cx: &mut Context<Self>) {
        if !self.md_split_dragging {
            return;
        }
        let (left_occ, right_occ) = self.dock_occupancy();
        let full_col = (view_w - left_occ - right_occ).max(1.0);
        let rel_x = x - left_occ;
        // Magnetic snap to exact center (50/50) within 14px.
        let ratio = if (rel_x - full_col * 0.5).abs() <= 14.0 {
            0.5
        } else {
            (rel_x / full_col).clamp(0.2, 0.8)
        };
        if (self.md_split_ratio - ratio).abs() > 1e-4 {
            self.md_split_ratio = ratio;
            self.cached_wrap_key = (usize::MAX, 0, 0);
            cx.notify();
        }
    }

    fn end_drag(&mut self, cx: &mut Context<Self>) {
        self.release_ink_slider(cx);
        let was_split = std::mem::take(&mut self.md_split_dragging);
        let was_scroll_x = self.scroll_x_drag.take().is_some();
        let was_dragging = self.drag.take().is_some() || self.scroll_drag.take().is_some();
        self.mouse_select = false;
        if was_dragging || was_split || was_scroll_x {
            if was_dragging {
                self.save_layout();
            }
            cx.notify();
        }
    }

    /// Viewport height: window minus top and bottom bars and the toolbar.
    pub(super) fn viewport_h(window: &mut Window) -> f32 {
        (window.viewport_size().height.as_f32()
            - TOP_BAR_HEIGHT
            - BOTTOM_BAR_HEIGHT
            - TOOLBAR_HEIGHT)
            .max(0.0)
    }

    fn scrollbar_thumb_height(&self, window: &mut Window) -> f32 {
        let track_h = Self::viewport_h(window);
        let total_h = self.scroll_content_rows as f32 * PREVIEW_LINE_HEIGHT;
        (track_h / total_h.max(1.0) * track_h)
            .max(24.0)
            .min(track_h)
    }

    // ----- render -----

    fn render_preview(
        &mut self,
        root: WeakEntity<RootView>,
        wrap_cache: Option<Rc<EditorWrapCache>>,
        focus: FocusHandle,
        geo: &PreviewGeometry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let (tab_id, title, image_path) = self
            .tabs
            .get(self.active_tab)
            .map(|tab| (Some(tab.id), tab.title.clone(), tab.path.clone()))
            .unwrap_or((None, String::new(), None));
        if title == "Welcome" {
            return self.render_welcome(root, cx);
        }
        if viewer_kind(image_path.as_deref()) == ViewerKind::Image {
            let Some(path) = image_path.as_deref() else {
                unreachable!("image viewer requires a file path");
            };
            return self.render_image_preview(path, root, window, cx);
        }
        if viewer_kind(image_path.as_deref()) == ViewerKind::Pdf {
            let Some(path) = image_path.as_deref() else { unreachable!("pdf viewer requires a file path"); };
            return self.render_pdf_preview(path, root, window, cx);
        }
        let is_svg_split =
            image_path.as_deref().is_some_and(svg_view::is_svg_path) && self.svg_split_view;
        let svg_split_pane: Option<gpui::AnyElement> = if is_svg_split {
            if let Some(id) = tab_id {
                let raw_svg = self
                    .editors
                    .get(&id)
                    .map(|e| e.text_string())
                    .unwrap_or_default();
                Some(self.render_svg_preview_pane(id, &raw_svg, cx))
            } else {
                None
            }
        } else {
            None
        };
        let t = *crate::theme::active(cx);
        let is_dark = crate::theme::mode(cx) == ThemeMode::Dark;
        let active_path = image_path.unwrap_or_default();
        let is_code = is_code_path(&active_path);
        let is_markdown = markdown::is_markdown_path(&active_path);
        let md_mode = if is_markdown {
            self.markdown_mode
        } else {
            MarkdownViewMode::Plain
        };
        let highlight = (is_code && self.syntax_highlighting) || is_markdown;
        let preview_ext = std::path::Path::new(&active_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let show_line_numbers = self.line_numbers && is_code;
        let gutter_width = if show_line_numbers { LINE_NUMBER_GUTTER_WIDTH } else { 0.0 };

        let ed = tab_id.and_then(|id| self.editors.get(&id));
        let has_selection = ed.is_some_and(|e| e.selection_range().is_some());
        let caret_visible_now = self.caret_visible();
        self.last_caret_visible = caret_visible_now;
        let show_caret = caret_visible_now && !has_selection;

        let total_rows = match (&wrap_cache, ed) {
            (Some(wrap), _) => wrap.rows.len().max(1),
            (None, Some(e)) => e.len_lines().max(1),
            (None, None) => 1,
        };

        // Scrollbar geometry (Zed-style: thick low-opacity thumb + a 2px
        // accent line at the cursor row).
        let view_h = Self::viewport_h(window);
        let total_h = total_rows as f32 * PREVIEW_LINE_HEIGHT;
        let offset = self.scroll_offset.min((total_h - view_h).max(0.0));
        let start_row = (offset / PREVIEW_LINE_HEIGHT).floor() as usize;
        let visible_count = (view_h / PREVIEW_LINE_HEIGHT).ceil() as usize + 2;
        let end_row = (start_row + visible_count).min(total_rows);
        let top_offset = offset - start_row as f32 * PREVIEW_LINE_HEIGHT;
        let show_scroll = total_h > view_h;
        let max_x = if geo.measure == f32::MAX {
            (self.scroll_x_content_w - geo.text_width).max(0.0)
        } else {
            0.0
        };
        let offset_x = if geo.measure == f32::MAX {
            self.scroll_x.clamp(0.0, max_x)
        } else {
            0.0
        };
        let show_scroll_x = geo.measure == f32::MAX && max_x > 1.0;
        // Flush with the left edge of the editor column and the right vertical
        // scrollbar, matching the vertical scrollbar's exact geometry.
        let track_w_x = (geo.col_w - if show_scroll { 8.0 } else { 0.0 }).max(0.0);
        let thumb_w_x = if show_scroll_x && self.scroll_x_content_w > 0.0 {
            (geo.text_width / self.scroll_x_content_w * track_w_x)
                .max(24.0)
                .min(track_w_x)
        } else {
            0.0
        };
        let thumb_left_x = if show_scroll_x && max_x > 0.0 {
            (offset_x / max_x) * (track_w_x - thumb_w_x).max(0.0)
        } else {
            0.0
        };
        let track_h = view_h.max(0.0);
        let thumb_h = if show_scroll {
            (view_h / total_h * track_h).max(24.0).min(track_h)
        } else {
            0.0
        };
        let thumb_top = if show_scroll {
            let max = (total_h - view_h).max(0.0);
            if max <= 0.0 {
                0.0
            } else {
                (offset / max) * (track_h - thumb_h).max(0.0)
            }
        } else {
            0.0
        };

        let (cur_line, cur_byte) = ed.map(|e| e.cursor_line_byte()).unwrap_or((0, 0));
        let sel_chars = ed.and_then(|e| e.selection_range());
        let cursor_row_idx = if let Some(wrap) = &wrap_cache {
            let first = wrap.rows.partition_point(|&(l, _, _)| l < cur_line);
            let mut found = first.min(wrap.rows.len().saturating_sub(1));
            for (offset_i, &(l, rs, re)) in wrap.rows[first..].iter().enumerate() {
                if l != cur_line {
                    break;
                }
                found = first + offset_i;
                if rs <= cur_byte && cur_byte <= re {
                    break;
                }
            }
            found
        } else {
            cur_line.min(total_rows - 1)
        };
        let cursor_y = if show_scroll && total_h > 0.0 {
            ((cursor_row_idx as f32 * PREVIEW_LINE_HEIGHT + PREVIEW_LINE_HEIGHT / 2.0) / total_h)
                * track_h
        } else {
            0.0
        };

        // Build only the rows currently visible in the viewport: syntax
        // highlighting and glyph shaping never run for off-screen lines.
        let primary_hsla = Hsla::from(rgba(t.text_primary));
        let mut visible_rows: Vec<VisibleRow> = Vec::with_capacity(end_row.saturating_sub(start_row));
        if let Some(ed) = ed {
            // Track fenced code block state up to the first visible line when
            // rendering Markdown preview (Split or Stacked).
            let mut in_code_block = false;
            if md_mode != MarkdownViewMode::Plain && start_row < end_row {
                let first_buf_line = if let Some(wrap) = &wrap_cache {
                    wrap.rows.get(start_row).map(|r| r.0).unwrap_or(0)
                } else {
                    start_row
                };
                for l in 0..first_buf_line.min(ed.len_lines()) {
                    if markdown::is_fence_line(&ed.line_text(l)) {
                        in_code_block = !in_code_block;
                    }
                }
            }

            let mut cached_line_no: Option<usize> = None;
            let mut cached_line_text = String::new();
            let mut cached_spans: Vec<(u32, usize, usize)> = Vec::new();
            let mut cached_sel_bytes: Option<(usize, usize)> = None;
            let mut cached_in_code_block = in_code_block;
            let mut caret_placed = false;

            for row_idx in start_row..end_row {
                let (buffer_line, rs, re) = if let Some(wrap) = &wrap_cache {
                    if row_idx < wrap.rows.len() {
                        wrap.rows[row_idx]
                    } else {
                        (0, 0, 0)
                    }
                } else {
                    (row_idx, 0, usize::MAX)
                };

                if cached_line_no != Some(buffer_line) {
                    cached_line_no = Some(buffer_line);
                    cached_line_text = if buffer_line < ed.len_lines() {
                        ed.line_text(buffer_line)
                    } else {
                        String::new()
                    };
                    if md_mode != MarkdownViewMode::Plain {
                        if markdown::is_fence_line(&cached_line_text) {
                            cached_in_code_block = in_code_block;
                            in_code_block = !in_code_block;
                        } else {
                            cached_in_code_block = in_code_block;
                        }
                    }
                    cached_spans = if is_markdown {
                        markdown::markdown_symbol_spans(&cached_line_text, t.accent, t.text_primary)
                    } else if highlight {
                        highlight_line_spans(&cached_line_text, &preview_ext, is_dark, t.text_primary)
                    } else {
                        Vec::new()
                    };
                    cached_sel_bytes = sel_chars.and_then(|(a, b)| {
                        if buffer_line >= ed.len_lines() {
                            return None;
                        }
                        let ls = ed.line_start_char(buffer_line);
                        let llen = ed.line_char_len(buffer_line);
                        let o0 = a.max(ls);
                        let o1 = b.min(ls + llen);
                        if o0 >= o1 {
                            None
                        } else {
                            Some((
                                EditorState::col_to_bytes(&cached_line_text, o0 - ls),
                                EditorState::col_to_bytes(&cached_line_text, o1 - ls),
                            ))
                        }
                    });
                }

                let row_rs = rs.min(cached_line_text.len());
                let row_re = re.min(cached_line_text.len());
                let row_slice = &cached_line_text[row_rs..row_re];
                let highlights = if highlight {
                    row_highlights(row_rs, row_re, t.text_primary, &cached_spans)
                } else {
                    Vec::new()
                };
                let row_contains_cursor = row_rs <= cur_byte && cur_byte <= row_re;
                let needs_caret = !caret_placed && buffer_line == cur_line && row_contains_cursor;
                let sel_on_row = cached_sel_bytes.and_then(|(s0, s1)| {
                    let a = s0.max(row_rs);
                    let b = s1.min(row_re);
                    (a < b).then(|| (a - row_rs, b - row_rs))
                });

                let (caret, sel_px) = if needs_caret || sel_on_row.is_some() {
                    let shaped = if row_slice.is_empty() {
                        None
                    } else {
                        Some(shape_row(
                            window,
                            row_slice,
                            primary_hsla,
                            PREVIEW_FONT_SIZE,
                            FontWeight::NORMAL,
                            false,
                        ))
                    };
                    let caret = if needs_caret {
                        let rel = (cur_byte - row_rs).min(row_slice.len());
                        let x = shaped
                            .as_ref()
                            .map(|sh| sh.x_for_index(rel).as_f32())
                            .unwrap_or(0.0);
                        caret_placed = true;
                        Some((x, (PREVIEW_FONT_SIZE + 4.0).min(PREVIEW_LINE_HEIGHT - 2.0)))
                    } else {
                        None
                    };
                    let sel_px = sel_on_row.and_then(|(a, b)| {
                        let sh = shaped.as_ref()?;
                        Some((sh.x_for_index(a).as_f32(), sh.x_for_index(b).as_f32()))
                    });
                    (caret, sel_px)
                } else {
                    (None, None)
                };

                visible_rows.push(VisibleRow {
                    text: row_slice.to_string(),
                    highlights,
                    buffer_line,
                    is_first_wrap_row: row_rs == 0,
                    is_last_wrap_row: row_re >= cached_line_text.len(),
                    in_code_block: cached_in_code_block,
                    caret,
                    sel_px,
                });
            }
        } else {
            visible_rows.push(VisibleRow {
                text: String::new(),
                highlights: Vec::new(),
                buffer_line: 0,
                is_first_wrap_row: true,
                is_last_wrap_row: true,
                in_code_block: false,
                caret: Some((0.0, CARET_HEIGHT)),
                sel_px: None,
            });
        }

        // In Split mode, prepare the right-hand rendered Markdown preview rows
        // (grouping contiguous Markdown table lines into proper bordered tables)
        // before `visible_rows` is consumed by the left-hand editor column.
        let split_preview_rows: Option<Vec<gpui::AnyElement>> = if md_mode == MarkdownViewMode::Split {
            let mut elements: Vec<gpui::AnyElement> = Vec::with_capacity(visible_rows.len());
            let mut idx = 0usize;
            while idx < visible_rows.len() {
                let row = &visible_rows[idx];
                if let Some(ed_ref) = ed {
                    let kind = table_row_kind(ed_ref, row.buffer_line);
                    if kind == markdown::TableRowKind::Header && row.is_first_wrap_row {
                        let header_line = ed_ref.line_text(row.buffer_line);
                        let mut body_lines: Vec<String> = Vec::new();
                        let mut end_line = row.buffer_line + 1; // separator
                        let mut scan = row.buffer_line + 2;
                        while scan < ed_ref.len_lines() {
                            let l = ed_ref.line_text(scan);
                            if markdown::parse_table_cells(&l).is_some()
                                && !markdown::is_table_separator(&l)
                            {
                                body_lines.push(l);
                                end_line = scan;
                                scan += 1;
                            } else {
                                break;
                            }
                        }
                        let header_cells =
                            markdown::parse_table_cells(&header_line).unwrap_or_default();
                        let body_cells: Vec<Vec<&str>> = body_lines
                            .iter()
                            .filter_map(|l| markdown::parse_table_cells(l))
                            .collect();
                        elements.push(markdown::render_markdown_table(
                            &header_cells,
                            &body_cells,
                            PREVIEW_FONT_SIZE,
                            &t,
                        ));
                        while idx < visible_rows.len() && visible_rows[idx].buffer_line <= end_line {
                            idx += 1;
                        }
                        continue;
                    }
                    if kind != markdown::TableRowKind::None {
                        let full_line = ed_ref.line_text(row.buffer_line);
                        elements.push(markdown::render_markdown_table_line(
                            &full_line,
                            kind,
                            row.is_first_wrap_row,
                            PREVIEW_LINE_HEIGHT,
                            PREVIEW_FONT_SIZE,
                            &t,
                        ));
                        idx += 1;
                        continue;
                    }
                }
                let full_line = ed
                    .filter(|e| row.buffer_line < e.len_lines())
                    .map(|e| e.line_text(row.buffer_line))
                    .unwrap_or_default();
                let q_pos = ed
                    .map(|e| {
                        quote_pos_for_row(
                            e,
                            row.buffer_line,
                            row.is_first_wrap_row,
                            row.is_last_wrap_row,
                        )
                    })
                    .unwrap_or_default();
                let task_cb: Option<Box<dyn Fn(&mut Window, &mut App)>> =
                    if row.is_first_wrap_row && markdown::parse_task_line(&full_line).is_some() {
                        let root_weak = cx.entity().downgrade();
                        let target_line = row.buffer_line;
                        Some(Box::new(move |_, cx| {
                            let _ = root_weak.update(cx, |root, cx| {
                                root.md_toggle_task_at_line(target_line, cx);
                            });
                        }))
                    } else {
                        None
                    };
                elements.push(markdown::render_markdown_wrapped_row(
                    &row.text,
                    &full_line,
                    row.is_first_wrap_row,
                    row.in_code_block,
                    q_pos,
                    PREVIEW_LINE_HEIGHT,
                    PREVIEW_FONT_SIZE,
                    &t,
                    task_cb,
                ));
                idx += 1;
            }
            Some(elements)
        } else {
            None
        };

        div()
            .flex_1()
            .min_w(px(0.))
            .flex()
            .flex_col()
            .items_center()
            .relative()
            .overflow_hidden()
            .bg(rgba(t.bg_primary))
            .cursor_text()
            .child(
                div()
                    .id("preview-viewport")
                    .relative()
                    .w_full()
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_hidden()
                    .track_focus(&focus)
                    .on_mouse_down(MouseButton::Left, {
                        let root = root.clone();
                        let fh = focus.clone();
                        move |ev: &MouseDownEvent, window, cx| {
                            window.focus(&fh, cx);
                            let _ = root.update(cx, |root, cx| {
                                root.position_from_point(
                                    ev.position.x.as_f32(),
                                    ev.position.y.as_f32(),
                                    window,
                                    ev.modifiers.shift && ev.click_count <= 1,
                                    cx,
                                );
                                if ev.click_count == 2 {
                                    root.mouse_select = false;
                                    if let Some(ed) = root.active_editor_mut() {
                                        ed.select_word_at_cursor();
                                    }
                                    root.reset_caret_blink();
                                    cx.notify();
                                } else if ev.click_count >= 3 {
                                    root.mouse_select = false;
                                    if let Some(ed) = root.active_editor_mut() {
                                        ed.select_line_at_cursor();
                                    }
                                    root.reset_caret_blink();
                                    cx.notify();
                                } else {
                                    root.mouse_select = true;
                                }
                            });
                        }
                    })
                    .on_mouse_move(cx.listener(
                        |this, ev: &gpui::MouseMoveEvent, window, cx| {
                            if this.mouse_select {
                                this.position_from_point(
                                    ev.position.x.as_f32(),
                                    ev.position.y.as_f32(),
                                    window,
                                    true,
                                    cx,
                                );
                            }
                        },
                    ))
                    .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                        this.handle_key(ev, window, cx);
                    }))
                    .on_scroll_wheel(cx.listener(|this, ev: &ScrollWheelEvent, window, cx| {
                        this.handle_wheel(ev, window, cx);
                    }))
                    .child(
                        div()
                            .absolute()
                            .left(px(0.))
                            .right(px(0.))
                            .top(px(-top_offset))
                            .flex()
                            .flex_col()
                            .items_start()
                            .when(show_line_numbers, |d| {
                                d.child(
                                    div()
                                        .absolute()
                                        .left(px(preview_text_left(geo) - gutter_width))
                                        .top_0()
                                        .pt(px(PREVIEW_TOP_PAD))
                                        .w(px(gutter_width))
                                        .flex()
                                        .flex_col()
                                        .items_end()
                                        .pr(px(24.))
                                        .pl(px(8.))
                                        .children(visible_rows.iter().map(|row| {
                                            let label = if row.is_first_wrap_row {
                                                (row.buffer_line + 1).to_string()
                                            } else {
                                                String::new()
                                            };
                                            div()
                                                .h(px(PREVIEW_LINE_HEIGHT))
                                                .flex()
                                                .items_center()
                                                .justify_end()
                                                .text_sm()
                                                .font_family(crate::theme::FONT_MONO)
                                                .text_color(rgba(t.text_secondary))
                                                .child(label)
                                        })),
                                )
                            })
                            .child(
                                div()
                                    .w(px(geo.text_width))
                                    .ml(px(preview_text_left(geo)))
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .w(px(self.scroll_x_content_w.max(geo.text_width)))
                                            .ml(px(-offset_x))
                                            .flex()
                                            .flex_col()
                                            .pt(px(PREVIEW_TOP_PAD))
                                            .children(visible_rows.into_iter().map(|row| {
                                        // In Stacked (live preview) mode, render
                                        // every non-active, unselected Markdown
                                        // line as formatted preview while the
                                        // active cursor line shows raw symbols.
                                        if md_mode == MarkdownViewMode::Stacked
                                            && row.buffer_line != cur_line
                                            && row.sel_px.is_none()
                                        {
                                            let full_line = ed
                                                .filter(|e| row.buffer_line < e.len_lines())
                                                .map(|e| e.line_text(row.buffer_line))
                                                .unwrap_or_default();
                                            let t_kind = ed
                                                .map(|e| table_row_kind(e, row.buffer_line))
                                                .unwrap_or(markdown::TableRowKind::None);
                                            if t_kind != markdown::TableRowKind::None {
                                                return markdown::render_markdown_table_line(
                                                    &full_line,
                                                    t_kind,
                                                    row.is_first_wrap_row,
                                                    PREVIEW_LINE_HEIGHT,
                                                    PREVIEW_FONT_SIZE,
                                                    &t,
                                                );
                                            }
                                            let q_pos = ed
                                                .map(|e| {
                                                    quote_pos_for_row(
                                                        e,
                                                        row.buffer_line,
                                                        row.is_first_wrap_row,
                                                        row.is_last_wrap_row,
                                                    )
                                                })
                                                .unwrap_or_default();
                                            let task_cb: Option<Box<dyn Fn(&mut Window, &mut App)>> =
                                                if row.is_first_wrap_row
                                                    && markdown::parse_task_line(&full_line)
                                                        .is_some()
                                                {
                                                    let root_weak = cx.entity().downgrade();
                                                    let target_line = row.buffer_line;
                                                    Some(Box::new(move |_, cx| {
                                                        let _ = root_weak.update(cx, |root, cx| {
                                                            root.md_toggle_task_at_line(
                                                                target_line,
                                                                cx,
                                                            );
                                                        });
                                                    }))
                                                } else {
                                                    None
                                                };
                                            return markdown::render_markdown_wrapped_row(
                                                &row.text,
                                                &full_line,
                                                row.is_first_wrap_row,
                                                row.in_code_block,
                                                q_pos,
                                                PREVIEW_LINE_HEIGHT,
                                                PREVIEW_FONT_SIZE,
                                                &t,
                                                task_cb,
                                            );
                                        }

                                        // Render the whole row as one StyledText
                                        // run so kerning and subpixel glyph
                                        // advances match `shape_row` exactly.
                                        // Selection and caret are overlays.
                                        let styled = StyledText::new(row.text)
                                            .with_highlights(row.highlights);
                                        div()
                                            .relative()
                                            .w_full()
                                            .h(px(PREVIEW_LINE_HEIGHT))
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .whitespace_nowrap()
                                            .overflow_hidden()
                                            .font_family(crate::theme::FONT_TEXT)
                                            .text_size(px(PREVIEW_FONT_SIZE))
                                            .font_weight(FontWeight::NORMAL)
                                            .text_color(rgba(t.text_primary))
                                            .child(styled)
                                            .when_some(row.sel_px, |r, range| {
                                                let (x0, x1) = range;
                                                r.child(
                                                    div()
                                                        .absolute()
                                                        .left(px(x0))
                                                        .top(px(0.))
                                                        .w(px((x1 - x0).max(0.0)))
                                                        .h_full()
                                                        .bg(rgba(t.selection)),
                                                )
                                            })
                                            .when_some(
                                                row.caret.filter(|_| show_caret),
                                                |r, (x, h)| {
                                                    r.child(
                                                        div()
                                                            .absolute()
                                                            .left(px(x))
                                                            .top(px(
                                                                (PREVIEW_LINE_HEIGHT - h)
                                                                    / 2.0,
                                                            ))
                                                            .w(px(1.5))
                                                            .h(px(h))
                                                            .bg(rgba(t.text_primary)),
                                                    )
                                                },
                                            )
                                            .into_any_element()
                                    }))
                                    )
                            )
                            .when_some(split_preview_rows, |d, preview_rows| {
                                let (left_occ, right_occ) = self.dock_occupancy();
                                let full_col =
                                    (window.viewport_size().width.as_f32() - left_occ - right_occ)
                                        .max(0.0);
                                let split_x = geo.left_occ + geo.col_w;
                                let right_w = (full_col - geo.col_w - 1.0).max(0.0);
                                let root_split = root.clone();
                                d.child(
                                    div()
                                        .absolute()
                                        .left(px(split_x + 1.0))
                                        .top(px(0.))
                                        .w(px(right_w))
                                        .flex()
                                        .flex_col()
                                        .pt(px(PREVIEW_TOP_PAD))
                                        .px(px(MIN_COLUMN_MARGIN))
                                        .cursor_default()
                                        .children(preview_rows),
                                )
                                .child(
                                    div()
                                        .id("md-split-divider")
                                        .absolute()
                                        .left(px(split_x - 3.0))
                                        .top(px(top_offset))
                                        .w(px(7.))
                                        .h(px(view_h))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor_col_resize()
                                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                            let _ = root_split.update(cx, |root, cx| {
                                                root.begin_md_split_drag(cx);
                                            });
                                            cx.stop_propagation();
                                        })
                                        .child(
                                            div()
                                                .w(px(1.))
                                                .h_full()
                                                .bg(rgba(t.chrome_border_color)),
                                        ),
                                )
                            })
                )
                .when(show_scroll_x, |v| {
                    let track_left_x = geo.left_occ;
                    v.child(
                        div()
                            .id("manuscript-scrollbar-x")
                            .absolute()
                            .left(px(track_left_x))
                            .bottom(px(0.))
                            .w(px(track_w_x))
                            .h(px(8.))
                            .cursor_pointer()
                            .on_hover({
                                let root = root.clone();
                                move |hovered, _, cx| {
                                    let _ = root.update(cx, |this, cx| {
                                        let next = *hovered;
                                        if this.scrollbar_x_hovered != next {
                                            this.scrollbar_x_hovered = next;
                                            cx.notify();
                                        }
                                    });
                                }
                            })
                            .on_mouse_down(MouseButton::Left, {
                                let root = root.clone();
                                move |ev: &MouseDownEvent, _, cx| {
                                    let _ = root.update(cx, |this, cx| {
                                        this.begin_scroll_x_drag(
                                            ev.position.x.as_f32(),
                                            track_left_x,
                                            thumb_left_x,
                                            thumb_w_x,
                                            track_w_x,
                                            max_x,
                                            cx,
                                        );
                                    });
                                    cx.stop_propagation();
                                }
                            })
                            .child(
                                div()
                                    .absolute()
                                    .left(px(thumb_left_x))
                                    .top(px(0.))
                                    .bottom(px(0.))
                                    .w(px(thumb_w_x))
                                    .bg(rgba(t.text_secondary))
                                    .opacity(if self.scrollbar_x_hovered || self.scroll_x_drag.is_some() {
                                        0.30
                                    } else {
                                        0.16
                                    }),
                            ),
                    )
                })
                .when(show_scroll, |v| {
                    let mut sb = div()
                        .id("manuscript-scrollbar")
                        .absolute()
                        .top(px(0.))
                        .bottom(px(0.))
                        .w(px(8.))
                        .cursor_pointer();
                    if is_svg_split {
                        sb = sb.left(px((geo.left_occ + geo.col_w - 8.0).max(0.0)));
                    } else {
                        sb = sb.right(px(0.));
                    }
                    v.child(
                        sb
                            .on_hover({
                                let root = root.clone();
                                move |hovered, _, cx| {
                                    let _ = root.update(cx, |this, cx| {
                                        let next = *hovered;
                                        if this.scrollbar_hovered != next {
                                            this.scrollbar_hovered = next;
                                            cx.notify();
                                        }
                                    });
                                }
                            })
                            .on_mouse_down(MouseButton::Left, {
                                let root = root.clone();
                                move |ev: &MouseDownEvent, _, cx| {
                                    let _ = root.update(cx, |this, cx| {
                                        this.begin_scroll_drag(
                                            ev.position.y.as_f32(),
                                            thumb_top,
                                            thumb_h,
                                                track_h,
                                                view_h,
                                            cx,
                                        );
                                    });
                                    cx.stop_propagation();
                                }
                            })
                            .child(
                                div()
                                    .absolute()
                                    .left(px(0.))
                                    .right(px(0.))
                                    .top(px(thumb_top))
                                    .h(px(thumb_h))
                                    .bg(rgba(t.text_secondary))
                                    .opacity(if self.scrollbar_hovered { 0.30 } else { 0.16 }),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .left(px(0.))
                                    .right(px(0.))
                                    .top(px(cursor_y))
                                    .h(px(2.))
                                    .bg(rgba(t.accent))
                                    .opacity(0.10),
                            ),
                    )
                })
                .when_some(svg_split_pane, |v, svg_pane| {
                    let (left_occ, right_occ) = self.dock_occupancy();
                    let full_col =
                        (window.viewport_size().width.as_f32() - left_occ - right_occ).max(0.0);
                    let split_x = geo.left_occ + geo.col_w;
                    let right_w = (full_col - geo.col_w - 1.0).max(0.0);
                    let root_split = root.clone();
                    v.child(
                        div()
                            .absolute()
                            .left(px(split_x + 1.0))
                            .top(px(0.))
                            .w(px(right_w))
                            .h(px(view_h))
                            .flex()
                            .cursor_default()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                cx.stop_propagation();
                            })
                            .child(svg_pane),
                    )
                    .child(
                        div()
                            .id("svg-split-divider")
                            .absolute()
                            .left(px(split_x - 3.0))
                            .top(px(0.))
                            .w(px(7.))
                            .h(px(view_h))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_col_resize()
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                let _ = root_split.update(cx, |root, cx| {
                                    root.begin_md_split_drag(cx);
                                });
                                cx.stop_propagation();
                            })
                            .child(
                                div()
                                    .w(px(1.))
                                    .h_full()
                                    .bg(rgba(t.chrome_border_color)),
                            ),
                    )
                })
            )
            .into_any_element()
    }

}

impl Render for RootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = *crate::theme::active(cx);
        let root = cx.entity().downgrade();

        // Crisp instant caret blink ticker, started once for the entity's
        // lifetime. Only notifies when `caret_visible()` actually flips so
        // clicking or moving the cursor never triggers an early flash.
        if !self.blink_started {
            self.blink_started = true;
            cx.spawn(async move |weak, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(80))
                        .await;
                    let alive = weak
                        .update(cx, |root, cx| {
                            let is_text_tab = root.tabs.get(root.active_tab).is_some_and(|tab| {
                                tab.title != "Welcome"
                                    && matches!(
                                        viewer_kind(tab.path.as_deref()),
                                        ViewerKind::Text | ViewerKind::Svg | ViewerKind::Csv
                                    )
                            });
                            if !is_text_tab {
                                return;
                            }
                            let has_sel = root
                                .tabs
                                .get(root.active_tab)
                                .and_then(|tab| root.editors.get(&tab.id))
                                .is_some_and(|ed| ed.selection_range().is_some());
                            if has_sel {
                                return;
                            }
                            let visible = root.caret_visible();
                            if visible != root.last_caret_visible {
                                root.last_caret_visible = visible;
                                cx.notify();
                            }
                        })
                        .is_ok();
                    if !alive {
                        break;
                    }
                }
            })
            .detach();
        }

        // Geometry first: everything preview-related derives from it.
        let geo = self.geometry(window);

        // Build or reuse the wrap map only when wrapping is enabled and the
        // buffer text or wrap width changed. Unwrapped files and cursor /
        // selection changes do O(1) work here.
        let active_text_tab_id = self
            .tabs
            .get(self.active_tab)
            .filter(|tab| {
                tab.title != "Welcome"
                    && matches!(
                        viewer_kind(tab.path.as_deref()),
                        ViewerKind::Text | ViewerKind::Svg | ViewerKind::Csv
                    )
            })
            .map(|tab| tab.id);
        let measure_bits = geo.measure.to_bits();
        let wrap_key = (self.active_tab, self.editor_version, measure_bits);
        // Populate the printable-ASCII glyph width LUT once so wrapping and
        // unwrapped horizontal scroll bounds run with zero per-line font shaping.
        if active_text_tab_id.is_some() && self.ascii_char_widths.is_none() {
            let color = Hsla::from(rgba(t.text_primary));
            let mut lut = [0.0f32; 128];
            let mut max_w = 0.0f32;
            let mut buf = [0u8; 1];
            for b in 32u8..=126u8 {
                buf[0] = b;
                let s = std::str::from_utf8(&buf).unwrap();
                let w = measure_text(window, s, color, PREVIEW_FONT_SIZE, FontWeight::NORMAL, false);
                lut[b as usize] = w;
                if w > max_w {
                    max_w = w;
                }
            }
            lut[b'\t' as usize] = lut[b' ' as usize] * 4.0;
            self.ascii_char_widths = Some(lut);
            self.ascii_max_width = max_w.max(1.0);
        }

        let wrap_cache: Option<Rc<EditorWrapCache>> =
            if active_text_tab_id.is_none() || geo.measure == f32::MAX {
                None
            } else if self.cached_wrap_key == wrap_key {
                self.cached_wrap.clone()
            } else {
                let color = Hsla::from(rgba(t.text_primary));
                let lut = self.ascii_char_widths.unwrap_or([8.0; 128]);
                let max_w = self.ascii_max_width;

            let built = active_text_tab_id.map(|id| {
                let ed = self.editors.entry(id).or_insert_with(EditorState::new);
                let mut rows = Vec::with_capacity(ed.len_lines());
                for line_no in 0..ed.len_lines() {
                    let text = ed.line_text(line_no);
                    if text.is_empty()
                        || (text.is_ascii() && (text.len() as f32) * max_w <= geo.measure)
                    {
                        rows.push((line_no, 0, text.len()));
                        continue;
                    }
                    if text.is_ascii() {
                        let mut width_of = |s: &str| {
                            s.bytes().map(|b| lut[b as usize]).sum::<f32>() * 1.01
                        };
                        for row in crate::editor::wrap_line(line_no, &text, geo.measure, &mut width_of) {
                            rows.push((row.buffer_line, row.start_byte, row.end_byte));
                        }
                    } else {
                        let mut width_of = |s: &str| {
                            measure_text(window, s, color, PREVIEW_FONT_SIZE, FontWeight::NORMAL, false)
                        };
                        for row in crate::editor::wrap_line(line_no, &text, geo.measure, &mut width_of) {
                            rows.push((row.buffer_line, row.start_byte, row.end_byte));
                        }
                    }
                }
                Rc::new(EditorWrapCache { rows })
            });
            self.cached_wrap = built.clone();
            self.cached_wrap_key = wrap_key;
            built
        };

        // Compute cached max line pixel width for unwrapped horizontal scrolling.
        if geo.measure == f32::MAX {
            if let Some(id) = active_text_tab_id {
                let max_key = (self.active_tab, self.editor_version);
                if self.cached_max_line_key != max_key {
                    let ed = self.editors.entry(id).or_insert_with(EditorState::new);
                    let lut = self.ascii_char_widths.unwrap_or([8.0; 128]);
                    let mut widest = 0.0f32;
                    for line_no in 0..ed.len_lines() {
                        let text = ed.line_text(line_no);
                        let w: f32 = text
                            .bytes()
                            .map(|b| if b < 128 { lut[b as usize] } else { 8.5 })
                            .sum();
                        if w > widest {
                            widest = w;
                        }
                    }
                    self.cached_max_line_w = widest + 40.0;
                    self.cached_max_line_key = max_key;
                }
                self.scroll_x_content_w = self.cached_max_line_w;
            } else {
                self.scroll_x_content_w = 0.0;
            }
        } else {
            self.scroll_x_content_w = 0.0;
            self.scroll_x = 0.0;
        }
        let active_ed = active_text_tab_id.and_then(|id| {
            self.editors.entry(id).or_insert_with(EditorState::new);
            self.editors.get(&id)
        });
        let can_undo = active_ed.is_some_and(|ed| ed.can_undo());
        let can_redo = active_ed.is_some_and(|ed| ed.can_redo());
        self.scroll_content_rows = match (&wrap_cache, active_ed) {
            (Some(wrap), _) => wrap.rows.len(),
            (None, Some(ed)) => ed.len_lines(),
            (None, None) => 0,
        };
        let sidebar_width = self.left_width * self.left_sidebar_progress;
        let sidebar_dock = self.visible_left.map(|kind| DockView {
            root: root.clone(),
            side: DockSide::Left,
            visible: kind,
            width: sidebar_width,
            header: Some(
                div()
                    .h(px(TOP_BAR_HEIGHT))
                    .bg(rgba(t.bg_primary))
                    .border_b_1()
                    .border_color(rgba(t.chrome_border_color))
                    .into_any_element(),
            ),
            footer: Some(SidebarBottomBar { root: root.clone() }.into_any_element()),
        });

        div()
            .id("root")
            .size_full()
            .flex()
            .flex_col()
            .font_weight(FontWeight::NORMAL)
            .bg(rgba(t.bg_primary))
            .text_color(rgba(t.text_primary))
            // Track drags across the entire window (including over the top and
            // bottom bars) so scrollbar/split/sidebar drags never stall until
            // the left button is actually released.
            .on_mouse_move(cx.listener(|this, ev: &gpui::MouseMoveEvent, window, cx| {
                this.drag_to(ev.position.x.as_f32(), cx);
                this.drag_md_split_to(
                    ev.position.x.as_f32(),
                    window.viewport_size().width.as_f32(),
                    cx,
                );
                this.drag_image_crop_to(ev.position.x.as_f32(), ev.position.y.as_f32(), cx);
                this.scroll_x_drag_to(ev.position.x.as_f32(), cx);
                let thumb_h = this.scrollbar_thumb_height(window);
                let view_h = Self::viewport_h(window);
                this.scroll_drag_to(ev.position.y.as_f32(), view_h, view_h, thumb_h, cx);
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _: &gpui::MouseUpEvent, _, cx| {
                this.image_crop_drag = None;
                this.end_drag(cx);
            }))
            .on_mouse_up(MouseButton::Right, cx.listener(|this, _, _, cx| {
                this.image_drag = None;
                cx.notify();
            }))
            .on_mouse_up(MouseButton::Middle, cx.listener(|this, _, _, cx| {
                this.image_drag = None;
                cx.notify();
            }))
            .child(div().h(px(TOP_BAR_HEIGHT)).flex_shrink_0())
            // Middle row: docks flank the tab bar + preview column, and
            // span exactly that height (below top bar, above bottom bar).
            .child(
                div()
                    .id("middle-row")
                    .flex_1()
                    .flex()
                    .flex_row()
                    .min_h(px(0.))
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                        this.close_menu(cx);
                        this.set_ink_popup(None, cx);
                        if this.md_heading_dropdown_open {
                            this.md_heading_dropdown_open = false;
                            cx.notify();
                        }
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .min_h(px(0.))
                            .child(Toolbar { root: root.clone() })
                            .child(self.render_preview(
                                root.clone(),
                                wrap_cache,
                                self.focus.clone(),
                                &geo,
                                window,
                                cx,
                            )),
                    ),
            )
            .child(BottomBar { root: root.clone() })
            .when_some(sidebar_dock, |root, dock| {
                root.child(
                    div()
                        .absolute()
                        .top(px(0.))
                        .bottom(px(0.))
                        .left(px(0.))
                        .w(px(sidebar_width))
                        .bg(rgba(t.bg_primary))
                        .child(dock),
                )
            })
            .child(
                div()
                    .absolute()
                    .top(px(0.))
                    .left(px(0.))
                    .right(px(0.))
                    .h(px(TOP_BAR_HEIGHT))
                    .child(TopBar {
                        root: root.clone(),
                        menu_open: self.menu_open,
                        icon_animation: self.icon_animation,
                        tabs: self.tabs.clone(),
                        active: self.active_tab,
                        hovered: self.hovered_tab,
                        dirty: self.dirty_tabs.clone(),
                        can_undo,
                        can_redo,
                        sidebar_progress: self.left_sidebar_progress,
                        sidebar_width,
                    }),
            )
            // Menu overlay, painted last so it sits above the middle row:
            // click-away catcher first, dropdown panel after.
            .when(self.menu_open, |d| {
                let shell = root.clone();
                d.child(
                    div()
                        .absolute()
                        .top(px(TOP_BAR_HEIGHT))
                        .bottom(px(0.))
                        .left(px(0.))
                        .right(px(0.))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            let _ = shell.update(cx, |root, cx| root.close_menu(cx));
                        }),
                )
                .child(AppMenu {
                    root: root.clone(),
                    active: self.active_menu,
                    animation: self.menu_animation,
                })
            })
            .when_some(self.render_image_context_menu(root.clone(), window, cx), |d, menu| {
                d.child(deferred(menu).with_priority(5))
            })
            .when(self.close_confirm.is_some(), |d| {
                let shell = root.clone();
                let shell_save = shell.clone();
                let shell_discard = shell.clone();
                let shell_cancel = shell.clone();
                let path = self.close_confirm.and_then(|id| self.tabs.iter().find(|t| t.id == id).and_then(|t| t.path.clone())).unwrap_or_default();
                let file_name = std::path::Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(path.clone());
                let id = self.close_confirm.unwrap();
                d.child(deferred(
                    div()
                        .absolute()
                        .top(px(0.)).bottom(px(0.)).left(px(0.)).right(px(0.))
                        .bg(rgba(0x00000066))
                        .flex().items_center().justify_center()
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            let _ = shell.update(cx, |root, cx| {
                                root.close_confirm = None;
                                cx.notify();
                            });
                        })
                        .child(
                            div()
                                .w(px(420.)).rounded(px(20.))
                                .bg(rgba(t.bg_primary)).border_1().border_color(rgba(t.chrome_border_color))
                                .p(px(10.)).flex().flex_col().gap(px(8.))
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .child(div().child(format!("Do you want to save changes to \"{}\"?", file_name)))
                                .child(
                                    div().flex().flex_row().gap(px(8.))
                                        .child(modal_button(t, "Save", true, move |cx| {
                                            let _ = shell_save.update(cx, |root, cx| {
                                                if let Some(i) = root.tabs.iter().position(|t| t.id == id) { root.active_tab = i; }
                                                root.save_to(None, cx);
                                                root.close_confirm = None;
                                                root.close_tab_force(id, cx);
                                            });
                                        }))
                                        .child(modal_button(t, "Don't Save", false, move |cx| {
                                            let _ = shell_discard.update(cx, |root, cx| {
                                                root.close_confirm = None;
                                                root.close_tab_force(id, cx);
                                            });
                                        }))
                                        .child(modal_button(t, "Cancel", false, move |cx| {
                                            let _ = shell_cancel.update(cx, |root, cx| {
                                                root.close_confirm = None;
                                                cx.notify();
                                            });
                                        })),
                                ),
                        ),
                ).with_priority(10000))
            })
            .when(self.settings_open, |d| {
                let shell = root.clone();
                let shell_click = shell.clone();
                let shell_close = shell.clone();
                let shell_light = shell.clone();
                let shell_dark = shell.clone();
                let wrap_text = self.wrap_text_files;
                let wrap_code = self.wrap_code_files;
                let line_numbers = self.line_numbers;
                let syntax_highlighting = self.syntax_highlighting;
                let is_dark = crate::theme::mode(cx) == ThemeMode::Dark;
                let tab = self.settings_tab;
                let section_title = if tab == 0 { "General" } else { "Formatting" };
                d.child(
                    deferred(
                        div()
                            .absolute()
                            .top(px(0.))
                            .bottom(px(0.))
                            .left(px(0.))
                            .right(px(0.))
                            .bg(rgba(0x00000055))
                            .flex()
                            .items_center()
                            .justify_center()
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                let _ = shell_click.update(cx, |root, cx| {
                                    root.settings_open = false;
                                    cx.notify();
                                });
                            })
                            .child(
                                div()
                                    .w(px(620.))
                                    .h(px(410.))
                                    .rounded(px(12.))
                                    .bg(rgba(t.bg_primary))
                                    .border_1()
                                    .border_color(rgba(t.component_border_color))
                                    .shadow(vec![t.shadow_lg()])
                                    .flex()
                                    .flex_row()
                                    .overflow_hidden()
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                    .child(
                                        div()
                                            .w(px(168.))
                                            .h_full()
                                            .bg(rgba(t.bg_secondary))
                                            .border_r_1()
                                            .border_color(rgba(t.chrome_border_color))
                                            .flex()
                                            .flex_col()
                                            .p(px(12.))
                                            .gap(px(4.))
                                            .child(
                                                div()
                                                    .px(px(8.))
                                                    .pb(px(6.))
                                                    .text_xs()
                                                    .font_weight(WEIGHT_MEDIUM)
                                                    .text_color(rgba(t.empty_text_secondary))
                                                    .child("Settings"),
                                            )
                                            .child(settings_tab_button(t, shell.clone(), 0, "General", tab == 0))
                                            .child(settings_tab_button(t, shell.clone(), 1, "Formatting", tab == 1)),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .h_full()
                                            .flex()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .px(px(20.))
                                                    .py(px(14.))
                                                    .border_b_1()
                                                    .border_color(rgba(t.chrome_border_color))
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .justify_between()
                                                    .child(
                                                        div()
                                                            .text_base()
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_color(rgba(t.text_primary))
                                                            .child(section_title),
                                                    )
                                                    .child(
                                                        div()
                                                            .id("settings-close")
                                                            .w(px(24.))
                                                            .h(px(24.))
                                                            .rounded(px(6.))
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .cursor_pointer()
                                                            .text_sm()
                                                            .text_color(rgba(t.text_secondary))
                                                            .hover(|s| s.bg(rgba(t.bg_darker)).text_color(rgba(t.text_primary)))
                                                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                                                let _ = shell_close.update(cx, |root, cx| {
                                                                    root.settings_open = false;
                                                                    cx.notify();
                                                                });
                                                            })
                                                            .child("âœ•"),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .px(px(20.))
                                                    .py(px(14.))
                                                    .flex()
                                                    .flex_col()
                                                    .when(tab == 0, |c| {
                                                        c.child(
                                                            div()
                                                                .py(px(12.))
                                                                .border_b_1()
                                                                .border_color(rgba(t.border_color))
                                                                .flex()
                                                                .flex_row()
                                                                .items_center()
                                                                .justify_between()
                                                                .gap(px(16.))
                                                                .child(
                                                                    div()
                                                                        .flex()
                                                                        .flex_col()
                                                                        .gap(px(2.))
                                                                        .child(
                                                                            div()
                                                                                .text_sm()
                                                                                .font_weight(WEIGHT_MEDIUM)
                                                                                .text_color(rgba(t.text_primary))
                                                                                .child("Appearance"),
                                                                        )
                                                                        .child(
                                                                            div()
                                                                                .text_xs()
                                                                                .text_color(rgba(t.text_secondary))
                                                                                .child("Switch between Paper Light and Paper Dark."),
                                                                        ),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .p(px(2.))
                                                                        .rounded(px(8.))
                                                                        .bg(rgba(t.bg_secondary))
                                                                        .border_1()
                                                                        .border_color(rgba(t.border_color))
                                                                        .flex()
                                                                        .flex_row()
                                                                        .gap(px(2.))
                                                                        .child(
                                                                            div()
                                                                                .id("settings-theme-light")
                                                                                .px(px(12.))
                                                                                .py(px(4.))
                                                                                .rounded(px(6.))
                                                                                .cursor_pointer()
                                                                                .text_xs()
                                                                                .font_weight(WEIGHT_MEDIUM)
                                                                                .bg(rgba(if !is_dark { t.bg_primary } else { 0x00000000 }))
                                                                                .text_color(rgba(if !is_dark { t.text_primary } else { t.text_secondary }))
                                                                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                                                                    crate::theme::state::set_mode(ThemeMode::Light, cx);
                                                                                    let _ = shell_light.update(cx, |root, cx| {
                                                                                        root.store.set("theme.mode", "light");
                                                                                        cx.notify();
                                                                                    });
                                                                                })
                                                                                .child("Light"),
                                                                        )
                                                                        .child(
                                                                            div()
                                                                                .id("settings-theme-dark")
                                                                                .px(px(12.))
                                                                                .py(px(4.))
                                                                                .rounded(px(6.))
                                                                                .cursor_pointer()
                                                                                .text_xs()
                                                                                .font_weight(WEIGHT_MEDIUM)
                                                                                .bg(rgba(if is_dark { t.bg_primary } else { 0x00000000 }))
                                                                                .text_color(rgba(if is_dark { t.text_primary } else { t.text_secondary }))
                                                                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                                                                    crate::theme::state::set_mode(ThemeMode::Dark, cx);
                                                                                    let _ = shell_dark.update(cx, |root, cx| {
                                                                                        root.store.set("theme.mode", "dark");
                                                                                        cx.notify();
                                                                                    });
                                                                                })
                                                                                .child("Dark"),
                                                                        ),
                                                                ),
                                                        )
                                                        .child(settings_toggle(
                                                            t,
                                                            shell.clone(),
                                                            "Windows \"Open with\"",
                                                            "Register Peeklet for supported files in Explorer and Default Apps.",
                                                            self.windows_open_with,
                                                            "windows_open_with",
                                                            false,
                                                        ))
                                                    })
                                                    .when(tab == 1, |c| {
                                                        c.child(settings_toggle(
                                                            t,
                                                            shell.clone(),
                                                            "Wrap text files",
                                                            "Wrap long lines in plain text and Markdown documents.",
                                                            wrap_text,
                                                            "wrap_text",
                                                            true,
                                                        ))
                                                        .child(settings_toggle(
                                                            t,
                                                            shell.clone(),
                                                            "Wrap code files",
                                                            "Wrap long lines in source code instead of scrolling horizontally.",
                                                            wrap_code,
                                                            "wrap_code",
                                                            true,
                                                        ))
                                                        .child(settings_toggle(
                                                            t,
                                                            shell.clone(),
                                                            "Line numbers",
                                                            "Show the line number gutter beside source code.",
                                                            line_numbers,
                                                            "line_numbers",
                                                            true,
                                                        ))
                                                        .child(settings_toggle(
                                                            t,
                                                            shell.clone(),
                                                            "Syntax highlighting",
                                                            "Colorize keywords, strings, and comments in code files.",
                                                            syntax_highlighting,
                                                            "syntax_highlighting",
                                                            false,
                                                        ))
                                                    }),
                                            ),
                                    ),
                            ),
                    )
                    .with_priority(10000),
                )
            })
    }
}

impl Drop for RootView {
    fn drop(&mut self) {
        for id in self.dirty_tabs.clone() {
            if let Some(tab) = self.tabs.iter().find(|t| t.id == id) {
                if let Some(path) = &tab.path {
                    // Only text buffers can be restored later; image
                    // annotations are dropped with the session.
                    if viewer_kind(Some(path)) == ViewerKind::Image {
                        self.store.set(&format!("dirty:{}", path), "0");
                        continue;
                    }
                    if let Some(ed) = self.editors.get(&id) {
                        self.store.set(&format!("buffer:{}", path), &ed.text_string());
                        self.store.set(&format!("dirty:{}", path), "1");
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_geometry_reserves_gutter_width() {
        let geo = preview_geometry(1000.0, 0.0, 0.0, false, LINE_NUMBER_GUTTER_WIDTH);
        assert_eq!(geo.margin, LINE_NUMBER_GUTTER_WIDTH);
        assert!(preview_text_left(&geo) - LINE_NUMBER_GUTTER_WIDTH >= 0.0);
    }

}

