//! Device enrollment: the Origin issues a one-use token that a new Lite
//! device presents. The token is a secret. Studio shows it masked and hands
//! the command to the clipboard once. It never renders the raw value.

use serde::{Deserialize, Serialize};

/// A subscription preset a device can start from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    /// The parameter the preset asks for, e.g. `store_id`, with its values.
    pub parameter: String,
    /// What the form calls it, e.g. `Store ID`.
    pub parameter_label: String,
    pub values: Vec<String>,
    /// The shapes the preset creates, for the preview line.
    pub shapes: Vec<String>,
}

/// What the principal may offer a new device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnrollOptions {
    pub origin: String,
    /// The secured destination the device is configured with.
    pub endpoint: String,
    /// The id the next device gets. The server assigns it.
    pub device_id: String,
    pub databases: Vec<String>,
    pub presets: Vec<Preset>,
    pub roles: Vec<String>,
    /// How long a token lives, in seconds.
    pub token_ttl: u64,
}

/// What the person typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnrollDraft {
    pub name: String,
    pub database: String,
    pub preset: String,
    pub value: String,
    pub role: String,
    /// Create token was pressed with a field missing.
    pub tried: bool,
    pub busy: bool,
    pub error: Option<String>,
    /// The token the server issued, once created.
    pub issued: Option<Enrollment>,
    /// The command reached the clipboard.
    pub copied: bool,
}

impl EnrollDraft {
    pub fn new(options: &EnrollOptions) -> Self {
        let preset = options.presets.first();
        Self {
            name: String::new(),
            database: options.databases.first().cloned().unwrap_or_default(),
            preset: preset.map(|p| p.name.clone()).unwrap_or_default(),
            value: preset
                .and_then(|p| p.values.first().cloned())
                .unwrap_or_default(),
            role: options.roles.first().cloned().unwrap_or_default(),
            tried: false,
            busy: false,
            error: None,
            issued: None,
            copied: false,
        }
    }

    /// A device name is a label: letters, digits, spaces, dots, hyphens.
    pub fn name_issue(&self) -> Option<String> {
        let name = self.name.trim();
        if name.is_empty() {
            Some("Enter a device name.".to_string())
        } else if name.len() > 48 {
            Some("Use 48 characters or fewer.".to_string())
        } else if !name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '.' | '-' | '_'))
        {
            Some("Use letters, digits, spaces, dots, hyphens, and underscores.".to_string())
        } else {
            None
        }
    }

    pub fn request(&self) -> EnrollRequest {
        EnrollRequest {
            name: self.name.trim().to_string(),
            database: self.database.clone(),
            preset: self.preset.clone(),
            value: self.value.clone(),
            role: self.role.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnrollRequest {
    pub name: String,
    pub database: String,
    pub preset: String,
    pub value: String,
    pub role: String,
}

/// The token the server issued. `masked` is all Studio ever shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Enrollment {
    pub device_id: String,
    pub name: String,
    pub masked: String,
    /// Seconds since the epoch.
    pub expires_at: i64,
    pub issued_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(name: &str) -> EnrollDraft {
        EnrollDraft {
            name: name.into(),
            database: "analytics".into(),
            preset: "store_orders".into(),
            value: "nyc".into(),
            role: "sync_checkout".into(),
            tried: false,
            busy: false,
            error: None,
            issued: None,
            copied: false,
        }
    }

    #[test]
    fn a_name_is_checked_before_a_token_is_made() {
        assert!(draft("").name_issue().is_some());
        assert!(draft("Checkout iPad").name_issue().is_none());
        assert!(draft("bad/name").name_issue().is_some());
        assert!(draft(&"x".repeat(49)).name_issue().is_some());
    }
}
