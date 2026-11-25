//! Window chrome: top bar (hamburger menu + history + tabs + window controls), bottom bar
//! (placeholder for per-file-type controls).

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, MouseButton, Pixels, WeakEntity, Window, WindowControlArea, canvas,
    deferred, div, point, prelude::*, px, rgba, size as gpui_size, svg,
};

use super::dropdown::{ENTRY_HEIGHT, PANEL_PADDING_X, PANEL_PADDING_Y};
use super::ink;
use super::panels::{ICON_PDF, PanelKind};
use super::root::{INK_PALETTE, INK_SIZE_MIN, MarkdownViewMode, RootView, Tab};
use crate::theme::WEIGHT_MEDIUM;

pub const TOP_BAR_HEIGHT: f32 = 36.0;
pub const BOTTOM_BAR_HEIGHT: f32 = 30.0;
pub const TOOLBAR_HEIGHT: f32 = 34.0;
const TOP_BAR_BUTTON_SIZE: f32 = 24.0;
const TOP_BAR_SMALL_BUTTON_SIZE: f32 = 22.0;
const TOP_BAR_ICON_SIZE: f32 = 20.0;
const TOP_BAR_CONTROL_CLUSTER_WIDTH: f32 = 116.0;

const MENU_ICON: &[u8] =
    br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<path fill="none" stroke="currentColor" stroke-linecap="round" stroke-width="2.6" d="M9.41 7.3H9.4m5.2 0h-.01M9.31 12H9.3m5.3 0h-.01m-5.18 4.7H9.4m5.2 0h-.01" />
</svg>
"#;

#[allow(dead_code)]
const SIDEBAR_ICON: &[u8] = br#"<svg viewBox="0 0 20 20" fill="none" xmlns="http://www.w3.org/2000/svg">
<path d="M6.83496 3.99992C6.38353 4.00411 6.01421 4.0122 5.69824 4.03801C5.31232 4.06954 5.03904 4.12266 4.82227 4.20012L4.62207 4.28606C4.18264 4.50996 3.81498 4.85035 3.55859 5.26848L3.45605 5.45207C3.33013 5.69922 3.25006 6.01354 3.20801 6.52824C3.16533 7.05065 3.16504 7.71885 3.16504 8.66301V11.3271C3.16504 12.2712 3.16533 12.9394 3.20801 13.4618C3.25006 13.9766 3.33013 14.2909 3.45605 14.538L3.55859 14.7216C3.81498 15.1397 4.18266 15.4801 4.62207 15.704L4.82227 15.79C5.03904 15.8674 5.31234 15.9205 5.69824 15.9521C6.01398 15.9779 6.383 15.986 6.83398 15.9902L6.83496 3.99992ZM18.165 11.3271C18.165 12.2493 18.1653 12.9811 18.1172 13.5702C18.0745 14.0924 17.9916 14.547 17.8125 14.9648L17.7295 15.1415C17.394 15.8 16.8834 16.3511 16.2568 16.7353L15.9814 16.8896C15.5157 17.1268 15.0069 17.2285 14.4102 17.2773C13.821 17.3251 13.0893 17.3251 12.167 17.3251H7.83301C6.91071 17.3251 6.17898 17.3251 5.58984 17.2773C5.06757 17.2346 4.61294 17.1505 4.19531 16.9716L4.01855 16.8896C3.36014 16.5541 2.80898 16.0434 2.4248 15.4169L2.27051 15.1415C2.03328 14.6758 1.93158 14.167 1.88281 13.5702C1.83468 12.9811 1.83496 12.2493 1.83496 11.3271V8.66301C1.83496 7.74072 1.83468 7.00898 1.88281 6.41985C1.93157 5.82309 2.03329 5.31432 2.27051 4.84856L2.4248 4.57317C2.80898 3.94666 3.36012 3.436 4.01855 3.10051L4.19531 3.0175C4.61285 2.83843 5.06771 2.75548 5.58984 2.71281C6.17898 2.66468 6.91071 2.66496 7.83301 2.66496H12.167C13.0893 2.66496 13.821 2.66468 14.4102 2.71281C15.0069 2.76157 15.5157 2.86329 15.9814 3.10051L16.2568 3.25481C16.8833 3.63898 17.394 4.19012 17.7295 4.84856L17.8125 5.02531C17.9916 5.44285 18.0745 5.89771 18.1172 6.41985C18.1653 7.00898 18.165 7.74072 18.165 8.66301V11.3271ZM8.16406 15.995H12.167C13.1112 15.995 13.7794 15.9947 14.3018 15.9521C14.8164 15.91 15.1308 15.8299 15.3779 15.704L15.5615 15.6015C15.9797 15.3451 16.32 14.9774 16.5439 14.538L16.6299 14.3378C16.7074 14.121 16.7605 13.8478 16.792 13.4618C16.8347 12.9394 16.835 12.2712 16.835 11.3271V8.66301C16.835 7.71885 16.8347 7.05065 16.792 6.52824C16.7605 6.14232 16.7073 5.86904 16.6299 5.65227L16.5439 5.45207C16.32 5.01264 15.9796 4.64498 15.5615 4.3886L15.3779 4.28606C15.1308 4.16013 14.8165 4.08006 14.3018 4.03801C13.7794 3.99533 13.111 3.99504 12.167 3.99504H8.16406C8.16407 3.99667 8.16504 3.99829 8.16504 3.99992L8.16406 15.995Z" fill="currentColor"/>
</svg>"#;

const SIDEBAR_ICON_COMPACT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<g fill="none" stroke="currentColor" stroke-width="1.5">
		<rect width="14" height="14" x="5" y="5" rx="4" />
		<path stroke-linecap="round" d="M9 9v6" />
	</g>
</svg>"#;

const UNDO_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<path fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 12H7m0 0l3.33-3.33M7 12l3.33 3.33" />
</svg>"#;

const REDO_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<path fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 12h10m0 0l-3.33-3.33M17 12l-3.33 3.33" />
</svg>"#;

const SETTINGS_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<g fill="none" stroke="currentColor" stroke-width="2">
		<path stroke-linecap="round" stroke-linejoin="round" d="M10.3 5.98q1.7-.98 3.4 0l2.66 1.54q1.7.98 1.7 2.94v3.08q0 1.96-1.7 2.94l-2.66 1.54q-1.7.98-3.4 0l-2.66-1.54q-1.7-.98-1.7-2.94v-3.08q0-1.96 1.7-2.94Z" />
		<circle cx="12" cy="12" r="2.5" />
	</g>
</svg>"#;

const TAB_CLOSE_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<path fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="m18 6l-6 6m0 0l-6 6m6-6l6 6m-6-6L6 6" />
</svg>
"#;

const TAB_MAX_WIDTH: f32 = 220.0;

const TAB_TEXT_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M0 0h24v24H0z" fill="none"/><path fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M8 18h8m-8-4h8m-8-4h2m3-7.5V3c0 2.828 0 4.243.879 5.121C14.757 9 16.172 9 19 9h.5m.5 1.657V14c0 3.771 0 5.657-1.172 6.828S15.771 22 12 22s-5.657 0-6.828-1.172S4 17.771 4 14V9.456c0-3.245 0-4.868.886-5.967a4 4 0 0 1 .603-.603C6.59 2 8.211 2 11.456 2c.705 0 1.058 0 1.381.114q.1.036.197.082c.31.148.559.397 1.058.896l4.736 4.736c.579.578.867.868 1.02 1.235c.152.368.152.776.152 1.594"/>
</svg>"#;

const TAB_IMAGE_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M0 0h24v24H0z" fill="none"/><g fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"><circle cx="9.5" cy="12.5" r="1.5"/><path d="m7.5 21.5l6.793-6.793a2.414 2.414 0 0 1 3.414 0l2.107 2.107"/><path d="M13 2.5V3c0 2.828 0 4.243.879 5.121C14.757 9 16.172 9 19 9h.5m.5 1.657V14c0 3.771 0 5.657-1.172 6.828S15.771 22 12 22s-5.657-1.172-6.828-1.172S4 17.771 4 14V9.456c0-3.245 0-4.868.886-5.967a4 4 0 0 1 .603-.603C6.59 2 8.211 2 11.456 2c.705 0 1.058 0 1.381.114q.1.036.197.082c.31.148.559.397 1.058.896l4.736 4.736c.579.578.867.868 1.02 1.235c.152.368.152.776.152 1.594"/></g>
</svg>"#;

const TAB_PDF_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M0 0h24v24H0z" fill="none"/><g fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"><path d="M19 11c0-.818 0-1.57-.152-1.937s-.441-.657-1.02-1.235l-4.736-4.736c-.499-.499-.748-.748-1.058-.896a2 2 0 0 0-.197-.082C11.514 2 11.161 2 10.456 2c-3.245 0-4.868 0-5.967.886a4 4 0 0 0-.603.603C3 4.59 3 6.211 3 9.456V14c0 3.771 0 5.657 1.172 6.828S7.229 22 11 22h8M12 2.5V3c0 2.828 0 4.243.879 5.121C13.757 9 15.172 9 18 9h.5"/><path d="M21 14h-2a1 1 0 0 0-1 1v1.5m0 0V19m0-2.5h2.5M7 19v-2m0 0v-3h1.5a1.5 1.5 0 0 1 0 3zm5.5-3h1.286c.947 0 1.714.746 1.714 1.667v1.666c0 .92-.768 1.667-1.714 1.667H12.5z"/></g>
</svg>"#;

const TAB_AUDIO_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M0 0h24v24H0z" fill="none"/><g fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"><path d="M20 11s0-1.57-.152-1.937s-.441-.657-1.02-1.235l-4.736-4.736c-.499-.499-.748-.748-1.058-.896a2 2 0 0 0-.197-.082C12.514 2 12.161 2 11.456 2c-3.245 0-4.868 0-5.967.886a4 4 0 0 0-.603.603C4 4.59 4 6.211 4 9.456V14c0 3.771 0 5.657 1.172 6.828S8.229 22 12 22m1-19.5V3c0 2.828 0 4.243.879 5.121C14.757 9 16.172 9 19 9h.5"/><path d="M20 19.407v-2.814c0-1.572 0-2.359-.54-2.554c-.54-.196-1.175.36-2.446 1.472L16.5 16h-1.496c-.943 0-1.414 0-1.707.293s-.293.764-.293 1.707s0 1.414.293 1.707s.764.293 1.707.293H16.5l.514.489c1.27 1.112 1.906 1.668 2.446 1.472s.54-.982.54-2.554"/></g>
</svg>"#;

const TAB_VIDEO_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M0 0h24v24H0z" fill="none"/><g fill="none" stroke="currentColor" stroke-linejoin="round" stroke-width="1.5"><path stroke-linecap="round" d="M19 14.005v-3.344c0-.818 0-1.227-.152-1.595s-.441-.657-1.02-1.235l-4.736-4.739c-.499-.499-.748-.748-1.058-.896a2 2 0 0 0-.197-.082C11.514 2 11.161 2 10.456 2c-3.245 0-4.868 0-5.967.886a4 4 0 0 0-.603.604C3 4.59 3 6.213 3 9.46v4.545c0 3.773 0 5.66 1.172 6.832C5.115 21.78 6.52 21.964 9 22m3-19.5V3c0 2.83 0 4.245.879 5.124c.878.879 2.293.879 5.121.879h.5"/><path d="m18 19.5l1.445.963c.685.457 1.027.685 1.29.544c.265-.142.265-.553.265-1.376V18.37c0-.823 0-1.234-.264-1.376c-.264-.14-.606.087-1.29.544L18 18.5m0 1v-1m0 1c0 .935 0 1.402-.201 1.75a1.5 1.5 0 0 1-.549.549c-.348.201-.815.201-1.75.201H15c-1.414 0-2.121 0-2.56-.44C12 21.122 12 20.415 12 19s0-2.121.44-2.56C12.878 16 13.585 16 15 16h.5c.935 0 1.402 0 1.75.201a1.5 1.5 0 0 1 .549.549c.201.348.201.815.201 1.75"/></g>
</svg>"#;

const TAB_CSV_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M0 0h24v24H0z" fill="none"/><path fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M8 14h2m4 0h2m-8 4h2m4 0h2M13 2.5V3c0 2.828 0 4.243.879 5.121C14.757 9 16.172 9 19 9h.5m.5 1.657V14c0 3.771 0 5.657-1.172 6.828S15.771 22 12 22s-5.657-1.172-6.828-1.172S4 17.771 4 14V9.456c0-3.245 0-4.868.886-5.967a4 4 0 0 1 .603-.603C6.59 2 8.211 2 11.456 2c.705 0 1.058 0 1.381.114q.1.036.197.082c.31.148.559.397 1.058.896l4.736 4.736c.579.578.867.868 1.02 1.235c.152.368.152.776.152 1.594"/>
</svg>"#;

const TAB_3D_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M0 0h24v24H0z" fill="none"/><path fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M12 2v20m2.87-8.324l7.119 3.91m0 0l.005.004m-.005-.004c.075-.437-.232-.928-.802-1.839L13.784 3.92C12.984 2.64 12.584 2 12 2c-.583 0-.983.64-1.784 1.92L2.813 15.746c-.57.911-.877 1.402-.802 1.84m19.978 0a1 1 0 0 1-.028.109c-.152.467-.694.683-1.777 1.116l-7.403 2.957c-.385.154-.578.231-.781.231s-.396-.077-.781-.231l-7.403-2.957c-1.083-.433-1.625-.65-1.777-1.116a1 1 0 0 1-.028-.11m-.005.004l.005-.004m0 0l7.12-3.91"/>
</svg>"#;

const TAB_SVG_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M0 0h24v24H0z" fill="none"/><g fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"><path d="M19 11c0-.818 0-1.57-.152-1.937s-.441-.657-1.02-1.235l-4.736-4.736c-.499-.499-.748-.748-1.058-.896a2 2 0 0 0-.197-.082C11.514 2 11.161 2 10.456 2c-3.245 0-4.868 0-5.967.886a4 4 0 0 0-.603.603C3 4.59 3 6.211 3 9.456V14c0 3.771 0 5.657 1.172 6.828S7.229 22 11 22h8M12 2.5V3c0 2.828 0 4.243.879 5.121C13.757 9 15.172 9 18 9h.5"/><path d="M21 15a1 1 0 0 0-1-1h-1.5a1 1 0 0 0-1 1v3a1 1 0 0 0 1 1H20a1 1 0 0 0 1-1v-1h-1m-10-3H8.5a1.5 1.5 0 0 0 0 3H9a1 1 0 0 1 1 1v.5a1.5 1.5 0 0 1-1.5 1.5H7.5m4.5-5l1.75 5l1.75-5"/></g>
</svg>"#;

fn tab_icon(tab: &Tab) -> &'static [u8] {
    let Some(path) = tab.path.as_deref() else {
        return TAB_TEXT_ICON;
    };
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tif" | "tiff" | "ico" | "heic"
        | "avif" => TAB_IMAGE_ICON,
        "pdf" => TAB_PDF_ICON,
        "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" => TAB_AUDIO_ICON,
        "mp4" | "mov" | "avi" | "mkv" | "webm" | "m4v" => TAB_VIDEO_ICON,
        "csv" | "tsv" => TAB_CSV_ICON,
        "obj" | "fbx" | "gltf" | "glb" | "stl" | "ply" | "3ds" | "dae" | "blend" => TAB_3D_ICON,
        "svg" => TAB_SVG_ICON,
        _ => TAB_TEXT_ICON,
    }
}

// ---------- Folder tab variant (ported from gpui-kit PR #3415) ----------

const FOLDER_TAB_GAP: f32 = 4.0;
const FOLDER_RADIUS: f32 = 8.0;
const FOLDER_TOP_PADDING: f32 = 4.0;
const FOLDER_PADDING_X: f32 = 20.0;
const FOLDER_SEPARATOR_HEIGHT: f32 = 16.0;

/// Adds a quarter circle from `from` to `to`, whose tangents meet at `corner`.
fn quarter_circle_to(
    path: &mut gpui::PathBuilder,
    from: gpui::Point<gpui::Pixels>,
    corner: gpui::Point<gpui::Pixels>,
    to: gpui::Point<gpui::Pixels>,
) {
    const K: f32 = 0.552_284_8;
    path.cubic_bezier_to(to, from + (corner - from) * K, to + (corner - to) * K);
}

/// The selected folder tab: rounded on top, curving out at the bottom into the
/// content below. Painted as a canvas so the curves can overhang the tab.
fn folder_tab_shape(bg: u32) -> impl gpui::IntoElement {
    let curve = gpui::px(FOLDER_RADIUS);
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let (top, bottom) = (bounds.top(), bounds.bottom());
            let (left, right) = (bounds.left(), bounds.right());
            let (tab_left, tab_right) = (left + curve, right - curve);
            // Keep at least a third of the top flat on narrow tabs.
            let radius = ((bounds.size.width - curve * 2.0) / 3.0)
                .clamp(gpui::px(0.), curve);

            let mut path = gpui::PathBuilder::fill()
                .with_style(gpui::PathStyle::Fill(gpui::FillOptions::tolerance(0.02)));
            path.move_to(gpui::point(left, bottom));
            quarter_circle_to(
                &mut path,
                gpui::point(left, bottom),
                gpui::point(tab_left, bottom),
                gpui::point(tab_left, bottom - curve),
            );
            path.line_to(gpui::point(tab_left, top + radius));
            quarter_circle_to(
                &mut path,
                gpui::point(tab_left, top + radius),
                gpui::point(tab_left, top),
                gpui::point(tab_left + radius, top),
            );
            path.line_to(gpui::point(tab_right - radius, top));
            quarter_circle_to(
                &mut path,
                gpui::point(tab_right - radius, top),
                gpui::point(tab_right, top),
                gpui::point(tab_right, top + radius),
            );
            path.line_to(gpui::point(tab_right, bottom - curve));
            quarter_circle_to(
                &mut path,
                gpui::point(tab_right, bottom - curve),
                gpui::point(tab_right, bottom),
                gpui::point(right, bottom),
            );
            path.close();
            if let Ok(path) = path.build() {
                window.paint_path(path, gpui::rgba(bg));
            }
        },
    )
    .absolute()
    .top_0()
    .bottom_0()
    .left(gpui::px(-FOLDER_RADIUS))
    .right(gpui::px(-FOLDER_RADIUS))
}

/// Outline for the active folder tab: rides up the convex corners and over the
/// tab, leaving the bottom edge open so it connects to the content below.
fn folder_tab_outline(color: u32, draw_left: bool, open_bottom: bool) -> impl gpui::IntoElement {
    let curve = gpui::px(FOLDER_RADIUS);
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let (top, bottom) = (bounds.top(), bounds.bottom());
            let (left, right) = (bounds.left(), bounds.right());
            let (tab_left, tab_right) = (left + curve, right - curve);
            let radius = ((bounds.size.width - curve * 2.0) / 3.0)
                .clamp(gpui::px(0.), curve);

            let mut path = gpui::PathBuilder::stroke(gpui::px(1.0));
            if draw_left {
                path.move_to(gpui::point(left, bottom));
                quarter_circle_to(
                    &mut path,
                    gpui::point(left, bottom),
                    gpui::point(tab_left, bottom),
                    gpui::point(tab_left, bottom - curve),
                );
            } else {
                path.move_to(gpui::point(tab_left, bottom));
            }
            path.line_to(gpui::point(tab_left, top + radius));
            quarter_circle_to(
                &mut path,
                gpui::point(tab_left, top + radius),
                gpui::point(tab_left, top),
                gpui::point(tab_left + radius, top),
            );
            path.line_to(gpui::point(tab_right - radius, top));
            quarter_circle_to(
                &mut path,
                gpui::point(tab_right - radius, top),
                gpui::point(tab_right, top),
                gpui::point(tab_right, top + radius),
            );
            path.line_to(gpui::point(tab_right, bottom - curve));
            quarter_circle_to(
                &mut path,
                gpui::point(tab_right, bottom - curve),
                gpui::point(tab_right, bottom),
                gpui::point(right, bottom),
            );
            if !open_bottom {
                if draw_left {
                    path.line_to(gpui::point(left, bottom));
                } else {
                    path.line_to(gpui::point(tab_left, bottom));
                }
            }
            if let Ok(path) = path.build() {
                window.paint_path(path, gpui::rgba(color));
            }
        },
    )
    .absolute()
    .top_0()
    .bottom_0()
    .left(gpui::px(-FOLDER_RADIUS))
    .right(gpui::px(-FOLDER_RADIUS))
}

// ---------- Top bar ----------

#[derive(IntoElement)]
pub struct TopBar {
    pub root: WeakEntity<RootView>,
    pub menu_open: bool,
    pub icon_animation: f32,
    pub tabs: Vec<Tab>,
    pub active: usize,
    pub hovered: Option<usize>,
    pub dirty: std::collections::HashSet<usize>,
    pub can_undo: bool,
    pub can_redo: bool,
    /// Continuous sidebar animation value. Keeping the divider in the layout
    /// during the animation prevents the tab strip from snapping when the
    /// sidebar finishes closing.
    pub sidebar_progress: f32,
    pub sidebar_width: f32,
}

impl gpui::RenderOnce for TopBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl gpui::IntoElement {
        let t = *crate::theme::active(cx);

        div()
            .id("top-bar")
            .w_full()
            .h(px(TOP_BAR_HEIGHT))
            .flex()
            .flex_row()
            .items_center()
            .relative()
            .bg(rgba(t.bg_secondary))
            // Empty space in the title bar is draggable. Interactive children
            // (tabs, menu, and window controls) retain their own hit areas.
            .window_control_area(WindowControlArea::Drag)
            // Bottom border is painted as a child layer so tabs can cover it
            // under the active tab (see below).
            // Clicking anywhere else on the title bar dismisses the menu.
            .on_mouse_down(MouseButton::Left, {
                let root = self.root.clone();
                move |_, _, cx| {
                    let _ = root.update(cx, |root, cx| root.close_menu(cx));
                }
            })
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .h(px(1.))
                    .bg(rgba(t.chrome_border_color)),
            )
            .child(render_history_buttons(
                t,
                self.root.clone(),
                self.can_undo,
                self.can_redo,
                shell_fade(&self.root, cx, "tab-undo"),
                shell_fade(&self.root, cx, "tab-redo"),
            ))
            .child(render_sidebar_button(
                t,
                self.root.clone(),
                shell_fade(&self.root, cx, "tb-sidebar"),
            ))
            .child(render_menu_button(
                self.menu_open,
                self.icon_animation,
                t,
                self.root.clone(),
                shell_fade(&self.root, cx, "tb-menu"),
            ))
            .child(render_settings_button(
                t,
                self.root.clone(),
                shell_fade(&self.root, cx, "tb-settings"),
            ))
            .child(
                div()
                    .w(px((self.sidebar_width - TOP_BAR_CONTROL_CLUSTER_WIDTH).max(0.)))
                    .h_full()
                    .flex_shrink_0(),
            )
            // Keep the divider's layout slot alive throughout the close
            // animation. It grows into place instead of being inserted after
            // the tabs have already moved to the left.
            .child(
                div()
                    .w(px(13.5 * (1.0 - self.sidebar_progress.clamp(0., 1.))))
                    .h_full()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .w(px(2.))
                            .h(px(FOLDER_SEPARATOR_HEIGHT))
                            .rounded(px(2.))
                            .bg(rgba(t.chrome_border_color)),
                    ),
            )
            .child(TabBar {
                root: self.root.clone(),
                tabs: self.tabs,
                active: self.active,
                hovered: self.hovered,
                dirty: self.dirty.clone(),
            })
            .child(render_window_controls(t, cx))
    }
}

pub fn render_sidebar_controls(
    t: crate::theme::Theme,
    root: WeakEntity<RootView>,
    can_undo: bool,
    can_redo: bool,
    sidebar_fade: f32,
    undo_fade: f32,
    redo_fade: f32,
    menu_fade: f32,
) -> AnyElement {
    div()
        .id("sidebar-controls")
        .h(px(TOP_BAR_HEIGHT))
        .flex()
        .flex_row()
        .items_center()
        .bg(rgba(t.bg_tertiary))
        .child(render_history_buttons(
            t,
            root.clone(),
            can_undo,
            can_redo,
            undo_fade,
            redo_fade,
        ))
        .child(render_sidebar_button(t, root.clone(), sidebar_fade))
        .child(render_menu_button(false, 0.0, t, root, menu_fade))
        .into_any_element()
}

fn divider(color: u32) -> AnyElement {
    div()
        .w(px(1.5))
        .h(px(TOP_BAR_HEIGHT - 8.))
        .my(px(4.))
        .mx(px(6.))
        .rounded(px(2.))
        .bg(rgba(color))
        .into_any_element()
}

fn tab_divider(color: u32) -> AnyElement {
    div()
        .w(px(2.))
        .h(px(FOLDER_SEPARATOR_HEIGHT))
        .my(px((TOP_BAR_HEIGHT - FOLDER_SEPARATOR_HEIGHT) / 2.))
        .mx(px(6.))
        .bg(rgba(color))
        .into_any_element()
}

fn render_menu_button(
    _menu_open: bool,
    animation: f32,
    t: crate::theme::Theme,
    root: WeakEntity<RootView>,
    hover_fade: f32,
) -> AnyElement {
    // Active fade follows the open/close animation; hover fade is a tween.
    let act = animation.clamp(0., 1.);
    let k = act.max(hover_fade);
    let bg = crate::theme::lerp_rgb(t.bg_secondary, t.menu_hover_bg, k);

    div()
        .id("app-menu-button")
        .flex()
        .items_center()
        .justify_center()
        .w(px(TOP_BAR_BUTTON_SIZE))
        .h(px(TOP_BAR_BUTTON_SIZE))
        .ml(px(2.))
        .my(px((TOP_BAR_HEIGHT - TOP_BAR_BUTTON_SIZE) / 2.))
        .rounded(px(6.))
        .cursor_pointer()
        .bg(rgba(bg))
        .on_hover({
            let root = root.clone();
            move |hovered, _, cx| {
                let _ = root.update(cx, |root, cx| {
                    root.animate_fade("tb-menu", if *hovered { 1.0 } else { 0.0 }, cx);
                });
            }
        })
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            let _ = root.update(cx, |root, cx| {
                root.toggle_menu(cx);
                cx.stop_propagation();
            });
        })
        .child(
            svg()
                .data(MENU_ICON)
                .w(px(TOP_BAR_ICON_SIZE))
                .h(px(TOP_BAR_ICON_SIZE))
                .text_color(rgba(crate::theme::lerp_rgb(
                    t.empty_text_primary,
                    t.text_primary,
                    k,
                ))),
        )
        .into_any_element()
}

fn render_sidebar_button(
    t: crate::theme::Theme,
    root: WeakEntity<RootView>,
    fade: f32,
) -> AnyElement {
    div()
        .id("sidebar-toggle")
        .flex()
        .items_center()
        .justify_center()
        .w(px(TOP_BAR_BUTTON_SIZE))
        .h(px(TOP_BAR_BUTTON_SIZE))
        .ml(px(2.))
        .my(px((TOP_BAR_HEIGHT - TOP_BAR_BUTTON_SIZE) / 2.))
        .rounded(px(6.))
        .cursor_pointer()
        .bg(rgba(crate::theme::lerp_rgb(
            t.bg_secondary,
            t.menu_hover_bg,
            fade,
        )))
        .on_hover({
            let root = root.clone();
            move |hovered, _, cx| {
                let _ = root.update(cx, |root, cx| {
                    root.animate_fade("tb-sidebar", if *hovered { 1.0 } else { 0.0 }, cx);
                });
            }
        })
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            let _ = root.update(cx, |root, cx| {
                root.toggle_panel(PanelKind::Files, cx);
                cx.stop_propagation();
            });
        })
        .child(
            svg()
                .data(SIDEBAR_ICON_COMPACT)
                .w(px(TOP_BAR_ICON_SIZE))
                .h(px(TOP_BAR_ICON_SIZE))
                .text_color(rgba(crate::theme::lerp_rgb(
                    t.empty_text_primary,
                    t.text_primary,
                    fade,
                ))),
        )
        .into_any_element()
}

fn render_settings_button(
    t: crate::theme::Theme,
    root: WeakEntity<RootView>,
    fade: f32,
) -> AnyElement {
    div()
        .id("app-settings-button")
        .flex()
        .items_center()
        .justify_center()
        .w(px(TOP_BAR_BUTTON_SIZE))
        .h(px(TOP_BAR_BUTTON_SIZE))
        .ml(px(2.))
        .my(px((TOP_BAR_HEIGHT - TOP_BAR_BUTTON_SIZE) / 2.))
        .rounded(px(6.))
        .cursor_pointer()
        .bg(rgba(crate::theme::lerp_rgb(
            t.bg_secondary,
            t.menu_hover_bg,
            fade,
        )))
        .on_hover({
            let root = root.clone();
            move |hovered, _, cx| {
                let _ = root.update(cx, |root, cx| {
                    root.animate_fade("tb-settings", if *hovered { 1.0 } else { 0.0 }, cx);
                });
            }
        })
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            let _ = root.update(cx, |root, cx| {
                root.toggle_settings(cx);
                cx.stop_propagation();
            });
        })
        .child(
            svg()
                .data(SETTINGS_ICON)
                .w(px(TOP_BAR_ICON_SIZE))
                .h(px(TOP_BAR_ICON_SIZE))
                .text_color(rgba(crate::theme::lerp_rgb(
                    t.empty_text_primary,
                    t.text_primary,
                    fade,
                ))),
        )
        .into_any_element()
}

fn render_history_buttons(
    t: crate::theme::Theme,
    root: WeakEntity<RootView>,
    can_undo: bool,
    can_redo: bool,
    undo_fade: f32,
    redo_fade: f32,
) -> AnyElement {
    let button = |id: &'static str, enabled: bool| {
        let root = root.clone();
        let fade = if id == "tab-undo" { undo_fade } else { redo_fade };
        div()
            .id(id)
            .w(px(TOP_BAR_SMALL_BUTTON_SIZE))
            .h(px(TOP_BAR_SMALL_BUTTON_SIZE))
            .my(px((TOP_BAR_HEIGHT - TOP_BAR_SMALL_BUTTON_SIZE) / 2.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(6.))
            .cursor_pointer()
            .text_sm()
            .bg(rgba(crate::theme::lerp_rgb(
                t.bg_secondary,
                t.menu_hover_bg,
                fade,
            )))
            .on_hover({
                let root = root.clone();
                move |hovered, _, cx| {
                    let _ = root.update(cx, |root, cx| {
                        root.animate_fade(id, if *hovered { 1.0 } else { 0.0 }, cx);
                    });
                }
            })
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                let _ = root.update(cx, |root, cx| {
                    if id == "tab-undo" {
                        root.undo(cx);
                    } else {
                        root.redo(cx);
                    }
                });
            })
            .child(
                svg()
                    .data(if id == "tab-undo" {
                        UNDO_ICON
                    } else {
                        REDO_ICON
                    })
                    .w(px(TOP_BAR_ICON_SIZE))
                    .h(px(TOP_BAR_ICON_SIZE))
                    .text_color(rgba(crate::theme::lerp_rgb(
                        if enabled {
                            t.empty_text_primary
                        } else {
                            t.empty_text_secondary
                        },
                        t.text_primary,
                        fade,
                    ))),
            )
    };

    div()
        .id("top-bar-history")
        .flex()
        .flex_row()
        .items_center()
        .ml(px(6.))
        .child(button("tab-undo", can_undo))
        .child(button("tab-redo", can_redo))
        .child(tab_divider(t.chrome_border_color))
        .into_any_element()
}

fn render_window_controls(t: crate::theme::Theme, _cx: &App) -> AnyElement {
    let control = |id: &'static str,
                   content: AnyElement,
                   on_press: Box<dyn Fn(&mut Window, &mut App)>|
     -> AnyElement {
        div()
            .id(id)
            .w(px(46.))
            .h_full()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|s| s.bg(rgba(t.menu_hover_bg)))
            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                cx.stop_propagation();
            })
            .on_click(move |_, window, cx| {
                on_press(window, cx);
            })
            .child(content)
            .into_any_element()
    };

    div()
        .relative()
        .flex_shrink_0()
        .w(px(46.0 * 3.0))
        .h_full()
        .child(
            deferred(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .flex_row()
                    .items_stretch()
                    .bg(rgba(t.bg_secondary))
                    .border_b_1()
                    .border_color(rgba(t.chrome_border_color))
                    .child(control(
                        "window-minimize",
                        div()
                            .w(px(10.))
                            .h(px(1.))
                            .bg(rgba(t.text_secondary))
                            .into_any_element(),
                        Box::new(|window, _| window.minimize_window()),
                    ))
                    .child(control(
                        "window-maximize",
                        div()
                            .size(px(9.))
                            .border_1()
                            .border_color(rgba(t.text_secondary))
                            .into_any_element(),
                        Box::new(|window, _| window.zoom_window()),
                    ))
                    .child(control(
                        "window-close",
                        div()
                            .text_sm()
                            .text_color(rgba(t.text_primary))
                            .child("✕")
                            .into_any_element(),
                        Box::new(|window, _| window.remove_window()),
                    )),
            )
            .with_priority(25000),
        )
        .into_any_element()
}

// Reads a fade value from the root without needing a full upgrade.
fn shell_fade(root: &WeakEntity<RootView>, cx: &App, key: &str) -> f32 {
    root.upgrade().map(|s| s.read(cx).fade(key)).unwrap_or(0.0)
}

// ---------- Tabs in the top bar ----------

#[derive(IntoElement)]
pub struct TabBar {
    pub root: WeakEntity<RootView>,
    pub tabs: Vec<Tab>,
    pub active: usize,
    pub hovered: Option<usize>,
    pub dirty: std::collections::HashSet<usize>,
}

impl gpui::RenderOnce for TabBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl gpui::IntoElement {
        let t = *crate::theme::active(cx);
        let root_for_scroll = self.root.clone();
        let root_for_move = self.root.clone();
        let root_for_up = self.root.clone();
        div()
            .id("tab-bar")
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .flex()
            .flex_row()
            .items_stretch()
            .overflow_hidden()
            .bg(rgba(0x00000000))
            .child(
                div()
                    .id("tab-viewport")
                    .relative()
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .pr(px(8.))
                    .overflow_hidden()
                    .on_mouse_down(
                        MouseButton::Middle,
                        move |event: &gpui::MouseDownEvent, _, cx| {
                            let _ = root_for_scroll.update(cx, |root, _| {
                                root.begin_tab_scroll(event.position.x.as_f32())
                            });
                            cx.stop_propagation();
                        },
                    )
                    .on_mouse_move(move |event: &gpui::MouseMoveEvent, _, cx| {
                        let _ = root_for_move.update(cx, |root, cx| {
                            root.update_tab_scroll(event.position.x.as_f32(), cx)
                        });
                    })
                    .on_mouse_up(MouseButton::Middle, move |_, _, cx| {
                        let _ = root_for_up.update(cx, |root, cx| root.end_tab_scroll(cx));
                    })
                    .child(
                        div()
                            .id("tab-strip")
                            .absolute()
                            .left(px(-self
                                .root
                                .upgrade()
                                .map(|r| r.read(cx).tab_scroll_offset)
                                .unwrap_or(0.)))
                            .top(px(0.))
                            .h_full()
                            .flex()
                            .flex_row()
                            .items_end()
                            .px(px(8.))
                            .children(self.tabs.iter().enumerate().map(|(i, tab)| {
                                let root = self.root.clone();
                                let tab_id = tab.id;
                                let is_active = i == self.active;
                                let is_hovered = self.hovered == Some(tab_id);
                                let is_dirty = self.dirty.contains(&tab_id);
                                // (dividers between tabs removed; all tabs now carry their own border)
                                // Constant geometry in both states: the outer shell always
                                // carries side borders, the inner row always carries the
                                // bottom border — only colors change. Inactive sides are
                                // transparent and the active bottom edge is manuscript-toned,
                                // so switching tabs never moves anything by a pixel.
                                div()
                                    .id(format!("tab-{tab_id}"))
                                    .group(format!("tab-{}", tab_id))
                                    .h(px(TOP_BAR_HEIGHT - 6.))
                                    .mt(px(6.))
                                    .mb(px(0.))
                                    .min_w(px(84.))
                                    .max_w(px(TAB_MAX_WIDTH))
                                    .flex_shrink_0()
                                    .flex()
                                    .flex_row()
                                    .items_stretch()
                                    .cursor_pointer()
                                    .rounded_tl(px(FOLDER_RADIUS))
                                    .rounded_tr(px(FOLDER_RADIUS))
                                    .relative()
                                    .bg(rgba(0x00000000))
                                    .hover(|s| {
                                        if is_active {
                                            s
                                        } else {
                                            s.bg(rgba(t.menu_hover_bg))
                                        }
                                    })
                                    .on_hover({
                                        let root = root.clone();
                                        move |hovered, _, cx| {
                                            let _ = root.update(cx, |root, cx| {
                                                if *hovered {
                                                    root.set_hovered_tab(Some(tab_id), cx);
                                                } else {
                                                    root.clear_hovered_tab(tab_id, cx);
                                                }
                                            });
                                        }
                                    })
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        let _ = root
                                            .update(cx, |root, cx| root.activate_tab(tab_id, cx));
                                    })
                                    .when(!is_active, |tab| {
                                        // Inactive tabs get a solid bg-tertiary folder shape so
                                        // they are not transparent.
                                        let p = self.tabs.len() - i;
                                        tab.child(deferred(folder_tab_shape(t.bg_secondary)).with_priority(p))
                                    })
                                    .when(is_hovered && !is_active, |tab| {
                                        let p = self.tabs.len() - i;
                                        tab.child(deferred(folder_tab_shape(t.menu_hover_bg)).with_priority(p))
                                    })
                                    .when(!is_active, |tab| {
                                        // Outline painted last so it stays visible on hover.
                                        let p = self.tabs.len() - i;
                                        tab.child(deferred(folder_tab_outline(t.chrome_border_color, i == 0, false)).with_priority(p))
                                    })
                                    .when(is_active, |tab| {
                                        // Deferred active-tab painting keeps its convex corners
                                        // above every other tab regardless of position.
                                        tab.child(
                                            deferred(folder_tab_shape(t.bg_primary))
                                                .with_priority(1000),
                                        )
                                        .child(
                                            deferred(folder_tab_outline(t.chrome_border_color, true, true))
                                                .with_priority(1000),
                                        )
                                    })
                                    .child(deferred(
                                        div()
                                            .absolute()
                                            .right(px(4.))
                                            .top_0()
                                            .bottom(px(0.))
                                            .w(px(16.))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(
                                                div()
                                                    .w(px(6.))
                                                    .h(px(6.))
                                                    .rounded(px(3.))
                                                    .bg(rgba(t.text_secondary))
                                                    .opacity(if is_dirty && !is_hovered { 1.0 } else { 0.0 }),
                                            ),
                                    ).with_priority(10000))
                                    .child(
                                        deferred(
                                            div()
                                            .relative()
                                            .id(format!("tab-surface-{tab_id}"))
                                            .min_w(px(0.))
                                            .flex_1()
                                            .h_full()
                                            .flex_shrink_0()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .pl(px(FOLDER_PADDING_X))
                                            // Keep the title clear of the close control and
                                            // give both sides enough breathing room.
                                            .pr(px(28.))
                                            .on_hover({
                                                let root = self.root.clone();
                                                move |hovered, _, cx| {
                                                    let _ = root.update(cx, |root, cx| {
                                                        if *hovered {
                                                            root.set_hovered_tab(Some(tab_id), cx);
                                                        } else {
                                                            root.clear_hovered_tab(tab_id, cx);
                                                        }
                                                    });
                                                }
                                            })
                                            .child(
                                                div()
                                                    .min_w(px(0.))
                                                    .flex_1()
                                                    .h_full()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(
                                                        div()
                                                            .w_full()
                                                            .min_w(px(0.))
                                                            .text_center()
                                                            .text_sm()
                                                            .font_weight(WEIGHT_MEDIUM)
                                                            .text_color(rgba(if is_active {
                                                                t.text_primary
                                                            } else {
                                                                t.text_secondary
                                                            }))
                                                            .whitespace_nowrap()
                                                            .overflow_hidden()
                                                            .text_ellipsis()
                                                            .child(tab.title.clone()),
                                                    ),
                                            )
                                            .child({
                                                let root = self.root.clone();
                                                let close_hovered = root
                                                    .upgrade()
                                                    .map(|view| {
                                                        view.read(cx).hovered_close_tab
                                                            == Some(tab_id)
                                                    })
                                                    .unwrap_or(false);
                                                // Fixed slot so the title never shifts. The close control
                                                // is always mounted so hover timing cannot hide it.
                                                div()
                                                    .absolute()
                                                    .right(px(4.))
                                                    .top_0()
                                                    .bottom(px(0.))
                                                    .w(px(16.))
                                                    .opacity(if is_hovered { 1.0 } else { 0.0 })
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(
                                                        div()
                                                            .id(format!("tab-close-{tab_id}"))
                                                            .w(px(16.))
                                                            .min_w(px(16.))
                                                            .h(px(16.))
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .rounded(px(4.))
                                                            .text_color(rgba(if close_hovered {
                                                                t.text_primary
                                                            } else {
                                                                t.text_secondary
                                                            }))
                                                            .when(close_hovered, |button| {
                                                                button.bg(rgba(t.bg_secondary))
                                                            })
                                                            .on_hover({
                                                                let root = root.clone();
                                                                move |hovered, _, cx| {
                                                                    let _ =
                                                                        root.update(
                                                                            cx,
                                                                            |root, cx| {
                                                                                if *hovered {
                                                    root.hovered_close_tab = Some(tab_id);
                                                } else if root.hovered_close_tab == Some(tab_id) {
                                                    root.hovered_close_tab = None;
                                                }
                                                                                cx.notify();
                                                                            },
                                                                        );
                                                                }
                                                            })
                                                            .on_mouse_down(
                                                                MouseButton::Left,
                                                                move |_, _, cx| {
                                                                    let _ = root.update(
                                                                        cx,
                                                                        |root, cx| {
                                                                            root.close_tab(
                                                                                tab_id, cx,
                                                                            )
                                                                        },
                                                                    );
                                                                    cx.stop_propagation();
                                                                },
                                                            )
                                                            .child(
                                                                svg()
                                                                    .data(TAB_CLOSE_ICON)
                                                                    .w(px(12.))
                                                                    .h(px(12.))
                                                                    .text_color(rgba(
                                                                        if close_hovered {
                                                                            t.text_primary
                                                                        } else {
                                                                            t.text_secondary
                                                                        },
                                                                    )),
                                                            ),
                                                    )
                                            }),
                                            )
                                            .with_priority(2000),
                                    )
                            }))
                    )
                    // Smooth right-edge gradient fade over overflowing tabs:
                    // background fades gently over 48px while the 1px bottom
                    // border fades in much tighter (16px) right before the
                    // window controls.
                    .child(
                        deferred(
                            div()
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .right_0()
                                .w(px(48.))
                                .child(
                                    div()
                                        .absolute()
                                        .top_0()
                                        .bottom(px(1.))
                                        .left_0()
                                        .right_0()
                                        .bg(gpui::linear_gradient(
                                            90.0,
                                            gpui::linear_color_stop(
                                                rgba(t.bg_secondary & 0xffffff00),
                                                0.0,
                                            ),
                                            gpui::linear_color_stop(rgba(t.bg_secondary), 1.0),
                                        )),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .right_0()
                                        .bottom_0()
                                        .w(px(28.))
                                        .h(px(1.))
                                        .bg(gpui::linear_gradient(
                                            90.0,
                                            gpui::linear_color_stop(
                                                rgba(t.chrome_border_color & 0xffffff00),
                                                0.0,
                                            ),
                                            gpui::linear_color_stop(
                                                rgba(t.chrome_border_color),
                                                1.0,
                                            ),
                                        )),
                                ),
                        )
                        .with_priority(20000),
                    ),
            )
    }
}

// ---------- Toolbar (per-file-kind tools) ----------

const TOOLBAR_ICON_CURSOR: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<path fill="none" stroke="currentColor" stroke-linejoin="round" stroke-width="1.5" d="m9.803 4.63l6.033 2.36c3.48 1.362 5.22 2.043 5.163 3.123c-.058 1.08-1.874 1.576-5.506 2.566c-1.081.295-1.622.442-1.997.817s-.522.916-.817 1.997c-.99 3.632-1.486 5.448-2.566 5.506s-1.76-1.683-3.122-5.163L4.63 9.803C3.204 6.159 2.49 4.338 3.414 3.414c.924-.923 2.745-.21 6.389 1.216Z" />
</svg>"#;

const TOOLBAR_ICON_PEN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<g fill="none" stroke="currentColor" stroke-linecap="round" stroke-width="1.5">
		<path stroke-linejoin="round" d="M16.422 7c.26-1.182 1.09-3.196.118-4.268C15.877 2 14.629 2 12.134 2h-.268c-2.495 0-3.742 0-4.406.732c-.972 1.072-.143 3.086.12 4.268m10.427 7.653l-4.22 5.705C12.977 21.453 12.573 22 12 22s-.977-.547-1.787-1.642l-4.22-5.705c-.676-.915-1.015-1.373-.992-1.86s.475-.998 1.379-2.02C7.25 9.79 7.579 8.77 7.579 7h8.843c0 1.769.327 2.789 1.198 3.773c.904 1.022 1.356 1.532 1.379 2.02c.023.487-.316.945-.992 1.86" />
		<path d="M13.5 14c-.441.318-.954.5-1.5.5m0 0a2.55 2.55 0 0 1-1.5-.5m1.5.5V22" />
	</g>
</svg>"#;

const TOOLBAR_ICON_HIGHLIGHTER: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<path fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="m6.678 16.207l2.115 2.115m-2.115-2.115L2.5 20.5h4l2.293-2.178m-2.115-2.115c-.39-.39-.383-1.02-.043-1.455c.792-1.014 1.08-1.93 1.164-2.614c.093-.752.275-1.565.81-2.1l.892-.89m-.708 9.174c.39.39 1.02.383 1.455.043c1.014-.792 1.93-1.08 2.614-1.164c.752-.093 1.565-.275 2.1-.81l.89-.892m0 0l-6.35-6.35m6.35 6.35a1 1 0 0 0 1.414 0l4.234-4.234M9.501 9.148a1 1 0 0 1 0-1.414L13.735 3.5" />
</svg>"#;

const TOOLBAR_ICON_ERASER: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<path fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="m8.738 7.78l-3.891 3.977c-1.17 1.196-1.755 1.794-1.834 2.523q-.025.232 0 .464c.08.73.664 1.327 1.834 2.523l.149.152c.624.639.937.958 1.31 1.172q.328.188.689.291c.413.118.856.118 1.743.118c.886 0 1.33 0 1.743-.118q.361-.104.688-.29c.374-.215.686-.534 1.31-1.173l2.845-2.907M8.738 7.78l3.582-3.657C13.706 2.708 14.4 2 15.263 2s1.556.708 2.941 2.125l.743.76C20.316 6.283 21 6.983 21 7.85s-.685 1.566-2.054 2.964l-3.622 3.698M8.738 7.78l6.586 6.732M10 22h11" />
</svg>"#;

const TOOLBAR_ICON_SHAPES: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 16 16">
	<path d="M0 0h16v16H0z" fill="none" />
	<path fill="currentColor" d="M5.5 2A3.5 3.5 0 0 0 5 8.965v1.008A4.5 4.5 0 1 1 9.973 5H8.965A3.5 3.5 0 0 0 5.5 2m3 4A2.5 2.5 0 0 0 6 8.5v4A2.5 2.5 0 0 0 8.5 15h4a2.5 2.5 0 0 0 2.5-2.5v-4A2.5 2.5 0 0 0 12.5 6zM7 8.5A1.5 1.5 0 0 1 8.5 7h4A1.5 1.5 0 0 1 14 8.5v4a1.5 1.5 0 0 1-1.5 1.5h-4A1.5 1.5 0 0 1 7 12.5z" />
</svg>"#;

const TOOLBAR_ICON_CROP: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<g fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2">
		<path d="M6 2v14a2 2 0 0 0 2 2h14" />
		<path d="M18 22V8a2 2 0 0 0-2-2H2" />
	</g>
</svg>"#;

const TOOLBAR_ICON_RECT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<path fill="none" stroke="currentColor" stroke-width="1.5" d="M2.5 12c0-4.478 0-6.718 1.391-8.109S7.521 2.5 12 2.5c4.478 0 6.718 0 8.109 1.391S21.5 7.521 21.5 12c0 4.478 0 6.718-1.391 8.109S16.479 21.5 12 21.5c-4.478 0-6.718 0-8.109-1.391S2.5 16.479 2.5 12Z" />
</svg>"#;

const TOOLBAR_ICON_CIRCLE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<circle cx="12" cy="12" r="10" fill="none" stroke="currentColor" stroke-linejoin="round" stroke-width="1.5" />
</svg>"#;

const TOOLBAR_ICON_LINE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<path fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M3 20L21 4" />
</svg>"#;

const TOOLBAR_ICON_ARROW: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<g fill="none">
		<path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M18.578 12c.019-1.313.922-5.664 0-6.586s-5.272-.019-6.586 0" />
		<path fill="currentColor" d="M4.462 18.47a.75.75 0 1 0 1.06 1.06l-.53-.53zm14.06-11.94a.75.75 0 0 0-1.06-1.06l.53.53zM4.993 19l.53.53l13-13l-.53-.53l-.53-.53l-13 13z" />
	</g>
</svg>"#;

const TOOLBAR_ICON_MD_SPLIT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<g fill="none" stroke="currentColor" stroke-linecap="round" stroke-width="1.5">
		<path stroke-linejoin="round" d="M11.992 21V3" />
		<path d="M7.992 20c-1.3-.13-2.175-.423-2.828-1.084C3.992 17.729 3.992 15.819 3.992 12s0-5.73 1.172-6.916C5.817 4.422 6.692 4.13 7.992 4m8 16c1.3-.13 2.175-.422 2.829-1.084c1.171-1.187 1.171-3.096 1.171-6.916s0-5.73-1.171-6.916c-.629-.637-1.463-.932-2.684-1.068" />
	</g>
</svg>"#;

const TOOLBAR_ICON_MD_STACKED: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<path fill="none" stroke="currentColor" stroke-linejoin="round" stroke-width="1.5" d="M11.992 2c2.8 0 4.2 0 5.27.545a5 5 0 0 1 2.185 2.185c.545 1.07.545 2.47.545 5.27v4c0 2.8 0 4.2-.545 5.27a5 5 0 0 1-2.185 2.185c-1.07.545-2.47.545-5.27.545s-4.2 0-5.27-.545a5 5 0 0 1-2.185-2.185c-.545-1.07-.545-2.47-.545-5.27v-4c0-2.8 0-4.2.545-5.27a5 5 0 0 1 2.185-2.185C7.792 2 9.192 2 11.992 2Z" />
</svg>"#;

const TOOLBAR_ICON_CHEVRON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<path fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.75" d="m6 9l6 6l6-6" />
</svg>"#;

const TOOLBAR_ICON_MD_PARAGRAPH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<path fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M15 3v18m0-18h-5m5 0h6m-11 9H7.5a4.5 4.5 0 0 1 0-9H10m0 9V3m0 9v9" />
</svg>"#;

const TOOLBAR_ICON_MD_BOLD: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<path fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M13 4h-3c-1.886 0-2.828 0-3.414.586S6 6.114 6 8v4h7a4 4 0 0 0 0-8m1 8H6v4c0 1.886 0 2.828.586 3.414S8.114 20 10 20h4a4 4 0 0 0 0-8" />
</svg>"#;

const TOOLBAR_ICON_MD_ITALIC: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<path fill="none" stroke="currentColor" stroke-linecap="round" stroke-width="1.5" d="M10.667 4H20M7.999 20l8-16m-12 16h9.333" />
</svg>"#;

const TOOLBAR_ICON_MD_STRIKE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<g fill="none" stroke="currentColor" stroke-linecap="round" stroke-width="1.5">
		<path stroke-linejoin="round" d="M3.992 12h16" />
		<path d="M17.492 7.667c0-2.578-2.462-4.667-5.5-4.667s-5.5 2.09-5.5 4.667Q6.491 8.396 6.66 9m-.667 7.333c0 2.578 2.686 4.667 6 4.667s6-1.333 6-4.667c0-2.393-1.03-3.755-3.092-4.333" />
	</g>
</svg>"#;

const TOOLBAR_ICON_MD_LINK: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<g fill="none" stroke="currentColor" stroke-linecap="round" stroke-width="1.5">
		<path d="M10 13.229q.213.349.504.654a3.56 3.56 0 0 0 4.454.59q.391-.24.73-.59l3.239-3.372c1.43-1.49 1.43-3.904 0-5.394a3.564 3.564 0 0 0-5.183 0l-.714.743" />
		<path d="m10.97 18.14l-.713.743a3.564 3.564 0 0 1-5.184 0c-1.43-1.49-1.43-3.905 0-5.394l3.24-3.372a3.564 3.564 0 0 1 5.183 0q.291.305.504.654" />
	</g>
</svg>"#;

const TOOLBAR_ICON_MD_BULLET_LIST: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<g fill="none" stroke="currentColor" stroke-linecap="round" stroke-width="1.5">
		<path d="M8 5.5h12m-12 7h12m-12 7h12" />
		<path stroke-linejoin="round" d="M4.375 5.5H4.25m.25 0a.25.25 0 1 1-.5 0a.25.25 0 0 1 .5 0m-.125 7H4.25m.25 0a.25.25 0 1 1-.5 0a.25.25 0 0 1 .5 0m-.125 7H4.25m.25 0a.25.25 0 1 1-.5 0a.25.25 0 0 1 .5 0" />
	</g>
</svg>"#;

const TOOLBAR_ICON_MD_NUMBERED_LIST: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<g fill="none" stroke="currentColor" stroke-linecap="round" stroke-width="1.5">
		<path d="M11 6h10m-10 6h10m-10 6h10" />
		<path stroke-linejoin="round" d="M3 15h1.5c.279 0 .418 0 .534.023a1.2 1.2 0 0 1 .943.943C6 16.082 6 16.22 6 16.5s0 .418-.023.534a1.2 1.2 0 0 1-.943.943C4.918 18 4.78 18 4.5 18s-.418 0-.534.023a1.2 1.2 0 0 0-.943.943C3 19.082 3 19.22 3 19.5v.9c0 .283 0 .424.088.512s.23.088.512.088H6M3 3h1.2a.3.3 0 0 1 .3.3V9m0 0H3m1.5 0H6" />
	</g>
</svg>"#;

const TOOLBAR_ICON_MD_TODO_LIST: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<g fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5">
		<path d="M11 6h10m-10 6h10m-10 6h10" />
		<path d="M3 7.393S4 8.045 4.5 9C4.5 9 6 5.25 8 4M3 18.393S4 19.045 4.5 20c0 0 1.5-3.75 3.5-5" />
	</g>
</svg>"#;

const TOOLBAR_ICON_MD_QUOTE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<path fill="none" stroke="currentColor" stroke-linejoin="round" stroke-width="1.5" d="M17.992 5.75c2.21 0 4 1.885 4 4.211c0 3.065-1.414 5.782-3.592 7.469c-.783.606-1.175.909-1.441.797a.5.5 0 0 1-.208-.162c-.174-.232.053-.775.505-1.862c.245-.588.367-.881.321-1.167a.8.8 0 0 0-.084-.272c-.123-.262-.612-.579-1.591-1.212c-1.146-.74-1.91-2.072-1.91-3.591c0-1.49.735-2.8 1.845-3.548a3.84 3.84 0 0 1 2.155-.663Zm-12 0c2.21 0 4 1.885 4 4.211c0 3.065-1.414 5.782-3.592 7.469c-.783.606-1.175.909-1.441.797a.5.5 0 0 1-.208-.162c-.174-.232.053-.775.505-1.862c.245-.588.367-.881.321-1.167a.8.8 0 0 0-.084-.272c-.123-.262-.612-.579-1.591-1.212c-1.146-.74-1.91-2.072-1.91-3.591c0-1.49.735-2.8 1.845-3.548a3.84 3.84 0 0 1 2.155-.663Z" clip-rule="evenodd" />
</svg>"#;

const TOOLBAR_ICON_MD_CODE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<path fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="m17 8l1.84 1.85c.773.778 1.16 1.167 1.16 1.65s-.387.872-1.16 1.65L17 15M7 8L5.16 9.85C4.387 10.628 4 11.017 4 11.5s.387.872 1.16 1.65L7 15m7.5-11l-5 16" />
</svg>"#;

#[derive(IntoElement)]
pub struct Toolbar {
    pub root: WeakEntity<RootView>,
}

impl gpui::RenderOnce for Toolbar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl gpui::IntoElement {
        let t = *crate::theme::active(cx);
        let root_upgrade = self.root.upgrade();
        let is_image = root_upgrade.as_ref().map(|r| r.read(cx).active_is_image()).unwrap_or(false);
        let is_markdown = root_upgrade.as_ref().map(|r| r.read(cx).active_is_markdown()).unwrap_or(false);
        let is_svg = root_upgrade.as_ref().map(|r| r.read(cx).active_is_svg()).unwrap_or(false);
        let svg_split = root_upgrade.as_ref().map(|r| r.read(cx).svg_split_view).unwrap_or(true);
        let md_mode = root_upgrade
            .as_ref()
            .map(|r| r.read(cx).markdown_mode)
            .unwrap_or(MarkdownViewMode::Plain);
        let active_tool = root_upgrade.as_ref().map(|r| r.read(cx).image_toolbar_tool).unwrap_or(0);
        let shapes_open = root_upgrade.as_ref().map(|r| r.read(cx).image_shapes_dropdown_open).unwrap_or(false);
        let (ink_popup_display, ink_popup_progress) = root_upgrade
            .as_ref()
            .map(|r| {
                let state = r.read(cx);
                (state.ink_popup_display, state.ink_popup_progress)
            })
            .unwrap_or((None, 0.0));
        let left_occ = root_upgrade.as_ref().map(|r| r.read(cx).left_occupancy()).unwrap_or(0.0);
        let right_occ = root_upgrade.as_ref().map(|r| r.read(cx).right_occupancy()).unwrap_or(0.0);
        let content_w = (window.viewport_size().width.as_f32() - left_occ - right_occ).max(0.0);

        let root = self.root.clone();
        let tool_button = |root: WeakEntity<RootView>, index: usize, icon: &'static [u8], tooltip: &'static str, active: bool| {
            let is_active = active_tool == index;
            div()
                .id(format!("toolbar-tool-{}", index))
                .w(px(28.))
                .h(px(28.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(8.))
                .cursor_pointer()
                .border_1()
                .border_color(rgba(if is_active { t.border_color } else { 0x00000000 }))
                .bg(rgba(if is_active { t.bg_darker } else { t.bg_primary }))
                .hover(|s| s.bg(rgba(t.bg_darker)))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    let _ = root.update(cx, |root, cx| {
                        // Clicking the tool that is already active opens its
                        // colour and size settings.
                        if index == 5 {
                            if root.image_toolbar_tool == 5 {
                                root.cancel_image_crop(cx);
                            } else {
                                root.begin_image_crop(cx);
                            }
                        } else if root.image_toolbar_tool == index && matches!(index, 1 | 2) {
                            let tool = index as u8;
                            let next = if root.ink_popup == Some(tool) {
                                None
                            } else {
                                Some(tool)
                            };
                            root.set_ink_popup(next, cx);
                        } else {
                            if root.image_toolbar_tool == 5 {
                                root.cancel_image_crop(cx);
                            }
                            root.image_toolbar_tool = index;
                            root.set_ink_popup(None, cx);
                        }
                        root.image_shapes_dropdown_open = false;
                        cx.notify();
                    });
                    cx.stop_propagation();
                })
                .child(
                    svg()
                        .data(icon)
                        .w(px(19.))
                        .h(px(19.))
                        .text_color(rgba(if is_active { t.text_primary } else { t.text_secondary })),
                )
        };

        let mut bar = div()
            .id("toolbar")
            .relative()
            .ml(px(left_occ))
            .w(px(content_w))
            .h(px(TOOLBAR_HEIGHT))
            .flex_shrink_0()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(4.))
            .px(px(8.))
            .bg(rgba(t.bg_primary))
            .border_b_1()
            .border_color(rgba(t.chrome_border_color));

        if is_image {
            // Cursor (0)
            bar = bar.child(tool_button(root.clone(), 0, TOOLBAR_ICON_CURSOR, "Cursor", active_tool == 0));
            // Pen (1) and Highlighter (2): clicking the active tool opens
            // its colour and size settings.
            for (index, icon) in [(1usize, TOOLBAR_ICON_PEN), (2, TOOLBAR_ICON_HIGHLIGHTER)] {
                let root_for_popup = root.clone();
                let mut wrap = div()
                    .relative()
                    .child(tool_button(root.clone(), index, icon, if index == 1 { "Pen" } else { "Highlighter" }, active_tool == index));
                if ink_popup_display == Some(index as u8) && ink_popup_progress > 0.005 {
                    let colour = root_upgrade
                        .as_ref()
                        .map(|r| r.read(cx).ink_color(index as u8))
                        .unwrap_or(0);
                    let size = root_upgrade
                        .as_ref()
                        .map(|r| r.read(cx).ink_size(index as u8))
                        .unwrap_or(4.0);
                    let handle = root_upgrade
                        .as_ref()
                        .map(|r| r.read(cx).ink_handle)
                        .unwrap_or(0.0);
                    let is_dark = crate::theme::mode(cx) == crate::theme::ThemeMode::Dark;
                    wrap = wrap.child(ink_settings_popup(
                        &t,
                        is_dark,
                        root_for_popup.clone(),
                        index as u8,
                        colour,
                        size,
                        handle,
                        ink_popup_progress,
                    ));
                }
                bar = bar.child(wrap);
            }
            // Eraser (3)
            bar = bar.child(tool_button(root.clone(), 3, TOOLBAR_ICON_ERASER, "Eraser", active_tool == 3));
            // Shapes (4) with dropdown
            {
                let root_for_shape = root.clone();
                let shapes_active = active_tool == 4;
                let shape_btn = div()
                    .id("toolbar-tool-shapes")
                    .w(px(28.))
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(8.))
                    .cursor_pointer()
                    .border_1()
                    .border_color(rgba(if shapes_active { t.border_color } else { 0x00000000 }))
                    .bg(rgba(if shapes_active { t.bg_darker } else { t.bg_primary }))
                    .hover(|s| s.bg(rgba(t.bg_darker)))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        let _ = root_for_shape.update(cx, |root, cx| {
                            root.image_toolbar_tool = 4;
                            root.image_shapes_dropdown_open = !root.image_shapes_dropdown_open;
                            cx.notify();
                        });
                        cx.stop_propagation();
                    })
                    .child(
                        svg()
                            .data(TOOLBAR_ICON_SHAPES)
                            .w(px(19.))
                            .h(px(19.))
                            .text_color(rgba(if shapes_active { t.text_primary } else { t.text_secondary })),
                    );
                let mut shapes_wrap = div().relative().child(shape_btn);
                if shapes_open {
                    let root_for_opt = root.clone();
                    let selected_shape = root_upgrade.as_ref().map(|r| r.read(cx).image_shapes_selected).unwrap_or(0);
                    let opt_button = |root: WeakEntity<RootView>, index: usize, icon: &'static [u8], label: &'static str, selected: usize| {
                        let is_active = selected == index;
                        div()
                            .id(format!("shape-opt-{}", label))
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .h(px(ENTRY_HEIGHT))
                            .pl(px(6.))
                            .pr(px(4.))
                            .rounded(px(8.))
                            .text_sm()
                            .font_weight(WEIGHT_MEDIUM)
                            .text_color(rgba(t.text_primary))
                            .cursor_pointer()
                            .bg(rgba(if is_active { t.bg_tertiary } else { t.bg_primary }))
                            .hover(|s| s.bg(rgba(t.menu_hover_bg)))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                let _ = root.update(cx, |root, cx| {
                                    root.image_shapes_selected = index;
                                    root.image_shapes_dropdown_open = false;
                                    cx.notify();
                                });
                                cx.stop_propagation();
                            })
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(svg().data(icon).w(px(14.)).h(px(14.)).text_color(rgba(t.text_primary)))
                                    .child(label),
                            )
                    };
                    shapes_wrap = shapes_wrap.child(
                        deferred(
                            div()
                                .absolute()
                                .top(px(32.))
                                .left(px(0.))
                                .w(px(160.))
                                .occlude()
                                .flex()
                                .flex_col()
                                .px(px(PANEL_PADDING_X))
                                .py(px(PANEL_PADDING_Y))
                                .gap(px(2.0))
                                .bg(rgba(t.bg_primary))
                                .border_1()
                                .border_color(rgba(t.menu_border_color))
                                .rounded(px(10.))
                                .shadow(vec![t.shadow_lg()])
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .child(opt_button(root_for_opt.clone(), 0, TOOLBAR_ICON_RECT, "Rectangle", selected_shape))
                                .child(opt_button(root_for_opt.clone(), 1, TOOLBAR_ICON_CIRCLE, "Circle", selected_shape))
                                .child(opt_button(root_for_opt.clone(), 2, TOOLBAR_ICON_LINE, "Line", selected_shape))
                                .child(opt_button(root_for_opt.clone(), 3, TOOLBAR_ICON_ARROW, "Arrow", selected_shape)),
                        )
                        .with_priority(1000),
                    );
                }
                bar = bar.child(shapes_wrap);
            }
            // divider (matches bottom bar dividers)
            bar = bar.child(
                div()
                    .w(px(2.))
                    .h(px(16.))
                    .mx(px(6.))
                    .bg(rgba(t.chrome_border_color)),
            );
            // Crop (5)
            bar = bar.child(tool_button(root.clone(), 5, TOOLBAR_ICON_CROP, "Crop", active_tool == 5));

            // Far-right Cancel / Save buttons when Crop is active.
            if active_tool == 5 {
                let root_cancel = root.clone();
                let root_save = root.clone();
                bar = bar.child(
                    div()
                        .absolute()
                        .right(px(8.))
                        .top_0()
                        .bottom_0()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.))
                        .child(
                            div()
                                .id("toolbar-crop-cancel")
                                .h(px(24.))
                                .px(px(10.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(7.))
                                .border_1()
                                .border_color(rgba(t.border_color))
                                .bg(rgba(t.bg_primary))
                                .text_xs()
                                .font_weight(WEIGHT_MEDIUM)
                                .text_color(rgba(t.text_secondary))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgba(t.bg_darker)).text_color(rgba(t.text_primary)))
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    let _ = root_cancel.update(cx, |root, cx| {
                                        root.cancel_image_crop(cx);
                                    });
                                    cx.stop_propagation();
                                })
                                .child("Cancel"),
                        )
                        .child(
                            div()
                                .id("toolbar-crop-save")
                                .h(px(24.))
                                .px(px(10.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(7.))
                                .border_1()
                                .border_color(rgba(t.accent))
                                .bg(rgba(t.accent))
                                .text_xs()
                                .font_weight(WEIGHT_MEDIUM)
                                .text_color(rgba(0xffffffff))
                                .cursor_pointer()
                                .hover(|s| s.opacity(0.9))
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    let _ = root_save.update(cx, |root, cx| {
                                        root.save_image_crop(cx);
                                    });
                                    cx.stop_propagation();
                                })
                                .child("Save"),
                        ),
                );
            }
        }

        if is_markdown {
            let md_heading_open = root_upgrade
                .as_ref()
                .map(|r| r.read(cx).md_heading_dropdown_open)
                .unwrap_or(false);
            let active_heading = root_upgrade
                .as_ref()
                .map(|r| r.read(cx).active_markdown_heading_level())
                .unwrap_or(0);

            let heading_indicator = if active_heading > 0 {
                div()
                    .w(px(18.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_xs()
                    .font_weight(WEIGHT_MEDIUM)
                    .text_color(rgba(t.text_secondary))
                    .child(format!("H{active_heading}"))
                    .into_any_element()
            } else {
                div()
                    .w(px(18.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        svg()
                            .data(TOOLBAR_ICON_MD_PARAGRAPH)
                            .w(px(15.))
                            .h(px(15.))
                            .text_color(rgba(t.text_secondary)),
                    )
                    .into_any_element()
            };
            let root_heading = root.clone();
            let mut heading_wrap = div().relative().child(
                div()
                    .id("toolbar-md-heading")
                    .w(px(56.))
                    .h(px(28.))
                    .px(px(10.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .rounded(px(8.))
                    .cursor_pointer()
                    .border_1()
                    .border_color(rgba(t.border_color))
                    .bg(rgba(if md_heading_open {
                        t.bg_darker
                    } else {
                        t.bg_primary
                    }))
                    .hover(|s| s.bg(rgba(t.bg_darker)))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        let _ = root_heading.update(cx, |root, cx| {
                            root.md_heading_dropdown_open = !root.md_heading_dropdown_open;
                            cx.notify();
                        });
                        cx.stop_propagation();
                    })
                    .child(heading_indicator)
                    .child(
                        svg()
                            .data(TOOLBAR_ICON_CHEVRON)
                            .w(px(12.))
                            .h(px(12.))
                            .text_color(rgba(t.text_secondary)),
                    ),
            );
            if md_heading_open {
                let mut menu = div()
                    .absolute()
                    .top(px(30.))
                    .left(px(0.))
                    .w(px(128.))
                    .occlude()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .px(px(PANEL_PADDING_X))
                    .py(px(PANEL_PADDING_Y))
                    .bg(rgba(t.bg_primary))
                    .border_1()
                    .border_color(rgba(t.component_border_color))
                    .rounded(px(10.))
                    .shadow(vec![t.shadow_lg()])
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
                let items: [(usize, &'static str, f32); 7] = [
                    (0, "Paragraph", 13.0),
                    (1, "Heading 1", 16.0),
                    (2, "Heading 2", 15.0),
                    (3, "Heading 3", 14.0),
                    (4, "Heading 4", 13.5),
                    (5, "Heading 5", 13.0),
                    (6, "Heading 6", 12.5),
                ];
                for (level, label, size) in items {
                    let selected = active_heading == level;
                    let root_item = root.clone();
                    menu = menu.child(
                        div()
                            .id(format!("toolbar-md-h-{level}"))
                            .w_full()
                            .h(px(ENTRY_HEIGHT))
                            .px(px(8.))
                            .rounded(px(6.))
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .bg(rgba(if selected { t.bg_darker } else { 0x00000000 }))
                            .hover(|s| s.bg(rgba(t.menu_hover_bg)))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                let _ = root_item.update(cx, |root, cx| {
                                    root.md_set_heading(level, cx);
                                });
                                cx.stop_propagation();
                            })
                            .child(
                                div()
                                    .text_size(px(size))
                                    .font_weight(if level > 0 {
                                        WEIGHT_MEDIUM
                                    } else {
                                        gpui::FontWeight::NORMAL
                                    })
                                    .text_color(rgba(if selected {
                                        t.text_primary
                                    } else {
                                        t.text_secondary
                                    }))
                                    .child(label),
                            ),
                    );
                }
                heading_wrap = heading_wrap.child(deferred(menu).with_priority(1000));
            }

            let md_action = |id: &'static str,
                             icon: &'static [u8],
                             action: Box<dyn Fn(&mut RootView, &mut gpui::Context<RootView>)>| {
                let root_click = root.clone();
                div()
                    .id(id)
                    .w(px(28.))
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(8.))
                    .cursor_pointer()
                    .bg(rgba(t.bg_primary))
                    .hover(|s| s.bg(rgba(t.bg_darker)))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        let _ = root_click.update(cx, |root, cx| {
                            action(root, cx);
                        });
                        cx.stop_propagation();
                    })
                    .child(
                        svg()
                            .data(icon)
                            .w(px(16.))
                            .h(px(16.))
                            .text_color(rgba(t.text_secondary)),
                    )
            };

            bar = bar
                .child(heading_wrap)
                .child(
                    div()
                        .w(px(1.))
                        .h(px(16.))
                        .mx(px(4.))
                        .bg(rgba(t.chrome_border_color)),
                )
                .child(md_action(
                    "toolbar-md-bold",
                    TOOLBAR_ICON_MD_BOLD,
                    Box::new(|r, cx| r.md_wrap_inline("**", "**", cx)),
                ))
                .child(md_action(
                    "toolbar-md-italic",
                    TOOLBAR_ICON_MD_ITALIC,
                    Box::new(|r, cx| r.md_wrap_inline("*", "*", cx)),
                ))
                .child(md_action(
                    "toolbar-md-strike",
                    TOOLBAR_ICON_MD_STRIKE,
                    Box::new(|r, cx| r.md_wrap_inline("~~", "~~", cx)),
                ))
                .child(md_action(
                    "toolbar-md-link",
                    TOOLBAR_ICON_MD_LINK,
                    Box::new(|r, cx| r.md_insert_link(cx)),
                ))
                .child(
                    div()
                        .w(px(1.))
                        .h(px(16.))
                        .mx(px(4.))
                        .bg(rgba(t.chrome_border_color)),
                )
                .child(md_action(
                    "toolbar-md-quote",
                    TOOLBAR_ICON_MD_QUOTE,
                    Box::new(|r, cx| r.md_toggle_line_prefix("> ", false, cx)),
                ))
                .child(md_action(
                    "toolbar-md-code",
                    TOOLBAR_ICON_MD_CODE,
                    Box::new(|r, cx| r.md_toggle_code(cx)),
                ))
                .child(md_action(
                    "toolbar-md-bullet",
                    TOOLBAR_ICON_MD_BULLET_LIST,
                    Box::new(|r, cx| r.md_toggle_line_prefix("- ", false, cx)),
                ))
                .child(md_action(
                    "toolbar-md-numbered",
                    TOOLBAR_ICON_MD_NUMBERED_LIST,
                    Box::new(|r, cx| r.md_toggle_line_prefix("1. ", true, cx)),
                ))
                .child(md_action(
                    "toolbar-md-todo",
                    TOOLBAR_ICON_MD_TODO_LIST,
                    Box::new(|r, cx| r.md_toggle_task_list(cx)),
                ));

            let md_toggle = |id: &'static str, icon: &'static [u8], mode: MarkdownViewMode| {
                let is_active = md_mode == mode;
                let root_click = root.clone();
                div()
                    .id(id)
                    .w(px(28.))
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(8.))
                    .cursor_pointer()
                    .border_1()
                    .border_color(rgba(if is_active { t.border_color } else { 0x00000000 }))
                    .bg(rgba(if is_active { t.bg_darker } else { t.bg_primary }))
                    .hover(|s| s.bg(rgba(t.bg_darker)))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        let _ = root_click.update(cx, |root, cx| {
                            root.toggle_markdown_mode(mode, cx);
                        });
                        cx.stop_propagation();
                    })
                    .child(
                        svg()
                            .data(icon)
                            .w(px(17.))
                            .h(px(17.))
                            .text_color(rgba(if is_active { t.text_primary } else { t.text_secondary })),
                    )
            };

            bar = bar.child(
                div()
                    .absolute()
                    .right(px(8.))
                    .top_0()
                    .bottom_0()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.))
                    .child(md_toggle("toolbar-md-split", TOOLBAR_ICON_MD_SPLIT, MarkdownViewMode::Split))
                    .child(md_toggle("toolbar-md-stacked", TOOLBAR_ICON_MD_STACKED, MarkdownViewMode::Stacked)),
            );
        } else if is_svg {
            let root_click = root.clone();
            bar = bar.child(
                div()
                    .absolute()
                    .right(px(8.))
                    .top_0()
                    .bottom_0()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.))
                    .child(
                        div()
                            .id("toolbar-svg-split")
                            .w(px(28.))
                            .h(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(8.))
                            .cursor_pointer()
                            .border_1()
                            .border_color(rgba(if svg_split {
                                t.border_color
                            } else {
                                0x00000000
                            }))
                            .bg(rgba(if svg_split { t.bg_darker } else { t.bg_primary }))
                            .hover(|s| s.bg(rgba(t.bg_darker)))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                let _ = root_click.update(cx, |root, cx| {
                                    root.toggle_svg_split_view(cx);
                                });
                                cx.stop_propagation();
                            })
                            .child(
                                svg()
                                    .data(TOOLBAR_ICON_MD_SPLIT)
                                    .w(px(17.))
                                    .h(px(17.))
                                    .text_color(rgba(if svg_split {
                                        t.text_primary
                                    } else {
                                        t.text_secondary
                                    })),
                            ),
                    ),
            );
        }

        // (shapes dropdown is now anchored to the shapes button above)

        bar.into_any_element()
    }
}

/// How many thickness steps the pen and the highlighter offer. One step is
/// one pixel of thickness.
const INK_PEN_STEPS: usize = 10;
const INK_HIGHLIGHTER_STEPS: usize = 20;

/// Colour chip geometry: 18px coloured chip (with 1px coloured border) inside
/// a 2px outer ring (1px transparent gap + 1px active/invisible border) = 22px.
const INK_CHIP_INNER: f32 = 18.0;
const INK_CHIP_OUTER: f32 = INK_CHIP_INNER + 4.0;
const INK_CHIP_GAP: f32 = 4.0;
const INK_POPUP_PADDING: f32 = 10.0;
/// Content width matches 8 colour chips + 7 gaps exactly (204px).
const INK_CONTENT_WIDTH: f32 = INK_CHIP_OUTER * 8.0 + INK_CHIP_GAP * 7.0;
/// Right controls column width (palette + slider + symmetric padding = 224px).
const INK_CONTROLS_WIDTH: f32 = INK_CONTENT_WIDTH + INK_POPUP_PADDING * 2.0;
/// Left wavy stroke preview panel (~40% of the controls width).
const INK_PREVIEW_PANEL_WIDTH: f32 = 90.0;
/// Dotted divider width between the left preview panel and the controls.
const INK_DIVIDER_WIDTH: f32 = 1.0;
/// Total popup width: left preview + 1px divider + right controls + 2px modal border.
const INK_POPUP_WIDTH: f32 =
    INK_PREVIEW_PANEL_WIDTH + INK_DIVIDER_WIDTH + INK_CONTROLS_WIDTH + 2.0;
/// Height of the slider.
const INK_SLIDER_HEIGHT: f32 = 26.0;
/// Space between the slider's outline and the fill inside it.
const INK_SLIDER_PADDING: f32 = 3.0;
/// Width reserved for the thickness value + "px" suffix.
const INK_VALUE_WIDTH: f32 = 38.0;
/// Width of the slider handle.
const INK_TOGGLE_WIDTH: f32 = 4.0;
/// Extra room around the handle so it is easy to grab.
const INK_TOGGLE_GRAB: f32 = 5.0;

/// Compute the 1px border colour for a colour chip: darker in light mode,
/// lighter in dark mode.
fn chip_border_color(rgb: u32, is_dark: bool) -> u32 {
    let r = ((rgb >> 16) & 0xff) as f32;
    let g = ((rgb >> 8) & 0xff) as f32;
    let b = (rgb & 0xff) as f32;
    let (nr, ng, nb) = if is_dark {
        const LIGHTEN: f32 = 0.25;
        (
            (r + (255.0 - r) * LIGHTEN).round() as u32,
            (g + (255.0 - g) * LIGHTEN).round() as u32,
            (b + (255.0 - b) * LIGHTEN).round() as u32,
        )
    } else {
        const DARKEN: f32 = 0.22;
        (
            (r * (1.0 - DARKEN)).round() as u32,
            (g * (1.0 - DARKEN)).round() as u32,
            (b * (1.0 - DARKEN)).round() as u32,
        )
    };
    (nr.min(255) << 24) | (ng.min(255) << 16) | (nb.min(255) << 8) | 0xff
}

/// Apply smooth magnetic tension around each integer pixel tick while dragging.
/// Strictly monotonic and C∞ smooth: slows near integer ticks and glides over
/// midpoints with no abrupt stepping.
fn apply_tick_magnet(raw: f32, max_step: f32) -> f32 {
    let clamped = raw.clamp(1.0, max_step);
    let d = clamped - clamped.round();
    const STRENGTH: f32 = 0.45;
    let pull = (d * std::f32::consts::TAU).sin() / std::f32::consts::TAU;
    (clamped - STRENGTH * pull).clamp(1.0, max_step)
}

/// The colour and size settings of the pen or the highlighter.
#[allow(clippy::too_many_arguments)]
fn ink_settings_popup(
    t: &crate::theme::Theme,
    is_dark: bool,
    root: WeakEntity<RootView>,
    tool: u8,
    color: u32,
    size: f32,
    handle: f32,
    progress: f32,
) -> impl IntoElement {
    let steps = if tool == 2 {
        INK_HIGHLIGHTER_STEPS
    } else {
        INK_PEN_STEPS
    };
    let max_step = steps as f32;
    let thickness = size.round().clamp(1.0, max_step);
    // The handle tracks magnetic dragging and eases smoothly on release.
    let handle = if handle > 0.0 {
        handle.clamp(1.0, max_step)
    } else {
        thickness
    };

    // The geometry of the slider, matched to the exact width of the palette.
    let slider_width = INK_CONTENT_WIDTH;
    let track_left = 1.0 + INK_SLIDER_PADDING;
    let value_left = slider_width - 8.0 - INK_VALUE_WIDTH;
    // The handle stops before the number, so the two never collide.
    let travel = (value_left - 6.0 - INK_TOGGLE_WIDTH - track_left).max(1.0);
    let fraction = if steps > 1 {
        (handle - 1.0) / (steps - 1) as f32
    } else {
        0.0
    };
    let toggle_x = track_left + fraction * travel;

    // The slider needs to know where its track starts on screen, which only
    // the layout knows. A tiny canvas reports it during prepaint.
    let track_origin: Rc<Cell<f32>> = Rc::new(Cell::new(0.0));
    let probe_origin = track_origin.clone();
    let probe = canvas(
        move |bounds: Bounds<Pixels>, _window, _app| {
            probe_origin.set(bounds.origin.x.as_f32());
        },
        |_bounds, _unit, _window, _app| {},
    )
    .absolute()
    .left(px(0.))
    .top(px(0.))
    .w(px(1.))
    .h(px(1.));

    let root_down = root.clone();
    let origin_down = track_origin.clone();
    let origin_move = track_origin.clone();
    let root_move = root.clone();
    let root_up = root.clone();

    let slider = div()
        .id("ink-size-track")
        .relative()
        .w(px(slider_width))
        .h(px(INK_SLIDER_HEIGHT))
        .border_1()
        .border_color(rgba(t.component_border_color))
        .rounded(px(8.))
        .on_mouse_down(MouseButton::Left, move |ev, _window, cx| {
            let raw = INK_SIZE_MIN
                + ((ev.position.x.as_f32() - origin_down.get() - track_left) / travel).clamp(0.0, 1.0)
                    * (max_step - 1.0);
            let value = apply_tick_magnet(raw, max_step);
            let _ = root_down.update(cx, |root, cx| {
                root.ink_slider_dragging = true;
                root.drag_ink_size(tool, value);
                cx.notify();
            });
            cx.stop_propagation();
        })
        .on_mouse_move(move |ev, _window, cx| {
            let dragging = root_move
                .upgrade()
                .map(|r| r.read(cx).ink_slider_dragging)
                .unwrap_or(false);
            if !dragging {
                return;
            }
            let raw = INK_SIZE_MIN
                + ((ev.position.x.as_f32() - origin_move.get() - track_left) / travel).clamp(0.0, 1.0)
                    * (max_step - 1.0);
            let value = apply_tick_magnet(raw, max_step);
            let _ = root_move.update(cx, |root, cx| {
                root.drag_ink_size(tool, value);
                cx.notify();
            });
        })
        .on_mouse_up(MouseButton::Left, move |_ev, _window, cx| {
            let _ = root_up.update(cx, |root, cx| {
                root.release_ink_slider(cx);
            });
        })
        // The filled part of the track, up to the handle.
        .child(
            div()
                .absolute()
                .left(px(track_left))
                .top(px(1.0 + INK_SLIDER_PADDING))
                .bottom(px(1.0 + INK_SLIDER_PADDING))
                .w(px((toggle_x - 2.0 - track_left).max(0.0)))
                .rounded(px(5.))
                .bg(rgba(t.border_color)),
        )
        // The handle, with a wider area around it to grab.
        .child(
            div()
                .id("ink-toggle-handle")
                .group("ink-toggle")
                .absolute()
                .left(px(toggle_x - INK_TOGGLE_GRAB))
                .top_0()
                .bottom_0()
                .w(px(INK_TOGGLE_WIDTH + INK_TOGGLE_GRAB * 2.0))
                .cursor_pointer()
                .child(
                    div()
                        .group_hover("ink-toggle", |style| {
                            style.bg(rgba(t.empty_text_primary))
                        })
                        .absolute()
                        .left(px(INK_TOGGLE_GRAB))
                        .top(px(2.))
                        .bottom(px(2.))
                        .w(px(INK_TOGGLE_WIDTH))
                        .rounded(px(3.))
                        .bg(rgba(t.text_secondary)),
                ),
        )
        // The thickness with "px" suffix, always at the right of the slider.
        .child(
            div()
                .absolute()
                .left(px(value_left))
                .top_0()
                .bottom_0()
                .w(px(INK_VALUE_WIDTH))
                .flex()
                .items_center()
                .justify_end()
                .whitespace_nowrap()
                .text_sm()
                .font_weight(WEIGHT_MEDIUM)
                .text_color(rgba(t.empty_text_primary))
                .child(format!("{}px", thickness as i32)),
        )
        .child(probe);

    let swatch = |root: WeakEntity<RootView>, tool: u8, value: u32, selected: bool| {
        let chip_border = chip_border_color(value, is_dark);
        div()
            .id(format!("ink-swatch-{tool}-{value}"))
            .w(px(INK_CHIP_OUTER))
            .h(px(INK_CHIP_OUTER))
            .p(px(1.))
            .rounded(px(6.))
            .border_1()
            .border_color(rgba(if selected { t.text_secondary } else { 0x00000000 }))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                let _ = root.update(cx, |root, cx| {
                    root.set_ink_color(tool, value);
                    cx.notify();
                });
                cx.stop_propagation();
            })
            .child(
                div()
                    .w(px(INK_CHIP_INNER))
                    .h(px(INK_CHIP_INNER))
                    .rounded(px(5.))
                    .border_1()
                    .border_color(rgba(chip_border))
                    .bg(rgba((value << 8) | 0xff)),
            )
    };

    let mut palette = div().flex().flex_col().gap(px(INK_CHIP_GAP));
    for row in INK_PALETTE.chunks(8) {
        let mut line = div().flex().flex_row().gap(px(INK_CHIP_GAP));
        for value in row {
            line = line.child(swatch(root.clone(), tool, *value, *value == color));
        }
        palette = palette.child(line);
    }

    // Wavy vector stroke preview on the left panel, rendered with the exact
    // same `ink::ribbon` + `ink::fill` pipeline used on the image canvas.
    let preview_alpha = {
        let base = if tool == 2 { 0x78u32 } else { 0xffu32 };
        ((base as f32) * progress).round().clamp(0.0, 255.0) as u32
    };
    let preview_rgba = (color << 8) | preview_alpha;
    let square_cap = tool == 2;
    let preview_canvas = canvas(
        |_bounds: Bounds<Pixels>, _window, _app| {},
        move |bounds: Bounds<Pixels>, _unit, window, _app| {
            let ox = bounds.origin.x.as_f32();
            let oy = bounds.origin.y.as_f32();
            let w = bounds.size.width.as_f32();
            let h = bounds.size.height.as_f32();
            let pad_y = 14.0;
            let span_y = (h - pad_y * 2.0).max(8.0);
            let cx = ox + w * 0.5;
            let amp = (w * 0.22).min(18.0);
            const SAMPLES: usize = 32;
            let mut wave: Vec<(f32, f32)> = Vec::with_capacity(SAMPLES + 1);
            for i in 0..=SAMPLES {
                let u = i as f32 / SAMPLES as f32;
                let x = cx + (u * std::f32::consts::TAU).sin() * amp;
                let y = oy + pad_y + u * span_y;
                wave.push((x, y));
            }
            let poly = ink::ribbon(&wave, handle * 0.5, square_cap);
            if let Some(path) = ink::fill(&[poly]) {
                window.paint_path(path, rgba(preview_rgba));
            }
        },
    )
    .w_full()
    .h_full();

    let preview_panel = div()
        .w(px(INK_PREVIEW_PANEL_WIDTH))
        .flex_shrink_0()
        .overflow_hidden()
        .child(preview_canvas);

    // Subtle vertical dotted divider aligned right at the original modal left
    // edge (x = 0 relative to the tool button).
    let dot_alpha = (((t.component_border_color & 0xff) as f32) * progress)
        .round()
        .clamp(0.0, 255.0) as u32;
    let dot_rgba = (t.component_border_color & 0xffffff00) | dot_alpha;
    let dotted_divider = div()
        .w(px(INK_DIVIDER_WIDTH))
        .flex_shrink_0()
        .child(
            canvas(
                |_bounds: Bounds<Pixels>, _window, _app| {},
                move |bounds: Bounds<Pixels>, _unit, window, _app| {
                    let x = bounds.origin.x.as_f32();
                    let top = bounds.origin.y.as_f32() + 8.0;
                    let bottom = bounds.origin.y.as_f32() + bounds.size.height.as_f32() - 8.0;
                    let mut y = top;
                    while y + 1.5 <= bottom {
                        window.paint_quad(
                            gpui::fill(
                                Bounds::new(point(px(x), px(y)), gpui_size(px(1.0), px(2.0))),
                                rgba(dot_rgba),
                            )
                            .corner_radii(px(0.5)),
                        );
                        y += 5.0;
                    }
                },
            )
            .w_full()
            .h_full(),
        );

    let controls_panel = div()
        .w(px(INK_CONTROLS_WIDTH))
        .flex_shrink_0()
        .flex()
        .flex_col()
        .gap(px(8.))
        .p(px(INK_POPUP_PADDING))
        .child(palette)
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .ml(px(INK_SLIDER_PADDING))
                        .text_xs()
                        .text_color(rgba(t.empty_text_primary))
                        .child("Thickness"),
                )
                .child(slider),
        );

    let popup_top = 30.0;
    let popup_left = -(INK_PREVIEW_PANEL_WIDTH + 1.0);

    deferred(
        div()
            .absolute()
            .top(px(popup_top))
            .left(px(popup_left))
            .w(px(INK_POPUP_WIDTH))
            .opacity(progress)
            .occlude()
            .flex()
            .flex_row()
            .items_stretch()
            .bg(rgba(t.bg_primary))
            .border_1()
            .border_color(rgba(t.component_border_color))
            .rounded(px(10.))
            .shadow(vec![t.shadow_lg()])
            .on_mouse_down(MouseButton::Left, |_ev, _window, cx| cx.stop_propagation())
            .child(preview_panel)
            .child(dotted_divider)
            .child(controls_panel),
    )
    .with_priority(1000)
}

// ---------- Bottom bar ----------// ---------- Bottom bar ----------// ---------- Bottom bar ----------// ---------- Bottom bar ----------

#[derive(IntoElement)]
pub struct BottomBar {
    pub root: WeakEntity<RootView>,
}

impl gpui::RenderOnce for BottomBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl gpui::IntoElement {
        let t = *crate::theme::active(cx);
        let root = self.root.clone();

        div()
            .id("bottom-bar")
            .w_full()
            .h(px(BOTTOM_BAR_HEIGHT))
            .flex()
            .flex_row()
            .items_center()
            .px(px(6.))
            .bg(rgba(t.bg_secondary))
            .border_t_1()
            .border_color(rgba(t.chrome_border_color))
            .children(self.root.upgrade().and_then(|root| {
                let left_occ = root.read(cx).left_occupancy();
                let status = root.read(cx).bottom_status();
                status.map(|items| {
                    let mut row = div()
                        .ml(px(left_occ))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(4.));
                    for (i, item) in items.iter().enumerate() {
                        if i > 0 {
                            row = row.child(tab_divider(t.chrome_border_color));
                        }
                        row = row.child(
                            div()
                                .text_xs()
                                .font_weight(WEIGHT_MEDIUM)
                                .text_color(rgba(t.empty_text_primary))
                                .child(item.clone()),
                        );
                    }
                    row
                })
            }).into_iter())
            .child(div().flex_1())
    }
}

/// The dock owns a second bottom bar so file-type tools do not compete with
/// the application-wide status controls.
#[derive(IntoElement)]
pub struct SidebarBottomBar {
    pub root: WeakEntity<RootView>,
}

impl gpui::RenderOnce for SidebarBottomBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl gpui::IntoElement {
        let t = *crate::theme::active(cx);
        let pdf_active = self.root.upgrade().is_some_and(|root| root.read(cx).panel_visible(PanelKind::PdfPages));
        let is_pdf = self.root.upgrade().is_some_and(|root| root.read(cx).active_is_pdf());
        let root = self.root.clone();

        div()
            .id("sidebar-bottom-bar")
            .w_full()
            .h(px(BOTTOM_BAR_HEIGHT))
            .flex()
            .flex_row()
            .items_center()
            .px(px(6.))
            .bg(rgba(t.bg_secondary))
            .border_t_1()
            .border_color(rgba(t.chrome_border_color))
            .when(is_pdf, |bar| bar.child(
                div()
                    .id("sidebar-pdf-toggle")
                    .w(px(24.))
                    .h(px(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(5.))
                    .cursor_pointer()
                    .when(pdf_active, |button| button.bg(rgba(t.bg_tertiary)))
                    .hover(|button| button.bg(rgba(t.bg_tertiary)))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        let _ = root.update(cx, |root, cx| root.toggle_panel(PanelKind::PdfPages, cx));
                        cx.stop_propagation();
                    })
                    .child(svg().data(ICON_PDF).w(px(15.)).h(px(15.)).text_color(rgba(t.text_secondary))),
            ))
    }
}
