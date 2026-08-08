//! The single front door. `Backend` composes every per-topic data trait plus the
//! connection/session/notification trait, so the app provides ONE `Rc<dyn Backend>`
//! and a screen reads the one method it needs without knowing which trait owns it.
//!
//! Adding a new domain trait later means extending this bound and implementing the
//! trait on the mock + stub — additive, never a reshape of existing methods.

use crate::services::connection_service::ConnectionService;
use crate::services::explorer_data::ExplorerData;
use crate::services::streams_data::StreamsData;

pub trait Backend: ConnectionService + StreamsData + ExplorerData {}

impl<T: ConnectionService + StreamsData + ExplorerData> Backend for T {}
