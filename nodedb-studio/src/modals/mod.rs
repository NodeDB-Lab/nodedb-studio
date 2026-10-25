//! Modal content + the host that decides which modal is open.

pub mod confirm_delete;
pub mod connection_form;
pub mod host;
pub mod preferences;
pub mod record_editor;

pub use host::ModalHost;
