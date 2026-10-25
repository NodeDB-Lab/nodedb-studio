//! The Analyze panels: the input, the tokens with their spans, the
//! processing trace, and the details of the bound analyzer.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::search::{SearchCollection, SearchMode, SearchText, VectorInput};
use crate::models::search_analyze::{Analysis, InputRole};
use crate::routes::Route;
use crate::routes::tabs::SearchTab;
use crate::state::search::{AnalyzeUi, SearchUi};

fn edit(mut a: Signal<AnalyzeUi>, f: impl FnOnce(&mut AnalyzeUi)) {
    f(&mut a.write());
}

#[component]
pub fn InputBody(
    text: String,
    role: InputRole,
    ready: bool,
    running: bool,
    on_run: EventHandler<()>,
) -> Element {
    let ui = use_context::<SearchUi>();
    let a = ui.analyze;
    rsx! {
        form {
            class: "srch-input-form",
            onsubmit: move |e| {
                e.prevent_default();
                if !running { on_run.call(()); }
            },
            div { class: "srch-text-row",
                label { r#for: "srch-an-text", "Input" }
                textarea {
                    id: "srch-an-text",
                    class: "textarea srch-an-text",
                    placeholder: "Text to analyze",
                    value: "{text}",
                    oninput: move |e| {
                        let v = e.value();
                        edit(a, |s| s.text = v);
                    },
                }
            }
            div { class: "srch-text-actions",
                select {
                    class: "select native",
                    "aria-label": "How the text is treated",
                    onchange: move |e| {
                        let v = e.value();
                        edit(a, |s| {
                            s.role = InputRole::ALL
                                .into_iter()
                                .find(|r| r.label() == v)
                                .unwrap_or_default();
                        });
                    },
                    for r in InputRole::ALL {
                        option { key: "{r.label()}", value: "{r.label()}", selected: r == role, "{r.label()}" }
                    }
                }
                div { class: "toolbar-spacer" }
                button {
                    class: "btn primary",
                    r#type: "submit",
                    disabled: !ready || running,
                    title: if ready { "Run the text through the bound analyzer. Nothing is saved." } else { "Enter the text to analyze" },
                    if running { "Analyzing…" } else { "Analyze" }
                }
            }
        }
    }
}

#[component]
pub fn TokensBody(state: AnalyzeUi, running: bool, stale: bool) -> Element {
    let ui = use_context::<SearchUi>();
    let a = ui.analyze;
    rsx! {
        div { class: "srch-tokens",
            if let Some(message) = state.error.clone() {
                div { class: "callout err srch-note", role: "alert",
                    Ico { icon: Icon::CircleAlert, size: 16 }
                    span {
                        "{message}"
                        if state.shown.is_some() { " The tokens below are from the last test that finished." }
                    }
                }
            }
            match state.shown.clone() {
                Some(done) => rsx! {
                    div { class: if running || stale { "dt-wrap srch-token-table dim" } else { "dt-wrap srch-token-table" },
                        table { class: "dt", "aria-label": "Analyzed tokens",
                            thead {
                                tr {
                                    th { "Token" }
                                    th { "Original text" }
                                    th { class: "num", "Start byte" }
                                    th { class: "num", "End byte" }
                                }
                            }
                            tbody {
                                for (i, t) in done.tokens.iter().enumerate() {
                                    {
                                        let on = state.token == Some(i);
                                        let key = format!("{i}-{}-{}", t.start, t.end);
                                        rsx! {
                                            tr {
                                                key: "{key}",
                                                class: if on { "clickable selected" } else { "clickable" },
                                                tabindex: "0",
                                                "aria-selected": "{on}",
                                                onclick: move |_| edit(a, |s| s.token = if on { None } else { Some(i) }),
                                                onkeydown: move |e| {
                                                    if e.key() == Key::Enter || e.key() == Key::Character(" ".into()) {
                                                        e.prevent_default();
                                                        edit(a, |s| s.token = if on { None } else { Some(i) });
                                                    }
                                                },
                                                td { class: "mono", "{t.token}" }
                                                td { "{t.original}" }
                                                td { class: "num", "{t.start}" }
                                                td { class: "num", "{t.end}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if done.tokens.is_empty() {
                        div { class: "srch-state", "The analyzer kept no token. Every word was removed." }
                    }
                    div { class: "srch-token-foot",
                        span {
                            if done.tokens.len() == 1 { "1 token" } else { "{done.tokens.len()} tokens" }
                        }
                        span { class: "srch-unit", "{done.unit.caption()}" }
                    }
                },
                None => rsx! {
                    if running {
                        div { class: "srch-state", role: "status", "Analyzing…" }
                    } else if state.error.is_none() {
                        div { class: "srch-state", "Enter text and press Analyze. A test never changes the index." }
                    }
                },
            }
        }
    }
}

#[component]
pub fn TraceBody(state: AnalyzeUi) -> Element {
    match state.shown {
        Some(done) => rsx! {
            div { class: "dt-wrap",
                table { class: "dt srch-trace", "aria-label": "Processing trace",
                    thead { tr { th { "Stage" } th { "Result" } } }
                    tbody {
                        for s in done.trace.iter() {
                            tr { key: "{s.stage}",
                                td { "{s.stage}" }
                                td { {s.result.clone().unwrap_or_else(|| "Not reported".to_string())} }
                            }
                        }
                    }
                }
            }
        },
        None => rsx! {
            div { class: "srch-state", "The stages the analyzer ran show here after a test." }
        },
    }
}

#[component]
pub fn DetailsBody(
    collection: SearchCollection,
    text: SearchText,
    field: String,
    shown: Option<Analysis>,
    raw: String,
) -> Element {
    let ui = use_context::<SearchUi>();
    let nav = use_navigator();
    let profile = shown.as_ref().map(|s| s.profile.clone());
    let analyzer = profile
        .as_ref()
        .map_or(text.analyzer.clone(), |p| p.name.clone());
    let revision = profile
        .as_ref()
        .map_or(text.binding_revision, |p| p.binding_revision);
    let stage = |v: Option<&Option<String>>| match v {
        Some(Some(s)) => s.clone(),
        Some(None) => "Not reported".to_string(),
        None => "—".to_string(),
    };
    let normalization = stage(profile.as_ref().map(|p| &p.normalization));
    let stop_words = stage(profile.as_ref().map(|p| &p.stop_words));
    let stemmer = stage(profile.as_ref().map(|p| &p.stemmer));
    let name = collection.name.clone();
    let index = text.index.clone();
    rsx! {
        div { class: "srch-details",
            div { class: "srch-details-scroll",
                h4 { class: "srch-heading", "Properties" }
                dl { class: "srch-props",
                    dt { "Collection" }
                    dd { "{collection.name}" }
                    dt { "Index" }
                    dd { "{text.index}" }
                    dt { "Field" }
                    dd { "{field}" }
                    dt { "Analyzer" }
                    dd { "{analyzer}" }
                    dt { "Binding revision" }
                    dd { "{revision}" }
                }
                h4 { class: "srch-heading", "Processing" }
                dl { class: "srch-props",
                    dt { "Normalization" }
                    dd { "{normalization}" }
                    dt { "Stop words" }
                    dd { "{stop_words}" }
                    dt { "Stemmer" }
                    dd { "{stemmer}" }
                }
                div { class: "srch-info",
                    Ico { icon: Icon::Info, size: 16 }
                    span { "This test uses the bound analyzer." }
                }
            }
            div { class: "srch-details-actions",
                button {
                    class: "btn",
                    disabled: raw.trim().is_empty(),
                    title: "Open Search with this text as the query. The raw text moves, not the tokens.",
                    onclick: {
                        let (name, field, raw) = (name.clone(), field.clone(), raw.clone());
                        move |_| {
                            let mode = {
                                let current = ui.results.peek().form.mode;
                                if current.uses_text() { current } else { SearchMode::Keyword }
                            };
                            ui.open_query(&name, mode, raw.trim(), VectorInput::None);
                            let mut r = ui.results;
                            r.write().form.text_field = field.clone();
                            nav.push(Route::Search { tab: SearchTab::Results });
                        }
                    },
                    Ico { icon: Icon::Search, size: 16 }
                    "Use input in Search"
                }
                button {
                    class: "btn",
                    title: "Select {index} on the Indexes tab",
                    onclick: {
                        let (name, index) = (name.clone(), index.clone());
                        move |_| {
                            ui.open_index(&name, &index);
                            nav.push(Route::Search { tab: SearchTab::Indexes });
                        }
                    },
                    Ico { icon: Icon::ExternalLink, size: 16 }
                    "Open index"
                }
            }
        }
    }
}
