//! The Designer states the design shows: the workbench with one staged
//! index, the Create collection drawer, and the review of two changes.

use crate::models::designer::{IndexChoice, ObjectState};
use crate::models::designer_change::ChangeKind;
use crate::models::designer_new::{Constraint, ConstraintKind, NewCollection, SchemaMode};
use crate::models::schema::{FieldType, IndexKind};
use crate::state::designer::{CreateState, DesignerState, ReviewState, Target};

/// What a Designer scene stages before it opens, and how it opens.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignerSeed {
    /// The staged changes, in order. Anything staged before is discarded.
    pub pending: Vec<ChangeKind>,
    pub state: DesignerState,
}

/// The collection the Create and Review scenes make. `shipments` is taken
/// in the mock catalog, so the new one is the archive beside it.
fn shipments() -> NewCollection {
    let mut f = NewCollection::blank("analytics");
    f.name = "shipments_archive".into();
    f.schema = SchemaMode::Strict;
    f.add_field("order_id", FieldType::Uuid, true, false);
    f.add_field("status", FieldType::String { max: None }, true, false);
    f.add_field("shipped_at", FieldType::Timestamp, false, false);
    f.add_constraint(ConstraintKind::ForeignKey);
    if let Some(Constraint::ForeignKey {
        field,
        to_collection,
        to_field,
        ..
    }) = f.constraints.first_mut()
    {
        *field = "order_id".into();
        *to_collection = "orders".into();
        *to_field = "id".into();
    }
    f
}

/// The full-text index the design stages on `articles.body`.
fn body_index() -> ChangeKind {
    use crate::models::designer::DesignIndex;
    ChangeKind::CreateIndex(DesignIndex {
        name: "idx_articles_body".into(),
        collection: "articles".into(),
        kind: IndexChoice::FullText.kind(),
        unique: false,
        fields: vec!["body".into()],
        vector: None,
        state: ObjectState::Draft,
    })
}

/// The workbench with the staged index selected.
pub fn workbench() -> DesignerSeed {
    debug_assert_eq!(IndexChoice::FullText.kind(), IndexKind::FullText);
    DesignerSeed {
        pending: vec![body_index()],
        state: DesignerState {
            selected: Some(Target::Index("idx_articles_body".into())),
            ..DesignerState::default()
        },
    }
}

/// The Create collection drawer, filled in, with the foreign key open.
pub fn create() -> DesignerSeed {
    let mut create = CreateState::new("analytics");
    create.form = shipments();
    create.open = Some(ConstraintKind::ForeignKey);
    DesignerSeed {
        pending: Vec::new(),
        state: DesignerState {
            create: Some(create),
            ..DesignerState::default()
        },
    }
}

/// Two staged changes under review, the canvas scaled to fit beside the
/// panel.
pub fn review() -> DesignerSeed {
    DesignerSeed {
        pending: shipments().into_changes(),
        state: DesignerState {
            zoom: 70,
            review: Some(ReviewState::default()),
            ..DesignerState::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::collection::Attached;
    use crate::services::MockConnectionService;
    use crate::services::designer_data::DesignerData;

    /// A seed the backend refuses would leave a different state on screen
    /// than the one the design shows.
    #[tokio::test]
    async fn every_seed_stages_cleanly() {
        for seed in [workbench(), create(), review()] {
            let svc = MockConnectionService::ready();
            svc.discard_all_changes().await.expect("clears");
            for kind in &seed.pending {
                svc.stage_change(None, kind.clone())
                    .await
                    .expect("the backend accepts the staged change");
            }
            assert_eq!(
                svc.pending_changes().await.expect("reads").len(),
                seed.pending.len()
            );
        }
    }

    #[tokio::test]
    async fn the_create_form_is_ready_to_review() {
        let svc = MockConnectionService::ready();
        let schema = svc.design_schema().await.expect("reads");
        let seed = create();
        let form = seed.state.create.expect("drawer open").form;
        assert!(
            form.issues(&schema).is_empty(),
            "{:?}",
            form.issues(&schema)
        );
        assert!(!form.attached(Attached::Graph));
    }

    #[test]
    fn the_review_seed_holds_the_collection_then_its_reference() {
        let kinds = review().pending;
        assert_eq!(kinds.len(), 2);
        assert!(matches!(kinds[0], ChangeKind::CreateCollection(_)));
        assert!(matches!(kinds[1], ChangeKind::AddReference(_)));
    }
}
