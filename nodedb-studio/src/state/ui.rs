//! Ephemeral shell UI state shared across the studio chrome.
//!
//! These are not "local" to one component: the header popovers are mutually
//! exclusive and the Escape handler closes whichever is open, so a single
//! shared signal models "which overlay is open" better than several booleans.

use dioxus::prelude::Signal;

/// Which header popover or drawer is currently open (at most one at a time).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Popover {
    Connection,
    Database,
    Notifications,
    /// The panel visibility checklist beside the avatar.
    View,
    Avatar,
}

/// A Preferences pane. Carried by `ModalKind::Preferences` so a trigger can
/// open the modal at a specific pane (the avatar popover's "Keyboard
/// shortcuts" and "About" items) and the sidebar switches panes through the
/// same signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefsPane {
    Appearance,
    Editor,
    Keyboard,
    Security,
    Telemetry,
    About,
}

impl PrefsPane {
    /// Sidebar order.
    pub const ALL: [PrefsPane; 6] = [
        PrefsPane::Appearance,
        PrefsPane::Editor,
        PrefsPane::Keyboard,
        PrefsPane::Security,
        PrefsPane::Telemetry,
        PrefsPane::About,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PrefsPane::Appearance => "Appearance",
            PrefsPane::Editor => "Editor",
            PrefsPane::Keyboard => "Keyboard shortcuts",
            PrefsPane::Security => "Security",
            PrefsPane::Telemetry => "Telemetry",
            PrefsPane::About => "About",
        }
    }
}

/// Which modal is currently open. Preferences and the connection dialog are
/// reachable in either app state. The connection dialog reads its fields from
/// the shared `ConnectionForm`, which its opener fills. The record editor and
/// the delete confirmation read theirs from `RecordEditor` and
/// `DeleteRequest`, filled the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalKind {
    NewConnection,
    Preferences(PrefsPane),
    RecordEditor,
    ConfirmDelete,
}

/// Whether the Connection Manager shows while a session is open. The studio
/// stays mounted underneath, so its tabs and selections survive a visit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ManagerOpen(pub bool);

/// The connection the manager highlights. `None` highlights the first entry.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ManagerSelection(pub Option<String>);

/// The command palette's search text. It lives above the palette so a
/// screenshot scene can open the palette with a query typed.
#[derive(Clone, Copy)]
pub struct PaletteQuery(pub Signal<String>);
