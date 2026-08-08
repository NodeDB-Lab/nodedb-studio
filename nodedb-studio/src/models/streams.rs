//! Streams-tier models beyond CDC (`models::cdc`): the consumer-group session
//! plus materialized views, durable topics, scheduled jobs and LISTEN/NOTIFY.
//! String fields mirror the wire, same convention as `models::admin`.

use serde::{Deserialize, Serialize};

/// A Studio-owned CDC consumer session. Studio never shares a consumer group:
/// committing on someone else's group would advance a production consumer past
/// events it never processed.
#[allow(dead_code)] // SEAM-UNWIRED(task-10)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamSession {
    pub stream: String,
    pub group: String,
}

/// One materialized view.
#[allow(dead_code)] // SEAM-UNWIRED(task-10)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaterializedView {
    pub id: String,
    pub name: String,
    pub source: String,
    pub refresh_mode: String,
    pub rows: String,
}

/// One durable, replayable topic.
#[allow(dead_code)] // SEAM-UNWIRED(task-10)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Topic {
    pub id: String,
    pub name: String,
    pub partitions: String,
    pub messages: String,
    pub retention: String,
    pub consumers: String,
    pub lag: String,
}

/// One cron-style scheduled job.
#[allow(dead_code)] // SEAM-UNWIRED(task-10)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledJob {
    pub id: String,
    pub name: String,
    pub cron: String,
    pub last_status: String,
    pub next_run: String,
}

/// One LISTEN/NOTIFY channel.
#[allow(dead_code)] // SEAM-UNWIRED(task-10)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotifyChannel {
    pub id: String,
    pub name: String,
    pub subscribers: String,
}

/// One message on the LISTEN/NOTIFY tail.
#[allow(dead_code)] // SEAM-UNWIRED(task-10)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotifyMessage {
    pub id: String,
    pub channel: String,
    pub at: String,
    pub payload_json: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_session_carries_stream_and_studio_group() {
        let s = StreamSession {
            stream: "cdc".to_string(),
            group: "studio_cdc".to_string(),
        };
        assert_eq!(s.stream, "cdc");
        assert_eq!(s.group, "studio_cdc");
    }
}
