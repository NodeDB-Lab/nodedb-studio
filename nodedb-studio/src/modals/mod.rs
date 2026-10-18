//! Modal content + the host that decides which modal is open.

pub mod confirm_delete;
pub mod entity_forms;
pub mod host;
pub mod new_connection;
pub mod preferences;

pub use host::ModalHost;
