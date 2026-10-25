//! The Ranking tab: where the selected record sits in each ranking, and what
//! each ranking contributed. Only values the run reported appear. A ranking
//! that does not hold the record shows as absent, never as a zero.

use dioxus::prelude::*;

use crate::models::search::{Hit, Leg, SearchMode, SearchResult};

fn contribution(l: Leg) -> String {
    l.contribution
        .map_or("—".to_string(), |c| format!("{c:.6}"))
}

#[component]
pub fn RankingBody(result: SearchResult, hit: Hit) -> Element {
    let req = &result.request;
    let metric = result.metric.map_or("", |m| m.label());
    let filters = if req.filters.is_empty() {
        "None".to_string()
    } else {
        req.filters
            .iter()
            .map(|f| f.label())
            .collect::<Vec<_>>()
            .join(", ")
    };
    match req.mode {
        SearchMode::Hybrid => {
            let fused = hit.score.text();
            rsx! {
                h4 { class: "srch-heading", "Reciprocal Rank Fusion" }
                table { class: "dt srch-rrf", "aria-label": "Contribution of each ranking",
                    thead { tr { th { "Source" } th { class: "num", "Rank" } th { class: "num", "Contribution" } } }
                    tbody {
                        tr {
                            td { "Keyword" }
                            td { class: "num", {hit.keyword.map_or("Not ranked".to_string(), |l| l.rank.to_string())} }
                            td { class: "num mono", {hit.keyword.map_or("—".to_string(), contribution)} }
                        }
                        tr {
                            td { "Vector" }
                            td { class: "num", {hit.vector.map_or("Not ranked".to_string(), |l| l.rank.to_string())} }
                            td { class: "num mono", {hit.vector.map_or("—".to_string(), contribution)} }
                        }
                        tr { class: "total",
                            td { colspan: "2", "Fused score" }
                            td { class: "num mono", "{fused}" }
                        }
                    }
                }
                dl { class: "srch-props",
                    dt { "Fusion" }
                    dd { "RRF" }
                    dt { "Smoothing k" }
                    dd { "{req.fusion.k}" }
                    dt { "Source weights" }
                    dd { "{req.fusion.weights_label()}" }
                    dt { "Text field" }
                    dd { {req.text_field.clone().unwrap_or_default()} }
                    dt { "Vector field" }
                    dd { {req.vector_field.clone().unwrap_or_default()} }
                    if let Some(l) = hit.vector {
                        dt { "Vector distance" }
                        dd { "{l.raw:.3} · {metric}" }
                    }
                    if let Some(l) = hit.keyword {
                        dt { "BM25 score" }
                        dd { "{l.raw:.3}" }
                    }
                    dt { "Filters" }
                    dd { "{filters}" }
                    dt { "Candidates" }
                    dd {
                        "{result.keyword_candidates.unwrap_or(0)} keyword · {result.vector_candidates.unwrap_or(0)} vector"
                    }
                }
                p { class: "srch-note-text", "Score is rank-based. It is not a probability, and raw BM25 and distance never enter it." }
            }
        }
        SearchMode::Keyword => rsx! {
            h4 { class: "srch-heading", "BM25" }
            dl { class: "srch-props",
                dt { "Rank" }
                dd { "{hit.rank}" }
                dt { "BM25 score" }
                dd { "{hit.score.text()}" }
                dt { "Text field" }
                dd { {req.text_field.clone().unwrap_or_default()} }
                dt { "Matching" }
                dd { "Any query word" }
                dt { "Filters" }
                dd { "{filters}" }
                dt { "Candidates" }
                dd { "{result.keyword_candidates.unwrap_or(0)}" }
            }
            p { class: "srch-note-text", "Larger is better. BM25 is not a probability, and scores only compare within this run." }
        },
        SearchMode::Semantic => rsx! {
            h4 { class: "srch-heading", "Vector distance" }
            dl { class: "srch-props",
                dt { "Rank" }
                dd { "{hit.rank}" }
                dt { "Distance" }
                dd { "{hit.score.text()} · {metric}" }
                dt { "Vector field" }
                dd { {req.vector_field.clone().unwrap_or_default()} }
                dt { "Query vector" }
                dd { "{req.vector.label()}" }
                dt { "Filters" }
                dd { "{filters}" }
            }
            p { class: "srch-note-text", "Smaller is closer. A distance is not a similarity or a probability." }
        },
    }
}
