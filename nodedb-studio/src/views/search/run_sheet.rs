//! A build run's details, by its stable id.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::sheet::{Sheet, SheetFooter, SheetWidth};
use crate::models::format::{date_second, duration, thousands};
use crate::models::search_index::RunState;
use crate::services::backend::Backend;
use crate::state::search::SearchUi;
use crate::views::search::index_history::run_dot;
use crate::views::search::indexes::edit;

#[component]
pub fn RunSheet(id: String) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let ui = use_context::<SearchUi>();
    let i = ui.indexes;
    let epoch = i.read().epoch;
    let mut read = {
        let id = id.clone();
        use_resource(use_reactive!(|id, epoch| {
            let backend = backend.clone();
            let _conn = conn();
            let _epoch = epoch;
            async move { backend.search_build_run(&id).await }
        }))
    };
    let state = resource_state(&read);
    let run = state.loaded().cloned();
    rsx! {
        Sheet {
            title: format!("Run {id}"),
            on_close: move |_| edit(i, |s| s.run = None),
            width: SheetWidth::Narrow,
            footer: rsx! {
                SheetFooter { error: None, hint: Some("Runs are read-only. Preview a rebuild to start another.".to_string()),
                    button { class: "btn", onclick: move |_| edit(i, |s| s.run = None), "Close" }
                }
            },
            AsyncView {
                loading: state.is_loading(),
                empty: false,
                error: state.error_message(),
                retriable: state.is_retriable(),
                on_retry: move |_| read.restart(),
            }
            if let Some(r) = run {
                if let RunState::Error { reason } = &r.state {
                    div { class: "callout err srch-note", role: "alert",
                        Ico { icon: Icon::CircleAlert, size: 16 }
                        span { "The build ended with an error: {reason}." }
                    }
                }
                dl { class: "srch-props roomy",
                    dt { "Run" }
                    dd { class: "mono", "{r.id}" }
                    dt { "Index" }
                    dd { class: "mono", "{r.index}" }
                    dt { "State" }
                    dd {
                        span { class: "status",
                            span { class: "{run_dot(&r.state)}" }
                            "{r.state.label()}"
                        }
                    }
                    dt { "Requested (UTC)" }
                    dd { {date_second(r.requested_at, 0)} }
                    dt { "Finished (UTC)" }
                    dd { {r.finished_at.map_or("Not finished".to_string(), |t| date_second(t, 0))} }
                    dt { "Duration" }
                    dd {
                        {r.finished_at.map_or("Not reported".to_string(), |t| duration((t - r.requested_at).max(0) as u64))}
                    }
                    dt { "Requested by" }
                    dd { "{r.requested_by}" }
                    dt { "Target revision" }
                    dd { "{r.target_revision}" }
                    dt { "Documents processed" }
                    dd { {r.documents.text(thousands)} }
                }
            }
        }
    }
}
