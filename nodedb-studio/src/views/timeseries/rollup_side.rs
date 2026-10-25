//! The Rollup panel: the selected tier's saved definition as SQL, its
//! refresh policy, and its queued refreshes. A saved definition is not a
//! runtime state: the latest run and data completeness read Not reported
//! until the server measures them.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::components::tabs::TabBar;
use crate::models::format::date_minute;
use crate::models::ts_lifecycle::{Lifecycle, RollupRun, Tier};
use crate::state::timeseries::RollupTab;
use crate::views::events::common::SqlBlock;

#[component]
pub fn RollupBody(
    l: Lifecycle,
    tier: Tier,
    tab: RollupTab,
    runs: Vec<RollupRun>,
    on_tab: EventHandler<RollupTab>,
    on_refresh: EventHandler<()>,
    on_explore: EventHandler<()>,
) -> Element {
    let rollup = l.rollup(&tier.name).cloned();
    let items: Vec<&'static str> = RollupTab::ALL.iter().map(|t| t.label()).collect();
    let active = RollupTab::ALL.iter().position(|t| *t == tab).unwrap_or(0);
    let sql = rollup.as_ref().map(|r| {
        let input = l.rollup(&r.input);
        r.sql(input.is_none(), input)
    });
    let policy = tier.refresh.as_ref().map(|r| r.label());
    let latest = runs
        .first()
        .map(|r| format!("Queued {} UTC", date_minute(r.requested, 0)));

    rsx! {
        div { class: "ts-rollup",
            div { class: "ts-rollup-head",
                label { r#for: "ts-rollup-name", "Name" }
                input { id: "ts-rollup-name", class: "input", readonly: true, value: "{tier.name}" }
                span { class: "status ts-saved",
                    span { class: "status-dot ok" }
                    if rollup.is_some() { "Definition saved" } else { "Collection" }
                }
            }
            if rollup.is_some() {
                TabBar {
                    items,
                    active,
                    on_select: move |i: usize| {
                        if let Some(t) = RollupTab::ALL.get(i) {
                            on_tab.call(*t);
                        }
                    },
                }
                match tab {
                    RollupTab::Definition => rsx! {
                        if let Some(sql) = sql {
                            SqlBlock { sql, class: "ts-sql" }
                        }
                    },
                    RollupTab::Runs => rsx! {
                        div { class: "dt-wrap ts-runs",
                            table { class: "dt",
                                thead { tr { th { "Operation" } th { "Requested" } th { "Status" } } }
                                tbody {
                                    for r in runs.iter() {
                                        tr { key: "{r.operation}",
                                            td { class: "mono", "{r.operation}" }
                                            td { "{date_minute(r.requested, 0)} UTC" }
                                            td { "{r.status}. Progress is not reported." }
                                        }
                                    }
                                    if runs.is_empty() {
                                        tr { td { colspan: "3", class: "ts-muted", "No refresh was queued from Studio. Refresh now queues one." } }
                                    }
                                }
                            }
                        }
                    },
                }
                h3 { class: "ts-props-title", "Refresh" }
                dl { class: "ts-props",
                    dt { "Policy" }
                    dd { "{policy.clone().unwrap_or_default()}" }
                    dt { "Latest run" }
                    dd {
                        match latest {
                            Some(t) => rsx! { "{t}" },
                            None => rsx! { span { class: "ts-muted", title: "The server does not report refresh runs yet.", "Not reported" } },
                        }
                    }
                    dt { "Data completeness" }
                    dd { span { class: "ts-muted", title: "Event-time completeness needs a source-aware measure the server does not report yet.", "Not reported" } }
                }
            } else {
                div { class: "ts-raw-note",
                    p { "{tier.name} is the raw collection. It stores every sale as written, for {tier.keep.label()}." }
                    p { class: "ts-muted", "Select a rollup to see its definition and refresh policy." }
                }
            }
            div { class: "ts-rollup-actions",
                button {
                    class: "btn primary",
                    disabled: rollup.is_none(),
                    title: if rollup.is_some() { "Queue a refresh. Its progress is not reported." } else { "Only a rollup refreshes." },
                    onclick: move |_| on_refresh.call(()),
                    "Refresh now"
                }
                button {
                    class: "btn",
                    title: "Open the same question over the raw rows in Explore",
                    onclick: move |_| on_explore.call(()),
                    Ico { icon: Icon::ExternalLink, size: 16 }
                    "Open in Explore"
                }
            }
        }
    }
}
