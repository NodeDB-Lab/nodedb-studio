//! The Explorer's selected collection. Provided by `Studio` rather than owned
//! by the Explorer view, because the topbar breadcrumb names it too, and so a
//! pick survives leaving the Explorer and coming back.

use crate::models::collection::StorageMode;

/// The currently-selected collection (name + its storage mode).
#[derive(Clone, PartialEq)]
pub struct Selected {
    pub name: String,
    pub mode: StorageMode,
}
