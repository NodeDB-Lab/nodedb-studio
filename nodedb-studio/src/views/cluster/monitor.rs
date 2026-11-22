//! Cluster → Monitoring: server telemetry over a time window. Four charts
//! with real axes and units, and the latest report of each node.
//!
//! This is what the servers report about themselves, not collection data.
//! The window, the node filter, and the refresh interval live in
//! `ClusterUi`. A node that stopped reporting stays in the table as Stale
//! with the time of its last report, and its chart line ends there.
//!
//! A server that keeps no history shows no charts and no time range: only
//! the point-in-time facts the connected node reports, labelled as its own.
//! A refresh is a click or the chosen interval (10 s or more, off by
//! default). The timer stops when the tab unmounts, and a new read drops
//! the one before it, so two reads never overlap.

use std::rc::Rc;
use std::time::Duration;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::cluster::monitor::{
    EngineBudget, Freshness, Metric, NodeTelemetry, Telemetry, TelemetryRange, capacity_text,
};
use crate::models::cluster::place::size_text;
use crate::models::feeds::Tone;
use crate::models::format::{ago, clock_second};
use crate::models::timeseries::axis::fixed;
use crate::models::timeseries::dashboard::Refresh;
use crate::routes::tabs::ClusterTab;
use crate::services::backend::Backend;
use crate::state::caps::cluster::shows_cluster_history;
use crate::state::cluster::ClusterUi;
use crate::state::gates::use_caps;
use crate::state::workspace::{Surface, Workspace};
use crate::views::cluster::common::{Ctl, NotReported, SeenBy};
use crate::views::cluster::metric_chart::MetricChart;
use crate::views::events::common::StatusText;

const SURFACE: Surface = Surface::Cluster(ClusterTab::Monitoring);
const NODES_SIZE: PanelSize = PanelSize::height(300.0, 150.0, 420.0);

fn summary(t: &Telemetry, metric: Metric) -> String {
    match t.series_of(metric).and_then(|s| s.average().zip(s.max())) {
        Some((avg, max)) => {
            let d = metric.decimals();
            let u = metric.unit();
            format!("Avg {} {u} · Max {} {u}", fixed(avg, d), fixed(max, d))
        }
        None => String::new(),
    }
}

/// `20.4 GiB / 32.0 GiB (62%)`, or Not reported.
fn used_of(used: Option<u64>, total: Option<u64>, percent: Option<u64>) -> Element {
    match (used, total, percent) {
        (Some(u), Some(t), Some(p)) => {
            rsx! { "{capacity_text(u)} / {capacity_text(t)} ({p}%)" }
        }
        (Some(u), Some(t), None) => rsx! { "{capacity_text(u)} / {capacity_text(t)}" },
        _ => rsx! { NotReported {} },
    }
}

fn freshness_tone(f: Freshness) -> Tone {
    match f {
        Freshness::Ready => Tone::Ok,
        Freshness::Stale => Tone::Err,
    }
}

#[component]
pub fn MonitoringTab() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let layout = use_context::<Signal<Workspace>>();
    let mut ui = use_context::<ClusterUi>().monitor;
    let history = shows_cluster_history(use_caps().as_ref());
    // The read depends on these three and nothing else.
    let key = use_memo(move || {
        let u = ui.read();
        (u.node.clone(), u.range, u.tick)
    });

    let mut telemetry = use_resource(move || {
        let backend = backend.clone();
        let _conn = conn();
        let (node, range, _tick) = key();
        async move { backend.telemetry(node.as_deref(), range).await }
    });
    // The refresh interval reads again on a timer. The loop stops with the tab.
    use_resource(move || {
        let every = ui.read().refresh.seconds();
        async move {
            let Some(secs) = every else { return };
            loop {
                tokio::time::sleep(Duration::from_secs(secs)).await;
                let next = ui.peek().tick + 1;
                ui.write().tick = next;
            }
        }
    });

    let state = resource_state(&telemetry);
    let data = state.loaded().cloned();
    let u = ui.read().clone();
    let nodes: Vec<NodeTelemetry> = data.as_ref().map(|t| t.nodes.clone()).unwrap_or_default();
    let visible = |id: &'static str| layout.read().is_visible(SURFACE, id);
    let row = |a: Metric, b: Metric| history && (visible(a.panel()) || visible(b.panel()));
    let charts = |a: Metric, b: Metric| {
        rsx! {
            if row(a, b) {
                div { class: "workbench",
                    for m in [a, b] {
                        Panel {
                            key: "{m.panel()}",
                            surface: SURFACE,
                            id: m.panel(),
                            title: m.title().to_string(),
                            class: "fill",
                            actions: rsx! {
                                if let Some(t) = data.as_ref() {
                                    span { "{summary(t, m)}" }
                                }
                            },
                            if let Some(t) = data.as_ref() {
                                if let Some(s) = t.series_of(m) {
                                    MetricChart { series: s.clone(), start: t.observed_at - t.range.seconds(), end: t.observed_at }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    rsx! {
        div { class: "page-body",
            div { class: "cl-bar",
                if history {
                Ctl { label: "Node", id: "mon-node",
                    select {
                        id: "mon-node",
                        class: "select native",
                        onchange: move |e| {
                            let v = e.value();
                            ui.write().node = (!v.is_empty()).then_some(v);
                        },
                        option { value: "", selected: u.node.is_none(), "All nodes" }
                        for n in nodes.iter() {
                            option { key: "{n.node_id}", value: "{n.node_id}", selected: u.node.as_deref() == Some(n.node_id.as_str()), "{n.node}" }
                        }
                    }
                }
                Ctl { label: "Time range", id: "mon-range",
                    select {
                        id: "mon-range",
                        class: "select native",
                        onchange: move |e| {
                            let v = e.value();
                            if let Some(r) = TelemetryRange::ALL.iter().find(|r| r.label() == v) {
                                ui.write().range = *r;
                            }
                        },
                        for r in TelemetryRange::ALL {
                            option { key: "{r.label()}", value: "{r.label()}", selected: r == u.range, "{r.label()}" }
                        }
                    }
                }
                }
                Ctl { label: "Refresh", id: "mon-refresh",
                    select {
                        id: "mon-refresh",
                        class: "select native",
                        onchange: move |e| {
                            let v = e.value();
                            if let Some(r) = Refresh::ALL.iter().find(|r| r.label() == v) {
                                ui.write().refresh = *r;
                            }
                        },
                        for r in Refresh::ALL {
                            option { key: "{r.label()}", value: "{r.label()}", selected: r == u.refresh, "{r.label()}" }
                        }
                    }
                }
                button {
                    class: "icon-btn",
                    title: "Read telemetry now",
                    "aria-label": "Refresh telemetry",
                    onclick: move |_| telemetry.restart(),
                    Ico { icon: Icon::RefreshCw, size: 16 }
                }
                div { class: "toolbar-spacer" }
                span { class: "badge outline", title: "What the servers report about themselves. Not collection records.", "Server telemetry" }
                if let Some(t) = data.as_ref() {
                    SeenBy { observer: t.observer.clone() }
                    span { class: "cl-observed", "Observed {clock_second(t.observed_at, 0)} UTC" }
                }
            }
            AsyncView {
                loading: state.is_loading(),
                empty: state.is_empty(),
                error: state.error_message(),
                retriable: state.is_retriable(),
                on_retry: move |_| telemetry.restart(),
                empty_message: "No node reports telemetry.",
            }
            div { class: "workbench col",
                {charts(Metric::Queries, Metric::P99Latency)}
                {charts(Metric::Cpu, Metric::Memory)}
                ResizeHandle { surface: SURFACE, target: "nodes", size: NODES_SIZE, side: Side::After }
                Panel {
                    surface: SURFACE,
                    id: "nodes",
                    title: "Nodes",
                    size: NODES_SIZE,
                    if let Some(t) = data.as_ref() {
                        NodeTable { telemetry: t.clone() }
                        if !history && t.nodes.iter().all(|n| n.cpu_percent.is_none()) {
                            p { class: "inspector-note",
                                "NodeDB 0.6.2 has no statement for CPU, process memory, disk, or network, and keeps no history to chart."
                            }
                        }
                        Budgets { budgets: t.budgets.clone(), node: t.observer.node_id.clone() }
                    }
                }
            }
        }
    }
}

#[component]
fn NodeTable(telemetry: Telemetry) -> Element {
    let mut ui = use_context::<ClusterUi>().monitor;
    let scope = telemetry.scope.clone();
    let now = telemetry.observed_at;
    rsx! {
        div { class: "dt-wrap",
            table { class: "dt cl-table",
                thead {
                    tr {
                        th { "Node" }
                        th { "Status" }
                        th { class: "num", "CPU" }
                        th { class: "num", "Memory" }
                        th { class: "num", "Disk" }
                        th { class: "num", "Network (In / Out)" }
                        th { "Last observed" }
                    }
                }
                tbody {
                    for n in telemetry.nodes.iter() {
                        {
                            let id = n.node_id.clone();
                            let f = n.freshness(now);
                            let is_selected = scope.as_deref() == Some(n.node_id.as_str());
                            let stale = f == Freshness::Stale;
                            rsx! {
                                tr {
                                    key: "{n.node_id}",
                                    class: match (is_selected, stale) {
                                        (true, true) => "clickable selected cl-stale-row",
                                        (true, false) => "clickable selected",
                                        (false, true) => "clickable cl-stale-row",
                                        (false, false) => "clickable",
                                    },
                                    "aria-selected": "{is_selected}",
                                    tabindex: "0",
                                    title: if stale { "This node stopped reporting. Values are from its last report." } else { "" },
                                    onclick: move |_| ui.write().node = Some(id.clone()),
                                    td { class: "key", "{n.node}" }
                                    td { StatusText { tone: freshness_tone(f), text: f.label().to_string() } }
                                    td { class: "num",
                                        match n.cpu_percent {
                                            Some(c) => rsx! { "{fixed(c, 0)}%" },
                                            None => rsx! { NotReported {} },
                                        }
                                    }
                                    td { class: "num", {used_of(n.memory_used_bytes, n.memory_total_bytes, n.memory_percent())} }
                                    td { class: "num", {used_of(n.disk_used_bytes, n.disk_total_bytes, n.disk_percent())} }
                                    td { class: "num",
                                        match (n.net_in_mbs, n.net_out_mbs) {
                                            (Some(i), Some(o)) => rsx! { "{fixed(i, 0)} MB/s / {fixed(o, 0)} MB/s" },
                                            _ => rsx! { NotReported {} },
                                        }
                                    }
                                    td {
                                        "{clock_second(n.observed_at, 0)} UTC"
                                        if stale {
                                            span { class: "cl-age", " · {ago((now - n.observed_at).max(0) as u64)}" }
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

/// The observer's engine memory budgets: budgets engines reserve against,
/// not the process's memory.
#[component]
fn Budgets(budgets: Vec<EngineBudget>, node: String) -> Element {
    if budgets.is_empty() {
        return rsx! {};
    }
    rsx! {
        h4 { class: "inspector-section", "Engine memory budgets on node {node}" }
        div { class: "dt-wrap",
            table { class: "dt cl-table",
                thead {
                    tr {
                        th { "Engine" }
                        th { class: "num", "Budget" }
                        th { class: "num", title: "Bytes reserved now. NodeDB reserves only on some engine paths (flushes, checkpoints, index builds), so a busy node can read 0.", "Reserved" }
                        th { class: "num", "Peak" }
                        th { class: "num", title: "Reservations refused for lack of budget", "Refused" }
                    }
                }
                tbody {
                    for b in budgets.iter() {
                        tr { key: "{b.engine}",
                            td { class: "key", "{b.engine}" }
                            td { class: "num", "{size_text(b.limit_bytes)}" }
                            td { class: "num", "{size_text(b.reserved_bytes)}" }
                            td { class: "num", "{size_text(b.peak_bytes)}" }
                            td { class: "num", "{b.rejections}" }
                        }
                    }
                }
            }
        }
        p { class: "inspector-note",
            "Budgets the engines reserve against on node {node}, not the process's memory. NodeDB reserves only on some engine paths, so a busy node can read 0."
        }
    }
}
