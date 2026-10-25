//! Preview rebuild: the plan a rebuild would follow. Creating the job is a
//! separate, confirmed step. Closing the sheet creates nothing.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::sheet::{Sheet, SheetFooter, SheetWidth};
use crate::models::search_index::RebuildPlan;
use crate::services::backend::Backend;
use crate::state::search::{IndexesUi, SearchUi};
use crate::views::search::indexes::edit;

/// Create the job the plan describes and show its run.
async fn create(mut i: Signal<IndexesUi>, backend: Rc<dyn Backend>, plan: RebuildPlan) {
    i.write().plan_busy = true;
    let out = backend
        .search_start_rebuild(&plan.index, plan.target_revision)
        .await;
    let mut s = i.write();
    s.plan_busy = false;
    match out {
        Ok(run) => {
            s.epoch += 1;
            s.plan = None;
            s.plan_error = None;
            s.selected = Some(plan.index.clone());
            s.history_index = None;
            s.run = Some(run.id);
        }
        Err(e) => s.plan_error = Some(e.to_string()),
    }
}

#[component]
pub fn RebuildSheet(index: String, error: Option<String>) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let ui = use_context::<SearchUi>();
    let i = ui.indexes;
    let busy = i.read().plan_busy;
    let mut read = {
        let backend = backend.clone();
        let index = index.clone();
        use_resource(use_reactive!(|index| {
            let backend = backend.clone();
            let _conn = conn();
            async move { backend.search_rebuild_plan(&index).await }
        }))
    };
    let state = resource_state(&read);
    let plan = state.loaded().cloned();
    let not_reported = |v: Option<String>| v.unwrap_or_else(|| "Not reported".to_string());
    rsx! {
        Sheet {
            title: format!("Rebuild {index}"),
            on_close: move |_| edit(i, |s| { s.plan = None; s.plan_error = None; }),
            width: SheetWidth::Narrow,
            footer: rsx! {
                SheetFooter {
                    error: error.clone(),
                    hint: Some("Nothing is created until you confirm.".to_string()),
                    button {
                        class: "btn",
                        onclick: move |_| edit(i, |s| { s.plan = None; s.plan_error = None; }),
                        "Cancel"
                    }
                    button {
                        class: "btn primary",
                        disabled: plan.is_none() || busy,
                        onclick: {
                            let (backend, plan) = (backend.clone(), plan.clone());
                            move |_| {
                                let Some(plan) = plan.clone() else { return };
                                let backend = backend.clone();
                                spawn_forever(create(i, backend, plan));
                            }
                        },
                        if busy { "Creating…" } else { "Create rebuild job" }
                    }
                }
            },
            AsyncView {
                loading: state.is_loading(),
                empty: false,
                error: state.error_message(),
                retriable: state.is_retriable(),
                on_retry: move |_| read.restart(),
            }
            if let Some(p) = state.loaded().cloned() {
                p { class: "srch-note-text",
                    "A rebuild reads the collection again and builds the index from it. The plan states what the server reports about it."
                }
                dl { class: "srch-props roomy",
                    dt { "Index" }
                    dd { class: "mono", "{p.index}" }
                    dt { "Collection" }
                    dd { "{p.collection}" }
                    dt { "Source snapshot" }
                    dd { "{p.source_snapshot}" }
                    dt { "Target revision" }
                    dd { "{p.target_revision}" }
                    dt { "Resource estimate" }
                    dd { {not_reported(p.resource_estimate.clone())} }
                    dt { "Availability" }
                    dd { {not_reported(p.availability.clone())} }
                    dt { "Publication" }
                    dd { {not_reported(p.publication.clone())} }
                }
            }
        }
    }
}
