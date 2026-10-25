//! Topic fixtures and the in-memory topic store the mock backend writes.
//!
//! `orders_events` has three partitions. Group `billing` trails each by four
//! records, as the Events design shows.

use std::collections::BTreeMap;

use crate::models::format::{date_second, epoch};
use crate::models::topics::{
    ConsumerGroup, MessageRange, NewTopic, PartitionOffset, PublishDraft, ResetPreview, ResetScope,
    ResetTarget, Topic, TopicMessage, parse_range, preview_reset,
};
use crate::services::error::StudioError;

/// The mock server's clock: just after the last fixture message.
pub const NOW: i64 = epoch(2026, 6, 13, 13, 42, 30);

/// Customer id prefixes of `orders_events` partition 1, offsets 104 down to 90.
const PREFIXES: [u32; 15] = [12, 77, 3, 56, 88, 42, 19, 63, 4, 91, 27, 68, 33, 10, 75];
/// Timestamps (seconds past 13:38:00) of those offsets.
const SECONDS: [i64; 15] = [
    126, 118, 111, 105, 97, 91, 84, 78, 71, 65, 58, 52, 45, 39, 32,
];

fn topic(n: u32, name: &str, partitions: u32, days: u32, created: &str) -> Topic {
    Topic {
        id: format!("topic_{n:02}"),
        name: name.to_string(),
        partitions,
        retention_days: days,
        created: created.to_string(),
        replicated_to: 3,
        dead_letter: None,
    }
}

pub fn seeded_topics() -> Vec<Topic> {
    vec![
        topic(1, "orders_events", 3, 7, "2026-06-10 09:21:00 UTC"),
        topic(2, "telemetry", 6, 1, "2026-06-12 08:00:00 UTC"),
    ]
}

/// Log bounds of a seeded partition: `(log_start, log_end)`.
fn bounds(topic: &str, partition: u32) -> (u64, u64) {
    match (topic, partition) {
        ("orders_events", 0) => (80, 102),
        ("orders_events", 1) => (80, 105),
        ("orders_events", _) => (80, 104),
        _ => (40 + u64::from(partition), 60 + u64::from(partition) * 2),
    }
}

fn message(topic: &str, partition: u32, offset: u64) -> TopicMessage {
    if topic == "orders_events" {
        let slot = (104 - offset.min(104)) as usize;
        let (prefix, secs) = if partition == 1 && slot < 15 {
            (PREFIXES[slot], SECONDS[slot])
        } else {
            (
                ((offset * 7 + u64::from(partition) * 13) % 100) as u32,
                126 - (104 - offset.min(104) as i64) * 7 + i64::from(partition),
            )
        };
        let at = epoch(2026, 6, 13, 13, 38, 0) + secs;
        return TopicMessage {
            partition,
            offset,
            timestamp: date_second(at, 0),
            key: format!("o{offset}"),
            value: format!(
                "{{ \"orderId\": \"o{offset}\", \"customerId\": \"c{prefix:02}{:02}\", \"total\": {}.{:02}, \"currency\": \"USD\" }}",
                (offset * 3) % 100,
                20 + (offset * 11) % 180,
                (offset * 37) % 100
            ),
        };
    }
    let at = epoch(2026, 6, 13, 13, 40, 0) - (60 - offset as i64) * 5;
    TopicMessage {
        partition,
        offset,
        timestamp: date_second(at, 0),
        key: format!("d{}", offset % 12),
        value: format!(
            "{{ \"device\": \"d{}\", \"temp_c\": {}.{} }}",
            offset % 12,
            18 + offset % 9,
            offset % 10
        ),
    }
}

#[derive(Debug)]
struct Log {
    log_start: u64,
    /// Oldest first. Offsets run from `log_start` without gaps.
    messages: Vec<TopicMessage>,
}

impl Log {
    fn log_end(&self) -> u64 {
        self.log_start + self.messages.len() as u64
    }
}

#[derive(Debug)]
struct GroupRec {
    id: String,
    topic: String,
    name: String,
    /// Committed offset by partition.
    committed: BTreeMap<u32, u64>,
}

#[derive(Debug)]
pub struct TopicStore {
    topics: Vec<Topic>,
    next: u32,
    logs: BTreeMap<(String, u32), Log>,
    groups: Vec<GroupRec>,
}

impl TopicStore {
    pub fn seeded() -> Self {
        let topics = seeded_topics();
        let mut logs = BTreeMap::new();
        for t in &topics {
            for p in 0..t.partitions {
                let (start, end) = bounds(&t.name, p);
                logs.insert(
                    (t.id.clone(), p),
                    Log {
                        log_start: start,
                        messages: (start..end).map(|o| message(&t.name, p, o)).collect(),
                    },
                );
            }
        }
        let committed = |pairs: &[(u32, u64)]| pairs.iter().copied().collect::<BTreeMap<_, _>>();
        let rec = |id: &str, topic: &str, name: &str, committed| GroupRec {
            id: id.to_string(),
            topic: topic.to_string(),
            name: name.to_string(),
            committed,
        };
        let groups = vec![
            rec(
                "grp_billing",
                "topic_01",
                "billing",
                committed(&[(0, 98), (1, 101), (2, 100)]),
            ),
            rec(
                "grp_shipping_sync",
                "topic_01",
                "shipping_sync",
                committed(&[(0, 102), (1, 105), (2, 104)]),
            ),
            rec(
                "grp_ingest",
                "topic_02",
                "ingest",
                (0..6).map(|p| (p, bounds("telemetry", p).1 - 3)).collect(),
            ),
        ];
        Self {
            topics,
            next: 3,
            logs,
            groups,
        }
    }

    pub fn topics(&self) -> Vec<Topic> {
        self.topics.clone()
    }

    fn find(&self, id: &str) -> Result<&Topic, StudioError> {
        self.topics
            .iter()
            .find(|t| t.id == id)
            .ok_or_else(|| StudioError::UnknownObject {
                kind: "topic",
                id: id.to_string(),
            })
    }

    fn log(&self, topic: &str, partition: u32) -> Result<&Log, StudioError> {
        self.find(topic)?;
        self.logs
            .get(&(topic.to_string(), partition))
            .ok_or_else(|| StudioError::Rejected {
                reason: format!("partition {partition} does not exist"),
            })
    }

    fn group_of(&self, g: &GroupRec) -> ConsumerGroup {
        let offsets = self
            .logs
            .iter()
            .filter(|((t, _), _)| *t == g.topic)
            .map(|((_, p), log)| PartitionOffset {
                partition: *p,
                committed: g.committed.get(p).copied().unwrap_or(log.log_start),
                log_start: log.log_start,
                log_end: log.log_end(),
            })
            .collect();
        ConsumerGroup {
            id: g.id.clone(),
            name: g.name.clone(),
            topic: g.topic.clone(),
            offsets,
        }
    }

    pub fn groups(&self, topic: &str) -> Result<Vec<ConsumerGroup>, StudioError> {
        self.find(topic)?;
        Ok(self
            .groups
            .iter()
            .filter(|g| g.topic == topic)
            .map(|g| self.group_of(g))
            .collect())
    }

    /// The messages of one partition in `from..=to`, newest first.
    pub fn read(
        &self,
        topic: &str,
        partition: u32,
        from: u64,
        to: u64,
    ) -> Result<MessageRange, StudioError> {
        parse_range(&from.to_string(), &to.to_string()).map_err(|e| StudioError::Rejected {
            reason: e.to_string(),
        })?;
        let log = self.log(topic, partition)?;
        let messages = log
            .messages
            .iter()
            .filter(|m| (from..=to).contains(&m.offset))
            .rev()
            .cloned()
            .collect();
        Ok(MessageRange {
            partition,
            from,
            to,
            log_start: log.log_start,
            log_end: log.log_end(),
            messages,
        })
    }

    pub fn create(&mut self, form: &NewTopic) -> Result<Topic, StudioError> {
        if let Some((_, reason)) = form.issues(&self.topics).into_iter().next() {
            return Err(StudioError::Rejected { reason });
        }
        let partitions = form.partitions_value().unwrap_or(1);
        let mut t = topic(
            self.next,
            form.name.trim(),
            partitions,
            form.retention_value().unwrap_or(7),
            &format!("{} UTC", date_second(NOW, 0)),
        );
        t.replicated_to = 3;
        self.next += 1;
        for p in 0..partitions {
            self.logs.insert(
                (t.id.clone(), p),
                Log {
                    log_start: 0,
                    messages: Vec::new(),
                },
            );
        }
        self.topics.push(t.clone());
        Ok(t)
    }

    pub fn publish(
        &mut self,
        topic: &str,
        draft: &PublishDraft,
    ) -> Result<TopicMessage, StudioError> {
        let t = self.find(topic)?.clone();
        if let Some(reason) = draft.value_issue() {
            return Err(StudioError::Rejected { reason });
        }
        let partition = match draft.partition {
            Some(p) if p < t.partitions => p,
            Some(p) => {
                return Err(StudioError::Rejected {
                    reason: format!("partition {p} does not exist"),
                });
            }
            // The key picks the partition, so equal keys keep their order.
            None => {
                let h = draft.key.bytes().fold(2_166_136_261u32, |h, b| {
                    (h ^ u32::from(b)).wrapping_mul(16_777_619)
                });
                h % t.partitions
            }
        };
        let log = self
            .logs
            .get_mut(&(t.id.clone(), partition))
            .ok_or_else(|| StudioError::Rejected {
                reason: format!("partition {partition} does not exist"),
            })?;
        let offset = log.log_end();
        let tick = log.messages.len() as i64;
        let msg = TopicMessage {
            partition,
            offset,
            timestamp: date_second(NOW + tick, 0),
            key: draft.key.trim().to_string(),
            value: draft.value.split_whitespace().collect::<Vec<_>>().join(" "),
        };
        log.messages.push(msg.clone());
        Ok(msg)
    }

    pub fn preview_reset(
        &self,
        group: &str,
        scope: ResetScope,
        target: ResetTarget,
    ) -> Result<ResetPreview, StudioError> {
        let g = self.group(group)?;
        preview_reset(&g.offsets, scope, target).map_err(|e| StudioError::Rejected {
            reason: e.to_string(),
        })
    }

    fn group(&self, id: &str) -> Result<ConsumerGroup, StudioError> {
        self.groups
            .iter()
            .find(|g| g.id == id)
            .map(|g| self.group_of(g))
            .ok_or_else(|| StudioError::UnknownObject {
                kind: "consumer group",
                id: id.to_string(),
            })
    }

    pub fn apply_reset(
        &mut self,
        group: &str,
        scope: ResetScope,
        target: ResetTarget,
    ) -> Result<ConsumerGroup, StudioError> {
        let preview = self.preview_reset(group, scope, target)?;
        if let Some(g) = self.groups.iter_mut().find(|g| g.id == group) {
            for row in &preview.rows {
                g.committed.insert(row.partition, row.new);
            }
        }
        self.group(group)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn billing_trails_each_partition_by_four() {
        let s = TopicStore::seeded();
        let g = &s.groups("topic_01").expect("groups")[0];
        assert_eq!(g.name, "billing");
        let rows: Vec<_> = g
            .offsets
            .iter()
            .map(|o| (o.partition, o.committed, o.log_end, o.lag()))
            .collect();
        assert_eq!(rows, [(0, 98, 102, 4), (1, 101, 105, 4), (2, 100, 104, 4)]);
        assert_eq!(g.total_lag(), 12);
    }

    #[test]
    fn the_design_range_reads_fifteen_messages() {
        let s = TopicStore::seeded();
        let r = s.read("topic_01", 1, 90, 120).expect("reads");
        assert_eq!(
            r.summary(),
            "Showing 15 messages (offset 90–104) from partition 1"
        );
        let top = &r.messages[0];
        assert_eq!(
            (top.offset, top.timestamp.as_str(), top.key.as_str()),
            (104, "2026-06-13 13:40:06", "o104")
        );
        assert!(
            top.value
                .starts_with("{ \"orderId\": \"o104\", \"customerId\": \"c12")
        );
        let last = &r.messages[14];
        assert_eq!(
            (last.offset, last.timestamp.as_str()),
            (90, "2026-06-13 13:38:32")
        );
        assert_eq!(r.messages[4].timestamp, "2026-06-13 13:39:37");
    }

    #[test]
    fn a_range_is_bounded_and_the_partition_must_exist() {
        let s = TopicStore::seeded();
        assert!(s.read("topic_01", 1, 0, 600).is_err());
        assert!(s.read("topic_01", 9, 0, 10).is_err());
        assert!(s.read("topic_99", 0, 0, 10).is_err());
        assert!(
            s.read("topic_01", 1, 500, 510)
                .expect("reads")
                .messages
                .is_empty()
        );
    }

    #[test]
    fn offsets_have_no_gaps() {
        let s = TopicStore::seeded();
        for ((t, p), log) in &s.logs {
            for (i, m) in log.messages.iter().enumerate() {
                assert_eq!(m.offset, log.log_start + i as u64, "{t}/{p}");
            }
        }
    }

    #[test]
    fn a_published_message_takes_the_next_offset_and_adds_lag() {
        let mut s = TopicStore::seeded();
        let d = PublishDraft {
            partition: Some(1),
            key: "o105".into(),
            value: "{\n  \"orderId\": \"o105\"\n}".into(),
        };
        let m = s.publish("topic_01", &d).expect("publishes");
        assert_eq!((m.partition, m.offset), (1, 105));
        assert_eq!(s.groups("topic_01").expect("groups")[0].offsets[1].lag(), 5);
        let bad = PublishDraft {
            value: "{".into(),
            ..d
        };
        assert!(s.publish("topic_01", &bad).is_err());
    }

    #[test]
    fn the_same_key_picks_the_same_partition() {
        let mut s = TopicStore::seeded();
        let d = PublishDraft {
            partition: None,
            key: "customer-7".into(),
            value: "{}".into(),
        };
        let a = s.publish("topic_01", &d).expect("publishes").partition;
        assert_eq!(s.publish("topic_01", &d).expect("publishes").partition, a);
    }

    #[test]
    fn a_reset_moves_only_the_named_partition() {
        let mut s = TopicStore::seeded();
        let g = s
            .apply_reset(
                "grp_billing",
                ResetScope::Partition(1),
                ResetTarget::Offset(95),
            )
            .expect("resets");
        let committed: Vec<_> = g.offsets.iter().map(|o| o.committed).collect();
        assert_eq!(committed, [98, 95, 100]);
        assert!(
            s.apply_reset(
                "grp_billing",
                ResetScope::Partition(1),
                ResetTarget::Offset(500)
            )
            .is_err()
        );
        assert!(
            s.apply_reset("grp_nope", ResetScope::AllPartitions, ResetTarget::Latest)
                .is_err()
        );
    }

    #[test]
    fn a_created_topic_starts_empty() {
        let mut s = TopicStore::seeded();
        let mut f = NewTopic::blank();
        f.name = "audit_events".into();
        f.partitions = "2".into();
        let t = s.create(&f).expect("creates");
        assert_eq!(t.id, "topic_03");
        let r = s.read(&t.id, 1, 0, 10).expect("reads");
        assert_eq!((r.log_end, r.messages.len()), (0, 0));
        assert!(s.create(&f).is_err());
    }
}
