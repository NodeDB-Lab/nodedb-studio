//! The indexes of `articles`, their build history, and the rebuild plan.
//! A rebuild job a person creates joins the history in memory.

use crate::models::format::epoch;
use crate::models::search_index::{
    BuildRun, Definition, IndexKind, IndexOverview, IndexState, Observed, RebuildPlan, RunHistory,
    RunState, SearchIndex,
};
use crate::models::vector::Metric;
use crate::services::error::StudioError;

pub const COLLECTION: &str = "articles";
pub const TEXT_INDEX: &str = "idx_articles_text";
pub const VECTOR_INDEX: &str = "idx_articles_embedding";
const REQUESTER: &str = "admin";

fn observed_at() -> i64 {
    super::run::observed_at()
}

fn unknown(index: &str) -> StudioError {
    StudioError::UnknownObject {
        kind: "search index",
        id: index.to_string(),
    }
}

pub fn overview(collection: &str) -> Result<IndexOverview, StudioError> {
    if collection != COLLECTION {
        return Err(StudioError::UnknownObject {
            kind: "collection",
            id: collection.to_string(),
        });
    }
    Ok(IndexOverview {
        collection: collection.to_string(),
        indexes: vec![
            SearchIndex {
                name: TEXT_INDEX.into(),
                collection: collection.into(),
                kind: IndexKind::FullText,
                fields: vec!["title".into(), "body".into()],
                state: IndexState::Ready,
                definition: Definition::FullText {
                    analyzer: "English".into(),
                    binding_revision: 7,
                },
                ddl: format!(
                    "CREATE FULLTEXT INDEX {TEXT_INDEX}\nON {collection} (title, body)\nANALYZER 'english';"
                ),
                observed_at: observed_at(),
                indexed_documents: Observed::NotReported,
                eligible_documents: Observed::NotReported,
            },
            SearchIndex {
                name: VECTOR_INDEX.into(),
                collection: collection.into(),
                kind: IndexKind::Vector,
                fields: vec!["embedding".into()],
                state: IndexState::Ready,
                definition: Definition::Vector {
                    dimensions: 768,
                    metric: Metric::Cosine,
                    model: Some("support-embed-v2".into()),
                },
                ddl: format!(
                    "CREATE VECTOR INDEX {VECTOR_INDEX}\nON {collection} (embedding)\nMETRIC cosine DIM 768;"
                ),
                observed_at: observed_at(),
                indexed_documents: Observed::NotReported,
                eligible_documents: Observed::NotReported,
            },
        ],
    })
}

fn run(
    id: &str,
    index: &str,
    requested: (u32, u32, u32, u32, u32),
    minutes: i64,
    state: RunState,
) -> BuildRun {
    let (month, day, hour, minute, second) = requested;
    let at = epoch(2026, month, day, hour, minute, second);
    BuildRun {
        id: id.into(),
        index: index.into(),
        requested_at: at,
        finished_at: match state {
            RunState::Queued | RunState::Running => None,
            RunState::Completed | RunState::Cancelled | RunState::Error { .. } => {
                Some(at + minutes * 60 + 42)
            }
        },
        state,
        requested_by: REQUESTER.into(),
        target_revision: 7,
        documents: Observed::NotReported,
    }
}

/// The seeded runs, newest first.
pub fn seeded_runs() -> Vec<BuildRun> {
    vec![
        run(
            "build-014",
            TEXT_INDEX,
            (6, 13, 13, 21, 0),
            2,
            RunState::Completed,
        ),
        run(
            "build-013",
            TEXT_INDEX,
            (6, 12, 9, 10, 0),
            0,
            RunState::Error {
                reason: "quota exceeded".into(),
            },
        ),
        run(
            "build-012",
            VECTOR_INDEX,
            (6, 11, 10, 24, 31),
            38,
            RunState::Completed,
        ),
        run(
            "build-011",
            TEXT_INDEX,
            (6, 10, 16, 40, 0),
            2,
            RunState::Completed,
        ),
        run(
            "build-010",
            VECTOR_INDEX,
            (6, 8, 9, 5, 12),
            41,
            RunState::Completed,
        ),
        run(
            "build-009",
            TEXT_INDEX,
            (6, 3, 11, 2, 0),
            2,
            RunState::Completed,
        ),
        run(
            "build-008",
            VECTOR_INDEX,
            (5, 30, 14, 50, 0),
            40,
            RunState::Completed,
        ),
        run(
            "build-007",
            TEXT_INDEX,
            (5, 29, 8, 15, 0),
            3,
            RunState::Cancelled,
        ),
    ]
}

/// What the mock holds beyond the fixtures: the runs created this session.
#[derive(Debug, Default)]
pub struct SearchStore {
    created: Vec<BuildRun>,
    /// Counts searches, so each result names its own run.
    pub executions: u32,
}

impl SearchStore {
    pub fn next_execution(&mut self) -> u32 {
        self.executions += 1;
        self.executions
    }

    fn all_runs(&self) -> Vec<BuildRun> {
        let mut runs: Vec<BuildRun> = self.created.iter().rev().cloned().collect();
        runs.extend(seeded_runs());
        runs
    }

    pub fn history(&self, index: &str) -> Result<RunHistory, StudioError> {
        if index != TEXT_INDEX && index != VECTOR_INDEX {
            return Err(unknown(index));
        }
        Ok(RunHistory {
            index: index.to_string(),
            runs: self
                .all_runs()
                .into_iter()
                .filter(|r| r.index == index)
                .collect(),
        })
    }

    pub fn run(&self, id: &str) -> Result<BuildRun, StudioError> {
        self.all_runs()
            .into_iter()
            .find(|r| r.id == id)
            .ok_or_else(|| StudioError::UnknownObject {
                kind: "build run",
                id: id.to_string(),
            })
    }

    /// Create a rebuild job from a plan the person reviewed. A plan for a
    /// revision the index no longer targets is refused: nothing is created.
    pub fn start_rebuild(
        &mut self,
        index: &str,
        target_revision: u32,
    ) -> Result<BuildRun, StudioError> {
        let plan = plan(index)?;
        if plan.target_revision != target_revision {
            return Err(StudioError::StaleRevision {
                object: index.to_string(),
                expected: u64::from(target_revision),
                stored: u64::from(plan.target_revision),
            });
        }
        if self
            .all_runs()
            .iter()
            .any(|r| r.index == index && matches!(r.state, RunState::Queued | RunState::Running))
        {
            return Err(StudioError::Rejected {
                reason: format!("{index} already has a build queued or running."),
            });
        }
        let number = 14 + 1 + self.created.len();
        let (month, day, hour, minute, second) = (6, 13, 13, 41, self.created.len() as u32);
        let mut created = run(
            &format!("build-{number:03}"),
            index,
            (month, day, hour, minute, second),
            0,
            RunState::Queued,
        );
        created.target_revision = plan.target_revision;
        self.created.push(created.clone());
        Ok(created)
    }
}

/// What a rebuild of `index` would do, for review before a job exists.
pub fn plan(index: &str) -> Result<RebuildPlan, StudioError> {
    let (revision, availability) = match index {
        TEXT_INDEX => (
            7,
            "Queries read the serving index until the new one is published.",
        ),
        VECTOR_INDEX => (
            3,
            "Queries read the serving index until the new one is published.",
        ),
        other => return Err(unknown(other)),
    };
    Ok(RebuildPlan {
        index: index.to_string(),
        collection: COLLECTION.to_string(),
        source_snapshot: format!("{COLLECTION} at the start of the job"),
        target_revision: revision,
        resource_estimate: None,
        availability: Some(availability.to_string()),
        publication: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_indexes_share_one_collection_and_report_no_coverage() {
        let o = overview("articles").expect("overview");
        let names: Vec<&str> = o.indexes.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, vec![TEXT_INDEX, VECTOR_INDEX]);
        for i in &o.indexes {
            assert_eq!(i.state, IndexState::Ready);
            assert_eq!(
                i.indexed_documents,
                Observed::NotReported,
                "Ready is not coverage"
            );
        }
        assert!(overview("users").is_err());
    }

    #[test]
    fn the_text_history_is_the_designed_runs_newest_first() {
        let s = SearchStore::default();
        let h = s.history(TEXT_INDEX).expect("history");
        let top: Vec<(&str, String)> = h
            .runs
            .iter()
            .take(3)
            .map(|r| (r.id.as_str(), r.state.text()))
            .collect();
        assert_eq!(
            top,
            vec![
                ("build-014", "Completed".to_string()),
                ("build-013", "Error · quota exceeded".to_string()),
                ("build-011", "Completed".to_string()),
            ]
        );
        assert!(
            h.runs
                .windows(2)
                .all(|w| w[0].requested_at > w[1].requested_at)
        );
        assert_eq!(h.runs[0].requested_at, epoch(2026, 6, 13, 13, 21, 0));
    }

    #[test]
    fn run_ids_are_unique_and_read_back() {
        let s = SearchStore::default();
        let mut ids: Vec<String> = seeded_runs().into_iter().map(|r| r.id).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n);
        assert_eq!(s.run("build-013").expect("run").index, TEXT_INDEX);
        assert!(s.run("build-999").is_err());
    }

    #[test]
    fn a_rebuild_joins_the_history_queued_and_blocks_a_second() {
        let mut s = SearchStore::default();
        let job = s.start_rebuild(TEXT_INDEX, 7).expect("created");
        assert_eq!(
            (job.id.as_str(), job.state.clone()),
            ("build-015", RunState::Queued)
        );
        assert_eq!(
            s.history(TEXT_INDEX).expect("history").runs[0].id,
            "build-015"
        );
        assert_eq!(
            s.history(VECTOR_INDEX).expect("history").runs[0].id,
            "build-012"
        );
        assert!(matches!(
            s.start_rebuild(TEXT_INDEX, 7),
            Err(StudioError::Rejected { .. })
        ));
    }

    #[test]
    fn a_plan_for_another_revision_creates_nothing() {
        let mut s = SearchStore::default();
        assert!(matches!(
            s.start_rebuild(TEXT_INDEX, 6),
            Err(StudioError::StaleRevision { .. })
        ));
        assert_eq!(
            s.history(TEXT_INDEX).expect("history").runs[0].id,
            "build-014"
        );
        assert!(s.start_rebuild("idx_nope", 1).is_err());
    }

    #[test]
    fn the_plan_states_what_it_does_not_know() {
        let p = plan(TEXT_INDEX).expect("plan");
        assert_eq!(p.target_revision, 7);
        assert!(p.resource_estimate.is_none() && p.publication.is_none());
    }
}
