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

/// Materialized views known to the cluster. `id` deliberately differs from
/// `name` (a `mv-N` handle vs. the human-readable view name) so a later bug
/// that keys a list by the wrong field is visible instead of invisible.
#[allow(dead_code)] // SEAM-UNWIRED
pub fn materialized_views() -> Vec<MaterializedView> {
    vec![
        MaterializedView {
            id: "mv-1".into(),
            name: "mv_top_users_24h".into(),
            source: "events".into(),
            refresh_mode: "incremental".into(),
            rows: "12,481".into(),
        },
        MaterializedView {
            id: "mv-2".into(),
            name: "mv_daily_revenue".into(),
            source: "orders".into(),
            refresh_mode: "scheduled".into(),
            rows: "3,204".into(),
        },
    ]
}

/// Durable, replayable topics with their consumer lag. `id` deliberately
/// differs from `name`, same reasoning as `materialized_views`.
#[allow(dead_code)] // SEAM-UNWIRED
pub fn topics() -> Vec<Topic> {
    vec![
        Topic {
            id: "topic-1".into(),
            name: "order_placed".into(),
            partitions: "8".into(),
            messages: "2.4M".into(),
            retention: "7d".into(),
            consumers: "3 active".into(),
            lag: "142ms".into(),
        },
        Topic {
            id: "topic-2".into(),
            name: "user_signup".into(),
            partitions: "4".into(),
            messages: "88,209".into(),
            retention: "30d".into(),
            consumers: "2 active".into(),
            lag: "22ms".into(),
        },
        Topic {
            id: "topic-3".into(),
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
/// accidentally-uniform status column. `id` deliberately differs from `name`,
/// same reasoning as `materialized_views`.
#[allow(dead_code)] // SEAM-UNWIRED
pub fn scheduled_jobs() -> Vec<ScheduledJob> {
    vec![
        ScheduledJob {
            id: "job-1".into(),
            name: "nightly_rollup".into(),
            cron: "0 2 * * *".into(),
            last_status: "success".into(),
            next_run: "in 2h 14m".into(),
        },
        ScheduledJob {
            id: "job-2".into(),
            name: "session_cleanup".into(),
            cron: "*/15 * * * *".into(),
            last_status: "success".into(),
            next_run: "in 11m".into(),
        },
        ScheduledJob {
            id: "job-3".into(),
            name: "vector_reindex".into(),
            cron: "0 4 * * 0".into(),
            last_status: "failed · oom".into(),
            next_run: "in 4d 8h".into(),
        },
    ]
}

/// LISTEN/NOTIFY channels. `id` deliberately differs from `name`, same
/// reasoning as `materialized_views`.
pub fn notify_channel_rows() -> Vec<NotifyChannel> {
    vec![
        NotifyChannel {
            id: "channel-1".into(),
            name: "user_events".into(),
            subscribers: "12".into(),
        },
        NotifyChannel {
            id: "channel-2".into(),
            name: "deploy_hooks".into(),
            subscribers: "3".into(),
        },
        NotifyChannel {
            id: "channel-3".into(),
            name: "cache_invalidate".into(),
            subscribers: "5".into(),
        },
    ]
}

/// The pub/sub message tail across channels.
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
    use crate::data::mock::test_support::{assert_ids_distinct_from_names, assert_unique_ids};

    #[test]
    fn materialized_views_have_unique_ids_distinct_from_name() {
        let rows = materialized_views();
        assert_unique_ids(&rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>());
        assert_ids_distinct_from_names(rows.iter().map(|r| (r.id.as_str(), r.name.as_str())));
    }

    #[test]
    fn topics_have_unique_ids_distinct_from_name() {
        let rows = topics();
        assert_unique_ids(&rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>());
        assert_ids_distinct_from_names(rows.iter().map(|r| (r.id.as_str(), r.name.as_str())));
    }

    #[test]
    fn scheduled_jobs_have_unique_ids_distinct_from_name_and_a_mixed_status() {
        let rows = scheduled_jobs();
        assert_unique_ids(&rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>());
        assert_ids_distinct_from_names(rows.iter().map(|r| (r.id.as_str(), r.name.as_str())));
        assert!(rows.iter().any(|j| j.last_status == "success"));
        assert!(rows.iter().any(|j| j.last_status.starts_with("failed")));
    }

    #[test]
    fn notify_channel_rows_have_unique_ids_distinct_from_name() {
        let rows = notify_channel_rows();
        assert_unique_ids(&rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>());
        assert_ids_distinct_from_names(rows.iter().map(|r| (r.id.as_str(), r.name.as_str())));
    }

    #[test]
    fn notify_message_rows_have_unique_ids() {
        let rows = notify_message_rows();
        assert_unique_ids(&rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>());
    }
}
