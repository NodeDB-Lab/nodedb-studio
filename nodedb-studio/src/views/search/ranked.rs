//! The Ranked results panel: the records of the last search in rank order,
//! as a list or a table, each with the score its mode ranks by.
//!
//! The list and the table render the same records in the same order. A ranked
//! record is a view of a source record: its rank and score belong to this run.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::search::{Hit, SearchMode, SearchResult};
use crate::state::search::{ResultsUi, ResultsView, SearchUi};
use crate::views::search::query_bar::edit;

/// The list/table switch in the panel header.
#[component]
pub fn ViewToggle() -> Element {
    let ui = use_context::<SearchUi>();
    let r = ui.results;
    let view = r.read().view;
    rsx! {
        div { class: "seg srch-view", role: "group", "aria-label": "Result view",
            button {
                r#type: "button",
                class: if view == ResultsView::List { "active" } else { "" },
                title: "List",
                "aria-label": "List",
                "aria-pressed": "{view == ResultsView::List}",
                onclick: move |_| edit(r, |s| s.view = ResultsView::List),
                Ico { icon: Icon::List, size: 16 }
            }
            button {
                r#type: "button",
                class: if view == ResultsView::Table { "active" } else { "" },
                title: "Table",
                "aria-label": "Table",
                "aria-pressed": "{view == ResultsView::Table}",
                onclick: move |_| edit(r, |s| s.view = ResultsView::Table),
                Ico { icon: Icon::Table, size: 16 }
            }
        }
    }
}

#[component]
pub fn RankedBody(
    state: ResultsUi,
    stale: bool,
    selected: Option<String>,
    compare_blocked: Option<String>,
    on_cancel: EventHandler<()>,
    on_open_compare: EventHandler<()>,
) -> Element {
    let running = state.pending.is_some();
    let mode = state.form.mode;
    let idle_hint = match mode {
        SearchMode::Keyword => "Enter words, then press Search. Nothing runs until you do.",
        SearchMode::Semantic => {
            "Choose a vector input, then press Search. Nothing runs until you do."
        }
        SearchMode::Hybrid => {
            "Enter words and choose a vector input, then press Search. Nothing runs until you do."
        }
    };
    rsx! {
        div { class: "srch-ranked",
            if let Some(message) = state.error.clone() {
                div { class: "callout err srch-note", role: "alert",
                    Ico { icon: Icon::CircleAlert, size: 16 }
                    span {
                        "{message}"
                        if state.shown.is_some() {
                            " The records below are from the last search that finished."
                        }
                    }
                }
            }
            if state.cancelled {
                div { class: "callout info srch-note", role: "status",
                    Ico { icon: Icon::Info, size: 16 }
                    span {
                        if state.shown.is_some() {
                            "Canceled. The records below are from the last search that finished."
                        } else {
                            "Canceled. Nothing was ranked."
                        }
                    }
                }
            }
            if running {
                div { class: "srch-state", role: "status",
                    "Searching…"
                    button { class: "link-btn", r#type: "button", onclick: move |_| on_cancel.call(()), "Cancel" }
                }
            }
            match state.shown.clone() {
                Some(result) if result.hits.is_empty() => rsx! {
                    if !running {
                        div { class: "srch-state",
                            "No record matches. The query and its filters are kept."
                        }
                    }
                },
                Some(result) => rsx! {
                    div { class: if running || state.error.is_some() { "srch-body dim" } else { "srch-body" },
                        match state.view {
                            ResultsView::List => rsx! { HitList { result: result.clone(), selected: selected.clone() } },
                            ResultsView::Table => rsx! { HitTable { result: result.clone(), selected: selected.clone() } },
                        }
                    }
                    if stale && !running {
                        div { class: "srch-stale", role: "status",
                            "The query changed after this ran. Press Search to rank again."
                        }
                    }
                },
                None => rsx! {
                    if !running && state.error.is_none() {
                        div { class: "srch-state", "{idle_hint}" }
                    }
                },
            }
            div { class: "panel-footer srch-foot",
                button {
                    class: "btn",
                    disabled: true,
                    title: "An export names whether it holds the displayed rows, the returned result set, or a new query. That flow is not designed yet.",
                    Ico { icon: Icon::Download, size: 16 }
                    "Export results"
                }
                button {
                    class: "btn",
                    disabled: compare_blocked.is_some() || running,
                    title: compare_blocked.clone().unwrap_or_else(|| "Run this query in Keyword, Semantic, and Hybrid with the same source, filters, and limit, and compare the three rankings".to_string()),
                    onclick: move |_| on_open_compare.call(()),
                    Ico { icon: Icon::ChartColumn, size: 16 }
                    "Compare modes"
                }
                span { class: "srch-foot-note", "A rank belongs to this run, not to the record" }
            }
        }
    }
}

fn select(r: Signal<ResultsUi>, id: &str) {
    let id = id.to_string();
    edit(r, |s| s.selected = Some(id));
}

#[component]
fn HitList(result: SearchResult, selected: Option<String>) -> Element {
    let ui = use_context::<SearchUi>();
    let r = ui.results;
    rsx! {
        ul { class: "srch-list", role: "listbox", "aria-label": "Ranked results",
            for h in result.hits.iter() {
                HitRow { key: "{h.id}", hit: h.clone(), on: selected.as_deref() == Some(h.id.as_str()), r }
            }
        }
    }
}

#[component]
fn HitRow(hit: Hit, on: bool, r: Signal<ResultsUi>) -> Element {
    let (id, id_key) = (hit.id.clone(), hit.id.clone());
    rsx! {
        li {
            class: if on { "srch-hit selected" } else { "srch-hit" },
            role: "option",
            tabindex: "0",
            "aria-selected": "{on}",
            onclick: move |_| select(r, &id),
            onkeydown: move |e| {
                if e.key() == Key::Enter || e.key() == Key::Character(" ".into()) {
                    e.prevent_default();
                    select(r, &id_key);
                }
            },
            span { class: "srch-rank", "{hit.rank}" }
            div { class: "srch-main",
                span { class: "srch-title", title: "{hit.title}", "{hit.title}" }
                span { class: "srch-id", "ID: {hit.id}" }
            }
            p { class: "srch-excerpt",
                for (i, (piece, matched)) in hit.excerpt.runs().into_iter().enumerate() {
                    if matched {
                        mark { key: "{i}", "{piece}" }
                    } else {
                        span { key: "{i}", "{piece}" }
                    }
                }
            }
            div { class: "srch-score",
                span { class: "srch-score-label", "{hit.score.caption()}" }
                span { class: "srch-score-value", "{hit.score.text()}" }
            }
        }
    }
}

#[component]
fn HitTable(result: SearchResult, selected: Option<String>) -> Element {
    let ui = use_context::<SearchUi>();
    let r = ui.results;
    let mode = result.request.mode;
    let caption = result.hits.first().map_or("Score", |h| h.score.caption());
    rsx! {
        div { class: "dt-wrap srch-table",
            table { class: "dt", "aria-label": "Ranked results",
                thead {
                    tr {
                        th { class: "num", "Rank" }
                        th { "ID" }
                        th { "Title" }
                        th { class: "num", "{caption}" }
                        if mode == SearchMode::Hybrid {
                            th { class: "num", "Keyword rank" }
                            th { class: "num", "Vector rank" }
                        }
                    }
                }
                tbody {
                    for h in result.hits.iter() {
                        {
                            let on = selected.as_deref() == Some(h.id.as_str());
                            let (id, id_key) = (h.id.clone(), h.id.clone());
                            rsx! {
                                tr {
                                    key: "{h.id}",
                                    class: if on { "clickable selected" } else { "clickable" },
                                    tabindex: "0",
                                    "aria-selected": "{on}",
                                    onclick: move |_| select(r, &id),
                                    onkeydown: move |e| {
                                        if e.key() == Key::Enter || e.key() == Key::Character(" ".into()) {
                                            e.prevent_default();
                                            select(r, &id_key);
                                        }
                                    },
                                    td { class: "num", "{h.rank}" }
                                    td { class: "mono", "{h.id}" }
                                    td { title: "{h.title}", "{h.title}" }
                                    td { class: "num mono", "{h.score.text()}" }
                                    if mode == SearchMode::Hybrid {
                                        td { class: "num", {h.keyword.map_or("—".to_string(), |l| l.rank.to_string())} }
                                        td { class: "num", {h.vector.map_or("—".to_string(), |l| l.rank.to_string())} }
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
