//! Root view: top bar, middle row (left dock + tab bar/preview +
//! right dock), bottom bar, menu overlay. Owns tabs, dock layout, menus,
//! hover tweens, and drag-resize state.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use gpui::{
    App, ClipboardItem, Context, FocusHandle, Font, FontFeatures, FontStyle, FontWeight, Hsla,
    IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, Point, Render, ScrollDelta,
    ScrollWheelEvent, ShapedLine, TextRun, WeakEntity, Window, div, prelude::*, px, rgba, svg,
};

use super::chrome::{BOTTOM_BAR_HEIGHT, BottomBar, TAB_BAR_HEIGHT, TOP_BAR_HEIGHT, TabBar, TopBar};
use super::dock::{DOCK_DEFAULT_WIDTH, DockView};
use super::dropdown::AppMenu;
use super::panels::{DockSide, PanelKind};
use crate::editor::EditorState;
use crate::persist::{Store, TabRecord};
use crate::theme::WEIGHT_MEDIUM;

/// Plus-in-document icon for New File.
const ICON_NEW_FILE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<g fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5">
		<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
		<path d="M14 2v6h6" />
		<path d="M12 18v-6" />
		<path d="M9 15h6" />
	</g>
</svg>"#;

/// Folder icon for Open File.
const ICON_OPEN_FILE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<g fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5">
		<path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
	</g>
</svg>"#;

#[derive(Clone)]
pub struct Tab {
    pub id: usize,
    pub title: String,
    /// Backing file path, if any. Write-through keeps it in sync.
    pub path: Option<String>,
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
    /// Margin on both sides of the text column.
    margin: f32,
    /// Text width; lines wrap to this.
    measure: f32,
}

fn preview_geometry(view_w: f32, left_occ: f32, right_occ: f32) -> PreviewGeometry {
    let col = (view_w - left_occ - right_occ).max(0.0);
    let measure = (col - 2.0 * MIN_COLUMN_MARGIN).clamp(0.0, MAX_TEXT_WIDTH);
    let margin = ((col - measure) / 2.0).max(0.0);
    PreviewGeometry { col_w: col, margin, measure }
}

fn preview_text_left(geo: &PreviewGeometry) -> f32 {
    geo.margin
}

/// Render-ready snapshot of the active buffer. Keeps borrows short: the
/// entity builds this once, then renders from plain data.
#[derive(Clone)]
struct SnapshotSeg {
    text: String,
    size: f32,
    weight: FontWeight,
    mono: bool,
    color: u32,
    src_start: usize,
    src_end: usize,
}

struct BuiltRow {
    segs: Vec<SnapshotSeg>,
}

struct SnapshotRow {
    segs: Vec<SnapshotSeg>,
    /// Caret (x, height) within the row, if the cursor is here.
    caret: Option<(f32, f32)>,
    /// Selection x-range (px) within the row, if any overlaps it.
    sel_px: Option<(f32, f32)>,
}

struct EditorSnapshot {
    rows: Vec<SnapshotRow>,
    can_undo: bool,
    can_redo: bool,
    has_selection: bool,
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

fn locate_src(shaped: &[(SnapshotSeg, ShapedLine, f32)], src: usize) -> (f32, f32) {
    let mut x = 0.0;
    for (seg, sh, w) in shaped {
        if src < seg.src_start { return (x, seg.size); }
        if src <= seg.src_end {
            let rel = (src - seg.src_start).min(seg.text.len());
            return (x + sh.split_at(rel).0.width().as_f32(), seg.size);
        }
        x += w;
    }
    let size = shaped.last().map(|(s, _, _)| s.size).unwrap_or(PREVIEW_FONT_SIZE);
    (x, size)
}

/// Lay out one buffer line into rows: wrap the raw text to the measure and
/// slice it into plain segments. No markup is interpreted — a preview shows
/// the file exactly as it is stored.
fn layout_line(
    line_text: &str,
    measure: f32,
    window: &mut Window,
    t: crate::theme::Theme,
) -> Vec<BuiltRow> {
    let color = Hsla::from(rgba(t.text_primary));
    let mut width_of = |s: &str| {
        measure_text(window, s, color, PREVIEW_FONT_SIZE, FontWeight::NORMAL, false)
    };
    crate::editor::wrap_line(0, line_text, measure, &mut width_of)
        .into_iter()
        .map(|row| {
            let text = &line_text[row.start_byte..row.end_byte];
            BuiltRow {
                segs: vec![SnapshotSeg {
                    text: text.to_string(),
                    size: PREVIEW_FONT_SIZE,
                    weight: FontWeight::NORMAL,
                    mono: false,
                    color: t.text_primary,
                    src_start: row.start_byte,
                    src_end: row.end_byte,
                }],
            }
        })
        .collect()
}

pub struct RootView {
    tabs: Vec<Tab>,
    active_tab: usize,
    next_tab_id: usize,
    visible_left: Option<PanelKind>,
    visible_right: Option<PanelKind>,
    left_width: f32,
    right_width: f32,
    /// Active dock drag: (side being resized, last cursor x, window width).
    /// Widths are unclamped; the only rule is the docks never overlap.
    drag: Option<(DockSide, f32, f32)>,
    recent_files: Vec<String>,
    project_root: Option<std::path::PathBuf>,
    project_expanded: HashSet<std::path::PathBuf>,
    hovered_tab: Option<usize>,
    store: Store,
    /// Buffers keyed by tab id, created lazily.
    editors: HashMap<usize, EditorState>,
    /// Preview focus follows the active tab.
    pub focus: FocusHandle,
    /// Caret blink state, toggled on a timer.
    caret_visible: bool,
    blink_started: bool,
    /// Vertical scroll offset (px) of the preview viewport.
    scroll_offset: f32,
    /// Scrollbar drag: (pointer start y, scroll offset at start).
    scroll_drag: Option<(f32, f32)>,
    /// Number of rendered preview rows from the most recent frame.
    scroll_content_rows: usize,
    scrollbar_hovered: bool,
    /// True while the left button is held for mouse selection.
    mouse_select: bool,
    // -- hamburger menu state (Parametric behavior) --
    pub menu_open: bool,
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
        for record in store.tabs() {
            if let Some(path) = record.path.as_ref() {
                let title = std::path::Path::new(path)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.to_string());
                let content = std::fs::read_to_string(path).unwrap_or_default();
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

        let recent_files: Vec<String> = store
            .get("recents")
            .map(|v| v.split('\n').filter(|s| !s.is_empty()).map(str::to_string).collect())
            .unwrap_or_default();
        let project_root = None;
        let project_expanded = HashSet::new();

        let mut view = Self {
            tabs,
            active_tab,
            next_tab_id,
            visible_left: visible("panels.left.visible"),
            visible_right: visible("panels.right.visible"),
            left_width: width("panels.left.width", DOCK_DEFAULT_WIDTH),
            right_width: width("panels.right.width", DOCK_DEFAULT_WIDTH),
            drag: None,
            recent_files,
            project_root,
            project_expanded,
            hovered_tab: None,
            store,
            editors: HashMap::new(),
            focus: cx.focus_handle(),
            caret_visible: true,
            blink_started: false,
            scroll_offset: 0.0,
            scroll_drag: None,
            scroll_content_rows: 0,
            scrollbar_hovered: false,
            mouse_select: false,
            menu_open: false,
            active_menu: None,
            menu_animation: 0.0,
            icon_animation: 0.0,
            hovered_entry: None,
            fades: HashMap::new(),
            fade_pending: HashMap::new(),
            fade_tween_active: HashSet::new(),
            cursor_trail: Vec::new(),
        };
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
            PanelKind::Files => DockSide::Left,
        }
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
            self.save_layout();
            cx.notify();
        }
    }

    pub fn close_tab(&mut self, id: usize, cx: &mut Context<Self>) {
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
    /// Open a new empty tab. Nothing touches the disk until the tab is saved.
    pub fn new_tab(&mut self, cx: &mut Context<Self>) {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        self.tabs.push(Tab { id, title: "Untitled".into(), path: None });
        self.active_tab = self.tabs.len() - 1;
        self.save_layout();
        cx.notify();
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

    fn dock_occupancy(&self) -> (f32, f32) {
        (
            self.visible_left.map(|_| self.left_width).unwrap_or(0.0),
            self.visible_right.map(|_| self.right_width).unwrap_or(0.0),
        )
    }

    fn geometry(&self, window: &mut Window) -> PreviewGeometry {
        let view_w = window.viewport_size().width.as_f32();
        let (left_occ, right_occ) = self.dock_occupancy();
        preview_geometry(view_w, left_occ, right_occ)
    }

    // ----- undo / redo (active tab's buffer) -----

    fn active_editor_mut(&mut self) -> Option<&mut EditorState> {
        let tab = self.tabs.get(self.active_tab)?;
        if tab.title == "Welcome" {
            return None;
        }
        Some(self.editors.entry(tab.id).or_insert_with(EditorState::new))
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
                "x" => {
                    if let Some(ed) = self.active_editor_mut() {
                        if let Some(text) = ed.selected_text() {
                            cx.write_to_clipboard(ClipboardItem::new_string(text));
                            ed.delete_selection();
                            self.caret_visible = true;
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
                                self.caret_visible = true;
                                cx.notify();
                            }
                        }
                    }
                    return;
                }
                "a" => {
                    if let Some(ed) = self.active_editor_mut() {
                        ed.select_all();
                        cx.notify();
                    }
                    return;
                }
                _ => return,
            }
        }
        let Some(ed) = self.active_editor_mut() else {
            return;
        };
        let key = ev.keystroke.key.as_str();
        let clean = !mods.control && !mods.platform && !mods.alt;
        let extend = mods.shift;
        match key {
            "left" if clean => ed.move_left(extend),
            "right" if clean => ed.move_right(extend),
            "home" if clean => ed.line_home(extend),
            "end" if clean => ed.line_end(extend),
            "up" if clean => self.move_visual(-1, extend, window, cx),
            "down" if clean => self.move_visual(1, extend, window, cx),
            "backspace" if clean => ed.backspace(),
            "delete" if clean => ed.delete_forward(),
            "enter" if clean => ed.insert_str("\n"),
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
                        ed.insert_str(&filtered);
                    }
                    None => return,
                }
            }
        }
        self.caret_visible = true;
        // Write-through: every keystroke lands on disk immediately.
        self.save_active();
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
        // Caret x on the current row, then the widest prefix fitting it.
        let rel = cur_byte.min(cur_row.end_byte).saturating_sub(cur_row.start_byte);
        let row_text = ed.line_text(cur_row.buffer_line);
        let row_slice = &row_text[cur_row.start_byte..cur_row.end_byte.min(row_text.len())];
        let prefix = &row_slice[..rel.min(row_slice.len())];
        let x = measure_text(window, prefix, color, size, weight, false);
        let target_text = ed.line_text(target_row.buffer_line);
        let target_slice =
            &target_text[target_row.start_byte..target_row.end_byte.min(target_text.len())];
        let lo = crate::editor::byte_for_x(target_slice, x, &mut |s: &str| {
            measure_text(window, s, color, target_size, target_weight, false)
        });
        ed.set_cursor_line_byte(target_row.buffer_line, target_row.start_byte + lo, extend);
    }

    /// Wheel scrolling over the preview viewport.
    fn handle_wheel(&mut self, ev: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
        let dy = match ev.delta {
            ScrollDelta::Pixels(p) => p.y.as_f32(),
            ScrollDelta::Lines(p) => p.y * PREVIEW_LINE_HEIGHT,
        };
        if dy == 0.0 {
            return;
        }
        let max = self.scroll_max_for_geometry(Self::viewport_h(window), 0.0);
        self.scroll_offset = (self.scroll_offset - dy).clamp(0.0, max);
        cx.notify();
    }

    fn begin_scroll_drag(&mut self, pointer_y: f32, thumb_top: f32, thumb_height: f32, track_h: f32, view_h: f32, cx: &mut Context<Self>) {
        let track_y = TOP_BAR_HEIGHT + TAB_BAR_HEIGHT;
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
        let y_view = y_win - (TOP_BAR_HEIGHT + TAB_BAR_HEIGHT);
        let view_h = Self::viewport_h(window);
        let ed = self.editors.entry(tab_id).or_insert_with(EditorState::new);
        // Rows carry their line's type for measurement.
        let mut rows: Vec<(crate::editor::VisualRow, f32, FontWeight)> = Vec::new();
        for line_no in 0..ed.len_lines() {
            let text = ed.line_text(line_no);
            let size = PREVIEW_FONT_SIZE;
            let weight = FontWeight::NORMAL;
            let mut width_of = |s: &str| measure_text(window, s, color, size, weight, false);
            rows.extend(
                crate::editor::wrap_line(line_no, &text, geo.measure, &mut width_of)
                    .into_iter()
                    .map(|r| (r, size, weight)),
            );
        }
        if rows.is_empty() {
            return;
        }
        let total_h = rows.len() as f32 * PREVIEW_LINE_HEIGHT;
        let max = (total_h - view_h).max(0.0);
        let offset = self.scroll_offset.min(max);
        let content_y = y_view + offset;
        let idx = ((content_y - PREVIEW_TOP_PAD) / PREVIEW_LINE_HEIGHT)
            .floor()
            .max(0.0) as usize;
        let idx = idx.min(rows.len() - 1);
        let (row, size, weight) = rows[idx].clone();
        // Keep the left edge visible when the column is wider than the
        // viewport: margins are consumed before the text itself is clipped.
        let text_left = preview_text_left(&geo);
        let x_text = x_col - text_left;
        let row_text = ed.line_text(row.buffer_line);
        let row_slice = &row_text[row.start_byte..row.end_byte.min(row_text.len())];
        let byte = crate::editor::byte_for_x(row_slice, x_text, &mut |s: &str| {
            measure_text(window, s, color, size, weight, false)
        });
        ed.set_cursor_line_byte(row.buffer_line, row.start_byte + byte, extend);
        self.caret_visible = true;
        cx.notify();
    }

    pub fn undo(&mut self, cx: &mut Context<Self>) {
        if let Some(ed) = self.active_editor_mut() {
            ed.undo();
        }
        self.save_active();
        cx.notify();
    }

    pub fn redo(&mut self, cx: &mut Context<Self>) {
        if let Some(ed) = self.active_editor_mut() {
            ed.redo();
        }
        self.save_active();
        cx.notify();
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

    // ----- panels -----

    pub fn toggle_panel(&mut self, kind: PanelKind, cx: &mut Context<Self>) {
        match Self::dock_of(kind) {
            DockSide::Left => {
                self.visible_left = if self.visible_left == Some(kind) { None } else { Some(kind) };
            }
            DockSide::Right => {
                self.visible_right =
                    if self.visible_right == Some(kind) { None } else { Some(kind) };
            }
        }
        self.save_layout();
        cx.notify();
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

    /// Open a path in a new tab, loading its content into the buffer.
    pub fn open_file_path(&mut self, path: String, cx: &mut Context<Self>) {
        let title = std::path::Path::new(&path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone());
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        self.recent_files.retain(|p| p != &path);
        self.recent_files.insert(0, path.clone());
        self.recent_files.truncate(12);
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        self.tabs.push(Tab { id, title, path: Some(path) });
        self.active_tab = self.tabs.len() - 1;
        self.editors
            .entry(id)
            .or_insert_with(EditorState::new)
            .set_text(&content);
        self.scroll_offset = 0.0;
        self.save_layout();
        cx.notify();
    }

    /// Write the active tab's buffer to its file. No-op without a path.
    fn save_active(&self) {
        let Some(tab) = self.tabs.get(self.active_tab) else {
            return;
        };
        let Some(path) = tab.path.as_ref() else {
            return;
        };
        if let Some(ed) = self.editors.get(&tab.id) {
            let _ = std::fs::write(path, ed.text_string());
        }
    }

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
        self.save_active();
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

    fn end_drag(&mut self, cx: &mut Context<Self>) {
        let was_dragging = self.drag.take().is_some() || self.scroll_drag.take().is_some();
        self.mouse_select = false;
        if was_dragging {
            self.save_layout();
            cx.notify();
        }
    }

    /// Viewport height: window minus top bar, tab bar, bottom bar.
    fn viewport_h(window: &mut Window) -> f32 {
        (window.viewport_size().height.as_f32()
            - TOP_BAR_HEIGHT
            - TAB_BAR_HEIGHT
            - BOTTOM_BAR_HEIGHT)
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
        &self,
        root: WeakEntity<RootView>,
        snapshot: Option<EditorSnapshot>,
        focus: FocusHandle,
        geo: &PreviewGeometry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let title = self.tabs.get(self.active_tab).map(|t| t.title.clone()).unwrap_or_default();
        if title == "Welcome" {
            return self.render_welcome(root, cx);
        }
        let t = *crate::theme::active(cx);
        let snap = snapshot.unwrap_or(EditorSnapshot {
            rows: vec![SnapshotRow {
                segs: vec![SnapshotSeg {
                    text: String::new(), size: PREVIEW_FONT_SIZE,
                    weight: FontWeight::NORMAL, mono: false,
                    color: t.text_primary,
                    src_start: 0, src_end: 0,
                }],
                caret: Some((0.0, CARET_HEIGHT)),
                sel_px: None,
            }],
            can_undo: false,
            can_redo: false,
            has_selection: false,
        });
        // No caret while any selection is visible.
        let show_caret = self.caret_visible && !snap.has_selection;

        // Scrollbar geometry (Zed-style: thick low-opacity thumb + a 2px
        // accent line at the cursor row).
        let view_h = Self::viewport_h(window);
        let total_h = snap.rows.len() as f32 * PREVIEW_LINE_HEIGHT;
        let offset = self.scroll_offset.min((total_h - view_h).max(0.0));
        let show_scroll = total_h > view_h;
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
        let cursor_y = if show_scroll && total_h > 0.0 {
            let idx = snap.rows.iter().position(|r| r.caret.is_some()).unwrap_or(0);
            ((idx as f32 * PREVIEW_LINE_HEIGHT + PREVIEW_LINE_HEIGHT / 2.0) / total_h)
                * track_h
        } else {
            0.0
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
                                root.mouse_select = true;
                                root.position_from_point(
                                    ev.position.x.as_f32(),
                                    ev.position.y.as_f32(),
                                    window,
                                    ev.modifiers.shift,
                                    cx,
                                );
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
                            .top(px(-offset))
                            .flex()
                            .flex_col()
                            .items_start()
                            .child(
                                div()
                                    .w(px(geo.measure))
                                    .ml(px(preview_text_left(geo)))
                                    .flex()
                                    .flex_col()
                                    .pt(px(PREVIEW_TOP_PAD))
                                    .children(snap.rows.iter().map(|row| {
                                        // The text always renders as one
                                        // unsplit run, so shaping (kerning,
                                        // ligatures) never shifts when the
                                        // selection or caret moves.
                                        // Selection and caret are overlays.
                                        div()
                                            .relative()
                                            .w_full()
                                            .h(px(PREVIEW_LINE_HEIGHT))
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .whitespace_nowrap()
                                            .overflow_hidden()
                                            .children(row.segs.iter().map(|seg| {
                                                div()
                                                    .font_family(if seg.mono { crate::theme::FONT_MONO } else { crate::theme::FONT_TEXT })
                                                    .text_size(px(seg.size))
                                                    .font_weight(seg.weight)
                                                    .text_color(rgba(seg.color))
                                                    .child(seg.text.clone())
                                            }))
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
                                                            .w(px(2.))
                                                            .h(px(h))
                                                            .bg(rgba(t.cursor)),
                                                    )
                                                },
                                            )
                                    })
                                )
                            )
                )
                .when(show_scroll, |v| {
                    v.child(
                        div()
                            .id("manuscript-scrollbar")
                            .absolute()
                            .right(px(0.))
                            .top(px(0.))
                            .bottom(px(0.))
                            .w(px(8.))
                            .cursor_pointer()
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
            )
            .into_any_element()
    }

    /// Zed-style welcome tab: big centered greeting, then a narrow
    /// left-aligned column of Get Started actions and recent files.
    fn render_welcome(&self, root: WeakEntity<RootView>, cx: &mut App) -> gpui::AnyElement {
        let t = *crate::theme::active(cx);

        // Uppercase section label in mono, with a 1px rule taking the
        // remaining width, vertically centered with the text.
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
                .child(div().flex_1().h(px(1.)).bg(rgba(t.chrome_border_color)))
        };

        let action_button = |id: &'static str, icon: &'static [u8], label: &'static str, cmd: u8| {
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
                .hover(|s| s.bg(rgba(t.menu_hover_bg)))
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    if cmd == 0 {
                        let _ = root.update(cx, |root, cx| root.new_tab(cx));
                    } else {
                        let root = root.clone();
                        window.defer(cx, move |_, cx| {
                            if let Some(path) = rfd::FileDialog::new().pick_file() {
                                let _ = root.update(cx, |root, cx| {
                                    root.open_file_path(path.to_string_lossy().into_owned(), cx);
                                });
                            }
                        });
                    }
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
            .bg(rgba(t.bg_primary))
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
                    .child(section("GET STARTED"))
                    .child(action_button("welcome-new", ICON_NEW_FILE, "New Tab", 0))
                    .child(action_button("welcome-open", ICON_OPEN_FILE, "Open File…", 1))
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
                                    .hover(|s| s.bg(rgba(t.menu_hover_bg)))
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

impl Render for RootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = *crate::theme::active(cx);
        let root = cx.entity().downgrade();

        // Caret blink ticker, started once for the entity's lifetime.
        if !self.blink_started {
            self.blink_started = true;
            cx.spawn(async move |weak, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(530))
                        .await;
                    let alive = weak
                        .update(cx, |root, cx| {
                            root.caret_visible = !root.caret_visible;
                            cx.notify();
                        })
                        .is_ok();
                    if !alive {
                        break;
                    }
                }
            })
            .detach();
        }

        let left_dock = self.visible_left.map(|kind| DockView {
            root: root.clone(),
            side: DockSide::Left,
            visible: kind,
            width: self.left_width,
        });
        let right_dock = self.visible_right.map(|kind| DockView {
            root: root.clone(),
            side: DockSide::Right,
            visible: kind,
            width: self.right_width,
        });

        // Geometry first: everything preview-related derives from it.
        let geo = self.geometry(window);

        // Snapshot the active buffer once per frame: wrapped visual rows
        // with caret/selection x-edges and history flags. Welcome tabs have
        // no buffer.
        let snapshot: Option<EditorSnapshot> = self
            .tabs
            .get(self.active_tab)
            .filter(|tab| tab.title != "Welcome")
            .map(|tab| tab.id)
            .map(|id| {
                let ed = self.editors.entry(id).or_insert_with(EditorState::new);
                let (cur_line, cur_byte) = ed.cursor_line_byte();
                let sel_chars = ed.selection_range();
                let mut rows = Vec::new();
                let mut caret_placed = false;
                for line_no in 0..ed.len_lines() {
                    let text = ed.line_text(line_no);
                    let ls = ed.line_start_char(line_no);
                    let llen = ed.line_char_len(line_no);
                    let sel_bytes = sel_chars.and_then(|(a, b)| {
                        let o0 = a.max(ls);
                        let o1 = b.min(ls + llen);
                        if o0 >= o1 {
                            None
                        } else {
                            Some((
                                EditorState::col_to_bytes(&text, o0 - ls),
                                EditorState::col_to_bytes(&text, o1 - ls),
                            ))
                        }
                    });
                    for built in layout_line(&text, geo.measure, window, t) {
                        let mut shaped = Vec::new();
                        for seg in &built.segs {
                            let color = Hsla::from(rgba(seg.color));
                            let sh = shape_row(window, &seg.text, color, seg.size, seg.weight, seg.mono);
                            let w = sh.width().as_f32();
                            shaped.push((seg.clone(), sh, w));
                        }
                        let row_contains_cursor = if let (Some(first), Some(last)) =
                            (built.segs.first(), built.segs.last())
                        {
                            first.src_start <= cur_byte && cur_byte <= last.src_end
                        } else {
                            cur_byte == 0
                        };
                        let caret = if !caret_placed
                            && line_no == cur_line
                            && row_contains_cursor
                        {
                            let (x, size) = locate_src(&shaped, cur_byte);
                            caret_placed = true;
                            Some((x, (size + 4.0).min(PREVIEW_LINE_HEIGHT - 2.0)))
                        } else { None };
                        let sel_px = sel_bytes.and_then(|(s0, s1)| {
                            let mut x0 = None;
                            let mut x1 = None;
                            let mut x = 0.0;
                            for (seg, sh, w) in &shaped {
                                let a = s0.max(seg.src_start);
                                let b = s1.min(seg.src_end);
                                if a < b {
                                    let local_a = sh.split_at(a - seg.src_start).0.width().as_f32();
                                    let local_b = sh.split_at(b - seg.src_start).0.width().as_f32();
                                    x0 = Some(x0.unwrap_or(x + local_a));
                                    x1 = Some(x + local_b);
                                }
                                x += *w;
                            }
                            x0.zip(x1)
                        });
                        rows.push(SnapshotRow { segs: built.segs, caret, sel_px });
                    }
                }
                EditorSnapshot {
                    rows,
                    can_undo: ed.can_undo(),
                    can_redo: ed.can_redo(),
                    has_selection: ed.selection_range().is_some(),
                }
            });
        let can_undo = snapshot.as_ref().is_some_and(|s| s.can_undo);
        let can_redo = snapshot.as_ref().is_some_and(|s| s.can_redo);
        self.scroll_content_rows = snapshot.as_ref().map(|s| s.rows.len()).unwrap_or(0);

        div()
            .id("root")
            .size_full()
            .flex()
            .flex_col()
            .font_weight(FontWeight::NORMAL)
            .bg(rgba(t.bg_primary))
            .text_color(rgba(t.text_primary))
            .child(TopBar {
                root: root.clone(),
                menu_open: self.menu_open,
                icon_animation: self.icon_animation,
            })
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
                    }))
                    .on_mouse_move(cx.listener(|this, ev: &gpui::MouseMoveEvent, window, cx| {
                        this.drag_to(ev.position.x.as_f32(), cx);
                        let thumb_h = this.scrollbar_thumb_height(window);
                        let view_h = Self::viewport_h(window);
                        this.scroll_drag_to(ev.position.y.as_f32(), view_h, view_h, thumb_h, cx);
                    }))
                    .on_mouse_up(MouseButton::Left, cx.listener(|this, _: &gpui::MouseUpEvent, _, cx| {
                        this.end_drag(cx);
                    }))
                    .when_some(left_dock, |row, dock| row.child(dock))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .min_h(px(0.))
                            .child(TabBar {
                                root: root.clone(),
                                tabs: self.tabs.clone(),
                                active: self.active_tab,
                                hovered: self.hovered_tab,
                                can_undo,
                                can_redo,
                            })
                            .child(self.render_preview(
                                root.clone(),
                                snapshot,
                                self.focus.clone(),
                                &geo,
                                window,
                                cx,
                            )),
                    )
                    .when_some(right_dock, |row, dock| row.child(dock)),
            )
            .child(BottomBar { root: root.clone() })
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
    }
}
