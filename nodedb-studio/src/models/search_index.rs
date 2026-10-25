//! Search indexes: the text and vector indexes of a collection, their
//! definitions, what the server observed about them, and their build
//! history.
//!
//! Three facts stay apart. A definition is schema metadata. A state and a
//! coverage are observations: Ready reports the index's state and never
//! proves that every record is indexed. A run is operational history. A
//! value the server did not report reads `Not reported`, never zero.

use crate::models::vector::Metric;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexKind {
    FullText,
    Vector,
}

impl IndexKind {
    pub fn label(self) -> &'static str {
        match self {
            IndexKind::FullText => "Full-text",
            IndexKind::Vector => "Vector",
        }
    }
}

/// The state the server reports for an index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexState {
    Ready,
    // A server reports these while it builds or after a build fails. The
    // fixtures hold only a ready index.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "services::nodedb::search builds it from the server's index state; that service is a stub today"
        )
    )]
    Building,
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "services::nodedb::search builds it from the server's index state; that service is a stub today"
        )
    )]
    Error,
}

impl IndexState {
    pub fn label(self) -> &'static str {
        match self {
            IndexState::Ready => "Ready",
            IndexState::Building => "Building",
            IndexState::Error => "Error",
        }
    }
}

/// A statistic the server may not report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observed<T> {
    /// What the server reported. The fixtures report no count.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "services::nodedb::search fills it from the server's index statistics; that service is a stub today"
        )
    )]
    Value(T),
    NotReported,
}

impl<T: Copy> Observed<T> {
    pub fn text(self, show: impl FnOnce(T) -> String) -> String {
        match self {
            Observed::Value(v) => show(v),
            Observed::NotReported => "Not reported".to_string(),
        }
    }
}

/// What differs between the two kinds of definition.
#[derive(Debug, Clone, PartialEq)]
pub enum Definition {
    FullText {
        analyzer: String,
        binding_revision: u32,
    },
    Vector {
        dimensions: u32,
        metric: Metric,
        model: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchIndex {
    pub name: String,
    pub collection: String,
    pub kind: IndexKind,
    pub fields: Vec<String>,
    pub state: IndexState,
    pub definition: Definition,
    /// The statement that defines the index.
    pub ddl: String,
    /// When the server last observed the index. Seconds since the epoch.
    pub observed_at: i64,
    /// Documents the index holds. Not the collection's row count.
    pub indexed_documents: Observed<u64>,
    /// Records the index could cover, from the same observation.
    pub eligible_documents: Observed<u64>,
}

impl SearchIndex {
    /// The share of eligible records the index holds, or `Not reported`. A
    /// share needs both counts from one observation.
    pub fn coverage_text(&self) -> String {
        match (self.indexed_documents, self.eligible_documents) {
            (Observed::Value(held), Observed::Value(eligible)) if eligible > 0 => {
                format!("{:.1}%", held as f64 * 100.0 / eligible as f64)
            }
            (Observed::Value(_) | Observed::NotReported, _) => "Not reported".to_string(),
        }
    }

    /// `title, body`.
    pub fn fields_label(&self) -> String {
        self.fields.join(", ")
    }
}

/// What a connection holds for one collection.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexOverview {
    pub collection: String,
    pub indexes: Vec<SearchIndex>,
}

impl crate::services::async_state::IsEmpty for IndexOverview {
    fn is_empty(&self) -> bool {
        self.indexes.is_empty()
    }
}

impl IndexOverview {
    pub fn index(&self, name: &str) -> Option<&SearchIndex> {
        self.indexes.iter().find(|i| i.name == name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunState {
    Queued,
    /// A server reports it while a build runs.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "services::nodedb::search builds it from a running server build; that service is a stub today"
        )
    )]
    Running,
    Completed,
    Cancelled,
    Error {
        reason: String,
    },
}

impl RunState {
    pub fn label(&self) -> &'static str {
        match self {
            RunState::Queued => "Queued",
            RunState::Running => "Running",
            RunState::Completed => "Completed",
            RunState::Cancelled => "Cancelled",
            RunState::Error { .. } => "Error",
        }
    }

    /// `Error · quota exceeded`, or the label alone.
    pub fn text(&self) -> String {
        match self {
            RunState::Error { reason } => format!("Error · {reason}"),
            RunState::Queued | RunState::Running | RunState::Completed | RunState::Cancelled => {
                self.label().to_string()
            }
        }
    }
}

/// One build of an index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildRun {
    /// Stable: `build-014`.
    pub id: String,
    pub index: String,
    /// Seconds since the epoch, UTC.
    pub requested_at: i64,
    pub finished_at: Option<i64>,
    pub state: RunState,
    pub requested_by: String,
    /// The revision of the definition the run built.
    pub target_revision: u32,
    pub documents: Observed<u64>,
}

impl crate::services::async_state::IsEmpty for BuildRun {
    fn is_empty(&self) -> bool {
        false
    }
}

/// The runs of one index, newest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunHistory {
    pub index: String,
    pub runs: Vec<BuildRun>,
}

impl crate::services::async_state::IsEmpty for RunHistory {
    fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }
}

/// The plan a rebuild would follow, shown before a job exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebuildPlan {
    pub index: String,
    pub collection: String,
    /// The data the build reads: `articles at revision 412`.
    pub source_snapshot: String,
    pub target_revision: u32,
    pub resource_estimate: Option<String>,
    /// What queries see while the build runs.
    pub availability: Option<String>,
    /// How the new index replaces the serving one.
    pub publication: Option<String>,
}

impl crate::services::async_state::IsEmpty for RebuildPlan {
    fn is_empty(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_error_run_reads_with_its_reason() {
        let s = RunState::Error {
            reason: "quota exceeded".into(),
        };
        assert_eq!(s.text(), "Error · quota exceeded");
        assert_eq!(s.label(), "Error");
        assert_eq!(RunState::Completed.text(), "Completed");
    }

    #[test]
    fn server_reported_states_have_labels() {
        assert_eq!(IndexState::Building.label(), "Building");
        assert_eq!(IndexState::Error.label(), "Error");
        assert_eq!(RunState::Running.text(), "Running");
        assert_eq!(Observed::Value(42u64).text(|n| n.to_string()), "42");
    }

    fn index(indexed: Observed<u64>, eligible: Observed<u64>) -> SearchIndex {
        SearchIndex {
            name: "idx".into(),
            collection: "c".into(),
            kind: IndexKind::FullText,
            fields: vec!["a".into()],
            state: IndexState::Ready,
            definition: Definition::FullText {
                analyzer: "English".into(),
                binding_revision: 1,
            },
            ddl: String::new(),
            observed_at: 0,
            indexed_documents: indexed,
            eligible_documents: eligible,
        }
    }

    #[test]
    fn coverage_needs_both_counts_and_ready_alone_reports_none() {
        let none = index(Observed::NotReported, Observed::NotReported);
        assert_eq!(none.state, IndexState::Ready);
        assert_eq!(none.coverage_text(), "Not reported");
        let held_only = index(Observed::Value(90), Observed::NotReported);
        assert_eq!(held_only.coverage_text(), "Not reported");
        let both = index(Observed::Value(90), Observed::Value(120));
        assert_eq!(both.coverage_text(), "75.0%");
        assert_eq!(
            index(Observed::Value(0), Observed::Value(0)).coverage_text(),
            "Not reported"
        );
    }

    #[test]
    fn an_unreported_value_never_reads_as_zero() {
        assert_eq!(
            Observed::<u64>::NotReported.text(|v| v.to_string()),
            "Not reported"
        );
        assert_eq!(Observed::Value(0u64).text(|v| v.to_string()), "0");
    }
}
