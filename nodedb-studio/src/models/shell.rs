//! Shell chrome models: nav-rail badge counts and the statusbar's session
//! summary. Both are single values (not lists), which is why the mock impl
//! cannot use `services::mock_behavior::apply` and instead uses
//! `apply_one`, the single-value counterpart `record_detail` also uses.

use serde::{Deserialize, Serialize};

/// Badge counts shown on the nav rail's Query and Streams entries.
#[allow(dead_code)] // SEAM-UNWIRED
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavBadges {
    pub query: u32,
    pub streams: u32,
}

/// The active session summary shown in the statusbar.
#[allow(dead_code)] // SEAM-UNWIRED
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    pub database: String,
    pub role: String,
    pub server_version: String,
    pub timezone: String,
    pub read_only: bool,
}
