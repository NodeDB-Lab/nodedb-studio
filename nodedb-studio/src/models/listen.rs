//! LISTEN/NOTIFY: transient messages on a named channel.
//!
//! A notification reaches only the sessions listening when it is sent. It has
//! no offset, no retention, and no replay, so none of these types carries
//! one. A topic is the durable alternative.

use serde::{Deserialize, Serialize};

use crate::models::json_text::json_issue;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotifyChannel {
    pub name: String,
    /// Sessions listening now, when the server reports it.
    pub listeners: Option<u32>,
}

/// Studio's listening session. Stopping it leaves other sessions alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListenSession {
    pub id: u32,
    pub channel: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotifyMessage {
    /// Unique within a session.
    pub id: String,
    /// `2026-06-13 13:42:18`, UTC.
    pub at: String,
    pub channel: String,
    /// The payload as sent.
    pub payload: String,
}

/// The most notifications Studio holds for display. The oldest drop first.
pub const DISPLAY_LIMIT: usize = 500;

/// Why a notification cannot be sent, if it cannot.
pub fn send_issue(channel: &str, payload: &str) -> Option<String> {
    if channel.trim().is_empty() {
        return Some("Choose a channel.".into());
    }
    json_issue(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_send_needs_a_channel_and_json() {
        assert_eq!(send_issue("", "{}").as_deref(), Some("Choose a channel."));
        assert_eq!(send_issue("user_events", "{\"event\": \"login\"}"), None);
        assert!(send_issue("user_events", "{").is_some());
    }
}
