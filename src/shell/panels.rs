//! Panel registry: every sidebar panel, dockable in either dock.
//!
//! Only the file tree survives from the manuscript era. The file-preview
//! panels (info, image tools, PDF page strip, color palette) land here as
//! the viewers for each file type are built.

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PanelKind {
    /// Tree of files on disk; clicking a file previews it.
    Files,
}

impl PanelKind {
    /// Every registered panel. The bottom bar and the file-type viewers
    /// both enumerate panels through this list.
    #[allow(dead_code)]
    pub fn all() -> [PanelKind; 1] {
        [PanelKind::Files]
    }

    /// Stable storage key for persistence.
    pub fn key(self) -> &'static str {
        match self {
            PanelKind::Files => "files",
        }
    }

    pub fn from_key(s: &str) -> Option<PanelKind> {
        match s {
            "files" => Some(PanelKind::Files),
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
        }
    }
}

pub const ICON_FILES: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 256 256">
	<path d="M0 0h256v256H0z" fill="none" />
	<path fill="currentColor" d="M176 150h32a14 14 0 0 0 14-14v-32a14 14 0 0 0-14-14h-32a14 14 0 0 0-14 14v10H86V78h10a14 14 0 0 0 14-14V32a14 14 0 0 0-14-14H64a14 14 0 0 0-14 14v32a14 14 0 0 0 14 14h10v114a22 22 0 0 0 22 22h66v10a14 14 0 0 0 14 14h32a14 14 0 0 0 14-14v-32a14 14 0 0 0-14-14h-32a14 14 0 0 0-14 14v10H96a10 10 0 0 1-10-10v-66h76v10a14 14 0 0 0 14 14M62 64V32a2 2 0 0 1 2-2h32a2 2 0 0 1 2 2v32a2 2 0 0 1-2 2H64a2 2 0 0 1-2-2m112 128a2 2 0 0 1 2-2h32a2 2 0 0 1 2 2v32a2 2 0 0 1-2 2h-32a2 2 0 0 1-2-2Zm0-88a2 2 0 0 1 2-2h32a2 2 0 0 1 2 2v32a2 2 0 0 1-2 2h-32a2 2 0 0 1-2-2Z" />
</svg>"#;
