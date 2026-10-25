//! Import and export jobs, newest first.

use crate::models::format::epoch;
use crate::models::transfer::{JobKind, JobState, TransferJob};

fn job(
    id: &str,
    name: &str,
    kind: JobKind,
    collection: &str,
    state: JobState,
    started_at: i64,
) -> TransferJob {
    TransferJob {
        id: id.to_string(),
        name: name.to_string(),
        kind,
        collection: collection.to_string(),
        state,
        started_at,
    }
}

pub fn transfer_jobs() -> Vec<TransferJob> {
    vec![
        job(
            "job_0412",
            "users.csv",
            JobKind::Export,
            "users",
            JobState::Completed,
            epoch(2026, 6, 13, 13, 21, 0),
        ),
        job(
            "job_0411",
            "orders.parquet",
            JobKind::Export,
            "orders",
            JobState::Running { percent: 45 },
            epoch(2026, 6, 13, 13, 18, 12),
        ),
        job(
            "job_0409",
            "sales.jsonl",
            JobKind::Export,
            "sales",
            JobState::Completed,
            epoch(2026, 6, 12, 9, 2, 41),
        ),
        job(
            "job_0405",
            "articles.jsonl",
            JobKind::Import,
            "articles",
            JobState::Failed {
                reason: "Row 14: `embedding` has 512 dimensions, the schema expects 768"
                    .to_string(),
            },
            epoch(2026, 6, 11, 16, 30, 5),
        ),
        job(
            "job_0398",
            "places.csv",
            JobKind::Import,
            "places",
            JobState::Completed,
            epoch(2026, 6, 10, 11, 12, 48),
        ),
        job(
            "job_0391",
            "inventory.csv",
            JobKind::Export,
            "inventory",
            JobState::Cancelled,
            epoch(2026, 6, 9, 14, 45, 30),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_ids_are_unique_and_newest_first() {
        let jobs = transfer_jobs();
        let mut ids: Vec<&String> = jobs.iter().map(|j| &j.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), jobs.len());
        assert!(jobs.windows(2).all(|w| w[0].started_at >= w[1].started_at));
    }
}
