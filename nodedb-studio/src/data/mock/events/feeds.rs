//! Change feed fixtures and the in-memory feed store the mock backend writes.
//!
//! The values reproduce the Events design: three feeds on two collections,
//! three retained events on `sales_changes`, and a warehouse webhook that
//! retried one of them.

use std::collections::BTreeMap;

use super::feed_history::{attempts, consumers, retained, upcoming};
use crate::models::feed_draft::FeedDraft;
use crate::models::feeds::{
    Backoff, ChangeFeed, ChangeOp, DeliveryAttempt, DeliveryKind, FeedConsumer, FeedEvent,
    FeedState, StartPosition,
};
use crate::services::error::StudioError;

const DATABASE: &str = "analytics";

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
}
