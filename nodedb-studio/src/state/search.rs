//! Search UI state, provided by `App` above the router so the three tabs keep
//! their queries, results, and selections while the user is on another page.
//!
//! A result belongs to the request that produced it. Changing the form never
//! replaces the shown result: it marks it stale until the search runs again.
//! A late answer to a search that was canceled or replaced no longer matches
//! `pending` and is dropped.

use dioxus::prelude::*;

use crate::models::search::{SearchMode, SearchRequest, SearchResult, VectorInput};
use crate::models::search_analyze::{Analysis, AnalyzeRequest, InputRole};
use crate::models::search_compare::Comparison;
use crate::models::search_form::{SearchForm, VectorEditor};

/// A tab of the Result details panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DetailsTab {
    Record,
    #[default]
    Ranking,
    Sql,
}

impl DetailsTab {
    pub const ALL: [DetailsTab; 3] = [DetailsTab::Record, DetailsTab::Ranking, DetailsTab::Sql];

    pub fn label(self) -> &'static str {
        match self {
            DetailsTab::Record => "Record",
            DetailsTab::Ranking => "Ranking",
            DetailsTab::Sql => "SQL",
        }
    }
}

/// How the ranked records show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResultsView {
    #[default]
    List,
    Table,
}

/// The filter being added.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FilterDraft {
    pub field: String,
    pub value: String,
}

/// The Compare modes sheet.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CompareUi {
    /// The request the three runs derive from.
    pub pending: Option<(u32, SearchRequest)>,
    pub seq: u32,
    pub shown: Option<Comparison>,
    pub error: Option<String>,
}

/// The Search tab.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ResultsUi {
    /// The collection the user chose. `None` is the first the connection offers.
    pub collection: Option<String>,
    pub form: SearchForm,
    /// The vector input under edit, and the reason it cannot apply.
    pub vector_edit: Option<VectorEditor>,
    pub vector_error: Option<String>,
    /// The filter under edit.
    pub filter_edit: Option<FilterDraft>,
    /// Whether the advanced ranking controls show.
    pub advanced: bool,
    /// Whether the user has pressed Search with a form that is not ready.
    pub tried: bool,
    /// The search being run. Cleared when it finishes or is canceled.
    pub pending: Option<(u32, SearchRequest)>,
    /// Counts searches, so a repeat is a new run.
    pub seq: u32,
    /// The last search that finished.
    pub shown: Option<SearchResult>,
    /// The last search's error, shown over the kept result.
    pub error: Option<String>,
    /// The last search was canceled. The result before it is kept.
    pub cancelled: bool,
    pub selected: Option<String>,
    pub details: DetailsTab,
    pub view: ResultsView,
    /// The Compare modes sheet, when open.
    pub compare: Option<CompareUi>,
}

/// The Analyze tab.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnalyzeUi {
    pub collection: Option<String>,
    pub index: Option<String>,
    pub field: Option<String>,
    pub text: String,
    pub role: InputRole,
    pub pending: Option<(u32, AnalyzeRequest)>,
    pub seq: u32,
    pub shown: Option<Analysis>,
    pub error: Option<String>,
    /// The token row the reader picked.
    pub token: Option<usize>,
}

/// A tab of the Index details panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IndexTab {
    #[default]
    Definition,
    Coverage,
    Activity,
}

impl IndexTab {
    pub const ALL: [IndexTab; 3] = [IndexTab::Definition, IndexTab::Coverage, IndexTab::Activity];

    pub fn label(self) -> &'static str {
        match self {
            IndexTab::Definition => "Definition",
            IndexTab::Coverage => "Coverage",
            IndexTab::Activity => "Activity",
        }
    }
}

/// The Indexes tab.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct IndexesUi {
    pub collection: Option<String>,
    pub filter: String,
    /// The selected index, by name.
    pub selected: Option<String>,
    pub tab: IndexTab,
    /// The index the Build history shows. `None` follows the selection.
    pub history_index: Option<String>,
    /// Whether the history lists every run or the latest three.
    pub all_runs: bool,
    /// The run whose details are open.
    pub run: Option<String>,
    /// The index whose rebuild plan is open.
    pub plan: Option<String>,
    /// The reason the last rebuild could not be created.
    pub plan_error: Option<String>,
    /// A rebuild job is being created.
    pub plan_busy: bool,
    /// Bumped after a write, so the reads that depend on it run again.
    pub epoch: u32,
}

/// What a screenshot scene sets. `None` keeps the current state.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SearchSeed {
    pub results: Option<ResultsUi>,
    pub analyze: Option<AnalyzeUi>,
    pub indexes: Option<IndexesUi>,
}

#[derive(Clone, Copy)]
pub struct SearchUi {
    pub results: Signal<ResultsUi>,
    pub analyze: Signal<AnalyzeUi>,
    pub indexes: Signal<IndexesUi>,
}

impl SearchUi {
    /// Create the state and provide it to the tree below the caller.
    pub fn provide() -> Self {
        use_context_provider(|| Self {
            results: Signal::new(ResultsUi::default()),
            analyze: Signal::new(AnalyzeUi::default()),
            indexes: Signal::new(IndexesUi::default()),
        })
    }

    /// Replace the state with a scene's seed.
    pub fn apply(mut self, seed: SearchSeed) {
        self.results.set(seed.results.unwrap_or_default());
        self.analyze.set(seed.analyze.unwrap_or_default());
        self.indexes.set(seed.indexes.unwrap_or_default());
    }

    /// Fill the Search tab with a query another page hands it. Nothing runs:
    /// the person presses Search. The shown result stays, marked stale.
    pub fn open_query(
        mut self,
        collection: &str,
        mode: SearchMode,
        query: &str,
        vector: VectorInput,
    ) {
        let mut r = self.results.write();
        if r.collection.as_deref() != Some(collection) {
            // Another collection's drafts and result do not carry over.
            *r = ResultsUi {
                collection: Some(collection.to_string()),
                ..ResultsUi::default()
            };
        }
        r.form.mode = mode;
        if !query.is_empty() {
            r.form.query = query.to_string();
        }
        if vector != VectorInput::None {
            r.form.vector = vector;
        }
        r.cancelled = false;
        r.error = None;
    }

    /// Fill the Analyze tab with raw text. Nothing runs.
    pub fn open_analysis(
        mut self,
        collection: &str,
        index: Option<&str>,
        field: Option<&str>,
        text: &str,
        role: InputRole,
    ) {
        let mut a = self.analyze.write();
        if a.collection.as_deref() != Some(collection) {
            *a = AnalyzeUi {
                collection: Some(collection.to_string()),
                ..AnalyzeUi::default()
            };
        }
        if let Some(i) = index {
            a.index = Some(i.to_string());
        }
        if let Some(f) = field {
            a.field = Some(f.to_string());
        }
        a.text = text.to_string();
        a.role = role;
        a.error = None;
    }

    /// Select an index on the Indexes tab.
    pub fn open_index(mut self, collection: &str, index: &str) {
        let mut i = self.indexes.write();
        i.collection = Some(collection.to_string());
        i.filter.clear();
        i.selected = Some(index.to_string());
        i.history_index = None;
        i.all_runs = false;
        i.run = None;
        i.plan = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_start_on_ranking_and_definition() {
        assert_eq!(ResultsUi::default().details, DetailsTab::Ranking);
        assert_eq!(IndexesUi::default().tab, IndexTab::Definition);
        assert!(ResultsUi::default().shown.is_none());
    }

    #[test]
    fn the_tab_labels_are_the_designs() {
        let labels: Vec<&str> = DetailsTab::ALL.iter().map(|t| t.label()).collect();
        assert_eq!(labels, vec!["Record", "Ranking", "SQL"]);
        let labels: Vec<&str> = IndexTab::ALL.iter().map(|t| t.label()).collect();
        assert_eq!(labels, vec!["Definition", "Coverage", "Activity"]);
    }
}
