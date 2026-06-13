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

/// Which modal is currently open. Preferences is reachable in either app state;
/// New connection only while disconnected/connected via the relevant trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalKind {
    NewConnection,
    Preferences,
}
