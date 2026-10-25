//! Change feed fixtures and the in-memory feed store the mock backend writes.
//!
//! The values reproduce the Events design: three feeds on two collections,
//! three retained events on `sales_changes`, and a warehouse webhook that
//! retried one of them.

use std::collections::BTreeMap;

use crate::models::feed_draft::{
    CheckStatus, FeedDraft, FeedPreview, PreviewCheck, SchemaNode, parse_projection,
};
use crate::models::feeds::{
    AttemptOutcome, Backoff, ChangeFeed, ChangeOp, ConsumerState, DeliveryAttempt, DeliveryKind,
    DeliveryStatus, FeedConsumer, FeedEvent, FeedState, StartPosition,
};
use crate::services::error::StudioError;

const DATABASE: &str = "analytics";

/// Fields the mock catalog knows for any collection it previews.
const SAMPLE_FIELDS: [(&str, &str); 6] = [
    ("_id", "\"681f2c9e8b4a1d3f7c9e2a11\""),
    ("order_id", "\"A100327\""),
    ("store_id", "\"nyc\""),
    ("amount", "129.50"),
    ("status", "\"completed\""),
    ("updated_at", "\"2025-06-13T13:20:11Z\""),
];
/// Fields a feed can also read that the sample document omits.
const OTHER_FIELDS: [&str; 4] = ["currency", "region", "sold_at", "revenue"];

fn feed(
    n: u32,
    name: &str,
    collection: &str,
    delivery: DeliveryKind,
    consumer: &str,
    endpoint: Option<&str>,
    state: FeedState,
) -> ChangeFeed {
    ChangeFeed {
        id: format!("feed_{n:02}"),
        name: name.to_string(),
        database: DATABASE.to_string(),
        collection: collection.to_string(),
        operations: ChangeOp::ALL.to_vec(),
        filter: String::new(),
        projection: String::new(),
        before_image: false,
        delivery,
        consumer: consumer.to_string(),
        endpoint: endpoint.map(str::to_string),
        state,
        error: None,
        checkpoint: checkpoint_for(name),
        backoff: Backoff::Exponential,
        max_retries: 10,
        retention_hours: 168,
        start: StartPosition::SnapshotThenTail,
        revision: 4,
    }
}

pub fn seeded_feeds() -> Vec<ChangeFeed> {
    let mut customers = feed(
        3,
        "customer_changes",
        "customers",
        DeliveryKind::Webhook,
        "crm_sync",
        Some("crm.acme.test/hooks/customers"),
        FeedState::Error,
    );
    customers.checkpoint = "…5c2".to_string();
    customers.error = Some("The endpoint answered HTTP 503 on the last 9 attempts.".to_string());
    let mut changes = feed(
        1,
        "sales_changes",
        "sales",
        DeliveryKind::Webhook,
        "warehouse_sink",
        Some("events.acme.test/sales"),
        FeedState::Running,
    );
    changes.checkpoint = "…d81".to_string();
    let mut audit = feed(
        2,
        "sales_audit",
        "sales",
        DeliveryKind::ClientSubscription,
        "audit_reader",
        None,
        FeedState::Paused,
    );
    audit.checkpoint = "…a07".to_string();
    vec![changes, audit, customers]
}

/// An event payload: the change, its record, and the new values.
fn payload(op: ChangeOp, collection: &str, record: &str, after: &[(&str, &str)]) -> String {
    let fields = after
        .iter()
        .map(|(k, v)| format!("    \"{k}\": {v}"))
        .collect::<Vec<_>>()
        .join(",\n");
    format!(
        "{{\n  \"operation\": \"{}\",\n  \"collection\": \"{collection}\",\n  \"record_id\": \"{record}\",\n  \"after\": {{\n{fields}\n  }}\n}}",
        op.label()
    )
}

fn event(
    feed: &str,
    id: u32,
    at: &str,
    op: ChangeOp,
    collection: &str,
    delivery: DeliveryStatus,
    after: &[(&str, &str)],
) -> FeedEvent {
    let record_id = format!(
        "{}_{id}",
        if collection == "sales" {
            "sale"
        } else {
            "cust"
        }
    );
    FeedEvent {
        id: format!("{feed}_e{id}"),
        received_at: format!("2026-06-13 {at}"),
        op,
        payload: payload(op, collection, &record_id, after),
        record_id,
        delivery,
    }
}

const SALE: [(&str, &str); 3] = [
    ("net_amount", "180.00"),
    ("currency", "\"USD\""),
    ("region", "\"APAC\""),
];

/// Events the feed retained, newest first.
fn retained(feed: &ChangeFeed) -> Vec<FeedEvent> {
    use ChangeOp::{Insert, Update};
    use DeliveryStatus::{Acknowledged, Failed, RetryScheduled};
    let f = feed.id.as_str();
    match f {
        "feed_01" => vec![
            event(
                f,
                1042,
                "13:40:04.100",
                Insert,
                "sales",
                Acknowledged,
                &SALE,
            ),
            event(
                f,
                1041,
                "13:40:03.600",
                Update,
                "sales",
                Acknowledged,
                &[
                    ("net_amount", "96.50"),
                    ("currency", "\"USD\""),
                    ("region", "\"EMEA\""),
                ],
            ),
            event(
                f,
                1040,
                "13:40:02.200",
                Insert,
                "sales",
                RetryScheduled,
                &[
                    ("net_amount", "42.00"),
                    ("currency", "\"USD\""),
                    ("region", "\"AMER\""),
                ],
            ),
        ],
        "feed_02" => vec![
            event(
                f,
                1039,
                "13:39:48.900",
                Insert,
                "sales",
                RetryScheduled,
                &SALE,
            ),
            event(
                f,
                1038,
                "13:39:41.300",
                Update,
                "sales",
                RetryScheduled,
                &SALE,
            ),
        ],
        _ => vec![
            event(
                f,
                8120,
                "13:38:12.400",
                Update,
                "customers",
                Failed,
                &[("tier", "\"gold\""), ("country", "\"SG\"")],
            ),
            event(
                f,
                8119,
                "13:37:55.050",
                Insert,
                "customers",
                Failed,
                &[("tier", "\"basic\""), ("country", "\"MY\"")],
            ),
        ],
    }
}

/// Events that arrive after the retained window, oldest first. A tail reads
/// them in order.
fn upcoming(feed: &ChangeFeed) -> Vec<FeedEvent> {
    if feed.id != "feed_01" {
        return Vec::new();
    }
    (0..8)
        .map(|i| {
            let op = if i % 3 == 1 {
                ChangeOp::Update
            } else {
                ChangeOp::Insert
            };
            event(
                &feed.id,
                1043 + i,
                &format!("13:40:{:02}.{:03}", 5 + i * 2, (i * 137) % 1000),
                op,
                "sales",
                DeliveryStatus::Acknowledged,
                &SALE,
            )
        })
        .collect()
}

fn consumers(feed: &ChangeFeed) -> Vec<FeedConsumer> {
    let primary = |state, pending, last_ack: &str| FeedConsumer {
        id: format!("{}_{}", feed.id, feed.consumer),
        name: feed.consumer.clone(),
        kind: feed.delivery,
        destination: feed
            .endpoint
            .clone()
            .unwrap_or_else(|| "Client subscription".to_string()),
        checkpoint: feed.checkpoint.clone(),
        last_ack: last_ack.to_string(),
        state,
        pending,
    };
    match feed.id.as_str() {
        "feed_01" => vec![
            primary(ConsumerState::Active, Some(1), "2026-06-13 13:40:04.100"),
            FeedConsumer {
                id: "feed_01_analytics_replay".to_string(),
                name: "analytics_replay".to_string(),
                kind: DeliveryKind::ClientSubscription,
                destination: "Client subscription".to_string(),
                checkpoint: "…c19".to_string(),
                last_ack: "2026-06-13 13:39:58.020".to_string(),
                state: ConsumerState::Active,
                pending: None,
            },
        ],
        "feed_02" => vec![primary(
            ConsumerState::Paused,
            None,
            "2026-06-13 13:31:09.870",
        )],
        _ => vec![primary(
            ConsumerState::Failing,
            Some(9),
            "2026-06-13 13:12:40.610",
        )],
    }
}

fn attempts(feed: &ChangeFeed) -> Vec<DeliveryAttempt> {
    let a = |id: &str, event: u32, at: &str, number, outcome, detail: &str| DeliveryAttempt {
        id: format!("{}_{id}", feed.id),
        event_id: format!("{}_e{event}", feed.id),
        record_id: format!("sale_{event}"),
        at: format!("2026-06-13 {at}"),
        consumer: feed.consumer.clone(),
        number,
        outcome,
        detail: detail.to_string(),
    };
    match feed.id.as_str() {
        "feed_01" => vec![
            a(
                "a5",
                1042,
                "13:40:04.184",
                1,
                AttemptOutcome::Acknowledged,
                "HTTP 200 in 84 ms",
            ),
            a(
                "a4",
                1041,
                "13:40:03.691",
                1,
                AttemptOutcome::Acknowledged,
                "HTTP 200 in 91 ms",
            ),
            a(
                "a3",
                1040,
                "13:40:07.412",
                2,
                AttemptOutcome::Retry,
                "HTTP 503 in 188 ms",
            ),
            a(
                "a2",
                1040,
                "13:40:02.412",
                1,
                AttemptOutcome::Retry,
                "HTTP 503 in 212 ms",
            ),
        ],
        "feed_02" => Vec::new(),
        _ => vec![
            a(
                "a2",
                8120,
                "13:38:42.400",
                3,
                AttemptOutcome::Failed,
                "HTTP 503 in 2,004 ms",
            ),
            a(
                "a1",
                8119,
                "13:38:12.900",
                2,
                AttemptOutcome::Failed,
                "HTTP 503 in 2,001 ms",
            ),
        ],
    }
}

/// The endpoint as a feed shows it: no scheme, no query string.
fn display_endpoint(url: &str) -> String {
    let rest = url.trim().strip_prefix("https://").unwrap_or(url.trim());
    rest.split(['?', '#']).next().unwrap_or(rest).to_string()
}

/// A short checkpoint ending in three hex digits derived from the name.
fn checkpoint_for(name: &str) -> String {
    let h = name
        .bytes()
        .fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)));
    format!("…{:03x}", h & 0xfff)
}

/// What the server reports for a draft.
pub fn preview(draft: &FeedDraft) -> FeedPreview {
    let source = draft.source.clone();
    let fields = parse_projection(&draft.projection).unwrap_or_default();
    let known = |f: &str| SAMPLE_FIELDS.iter().any(|(k, _)| *k == f) || OTHER_FIELDS.contains(&f);
    let unknown: Vec<&String> = fields.iter().filter(|f| !known(f)).collect();
    let mut checks = vec![PreviewCheck {
        label: "Permission".into(),
        status: CheckStatus::Pass,
        detail: format!("admin can create change feeds on {source}."),
    }];
    checks.push(if unknown.is_empty() {
        PreviewCheck {
            label: "Schema references".into(),
            status: CheckStatus::Pass,
            detail: if fields.is_empty() {
                "The feed delivers the whole document.".into()
            } else {
                format!("All {} projected fields exist in {source}.", fields.len())
            },
        }
    } else {
        PreviewCheck {
            label: "Schema references".into(),
            status: CheckStatus::Fail,
            detail: format!(
                "{} not in {source}.",
                unknown
                    .iter()
                    .map(|f| format!("`{f}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    });
    checks.push(PreviewCheck {
        label: "Filter".into(),
        status: CheckStatus::Pass,
        detail: if draft.filter.trim().is_empty() {
            "No filter: every row is captured.".into()
        } else {
            format!("`{}` is a supported filter.", draft.filter.trim())
        },
    });
    if draft.before_image {
        let kv = draft.collection() == "sessions";
        checks.push(PreviewCheck {
            label: "Before image".into(),
            status: if kv {
                CheckStatus::Warn
            } else {
                CheckStatus::Pass
            },
            detail: if kv {
                format!("{source} cannot supply before images. Update and delete events omit them.")
            } else {
                format!("{source} retains previous documents. Update and delete events carry them.")
            },
        });
    }
    checks.push(PreviewCheck {
        label: "Delivery target".into(),
        status: CheckStatus::Pass,
        detail: match draft.delivery {
            DeliveryKind::Webhook => "The server checks the endpoint on the first delivery.".into(),
            DeliveryKind::ClientSubscription => {
                "A client subscribes under the alias and pulls events.".into()
            }
        },
    });
    let snapshot = draft.start == StartPosition::SnapshotThenTail;
    checks.push(PreviewCheck {
        label: "Start position".into(),
        status: CheckStatus::Pass,
        detail: if snapshot {
            "The snapshot reads the rows that exist now, then continues with new changes.".into()
        } else {
            format!("Starts at: {}.", draft.start.label().to_lowercase())
        },
    });

    let doc_fields: Vec<(&str, &str)> = SAMPLE_FIELDS
        .iter()
        .copied()
        .filter(|(k, _)| fields.is_empty() || fields.iter().any(|f| f == k))
        .collect();
    let leaf = |name: &str, ty: &str| SchemaNode {
        name: name.into(),
        ty: ty.into(),
        value: None,
        children: Vec::new(),
    };
    let object = |name: &str, children: Vec<SchemaNode>| SchemaNode {
        name: name.into(),
        ty: "object".into(),
        value: None,
        children,
    };
    let mut doc_nodes: Vec<SchemaNode> = doc_fields
        .iter()
        .map(|(k, v)| {
            let ty = if v.starts_with('"') {
                "string"
            } else {
                "number"
            };
            leaf(k, ty)
        })
        .collect();
    if doc_nodes.is_empty() {
        doc_nodes.push(leaf("_id", "string"));
    }
    let mut schema = vec![
        leaf("operation", "string"),
        object(
            "ns",
            vec![
                SchemaNode {
                    value: Some(draft.database().to_string()),
                    ..leaf("db", "string")
                },
                SchemaNode {
                    value: Some(draft.collection().to_string()),
                    ..leaf("coll", "string")
                },
            ],
        ),
        object("documentKey", vec![leaf("_id", "string")]),
        object("fullDocument", doc_nodes.clone()),
    ];
    if draft.before_image {
        schema.push(object("fullDocumentBeforeChange", doc_nodes));
    }
    if draft.operations.contains(&ChangeOp::Update) {
        schema.push(object(
            "updateDescription",
            vec![
                object("updatedFields", Vec::new()),
                leaf("removedFields", "array"),
            ],
        ));
    }
    schema.push(leaf("clusterTime", "timestamp"));
    schema.push(leaf("eventTime", "timestamp"));

    let op = draft.operations.first().map_or("insert", |o| match o {
        ChangeOp::Insert => "insert",
        ChangeOp::Update => "update",
        ChangeOp::Delete => "delete",
    });
    let body = doc_fields
        .iter()
        .map(|(k, v)| format!("    \"{k}\": {v}"))
        .collect::<Vec<_>>()
        .join(",\n");
    let sample = format!(
        "{{\n  \"operation\": \"{op}\",\n  \"ns\": {{\n    \"db\": \"{}\",\n    \"coll\": \"{}\"\n  }},\n  \"documentKey\": {{ \"_id\": \"681f2c9e8b4a1d3f7c9e2a11\" }},\n  \"fullDocument\": {{\n{body}\n  }},\n  \"clusterTime\": \"2025-06-13T13:20:11Z\",\n  \"eventTime\": \"2025-06-13T13:20:11Z\"\n}}",
        draft.database(),
        draft.collection()
    );
    FeedPreview {
        checks,
        schema,
        sample,
        checkpoints: vec!["…d81".into(), "…a07".into(), "…5c2".into()],
    }
}

#[derive(Debug)]
pub struct FeedStore {
    feeds: Vec<ChangeFeed>,
    next: u32,
    extra_consumers: BTreeMap<String, Vec<FeedConsumer>>,
    committed: BTreeMap<String, usize>,
    read_end: BTreeMap<String, usize>,
}

impl FeedStore {
    pub fn seeded() -> Self {
        Self {
            feeds: seeded_feeds(),
            next: 4,
            extra_consumers: BTreeMap::new(),
            committed: BTreeMap::new(),
            read_end: BTreeMap::new(),
        }
    }

    pub fn list(&self) -> Vec<ChangeFeed> {
        self.feeds.clone()
    }

    fn find(&self, id: &str) -> Result<&ChangeFeed, StudioError> {
        self.feeds
            .iter()
            .find(|f| f.id == id)
            .ok_or_else(|| StudioError::UnknownObject {
                kind: "change feed",
                id: id.to_string(),
            })
    }

    pub fn consumers(&self, id: &str) -> Result<Vec<FeedConsumer>, StudioError> {
        let feed = self.find(id)?;
        Ok(self
            .extra_consumers
            .get(id)
            .cloned()
            .unwrap_or_else(|| consumers(feed)))
    }

    pub fn recent(&self, id: &str) -> Result<Vec<FeedEvent>, StudioError> {
        Ok(retained(self.find(id)?))
    }

    pub fn attempts(&self, id: &str) -> Result<Vec<DeliveryAttempt>, StudioError> {
        Ok(attempts(self.find(id)?))
    }

    fn reject(
        draft: &FeedDraft,
        others: &[ChangeFeed],
        editing: Option<&str>,
    ) -> Result<(), StudioError> {
        match draft.issues(others, editing).into_iter().next() {
            Some(i) => Err(StudioError::Rejected { reason: i.message }),
            None => Ok(()),
        }
    }

    fn apply(feed: &mut ChangeFeed, draft: &FeedDraft) {
        feed.name = draft.name.trim().to_string();
        feed.operations = draft.operations.clone();
        feed.filter = draft.filter.trim().to_string();
        feed.projection = draft.projection.trim().to_string();
        feed.before_image = draft.before_image;
        feed.delivery = draft.delivery;
        feed.consumer = draft.alias.trim().to_string();
        feed.endpoint = match draft.delivery {
            DeliveryKind::Webhook => Some(display_endpoint(&draft.endpoint)),
            DeliveryKind::ClientSubscription => None,
        };
        feed.backoff = draft.backoff;
        feed.max_retries = draft.max_retries_value().unwrap_or(feed.max_retries);
        feed.retention_hours = draft.retention_hours().unwrap_or(feed.retention_hours);
    }

    pub fn create(&mut self, draft: &FeedDraft) -> Result<ChangeFeed, StudioError> {
        Self::reject(draft, &self.feeds, None)?;
        let name = draft.name.trim().to_string();
        let mut feed = feed(
            self.next,
            &name,
            draft.collection(),
            draft.delivery,
            draft.alias.trim(),
            None,
            FeedState::Running,
        );
        feed.database = draft.database().to_string();
        feed.start = draft.start;
        feed.revision = 1;
        Self::apply(&mut feed, draft);
        self.next += 1;
        self.feeds.push(feed.clone());
        Ok(feed)
    }

    pub fn update(
        &mut self,
        id: &str,
        revision: u64,
        draft: &FeedDraft,
    ) -> Result<ChangeFeed, StudioError> {
        let stored = self.find(id)?.revision;
        if stored != revision {
            return Err(StudioError::StaleRevision {
                object: format!("change feed `{}`", self.find(id)?.name),
                expected: revision,
                stored,
            });
        }
        Self::reject(draft, &self.feeds, Some(id))?;
        let feed = self.feeds.iter_mut().find(|f| f.id == id).ok_or_else(|| {
            StudioError::UnknownObject {
                kind: "change feed",
                id: id.to_string(),
            }
        })?;
        Self::apply(feed, draft);
        feed.revision += 1;
        // The consumer list is derived from the feed again.
        self.extra_consumers.remove(id);
        Ok(feed.clone())
    }

    pub fn set_paused(&mut self, id: &str, paused: bool) -> Result<ChangeFeed, StudioError> {
        self.find(id)?;
        let feed = self.feeds.iter_mut().find(|f| f.id == id).ok_or_else(|| {
            StudioError::UnknownObject {
                kind: "change feed",
                id: id.to_string(),
            }
        })?;
        feed.state = if paused {
            FeedState::Paused
        } else {
            FeedState::Running
        };
        feed.error = None;
        feed.revision += 1;
        Ok(feed.clone())
    }

    pub fn delete(&mut self, id: &str) -> Result<(), StudioError> {
        self.find(id)?;
        self.feeds.retain(|f| f.id != id);
        self.extra_consumers.remove(id);
        self.reset(id);
        Ok(())
    }

    /// Up to `limit` events from the committed cursor. Reading never moves it.
    pub fn peek(&self, id: &str, limit: usize) -> Result<Vec<FeedEvent>, StudioError> {
        let start = self.committed.get(id).copied().unwrap_or(0);
        Ok(upcoming(self.find(id)?)
            .into_iter()
            .skip(start)
            .take(limit)
            .collect())
    }

    /// Record how far the last read reached.
    pub fn mark_read(&mut self, id: &str, delivered: usize) {
        let start = self.committed.get(id).copied().unwrap_or(0);
        self.read_end.insert(id.to_string(), start + delivered);
    }

    pub fn commit(&mut self, id: &str) {
        if let Some(end) = self.read_end.get(id).copied() {
            self.committed.insert(id.to_string(), end);
        }
    }

    pub fn reset(&mut self, id: &str) {
        self.committed.remove(id);
        self.read_end.remove(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_seeded_feeds_match_the_design() {
        let feeds = seeded_feeds();
        let rows: Vec<_> = feeds
            .iter()
            .map(|f| {
                (
                    f.name.as_str(),
                    f.collection.as_str(),
                    f.delivery.label(),
                    f.state.label(),
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                ("sales_changes", "sales", "Webhook", "Running"),
                ("sales_audit", "sales", "Client subscription", "Paused"),
                ("customer_changes", "customers", "Webhook", "Error"),
            ]
        );
    }

    #[test]
    fn the_retained_events_match_the_design() {
        let store = FeedStore::seeded();
        let events = store.recent("feed_01").expect("events");
        let rows: Vec<_> = events
            .iter()
            .map(|e| {
                (
                    e.received_at.as_str(),
                    e.op.label(),
                    e.record_id.as_str(),
                    e.delivery.label(),
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                (
                    "2026-06-13 13:40:04.100",
                    "INSERT",
                    "sale_1042",
                    "Acknowledged"
                ),
                (
                    "2026-06-13 13:40:03.600",
                    "UPDATE",
                    "sale_1041",
                    "Acknowledged"
                ),
                (
                    "2026-06-13 13:40:02.200",
                    "INSERT",
                    "sale_1040",
                    "Retry scheduled"
                ),
            ]
        );
        assert!(events[0].payload.contains("\"net_amount\": 180.00"));
    }

    #[test]
    fn event_ids_are_unique_per_feed() {
        let store = FeedStore::seeded();
        for f in store.list() {
            let mut ids: Vec<_> = store
                .recent(&f.id)
                .expect("events")
                .into_iter()
                .map(|e| e.id)
                .collect();
            ids.extend(upcoming(&f).into_iter().map(|e| e.id));
            let n = ids.len();
            ids.sort();
            ids.dedup();
            assert_eq!(ids.len(), n, "{}", f.id);
        }
    }

    #[test]
    fn reads_are_idempotent_until_committed() {
        let mut s = FeedStore::seeded();
        let first = s.peek("feed_01", 3).expect("read");
        assert_eq!(first, s.peek("feed_01", 3).expect("read"));
        s.mark_read("feed_01", first.len());
        assert_eq!(first, s.peek("feed_01", 3).expect("read"));
        s.commit("feed_01");
        assert_ne!(first, s.peek("feed_01", 3).expect("read"));
        s.reset("feed_01");
        assert_eq!(first, s.peek("feed_01", 3).expect("read"));
    }

    #[test]
    fn a_created_feed_gets_a_stable_id_and_a_consumer() {
        let mut s = FeedStore::seeded();
        let mut d = FeedDraft::blank("analytics.sales");
        d.name = "export_sales".into();
        d.endpoint = "https://warehouse.example.com/webhooks/sales?page=2".into();
        d.alias = "warehouse_sink".into();
        let f = s.create(&d).expect("creates");
        assert_eq!(f.id, "feed_04");
        assert_eq!(
            f.endpoint.as_deref(),
            Some("warehouse.example.com/webhooks/sales")
        );
        assert_eq!(s.consumers(&f.id).expect("consumers").len(), 1);
        // The same name is refused, and a delete frees it without reusing the id.
        assert!(s.create(&d).is_err());
        s.delete(&f.id).expect("deletes");
        assert_eq!(s.create(&d).expect("creates").id, "feed_05");
    }

    #[test]
    fn a_stale_edit_is_refused() {
        let mut s = FeedStore::seeded();
        let feed = s.list().remove(0);
        let mut d = FeedDraft::of(&feed);
        d.max_retries = "3".into();
        assert!(matches!(
            s.update(&feed.id, feed.revision - 1, &d),
            Err(StudioError::StaleRevision { .. })
        ));
        let saved = s.update(&feed.id, feed.revision, &d).expect("saves");
        assert_eq!((saved.max_retries, saved.revision), (3, feed.revision + 1));
    }

    #[test]
    fn pausing_changes_delivery_and_resuming_clears_the_error() {
        let mut s = FeedStore::seeded();
        assert_eq!(
            s.set_paused("feed_01", true).expect("pauses").state,
            FeedState::Paused
        );
        let resumed = s.set_paused("feed_03", false).expect("resumes");
        assert_eq!((resumed.state, resumed.error), (FeedState::Running, None));
    }

    #[test]
    fn the_preview_follows_the_design_draft() {
        let mut d = FeedDraft::blank("analytics.sales");
        d.projection = "_id, order_id, store_id, amount, status, updated_at".into();
        d.before_image = true;
        let p = preview(&d);
        assert!(p.can_create());
        let names: Vec<_> = p.schema.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "operation",
                "ns",
                "documentKey",
                "fullDocument",
                "fullDocumentBeforeChange",
                "updateDescription",
                "clusterTime",
                "eventTime"
            ]
        );
        assert!(p.sample.contains("\"order_id\": \"A100327\""));
        assert!(!p.sample.contains("currency"));
    }

    #[test]
    fn an_unknown_projected_field_fails_the_preview() {
        let mut d = FeedDraft::blank("analytics.sales");
        d.projection = "_id, nope".into();
        let p = preview(&d);
        assert!(!p.can_create());
        assert!(
            p.checks
                .iter()
                .any(|c| c.status == CheckStatus::Fail && c.detail.contains("`nope`"))
        );
    }
}
