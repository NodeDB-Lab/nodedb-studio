//! Vector Space: the target row and the active tab.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::tabs::{RouteTabs, tab_items};
use crate::models::vector::{QuerySource, VectorSetup, VectorTarget};
use crate::models::vector_sql::{index_status_sql, neighbors_sql};
use crate::routes::Route;
use crate::routes::tabs::VectorTab;
use crate::services::backend::Backend;
use crate::state::connection::ActiveConnection;
use crate::state::query::QueryWorkspace;
use crate::state::vector::{TargetChoice, VectorUi, target_key};
use crate::views::vector::index::IndexTab;
use crate::views::vector::projection::ProjectionTab;
use crate::views::vector::search::SearchTab;
use crate::views::vector::target_bar::TargetBar;
use crate::views::vector::tasks::open_in_query;

/// The target the user chose, or the first the connection offers.
fn resolve(setup: &VectorSetup, choice: &TargetChoice) -> Option<VectorTarget> {
    let collection = choice
        .collection
        .as_deref()
        .filter(|c| setup.targets.iter().any(|t| t.collection == *c))
        .or_else(|| setup.targets.first().map(|t| t.collection.as_str()))?;
    let field = choice
        .field
        .as_deref()
        .filter(|f| setup.target(collection, f).is_some())
        .or_else(|| setup.fields(collection).first().copied())?;
    setup.target(collection, field).cloned()
}

#[component]
pub fn VectorPage(tab: VectorTab) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let ui = use_context::<VectorUi>();
    let nav = use_navigator();

    let mut setup_read = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let _conn = conn();
            async move { backend.vector_setup().await }
        })
    };
    let state = resource_state(&setup_read);
    let setup: Option<VectorSetup> = state.loaded().cloned();
    let choice = ui.choice.read().clone();
    let target = setup.as_ref().and_then(|s| resolve(s, &choice));

    rsx! {
        RouteTabs {
            items: tab_items(VectorTab::ALL, |t| t.label(), |tab| Route::Vector { tab }),
            current: Route::Vector { tab },
            actions: rsx! { OpenInQuery { tab, target: target.clone() } },
        }
        div { class: "page-body vec-page",
            AsyncView {
                loading: state.is_loading(),
                empty: false,
                error: state.error_message(),
                retriable: state.is_retriable(),
                on_retry: move |_| setup_read.restart(),
            }
            if state.is_empty() {
                div { class: "empty-state",
                    Ico { icon: Icon::Grid3x3, size: 24 }
                    h2 { "No vector index" }
                    p { "No collection has a field with a vector index. Add one in the Designer, and it appears here." }
                    div { class: "actions",
                        button {
                            class: "btn",
                            onclick: move |_| { nav.push(Route::Designer {}); },
                            Ico { icon: Icon::Pencil, size: 16 }
                            "Open Designer"
                        }
                    }
                }
            }
            if let (Some(setup), Some(target)) = (setup.clone(), target) {
                TargetBar { setup: setup.clone(), target: target.clone() }
                match tab {
                    VectorTab::Search => rsx! { SearchTab { target, provider: setup.provider.clone() } },
                    VectorTab::Projection => rsx! { ProjectionTab { target } },
                    VectorTab::Index => rsx! { IndexTab { target } },
                }
            }
        }
    }
}

/// Open the query that equals this tab in a new Query tab, without running
/// it. A Projection has no equivalent SQL: its seeded sample is Studio's.
#[component]
fn OpenInQuery(tab: VectorTab, target: Option<VectorTarget>) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let ui = use_context::<VectorUi>();
    let ws = use_context::<Signal<QueryWorkspace>>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let nav = use_navigator();

    let database = active
        .read()
        .as_ref()
        .map(|c| c.current_database.clone())
        .unwrap_or_default();
    let (title_text, why_not, run) = match (&target, tab) {
        (None, _) => (
            String::new(),
            Some("No vector index is selected.".to_string()),
            None,
        ),
        (Some(_), VectorTab::Projection) => (
            String::new(),
            Some("A projection's seeded sample has no equivalent SQL.".to_string()),
            None,
        ),
        (Some(t), VectorTab::Index) => (
            format!("{} index status", t.label()),
            None,
            Some(index_status_sql(&t.collection, &t.field)),
        ),
        (Some(t), VectorTab::Search) => {
            let form = ui.drafts.read().search(&t.label()).form;
            match form.check(t) {
                Ok(_) => (format!("{} neighbors", t.label()), None, None),
                Err(issues) => (
                    String::new(),
                    Some(
                        issues
                            .first()
                            .map_or("The search is not ready.".to_string(), |i| {
                                i.message.clone()
                            }),
                    ),
                    None,
                ),
            }
        }
    };
    let label = "Open in Query";
    let onclick = {
        let target = target.clone();
        move |_| {
            let Some(t) = target.clone() else { return };
            let key = target_key(&t.collection, &t.field);
            let (backend, database, title) =
                (backend.clone(), database.clone(), title_text.clone());
            let mut drafts = ui.drafts;
            match tab {
                VectorTab::Projection => {}
                VectorTab::Index => {
                    let sql = run.clone().unwrap_or_default();
                    spawn_forever(async move {
                        open_in_query(ws, backend, nav, database, title, sql).await;
                    });
                }
                VectorTab::Search => {
                    let Ok(req) = drafts.peek().search(&key).form.check(&t) else {
                        return;
                    };
                    spawn_forever(async move {
                        let vector = match &req.source {
                            QuerySource::Vector(v) => Ok(v.clone()),
                            QuerySource::Record(id) => backend
                                .vector_record(&req.collection, &req.field, id)
                                .await
                                .map(|r| r.vector),
                        };
                        match vector {
                            Ok(v) => {
                                let sql = neighbors_sql(&req, &t.id_field, &v);
                                open_in_query(ws, backend, nav, database, title, sql).await;
                            }
                            Err(e) => {
                                drafts.write().search_mut(&key).error = Some(e.to_string());
                            }
                        }
                    });
                }
            }
        }
    };
    rsx! {
        button {
            class: "btn",
            disabled: why_not.is_some(),
            title: why_not.clone().unwrap_or_else(|| "Open the equivalent query in a new tab. It does not run.".to_string()),
            onclick,
            Ico { icon: Icon::ExternalLink, size: 16 }
            "{label}"
        }
    }
}
