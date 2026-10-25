//! Live UI state: the active connection, the saved-connection registry,
//! notifications, preferences, and the workspace layout. Provided as
//! fine-grained Dioxus signals at the app root (see `crate::app`), never as
//! global statics.

pub mod connection;
pub mod connection_form;
pub mod connections_registry;
pub mod console;
pub mod explorer;
pub mod notifications;
pub mod palette;
pub mod panel_registry;
pub mod persist;
pub mod preferences;
pub mod query;
pub mod record_editor;
pub mod ui;
pub mod workspace;
