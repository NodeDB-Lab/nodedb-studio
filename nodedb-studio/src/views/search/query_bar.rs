//! The query bar: the collection, the retrieval mode, the limit, the query,
//! and the strip of fields and filters under it. Nothing runs until Search.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::search::{SearchCollection, SearchMode, SearchResult, SearchSetup};
use crate::models::search_form::{Issue, IssueField, SearchForm};
use crate::services::backend::Backend;
use crate::state::connection::ActiveConnection;
use crate::state::query::QueryWorkspace;
use crate::state::search::{ResultsUi, SearchUi};
use crate::views::search::query_strip::QueryStrip;
use crate::views::search::tasks::open_in_query;

/// Change the Search tab's state.
pub fn edit(mut r: Signal<ResultsUi>, f: impl FnOnce(&mut ResultsUi)) {
    f(&mut r.write());
}

#[component]
pub fn QueryBar(
    setup: SearchSetup,
    collection: SearchCollection,
    form: SearchForm,
    issues: Vec<Issue>,
    running: bool,
    shown: Option<SearchResult>,
    stale: bool,
    on_run: EventHandler<()>,
) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let ui = use_context::<SearchUi>();
    let r = ui.results;
    let ws = use_context::<Signal<QueryWorkspace>>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let nav = use_navigator();

    let tried = r.read().tried;
    let has = |f: IssueField| tried && issues.iter().any(|i| i.field == f);
    let names: Vec<String> = setup.collections.iter().map(|c| c.name.clone()).collect();
    let semantic = form.mode == SearchMode::Semantic;

    let open_query = shown
        .as_ref()
        .filter(|_| !stale)
        .map(|s| s.sql.executable.clone());
    let open_title = match (&shown, stale) {
        (None, _) => {
            "Run the search first. Open in Query opens the statement of the result.".to_string()
        }
        (Some(_), true) => {
            "The query changed after this ran. Search again, then open it.".to_string()
        }
        (Some(_), false) => {
            "Open the statement of this result in a new tab. It does not run.".to_string()
        }
    };
    let database = active
        .read()
        .as_ref()
        .map(|c| c.current_database.clone())
        .unwrap_or_default();
    let query_title = format!("{} search of {}", form.mode.label(), collection.name);

    rsx! {
        div { class: "toolbar nowrap srch-bar",
            label { r#for: "srch-collection", class: "toolbar-label", "Collection" }
            select {
                id: "srch-collection",
                class: "select native",
                "aria-label": "Collection",
                onchange: move |e| {
                    let name = e.value();
                    edit(r, |s| {
                        *s = ResultsUi { collection: Some(name), ..ResultsUi::default() };
                    });
                },
                for c in names.iter() {
                    option { key: "{c}", value: "{c}", selected: *c == collection.name, "{c}" }
                }
            }
            div { class: "seg accent", role: "radiogroup", "aria-label": "Retrieval mode",
                for mode in SearchMode::ALL {
                    {
                        let supported = collection.supports(mode);
                        let on = form.mode == mode;
                        rsx! {
                            button {
                                key: "{mode.label()}",
                                r#type: "button",
                                class: if on { "active" } else { "" },
                                role: "radio",
                                "aria-checked": "{on}",
                                disabled: !supported,
                                title: if supported { "" } else { "{collection.name} lacks the index {mode.label()} search needs" },
                                onclick: move |_| edit(r, |s| s.form.mode = mode),
                                "{mode.label()}"
                            }
                        }
                    }
                }
            }
            label { r#for: "srch-limit", class: "toolbar-label", "Limit" }
            input {
                id: "srch-limit",
                class: if has(IssueField::Limit) { "input num invalid srch-limit" } else { "input num srch-limit" },
                r#type: "text",
                inputmode: "numeric",
                value: "{form.limit}",
                "aria-invalid": "{has(IssueField::Limit)}",
                title: "How many ranked records come back. Candidate budgets are separate.",
                oninput: move |e| {
                    let v = e.value();
                    edit(r, |s| s.form.limit = v);
                },
            }
            div { class: "toolbar-spacer" }
            button {
                class: "btn",
                disabled: true,
                title: "Saved queries hold SQL only. A search document needs its mode, fields, filters, typed inputs, and fusion policy in the shared query library, which is not designed yet.",
                Ico { icon: Icon::FolderOpen, size: 16 }
                "Open query"
            }
            button {
                class: "btn",
                disabled: true,
                title: "A search document needs its mode, fields, filters, typed inputs, and fusion policy in the shared query library, which is not designed yet.",
                Ico { icon: Icon::Save, size: 16 }
                "Save query"
            }
            button {
                class: "btn",
                disabled: open_query.is_none(),
                title: "{open_title}",
                onclick: {
                    let backend = backend.clone();
                    move |_| {
                        let Some(sql) = open_query.clone() else { return };
                        let (backend, database, title) =
                            (backend.clone(), database.clone(), query_title.clone());
                        spawn_forever(async move {
                            open_in_query(ws, backend, nav, database, title, sql).await;
                        });
                    }
                },
                Ico { icon: Icon::ExternalLink, size: 16 }
                "Open in Query"
            }
        }
        form {
            class: "srch-query",
            onsubmit: move |e| {
                e.prevent_default();
                if !running { on_run.call(()); }
            },
            div { class: "srch-input",
                input {
                    id: "srch-query",
                    class: if has(IssueField::Query) { "input invalid" } else { "input" },
                    r#type: "text",
                    "aria-label": "Query",
                    "aria-invalid": "{has(IssueField::Query)}",
                    disabled: semantic,
                    placeholder: if semantic { "Semantic search ranks by the vector input below. Text is not embedded." } else { "Words to search for" },
                    value: "{form.query}",
                    oninput: move |e| {
                        let v = e.value();
                        edit(r, |s| s.form.query = v);
                    },
                }
                if !form.query.is_empty() && !semantic {
                    button {
                        class: "icon-btn sm srch-clear",
                        r#type: "button",
                        title: "Clear the query",
                        "aria-label": "Clear the query",
                        onclick: move |_| edit(r, |s| s.form.query.clear()),
                        Ico { icon: Icon::X, size: 14 }
                    }
                }
            }
            button {
                class: "btn primary srch-go",
                r#type: "submit",
                disabled: running,
                Ico { icon: Icon::Search, size: 16 }
                if running { "Searching…" } else { "Search" }
            }
        }
        QueryStrip {
            collection: collection.clone(),
            form: form.clone(),
            issues: issues.clone(),
            provider: setup.provider.clone(),
        }
    }
}
