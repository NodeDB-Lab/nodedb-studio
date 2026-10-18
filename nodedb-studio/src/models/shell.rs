//! Shell chrome models: nav-rail badge counts and the statusbar's session
//! summary. Both are single values (not lists), which is why the mock impl
//! cannot use `services::mock_behavior::apply` and instead uses
//! `apply_one`, the single-value counterpart `record_detail` also uses.

use serde::{Deserialize, Serialize};

/// Badge counts shown on the nav rail's Query and Streams entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavBadges {
    pub query: u32,
    pub streams: u32,
}

/// Server facts about the active session that the connect result does not
/// carry, shown in the statusbar. Role, current database and read-only stay
/// on `ActiveConnection`: one owner per fact, so the statusbar and the
/// avatar popover cannot show two different roles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    pub server_version: String,
    pub timezone: String,
}
