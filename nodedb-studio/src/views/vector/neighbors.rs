//! The Neighbors panel: the ranked result of the last search.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::vector::{Metric, NeighborResult};

#[component]
pub fn NeighborsBody(
    metric: Metric,
    result: Option<NeighborResult>,
    /// The result's query differs from the form.
    stale: bool,
    running: bool,
    error: Option<String>,
    selected: Option<String>,
    on_select: EventHandler<String>,
) -> Element {
    rsx! {
        p { class: "panel-subtitle vec-metric",
            "{metric.distance_note()}"
            if let Some(r) = result.as_ref() {
                span { class: "vec-context",
                    " · "
                    {query_text(r)}
                }
            }
        }
        if let Some(message) = error {
            div { class: "callout err vec-error", role: "alert",
                Ico { icon: Icon::CircleAlert, size: 16 }
                span { "{message}" }
            }
        }
        if running {
            div { class: "vec-state", role: "status", "Searching…" }
        } else if let Some(r) = result.as_ref() {
            if r.neighbors.is_empty() {
                div { class: "vec-state",
                    "No record matches. The query and its filters are kept."
                }
            } else {
                div { class: "dt-wrap vec-table",
                    table { class: "dt vec-neighbors", "aria-label": "Ranked neighbors",
                        thead {
                            tr {
                                th { class: "num rank", "Rank" }
                                th { "ID" }
                                th { class: "num", "aria-sort": "ascending",
                                    span { class: "vec-sorted",
                                        "Distance"
                                        Ico { icon: Icon::ArrowUp, size: 14 }
                                    }
                                }
                                th { "Title" }
                                th { "Preview" }
                            }
                        }
                        tbody {
                            for n in r.neighbors.iter() {
                                {
                                    let id = n.id.clone();
                                    let id_key = n.id.clone();
                                    let on = selected.as_deref() == Some(n.id.as_str());
                                    rsx! {
                                        tr {
                                            key: "{n.id}",
                                            class: if on { "clickable selected" } else { "clickable" },
                                            tabindex: "0",
                                            "aria-selected": "{on}",
                                            onclick: move |_| on_select.call(id.clone()),
                                            onkeydown: move |e| {
                                                if e.key() == Key::Enter || e.key() == Key::Character(" ".into()) {
                                                    e.prevent_default();
                                                    on_select.call(id_key.clone());
                                                }
                                            },
                                            td { class: "num rank", "{n.rank}" }
                                            td { class: "mono", "{n.id}" }
                                            td { class: "num mono", "{n.distance:.3}" }
                                            td { class: "vec-title", title: "{n.title}", "{n.title}" }
                                            td { class: "vec-preview", title: "{n.excerpt}", "{n.excerpt}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if stale {
                div { class: "vec-stale", role: "status",
                    "The query changed after this ran. Press Find similar to rank again."
                }
            }
        } else {
            div { class: "vec-state",
                "Choose a record or paste a vector, then press Find similar. Nothing runs until you do."
            }
        }
    }
}

/// What the result answered, in one phrase.
fn query_text(r: &NeighborResult) -> String {
    use crate::models::vector::QuerySource;
    let from = match &r.request.source {
        QuerySource::Record(id) => format!("record {id}"),
        QuerySource::Vector(v) => format!("pasted vector, {} values", v.len()),
    };
    let mut text = format!("from {from}");
    if r.request.exclude_source && matches!(r.request.source, QuerySource::Record(_)) {
        text.push_str(", source excluded");
    }
    for f in &r.request.filters {
        text.push_str(&format!(", {}", f.label()));
    }
    text
}
