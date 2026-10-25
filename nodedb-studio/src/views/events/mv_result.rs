//! A materialized view's result rows, one page at a time.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::pager::Pager;
use crate::models::matviews::MaterializedView;
use crate::services::backend::Backend;
use crate::state::events::EventsUi;

#[component]
pub fn ResultRows(view: MaterializedView, page: usize, page_size: usize) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut ui = use_context::<EventsUi>().views;
    let epoch = use_memo(move || ui.read().epoch);
    let key = (view.id.clone(), page, page_size);
    let mut rows = {
        let backend = backend.clone();
        use_resource(use_reactive!(|key| {
            let backend = backend.clone();
            let (_conn, _epoch) = (conn(), epoch());
            async move {
                let (id, page, size) = key;
                backend.view_rows(&id, page, size).await
            }
        }))
    };
    let state = resource_state(&rows);
    rsx! {
        AsyncView {
            loading: state.is_loading(),
            empty: state.is_empty(),
            error: state.error_message(),
            retriable: state.is_retriable(),
            on_retry: move |_| rows.restart(),
            empty_message: "This view holds no rows yet.",
        }
        if let Some(r) = state.loaded().cloned() {
            div { class: "dt-wrap",
                table { class: "dt",
                    thead {
                        tr {
                            for c in r.columns.iter() {
                                th { key: "{c.name}", class: if c.numeric { "num" } else { "" }, "{c.name}" }
                            }
                        }
                    }
                    tbody {
                        for (key, cells) in r.rows.iter() {
                            tr { key: "{key}",
                                for (i, cell) in cells.iter().enumerate() {
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
            div { class: "dt-footer",
                span { "{r.range_label()}" }
                div { class: "toolbar-spacer" }
                Pager {
                    page: r.page,
                    pages: r.pages(),
                    on_page: move |p: usize| ui.write().page = p,
                }
            }
        }
    }
}
