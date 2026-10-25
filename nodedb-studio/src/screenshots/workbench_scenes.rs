//! The Query and Console states the design shows, built from the same
//! fixtures the mock backend serves.

use crate::data::mock::{self, CUSTOMER_SPEND_ID, ORDERS_SQL, WorkbenchStore};
use crate::models::console::Protocol;
use crate::models::query_plan::ExplainMode;
use crate::models::saved_query::SavedQuery;
use crate::models::workbench::QueryDraft;
use crate::state::console::ConsoleState;
use crate::state::query::{PlanState, QueryWorkspace, ResultsView, RunState, SidePane};

/// Run the active tab's statement on a fresh mock store and keep the rows.
fn with_run(ws: &mut QueryWorkspace) {
    let Some(tab) = ws.active_mut() else { return };
    if let Ok(run) = WorkbenchStore::seeded().run(&tab.request(0)) {
        tab.run = RunState::Done(Box::new(run));
    }
}

fn spend() -> Option<SavedQuery> {
    mock::query_library()
        .queries
        .into_iter()
        .find(|q| q.id == CUSTOMER_SPEND_ID)
}

/// Three tabs, the first run: events by type.
pub fn query() -> QueryWorkspace {
    let mut ws = QueryWorkspace::from_drafts(&mock::query_session());
    with_run(&mut ws);
    ws
}

/// The statement of the third tab, planned by Explain.
pub fn query_plan() -> QueryWorkspace {
    let mut ws = QueryWorkspace::from_drafts(&mock::query_session());
    ws.activate(3);
    if let Some(tab) = ws.active_mut() {
        debug_assert_eq!(tab.sql, ORDERS_SQL);
        tab.view = ResultsView::Plan;
        tab.mode = ExplainMode::Explain;
        tab.plan = PlanState::Ready(Box::new(mock::query_plan(&tab.sql, ExplainMode::Explain)));
        tab.selected_op = Some(1);
    }
    ws
}

/// Customer spend open in the first tab with a renamed draft, selected in
/// the library, five rows to a page.
pub fn query_library() -> QueryWorkspace {
    let mut drafts = mock::query_session();
    if let (Some(q), Some(first)) = (spend(), drafts.first_mut()) {
        *first = QueryDraft {
            id: "tab-1".into(),
            title: q.name.clone(),
            sql: q.sql.clone(),
            saved_sql: q.sql.clone(),
            saved_query: Some(q.id.clone()),
            database: q.database.clone(),
        };
    }
    let mut ws = QueryWorkspace::from_drafts(&drafts);
    if let (Some(q), Some(tab)) = (spend(), ws.active_mut()) {
        tab.params = q.params.clone();
        tab.tags = q.tags.clone();
        tab.baseline.params = q.params.clone();
        tab.baseline.tags = q.tags.clone();
        // The draft was renamed after it loaded, so the tab reads unsaved.
        tab.baseline.title = "Customer spend report".into();
        tab.page_size = 5;
    }
    with_run(&mut ws);
    ws.library_selected = Some(CUSTOMER_SPEND_ID.to_string());
    ws.side = SidePane::Details;
    ws
}

/// Two commands run, and a third waiting in the prompt.
pub fn console() -> ConsoleState {
    let mut store = WorkbenchStore::seeded();
    let mut state = ConsoleState::default();
    for text in ["SELECT current_database();", "SELECT count(*) FROM users;"] {
        let outcome = store
            .console(Protocol::Sql, mock::WORKBENCH_DATABASE, text)
            .map_err(|e| e.to_string());
        state.push(Protocol::Sql, text, outcome);
    }
    state.edit_prompt("SELECT count(*) FROM users;".into());
    state.selected = Some("h-014".into());
    state
}
