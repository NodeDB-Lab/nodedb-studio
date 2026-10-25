//! Designer reads and writes at the backend seam.
//!
//! A change is staged first. Staging leaves the live database as it was and
//! the server validates it against the catalog and the other staged changes.
//! Review generates the DDL and dry-runs it. Apply runs the statements one
//! after another: a failure leaves the earlier statements applied, so the
//! caller reads the report instead of assuming one transaction.

use async_trait::async_trait;

use crate::models::designer::DesignSchema;
use crate::models::designer_change::{ApplyReport, ChangeKind, ChangeReview, PendingChange};
use crate::services::error::StudioError;

#[async_trait(?Send)]
pub trait DesignerData {
    /// The applied catalog: collections with fields and indexes, and the
    /// logical references. Staged changes are not in it.
    async fn design_schema(&self) -> Result<DesignSchema, StudioError>;

    /// The staged changes, in staging order.
    async fn pending_changes(&self) -> Result<Vec<PendingChange>, StudioError>;

    /// Stage a change, or replace the staged change `replace`. A change the
    /// catalog or the staged changes rule out is `StudioError::Rejected`.
    async fn stage_change(
        &self,
        replace: Option<&str>,
        kind: ChangeKind,
    ) -> Result<PendingChange, StudioError>;

    /// Remove a staged change, and the staged changes that need it.
    /// `StudioError::UnknownChange` when it is not staged.
    async fn discard_change(&self, id: &str) -> Result<(), StudioError>;

    /// Remove every staged change.
    async fn discard_all_changes(&self) -> Result<(), StudioError>;

    /// The DDL of the staged changes and what a dry run of it reports. The
    /// review carries the revision of the catalog and the staged set.
    async fn review_changes(&self) -> Result<ChangeReview, StudioError>;

    /// Apply the staged changes the review of `revision` read.
    /// `StudioError::StaleRevision` when the catalog or the staged set has
    /// changed since: nothing is applied. `StudioError::PermissionDenied`
    /// when the session cannot create. A change that fails to apply is in the
    /// report, not an error.
    async fn apply_changes(&self, revision: u64) -> Result<ApplyReport, StudioError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::collection::Attached;
    use crate::models::designer::{DesignIndex, ObjectState};
    use crate::models::designer_new::NewCollection;
    use crate::models::schema::IndexKind;
    use crate::services::mock::MockConnectionService;

    fn index(name: &str) -> ChangeKind {
        ChangeKind::CreateIndex(DesignIndex {
            name: name.into(),
            collection: "articles".into(),
            kind: IndexKind::FullText,
            unique: false,
            fields: vec!["title".into()],
            vector: None,
            state: ObjectState::Draft,
        })
    }

    #[tokio::test]
    async fn the_schema_holds_unique_collection_and_index_names() {
        let svc = MockConnectionService::ready();
        let s = svc.design_schema().await.expect("reads");
        let mut names: Vec<_> = s.collections.iter().map(|c| c.name().to_string()).collect();
        let n = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), n);
        let mut idx: Vec<String> = s
            .collections
            .iter()
            .flat_map(|c| c.indexes.iter().map(|i| i.name.clone()))
            .collect();
        let n = idx.len();
        idx.sort();
        idx.dedup();
        assert_eq!(idx.len(), n);
        assert!(
            s.collections
                .iter()
                .all(|c| c.state == ObjectState::Applied)
        );
    }

    #[tokio::test]
    async fn the_design_opens_with_one_pending_change() {
        let svc = MockConnectionService::ready();
        let p = svc.pending_changes().await.expect("reads");
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].id, "chg-1");
    }

    #[tokio::test]
    async fn pending_ids_are_unique() {
        let svc = MockConnectionService::ready();
        let a = svc
            .stage_change(None, index("idx_a"))
            .await
            .expect("stages");
        let b = svc
            .stage_change(None, index("idx_b"))
            .await
            .expect("stages");
        let mut ids: Vec<_> = svc
            .pending_changes()
            .await
            .expect("reads")
            .into_iter()
            .map(|p| p.id)
            .collect();
        assert_eq!(ids.len(), 3);
        assert_ne!(a.id, b.id);
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 3);
    }

    #[tokio::test]
    async fn discarding_removes_and_a_second_discard_is_an_error() {
        let svc = MockConnectionService::ready();
        svc.discard_change("chg-1").await.expect("discards");
        assert!(svc.pending_changes().await.expect("reads").is_empty());
        assert!(matches!(
            svc.discard_change("chg-1").await,
            Err(StudioError::UnknownChange(_))
        ));
    }

    #[tokio::test]
    async fn with_nothing_staged_a_review_is_empty() {
        let svc = MockConnectionService::ready();
        svc.discard_all_changes().await.expect("clears");
        let r = svc.review_changes().await.expect("reviews");
        assert!(r.blocks.is_empty());
        assert_eq!(
            (r.permission.as_str(), r.target.as_str()),
            ("admin", "analytics")
        );
    }

    #[tokio::test]
    async fn an_applied_collection_reaches_the_explorer_catalog() {
        use crate::services::explorer_data::ExplorerData;
        let svc = MockConnectionService::ready();
        svc.discard_all_changes().await.expect("clears");
        let mut form = NewCollection::blank("analytics");
        form.name = "returns".into();
        form.set_attached(Attached::Graph, true);
        for c in form.into_changes() {
            svc.stage_change(None, c).await.expect("stages");
        }
        let before = svc.catalog().await.expect("reads").len();
        let revision = svc.review_changes().await.expect("reviews").revision;
        let report = svc.apply_changes(revision).await.expect("applies");
        assert!(report.failed.is_none());
        let catalog = svc.catalog().await.expect("reads");
        assert_eq!(catalog.len(), before + 1);
        let made = catalog
            .iter()
            .find(|c| c.name == "returns")
            .expect("listed");
        assert!(made.has(Attached::Graph));
        assert_eq!(made.rows, Some(0));
        let schema = svc.collection_schema("returns").await.expect("schema");
        assert_eq!(schema.fields.len(), 1, "the id field");
        let page = svc
            .records("returns", &crate::models::records::RecordQuery::new(10))
            .await
            .expect("empty page");
        assert_eq!(page.total, 0);
    }

    #[tokio::test]
    async fn apply_with_the_reviewed_revision_succeeds() {
        let svc = MockConnectionService::ready();
        let review = svc.review_changes().await.expect("reviews");
        let report = svc.apply_changes(review.revision).await.expect("applies");
        assert_eq!(report.applied, vec!["chg-1".to_string()]);
        assert!(report.failed.is_none());
        assert!(svc.pending_changes().await.expect("reads").is_empty());
    }

    #[tokio::test]
    async fn staging_after_a_review_makes_its_revision_stale() {
        let svc = MockConnectionService::ready();
        let reviewed = svc.review_changes().await.expect("reviews").revision;
        svc.stage_change(None, index("idx_late"))
            .await
            .expect("stages");
        let err = svc.apply_changes(reviewed).await.expect_err("refused");
        assert!(matches!(err, StudioError::StaleRevision { .. }), "{err}");
        assert_eq!(
            svc.pending_changes().await.expect("reads").len(),
            2,
            "nothing was applied"
        );
        let again = svc.review_changes().await.expect("reviews").revision;
        assert_ne!(again, reviewed);
        svc.apply_changes(again).await.expect("applies");
    }

    #[tokio::test]
    async fn a_second_apply_of_the_same_revision_is_refused() {
        let svc = MockConnectionService::ready();
        let revision = svc.review_changes().await.expect("reviews").revision;
        svc.apply_changes(revision).await.expect("applies");
        let err = svc.apply_changes(revision).await.expect_err("refused");
        assert!(matches!(err, StudioError::StaleRevision { .. }), "{err}");
    }

    #[tokio::test]
    async fn the_empty_catalog_has_nothing_to_draw() {
        let svc = MockConnectionService::empty();
        let s = svc.design_schema().await.expect("reads");
        assert!(s.collections.is_empty());
        assert!(svc.pending_changes().await.expect("reads").is_empty());
    }

    #[tokio::test]
    async fn erroring_surfaces_through_every_method() {
        let svc = MockConnectionService::erroring();
        assert!(svc.design_schema().await.is_err());
        assert!(svc.pending_changes().await.is_err());
        assert!(svc.stage_change(None, index("idx_a")).await.is_err());
        assert!(svc.discard_change("chg-1").await.is_err());
        assert!(svc.review_changes().await.is_err());
        assert!(svc.apply_changes(1).await.is_err());
    }
}
