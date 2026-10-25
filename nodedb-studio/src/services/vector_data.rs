//! Vector Space reads at the backend seam: the vector indexes, a record's
//! embedding, nearest-neighbor search, projection, and index status.
//!
//! A vector index belongs to one field of one collection, so every read
//! names both. Search and Projection read the original vectors. Index status
//! reads the server's own counters, and a value the server does not report
//! stays unreported.

use async_trait::async_trait;

use crate::models::vector::{NeighborRequest, NeighborResult, VectorRecord, VectorSetup};
use crate::models::vector_index::IndexStatus;
use crate::models::vector_projection::{Projection, ProjectionRequest};
use crate::services::error::StudioError;

#[async_trait(?Send)]
pub trait VectorData {
    /// The vector indexes this connection holds, and whether an embedding
    /// provider is configured.
    async fn vector_setup(&self) -> Result<VectorSetup, StudioError>;

    /// One record with its embedding. `RecordMissing` when the id is absent.
    async fn vector_record(
        &self,
        collection: &str,
        field: &str,
        id: &str,
    ) -> Result<VectorRecord, StudioError>;

    /// The nearest neighbors of the request's query vector, ascending by
    /// distance. Filters and the source exclusion apply before the top-k cut.
    async fn vector_search(&self, request: &NeighborRequest)
    -> Result<NeighborResult, StudioError>;

    /// Sample the index and reduce the sample to two dimensions. Dropping the
    /// returned future cancels the work, and its output is discarded.
    async fn vector_project(&self, request: &ProjectionRequest) -> Result<Projection, StudioError>;

    /// One snapshot of the index's counts, builds, and definition.
    async fn vector_index_status(
        &self,
        collection: &str,
        field: &str,
    ) -> Result<IndexStatus, StudioError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::vector::{NeighborRequest, QuerySource};
    use crate::models::vector_projection::{Method, ProjectionRequest};
    use crate::services::async_state::AsyncState;
    use crate::services::mock::MockConnectionService;

    fn neighbors(id: &str, k: u32) -> NeighborRequest {
        NeighborRequest {
            collection: "articles".into(),
            field: "embedding".into(),
            source: QuerySource::Record(id.into()),
            top_k: k,
            exclude_source: true,
            filters: Vec::new(),
            ef_search: None,
        }
    }

    fn projection(sample: u32) -> ProjectionRequest {
        ProjectionRequest {
            collection: "articles".into(),
            field: "embedding".into(),
            method: Method::Pca,
            sample,
            seed: 42,
            color_by: None,
            include: Vec::new(),
        }
    }

    #[tokio::test]
    async fn setup_target_names_are_unique() {
        let svc = MockConnectionService::ready();
        let setup = svc.vector_setup().await.expect("setup");
        let mut labels: Vec<String> = setup.targets.iter().map(|t| t.label()).collect();
        let n = labels.len();
        labels.sort();
        labels.dedup();
        assert_eq!(labels.len(), n);
        assert_eq!(n, 1);
    }

    #[tokio::test]
    async fn setup_empty_has_no_targets() {
        let svc = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(svc.vector_setup().await));
        assert!(s.is_empty());
    }

    #[tokio::test]
    async fn setup_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.vector_setup().await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn search_ready_ranks_with_unique_ids() {
        let svc = MockConnectionService::ready();
        let r = svc
            .vector_search(&neighbors("art104", 8))
            .await
            .expect("search");
        let mut ids: Vec<&str> = r.neighbors.iter().map(|n| n.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 8, "every neighbor needs a stable key");
        assert_eq!(r.request, neighbors("art104", 8));
    }

    #[tokio::test]
    async fn search_empty_returns_no_neighbors() {
        let svc = MockConnectionService::empty();
        let r = svc
            .vector_search(&neighbors("art104", 5))
            .await
            .expect("search");
        assert!(r.neighbors.is_empty());
    }

    #[tokio::test]
    async fn search_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.vector_search(&neighbors("art104", 5)).await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn record_ready_carries_the_embedding() {
        let svc = MockConnectionService::ready();
        let r = svc
            .vector_record("articles", "embedding", "art104")
            .await
            .expect("record");
        assert_eq!(r.vector.len(), 768);
        let err = svc
            .vector_record("articles", "embedding", "nope")
            .await
            .expect_err("missing");
        assert!(matches!(err, StudioError::RecordMissing { .. }));
    }

    #[tokio::test]
    async fn record_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(
            svc.vector_record("articles", "embedding", "art104").await,
        ));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn project_ready_has_the_sample_and_unique_ids() {
        let svc = MockConnectionService::ready();
        let p = svc
            .vector_project(&projection(200))
            .await
            .expect("projection");
        assert_eq!(p.points.len(), 200);
        let mut ids: Vec<&str> = p.points.iter().map(|x| x.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 200, "every point needs a stable key");
    }

    #[tokio::test]
    async fn project_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.vector_project(&projection(100)).await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn dropping_a_projection_cancels_it() {
        let svc = MockConnectionService::ready();
        let request = projection(20_000);
        let fut = svc.vector_project(&request);
        // Poll once, then drop: the worker must stop and nothing is returned.
        let polled = tokio::time::timeout(std::time::Duration::from_millis(1), fut).await;
        assert!(polled.is_err(), "the projection was still running");
    }

    #[tokio::test]
    async fn index_status_ready_adds_up() {
        let svc = MockConnectionService::ready();
        let s = svc
            .vector_index_status("articles", "embedding")
            .await
            .expect("status");
        assert_eq!(s.counts.live(), 1_108_492);
    }

    #[tokio::test]
    async fn index_status_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s =
            AsyncState::from_value(Some(svc.vector_index_status("articles", "embedding").await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn index_status_for_a_field_with_no_index_is_an_error() {
        let svc = MockConnectionService::ready();
        let err = svc
            .vector_index_status("articles", "title")
            .await
            .expect_err("no index");
        assert!(matches!(err, StudioError::UnknownObject { .. }));
    }
}
