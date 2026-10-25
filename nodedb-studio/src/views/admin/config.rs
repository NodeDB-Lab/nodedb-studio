//! Administration → Config: the values a cluster or one node runs with, and
//! the review of the values the person proposes.
//!
//! The scope and the target are explicit and sit above the panels. A
//! proposal belongs to its target, so choosing another node keeps it. The
//! review panel reads what the proposal would do and applies it only on
//! confirmation. Studio's own preferences are not config and are not here.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::admin_config::{ConfigScope, ConfigSetting, ConfigTarget};
use crate::models::format::clock_second;
use crate::routes::tabs::AdminTab;
use crate::services::backend::Backend;
use crate::state::admin::AdminUi;
use crate::state::workspace::Surface;
use crate::views::admin::common::Cut;
use crate::views::admin::config_review::ConfigReview;
use crate::views::cluster::common::Ctl;
use crate::views::events::common::NoticeLine;

const SURFACE: Surface = Surface::Admin(AdminTab::Config);
const REVIEW_SIZE: PanelSize = PanelSize::width(480.0, 360.0, 640.0);

/// The values the control offers: the server's choices, and the current
/// value when it is not among them.
fn options(s: &ConfigSetting) -> Vec<String> {
    let mut out = s.choices.clone();
    if !out.contains(&s.current) {
        out.push(s.current.clone());
    }
    out
}

#[component]
pub fn ConfigTab() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut ui = use_context::<AdminUi>().config;
    let epoch = use_memo(move || ui.read().epoch);

    let members = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let _conn = conn();
            async move { backend.cluster_members().await }
        }
    });
    let nodes: Vec<(String, String)> = members
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .map(|m| {
            m.members
                .iter()
                .map(|n| (n.id.clone(), n.name.clone()))
                .collect()
        })
        .unwrap_or_default();
    // The chosen node while it is listed, else the first.
    let target = use_memo(move || {
        let (scope, chosen) = {
            let w = ui.read();
            (w.scope, w.node.clone())
        };
        let node = match scope {
            ConfigScope::Cluster => None,
            ConfigScope::Node => {
                let read = members.read();
                let list = read.as_ref().and_then(|r| r.as_ref().ok());
                list.and_then(|l| {
                    chosen
                        .filter(|id| l.members.iter().any(|m| &m.id == id))
                        .or_else(|| l.members.first().map(|m| m.id.clone()))
                })
            }
        };
        ConfigTarget { scope, node }
    });

    let mut view = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let _conn = conn();
            let _epoch = epoch();
            let t = target();
            async move {
                // A node scope waits for the node list.
                if t.scope == ConfigScope::Node && t.node.is_none() {
                    return Err(crate::services::error::StudioError::NotConnected);
                }
                backend.config_view(&t).await
            }
        }
    });
    let state = resource_state(&view);
    let data = state.loaded().cloned();
    let u = ui.read().clone();
    let t = target();
    let proposed = u.changes_for(&t);
    let needle = u.filter.trim().to_lowercase();
    let selected = data.as_ref().and_then(|v| {
        u.selected
            .as_deref()
            .and_then(|k| v.settings.iter().find(|s| s.key == k))
            .or(v.settings.first())
            .map(|s| s.key.clone())
    });

    rsx! {
        div { class: "page-body",
            div { class: "cl-bar ad-scope",
                Ctl { label: "Scope", id: "cfg-scope",
                    select {
                        id: "cfg-scope",
                        class: "select native",
                        onchange: move |e| {
                            let v = e.value();
                            if let Some(s) = ConfigScope::ALL.iter().find(|s| s.label() == v) {
                                ui.write().scope = *s;
                            }
                        },
                        for s in ConfigScope::ALL {
                            option { key: "{s.label()}", value: "{s.label()}", selected: s == u.scope, "{s.label()}" }
                        }
                    }
                }
                Ctl { label: "Target", id: "cfg-target",
                    select {
                        id: "cfg-target",
                        class: "select native",
                        disabled: u.scope == ConfigScope::Cluster,
                        onchange: move |e| ui.write().node = Some(e.value()),
                        if u.scope == ConfigScope::Cluster {
                            option { selected: true, "All nodes" }
                        } else {
                            for (id , name) in nodes.iter() {
                                option { key: "{id}", value: "{id}", selected: t.node.as_deref() == Some(id.as_str()), "{name}" }
                            }
                        }
                    }
                }
                if let Some(v) = data.as_ref() {
                    span { class: "cl-observed", "Observed {clock_second(v.observed_at, 0)} UTC" }
                }
            }
            div { class: "workbench events-wb",
                Panel {
                    surface: SURFACE,
                    id: "settings",
                    title: "Server config",
                    class: "fill",
                    NoticeLine {
                        notice: u.notice.clone(),
                        on_dismiss: move |_| ui.write().notice = None,
                    }
                    div { class: "toolbar nowrap cl-filters",
                        div { class: "search-field grow",
                            Ico { icon: Icon::Search, size: 16 }
                            input {
                                class: "input",
                                placeholder: "Filter config…",
                                "aria-label": "Filter config",
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
                        on_retry: move |_| view.restart(),
                        empty_message: "This target reports no config.",
                    }
                    if let Some(v) = data.as_ref() {
                        div { class: "dt-wrap",
                            table { class: "dt cl-table ad-table ad-config",
                                thead {
                                    tr {
                                        th { "Setting" }
                                        th { "Current value" }
                                        th { "Proposed value" }
                                        th { "Source" }
                                        th { "Restart required" }
                                    }
                                }
                                tbody {
                                    for s in v.settings.iter().filter(|s| s.matches(&needle)) {
                                        {
                                            let key = s.key.clone();
                                            let is_selected = selected.as_deref() == Some(s.key.as_str());
                                            let value = proposed
                                                .iter()
                                                .find(|(k, _)| *k == s.key)
                                                .map(|(_, v)| v.clone())
                                                .unwrap_or_else(|| s.current.clone());
                                            let changed = value != s.current;
                                            let (key_pick, current, target_pick) = (s.key.clone(), s.current.clone(), t.clone());
                                            rsx! {
                                                tr {
                                                    key: "{s.key}",
                                                    class: if is_selected { "clickable selected" } else { "clickable" },
                                                    "aria-selected": "{is_selected}",
                                                    onclick: move |_| ui.write().selected = Some(key.clone()),
                                                    td { class: "key",
                                                        span { class: "ad-name",
                                                            if changed {
                                                                span { class: "unsaved-dot", title: "A change is proposed" }
                                                            }
                                                            span { title: "{s.description}", "{s.key}" }
                                                        }
                                                    }
                                                    td { "{s.current}" }
                                                    td {
                                                        input {
                                                            class: "input ad-proposal-input",
                                                            list: "dl-{s.key}",
                                                            "aria-label": "Proposed value of {s.key}",
                                                            value: "{value}",
                                                            spellcheck: false,
                                                            onclick: move |e| e.stop_propagation(),
                                                            onchange: move |e| {
                                                                let mut w = ui.write();
                                                                w.propose(&target_pick, &key_pick, e.value().trim(), &current);
                                                                w.selected = Some(key_pick.clone());
                                                            },
                                                        }
                                                        datalist { id: "dl-{s.key}",
                                                            for o in options(s) {
                                                                option { key: "{o}", value: "{o}" }
                                                            }
                                                        }
                                                    }
                                                    td { Cut { text: s.source.label().to_string() } }
                                                    td { if s.restart_required { "Yes" } else { "No" } }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        div { class: "dt-footer",
                            "{v.settings.len()} settings on {v.target_name}"
                            if !proposed.is_empty() {
                                " · {proposed.len()} proposed"
                            }
                        }
                    }
                }
                ResizeHandle { surface: SURFACE, target: "review", size: REVIEW_SIZE, side: Side::After }
                Panel {
                    surface: SURFACE,
                    id: "review",
                    title: "Proposal review",
                    size: REVIEW_SIZE,
                    ConfigReview { key: "{t:?}", target: t.clone(), selected: selected.clone() }
                }
            }
        }
    }
}
