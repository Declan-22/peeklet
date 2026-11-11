//! Window chrome: top bar (hamburger menu + history + tabs + window controls), bottom bar
//! (placeholder for per-file-type controls).

use gpui::{
    AnyElement, App, MouseButton, WeakEntity, Window, WindowControlArea, canvas, deferred, div,
    prelude::*, px, rgba, svg,
};

use super::panels::{ICON_INFO, ICON_PDF, PanelKind};
use super::root::{RootView, Tab};
use crate::theme::WEIGHT_MEDIUM;

pub const TOP_BAR_HEIGHT: f32 = 36.0;
pub const BOTTOM_BAR_HEIGHT: f32 = 30.0;
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
    let control = |id: &'static str, area: WindowControlArea, content: AnyElement| -> AnyElement {
        div()
            .id(id)
            .w(px(46.))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .window_control_area(area)
            .hover(|s| s.bg(rgba(t.menu_hover_bg)))
            .child(content)
            .into_any_element()
    };

    div()
        .flex()
        .flex_row()
        .items_stretch()
        .h_full()
        .child(control(
            "window-minimize",
            WindowControlArea::Min,
            div()
                .w(px(10.))
                .h(px(1.))
                .bg(rgba(t.text_secondary))
                .into_any_element(),
        ))
        .child(control(
            "window-maximize",
            WindowControlArea::Max,
            div()
                .size(px(9.))
                .border_1()
                .border_color(rgba(t.text_secondary))
                .into_any_element(),
        ))
        .child(control(
            "window-close",
            WindowControlArea::Close,
            div()
                .text_sm()
                .text_color(rgba(t.text_primary))
                .child("✕")
                .into_any_element(),
        ))
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
            .flex_grow(2.0)
            .flex_shrink_1()
            .flex_basis(px(0.))
            .min_w(px(0.))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_row()
            .items_stretch()
            .bg(rgba(0x00000000))
            .child(
                div()
                    .id("tab-viewport")
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
                    ),
            )
    }
}

// ---------- Bottom bar ----------

#[derive(IntoElement)]
pub struct BottomBar {
    pub root: WeakEntity<RootView>,
}

impl gpui::RenderOnce for BottomBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl gpui::IntoElement {
        let t = *crate::theme::active(cx);
        let info_active = self
            .root
            .upgrade()
            .is_some_and(|root| root.read(cx).panel_visible(PanelKind::Info));
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
                status.map(|(cursor, chars, file_type, size)| {
                    div()
                        .ml(px(left_occ))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(4.))
                        .child(div().text_xs().font_weight(WEIGHT_MEDIUM).text_color(rgba(t.empty_text_primary)).child(cursor))
                        .child(tab_divider(t.chrome_border_color))
                        .child(div().text_xs().font_weight(WEIGHT_MEDIUM).text_color(rgba(t.empty_text_primary)).child(format!("{} chars", chars)))
                        .child(tab_divider(t.chrome_border_color))
                        .child(div().text_xs().font_weight(WEIGHT_MEDIUM).text_color(rgba(t.empty_text_primary)).child(file_type))
                        .child(tab_divider(t.chrome_border_color))
                        .child(div().text_xs().font_weight(WEIGHT_MEDIUM).text_color(rgba(t.empty_text_primary)).child(size))
                })
            }).into_iter())
            .child(div().flex_1())
            .child(
                div()
                    .id("bottom-info-toggle")
                    .w(px(24.))
                    .h(px(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(5.))
                    .cursor_pointer()
                    .when(info_active, |button| button.bg(rgba(t.bg_tertiary)))
                    .hover(|button| button.bg(rgba(t.bg_tertiary)))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        let _ = root.update(cx, |root, cx| {
                            root.toggle_panel(PanelKind::Info, cx);
                        });
                        cx.stop_propagation();
                    })
                    .child(
                        svg()
                            .data(ICON_INFO)
                            .w(px(15.))
                            .h(px(15.))
                            .text_color(rgba(t.text_secondary)),
                    ),
            )
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
