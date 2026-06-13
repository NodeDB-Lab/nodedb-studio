//! Live UI state: the active connection, the saved-connection registry,
//! notifications, and preferences. Provided as fine-grained Dioxus signals at
//! the app root (see `crate::app`), never as global statics.

pub mod connection;
pub mod connections_registry;
pub mod notifications;
pub mod preferences;
pub mod ui;
