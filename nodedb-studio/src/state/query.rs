//! The Query page's open tabs and what each shows.
//!
//! Provided by `App` above the router, so the tabs survive leaving the page,
//! a hidden panel keeps them, and the sidebar badge counts them.
//!
//! A tab is a draft. It can be bound to a saved query, which is a separate,
//! named object in the library. A tab is unsaved when its text, name, tags,
//! or parameters differ from what the library (or the session) last stored.

use dioxus::prelude::Signal;

use crate::models::query_plan::{ExplainMode, QueryPlan};
use crate::models::saved_query::SavedQuery;
use crate::models::sql_text::parameters;
use crate::models::workbench::{
    PAGE_SIZE, ParamBinding, ParamError, ParamType, QueryDraft, QueryRequest, QueryRun,
};

/// Bumped after any transaction change, so every reader of the session's
/// transaction (the Query toolbar, the Console) reads it again.
#[derive(Clone, Copy)]
pub struct TxEpoch(pub Signal<u32>);

/// Which view of the statement the results panel shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResultsView {
    #[default]
    Results,
    Plan,
    Messages,
}

impl ResultsView {
    pub const ALL: [ResultsView; 3] = [
        ResultsView::Results,
        ResultsView::Plan,
        ResultsView::Messages,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ResultsView::Results => "Results",
            ResultsView::Plan => "Plan",
            ResultsView::Messages => "Messages",
        }
    }
}

/// Which view the right-hand panel shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SidePane {
    #[default]
    Schema,
    Details,
}

impl SidePane {
    pub const ALL: [SidePane; 2] = [SidePane::Schema, SidePane::Details];

    pub fn label(self) -> &'static str {
        match self {
            SidePane::Schema => "Schema",
            SidePane::Details => "Details",
        }
    }
}

/// The last run of a tab's statement.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum RunState {
    #[default]
    Idle,
    Running,
    Done(Box<QueryRun>),
    Failed(String),
}

/// The last plan of a tab's statement.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum PlanState {
    #[default]
    None,
    Running,
    Ready(Box<QueryPlan>),
    Failed(String),
}

/// What the library or the session last stored for a tab.
#[derive(Debug, Clone, PartialEq)]
pub struct Baseline {
    pub title: String,
    pub sql: String,
    pub tags: Vec<String>,
    pub params: Vec<ParamBinding>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueryTab {
    pub id: u32,
    pub title: String,
    pub sql: String,
    /// The database the statement is bound to.
    pub database: String,
    /// The saved query this tab edits, if any.
    pub saved: Option<String>,
    pub params: Vec<ParamBinding>,
    pub tags: Vec<String>,
    pub baseline: Baseline,
    pub view: ResultsView,
    pub mode: ExplainMode,
    pub run: RunState,
    pub plan: PlanState,
    pub selected_op: Option<u32>,
    pub page: usize,
    pub page_size: usize,
}

/// `params` with one binding per `:name` in `sql`, keeping the type and
/// value of a name that stays.
fn synced(sql: &str, old: &[ParamBinding]) -> Vec<ParamBinding> {
    parameters(sql)
        .into_iter()
        .map(|name| {
            old.iter()
                .find(|p| p.name == name)
                .cloned()
                .unwrap_or_else(|| ParamBinding::new(&name, ParamType::Text, ""))
        })
        .collect()
}

impl QueryTab {
    fn blank(id: u32, title: &str, sql: &str, database: &str) -> Self {
        let params = synced(sql, &[]);
        Self {
            id,
            title: title.to_string(),
            sql: sql.to_string(),
            database: database.to_string(),
            saved: None,
            baseline: Baseline {
                title: title.to_string(),
                sql: sql.to_string(),
                tags: Vec::new(),
                params: params.clone(),
            },
            params,
            tags: Vec::new(),
            view: ResultsView::Results,
            mode: ExplainMode::Explain,
            run: RunState::Idle,
            plan: PlanState::None,
            selected_op: None,
            page: 0,
            page_size: PAGE_SIZE,
        }
    }

    pub fn from_draft(id: u32, draft: &QueryDraft) -> Self {
        let mut tab = Self::blank(id, &draft.title, &draft.sql, &draft.database);
        tab.baseline.sql = draft.saved_sql.clone();
        tab.saved = draft.saved_query.clone();
        tab
    }

    pub fn from_saved(id: u32, q: &SavedQuery) -> Self {
        let params = synced(&q.sql, &q.params);
        Self {
            saved: Some(q.id.clone()),
            tags: q.tags.clone(),
            baseline: Baseline {
                title: q.name.clone(),
                sql: q.sql.clone(),
                tags: q.tags.clone(),
                params: params.clone(),
            },
            params,
            ..Self::blank(id, &q.name, &q.sql, &q.database)
        }
    }

    /// Whether anything differs from what was last stored.
    pub fn dirty(&self) -> bool {
        self.title != self.baseline.title
            || self.sql != self.baseline.sql
            || self.tags != self.baseline.tags
            || self.params != self.baseline.params
    }

    /// Replace the text. The parameters follow it, and the last results go
    /// stale only when the user runs again.
    pub fn set_sql(&mut self, sql: String) {
        self.params = synced(&sql, &self.params);
        self.sql = sql;
    }

    /// The first parameter whose value does not bind.
    pub fn check_params(&self) -> Result<(), ParamError> {
        self.params.iter().try_for_each(ParamBinding::check)
    }

    pub fn request(&self, page: usize) -> QueryRequest {
        QueryRequest {
            sql: self.sql.clone(),
            params: self.params.clone(),
            database: self.database.clone(),
            page,
            page_size: self.page_size,
        }
    }

    /// Record that the library now holds this tab's content as `q`.
    pub fn mark_saved(&mut self, q: &SavedQuery) {
        self.saved = Some(q.id.clone());
        self.title = q.name.clone();
        self.tags = q.tags.clone();
        self.baseline = Baseline {
            title: q.name.clone(),
            sql: q.sql.clone(),
            tags: q.tags.clone(),
            params: q.params.clone(),
        };
    }
}

/// Every open tab, which one shows, and the page's other shared state.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct QueryWorkspace {
    loaded: bool,
    tabs: Vec<QueryTab>,
    active: Option<u32>,
    next_id: u32,
    /// The library row whose details the right panel shows.
    pub library_selected: Option<String>,
    pub side: SidePane,
    /// Bumped after a library write, so the library and details read again.
    pub library_epoch: u32,
    /// The last refusal of an action, shown above the editor.
    pub notice: Option<String>,
}

impl QueryWorkspace {
    /// The workspace the session restores.
    pub fn from_drafts(drafts: &[QueryDraft]) -> Self {
        let tabs: Vec<QueryTab> = drafts
            .iter()
            .enumerate()
            .map(|(i, d)| QueryTab::from_draft(i as u32 + 1, d))
            .collect();
        Self {
            loaded: true,
            active: tabs.first().map(|t| t.id),
            next_id: tabs.len() as u32 + 1,
            tabs,
            ..Self::default()
        }
    }

    /// Whether the session's tabs were read. Until then the sidebar badge
    /// uses the server's count.
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    pub fn tabs(&self) -> &[QueryTab] {
        &self.tabs
    }

    pub fn count(&self) -> usize {
        self.tabs.len()
    }

    pub fn active_id(&self) -> Option<u32> {
        self.active
    }

    pub fn active(&self) -> Option<&QueryTab> {
        let id = self.active?;
        self.tabs.iter().find(|t| t.id == id)
    }

    pub fn active_mut(&mut self) -> Option<&mut QueryTab> {
        let id = self.active?;
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    pub fn tab_mut(&mut self, id: u32) -> Option<&mut QueryTab> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    pub fn activate(&mut self, id: u32) {
        if self.tabs.iter().any(|t| t.id == id) {
            self.active = Some(id);
            self.notice = None;
        }
    }

    /// Open an empty tab bound to `database`, and show it.
    pub fn new_tab(&mut self, database: &str) -> u32 {
        let id = self.take_id();
        let title = format!("Untitled {id}");
        self.tabs.push(QueryTab::blank(id, &title, "", database));
        self.active = Some(id);
        self.notice = None;
        id
    }

    /// Show the tab that edits `q`, opening one when none does.
    pub fn open_saved(&mut self, q: &SavedQuery) -> u32 {
        if let Some(tab) = self.tabs.iter().find(|t| t.saved.as_deref() == Some(&q.id)) {
            let id = tab.id;
            self.activate(id);
            return id;
        }
        let id = self.take_id();
        self.tabs.push(QueryTab::from_saved(id, q));
        self.active = Some(id);
        self.notice = None;
        id
    }

    /// Close a tab. The tab to its left (or the first) takes over.
    pub fn close(&mut self, id: u32) {
        let Some(at) = self.tabs.iter().position(|t| t.id == id) else {
            return;
        };
        self.tabs.remove(at);
        if self.active == Some(id) {
            self.active = self
                .tabs
                .get(at.saturating_sub(1))
                .or_else(|| self.tabs.first())
                .map(|t| t.id);
        }
    }

    /// A saved query was removed: its tabs keep their text and lose the
    /// binding, so they read as unsaved drafts.
    pub fn unbind(&mut self, saved_id: &str) {
        for tab in self
            .tabs
            .iter_mut()
            .filter(|t| t.saved.as_deref() == Some(saved_id))
        {
            tab.saved = None;
            tab.baseline.sql.clear();
        }
        if self.library_selected.as_deref() == Some(saved_id) {
            self.library_selected = None;
        }
    }

    fn take_id(&mut self) -> u32 {
        let id = self.next_id.max(1);
        self.next_id = id + 1;
        id
    }

    /// The tab that edits saved query `id`, if one is open.
    pub fn tab_for(&self, saved_id: &str) -> Option<&QueryTab> {
        self.tabs
            .iter()
            .find(|t| t.saved.as_deref() == Some(saved_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::saved_query::QueryScope;

    fn draft(id: &str, sql: &str, saved: &str) -> QueryDraft {
        QueryDraft {
            id: id.into(),
            title: format!("tab {id}"),
            sql: sql.into(),
            saved_sql: saved.into(),
            saved_query: None,
            database: "analytics".into(),
        }
    }

    fn saved(id: &str, sql: &str) -> SavedQuery {
        SavedQuery {
            id: id.into(),
            name: "Spend".into(),
            folder: None,
            scope: QueryScope::MyWorkspace,
            owner: "Hana".into(),
            database: "analytics".into(),
            sql: sql.into(),
            params: vec![ParamBinding::new("cur", ParamType::Text, "USD")],
            tags: vec!["a".into()],
            version: 1,
            updated_at: 0,
            last_error: None,
        }
    }

    fn two() -> QueryWorkspace {
        QueryWorkspace::from_drafts(&[
            draft("a", "SELECT 1", "SELECT 1"),
            draft("b", "SELECT 2", "SELECT 0"),
        ])
    }

    #[test]
    fn a_session_opens_its_tabs_with_the_first_active() {
        let ws = two();
        assert!(ws.is_loaded());
        assert_eq!(ws.count(), 2);
        assert_eq!(ws.active().map(|t| t.title.as_str()), Some("tab a"));
    }

    #[test]
    fn a_tab_is_unsaved_when_its_text_differs_from_the_stored_text() {
        let ws = two();
        assert_eq!(
            ws.tabs().iter().map(QueryTab::dirty).collect::<Vec<_>>(),
            [false, true]
        );
    }

    #[test]
    fn editing_and_reverting_clears_the_mark() {
        let mut ws = two();
        let tab = ws.active_mut().expect("tab");
        tab.set_sql("SELECT 9".into());
        assert!(tab.dirty());
        tab.set_sql("SELECT 1".into());
        assert!(!tab.dirty());
    }

    #[test]
    fn parameters_follow_the_text_and_keep_their_values() {
        let mut ws = two();
        let tab = ws.active_mut().expect("tab");
        tab.set_sql("SELECT :a, :b".into());
        tab.params[0].value = "x".into();
        tab.set_sql("SELECT :a, :c, :b".into());
        let names: Vec<_> = tab.params.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["a", "c", "b"]);
        assert_eq!(tab.params[0].value, "x");
        assert!(tab.params[1].value.is_empty());
        tab.set_sql("SELECT 1".into());
        assert!(tab.params.is_empty());
    }

    #[test]
    fn running_needs_every_parameter_to_bind() {
        let mut ws = two();
        let tab = ws.active_mut().expect("tab");
        tab.set_sql("SELECT :n".into());
        assert!(tab.check_params().is_err(), "a new parameter has no value");
        tab.params[0].ty = ParamType::Integer;
        tab.params[0].value = "x".into();
        assert!(tab.check_params().is_err());
        tab.params[0].value = "7".into();
        assert!(tab.check_params().is_ok());
        let req = tab.request(2);
        assert_eq!((req.page, req.params.len()), (2, 1));
    }

    #[test]
    fn opening_a_saved_query_binds_a_tab_and_reopening_shows_it() {
        let mut ws = two();
        let q = saved("q1", "SELECT :cur");
        let id = ws.open_saved(&q);
        assert_eq!(ws.count(), 3);
        let tab = ws.active().expect("tab");
        assert_eq!(
            (tab.saved.as_deref(), tab.title.as_str()),
            (Some("q1"), "Spend")
        );
        assert!(!tab.dirty());
        assert_eq!(tab.params[0].value, "USD");
        ws.activate(1);
        assert_eq!(ws.open_saved(&q), id);
        assert_eq!(ws.count(), 3, "a saved query opens in one tab");
        assert_eq!(ws.active_id(), Some(id));
    }

    #[test]
    fn renaming_or_tagging_a_bound_tab_marks_it_unsaved() {
        let mut ws = two();
        ws.open_saved(&saved("q1", "SELECT :cur"));
        let tab = ws.active_mut().expect("tab");
        tab.title = "Spend by currency".into();
        assert!(tab.dirty());
        tab.title = "Spend".into();
        tab.tags.push("b".into());
        assert!(tab.dirty());
        tab.tags.pop();
        assert!(!tab.dirty());
    }

    #[test]
    fn saving_makes_the_tab_match_the_library() {
        let mut ws = two();
        let tab = ws.active_mut().expect("tab");
        tab.set_sql("SELECT 5".into());
        let mut stored = saved("q9", "SELECT 5");
        stored.params.clear();
        tab.mark_saved(&stored);
        assert!(!tab.dirty());
        assert_eq!(tab.saved.as_deref(), Some("q9"));
    }

    #[test]
    fn closing_the_active_tab_activates_its_left_neighbour() {
        let mut ws = QueryWorkspace::from_drafts(&[
            draft("a", "1", "1"),
            draft("b", "2", "2"),
            draft("c", "3", "3"),
        ]);
        ws.activate(3);
        ws.close(3);
        assert_eq!(ws.active_id(), Some(2));
        ws.close(1);
        assert_eq!(
            ws.active_id(),
            Some(2),
            "closing another tab keeps the active one"
        );
        ws.close(2);
        assert_eq!(ws.active_id(), None);
        assert_eq!(ws.count(), 0);
    }

    #[test]
    fn new_tabs_get_fresh_ids_after_closing() {
        let mut ws = two();
        ws.close(2);
        let id = ws.new_tab("analytics");
        assert_eq!(id, 3, "an id is never reused");
        assert_eq!(ws.active().map(|t| t.title.as_str()), Some("Untitled 3"));
    }

    #[test]
    fn deleting_a_saved_query_leaves_its_tab_as_an_unsaved_draft() {
        let mut ws = two();
        ws.open_saved(&saved("q1", "SELECT :cur"));
        ws.library_selected = Some("q1".into());
        ws.unbind("q1");
        let tab = ws.active().expect("tab");
        assert!(tab.saved.is_none());
        assert!(tab.dirty());
        assert!(ws.library_selected.is_none());
    }
}
