//! The Explore strip actions: open a saved query, save this one, and add it
//! to a dashboard. Each opens a small popover under its button.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::models::format::date_minute;
use crate::models::saved_query::{QueryLibrary, SavedQuery};
use crate::models::ts_dashboard::DashboardSummary;
use crate::services::backend::Backend;
use crate::state::connection::ActiveConnection;
use crate::state::query::QueryWorkspace;
use crate::state::timeseries::TimeseriesUi;
use crate::views::query::tasks::open_in_query;
use crate::views::timeseries::explore_tasks::{add_to_dashboard, save_query};

/// Which popover is open. One at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Which {
    Open,
    Save,
    Add,
}

/// Saved queries bound to `database` whose name or tags match `needle`.
pub fn compatible<'a>(lib: &'a QueryLibrary, database: &str, needle: &str) -> Vec<&'a SavedQuery> {
    lib.queries
        .iter()
        .filter(|q| q.database == database && QueryLibrary::matches(q, needle))
        .collect()
}

#[component]
pub fn ExploreActions() -> Element {
    let ui = use_context::<TimeseriesUi>();
    let mut open = use_signal(|| None::<Which>);
    let draft = ui.explore.read().draft.clone();
    let mut toggle = move |w: Which| open.set(if open() == Some(w) { None } else { Some(w) });

    rsx! {
        div { class: "anchor",
            button {
                class: "btn",
                "aria-haspopup": "dialog",
                title: "Open a saved query in the Query page",
                onclick: move |_| toggle(Which::Open),
                Ico { icon: Icon::FolderOpen, size: 16 }
                "Open query"
            }
            if open() == Some(Which::Open) {
                div { class: "ts-scrim", onclick: move |_| open.set(None) }
                OpenQuery { on_done: move |_| open.set(None) }
            }
        }
        div { class: "anchor",
            button {
                class: "btn",
                disabled: draft.is_none(),
                "aria-haspopup": "dialog",
                title: "Save this query to the shared query library",
                onclick: move |_| toggle(Which::Save),
                Ico { icon: Icon::Save, size: 16 }
                "Save query"
            }
            if open() == Some(Which::Save) {
                div { class: "ts-scrim", onclick: move |_| open.set(None) }
                SaveQuery { on_done: move |_| open.set(None) }
            }
        }
        div { class: "anchor",
            button {
                class: "btn",
                disabled: draft.is_none(),
                "aria-haspopup": "dialog",
                title: "Add this query to a dashboard as a panel",
                onclick: move |_| toggle(Which::Add),
                Ico { icon: Icon::Plus, size: 16 }
                "Add to dashboard"
            }
            if open() == Some(Which::Add) {
                div { class: "ts-scrim", onclick: move |_| open.set(None) }
                AddToDashboard { on_done: move |_| open.set(None) }
            }
        }
    }
}

#[component]
fn OpenQuery(on_done: EventHandler<()>) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let ws = use_context::<Signal<QueryWorkspace>>();
    let nav = use_navigator();
    let mut filter = use_signal(String::new);
    let mut read = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            async move { backend.query_library().await }
        })
    };
    let state = resource_state(&read);
    let database = active
        .read()
        .as_ref()
        .map(|c| c.current_database.clone())
        .unwrap_or_default();
    let lib = state.loaded().cloned();

    rsx! {
        div {
            class: "popover align-right ts-action-pop",
            role: "dialog",
            "aria-label": "Open query",
            onclick: move |e| e.stop_propagation(),
            onkeydown: move |e| {
                if e.key() == Key::Escape {
                    e.stop_propagation();
                    on_done.call(());
                }
            },
            div { class: "menu-title", "Saved queries in {database}" }
            div { class: "search-field",
                Ico { icon: Icon::Search, size: 16 }
                input {
                    class: "input",
                    r#type: "search",
                    placeholder: "Search saved queries…",
                    "aria-label": "Search saved queries",
                    value: "{filter}",
                    oninput: move |e| filter.set(e.value()),
                }
            }
            AsyncView {
                loading: state.is_loading(),
                empty: false,
                error: state.error_message(),
                retriable: state.is_retriable(),
                on_retry: move |_| read.restart(),
            }
            if let Some(lib) = lib {
                {
                    let found: Vec<SavedQuery> = compatible(&lib, &database, &filter.read()).into_iter().cloned().collect();
                    rsx! {
                        div { class: "list ts-action-list",
                            for q in found.iter() {
                                {
                                    let q = q.clone();
                                    let backend = backend.clone();
                                    rsx! {
                                        button {
                                            key: "{q.id}",
                                            class: "list-item",
                                            title: "Open {q.name} in a new Query tab. It does not run.",
                                            onclick: {
                                                let q = q.clone();
                                                move |_| {
                                                    let (backend, q) = (backend.clone(), q.clone());
                                                    on_done.call(());
                                                    spawn_forever(async move {
                                                        open_in_query(ws, backend, nav, q.database.clone(), q.name.clone(), q.sql.clone()).await;
                                                    });
                                                }
                                            },
                                            Ico { icon: Icon::FileCode, size: 16 }
                                            span { class: "ts-pick-text",
                                                span { class: "ts-pick-name", "{q.name}" }
                                                span { class: "ts-pick-sub", "{q.scope.label()} · saved {date_minute(q.updated_at, 0)} UTC" }
                                            }
                                        }
                                    }
                                }
                            }
                            if found.is_empty() {
                                div { class: "ts-pick-none", "No saved query is bound to {database}." }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn SaveQuery(on_done: EventHandler<()>) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let ui = use_context::<TimeseriesUi>();
    let q = ui.explore.read().draft.clone();
    let mut name = use_signal(|| q.as_ref().map(|q| q.title()).unwrap_or_default());
    let database = active
        .read()
        .as_ref()
        .map(|c| c.current_database.clone())
        .unwrap_or_default();
    let blank = name.read().trim().is_empty();

    rsx! {
        div {
            class: "popover align-right ts-action-pop",
            role: "dialog",
            "aria-label": "Save query",
            onclick: move |e| e.stop_propagation(),
            onkeydown: move |e| {
                if e.key() == Key::Escape {
                    e.stop_propagation();
                    on_done.call(());
                }
            },
            div { class: "menu-title", "Save query" }
            div { class: "field",
                label { r#for: "ts-save-name", "Name" }
                input {
                    id: "ts-save-name",
                    class: "input",
                    value: "{name}",
                    autofocus: true,
                    oninput: move |e| name.set(e.value()),
                }
                div { class: "hint", "Saved to the shared query library for {database}, as My workspace. Opening it later shows the SQL in the Query page." }
            }
            div { class: "ts-pop-actions",
                button { class: "btn sm", onclick: move |_| on_done.call(()), "Cancel" }
                button {
                    class: "btn sm primary",
                    disabled: blank || q.is_none(),
                    onclick: move |_| {
                        let Some(q) = q.clone() else { return };
                        let (backend, database, label) = (backend.clone(), database.clone(), name.read().trim().to_string());
                        on_done.call(());
                        spawn_forever(save_query(ui.explore, backend, database, label, q));
                    },
                    "Save"
                }
            }
        }
    }
}

#[component]
fn AddToDashboard(on_done: EventHandler<()>) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let ui = use_context::<TimeseriesUi>();
    let nav = use_navigator();
    let mut read = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            async move { backend.dashboards().await }
        })
    };
    let state = resource_state(&read);
    let rows: Vec<DashboardSummary> = state.loaded().cloned().unwrap_or_default();
    let q = ui.explore.read().draft.clone();

    rsx! {
        div {
            class: "popover align-right ts-action-pop",
            role: "dialog",
            "aria-label": "Add to dashboard",
            onclick: move |e| e.stop_propagation(),
            onkeydown: move |e| {
                if e.key() == Key::Escape {
                    e.stop_propagation();
                    on_done.call(());
                }
            },
            div { class: "menu-title", "Add to dashboard" }
            AsyncView {
                loading: state.is_loading(),
                empty: false,
                error: state.error_message(),
                retriable: state.is_retriable(),
                on_retry: move |_| read.restart(),
            }
            div { class: "list ts-action-list",
                for d in rows.iter() {
                    {
                        let id = d.id.clone();
                        let q = q.clone();
                        let meta = format!("{} panels · {}", d.panels, d.collections.join(", "));
                        rsx! {
                            button {
                                key: "{id}",
                                class: "list-item",
                                title: "Embed this query as a panel. The dashboard keeps its own copy.",
                                onclick: move |_| {
                                    if let Some(q) = &q {
                                        add_to_dashboard(ui.dash, nav, Some(id.clone()), q);
                                    }
                                    on_done.call(());
                                },
                                Ico { icon: Icon::ChartColumn, size: 16 }
                                span { class: "ts-pick-text",
                                    span { class: "ts-pick-name", "{d.name}" }
                                    span { class: "ts-pick-sub", "{meta}" }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "menu-sep" }
            button {
                class: "menu-item",
                title: "Start an unsaved dashboard with this panel. Nothing is created until you save it.",
                onclick: move |_| {
                    if let Some(q) = &q {
                        add_to_dashboard(ui.dash, nav, None, q);
                    }
                    on_done.call(());
                },
                Ico { icon: Icon::FilePlus, size: 16 }
                span { "New dashboard" }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockConnectionService;
    use crate::services::workbench_data::WorkbenchData;

    #[tokio::test]
    async fn only_queries_bound_to_the_database_are_offered() {
        let lib = MockConnectionService::ready()
            .query_library()
            .await
            .expect("library");
        let all = compatible(&lib, "analytics", "");
        assert!(all.iter().all(|q| q.database == "analytics"));
        assert!(compatible(&lib, "no-such-database", "").is_empty());
        let narrowed = compatible(&lib, "analytics", "zzzz-no-match");
        assert!(narrowed.is_empty());
    }
}
