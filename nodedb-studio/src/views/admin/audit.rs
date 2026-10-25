//! Administration → Audit: the trail of administrative changes, filtered
//! and paged, with the chosen event's detail beside it.
//!
//! Two panels. Every filter runs on the server. An event is read-only, and
//! the footer states how long the trail is kept and which permission scopes
//! what the reader sees. It does not promise the events last forever.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::pager::Pager;
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::admin_audit::{AuditRange, AuditResult, TargetType};
use crate::models::format::date_second;
use crate::routes::tabs::AdminTab;
use crate::services::backend::Backend;
use crate::state::admin::AdminUi;
use crate::state::workspace::Surface;
use crate::views::admin::audit_details::EventDetails;
use crate::views::admin::common::Cut;
use crate::views::cluster::common::Ctl;
use crate::views::events::common::{InfoTip, StatusText};

const SURFACE: Surface = Surface::Admin(AdminTab::Audit);
const DETAILS_SIZE: PanelSize = PanelSize::width(380.0, 340.0, 620.0);

#[component]
pub fn AuditTab() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut ui = use_context::<AdminUi>().audit;
    let query = use_memo(move || ui.read().query());

    let mut page = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let _conn = conn();
            let q = query();
            async move { backend.audit_events(&q).await }
        }
    });
    // The filter menus list every actor and action, not only those on the page.
    let facets = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let _conn = conn();
            async move { backend.audit_facets().await }
        }
    });
    let state = resource_state(&page);
    let data = state.loaded().cloned();
    let u = ui.read().clone();
    let facet = facets
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned();

    // The chosen event, else the first on the page.
    let event_id = use_memo(move || {
        let chosen = ui.read().selected.clone();
        let read = page.read();
        let first = read
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .and_then(|p| p.events.first().map(|e| e.id.clone()));
        chosen.or(first)
    });
    let selected = event_id();

    rsx! {
        div { class: "page-body",
            div { class: "workbench events-wb",
                Panel {
                    surface: SURFACE,
                    id: "log",
                    title: "Audit log",
                    class: "fill",
                    div { class: "toolbar nowrap cl-filters",
                        Ctl { label: "Time range (UTC)", id: "au-range",
                            select {
                                id: "au-range",
                                class: "select native",
                                onchange: move |e| {
                                    let v = e.value();
                                    if let Some(r) = AuditRange::ALL.iter().find(|r| r.label() == v) {
                                        let mut w = ui.write();
                                        w.range = *r;
                                        w.refilter();
                                    }
                                },
                                for r in AuditRange::ALL {
                                    option { key: "{r.label()}", value: "{r.label()}", selected: r == u.range, "{r.label()}" }
                                }
                            }
                        }
                        Ctl { label: "Actor", id: "au-actor",
                            select {
                                id: "au-actor",
                                class: "select native",
                                onchange: move |e| {
                                    let v = e.value();
                                    let mut w = ui.write();
                                    w.actor = (!v.is_empty()).then_some(v);
                                    w.refilter();
                                },
                                option { value: "", selected: u.actor.is_none(), "All actors" }
                                for a in facet.iter().flat_map(|f| f.actors.iter()) {
                                    option { key: "{a}", value: "{a}", selected: u.actor.as_deref() == Some(a.as_str()), "{a}" }
                                }
                            }
                        }
                        Ctl { label: "Action", id: "au-action",
                            select {
                                id: "au-action",
                                class: "select native",
                                onchange: move |e| {
                                    let v = e.value();
                                    let mut w = ui.write();
                                    w.action = (!v.is_empty()).then_some(v);
                                    w.refilter();
                                },
                                option { value: "", selected: u.action.is_none(), "All actions" }
                                for a in facet.iter().flat_map(|f| f.actions.iter()) {
                                    option { key: "{a}", value: "{a}", selected: u.action.as_deref() == Some(a.as_str()), "{a}" }
                                }
                            }
                        }
                    }
                    div { class: "toolbar nowrap cl-filters",
                        Ctl { label: "Target", id: "au-target",
                            select {
                                id: "au-target",
                                class: "select native",
                                onchange: move |e| {
                                    let v = e.value();
                                    let mut w = ui.write();
                                    w.target = TargetType::ALL.iter().copied().find(|t| t.label() == v);
                                    w.refilter();
                                },
                                option { value: "", selected: u.target.is_none(), "All targets" }
                                for t in TargetType::ALL {
                                    option { key: "{t.label()}", value: "{t.label()}", selected: Some(t) == u.target, "{t.label()}" }
                                }
                            }
                        }
                        Ctl { label: "Result", id: "au-result",
                            select {
                                id: "au-result",
                                class: "select native",
                                onchange: move |e| {
                                    let v = e.value();
                                    let mut w = ui.write();
                                    w.result = AuditResult::ALL.iter().copied().find(|r| r.label() == v);
                                    w.refilter();
                                },
                                option { value: "", selected: u.result.is_none(), "All results" }
                                for r in AuditResult::ALL {
                                    option { key: "{r.label()}", value: "{r.label()}", selected: Some(r) == u.result, "{r.label()}" }
                                }
                            }
                        }
                        div { class: "toolbar-spacer" }
                        button {
                            class: "icon-btn",
                            title: "Read the trail again",
                            "aria-label": "Refresh the audit log",
                            onclick: move |_| page.restart(),
                            Ico { icon: Icon::RefreshCw, size: 16 }
                        }
                        button {
                            class: "btn primary",
                            disabled: true,
                            title: "The export format and destination are not designed yet",
                            Ico { icon: Icon::Download, size: 16 }
                            "Export"
                        }
                    }
                    AsyncView {
                        loading: state.is_loading(),
                        empty: state.is_empty(),
                        error: state.error_message(),
                        retriable: state.is_retriable(),
                        on_retry: move |_| page.restart(),
                        empty_message: "No event matches these filters in this time range.",
                    }
                    if let Some(p) = data.as_ref() {
                        div { class: "dt-wrap",
                            table { class: "dt cl-table ad-table ad-audit",
                                thead {
                                    tr {
                                        th { "Timestamp (UTC)" }
                                        th { "Actor" }
                                        th { "Action" }
                                        th { "Database object" }
                                        th { "Result" }
                                        th { "Correlation ID" }
                                    }
                                }
                                tbody {
                                    for e in p.events.iter() {
                                        {
                                            let id = e.id.clone();
                                            let is_selected = selected.as_deref() == Some(e.id.as_str());
                                            rsx! {
                                                tr {
                                                    key: "{e.id}",
                                                    class: if is_selected { "clickable selected" } else { "clickable" },
                                                    "aria-selected": "{is_selected}",
                                                    tabindex: "0",
                                                    onclick: move |_| ui.write().selected = Some(id.clone()),
                                                    td { class: "mono", "{date_second(e.at, 0)}" }
                                                    td { Cut { text: e.actor.clone() } }
                                                    td { class: "key", Cut { text: e.action.clone() } }
                                                    td { Cut { text: e.object.clone() } }
                                                    td { StatusText { tone: e.result.tone(), text: e.result.label().to_string() } }
                                                    td { class: "mono", "{e.correlation}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "dt-footer ad-audit-footer",
                        if let Some(p) = data.as_ref() {
                            Pager {
                                page: p.page,
                                pages: p.pages(),
                                on_page: move |n| ui.write().page = n,
                            }
                            span { class: "ad-span", "{p.span_text()}" }
                        }
                        div { class: "toolbar-spacer" }
                        if let Some(f) = facet.as_ref() {
                            span { class: "ad-retention", "Retention: {f.retention_days} days ({f.scope})" }
                            InfoTip {
                                text: format!(
                                    "The server removes events older than {} days. You see the events your {} permission covers. Studio cannot edit or delete an event.",
                                    f.retention_days,
                                    f.scope,
                                ),
                            }
                        }
                    }
                }
                ResizeHandle { surface: SURFACE, target: "details", size: DETAILS_SIZE, side: Side::After }
                Panel {
                    surface: SURFACE,
                    id: "details",
                    title: "Event details",
                    size: DETAILS_SIZE,
                    if let Some(id) = selected {
                        // The read belongs to the event: a new id mounts a new one.
                        EventDetails { key: "{id}", id: id.clone() }
                    } else if !state.is_loading() {
                        div { class: "async-empty", "Select an event to see its details." }
                    }
                }
            }
        }
    }
}
