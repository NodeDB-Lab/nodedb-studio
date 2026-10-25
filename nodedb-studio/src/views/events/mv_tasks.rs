//! The materialized views page's writes. Each runs under `spawn_forever` and
//! touches root state only.

use std::rc::Rc;

use dioxus::prelude::*;
use dioxus::router::Navigator;

use crate::models::matviews::MaterializedView;
use crate::routes::Route;
use crate::services::backend::Backend;
use crate::state::events::feeds::Notice;
use crate::state::events::matviews::ViewsUi;
use crate::state::query::QueryWorkspace;

/// Start a manual refresh of `view`.
pub async fn refresh(mut ui: Signal<ViewsUi>, backend: Rc<dyn Backend>, view: MaterializedView) {
    ui.write().refreshing = true;
    let result = backend.refresh_view(&view.id).await;
    let mut w = ui.write();
    w.refreshing = false;
    match result {
        Ok(run) => {
            w.notice = Some(Notice::ok(format!(
                "Refreshed {}: {}.",
                view.name,
                run.detail.to_lowercase()
            )));
            w.epoch += 1;
        }
        Err(e) => w.notice = Some(Notice::err(format!("Refreshing {} failed: {e}", view.name))),
    }
}

/// Open the view's definition in a new Query tab, where it can be read,
/// edited, and run.
pub async fn open_definition(
    mut ws: Signal<QueryWorkspace>,
    backend: Rc<dyn Backend>,
    nav: Navigator,
    view: MaterializedView,
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
        let id = w.new_tab(&view.database);
        if let Some(tab) = w.tab_mut(id) {
            let title = format!("{} definition", view.name);
            tab.title.clone_from(&title);
            tab.baseline.title = title;
            tab.set_sql(view.definition.clone());
            tab.baseline.sql.clone_from(&view.definition);
        }
    }
    nav.push(Route::Query {});
}
