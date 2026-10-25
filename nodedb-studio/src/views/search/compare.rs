//! Compare modes: the same query ranked by Keyword, Semantic, and Hybrid with
//! one source, one set of filters, and one limit. It reports overlap and rank
//! changes. It judges no relevance.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::AsyncView;
use crate::components::sheet::{Sheet, SheetFooter, SheetWidth};
use crate::models::search::{SearchMode, SearchRequest, VectorInput};
use crate::models::search_compare::{Comparison, compare};
use crate::services::backend::Backend;
use crate::services::error::StudioError;
use crate::state::search::SearchUi;
use crate::views::search::query_bar::edit;

/// The Keyword and Semantic requests that go with a Hybrid one: the same
/// inputs, filters, and limit, with the other mode's inputs left out.
fn derive(hybrid: &SearchRequest, mode: SearchMode) -> SearchRequest {
    let mut q = hybrid.clone();
    q.mode = mode;
    match mode {
        SearchMode::Keyword => {
            q.vector_field = None;
            q.vector = VectorInput::None;
        }
        SearchMode::Semantic => {
            q.query.clear();
            q.text_field = None;
        }
        SearchMode::Hybrid => {}
    }
    q
}

async fn run_three(
    backend: &Rc<dyn Backend>,
    hybrid: &SearchRequest,
) -> Result<Comparison, StudioError> {
    let keyword = backend
        .search_run(&derive(hybrid, SearchMode::Keyword))
        .await?;
    let semantic = backend
        .search_run(&derive(hybrid, SearchMode::Semantic))
        .await?;
    let fused = backend.search_run(hybrid).await?;
    Ok(compare(&keyword, &semantic, &fused))
}

#[component]
pub fn CompareSheet(collection_name: String) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let ui = use_context::<SearchUi>();
    let r = ui.results;

    let pending = use_memo(move || r.read().compare.as_ref().and_then(|c| c.pending.clone()));
    let run = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let _conn = conn();
            let pending = pending();
            async move {
                match pending {
                    Some((seq, hybrid)) => Some((seq, run_three(&backend, &hybrid).await)),
                    None => None,
                }
            }
        })
    };
    use_effect(move || {
        let Some(Some((seq, outcome))) = run.read().clone() else {
            return;
        };
        edit(r, |s| {
            let Some(c) = &mut s.compare else { return };
            if c.pending.as_ref().map(|p| p.0) != Some(seq) {
                return;
            }
            c.pending = None;
            match outcome {
                Ok(done) => {
                    c.shown = Some(done);
                    c.error = None;
                }
                Err(e) => c.error = Some(e.to_string()),
            }
        });
    });

    let c = r.read().compare.clone().unwrap_or_default();
    let running = c.pending.is_some();
    rsx! {
        Sheet {
            title: "Compare modes".to_string(),
            on_close: move |_| edit(r, |s| s.compare = None),
            width: SheetWidth::Medium,
            footer: rsx! {
                SheetFooter {
                    error: c.error.clone(),
                    hint: Some("Overlap and rank changes only. No relevance is judged.".to_string()),
                    button {
                        class: "btn",
                        onclick: move |_| edit(r, |s| s.compare = None),
                        "Close"
                    }
                }
            },
            AsyncView { loading: running && c.shown.is_none(), empty: false, error: None }
            if let Some(done) = c.shown.clone() {
                div { class: "srch-compare",
                    p { class: "srch-note-text",
                        "{collection_name} · each mode returns up to {done.limit}, with the same filters."
                    }
                    dl { class: "srch-props",
                        dt { "Keyword and Semantic share" }
                        dd { "{done.keyword_and_semantic} of {done.semantic_count}" }
                        dt { "In all three lists" }
                        dd { "{done.in_all_three}" }
                    }
                    div { class: "dt-wrap",
                        table { class: "dt", "aria-label": "Rank of each record in each mode",
                            thead {
                                tr {
                                    th { "Record" }
                                    th { class: "num", "Keyword" }
                                    th { class: "num", "Semantic" }
                                    th { class: "num", "Hybrid" }
                                    th { class: "num", "Hybrid vs Keyword" }
                                }
                            }
                            tbody {
                                for row in done.rows.iter() {
                                    tr { key: "{row.id}",
                                        td { title: "{row.title}", "{row.title} " span { class: "srch-id", "{row.id}" } }
                                        td { class: "num", {row.keyword.map_or("—".to_string(), |k| k.to_string())} }
                                        td { class: "num", {row.semantic.map_or("—".to_string(), |k| k.to_string())} }
                                        td { class: "num", {row.hybrid.map_or("—".to_string(), |k| k.to_string())} }
                                        td { class: "num",
                                            {match row.hybrid_moved_from_keyword() {
                                                Some(0) => "same".to_string(),
                                                Some(n) if n > 0 => format!("up {n}"),
                                                Some(n) => format!("down {}", -n),
                                                None => "—".to_string(),
                                            }}
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
