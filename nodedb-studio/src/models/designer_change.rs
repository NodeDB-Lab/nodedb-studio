//! Staged schema changes and what a review of them reports.
//!
//! A change is staged, never applied, until the user has reviewed it. A
//! staged change leaves the live database as it was. Reference changes are
//! workspace metadata: they appear in the review and run no DDL.

use serde::{Deserialize, Serialize};

use crate::models::designer::{DesignIndex, LogicalReference};
use crate::models::designer_new::NewCollection;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeKind {
    CreateCollection(NewCollection),
    CreateIndex(DesignIndex),
    AddReference(LogicalReference),
}

/// A staged change with the id the backend gave it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingChange {
    pub id: String,
    pub kind: ChangeKind,
}

impl PendingChange {
    /// The line the review list opens with.
    pub fn title(&self) -> String {
        match &self.kind {
            ChangeKind::CreateCollection(c) => format!("Create {} collection", c.name),
            ChangeKind::CreateIndex(i) => format!("Create index {}", i.name),
            ChangeKind::AddReference(_) => "Add logical reference".to_string(),
        }
    }

    /// The line under the title.
    pub fn detail(&self) -> String {
        match &self.kind {
            ChangeKind::CreateCollection(c) => {
                let n = c.fields.len();
                format!(
                    "{} collection with {n} field{}",
                    c.profile().badge(),
                    if n == 1 { "" } else { "s" }
                )
            }
            ChangeKind::CreateIndex(i) => format!("{} on {}", i.summary(), i.collection),
            ChangeKind::AddReference(r) => format!(
                "{}.{} references {}.{}",
                r.from_collection, r.from_field, r.to_collection, r.to_field
            ),
        }
    }

    /// Whether applying it changes the database. A reference changes the
    /// workspace's metadata only.
    pub fn runs_ddl(&self) -> bool {
        match &self.kind {
            ChangeKind::CreateCollection(_) | ChangeKind::CreateIndex(_) => true,
            ChangeKind::AddReference(_) => false,
        }
    }
}

/// The DDL one change generates, line by line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DdlBlock {
    pub change_id: String,
    pub lines: Vec<String>,
    /// A block of comments for a change that runs no statement.
    pub metadata: bool,
}

/// A change the generator cannot express. The review reports it and applies
/// nothing for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsupported {
    pub change_id: String,
    pub reason: String,
}

/// How a dry run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DryRun {
    /// Every statement parsed and passed the server's checks. Warnings are
    /// listed, and may be none.
    Completed { warnings: Vec<String> },
    /// A statement did not pass. Nothing was applied.
    Failed { errors: Vec<String> },
}

/// What a review reports about staged changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeReview {
    /// The catalog revision plus the staged set this review read. Apply
    /// names it, and the server refuses it once either has changed.
    pub revision: u64,
    pub blocks: Vec<DdlBlock>,
    pub unsupported: Vec<Unsupported>,
    /// Objects that depend on something a change touches. Empty when none.
    pub dependents: Vec<String>,
    /// The scan or backfill cost. `None` when the server does not report it.
    pub scan: Option<String>,
    /// The locks the statements take. `None` when the server does not report
    /// it.
    pub locks: Option<String>,
    pub dry_run: DryRun,
    /// The role the session runs as.
    pub permission: String,
    /// The database the statements run against.
    pub target: String,
    /// Why the session cannot apply, when it cannot.
    pub blocked: Option<String>,
}

impl ChangeReview {
    /// The statements, as one script.
    pub fn script(&self) -> String {
        self.blocks
            .iter()
            .map(|b| b.lines.join("\n"))
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Whether Apply is available. A failed dry run, an unsupported change,
    /// or a blocked session each stop it.
    pub fn can_apply(&self) -> bool {
        self.blocked.is_none()
            && self.unsupported.is_empty()
            && !matches!(self.dry_run, DryRun::Failed { .. })
    }
}

/// What applying did. The statements run one after another, and the server
/// does not promise they run as one transaction: a failure leaves the
/// earlier ones applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyReport {
    /// Ids of the changes now in the database or the workspace.
    pub applied: Vec<String>,
    /// The first change that did not apply, with the server's reason.
    /// Everything after it stays staged.
    pub failed: Option<(String, String)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::designer::ObjectState;

    fn reference() -> PendingChange {
        PendingChange {
            id: "chg-2".into(),
            kind: ChangeKind::AddReference(LogicalReference {
                id: String::new(),
                from_collection: "shipments".into(),
                from_field: "order_id".into(),
                to_collection: "orders".into(),
                to_field: "id".into(),
                state: ObjectState::Draft,
            }),
        }
    }

    #[test]
    fn a_reference_runs_no_ddl() {
        let c = reference();
        assert!(!c.runs_ddl());
        assert_eq!(c.detail(), "shipments.order_id references orders.id");
    }

    fn review(dry_run: DryRun, blocked: Option<&str>) -> ChangeReview {
        ChangeReview {
            revision: 1,
            blocks: vec![],
            unsupported: vec![],
            dependents: vec![],
            scan: None,
            locks: None,
            dry_run,
            permission: "admin".into(),
            target: "analytics".into(),
            blocked: blocked.map(str::to_string),
        }
    }

    #[test]
    fn apply_needs_a_passing_dry_run_and_a_permitted_session() {
        assert!(review(DryRun::Completed { warnings: vec![] }, None).can_apply());
        assert!(
            !review(
                DryRun::Failed {
                    errors: vec!["x".into()]
                },
                None
            )
            .can_apply()
        );
        assert!(!review(DryRun::Completed { warnings: vec![] }, Some("read-only")).can_apply());
    }
}
