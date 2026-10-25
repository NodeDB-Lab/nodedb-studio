//! Search: the page tabs and the setup every tab reads.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::tabs::{RouteTabs, tab_items};
use crate::models::search::{SearchCollection, SearchSetup};
use crate::routes::Route;
use crate::routes::tabs::SearchTab;
use crate::services::backend::Backend;
use crate::views::search::analyze::AnalyzeTab;
use crate::views::search::indexes::IndexesTab;
use crate::views::search::results::ResultsTab;

/// The collection the user chose while it is listed, else the first.
pub fn resolve<'a>(setup: &'a SearchSetup, choice: Option<&str>) -> Option<&'a SearchCollection> {
    choice
        .and_then(|c| setup.collection(c))
        .or_else(|| setup.collections.first())
}

#[component]
pub fn SearchPage(tab: SearchTab) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let nav = use_navigator();

    let mut setup_read = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let _conn = conn();
            async move { backend.search_setup().await }
        })
    };
    let state = resource_state(&setup_read);
    let setup: Option<SearchSetup> = state.loaded().cloned();

    rsx! {
        RouteTabs {
            items: tab_items(SearchTab::ALL, |t| t.label(), |tab| Route::Search { tab }),
            current: Route::Search { tab },
        }
        div { class: "page-body srch-page",
            AsyncView {
                loading: state.is_loading(),
                empty: false,
                error: state.error_message(),
                retriable: state.is_retriable(),
                on_retry: move |_| setup_read.restart(),
            }
            if state.is_empty() {
                div { class: "empty-state",
                    Ico { icon: Icon::Search, size: 24 }
                    h2 { "No searchable collection" }
                    p {
                        "No collection has a full-text or vector index. Add one in the Designer, and the collection appears here."
                    }
                    div { class: "actions",
                        button {
                            class: "btn",
                            onclick: move |_| { nav.push(Route::Designer {}); },
                            Ico { icon: Icon::Pencil, size: 16 }
                            "Open Designer"
                        }
                    }
                }
            }
            if let Some(setup) = setup {
                match tab {
                    SearchTab::Results => rsx! { ResultsTab { setup } },
                    SearchTab::Analyze => rsx! { AnalyzeTab { setup } },
                    SearchTab::Indexes => rsx! { IndexesTab { setup } },
                }
            }
        }
    }
}
