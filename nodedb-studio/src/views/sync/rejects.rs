//! Sync → Rejected writes: the writes devices made offline that the Origin
//! refused, and the attempt selected. The direction is device to Origin. A
//! rejection is repaired through typed actions that follow the Origin's
//! hint. There is no keep-local or keep-remote choice.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::format::{clock, clock_second};
use crate::models::sync_reject::{RejectReason, RejectState, Rejection};
use crate::routes::tabs::SyncTab;
use crate::services::backend::Backend;
use crate::state::sync::{RepairSheet, SyncUi, matching_rejects, pick_reject};
use crate::state::workspace::Surface;
use crate::views::events::common::{NoticeLine, StatusText};
use crate::views::sync::reject_details::RejectDetails;
use crate::views::sync::reject_sheets::{CompensationSheet, DiscardSheet, EditSheet};

const SURFACE: Surface = Surface::Sync(SyncTab::RejectedWrites);
const DETAILS_SIZE: PanelSize = PanelSize::width(400.0, 340.0, 620.0);

#[component]
pub fn RejectsTab() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut ui = use_context::<SyncUi>().rejects;
    let epoch = use_memo(move || ui.read().epoch);

    let mut rejected = use_resource(move || {
        let backend = backend.clone();
        let _conn = conn();
        let _epoch = epoch();
        async move { backend.rejected_writes().await }
    });
    let state = resource_state(&rejected);
    let data = state.loaded().cloned();
    let u = ui.read().clone();
    let all: Vec<Rejection> = data.as_ref().map(|d| d.rows.clone()).unwrap_or_default();
    let rows = matching_rejects(&all, &u);
    let selected = pick_reject(&rows, u.selected.as_deref()).cloned();
    let observed = data.as_ref().map(|d| d.observed_at);
    // Every source that has a rejection, once.
    let mut sources: Vec<String> = Vec::new();
    for r in &all {
        if !sources.contains(&r.source()) {
            sources.push(r.source());
        }
    }
    let sheet = u.sheet.clone().and_then(|s| {
        all.iter()
            .find(|r| r.mutation == s.mutation())
            .cloned()
            .map(|r| (s, r))
    });

    rsx! {
        div { class: "page-body",
            div { class: "workbench events-wb",
                Panel {
                    surface: SURFACE,
                    id: "rejected",
                    title: "Rejected writes",
                    class: "fill",
                    actions: rsx! {
                        span { "Device → Origin" }
                        if let Some(at) = observed {
                            span { " · Observed {clock_second(at, 0)} UTC" }
                        }
                    },
                    NoticeLine {
                        notice: u.notice.clone(),
                        on_dismiss: move |_| ui.write().notice = None,
                    }
                    div { class: "toolbar nowrap cl-filters",
                        select {
                            class: "select native",
                            "aria-label": "Source",
                            onchange: move |e| {
                                let v = e.value();
                                ui.write().source = (!v.is_empty()).then_some(v);
                            },
                            option { value: "", selected: u.source.is_none(), "Sources" }
                            for s in sources.iter() {
                                option { key: "{s}", value: "{s}", selected: u.source.as_deref() == Some(s.as_str()), "{s}" }
                            }
                        }
                        select {
                            class: "select native",
                            "aria-label": "Reason",
                            onchange: move |e| {
                                let v = e.value();
                                ui.write().reason = RejectReason::ALL.iter().copied().find(|r| r.label() == v);
                            },
                            option { value: "", selected: u.reason.is_none(), "Reasons" }
                            for r in RejectReason::ALL {
                                option { key: "{r.label()}", value: "{r.label()}", selected: Some(r) == u.reason, "{r.label()}" }
                            }
                        }
                        select {
                            class: "select native",
                            "aria-label": "State",
                            onchange: move |e| {
                                let v = e.value();
                                ui.write().state = RejectState::ALL.iter().copied().find(|s| s.label() == v);
                            },
                            option { value: "", selected: u.state.is_none(), "States" }
                            for s in RejectState::ALL {
                                option { key: "{s.label()}", value: "{s.label()}", selected: Some(s) == u.state, "{s.label()}" }
                            }
                        }
                        div { class: "search-field grow",
                            Ico { icon: Icon::Search, size: 16 }
                            input {
                                class: "input",
                                placeholder: "Filter…",
                                "aria-label": "Filter rejected writes",
                                value: "{u.filter}",
                                oninput: move |e| ui.write().filter = e.value(),
                            }
                        }
                    }
                    AsyncView {
                        loading: state.is_loading(),
                        empty: state.is_empty(),
                        error: state.error_message(),
                        retriable: state.is_retriable(),
                        on_retry: move |_| rejected.restart(),
                        empty_message: "The Origin has rejected no write from any device.",
                    }
                    if data.is_some() {
                        div { class: "dt-wrap",
                            table { class: "dt cl-table",
                                thead {
                                    tr {
                                        th { "Device" }
                                        th { "Mutation" }
                                        th { "Source" }
                                        th { "Reason" }
                                        th { "State" }
                                        th { "Received (UTC)" }
                                    }
                                }
                                tbody {
                                    for r in rows.iter() {
                                        {
                                            let id = r.mutation.clone();
                                            let is_selected = selected.as_ref().is_some_and(|s| s.mutation == r.mutation);
                                            rsx! {
                                                tr {
                                                    key: "{r.mutation}",
                                                    class: if is_selected { "clickable selected" } else { "clickable" },
                                                    "aria-selected": "{is_selected}",
                                                    tabindex: "0",
                                                    onclick: move |_| {
                                                        let mut w = ui.write();
                                                        w.selected = Some(id.clone());
                                                        w.sheet = None;
                                                    },
                                                    td { class: "key", "{r.device_id}" }
                                                    td { class: "mono", "{r.mutation}" }
                                                    td { "{r.source()}" }
                                                    td { class: "mono", "{r.reason.label()}" }
                                                    td { StatusText { tone: r.state.tone(), text: r.state.label().to_string() } }
                                                    td { class: "num", "{clock(r.received_at, 0)}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            if rows.is_empty() && !all.is_empty() {
                                div { class: "async-empty", "No rejected write matches these filters." }
                            }
                        }
                        div { class: "dt-footer", "{rows.len()} of {all.len()} rejected writes" }
                    }
                }
                ResizeHandle { surface: SURFACE, target: "details", size: DETAILS_SIZE, side: Side::After }
                Panel {
                    surface: SURFACE,
                    id: "details",
                    title: "Write attempt details",
                    size: DETAILS_SIZE,
                    if let Some(r) = selected {
                        RejectDetails { key: "{r.mutation}", rejection: r }
                    } else if !state.is_loading() {
                        div { class: "async-empty", "Select a rejected write to see its attempt." }
                    }
                }
                if let Some((s, r)) = sheet {
                    match s {
                        RepairSheet::Edit { text, busy, error, .. } => rsx! {
                            EditSheet { rejection: r, text, busy, error }
                        },
                        RepairSheet::Discard { busy, error, .. } => rsx! {
                            DiscardSheet { rejection: r, busy, error }
                        },
                        RepairSheet::Compensation { .. } => rsx! {
                            CompensationSheet { key: "{r.mutation}", rejection: r }
                        },
                    }
                }
            }
        }
    }
}
