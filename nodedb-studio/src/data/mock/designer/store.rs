//! The mock's Designer state: the changes staged, and the schema changes
//! applied. An applied change reaches the Explorer's catalog.

use super::schema::{DATABASE, design_schema, seed_references};
use crate::models::collection::Collection;
use crate::models::designer::{
    DesignCollection, DesignIndex, DesignSchema, LogicalReference, ObjectState, VectorMetric,
    VectorParams,
};
use crate::models::designer_change::{
    ApplyReport, ChangeKind, ChangeReview, DryRun, PendingChange,
};
use crate::models::designer_ddl;
use crate::models::designer_new::is_identifier;
use crate::models::schema::{FieldType, IndexKind};
use crate::services::error::StudioError;

fn rejected(reason: impl Into<String>) -> StudioError {
    StudioError::Rejected {
        reason: reason.into(),
    }
}

#[derive(Debug, Clone)]
pub struct DesignerStore {
    created: Vec<DesignCollection>,
    indexes: Vec<DesignIndex>,
    references: Vec<LogicalReference>,
    pending: Vec<PendingChange>,
    seq: u32,
}

impl DesignerStore {
    /// The state the design shows: one staged full-text index.
    pub fn seeded() -> Self {
        let mut store = Self {
            created: Vec::new(),
            indexes: Vec::new(),
            references: seed_references(),
            pending: Vec::new(),
            seq: 0,
        };
        let draft = DesignIndex {
            name: "idx_articles_body".to_string(),
            collection: "articles".to_string(),
            kind: IndexKind::FullText,
            unique: false,
            fields: vec!["body".to_string()],
            vector: None,
            state: ObjectState::Draft,
        };
        // The seed is valid by construction. A failure would leave no draft.
        let _ = store.stage(None, ChangeKind::CreateIndex(draft));
        store
    }

    /// The applied schema: the fixtures, then everything applied since.
    pub fn schema(&self) -> DesignSchema {
        let mut s = design_schema();
        s.collections.extend(self.created.iter().cloned());
        for index in &self.indexes {
            if let Some(owner) = s
                .collections
                .iter_mut()
                .find(|c| c.name() == index.collection)
            {
                owner.indexes.push(index.clone());
            }
        }
        s.references = self.references.clone();
        // A created collection is drawn. Its name leaves the free ones.
        s
    }

    /// Collections created and applied this session, for the Explorer.
    pub fn created(&self) -> Vec<Collection> {
        self.created.iter().map(|c| c.collection.clone()).collect()
    }

    pub fn pending(&self) -> Vec<PendingChange> {
        self.pending.clone()
    }

    /// Check a change against the applied schema plus the other staged
    /// changes. The index gains the vector parameters its field implies.
    fn check(&self, others: &[PendingChange], kind: ChangeKind) -> Result<ChangeKind, StudioError> {
        let schema = self.schema().with_pending(others);
        match kind {
            ChangeKind::CreateCollection(new) => match new.issues(&schema).into_iter().next() {
                Some(first) => Err(rejected(first.message)),
                None => Ok(ChangeKind::CreateCollection(new)),
            },
            ChangeKind::CreateIndex(mut index) => {
                if !is_identifier(&index.name) {
                    return Err(rejected(format!(
                        "`{}` is not a valid index name. Use lowercase letters, digits, and underscores.",
                        index.name
                    )));
                }
                if schema.index(&index.name).is_some() {
                    return Err(rejected(format!(
                        "An index named `{}` already exists. Index names are unique in {}.",
                        index.name, schema.database
                    )));
                }
                let owner = schema.collection(&index.collection).ok_or_else(|| {
                    rejected(format!("`{}` is not a collection.", index.collection))
                })?;
                let choice = index
                    .choice()
                    .ok_or_else(|| rejected("The Designer cannot create this kind of index."))?;
                let field = index
                    .fields
                    .first()
                    .and_then(|f| owner.field(f))
                    .ok_or_else(|| {
                        rejected(format!("Pick a field of {} to index.", owner.name()))
                    })?;
                if !choice.accepts(field.ty()) {
                    return Err(rejected(format!(
                        "A {} index cannot sit on `{}` ({}).",
                        choice.label().to_lowercase(),
                        field.name(),
                        field.ty().keyword()
                    )));
                }
                index.vector = match field.ty() {
                    FieldType::Vector { dimensions } if index.kind == IndexKind::Vector => {
                        Some(VectorParams {
                            dimensions,
                            metric: index.vector.map_or(VectorMetric::Cosine, |v| v.metric),
                        })
                    }
                    _ => None,
                };
                Ok(ChangeKind::CreateIndex(index))
            }
            ChangeKind::AddReference(r) => {
                let has =
                    |c: &str, f: &str| schema.collection(c).is_some_and(|c| c.field(f).is_some());
                if !has(&r.from_collection, &r.from_field) {
                    return Err(rejected(format!(
                        "`{}.{}` does not exist.",
                        r.from_collection, r.from_field
                    )));
                }
                if !has(&r.to_collection, &r.to_field) {
                    return Err(rejected(format!(
                        "`{}.{}` does not exist.",
                        r.to_collection, r.to_field
                    )));
                }
                let twin = schema.references.iter().any(|e| {
                    e.from_collection == r.from_collection
                        && e.from_field == r.from_field
                        && e.to_collection == r.to_collection
                        && e.to_field == r.to_field
                });
                if twin {
                    return Err(rejected(format!("{} already exists.", r.label())));
                }
                Ok(ChangeKind::AddReference(r))
            }
        }
    }

    /// Stage a change, or replace the staged change `replace`.
    pub fn stage(
        &mut self,
        replace: Option<&str>,
        kind: ChangeKind,
    ) -> Result<PendingChange, StudioError> {
        let others: Vec<PendingChange> = self
            .pending
            .iter()
            .filter(|p| Some(p.id.as_str()) != replace)
            .cloned()
            .collect();
        let kind = self.check(&others, kind)?;
        if let Some(id) = replace {
            let slot = self
                .pending
                .iter_mut()
                .find(|p| p.id == id)
                .ok_or_else(|| StudioError::UnknownChange(id.to_string()))?;
            slot.kind = kind;
            return Ok(slot.clone());
        }
        self.seq += 1;
        let change = PendingChange {
            id: format!("chg-{}", self.seq),
            kind,
        };
        self.pending.push(change.clone());
        Ok(change)
    }

    /// Remove a staged change, and any staged change that needs it.
    pub fn discard(&mut self, id: &str) -> Result<(), StudioError> {
        let before = self.pending.len();
        let gone: Option<PendingChange> = self.pending.iter().find(|p| p.id == id).cloned();
        self.pending.retain(|p| p.id != id);
        if self.pending.len() == before {
            return Err(StudioError::UnknownChange(id.to_string()));
        }
        if let Some(ChangeKind::CreateCollection(c)) = gone.map(|g| g.kind) {
            self.pending.retain(|p| match &p.kind {
                ChangeKind::CreateIndex(i) => i.collection != c.name,
                ChangeKind::AddReference(r) => {
                    r.from_collection != c.name && r.to_collection != c.name
                }
                ChangeKind::CreateCollection(_) => true,
            });
        }
        Ok(())
    }

    pub fn clear(&mut self) {
        self.pending.clear();
    }

    /// Re-check every staged change against the applied schema, as a dry
    /// run does. Returns each error found.
    fn dry_run(&self) -> DryRun {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut kept: Vec<PendingChange> = Vec::new();
        for change in &self.pending {
            match self.check(&kept, change.kind.clone()) {
                Ok(_) => kept.push(change.clone()),
                Err(e) => errors.push(format!("{}: {e}", change.title())),
            }
        }
        let schema = self.schema().with_pending(&self.pending);
        for r in schema.references.iter().filter(|r| r.state.is_draft()) {
            let Some(target) = schema.collection(&r.to_collection) else {
                continue;
            };
            let keyed = target.field(&r.to_field).is_some_and(|f| f.primary)
                || target.indexes.iter().any(|i| {
                    i.unique && i.fields.first().map(String::as_str) == Some(r.to_field.as_str())
                });
            if !keyed {
                warnings.push(format!(
                    "{}.{} is not a primary or unique field, so a value can match many records.",
                    r.to_collection, r.to_field
                ));
            }
        }
        if errors.is_empty() {
            DryRun::Completed { warnings }
        } else {
            DryRun::Failed { errors }
        }
    }

    /// What reviewing the staged changes reports, for a session running as
    /// `role`. A read-only session cannot apply.
    pub fn review(&self, role: &str, read_only: bool) -> ChangeReview {
        let (blocks, unsupported) = designer_ddl::blocks(&self.pending);
        ChangeReview {
            blocks,
            unsupported,
            dependents: Vec::new(),
            scan: None,
            locks: None,
            dry_run: self.dry_run(),
            permission: role.to_string(),
            target: DATABASE.to_string(),
            blocked: read_only
                .then(|| format!("{role} has read-only access. Applying needs CREATE permission.")),
        }
    }

    /// Apply the staged changes one after another. The first change that
    /// does not apply stops the run: earlier ones stay applied, the rest
    /// stay staged.
    pub fn apply(&mut self, read_only: bool) -> Result<ApplyReport, StudioError> {
        if read_only {
            return Err(StudioError::PermissionDenied {
                privilege: "CREATE",
                collection: DATABASE.to_string(),
            });
        }
        let mut applied = Vec::new();
        let mut failed = None;
        while !self.pending.is_empty() {
            let change = self.pending[0].clone();
            // Re-check against what the database holds now: another session
            // may have taken a name since the change was staged.
            let ok = self.check(&[], change.kind.clone());
            let kind = match ok {
                Ok(kind) => kind,
                Err(e) => {
                    failed = Some((change.id.clone(), e.to_string()));
                    break;
                }
            };
            match kind {
                ChangeKind::CreateCollection(new) => self.created.push(new.to_design().applied()),
                ChangeKind::CreateIndex(index) => self.indexes.push(DesignIndex {
                    state: ObjectState::Applied,
                    ..index
                }),
                ChangeKind::AddReference(r) => {
                    self.references.push(LogicalReference {
                        id: format!("ref-{}", self.references.len() + 1),
                        state: ObjectState::Applied,
                        ..r
                    });
                }
            }
            applied.push(change.id.clone());
            self.pending.remove(0);
        }
        Ok(ApplyReport { applied, failed })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::designer_new::{ConstraintKind, NewCollection, SchemaMode};

    fn index(name: &str, collection: &str, field: &str, kind: IndexKind) -> ChangeKind {
        ChangeKind::CreateIndex(DesignIndex {
            name: name.into(),
            collection: collection.into(),
            kind,
            unique: false,
            fields: vec![field.into()],
            vector: None,
            state: ObjectState::Draft,
        })
    }

    fn shipments() -> NewCollection {
        let mut f = NewCollection::blank("analytics");
        f.name = "shipments_archive".into();
        f.schema = SchemaMode::Strict;
        f.add_field("order_id", FieldType::Uuid, true, false);
        f.add_constraint(ConstraintKind::ForeignKey);
        if let Some(crate::models::designer_new::Constraint::ForeignKey {
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

    fn empty() -> DesignerStore {
        let mut s = DesignerStore::seeded();
        s.clear();
        s
    }

    #[test]
    fn the_design_opens_with_one_staged_full_text_index() {
        let s = DesignerStore::seeded();
        assert_eq!(s.pending().len(), 1);
        assert_eq!(s.pending()[0].title(), "Create index idx_articles_body");
        assert!(
            s.schema().index("idx_articles_body").is_none(),
            "staging leaves the live schema alone"
        );
    }

    #[test]
    fn staging_rejects_a_taken_index_name_across_collections() {
        let mut s = empty();
        let e = s.stage(
            None,
            index("idx_users_email", "orders", "id", IndexKind::Secondary),
        );
        assert!(
            matches!(e, Err(StudioError::Rejected { reason }) if reason.contains("already exists"))
        );
    }

    #[test]
    fn a_staged_index_name_is_taken_too() {
        let mut s = DesignerStore::seeded();
        let e = s.stage(
            None,
            index(
                "idx_articles_body",
                "articles",
                "title",
                IndexKind::FullText,
            ),
        );
        assert!(e.is_err());
    }

    #[test]
    fn replacing_a_draft_may_keep_its_own_name() {
        let mut s = DesignerStore::seeded();
        let id = s.pending()[0].id.clone();
        let kept = s.stage(
            Some(&id),
            index(
                "idx_articles_body",
                "articles",
                "title",
                IndexKind::FullText,
            ),
        );
        assert!(kept.is_ok());
        assert_eq!(s.pending().len(), 1);
    }

    #[test]
    fn an_index_needs_a_field_it_can_sit_on() {
        let mut s = empty();
        let e = s.stage(
            None,
            index("idx_a", "articles", "embedding", IndexKind::FullText),
        );
        assert!(
            matches!(e, Err(StudioError::Rejected { reason }) if reason.contains("cannot sit on"))
        );
        let ok = s.stage(
            None,
            index("idx_a", "articles", "embedding", IndexKind::Vector),
        );
        match ok.map(|c| c.kind) {
            Ok(ChangeKind::CreateIndex(i)) => {
                assert_eq!(i.vector.map(|v| v.dimensions), Some(768));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn a_reference_may_point_at_a_staged_collection() {
        let mut s = empty();
        let mut changes = shipments().into_changes();
        let first = changes.remove(0);
        s.stage(None, first).expect("collection stages");
        for c in changes {
            s.stage(None, c).expect("reference to orders.id stages");
        }
        assert_eq!(s.pending().len(), 2);
    }

    #[test]
    fn discarding_a_collection_drops_what_hangs_on_it() {
        let mut s = empty();
        let mut ids = Vec::new();
        for c in shipments().into_changes() {
            ids.push(s.stage(None, c).expect("stages").id);
        }
        s.discard(&ids[0]).expect("discards");
        assert!(s.pending().is_empty(), "the reference went with it");
        assert!(s.discard(&ids[0]).is_err());
    }

    #[test]
    fn review_reports_what_the_server_does_not() {
        let mut s = empty();
        for c in shipments().into_changes() {
            s.stage(None, c).expect("stages");
        }
        let r = s.review("admin", false);
        assert_eq!(r.blocks.len(), 2);
        assert!(r.scan.is_none() && r.locks.is_none());
        assert!(r.dependents.is_empty());
        assert!(r.can_apply());
        assert!(r.script().contains("CREATE COLLECTION shipments_archive ("));
        assert!(!r.script().contains("FOREIGN KEY"));
    }

    #[test]
    fn a_reference_to_a_non_key_field_warns() {
        let mut s = empty();
        s.stage(
            None,
            ChangeKind::AddReference(LogicalReference {
                id: String::new(),
                from_collection: "articles".into(),
                from_field: "title".into(),
                to_collection: "orders".into(),
                to_field: "customer_id".into(),
                state: ObjectState::Draft,
            }),
        )
        .expect("stages");
        match s.review("admin", false).dry_run {
            DryRun::Completed { warnings } => assert_eq!(warnings.len(), 1),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn a_read_only_session_cannot_apply() {
        let mut s = DesignerStore::seeded();
        assert!(s.review("analyst", true).blocked.is_some());
        assert!(matches!(
            s.apply(true),
            Err(StudioError::PermissionDenied { .. })
        ));
        assert_eq!(s.pending().len(), 1, "nothing was applied");
    }

    #[test]
    fn applying_adds_to_the_schema_and_the_catalog() {
        let mut s = empty();
        for c in shipments().into_changes() {
            s.stage(None, c).expect("stages");
        }
        let report = s.apply(false).expect("applies");
        assert_eq!(report.applied.len(), 2);
        assert!(report.failed.is_none());
        assert!(s.pending().is_empty());
        assert_eq!(s.created()[0].name, "shipments_archive");
        let schema = s.schema();
        let c = schema.collection("shipments_archive").expect("created");
        assert_eq!(c.state, ObjectState::Applied);
        assert_eq!(schema.references.len(), 2);
        assert!(
            schema
                .references
                .iter()
                .all(|r| r.state == ObjectState::Applied)
        );
        let ids: std::collections::BTreeSet<_> =
            schema.references.iter().map(|r| r.id.clone()).collect();
        assert_eq!(ids.len(), 2, "reference ids are unique");
    }

    #[test]
    fn apply_stops_at_the_first_change_that_drifted() {
        let mut s = empty();
        s.stage(
            None,
            index("idx_one", "orders", "customer_id", IndexKind::Secondary),
        )
        .expect("stages");
        s.stage(
            None,
            index("idx_two", "orders", "total", IndexKind::Secondary),
        )
        .expect("stages");
        // Another session takes the second name before this one applies.
        s.indexes.push(DesignIndex {
            name: "idx_two".into(),
            collection: "users".into(),
            kind: IndexKind::Secondary,
            unique: false,
            fields: vec!["country".into()],
            vector: None,
            state: ObjectState::Applied,
        });
        let dry = s.review("admin", false);
        assert!(matches!(dry.dry_run, DryRun::Failed { .. }));
        assert!(!dry.can_apply());
        let report = s.apply(false).expect("reports");
        assert_eq!(report.applied.len(), 1);
        assert!(report.failed.is_some());
        assert_eq!(s.pending().len(), 1, "the failed change stays staged");
    }
}
