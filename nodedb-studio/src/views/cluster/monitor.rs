//! Cluster → Monitoring: server telemetry over a time window. Four charts
//! with real axes and units, and the latest report of each node.
//!
//! This is what the servers report about themselves, not collection data.
//! The window, the node filter, and the refresh interval live in
//! `ClusterUi`. A node that stopped reporting stays in the table as Stale
//! with the time of its last report, and its chart line ends there.

use std::rc::Rc;
use std::time::Duration;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::cluster_monitor::{
    Freshness, Metric, NodeTelemetry, Telemetry, TelemetryRange, capacity_text,
};
use crate::models::feeds::Tone;
use crate::models::format::{ago, clock_second};
use crate::models::timeseries_axis::fixed;
use crate::models::ts_dashboard::Refresh;
use crate::routes::tabs::ClusterTab;
use crate::services::backend::Backend;
use crate::state::cluster::ClusterUi;
use crate::state::workspace::{Surface, Workspace};
use crate::views::cluster::common::Ctl;
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
    let row = |a: Metric, b: Metric| visible(a.panel()) || visible(b.panel());
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
                                    td { class: "num", "{fixed(n.cpu_percent, 0)}%" }
                                    td { class: "num", "{capacity_text(n.memory_used_bytes)} / {capacity_text(n.memory_total_bytes)} ({n.memory_percent()}%)" }
                                    td { class: "num", "{capacity_text(n.disk_used_bytes)} / {capacity_text(n.disk_total_bytes)} ({n.disk_percent()}%)" }
                                    td { class: "num", "{fixed(n.net_in_mbs, 0)} MB/s / {fixed(n.net_out_mbs, 0)} MB/s" }
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
