//! Durable topics: partitions, retained messages, and consumer groups.
//!
//! A topic keeps its messages for the retention window and replays them. A
//! group's committed offset belongs to one partition of one topic, so a
//! reset names the partitions it moves and what that does to the records
//! between the old and the new offset.

use serde::{Deserialize, Serialize};

/// The most offsets one range read covers.
pub const MAX_READ: u64 = 500;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Topic {
    /// Stable identity.
    pub id: String,
    pub name: String,
    pub partitions: u32,
    pub retention_days: u32,
    pub created: String,
    /// How many nodes hold a replica of each partition.
    pub replicated_to: u32,
    /// The topic that receives messages a consumer gave up on, where the
    /// server supports it.
    pub dead_letter: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopicMessage {
    pub partition: u32,
    pub offset: u64,
    pub timestamp: String,
    pub key: String,
    /// The message value as JSON.
    pub value: String,
}

/// A bounded read of one partition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageRange {
    pub partition: u32,
    pub from: u64,
    pub to: u64,
    /// The first retained offset of the partition.
    pub log_start: u64,
    /// The offset the next message gets.
    pub log_end: u64,
    /// Newest first.
    pub messages: Vec<TopicMessage>,
}

impl MessageRange {
    /// `Showing 15 messages (offset 90–104) from partition 1`.
    pub fn summary(&self) -> String {
        match (self.messages.last(), self.messages.first()) {
            (Some(lo), Some(hi)) => format!(
                "Showing {} message{} (offset {}–{}) from partition {}",
                self.messages.len(),
                if self.messages.len() == 1 { "" } else { "s" },
                lo.offset,
                hi.offset,
                self.partition
            ),
            _ => format!(
                "No retained messages in offsets {}–{} of partition {}",
                self.from, self.to, self.partition
            ),
        }
    }
}

/// Why a range read is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RangeError {
    #[error("Enter a whole number for the start offset.")]
    FromNotANumber,
    #[error("Enter a whole number for the end offset.")]
    ToNotANumber,
    #[error("The start offset is after the end offset.")]
    Backwards,
    #[error("A read covers at most {MAX_READ} offsets. This one covers {0}.")]
    TooWide(u64),
}

/// The offsets a range read asks for.
pub fn parse_range(from: &str, to: &str) -> Result<(u64, u64), RangeError> {
    let from: u64 = from
        .trim()
        .parse()
        .map_err(|_| RangeError::FromNotANumber)?;
    let to: u64 = to.trim().parse().map_err(|_| RangeError::ToNotANumber)?;
    if from > to {
        return Err(RangeError::Backwards);
    }
    let width = to - from + 1;
    if width > MAX_READ {
        return Err(RangeError::TooWide(width));
    }
    Ok((from, to))
}

/// One partition's position for one group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartitionOffset {
    pub partition: u32,
    pub committed: u64,
    pub log_start: u64,
    pub log_end: u64,
}

impl PartitionOffset {
    /// Records the group has not committed yet.
    pub fn lag(&self) -> u64 {
        self.log_end.saturating_sub(self.committed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsumerGroup {
    pub id: String,
    pub name: String,
    pub topic: String,
    pub offsets: Vec<PartitionOffset>,
}

impl ConsumerGroup {
    pub fn total_lag(&self) -> u64 {
        self.offsets.iter().map(PartitionOffset::lag).sum()
    }
}

/// The partitions a reset moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetScope {
    AllPartitions,
    Partition(u32),
}

/// The offset a reset moves to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetTarget {
    Earliest,
    Latest,
    Offset(u64),
}

/// What a reset does to the records between the old and the new offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetEffect {
    /// The group processes these records again.
    Replays(u64),
    /// The group never processes these records.
    Skips(u64),
    Unchanged,
}

impl ResetEffect {
    pub fn label(self) -> String {
        match self {
            ResetEffect::Replays(n) => format!("Replays {n} record{}", plural(n)),
            ResetEffect::Skips(n) => format!("Skips {n} record{}", plural(n)),
            ResetEffect::Unchanged => "No change".to_string(),
        }
    }
}

fn plural(n: u64) -> &'static str {
    if n == 1 { "" } else { "s" }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResetRow {
    pub partition: u32,
    pub current: u64,
    pub new: u64,
    pub effect: ResetEffect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResetPreview {
    pub rows: Vec<ResetRow>,
}

impl ResetPreview {
    pub fn replayed(&self) -> u64 {
        self.rows
            .iter()
            .map(|r| match r.effect {
                ResetEffect::Replays(n) => n,
                ResetEffect::Skips(_) | ResetEffect::Unchanged => 0,
            })
            .sum()
    }

    pub fn skipped(&self) -> u64 {
        self.rows
            .iter()
            .map(|r| match r.effect {
                ResetEffect::Skips(n) => n,
                ResetEffect::Replays(_) | ResetEffect::Unchanged => 0,
            })
            .sum()
    }

    pub fn moves_anything(&self) -> bool {
        self.rows.iter().any(|r| r.effect != ResetEffect::Unchanged)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResetError {
    #[error("Partition {0} does not exist.")]
    UnknownPartition(u32),
    #[error("Offset {offset} is outside partition {partition}'s retained range {start}–{end}.")]
    OutsideRetention {
        partition: u32,
        offset: u64,
        start: u64,
        end: u64,
    },
}

/// What resetting `offsets` to `target` would do, partition by partition.
pub fn preview_reset(
    offsets: &[PartitionOffset],
    scope: ResetScope,
    target: ResetTarget,
) -> Result<ResetPreview, ResetError> {
    if let ResetScope::Partition(p) = scope
        && !offsets.iter().any(|o| o.partition == p)
    {
        return Err(ResetError::UnknownPartition(p));
    }
    let mut rows = Vec::new();
    for o in offsets {
        let moved = match scope {
            ResetScope::AllPartitions => true,
            ResetScope::Partition(p) => p == o.partition,
        };
        if !moved {
            continue;
        }
        let new = match target {
            ResetTarget::Earliest => o.log_start,
            ResetTarget::Latest => o.log_end,
            ResetTarget::Offset(n) => {
                if n < o.log_start || n > o.log_end {
                    return Err(ResetError::OutsideRetention {
                        partition: o.partition,
                        offset: n,
                        start: o.log_start,
                        end: o.log_end,
                    });
                }
                n
            }
        };
        let effect = match new.cmp(&o.committed) {
            std::cmp::Ordering::Less => ResetEffect::Replays(o.committed - new),
            std::cmp::Ordering::Greater => ResetEffect::Skips(new - o.committed),
            std::cmp::Ordering::Equal => ResetEffect::Unchanged,
        };
        rows.push(ResetRow {
            partition: o.partition,
            current: o.committed,
            new,
            effect,
        });
    }
    Ok(ResetPreview { rows })
}

/// The Create topic form.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NewTopic {
    pub name: String,
    pub partitions: String,
    pub retention_days: String,
}

impl NewTopic {
    pub fn blank() -> Self {
        Self {
            name: String::new(),
            partitions: "3".into(),
            retention_days: "7".into(),
        }
    }

    pub fn partitions_value(&self) -> Option<u32> {
        self.partitions
            .trim()
            .parse()
            .ok()
            .filter(|n| (1..=256).contains(n))
    }

    pub fn retention_value(&self) -> Option<u32> {
        self.retention_days
            .trim()
            .parse()
            .ok()
            .filter(|n| (1..=365).contains(n))
    }

    /// `(field, message)` pairs, in form order.
    pub fn issues(&self, existing: &[Topic]) -> Vec<(&'static str, String)> {
        let mut out = Vec::new();
        let name = self.name.trim();
        if name.is_empty() {
            out.push(("name", "Name the topic.".to_string()));
        } else if !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            || !name.starts_with(|c: char| c.is_ascii_lowercase())
        {
            out.push((
                "name",
                "Use lowercase letters, digits, and underscores, starting with a letter."
                    .to_string(),
            ));
        } else if existing.iter().any(|t| t.name == name) {
            out.push((
                "name",
                format!("A topic named `{name}` exists. Choose another name."),
            ));
        }
        if self.partitions_value().is_none() {
            out.push(("partitions", "Enter 1 to 256 partitions.".to_string()));
        }
        if self.retention_value().is_none() {
            out.push((
                "retention",
                "Enter a retention of 1 to 365 days.".to_string(),
            ));
        }
        out
    }
}

/// The Publish form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishDraft {
    /// `None` lets the key choose the partition.
    pub partition: Option<u32>,
    pub key: String,
    pub value: String,
}

impl PublishDraft {
    pub fn blank() -> Self {
        Self {
            partition: None,
            key: String::new(),
            value: "{\n  \"orderId\": \"\"\n}".into(),
        }
    }

    /// Why the value cannot be published, if it cannot.
    pub fn value_issue(&self) -> Option<String> {
        crate::models::json_text::json_issue(&self.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offsets() -> Vec<PartitionOffset> {
        [(0, 98, 102), (1, 101, 105), (2, 100, 104)]
            .into_iter()
            .map(|(partition, committed, log_end)| PartitionOffset {
                partition,
                committed,
                log_start: 80,
                log_end,
            })
            .collect()
    }

    #[test]
    fn lag_is_the_distance_to_the_log_end() {
        let g = ConsumerGroup {
            id: "g".into(),
            name: "billing".into(),
            topic: "t".into(),
            offsets: offsets(),
        };
        assert_eq!(g.offsets[1].lag(), 4);
        assert_eq!(g.total_lag(), 12);
    }

    #[test]
    fn ranges_are_bounded() {
        assert_eq!(parse_range("90", "120"), Ok((90, 120)));
        assert_eq!(parse_range("x", "1"), Err(RangeError::FromNotANumber));
        assert_eq!(parse_range("1", ""), Err(RangeError::ToNotANumber));
        assert_eq!(parse_range("5", "4"), Err(RangeError::Backwards));
        assert_eq!(parse_range("0", "499"), Ok((0, 499)));
        assert_eq!(parse_range("0", "500"), Err(RangeError::TooWide(501)));
    }

    #[test]
    fn the_summary_matches_the_design() {
        let msg = |offset| TopicMessage {
            partition: 1,
            offset,
            timestamp: String::new(),
            key: String::new(),
            value: String::new(),
        };
        let r = MessageRange {
            partition: 1,
            from: 90,
            to: 120,
            log_start: 80,
            log_end: 105,
            messages: (90..=104).rev().map(msg).collect(),
        };
        assert_eq!(
            r.summary(),
            "Showing 15 messages (offset 90–104) from partition 1"
        );
        let empty = MessageRange {
            messages: Vec::new(),
            ..r
        };
        assert!(empty.summary().starts_with("No retained messages"));
    }

    #[test]
    fn resetting_to_the_earliest_offset_replays() {
        let p = preview_reset(&offsets(), ResetScope::Partition(1), ResetTarget::Earliest)
            .expect("previews");
        assert_eq!(p.rows.len(), 1);
        assert_eq!(p.rows[0].effect, ResetEffect::Replays(21));
        assert_eq!(p.replayed(), 21);
        assert_eq!(p.skipped(), 0);
    }

    #[test]
    fn resetting_to_the_latest_offset_skips() {
        let p = preview_reset(&offsets(), ResetScope::AllPartitions, ResetTarget::Latest)
            .expect("previews");
        assert_eq!(p.rows.len(), 3);
        assert_eq!(p.skipped(), 12);
        assert_eq!(p.replayed(), 0);
    }

    #[test]
    fn the_committed_offset_is_no_change() {
        let p = preview_reset(
            &offsets(),
            ResetScope::Partition(1),
            ResetTarget::Offset(101),
        )
        .expect("previews");
        assert!(!p.moves_anything());
    }

    #[test]
    fn an_offset_outside_retention_is_refused() {
        let e = preview_reset(
            &offsets(),
            ResetScope::Partition(0),
            ResetTarget::Offset(10),
        )
        .expect_err("before the log start");
        assert!(matches!(
            e,
            ResetError::OutsideRetention { partition: 0, .. }
        ));
        assert!(
            preview_reset(
                &offsets(),
                ResetScope::Partition(0),
                ResetTarget::Offset(103)
            )
            .is_err()
        );
        assert_eq!(
            preview_reset(&offsets(), ResetScope::Partition(9), ResetTarget::Latest),
            Err(ResetError::UnknownPartition(9))
        );
    }

    #[test]
    fn a_new_topic_is_checked() {
        let existing = [Topic {
            id: "t1".into(),
            name: "telemetry".into(),
            partitions: 6,
            retention_days: 1,
            created: String::new(),
            replicated_to: 3,
            dead_letter: None,
        }];
        let mut t = NewTopic::blank();
        assert_eq!(t.issues(&existing)[0].0, "name");
        t.name = "telemetry".into();
        assert!(t.issues(&existing)[0].1.contains("exists"));
        t.name = "audit_events".into();
        assert!(t.issues(&existing).is_empty());
        t.partitions = "0".into();
        t.retention_days = "400".into();
        assert_eq!(t.issues(&existing).len(), 2);
    }

    #[test]
    fn a_publish_value_must_be_json() {
        let mut d = PublishDraft::blank();
        assert_eq!(d.value_issue(), None);
        d.value = "{ nope".into();
        assert!(
            d.value_issue()
                .is_some_and(|m| m.starts_with("Not valid JSON"))
        );
        d.value = "  ".into();
        assert_eq!(d.value_issue().as_deref(), Some("Enter a JSON value."));
    }
}
