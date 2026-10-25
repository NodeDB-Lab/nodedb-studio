//! The page's writes. Each runs under `spawn_forever` and touches root
//! state only, so it finishes when the user leaves the page mid-run.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::models::query_plan::ExplainMode;
use crate::models::saved_query::SaveQuery;
use crate::models::workbench::{QueryRequest, TxAccess};
use crate::services::backend::Backend;
use crate::state::query::{PlanState, QueryTab, QueryWorkspace, ResultsView, RunState, TxEpoch};

/// The root signals every task writes.
#[derive(Clone, Copy)]
pub struct Env {
    pub ws: Signal<QueryWorkspace>,
    pub tx: TxEpoch,
}

impl Env {
    pub fn new(ws: Signal<QueryWorkspace>, tx: TxEpoch) -> Self {
        Self { ws, tx }
    }

    fn bump_tx(self) {
        let mut epoch = self.tx.0;
        let next = *epoch.peek() + 1;
        epoch.set(next);
    }
}

/// Open `sql` in a new Query tab, where it can be read, edited, and run, and
/// show the Query page. Studio never runs it.
pub async fn open_in_query(
    mut ws: Signal<QueryWorkspace>,
    backend: Rc<dyn Backend>,
    nav: dioxus::router::Navigator,
    database: String,
    title: String,
    sql: String,
) {
    // The Query page restores the session's tabs once. Do that first, or the
    // restore would replace the tab opened here.
    if !ws.peek().is_loaded()
        && let Ok(drafts) = backend.query_session().await
    {
        ws.set(QueryWorkspace::from_drafts(&drafts));
    }
    {
        let mut w = ws.write();
        let id = w.new_tab(&database);
        if let Some(tab) = w.tab_mut(id) {
            tab.title.clone_from(&title);
            tab.baseline.title = title;
            tab.set_sql(sql.clone());
            tab.baseline.sql = sql;
        }
    }
    nav.push(crate::routes::Route::Query {});
}

/// Run `request` in tab `id` and keep its result there.
pub async fn run(env: Env, backend: Rc<dyn Backend>, id: u32, request: QueryRequest) {
    let mut ws = env.ws;
    if let Some(tab) = ws.write().tab_mut(id) {
        tab.run = RunState::Running;
        tab.view = ResultsView::Results;
        tab.page = request.page;
    }
    let out = backend.run_query(&request).await;
    if let Some(tab) = ws.write().tab_mut(id) {
        tab.run = match out {
            Ok(run) => RunState::Done(Box::new(run)),
            Err(e) => RunState::Failed(e.to_string()),
        };
    }
    env.bump_tx();
}

/// Plan `request` in tab `id`. Analyze runs the statement, so the
/// transaction may change.
pub async fn explain(
    env: Env,
    backend: Rc<dyn Backend>,
    id: u32,
    request: QueryRequest,
    mode: ExplainMode,
) {
    let mut ws = env.ws;
    if let Some(tab) = ws.write().tab_mut(id) {
        tab.plan = PlanState::Running;
        tab.view = ResultsView::Plan;
        tab.selected_op = None;
    }
    let out = backend.explain(&request, mode).await;
    if let Some(tab) = ws.write().tab_mut(id) {
        tab.plan = match out {
            Ok(plan) => PlanState::Ready(Box::new(plan)),
            Err(e) => PlanState::Failed(e.to_string()),
        };
    }
    env.bump_tx();
}

/// What to do with the session's transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxOp {
    Begin(TxAccess),
    Commit,
    Rollback,
}

pub async fn transaction(env: Env, backend: Rc<dyn Backend>, op: TxOp) {
    let mut ws = env.ws;
    let out = match op {
        TxOp::Begin(access) => backend.begin_transaction(access).await,
        TxOp::Commit => backend.commit_transaction().await,
        TxOp::Rollback => backend.rollback_transaction().await,
    };
    ws.write().notice = out.err().map(|e| e.to_string());
    env.bump_tx();
}

/// Store tab `id` as a saved query. `save.id` names the query to version,
/// or is `None` for a new one.
pub async fn save(env: Env, backend: Rc<dyn Backend>, id: u32, save: SaveQuery) {
    let mut ws = env.ws;
    match backend.save_query(&save).await {
        Ok(saved) => {
            let mut w = ws.write();
            if let Some(tab) = w.tab_mut(id) {
                tab.mark_saved(&saved);
            }
            w.library_selected = Some(saved.id);
            w.library_epoch += 1;
            w.notice = None;
        }
        Err(e) => ws.write().notice = Some(e.to_string()),
    }
}

pub async fn delete(env: Env, backend: Rc<dyn Backend>, id: String) {
    let mut ws = env.ws;
    match backend.delete_saved_query(&id).await {
        Ok(()) => {
            let mut w = ws.write();
            w.unbind(&id);
            w.library_epoch += 1;
            w.notice = None;
        }
        Err(e) => ws.write().notice = Some(e.to_string()),
    }
}

pub async fn create_folder(env: Env, backend: Rc<dyn Backend>, name: String) {
    let mut ws = env.ws;
    match backend.create_query_folder(&name).await {
        Ok(()) => {
            let mut w = ws.write();
            w.library_epoch += 1;
            w.notice = None;
        }
        Err(e) => ws.write().notice = Some(e.to_string()),
    }
}

/// Why the tab cannot run or explain now, if it cannot.
pub fn blocked(tab: &QueryTab, active_db: &str) -> Option<String> {
    if tab.sql.trim().is_empty() {
        return Some("Type a statement first".to_string());
    }
    if tab.database != active_db {
        return Some(format!(
            "This query is bound to `{}` but the session uses `{active_db}`. \
             Switch the active database or bind the query to `{active_db}`.",
            tab.database
        ));
    }
    None
}

/// What starting a statement needs from the active tab: its id and request,
/// or the refusal to show.
fn prepare(
    ws: &mut Signal<QueryWorkspace>,
    active_db: &str,
    page: usize,
) -> Option<(u32, QueryRequest)> {
    let (id, request, refusal) = {
        let w = ws.peek();
        let tab = w.active()?;
        let refusal =
            blocked(tab, active_db).or_else(|| tab.check_params().err().map(|e| e.to_string()));
        (tab.id, tab.request(page), refusal)
    };
    ws.write().notice = refusal.clone();
    refusal.is_none().then_some((id, request))
}

/// Run the active tab's statement, page `page` of its rows.
pub fn start_run(env: Env, backend: Rc<dyn Backend>, active_db: &str, page: usize) {
    let mut ws = env.ws;
    if let Some((id, request)) = prepare(&mut ws, active_db, page) {
        spawn_forever(async move { run(env, backend, id, request).await });
    }
}

/// Plan the active tab's statement in its chosen mode.
pub fn start_explain(env: Env, backend: Rc<dyn Backend>, active_db: &str) {
    let mut ws = env.ws;
    let mode = ws.peek().active().map(|t| t.mode).unwrap_or_default();
    if let Some((id, request)) = prepare(&mut ws, active_db, 0) {
        spawn_forever(async move { explain(env, backend, id, request, mode).await });
    }
}

/// Begin, commit, or roll back the session's transaction.
pub fn start_transaction(env: Env, backend: Rc<dyn Backend>, op: TxOp) {
    spawn_forever(async move { transaction(env, backend, op).await });
}
