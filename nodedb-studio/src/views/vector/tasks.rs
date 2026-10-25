//! Vector Space writes that outlive a click. Each runs under `spawn_forever`
//! and touches root state only.

use std::rc::Rc;

use dioxus::prelude::*;
use dioxus::router::Navigator;

use crate::routes::Route;
use crate::services::backend::Backend;
use crate::state::query::QueryWorkspace;

/// Open `sql` in a new Query tab, where it can be read, edited, and run.
/// Studio never runs it.
pub async fn open_in_query(
    mut ws: Signal<QueryWorkspace>,
    backend: Rc<dyn Backend>,
    nav: Navigator,
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
    nav.push(Route::Query {});
}
