//! The Index details panel. Definition is schema metadata, Coverage is what
//! the server observed, and Activity is operational history. They stay on
//! separate tabs because Ready says nothing about coverage.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::resource_state;
use crate::components::icon::{Ico, Icon};
use crate::components::tabs::TabBar;
use crate::models::format::{date_second, thousands};
use crate::models::search::{SearchMode, VectorInput};
use crate::models::search_index::{Definition, IndexKind, SearchIndex};
use crate::routes::Route;
use crate::routes::tabs::{SearchTab, VectorTab};
use crate::services::backend::Backend;
use crate::state::designer::DesignerState;
use crate::state::search::{IndexTab, SearchUi};
use crate::views::search::index_history::run_dot;
use crate::views::search::index_list::state_dot;
use crate::views::search::indexes::edit;
use crate::views::search::tasks::open_index_in_designer;

/// `13:40:06 UTC`.
fn utc_clock(at: i64) -> String {
    let full = date_second(at, 0);
    format!(
        "{} UTC",
        full.split_once(' ').map_or(full.as_str(), |(_, t)| t)
    )
}

#[component]
pub fn DetailsBody(index: Option<SearchIndex>, tab: IndexTab) -> Element {
    let ui = use_context::<SearchUi>();
    let i = ui.indexes;
    let designer = use_context::<Signal<DesignerState>>();
    let nav = use_navigator();
    let Some(ix) = index else {
        return rsx! {
            div { class: "srch-state", "Select an index to read its definition, coverage, and activity." }
        };
    };
    let active = IndexTab::ALL.iter().position(|t| *t == tab).unwrap_or(0);
    let name = ix.name.clone();
    rsx! {
        div { class: "srch-details",
            div { class: "srch-details-scroll",
                div { class: "srch-details-head",
                    h3 { class: "srch-details-id", "{ix.name}" }
                    span { class: "badge green srch-ready",
                        span { class: "{state_dot(ix.state)}" }
                        "{ix.state.label()}"
                    }
                }
                TabBar {
                    class: "sub",
                    items: IndexTab::ALL.iter().map(|t| t.label()).collect::<Vec<_>>(),
                    active,
                    on_select: move |n: usize| {
                        if let Some(t) = IndexTab::ALL.get(n) { edit(i, |s| s.tab = *t); }
                    },
                }
                match tab {
                    IndexTab::Definition => rsx! { DefinitionTab { ix: ix.clone() } },
                    IndexTab::Coverage => rsx! { CoverageTab { ix: ix.clone() } },
                    IndexTab::Activity => rsx! { ActivityTab { ix: ix.clone() } },
                }
            }
            div { class: "srch-details-actions",
                button {
                    class: "btn",
                    title: "Open {ix.name} in the Designer, where its definition changes",
                    onclick: {
                        let name = name.clone();
                        move |_| open_index_in_designer(designer, nav, &name)
                    },
                    Ico { icon: Icon::ExternalLink, size: 16 }
                    "Open in Designer"
                }
                button {
                    class: "btn",
                    title: "Review what a rebuild would do. Nothing is created until you confirm.",
                    onclick: {
                        let name = name.clone();
                        move |_| edit(i, |s| { s.plan = Some(name.clone()); s.plan_error = None; })
                    },
                    Ico { icon: Icon::Play, size: 16 }
                    "Preview rebuild"
                }
            }
            div { class: "srch-details-link",
                button {
                    class: "link-btn",
                    title: "Open Search on this collection with the fields this index covers",
                    onclick: {
                        let ix = ix.clone();
                        move |_| {
                            use_in_search(ui, &ix);
                            nav.push(Route::Search { tab: SearchTab::Results });
                        }
                    },
                    Ico { icon: Icon::Search, size: 16 }
                    "Use in Search"
                }
            }
        }
    }
}

/// Hand the index's collection and fields to Search. The mode is the one the
/// index serves, and no index hint is forced.
fn use_in_search(ui: SearchUi, ix: &SearchIndex) {
    let mode = match ix.kind {
        IndexKind::FullText => SearchMode::Keyword,
        IndexKind::Vector => SearchMode::Semantic,
    };
    ui.open_query(&ix.collection, mode, "", VectorInput::None);
    if let Some(field) = ix
        .fields
        .iter()
        .find(|f| f.as_str() == "body")
        .or(ix.fields.first())
        && ix.kind == IndexKind::FullText
    {
        let mut r = ui.results;
        r.write().form.text_field.clone_from(field);
    }
}

#[component]
fn DefinitionTab(ix: SearchIndex) -> Element {
    let nav = use_navigator();
    rsx! {
        dl { class: "srch-props roomy",
            dt { "Collection" }
            dd { "{ix.collection}" }
            dt { "Kind" }
            dd { "{ix.kind.label()}" }
            dt { "Fields" }
            dd { "{ix.fields_label()}" }
            match ix.definition.clone() {
                Definition::FullText { analyzer, binding_revision } => rsx! {
                    dt { "Analyzer" }
                    dd { "{analyzer}" }
                    dt { "Binding revision" }
                    dd { "{binding_revision}" }
                },
                Definition::Vector { dimensions, metric, model } => rsx! {
                    dt { "Dimensions" }
                    dd { "{dimensions}" }
                    dt { "Metric" }
                    dd { "{metric.label()}" }
                    dt { "Model" }
                    dd { {model.unwrap_or_else(|| "Not recorded".to_string())} }
                },
            }
        }
        h4 { class: "srch-heading", "Definition" }
        pre { class: "code-block srch-ddl", "{ix.ddl}" }
        h4 { class: "srch-heading", "Observations" }
        dl { class: "srch-props roomy",
            dt { "Indexed documents" }
            dd { {ix.indexed_documents.text(thousands)} }
            dt { "Coverage" }
            dd { "{ix.coverage_text()}" }
            dt { "Observed" }
            dd { {utc_clock(ix.observed_at)} }
        }
        if ix.kind == IndexKind::Vector {
            div { class: "srch-details-link",
                button {
                    class: "link-btn",
                    title: "Read this index's counts and builds in Vector Space",
                    onclick: move |_| { nav.push(Route::Vector { tab: VectorTab::Index }); },
                    Ico { icon: Icon::Grid3x3, size: 16 }
                    "Open in Vector Space"
                }
            }
        }
    }
}

#[component]
fn CoverageTab(ix: SearchIndex) -> Element {
    rsx! {
        div { class: "callout info srch-note", role: "note",
            Ico { icon: Icon::Info, size: 16 }
            span {
                "State and coverage are separate facts. {ix.state.label()} is the state the server reports. It does not show that every record is indexed."
            }
        }
        dl { class: "srch-props roomy",
            dt { "State" }
            dd {
                span { class: "status",
                    span { class: "{state_dot(ix.state)}" }
                    "{ix.state.label()}"
                }
            }
            dt { "Indexed documents" }
            dd { {ix.indexed_documents.text(thousands)} }
            dt { "Eligible records" }
            dd { {ix.eligible_documents.text(thousands)} }
            dt { "Coverage" }
            dd { "{ix.coverage_text()}" }
            dt { "Measured" }
            dd { {utc_clock(ix.observed_at)} }
        }
        p { class: "srch-note-text",
            "The collection's row count is not an indexed count. A value the server does not report stays Not reported."
        }
    }
}

#[component]
fn ActivityTab(ix: SearchIndex) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let ui = use_context::<SearchUi>();
    let i = ui.indexes;
    let epoch = i.read().epoch;
    let name = ix.name.clone();
    let read = use_resource(use_reactive!(|name, epoch| {
        let backend = backend.clone();
        let _conn = conn();
        let _epoch = epoch;
        async move { backend.search_build_history(&name).await }
    }));
    let state = resource_state(&read);
    let runs = state.loaded().map(|h| h.runs.clone()).unwrap_or_default();
    rsx! {
        h4 { class: "srch-heading", "Recent builds" }
        if let Some(message) = state.error_message() {
            div { class: "callout err srch-note", role: "alert",
                Ico { icon: Icon::CircleAlert, size: 16 }
                span { "{message}" }
            }
        } else if state.is_loading() {
            div { class: "srch-state", role: "status", "Reading the builds…" }
        } else if runs.is_empty() {
            div { class: "srch-state", "No build has run for this index." }
        } else {
            ul { class: "srch-activity", "aria-label": "Recent builds",
                for r in runs.iter().take(8) {
                    {
                        let id = r.id.clone();
                        rsx! {
                            li { key: "{r.id}",
                                span { class: "status",
                                    span { class: "{run_dot(&r.state)}" }
                                    "{r.state.text()}"
                                }
                                button {
                                    class: "link-btn mono",
                                    onclick: move |_| edit(i, |s| s.run = Some(id.clone())),
                                    "{r.id}"
                                }
                                span { class: "srch-when", {date_second(r.requested_at, 0)} }
                            }
                        }
                    }
                }
            }
        }
    }
}
