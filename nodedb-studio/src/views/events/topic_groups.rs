//! A topic's consumer group offsets and the topic's own details.
//!
//! A committed offset belongs to one partition of one topic. Reset offsets
//! opens a review: it never moves an offset by itself.

use dioxus::prelude::*;

use crate::components::async_view::AsyncView;
use crate::components::icon::{Ico, Icon};
use crate::models::format::thousands;
use crate::models::topics::{ConsumerGroup, ResetScope, ResetTarget, Topic};
use crate::state::events::EventsUi;
use crate::state::events::topics::ResetForm;

#[component]
pub fn GroupPanel(
    groups: Vec<ConsumerGroup>,
    group: Option<ConsumerGroup>,
    loading: bool,
    error: Option<String>,
    on_retry: EventHandler<()>,
) -> Element {
    let mut ui = use_context::<EventsUi>().topics;
    let (chosen_partition, input_partition) = {
        let u = ui.read();
        (u.selected_partition, u.input.partition)
    };
    let selected_partition = chosen_partition.unwrap_or(input_partition);
    rsx! {
        AsyncView {
            loading,
            empty: groups.is_empty(),
            error: error.clone(),
            retriable: error.is_some(),
            on_retry: move |_| on_retry.call(()),
            empty_message: "This topic has no consumer groups.",
        }
        if let Some(g) = group.clone() {
            div { class: "group-pick",
                label { class: "toolbar-label", r#for: "tg-group", "Group name" }
                select {
                    id: "tg-group",
                    class: "select native",
                    onchange: move |e| {
                        let mut w = ui.write();
                        w.selected_group = Some(e.value());
                        w.selected_partition = None;
                    },
                    for x in groups.iter() {
                        option { key: "{x.id}", value: "{x.id}", selected: x.id == g.id, "{x.name}" }
                    }
                }
            }
            div { class: "dt-wrap",
                table { class: "dt group-table",
                    thead {
                        tr {
                            th { "Partition" }
                            th { class: "num", "Committed offset" }
                            th { class: "num", "Log end offset" }
                            th { class: "num", "Lag" }
                        }
                    }
                    tbody {
                        for o in g.offsets.iter() {
                            {
                                let p = o.partition;
                                let is_selected = p == selected_partition;
                                rsx! {
                                    tr {
                                        key: "{o.partition}",
                                        class: if is_selected { "clickable selected" } else { "clickable" },
                                        "aria-selected": "{is_selected}",
                                        tabindex: "0",
                                        title: "Partition {p}: lag counts the records after its committed offset",
                                        onclick: move |_| {
                                            let mut w = ui.write();
                                            w.selected_partition = Some(p);
                                            w.input.partition = p;
                                        },
                                        onkeydown: move |e| {
                                            if e.key() == Key::Enter {
                                                let mut w = ui.write();
                                                w.selected_partition = Some(p);
                                                w.input.partition = p;
                                            }
                                        },
                                        td { class: "mono", "{o.partition}" }
                                        td { class: "num mono", "{o.committed}" }
                                        td { class: "num mono", "{o.log_end}" }
                                        td { class: "num mono", "{o.lag()}" }
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "total-lag",
                    span { "Total lag" }
                    span { class: "mono", "{thousands(g.total_lag())}" }
                }
            }
            div { class: "group-actions",
                button {
                    class: "btn",
                    title: "Review what resetting the committed offsets of {g.name} does",
                    onclick: {
                        let name = g.id.clone();
                        move |_| {
                            ui.write().reset = Some(ResetForm::new(&name, ResetScope::AllPartitions, ResetTarget::Earliest));
                        }
                    },
                    Ico { icon: Icon::RefreshCw, size: 16 }
                    "Reset offsets…"
                }
                span { class: "hint", "Reset requires review" }
            }
        }
    }
}

#[component]
pub fn TopicDetails(topic: Topic, group: Option<ConsumerGroup>) -> Element {
    let mut ui = use_context::<EventsUi>().topics;
    let partition = {
        let u = ui.read();
        u.selected_partition.unwrap_or(u.input.partition)
    };
    let replay = group.clone();
    rsx! {
        div { class: "inspector topic-details",
            div { class: "kv",
                div { class: "kv-row", span { "Name" } span { "{topic.name}" } }
                div { class: "kv-row", span { "Partitions" } span { "{topic.partitions}" } }
                div { class: "kv-row", span { "Retention" } span { "{topic.retention_days} days" } }
                div { class: "kv-row", span { "Durability" } span { "Enabled" } }
                div { class: "kv-row",
                    span { "Status" }
                    span { class: "status", span { class: "status-dot ok" } "Active" }
                }
                div { class: "kv-row", span { "Created" } span { "{topic.created}" } }
                div { class: "kv-row", span { "Replicated to" } span { "{topic.replicated_to} nodes" } }
                div { class: "kv-row",
                    span { "Dead letter" }
                    span { class: "muted",
                        if let Some(d) = topic.dead_letter.clone() { "{d}" } else { "Not configured" }
                    }
                }
            }
            div { class: "inspector-actions feed-actions",
                button {
                    class: "btn",
                    disabled: true,
                    title: "Editing a topic's retention and partitions is not designed yet",
                    Ico { icon: Icon::Settings, size: 16 }
                    "Edit topic"
                }
                button {
                    class: "btn",
                    disabled: replay.is_none(),
                    title: match replay.as_ref() {
                        Some(g) => format!("Review replaying partition {partition} for group {} from an earlier offset", g.name),
                        None => "This topic has no consumer group to replay for".to_string(),
                    },
                    onclick: move |_| {
                        if let Some(g) = replay.as_ref() {
                            let current = g
                                .offsets
                                .iter()
                                .find(|o| o.partition == partition)
                                .map_or(0, |o| o.committed.saturating_sub(10).max(o.log_start));
                            ui.write().reset = Some(ResetForm::new(
                                &g.id,
                                ResetScope::Partition(partition),
                                ResetTarget::Offset(current),
                            ));
                        }
                    },
                    Ico { icon: Icon::RotateCcw, size: 16 }
                    "Replay from offset…"
                }
            }
        }
    }
}
