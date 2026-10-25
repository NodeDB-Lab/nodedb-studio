//! A materialized view's definition and refresh policy, and its refresh
//! history.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::models::matviews::{MaterializedView, RefreshStatus};
use crate::routes::Route;
use crate::services::backend::Backend;
use crate::state::events::EventsUi;
use crate::state::explorer::{RecordView, Selected, select_collection};
use crate::views::events::common::{SqlBlock, StatusText};

#[component]
pub fn DefinitionPanel(view: MaterializedView) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let nav = use_navigator();
    let ui = use_context::<EventsUi>().views;
    let mut selected = use_context::<Signal<Option<Selected>>>();
    let mut explorer_view = use_context::<Signal<RecordView>>();
    let epoch = use_memo(move || ui.read().epoch);
    let id = view.id.clone();
    let history = {
        let backend = backend.clone();
        use_resource(use_reactive!(|id| {
            let backend = backend.clone();
            let (_conn, _epoch) = (conn(), epoch());
            async move { backend.refresh_history(&id).await }
        }))
    };
    let catalog = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let _conn = conn();
            async move { backend.catalog().await }
        })
    };
    let state = resource_state(&history);
    let latest = state.loaded().and_then(|h| h.first().cloned());
    let in_catalog = |dep: &str| {
        let name = dep.rsplit('.').next().unwrap_or(dep);
        catalog
            .read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .is_some_and(|c| c.iter().any(|x| x.name == name))
    };
    rsx! {
        div { class: "inspector mv-definition",
            SqlBlock { sql: view.definition.clone(), class: "definition-sql" }
            h4 { class: "inspector-section sub", "Refresh policy" }
            div { class: "kv",
                div { class: "kv-row", span { "Policy" } span { "{view.policy.label()}" } }
                div { class: "kv-row",
                    span { "Last applied watermark" }
                    span { class: "mono",
                        if let Some(w) = view.watermark.clone() { "{w}" } else { "Not reported" }
                    }
                }
                div { class: "kv-row",
                    span { "Observed lag" }
                    span { class: if view.lag_secs.is_none() { "muted" } else { "" }, "{view.lag_label()}" }
                }
                div { class: "kv-row",
                    span { "Dependency" }
                    span { class: "dep-list",
                        for dep in view.dependencies.iter() {
                            {
                                let name = dep.rsplit('.').next().unwrap_or(dep).to_string();
                                let linked = in_catalog(dep);
                                rsx! {
                                    if linked {
                                        button {
                                            key: "{dep}",
                                            class: "link-btn",
                                            title: "Open {dep} in the Explorer",
                                            onclick: move |_| {
                                                select_collection(&mut selected, &mut explorer_view, &name);
                                                nav.push(Route::Explorer {});
                                            },
                                            "{dep}"
                                        }
                                    } else {
                                        span { key: "{dep}", title: "{dep} is not in this database's catalog", "{dep}" }
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "kv-row stacked-value",
                    span { "Latest refresh" }
                    span {
                        if let Some(r) = latest {
                            StatusText { tone: r.status.tone(), text: r.status.label().to_string() }
                            span { class: "mono muted block", "{r.at} UTC" }
                        } else {
                            "No refresh recorded"
                        }
                    }
                }
            }
            div { class: "insp-note",
                Ico { icon: Icon::Info, size: 16 }
                span { "The server reports a watermark and a lag only when it measures them." }
            }
        }
    }
}

#[component]
pub fn HistoryPanel(view: MaterializedView) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut ui = use_context::<EventsUi>().views;
    let epoch = use_memo(move || ui.read().epoch);
    let id = view.id.clone();
    let mut history = {
        let backend = backend.clone();
        use_resource(use_reactive!(|id| {
            let backend = backend.clone();
            let (_conn, _epoch) = (conn(), epoch());
            async move { backend.refresh_history(&id).await }
        }))
    };
    let state = resource_state(&history);
    let open = ui.read().open_run.clone();
    rsx! {
        AsyncView {
            loading: state.is_loading(),
            empty: state.is_empty(),
            error: state.error_message(),
            retriable: state.is_retriable(),
            on_retry: move |_| history.restart(),
            empty_message: "This view has not refreshed yet.",
        }
        if let Some(runs) = state.loaded().cloned() {
            div { class: "dt-wrap",
                table { class: "dt",
                    thead {
                        tr {
                            th { "Run time (UTC)" }
                            th { "Status" }
                            th { "Details" }
                            th { "Started by" }
                        }
                    }
                    tbody {
                        for r in runs.iter() {
                            {
                                let rid = r.id.clone();
                                let is_open = open.as_deref() == Some(r.id.as_str());
                                let failed = r.status == RefreshStatus::Error;
                                rsx! {
                                    tr { key: "{r.id}", class: if is_open { "selected" } else { "" },
                                        td { class: "mono", "{r.at}" }
                                        td { StatusText { tone: r.status.tone(), text: r.status.label().to_string() } }
                                        td {
                                            if failed {
                                                button {
                                                    class: "link-btn",
                                                    "aria-expanded": "{is_open}",
                                                    title: "Show the error of this refresh",
                                                    onclick: move |_| {
                                                        let mut w = ui.write();
                                                        w.open_run = if is_open { None } else { Some(rid.clone()) };
                                                    },
                                                    "{r.detail}"
                                                }
                                            } else {
                                                "{r.detail}"
                                            }
                                        }
                                        td { "{r.started_by.label()}" }
                                    }
                                    if is_open && let Some(e) = r.error.clone() {
                                        tr { key: "{r.id}-error", class: "error-row",
                                            td { colspan: "4",
                                                div { class: "callout err",
                                                    Ico { icon: Icon::CircleAlert, size: 16 }
                                                    div { class: "stack",
                                                        strong { "{e.message}" }
                                                        if let Some(src) = e.source.clone() {
                                                            span { class: "insp-sub", "Source: {src}" }
                                                        }
                                                        span { class: "insp-sub", "{e.hint}" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
