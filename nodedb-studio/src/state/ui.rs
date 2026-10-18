//! Ephemeral shell UI state shared across the studio chrome.
//!
//! These are not "local" to one component: the four popovers are mutually
//! exclusive and the Escape handler closes whichever is open, so a single
//! shared signal models "which overlay is open" better than four booleans.

/// Which topbar popover is currently open (at most one at a time).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Popover {
    Connection,
    Database,
    Notifications,
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
            PrefsPane::Keyboard => "Keyboard",
            PrefsPane::Security => "Security",
            PrefsPane::Telemetry => "Telemetry",
            PrefsPane::About => "About",
        }
    }
}

/// Which modal is currently open. Preferences is reachable in either app state;
/// New connection only while disconnected/connected via the relevant trigger.
/// The entity-form and confirm-delete variants carry no record yet: nothing
/// selects one until the Explorer's master-detail lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalKind {
    NewConnection,
    Preferences(PrefsPane),
    DocForm,
    StrictForm,
    VectorForm,
    GraphNodeForm,
    GraphEdgeForm,
    KvForm,
    SpatialForm,
    #[allow(dead_code)] // SEAM-UNWIRED: opened from the detail-panel footer (Phase 4)
    ConfirmDelete,
}
