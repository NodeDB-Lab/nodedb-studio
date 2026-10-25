//! The Results panel: the rows of the last run, ordered by the measure the
//! query declared. Selecting a row selects its feature on the map.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::format::thousands;
use crate::models::spatial::FeatureRef;
use crate::models::spatial_analyze::AnalysisResult;
use crate::state::spatial::SpatialUi;

#[component]
pub fn ResultsBody(
    key_id: String,
    result: Option<AnalysisResult>,
    running: bool,
    error: Option<String>,
    stale: bool,
    selected: Option<String>,
) -> Element {
    let ui = use_context::<SpatialUi>();
    let mut analyze = ui.analyze;
    let mut map_ui = ui.map;

    // Move the selected row by `delta`, and keep the map's selection in step.
    let select = {
        let (key_id, result) = (key_id.clone(), result.clone());
        move |id: String| {
            analyze.write().draft_mut(&key_id).selected = Some(id.clone());
            if let Some(r) = &result {
                map_ui.write().select(Some(FeatureRef {
                    source: r.request.source.clone(),
                    id,
                }));
            }
        }
    };

    rsx! {
        if running {
            div { class: "sp-note", role: "status", "Running the analysis…" }
        }
        if let Some(e) = error.clone() {
            div { class: "callout err sp-result-error", role: "alert",
                Ico { icon: Icon::CircleAlert, size: 16 }
                span {
                    "{e}"
                    if result.is_some() {
                        " The rows below are from the last run."
                    }
                }
            }
        }
        if let Some(r) = result.clone() {
            if let Some(note) = r.measure_note.clone() {
                div { class: "sp-measure-note", "{note}" }
            }
            if r.rows.is_empty() {
                div { class: "sp-empty",
                    "No feature matches. The draft stays as it is, so you can change it and run again."
                }
            } else {
                div { class: "dt-wrap sp-results",
                    table { class: "dt sp-table",
                        thead {
                            tr {
                                for c in r.columns.iter() {
                                    th { key: "{c.name}", class: if c.numeric { "num" } else { "" }, "{c.name}" }
                                }
                            }
                        }
                        tbody {
                            for (n , row) in r.rows.iter().enumerate() {
                                {
                                    let is_selected = selected.as_deref() == Some(row.id.as_str());
                                    let id = row.id.clone();
                                    let neighbours: (Option<String>, Option<String>) = (
                                        n.checked_sub(1).and_then(|i| r.rows.get(i)).map(|x| x.id.clone()),
                                        r.rows.get(n + 1).map(|x| x.id.clone()),
                                    );
                                    rsx! {
                                        tr {
                                            key: "{row.id}",
                                            class: if is_selected { "clickable selected" } else { "clickable" },
                                            tabindex: "0",
                                            "aria-selected": "{is_selected}",
                                            onclick: {
                                                let (id, mut select) = (id.clone(), select.clone());
                                                move |_| select(id.clone())
                                            },
                                            onkeydown: {
                                                let (id, mut select) = (id.clone(), select.clone());
                                                let (prev, next) = neighbours.clone();
                                                move |e| match e.key() {
                                                    Key::Enter => select(id.clone()),
                                                    Key::Character(c) if c == " " => {
                                                        e.prevent_default();
                                                        select(id.clone());
                                                    }
                                                    Key::ArrowDown => {
                                                        if let Some(n) = next.clone() {
                                                            e.prevent_default();
                                                            select(n);
                                                        }
                                                    }
                                                    Key::ArrowUp => {
                                                        if let Some(p) = prev.clone() {
                                                            e.prevent_default();
                                                            select(p);
                                                        }
                                                    }
                                                    _ => {}
                                                }
                                            },
                                            for (i , cell) in row.cells.iter().enumerate() {
                                                td {
                                                    key: "{i}",
                                                    class: if r.columns.get(i).is_some_and(|c| c.numeric) { "num" } else { "" },
                                                    "{cell}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if r.limit_reached() {
                    div { class: "sp-note warn", role: "status",
                        "The limit cut the rows: {thousands(r.rows.len() as u64)} of {thousands(r.matching)} matches. Raise Limit to read more."
                    }
                }
            }
            if stale {
                div { class: "sp-note", role: "status",
                    "The form changed after this run. Press Run to read the rows it describes."
                }
            }
        } else if !running && error.is_none() {
            div { class: "sp-empty", "Run the analysis to read its rows." }
        }
    }
}
