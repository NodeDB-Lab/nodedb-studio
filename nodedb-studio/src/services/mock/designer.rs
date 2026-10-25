//! Staged schema changes, kept in the service's memory. Applying one adds to
//! the catalog the Explorer reads.

use async_trait::async_trait;

use crate::models::designer::DesignSchema;
use crate::models::designer_change::{ApplyReport, ChangeKind, ChangeReview, PendingChange};
use crate::services::designer_data::DesignerData;
use crate::services::error::StudioError;
use crate::services::mock::MockConnectionService;
use crate::services::mock_behavior::{apply, apply_one, apply_one_or_empty};

impl MockConnectionService {
    /// A write is refused while the behavior is `Erroring`, like a read.
    async fn designer_gate(&self) -> Result<(), StudioError> {
        apply_one(self.behavior, || ()).await
    }
}

#[async_trait(?Send)]
impl DesignerData for MockConnectionService {
    async fn design_schema(&self) -> Result<DesignSchema, StudioError> {
        let schema = self.designer.borrow().schema();
        let bare = DesignSchema {
            collections: Vec::new(),
            off_canvas: Vec::new(),
            references: Vec::new(),
            ..schema.clone()
        };
        apply_one_or_empty(self.behavior, move || schema, move || bare).await
    }

    async fn pending_changes(&self) -> Result<Vec<PendingChange>, StudioError> {
        let pending = self.designer.borrow().pending();
        apply(self.behavior, move || pending).await
    }

    async fn stage_change(
        &self,
        replace: Option<&str>,
        kind: ChangeKind,
    ) -> Result<PendingChange, StudioError> {
        self.designer_gate().await?;
        self.designer.borrow_mut().stage(replace, kind)
    }

    async fn discard_change(&self, id: &str) -> Result<(), StudioError> {
        self.designer_gate().await?;
        self.designer.borrow_mut().discard(id)
    }

    async fn discard_all_changes(&self) -> Result<(), StudioError> {
        self.designer_gate().await?;
        self.designer.borrow_mut().clear();
        Ok(())
    }

    async fn review_changes(&self) -> Result<ChangeReview, StudioError> {
        let (role, read_only) = self.identity();
        let review = self.designer.borrow().review(&role, read_only);
        // A review is one value with no empty shape, so `Empty` reads it too.
        apply_one(self.behavior, move || review).await
    }

    async fn apply_changes(&self) -> Result<ApplyReport, StudioError> {
        self.designer_gate().await?;
        let (_, read_only) = self.identity();
        self.designer.borrow_mut().apply(read_only)
    }
}
