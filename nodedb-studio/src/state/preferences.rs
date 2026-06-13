//! App-level preferences (theme, fonts, keyboard, telemetry).
//!
//! These are global to Studio, NOT per-connection, and live behind the
//! Preferences modal — never in the studio rail. See CLAUDE.md
//! "Settings vs preferences".

use serde::{Deserialize, Serialize};

/// Color theme. `System` follows the OS preference, which is the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preferences {
    pub theme: Theme,
    /// Monospace font stack used by editors/grids.
    pub mono_font: String,
    /// Whether anonymous usage telemetry is enabled.
    pub telemetry: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            mono_font: "ui-monospace, SFMono-Regular, monospace".into(),
            telemetry: false,
        }
    }
}
