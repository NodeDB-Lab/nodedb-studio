//! The Search indexes panel: the collection, a filter, and the indexes with
//! the state each reports. State is not coverage.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::search_index::{IndexState, SearchIndex};
use crate::services::backend::Backend;
use crate::state::designer::DesignerState;
use crate::state::search::SearchUi;
use crate::state::workspace::Workspace;
use crate::views::search::indexes::edit;
use crate::views::search::tasks::open_new_index;

pub fn state_dot(s: IndexState) -> &'static str {
    match s {
        IndexState::Ready => "status-dot ok",
        IndexState::Building => "status-dot info",
        IndexState::Error => "status-dot err",
    }
}

/// Whether an index shows under the filter text.
fn matches(i: &SearchIndex, needle: &str) -> bool {
    let n = needle.trim().to_lowercase();
    n.is_empty()
        || i.name.to_lowercase().contains(&n)
        || i.kind.label().to_lowercase().contains(&n)
        || i.fields_label().to_lowercase().contains(&n)
}

#[component]
pub fn IndexListBody(
    collections: Vec<String>,
    collection: String,
    filter: String,
    indexes: Vec<SearchIndex>,
    selected: Option<String>,
) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let ui = use_context::<SearchUi>();
    let i = ui.indexes;
    let designer = use_context::<Signal<DesignerState>>();
    let layout = use_context::<Signal<Workspace>>();
    let nav = use_navigator();
    let shown: Vec<SearchIndex> = indexes
        .iter()
        .filter(|x| matches(x, &filter))
        .cloned()
        .collect();
    rsx! {
        div { class: "toolbar nowrap srch-ix-bar",
            label { r#for: "srch-ix-collection", class: "toolbar-label", "Collection" }
            select {
                id: "srch-ix-collection",
                class: "select native",
                onchange: move |e| {
                    let name = e.value();
                    edit(i, |s| {
                        s.collection = Some(name);
                        s.selected = None;
                        s.history_index = None;
                        s.all_runs = false;
                    });
                },
                for c in collections.iter() {
                    option { key: "{c}", value: "{c}", selected: *c == collection, "{c}" }
                }
            }
            div { class: "search-field grow",
                Ico { icon: Icon::Search, size: 16 }
                input {
                    class: "input",
                    "aria-label": "Filter indexes",
                    placeholder: "Filter indexes…",
                    value: "{filter}",
                    oninput: move |e| {
                        let v = e.value();
                        edit(i, |s| s.filter = v);
                    },
                }
            }
            button {
                class: "btn primary",
                title: "Open the Designer's New index form on {collection}",
                disabled: collection.is_empty(),
                onclick: {
                    let (backend, collection) = (backend.clone(), collection.clone());
                    move |_| {
                        let (backend, collection) = (backend.clone(), collection.clone());
                        spawn_forever(open_new_index(designer, layout, backend, nav, collection));
                    }
                },
                Ico { icon: Icon::Plus, size: 16 }
                "Index"
            }
        }
        if !shown.is_empty() {
            div { class: "dt-wrap srch-ix-table",
                table { class: "dt", "aria-label": "Search indexes",
                    thead {
                        tr { th { "Index" } th { "Kind" } th { "Fields" } th { "State" } }
                    }
                    tbody {
                        for x in shown.iter() {
                            {
                                let on = selected.as_deref() == Some(x.name.as_str());
                                let (name, name_key) = (x.name.clone(), x.name.clone());
                                rsx! {
                                    tr {
                                        key: "{x.name}",
                                        class: if on { "clickable selected" } else { "clickable" },
                                        tabindex: "0",
                                        "aria-selected": "{on}",
                                        onclick: move |_| edit(i, |s| s.selected = Some(name.clone())),
                                        onkeydown: move |e| {
                                            if e.key() == Key::Enter || e.key() == Key::Character(" ".into()) {
                                                e.prevent_default();
                                                edit(i, |s| s.selected = Some(name_key.clone()));
                                            }
                                        },
                                        td { class: "mono", "{x.name}" }
                                        td { "{x.kind.label()}" }
                                        td { "{x.fields_label()}" }
                                        td {
                                            span { class: "status",
                                                span { class: "{state_dot(x.state)}" }
                                                "{x.state.label()}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else if !indexes.is_empty() {
            div { class: "srch-state", "No index matches the filter." }
        }
    }
}
