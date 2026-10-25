//! Change feeds: a database object that captures the changes of one
//! collection and delivers them to consumers.
//!
//! A feed's state is its server delivery. An event's delivery status belongs
//! to one consumer, and several consumers can acknowledge the same event at
//! different times. Studio's tail is a separate observation session and never
//! changes either.

use serde::{Deserialize, Serialize};

/// A data change a feed can capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeOp {
    Insert,
    Update,
    Delete,
}

impl ChangeOp {
    pub const ALL: [ChangeOp; 3] = [ChangeOp::Insert, ChangeOp::Update, ChangeOp::Delete];

    /// The uppercase name an event row and a definition show.
    pub fn label(self) -> &'static str {
        match self {
            ChangeOp::Insert => "INSERT",
            ChangeOp::Update => "UPDATE",
            ChangeOp::Delete => "DELETE",
        }
    }

    /// The name a form shows.
    pub fn title(self) -> &'static str {
        match self {
            ChangeOp::Insert => "Insert",
            ChangeOp::Update => "Update",
            ChangeOp::Delete => "Delete",
        }
    }
}

/// Whether the server is delivering a feed's events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedState {
    /// Delivering. Individual events can still be retrying.
    Running,
    /// A person paused delivery. Events accumulate until it resumes.
    Paused,
    /// Delivery stopped on an error the feed reports.
    Error,
}

impl FeedState {
    pub fn label(self) -> &'static str {
        match self {
            FeedState::Running => "Running",
            FeedState::Paused => "Paused",
            FeedState::Error => "Error",
        }
    }

    /// The `status-dot` and `badge` tone.
    pub fn tone(self) -> Tone {
        match self {
            FeedState::Running => Tone::Ok,
            FeedState::Paused => Tone::Warn,
            FeedState::Error => Tone::Err,
        }
    }
}

/// A status color. The word always accompanies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Ok,
    Warn,
    Err,
    Idle,
}

impl Tone {
    pub fn dot(self) -> &'static str {
        match self {
            Tone::Ok => "status-dot ok",
            Tone::Warn => "status-dot warn",
            Tone::Err => "status-dot err",
            Tone::Idle => "status-dot idle",
        }
    }

    pub fn badge(self) -> &'static str {
        match self {
            Tone::Ok => "badge green",
            Tone::Warn => "badge amber",
            Tone::Err => "badge red",
            Tone::Idle => "badge gray",
        }
    }
}

/// How a feed reaches its consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeliveryKind {
    /// The server posts each event to an HTTPS endpoint.
    Webhook,
    /// A client subscribes and pulls events.
    ClientSubscription,
}

impl DeliveryKind {
    pub const ALL: [DeliveryKind; 2] = [DeliveryKind::Webhook, DeliveryKind::ClientSubscription];

    pub fn label(self) -> &'static str {
        match self {
            DeliveryKind::Webhook => "Webhook",
            DeliveryKind::ClientSubscription => "Client subscription",
        }
    }

    /// The consumer type a form offers.
    pub fn consumer_label(self) -> &'static str {
        match self {
            DeliveryKind::Webhook => "HTTPS webhook",
            DeliveryKind::ClientSubscription => "Client subscription",
        }
    }
}

/// The wait between delivery retries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Backoff {
    Fixed,
    Linear,
    Exponential,
}

impl Backoff {
    pub const ALL: [Backoff; 3] = [Backoff::Fixed, Backoff::Linear, Backoff::Exponential];

    pub fn label(self) -> &'static str {
        match self {
            Backoff::Fixed => "Fixed",
            Backoff::Linear => "Linear",
            Backoff::Exponential => "Exponential",
        }
    }
}

/// How long the server keeps events for replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetentionUnit {
    Hours,
    Days,
}

impl RetentionUnit {
    pub const ALL: [RetentionUnit; 2] = [RetentionUnit::Hours, RetentionUnit::Days];

    pub fn label(self) -> &'static str {
        match self {
            RetentionUnit::Hours => "Hours",
            RetentionUnit::Days => "Days",
        }
    }

    pub fn hours(self, n: u32) -> u32 {
        match self {
            RetentionUnit::Hours => n,
            RetentionUnit::Days => n * 24,
        }
    }
}

/// Where a new feed starts reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum StartPosition {
    /// New changes only.
    Now,
    /// Resume from a checkpoint a feed saved earlier.
    SavedCheckpoint,
    /// Backfill the existing data, then continue with new changes.
    #[default]
    SnapshotThenTail,
}

impl StartPosition {
    pub const ALL: [StartPosition; 3] = [
        StartPosition::Now,
        StartPosition::SavedCheckpoint,
        StartPosition::SnapshotThenTail,
    ];

    pub fn label(self) -> &'static str {
        match self {
            StartPosition::Now => "Now",
            StartPosition::SavedCheckpoint => "Saved checkpoint",
            StartPosition::SnapshotThenTail => "Snapshot then tail",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            StartPosition::Now => "Start from new changes only",
            StartPosition::SavedCheckpoint => "Resume from a previous checkpoint",
            StartPosition::SnapshotThenTail => {
                "Backfill existing data, then continue with new changes"
            }
        }
    }
}

/// One change feed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeFeed {
    /// Stable identity. A name can be reused after a delete, an id never is.
    pub id: String,
    pub name: String,
    pub database: String,
    pub collection: String,
    pub operations: Vec<ChangeOp>,
    /// A row filter, e.g. `store_id = 'nyc'`. Empty captures every row.
    pub filter: String,
    /// Fields of the document to deliver. Empty delivers the whole document.
    pub projection: String,
    /// Whether update and delete events carry the previous document.
    pub before_image: bool,
    pub delivery: DeliveryKind,
    /// The primary consumer's alias, e.g. `warehouse_sink`.
    pub consumer: String,
    /// The endpoint as shown: no scheme credentials, no query string.
    pub endpoint: Option<String>,
    pub state: FeedState,
    /// Why the feed is in `Error`.
    pub error: Option<String>,
    /// The feed's opaque checkpoint, shortened for display.
    pub checkpoint: String,
    pub backoff: Backoff,
    pub max_retries: u32,
    pub retention_hours: u32,
    pub start: StartPosition,
    /// Changes with every edit. An edit names the revision it read.
    pub revision: u64,
}

impl ChangeFeed {
    /// `sales`, or `analytics.sales` with the database.
    pub fn source(&self) -> String {
        format!("{}.{}", self.database, self.collection)
    }

    pub fn operations_label(&self) -> String {
        self.operations
            .iter()
            .map(|o| o.label())
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

/// Delivery of one event to one consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeliveryStatus {
    Acknowledged,
    RetryScheduled,
    Failed,
}

impl DeliveryStatus {
    pub fn label(self) -> &'static str {
        match self {
            DeliveryStatus::Acknowledged => "Acknowledged",
            DeliveryStatus::RetryScheduled => "Retry scheduled",
            DeliveryStatus::Failed => "Failed",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            DeliveryStatus::Acknowledged => Tone::Ok,
            DeliveryStatus::RetryScheduled => Tone::Warn,
            DeliveryStatus::Failed => Tone::Err,
        }
    }
}

/// One captured change. The payload keeps the source types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedEvent {
    /// Unique within its feed.
    pub id: String,
    pub received_at: String,
    pub op: ChangeOp,
    pub record_id: String,
    /// Delivery to the feed's primary consumer.
    pub delivery: DeliveryStatus,
    /// The event as pretty-printed JSON.
    pub payload: String,
}

/// The displayed events as JSON Lines: one object per event, the payload
/// kept as its own JSON. This exports what is on screen, not the feed's
/// retained range.
pub fn export_jsonl(events: &[FeedEvent]) -> String {
    #[derive(Serialize)]
    struct Line<'a> {
        received_at: &'a str,
        operation: &'a str,
        record_id: &'a str,
        delivery: &'a str,
        event: sonic_rs::Value,
    }
    events
        .iter()
        .filter_map(|e| {
            let event = sonic_rs::from_str::<sonic_rs::Value>(&e.payload).ok()?;
            sonic_rs::to_string(&Line {
                received_at: &e.received_at,
                operation: e.op.label(),
                record_id: &e.record_id,
                delivery: e.delivery.label(),
                event,
            })
            .ok()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A consumer's health.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsumerState {
    Active,
    Retrying,
    Paused,
    Failing,
}

impl ConsumerState {
    pub fn label(self) -> &'static str {
        match self {
            ConsumerState::Active => "Active",
            ConsumerState::Retrying => "Retrying",
            ConsumerState::Paused => "Paused",
            ConsumerState::Failing => "Failing",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            ConsumerState::Active => Tone::Ok,
            ConsumerState::Retrying | ConsumerState::Paused => Tone::Warn,
            ConsumerState::Failing => Tone::Err,
        }
    }
}

/// A consumer of one feed. Its checkpoint is scoped to that feed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedConsumer {
    pub id: String,
    pub name: String,
    pub kind: DeliveryKind,
    pub destination: String,
    pub checkpoint: String,
    /// When the consumer last acknowledged an event.
    pub last_ack: String,
    pub state: ConsumerState,
    /// Events waiting for this consumer. `None` when the server does not
    /// report it.
    pub pending: Option<u64>,
}

/// How one delivery attempt ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttemptOutcome {
    Acknowledged,
    Retry,
    Failed,
}

impl AttemptOutcome {
    pub fn label(self) -> &'static str {
        match self {
            AttemptOutcome::Acknowledged => "Acknowledged",
            AttemptOutcome::Retry => "Will retry",
            AttemptOutcome::Failed => "Failed",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            AttemptOutcome::Acknowledged => Tone::Ok,
            AttemptOutcome::Retry => Tone::Warn,
            AttemptOutcome::Failed => Tone::Err,
        }
    }
}

/// One try to deliver one event to one consumer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryAttempt {
    pub id: String,
    pub event_id: String,
    pub record_id: String,
    pub at: String,
    pub consumer: String,
    pub number: u32,
    pub outcome: AttemptOutcome,
    pub detail: String,
}

/// A Studio observation session on a feed. Studio reads under its own
/// consumer group and never advances a production consumer's checkpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedSession {
    pub feed: String,
    pub group: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operations_read_in_the_order_the_feed_lists_them() {
        let mut f = crate::data::mock::seeded_feeds().remove(0);
        f.operations = vec![ChangeOp::Insert, ChangeOp::Update, ChangeOp::Delete];
        assert_eq!(f.operations_label(), "INSERT · UPDATE · DELETE");
    }

    #[test]
    fn the_export_is_one_compact_object_per_event() {
        let events = crate::data::mock::FeedStore::seeded()
            .recent("feed_01")
            .expect("events");
        let out = export_jsonl(&events);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(
            lines[0].contains("\"record_id\":\"sale_1042\""),
            "{}",
            lines[0]
        );
        assert!(
            lines[0].contains("\"delivery\":\"Acknowledged\""),
            "{}",
            lines[0]
        );
        assert!(lines[0].contains("\"net_amount\":180"), "{}", lines[0]);
        assert_eq!(export_jsonl(&[]), "");
    }

    #[test]
    fn retention_converts_to_hours() {
        assert_eq!(RetentionUnit::Days.hours(7), 168);
        assert_eq!(RetentionUnit::Hours.hours(36), 36);
    }

    #[test]
    fn every_state_has_a_word_and_a_tone() {
        for s in [FeedState::Running, FeedState::Paused, FeedState::Error] {
            assert!(!s.label().is_empty());
            assert!(s.tone().dot().starts_with("status-dot"));
        }
    }
}
