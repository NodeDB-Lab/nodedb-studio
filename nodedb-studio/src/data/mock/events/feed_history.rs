//! Per-feed fixtures: retained events, events still to arrive, consumers, and
//! delivery attempts.

use crate::models::feeds::{
    AttemptOutcome, ChangeFeed, ChangeOp, ConsumerState, DeliveryAttempt, DeliveryKind,
    DeliveryStatus, FeedConsumer, FeedEvent,
};

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
pub(super) fn retained(feed: &ChangeFeed) -> Vec<FeedEvent> {
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
pub(super) fn upcoming(feed: &ChangeFeed) -> Vec<FeedEvent> {
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

pub(super) fn consumers(feed: &ChangeFeed) -> Vec<FeedConsumer> {
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

pub(super) fn attempts(feed: &ChangeFeed) -> Vec<DeliveryAttempt> {
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
