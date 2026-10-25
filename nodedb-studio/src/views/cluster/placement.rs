//! Cluster → Placement: which group owns each vShard, where its replicas
//! sit, and the reviewed plan that moves one.
//!
//! Four panels. vShards and Rebalance preview stack on the left. vShard
//! details and Nodes health stack on the right. The selection and the staged
//! plan live in `ClusterUi`. A plan never applies from here: Review plan
//! opens a sheet with the impact preview first.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side, region_style};
use crate::models::cluster_place::size_text;
use crate::models::format::clock_second;
use crate::routes::tabs::ClusterTab;
use crate::services::backend::Backend;
use crate::state::cluster::{ClusterUi, pick_vshard};
use crate::state::workspace::{Surface, Workspace};
use crate::views::cluster::place_details::{NodesHealth, VShardDetails};
use crate::views::cluster::place_rebalance::Rebalance;
use crate::views::cluster::place_review::PlanSheet;
use crate::views::events::common::{NoticeLine, StatusText};

const SURFACE: Surface = Surface::Cluster(ClusterTab::Placement);
const REBALANCE_SIZE: PanelSize = PanelSize::height(300.0, 220.0, 480.0);
const SIDE_SIZE: PanelSize = PanelSize::width(400.0, 300.0, 620.0);
const HEALTH_SIZE: PanelSize = PanelSize::height(260.0, 150.0, 420.0);

#[component]
pub fn PlacementTab() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let layout = use_context::<Signal<Workspace>>();
    let mut ui = use_context::<ClusterUi>().place;
    let epoch = use_memo(move || ui.read().epoch);

    let mut placement = use_resource(move || {
        let backend = backend.clone();
        let _conn = conn();
        let _epoch = epoch();
        async move { backend.placement().await }
    });
    let state = resource_state(&placement);
    let list = state.loaded().cloned();

    let u = ui.read().clone();
    let visible = list.as_ref().map(|p| u.visible(p)).unwrap_or_default();
    let selected = pick_vshard(&visible, u.vshard).cloned();
    let databases = list.as_ref().map(|p| p.databases()).unwrap_or_default();
    let collections = list
        .as_ref()
        .map(|p| p.collections(u.database.as_deref()))
        .unwrap_or_default();
    let observed = list
        .as_ref()
        .map(|p| format!("Observed {} UTC", clock_second(p.observed_at, 0)))
        .unwrap_or_default();
    let side_style = region_style(&layout.read(), SURFACE, "side", SIDE_SIZE);
    let side_shown = {
        let ws = layout.read();
        ws.is_visible(SURFACE, "details") || ws.is_visible(SURFACE, "health")
    };

    rsx! {
        div { class: "page-body",
            div { class: "workbench events-wb",
                div { class: "workbench col",
                    Panel {
                        surface: SURFACE,
                        id: "vshards",
                        title: "vShards",
                        class: "fill",
                        actions: rsx! {
                            if !observed.is_empty() {
                                span { "{observed}" }
                            }
                        },
                        div { class: "toolbar nowrap",
                            div { class: "cl-ctl",
                                label { r#for: "place-db", "Database" }
                                select {
                                    id: "place-db",
                                    class: "select native",
                                    onchange: {
                                        let list = list.clone();
                                        move |e| {
                                            let Some(p) = list.as_ref() else { return };
                                            let v = e.value();
                                            ui.write().set_database((!v.is_empty()).then_some(v), p);
                                        }
                                    },
                                    option { value: "", selected: u.database.is_none(), "All databases" }
                                    for d in databases.iter() {
                                        option { key: "{d}", value: "{d}", selected: u.database.as_deref() == Some(d.as_str()), "{d}" }
                                    }
                                }
                            }
                            div { class: "cl-ctl",
                                label { r#for: "place-coll", "Collection" }
                                select {
                                    id: "place-coll",
                                    class: "select native",
                                    onchange: move |e| {
                                        let v = e.value();
                                        ui.write().collection = (!v.is_empty()).then_some(v);
                                    },
                                    option { value: "", selected: u.collection.is_none(), "All collections" }
                                    for c in collections.iter() {
                                        option { key: "{c}", value: "{c}", selected: u.collection.as_deref() == Some(c.as_str()), "{c}" }
                                    }
                                }
                            }
                            div { class: "search-field grow",
                                Ico { icon: Icon::Search, size: 16 }
                                input {
                                    class: "input",
                                    placeholder: "Filter vShards…",
                                    "aria-label": "Filter vShards",
                                    value: "{u.filter}",
                                    oninput: move |e| ui.write().filter = e.value(),
                                }
                            }
                            button {
                                class: "btn",
                                title: "Read placement again",
                                "aria-label": "Refresh placement",
                                onclick: move |_| placement.restart(),
                                Ico { icon: Icon::RefreshCw, size: 16 }
                            }
                        }
                        NoticeLine { notice: u.notice.clone(), on_dismiss: move |_| ui.write().notice = None }
                        AsyncView {
                            loading: state.is_loading(),
                            empty: state.is_empty(),
                            error: state.error_message(),
                            retriable: state.is_retriable(),
                            on_retry: move |_| placement.restart(),
                            empty_message: "No vShard is placed. Create a collection to place one.",
                        }
                        if list.is_some() {
                            div { class: "dt-wrap",
                                table { class: "dt cl-table",
                                    thead {
                                        tr {
                                            if u.collection.is_none() {
                                                th { "Collection" }
                                            }
                                            th { class: "num", "vShard" }
                                            th { "Group" }
                                            th { "Leader" }
                                            th { "Replicas" }
                                            th { class: "num", "Size" }
                                            th { "Status" }
                                        }
                                    }
                                    tbody {
                                        for v in visible.iter() {
                                            {
                                                let number = v.number;
                                                let is_selected = selected.as_ref().is_some_and(|s| s.number == v.number);
                                                let status = v.status();
                                                rsx! {
                                                    tr {
                                                        key: "{v.number}",
                                                        class: if is_selected { "clickable selected" } else { "clickable" },
                                                        "aria-selected": "{is_selected}",
                                                        tabindex: "0",
                                                        onclick: move |_| {
                                                            let mut w = ui.write();
                                                            w.vshard = Some(number);
                                                            w.from = None;
                                                            w.to = None;
                                                        },
                                                        if u.collection.is_none() {
                                                            td { "{v.source()}" }
                                                        }
                                                        td { class: "num key", "{v.number}" }
                                                        td { "{v.group}" }
                                                        td { "{v.leader}" }
                                                        td {
                                                            for (i , r) in v.replicas.iter().enumerate() {
                                                                span { key: "{r}",
                                                                    if i > 0 {
                                                                        span { class: "cl-sep", " / " }
                                                                    }
                                                                    span {
                                                                        class: if v.unreachable.contains(r) { "cl-replica down" } else { "cl-replica" },
                                                                        title: if v.unreachable.contains(r) { "This node does not answer" } else { "" },
                                                                        "{r}"
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        td { class: "num", "{size_text(v.size_bytes)}" }
                                                        td { StatusText { tone: status.tone(), text: status.label().to_string() } }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                if visible.is_empty() {
                                    div { class: "async-empty", "No vShard matches the filters." }
                                }
                            }
                            div { class: "dt-footer", "{visible.len()} vShards shown" }
                        }
                    }
                    ResizeHandle { surface: SURFACE, target: "rebalance", size: REBALANCE_SIZE, side: Side::After }
                    Panel {
                        surface: SURFACE,
                        id: "rebalance",
                        title: "Rebalance preview",
                        size: REBALANCE_SIZE,
                        if let (Some(p), Some(v)) = (list.clone(), selected.clone()) {
                            Rebalance { placement: p, vshard: v }
                        }
                    }
                }
                if side_shown {
                    ResizeHandle { surface: SURFACE, target: "side", size: SIDE_SIZE, side: Side::After }
                    div { class: "workbench col", style: "{side_style}",
                        Panel {
                            surface: SURFACE,
                            id: "details",
                            title: "vShard details",
                            class: "fill",
                            if let (Some(p), Some(v)) = (list.clone(), selected.clone()) {
                                VShardDetails { placement: p, vshard: v }
                            }
                        }
                        ResizeHandle { surface: SURFACE, target: "health", size: HEALTH_SIZE, side: Side::After }
                        Panel {
                            surface: SURFACE,
                            id: "health",
                            title: "Nodes health",
                            size: HEALTH_SIZE,
                            if let Some(p) = list.clone() {
                                NodesHealth { placement: p, group: selected.as_ref().map(|v| v.group.clone()) }
                            }
                        }
                    }
                }
                if let (Some(r), Some(p)) = (u.review.clone(), list.clone()) {
                    PlanSheet { review: r, placement: p }
                }
            }
        }
    }
}
