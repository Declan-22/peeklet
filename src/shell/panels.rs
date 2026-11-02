//! Panel registry: every sidebar panel, dockable in either dock.
//!
//! Only the file tree survives from the manuscript era. The file-preview
//! panels (info, image tools, PDF page strip, color palette) land here as
//! the viewers for each file type are built.

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PanelKind {
    /// Tree of files on disk; clicking a file previews it.
    Files,
    /// Metadata for the active file.
    Info,
    PdfPages,
}

impl PanelKind {
    /// Every registered panel. The bottom bar and the file-type viewers
    /// both enumerate panels through this list.
    #[allow(dead_code)]
    pub fn all() -> [PanelKind; 3] {
        [PanelKind::Files, PanelKind::Info, PanelKind::PdfPages]
    }

    /// Stable storage key for persistence.
    pub fn key(self) -> &'static str {
        match self {
            PanelKind::Files => "files",
            PanelKind::Info => "info",
            PanelKind::PdfPages => "pdf-pages",
        }
    }

    pub fn from_key(s: &str) -> Option<PanelKind> {
        match s {
            "files" => Some(PanelKind::Files),
            "info" => Some(PanelKind::Info),
            "pdf-pages" => Some(PanelKind::PdfPages),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DockSide {
    Left,
    Right,
}

impl PanelKind {
    #[allow(dead_code)]
    pub fn icon(self) -> &'static [u8] {
        match self {
            PanelKind::Files => ICON_FILES,
            PanelKind::Info => ICON_INFO,
            PanelKind::PdfPages => ICON_PDF,
        }
    }
}

pub const ICON_FILES: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 256 256">
	<path d="M0 0h256v256H0z" fill="none" />
	<path fill="currentColor" d="M176 150h32a14 14 0 0 0 14-14v-32a14 14 0 0 0-14-14h-32a14 14 0 0 0-14 14v10H86V78h10a14 14 0 0 0 14-14V32a14 14 0 0 0-14-14H64a14 14 0 0 0-14 14v32a14 14 0 0 0 14 14h10v114a22 22 0 0 0 22 22h66v10a14 14 0 0 0 14 14h32a14 14 0 0 0 14-14v-32a14 14 0 0 0-14-14h-32a14 14 0 0 0-14 14v10H96a10 10 0 0 1-10-10v-66h76v10a14 14 0 0 0 14 14M62 64V32a2 2 0 0 1 2-2h32a2 2 0 0 1 2 2v32a2 2 0 0 1-2 2H64a2 2 0 0 1-2-2m112 128a2 2 0 0 1 2-2h32a2 2 0 0 1 2 2v32a2 2 0 0 1-2 2h-32a2 2 0 0 1-2-2Zm0-88a2 2 0 0 1 2-2h32a2 2 0 0 1 2 2v32a2 2 0 0 1-2 2h-32a2 2 0 0 1-2-2Z" />
</svg>"#;

pub const ICON_INFO: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 256 256">
	<path d="M0 0h256v256H0z" fill="none" />
	<path fill="currentColor" d="M128 24a104 104 0 1 0 104 104A104.11 104.11 0 0 0 128 24m0 192a88 88 0 1 1 88-88a88.1 88.1 0 0 1-88 88m16-40a8 8 0 0 1-8 8a16 16 0 0 1-16-16v-40a8 8 0 0 1 0-16a16 16 0 0 1 16 16v40a8 8 0 0 1-8 8m-32-92a12 12 0 1 1 12 12a12 12 0 0 1-12-12" />
</svg>"#;

pub const ICON_PDF: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<path fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M6 3.5h7l5 5V20a.5.5 0 0 1-.5.5h-11A.5.5 0 0 1 6 20zM13 3.5V9h5M8.5 15.5v-3h1a1 1 0 0 1 0 2h-1m3 1v-3h1a1.5 1.5 0 0 1 0 3zm4-3v3m0-1.5h1.5" />
</svg>"#;
