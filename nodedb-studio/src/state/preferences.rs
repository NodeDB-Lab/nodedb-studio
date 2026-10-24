//! App-level preferences (theme, fonts, keyboard, telemetry).
//!
//! These are global to Studio, NOT per-connection, and live behind the
//! Preferences modal, never in the studio rail. Connection-scoped settings
//! belong with the connection; anything global to Studio belongs here.

use serde::{Deserialize, Serialize};

/// Color theme. `System` follows the OS preference, which is the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    System,
    Light,
    Dark,
}

impl Theme {
    /// Value of the root `data-theme` attribute. The stylesheet keys the dark
    /// tokens on it, and on the OS preference when it is `system`.
    pub fn attr(self) -> &'static str {
        match self {
            Theme::System => "system",
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }
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
