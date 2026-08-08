//! Streams fixtures beyond CDC (`data::mock::cdc`): materialized views,
//! durable topics, scheduled jobs and LISTEN/NOTIFY. Shapes mirror the
//! server's introspection output so the real implementation is a decoder
//! swap, not a model change.
//!
//! `notify_channel_rows`/`notify_message_rows` are deliberately not named
//! `notify_channels`/`notify_messages`: those names are already taken at
//! `data::mock`'s root by the pre-seam fixtures `views::streams::notify`
//! reads directly (see that module's doc comment). The two fixture sets
//! describe different shapes and are wired independently; task 10 reconciles
//! the view onto the seam.

use crate::models::streams::{MaterializedView, NotifyChannel, NotifyMessage, ScheduledJob, Topic};

/// Materialized views known to the cluster.
#[allow(dead_code)] // SEAM-UNWIRED(task-10)
pub fn materialized_views() -> Vec<MaterializedView> {
    vec![
        MaterializedView {
            id: "mv_top_users_24h".into(),
            name: "mv_top_users_24h".into(),
            source: "events".into(),
            refresh_mode: "incremental".into(),
            rows: "12,481".into(),
        },
        MaterializedView {
            id: "mv_daily_revenue".into(),
            name: "mv_daily_revenue".into(),
            source: "orders".into(),
            refresh_mode: "scheduled".into(),
            rows: "3,204".into(),
        },
    ]
}

/// Durable, replayable topics with their consumer lag.
#[allow(dead_code)] // SEAM-UNWIRED(task-10)
pub fn topics() -> Vec<Topic> {
    vec![
        Topic {
            id: "order_placed".into(),
            name: "order_placed".into(),
            partitions: "8".into(),
            messages: "2.4M".into(),
            retention: "7d".into(),
            consumers: "3 active".into(),
            lag: "142ms".into(),
        },
        Topic {
            id: "user_signup".into(),
            name: "user_signup".into(),
            partitions: "4".into(),
            messages: "88,209".into(),
            retention: "30d".into(),
            consumers: "2 active".into(),
            lag: "22ms".into(),
        },
        Topic {
            id: "payment_failed".into(),
            name: "payment_failed".into(),
            partitions: "2".into(),
            messages: "12,488".into(),
            retention: "90d".into(),
            consumers: "1 active · 1 stalled".into(),
            lag: "4.2s".into(),
        },
    ]
}

/// Cron-style scheduled jobs. Deliberately mixes a failed job in with
/// successful ones so fixture-content tests can't be satisfied by an
/// accidentally-uniform status column.
#[allow(dead_code)] // SEAM-UNWIRED(task-10)
pub fn scheduled_jobs() -> Vec<ScheduledJob> {
    vec![
        ScheduledJob {
            id: "nightly_rollup".into(),
            name: "nightly_rollup".into(),
            cron: "0 2 * * *".into(),
            last_status: "success".into(),
            next_run: "in 2h 14m".into(),
        },
        ScheduledJob {
            id: "session_cleanup".into(),
            name: "session_cleanup".into(),
            cron: "*/15 * * * *".into(),
            last_status: "success".into(),
            next_run: "in 11m".into(),
        },
        ScheduledJob {
            id: "vector_reindex".into(),
            name: "vector_reindex".into(),
            cron: "0 4 * * 0".into(),
            last_status: "failed · oom".into(),
            next_run: "in 4d 8h".into(),
        },
    ]
}

/// LISTEN/NOTIFY channels.
#[allow(dead_code)] // SEAM-UNWIRED(task-10)
pub fn notify_channel_rows() -> Vec<NotifyChannel> {
    vec![
        NotifyChannel {
            id: "user_events".into(),
            name: "user_events".into(),
            subscribers: "12".into(),
        },
        NotifyChannel {
            id: "deploy_hooks".into(),
            name: "deploy_hooks".into(),
            subscribers: "3".into(),
        },
        NotifyChannel {
            id: "cache_invalidate".into(),
            name: "cache_invalidate".into(),
            subscribers: "5".into(),
        },
    ]
}

/// The pub/sub message tail across channels.
#[allow(dead_code)] // SEAM-UNWIRED(task-10)
pub fn notify_message_rows() -> Vec<NotifyMessage> {
    (0..4)
        .map(|i| NotifyMessage {
            id: format!("notify-{i}"),
            channel: "user_events".into(),
            at: format!("04:23:{:02}.041", 18 - i),
            payload_json: format!("{{\"event\":\"login\",\"seq\":{i}}}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materialized_views_have_unique_ids() {
        let rows = materialized_views();
        assert_unique_ids(&rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>());
    }

    #[test]
    fn topics_have_unique_ids() {
        let rows = topics();
        assert_unique_ids(&rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>());
    }

    #[test]
    fn scheduled_jobs_have_unique_ids_and_a_mixed_status() {
        let rows = scheduled_jobs();
        assert_unique_ids(&rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>());
        assert!(rows.iter().any(|j| j.last_status == "success"));
        assert!(rows.iter().any(|j| j.last_status.starts_with("failed")));
    }

    #[test]
    fn notify_channel_rows_have_unique_ids() {
        let rows = notify_channel_rows();
        assert_unique_ids(&rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>());
    }

    #[test]
    fn notify_message_rows_have_unique_ids() {
        let rows = notify_message_rows();
        assert_unique_ids(&rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>());
    }

    fn assert_unique_ids(ids: &[&str]) {
        assert!(!ids.is_empty(), "fixture must not be empty");
        let mut sorted = ids.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "ids must be unique");
    }
}
