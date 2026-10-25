//! Modal content + the host that decides which modal is open.

pub mod confirm_delete;
pub mod connection_form;
pub mod entity_forms;
pub mod host;
pub mod preferences;

pub use host::ModalHost;
