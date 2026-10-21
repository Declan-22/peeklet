//! App actions + root handle for global dispatch.

use gpui::{Global, WeakEntity, actions};

use super::root::RootView;

actions!(
    peekview,
    [
        Quit,
        ToggleTheme,
        NewTab,
        OpenFile,
        OpenFolder,
        CloseTab,
        Undo,
        Redo,
        Save,
        ShowWelcome,
        ToggleFilesPanel,
    ]
);

/// Global handle so action handlers (which only get `&mut App`) can reach
/// the root view.
pub struct RootHandle(pub WeakEntity<RootView>);

impl Global for RootHandle {}
