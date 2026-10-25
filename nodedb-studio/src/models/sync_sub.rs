//! Shape subscriptions: what an Origin sends to one Lite device, and the
//! stage it has reached. A change to a subscription is previewed first. The
//! preview names the revision it read and the server refuses a stale apply.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::models::cluster_op::PermissionCheck;
use crate::models::feeds::Tone;
use crate::models::format::{clock, thousands};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeKind {
    /// The rows of one collection that match a filter.
    FilteredRecordSet,
}

impl ShapeKind {
    pub fn label(self) -> &'static str {
        match self {
            ShapeKind::FilteredRecordSet => "FilteredRecordSet",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            ShapeKind::FilteredRecordSet => {
                "The rows of one collection that match a filter. Origin sends them to the device."
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubState {
    Active,
    Paused,
    Bootstrapping,
    Error,
}

impl SubState {
    pub fn label(self) -> &'static str {
        match self {
            SubState::Active => "Active",
            SubState::Paused => "Paused",
            SubState::Bootstrapping => "Bootstrapping",
            SubState::Error => "Error",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            SubState::Active => Tone::Ok,
            SubState::Paused => Tone::Idle,
            SubState::Bootstrapping => Tone::Warn,
            SubState::Error => Tone::Err,
        }
    }
}

/// The initial snapshot, the first of the stages a subscription passes
/// through. Schema announcement comes before it, live changes after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Snapshot {
    Pending,
    Running { done: u64, total: u64 },
    Complete { rows: u64 },
}

impl Snapshot {
    pub fn label(self) -> String {
        match self {
            Snapshot::Pending => "Not started".to_string(),
            Snapshot::Running { done, total } => {
                format!("Running, {} of {} rows", thousands(done), thousands(total))
            }
            Snapshot::Complete { .. } => "Complete".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubEventKind {
    Activated,
    StartingSnapshot,
    ApplyingSnapshot,
    SnapshotComplete,
    CheckpointUpdated,
    ReceivedLive,
    AppliedLive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubEvent {
    /// Seconds since the epoch.
    pub at: i64,
    pub kind: SubEventKind,
    /// Rows for a batch or the snapshot, the checkpoint for an update.
    pub count: Option<u64>,
}

impl SubEvent {
    pub fn text(&self) -> String {
        let n = self.count.map(thousands).unwrap_or_default();
        match self.kind {
            SubEventKind::Activated => "Subscription activated".to_string(),
            SubEventKind::StartingSnapshot => "Starting initial snapshot".to_string(),
            SubEventKind::ApplyingSnapshot => "Applying snapshot data".to_string(),
            SubEventKind::SnapshotComplete => format!("Initial snapshot complete ({n} rows)"),
            SubEventKind::CheckpointUpdated => {
                format!("Checkpoint updated to {}", self.count.unwrap_or_default())
            }
            SubEventKind::ReceivedLive => format!("Received live changes ({n} rows)"),
            SubEventKind::AppliedLive => format!("Applied live changes ({n} rows)"),
        }
    }

    /// Steps still in flight show grey, finished ones green.
    pub fn tone(&self) -> Tone {
        match self.kind {
            SubEventKind::StartingSnapshot | SubEventKind::ApplyingSnapshot => Tone::Idle,
            _ => Tone::Ok,
        }
    }

    pub fn clock(&self) -> String {
        clock(self.at, 0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subscription {
    pub id: String,
    pub name: String,
    pub device_id: String,
    pub device_name: String,
    pub shape: ShapeKind,
    pub database: String,
    pub collection: String,
    /// The filter text, with `:name` parameters.
    pub filter: String,
    /// Typed parameters, in the order the filter names them.
    pub parameters: Vec<(String, String)>,
    pub permission_scope: String,
    /// The named bundle of shapes the subscription came from.
    pub preset: Option<String>,
    pub state: SubState,
    pub snapshot: Snapshot,
    pub checkpoint: Option<u64>,
    /// What the device reported, seconds since the epoch.
    pub last_received: Option<i64>,
    pub last_applied: Option<i64>,
    pub revision: u64,
    /// Newest first.
    pub events: Vec<SubEvent>,
}

/// The definition as the server stores it.
#[derive(Serialize)]
struct Definition<'a> {
    database: &'a str,
    collection: &'a str,
    filter: &'a str,
    parameters: BTreeMap<&'a str, &'a str>,
}

impl Subscription {
    pub fn source(&self) -> String {
        format!("{}.{}", self.database, self.collection)
    }

    /// The filter with each `:name` replaced by its quoted value, as the
    /// table shows it: `store_id = 'nyc'`.
    pub fn resolved_filter(&self) -> String {
        let mut out = self.filter.clone();
        for (name, value) in &self.parameters {
            out = out.replace(&format!(":{name}"), &format!("'{value}'"));
        }
        out
    }

    /// `d17 (checkout-ipad)`.
    pub fn device_label(&self) -> String {
        format!("{} ({})", self.device_id, self.device_name)
    }

    pub fn matches(&self, needle: &str) -> bool {
        needle.is_empty()
            || [
                &self.name,
                &self.device_id,
                &self.device_name,
                &self.collection,
            ]
            .iter()
            .any(|f| f.to_lowercase().contains(needle))
    }

    /// The definition for `change`, or the stored one, as indented JSON.
    pub fn definition_json(&self, change: Option<&SubChange>) -> String {
        let (filter, params) = match change {
            Some(c) => (c.filter.as_str(), &c.parameters),
            None => (self.filter.as_str(), &self.parameters),
        };
        let def = Definition {
            database: &self.database,
            collection: &self.collection,
            filter,
            parameters: params
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect(),
        };
        sonic_rs::to_string_pretty(&def).unwrap_or_default()
    }

    /// The change that leaves the definition as stored.
    pub fn as_change(&self) -> SubChange {
        SubChange {
            filter: self.filter.clone(),
            parameters: self.parameters.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subscriptions {
    pub items: Vec<Subscription>,
    pub observed_at: i64,
}

impl Subscriptions {
    pub fn get(&self, id: &str) -> Option<&Subscription> {
        self.items.iter().find(|s| s.id == id)
    }
}

/// A new filter and parameter set for one subscription.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubChange {
    pub filter: String,
    pub parameters: Vec<(String, String)>,
}

impl SubChange {
    /// Why the draft cannot be previewed. Empty when it can.
    pub fn issues(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.filter.trim().is_empty() {
            out.push("Enter a filter.".to_string());
        }
        for (name, value) in &self.parameters {
            if !self.filter.contains(&format!(":{name}")) {
                out.push(format!("The filter does not use parameter :{name}."));
            }
            if value.trim().is_empty() {
                out.push(format!("Enter a value for :{name}."));
            }
        }
        out
    }
}

/// How much one subscription sends. The server estimates it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scope {
    pub rows: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangePreview {
    pub subscription: String,
    pub name: String,
    pub device_id: String,
    /// The revision the preview read. Apply names it.
    pub revision: u64,
    pub change: SubChange,
    pub current: Scope,
    pub proposed: Scope,
    pub rows_removed: u64,
    pub rows_added: u64,
    pub resnapshot: bool,
    pub effects: Vec<String>,
    pub warnings: Vec<String>,
    pub permission: PermissionCheck,
}

impl ChangePreview {
    pub fn unchanged(&self) -> bool {
        self.current == self.proposed && !self.resnapshot
    }

    pub fn can_apply(&self) -> bool {
        self.permission.granted && !self.unchanged()
    }
}

/// `182 MB` or `4.2 GB`, from bytes.
pub fn size_text(bytes: u64) -> String {
    const MB: f64 = 1_000_000.0;
    let b = bytes as f64;
    if b >= 1000.0 * MB {
        format!("{:.1} GB", b / (1000.0 * MB))
    } else {
        format!("{:.0} MB", b / MB)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(filter: &str, value: &str) -> SubChange {
        SubChange {
            filter: filter.into(),
            parameters: vec![("store".into(), value.into())],
        }
    }

    #[test]
    fn a_draft_names_what_blocks_it() {
        assert!(change("store_id = :store", "nyc").issues().is_empty());
        assert_eq!(change("", "nyc").issues().len(), 2);
        assert!(change("store_id = :store", " ").issues()[0].contains(":store"));
    }

    #[test]
    fn the_table_filter_shows_the_value() {
        let store = crate::data::mock::SyncStore::seeded().subscriptions();
        assert_eq!(store.items[0].resolved_filter(), "store_id = 'nyc'");
    }

    #[test]
    fn events_read_as_the_mockup_words() {
        let e = SubEvent {
            at: 0,
            kind: SubEventKind::SnapshotComplete,
            count: Some(128_431),
        };
        assert_eq!(e.text(), "Initial snapshot complete (128,431 rows)");
        assert_eq!(size_text(182_000_000), "182 MB");
        assert_eq!(size_text(4_200_000_000), "4.2 GB");
    }
}
