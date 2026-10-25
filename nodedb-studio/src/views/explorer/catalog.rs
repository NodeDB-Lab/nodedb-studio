//! The Catalog panel: every collection of the database, with its storage
//! profile and attached capabilities as badges.

use dioxus::prelude::*;

use crate::components::async_view::AsyncView;
use crate::components::icon::{Ico, Icon};
use crate::models::collection::Collection;
use crate::services::async_state::AsyncState;
use crate::state::explorer::{RecordView, Selected, select_collection};

/// Collections whose name contains `filter`, ignoring case.
pub fn matching<'a>(all: &'a [Collection], filter: &str) -> Vec<&'a Collection> {
    let needle = filter.trim().to_lowercase();
    all.iter()
        .filter(|c| needle.is_empty() || c.name.to_lowercase().contains(&needle))
        .collect()
}

#[component]
pub fn CatalogBody(
    catalog: AsyncState<Vec<Collection>>,
    selected: Option<String>,
    on_retry: EventHandler<()>,
) -> Element {
    let mut filter = use_signal(String::new);
    let mut sel = use_context::<Signal<Option<Selected>>>();
    let mut view = use_context::<Signal<RecordView>>();
    let list = catalog.loaded().cloned();
    rsx! {
        div { class: "toolbar nowrap catalog-search",
            div { class: "search-field grow",
                Ico { icon: Icon::Search, size: 16 }
                input {
                    class: "input",
                    placeholder: "Search collections…",
                    "aria-label": "Search collections",
                    value: "{filter}",
                    oninput: move |e| filter.set(e.value()),
                }
            }
        }
        AsyncView {
            loading: catalog.is_loading(),
            empty: catalog.is_empty(),
            error: catalog.error_message(),
            retriable: catalog.is_retriable(),
            on_retry: move |_| on_retry.call(()),
            empty_message: "This database has no collections.",
        }
        if let Some(all) = list {
            div { class: "list catalog-list", role: "listbox", "aria-label": "Collections",
                for c in matching(&all, &filter.read()) {
                    {
                        let name = c.name.clone();
                        let is_selected = selected.as_deref() == Some(c.name.as_str());
                        rsx! {
                            button {
                                key: "{c.name}",
                                class: if is_selected { "list-item catalog-item selected" } else { "list-item catalog-item" },
                                role: "option",
                                "aria-selected": "{is_selected}",
                                onclick: move |_| select_collection(&mut sel, &mut view, &name),
                                Ico { icon: c.profile.icon(), size: 18 }
                                span { class: "list-item-main", "{c.name}" }
                                span { class: "catalog-badges",
                                    // Two capabilities say enough, as the design shows: the
                                    // profile stays in the header.
                                    if c.attached.len() < 2 {
                                        span { class: "badge {c.profile.tone()}", "{c.profile.badge()}" }
                                    }
                                    for a in c.attached.iter() {
                                        span { key: "{a.badge()}", class: "badge {a.tone()}", "{a.badge()}" }
                                    }
                                }
                            }
                        }
                    }
                }
                if matching(&all, &filter.read()).is_empty() && !all.is_empty() {
                    div { class: "async-empty", "No collection matches “{filter}”." }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockConnectionService;
    use crate::services::explorer_data::ExplorerData;

    #[tokio::test]
    async fn the_search_matches_a_name_part_ignoring_case() {
        let all = MockConnectionService::ready()
            .catalog()
            .await
            .expect("catalog");
        assert_eq!(matching(&all, "").len(), all.len());
        assert_eq!(matching(&all, "ARTI").len(), 1);
        assert_eq!(
            matching(&all, "s").len(),
            all.iter().filter(|c| c.name.contains('s')).count()
        );
        assert!(matching(&all, "zzz").is_empty());
    }
}
