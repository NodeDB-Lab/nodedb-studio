//! The vector index of `articles.embedding` as one snapshot reports it.
//!
//! Every counter, estimate, and definition value is a fixture. Nothing here
//! is a measurement of recall or latency, and the snapshot reports none.

use crate::data::mock::vector::corpus::{COLLECTION, FIELD, INDEX, REVISION};
use crate::data::mock::vector::search::check_target;
use crate::models::format::epoch;
use crate::models::vector_index::{
    BuildCounters, BuildState, Coverage, HnswParams, IndexDefinition, IndexKind, IndexStatus,
    Quantization, RawPrecision, Reported, VectorCounts,
};
use crate::services::error::StudioError;

pub fn index_status(collection: &str, field: &str) -> Result<IndexStatus, StudioError> {
    check_target(collection, field)?;
    Ok(IndexStatus {
        collection: COLLECTION.into(),
        field: FIELD.into(),
        index: INDEX.into(),
        revision: REVISION,
        snapshot_at: epoch(2026, 6, 13, 10, 42, 0),
        coverage: Coverage::of(1, 1),
        counts: VectorCounts {
            sealed: 1_100_000,
            growing: 10_492,
            tombstoned: 2_000,
        },
        sealed_segments: Reported::of(Some(17), true),
        // 1.1 GiB and 3.2 GiB, rounded the way the screen shows them.
        memory_bytes: Reported::of(Some(1_181_116_006), true),
        disk_bytes: Reported::of(Some(3_435_973_837), true),
        build: BuildState::from_counts(0, false),
        builds: BuildCounters {
            queued: 0,
            completed: 17,
            failed: 0,
        },
        definition: IndexDefinition {
            kind: IndexKind::from_name("hnsw"),
            quantization: Quantization::from_name("sq8"),
            raw_precision: Reported::of(RawPrecision::from_name("f32"), true),
            hnsw: Some(HnswParams {
                m: 16,
                m0: 32,
                ef_construction: 200,
            }),
            ivf: None,
            seal_threshold: 65_536,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::vector_index::{estimate, percent};

    #[test]
    fn the_snapshot_adds_up() {
        let s = index_status("articles", "embedding").expect("status");
        assert_eq!(s.counts.live(), 1_108_492);
        assert_eq!(percent(s.counts.tombstone_ratio()), "0.18%");
        assert_eq!(s.memory_bytes.text(estimate), "≈ 1.1 GiB");
        assert_eq!(s.disk_bytes.text(estimate), "≈ 3.2 GiB");
        assert_eq!(s.build, BuildState::Idle);
    }

    #[test]
    fn another_field_has_no_index() {
        assert!(index_status("articles", "title").is_err());
    }
}
