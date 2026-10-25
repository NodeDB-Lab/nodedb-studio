//! The Result details panel: the selected ranked record as its source
//! record, its ranking, and the statement that equals the search.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::components::tabs::TabBar;
use crate::models::search::{Hit, SearchCollection, SearchMode, SearchResult};
use crate::models::search_analyze::InputRole;
use crate::routes::Route;
use crate::routes::tabs::SearchTab;
use crate::state::explorer::{RecordView, Selected, select_record};
use crate::state::search::{DetailsTab, ResultsUi, SearchUi};
use crate::views::search::hit_ranking::RankingBody;
use crate::views::search::query_bar::edit;

#[component]
pub fn DetailsBody(
    collection: SearchCollection,
    state: ResultsUi,
    selected: Option<String>,
) -> Element {
    let ui = use_context::<SearchUi>();
    let r = ui.results;
    let nav = use_navigator();
    let mut explorer = use_context::<Signal<Option<Selected>>>();
    let mut view = use_context::<Signal<RecordView>>();

    let hit = selected
        .as_deref()
        .and_then(|id| state.shown.as_ref().and_then(|s| s.hit(id)))
        .cloned();
    let (Some(hit), Some(result)) = (hit, state.shown.clone()) else {
        return rsx! {
            div { class: "srch-state", "Select a ranked record to read its source record and its ranking." }
        };
    };
    let tab = state.details;
    let active = DetailsTab::ALL.iter().position(|t| *t == tab).unwrap_or(0);
    let mode = result.request.mode;
    let query = result.request.query.clone();
    let has_words = mode.uses_text() && !query.is_empty();
    let text_index = collection.text.as_ref().map(|t| t.index.clone());
    let field = result.request.text_field.clone();
    let name = collection.name.clone();

    rsx! {
        div { class: "srch-details",
            div { class: "srch-details-scroll",
                h3 { class: "srch-details-id", "{hit.id}" }
                TabBar {
                    class: "sub",
                    items: DetailsTab::ALL.iter().map(|t| t.label()).collect::<Vec<_>>(),
                    active,
                    on_select: move |i: usize| {
                        if let Some(t) = DetailsTab::ALL.get(i) { edit(r, |s| s.details = *t); }
                    },
                }
                match tab {
                    DetailsTab::Record => rsx! { RecordBody { hit: hit.clone(), id_field: collection.id_field.clone() } },
                    DetailsTab::Ranking => rsx! { RankingBody { result: result.clone(), hit: hit.clone() } },
                    DetailsTab::Sql => rsx! { SqlBody { result: result.clone() } },
                }
            }
            div { class: "srch-details-actions",
                button {
                    class: "btn",
                    title: "Open the source record in Explorer",
                    onclick: {
                        let (name, id) = (name.clone(), hit.id.clone());
                        move |_| {
                            select_record(&mut explorer, &mut view, &name, &id);
                            nav.push(Route::Explorer {});
                        }
                    },
                    Ico { icon: Icon::FileText, size: 16 }
                    "Open in Explorer"
                }
                button {
                    class: "btn",
                    disabled: !has_words,
                    title: if has_words { "Test how the index analyzes this query" } else { "A semantic search has no words to analyze" },
                    onclick: {
                        let name = name.clone();
                        move |_| {
                            ui.open_analysis(&name, text_index.as_deref(), field.as_deref(), &query, InputRole::Query);
                            nav.push(Route::Search { tab: SearchTab::Analyze });
                        }
                    },
                    Ico { icon: Icon::ChartColumn, size: 16 }
                    "Analyze query"
                }
            }
        }
    }
}

#[component]
fn RecordBody(hit: Hit, id_field: String) -> Element {
    rsx! {
        h4 { class: "srch-heading", "Source record" }
        dl { class: "srch-fields",
            dt { "{id_field}" }
            dd { class: "mono", "{hit.id}" }
            dt { "title" }
            dd { "{hit.title}" }
            dt { "language" }
            dd { {hit.language.clone().unwrap_or_else(|| "Missing".to_string())} }
            dt { "body" }
            dd { class: "srch-body-text", "{hit.body}" }
        }
        p { class: "srch-note-text", "Read from the collection. Edit the record in Explorer." }
    }
}

#[component]
fn SqlBody(result: SearchResult) -> Element {
    let mode = result.request.mode;
    let params = match (mode, result.vector_dimensions) {
        (SearchMode::Keyword, _) | (_, None) => None,
        (_, Some(n)) => Some(format!(
            "$query_vec holds {n} values ({}).",
            result.request.vector.label()
        )),
    };
    rsx! {
        h4 { class: "srch-heading", "Statement" }
        pre { class: "code-block srch-sql", "{result.sql.display}" }
        if let Some(p) = params {
            p { class: "srch-note-text", "{p}" }
        }
        if let Some(c) = result.sql.caveat.clone() {
            div { class: "callout warn srch-caveat",
                Ico { icon: Icon::TriangleAlert, size: 16 }
                span { "{c}" }
            }
        }
        p { class: "srch-note-text", "Open in Query opens this statement with the vector written in. It does not run." }
    }
}
