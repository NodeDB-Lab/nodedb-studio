//! The Collection picker: the database's collections, searchable, with the
//! timestamp field each offers. A collection that cannot be explored over
//! time stays listed with its reason: the picker never converts a storage
//! profile to make one eligible.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::timeseries::TimeSource;
use crate::routes::Route;
use crate::state::explorer::Selected;

/// `sold_at · TIME_KEY`, or the timestamp fields when none is the key.
pub fn time_hint(s: &TimeSource) -> String {
    match s.unsupported {
        Some(why) => why.reason().to_string(),
        None => s
            .time_fields
            .iter()
            .map(|t| {
                if t.time_key {
                    format!("{} · TIME_KEY", t.name)
                } else {
                    t.name.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(", "),
    }
}

/// Sources whose name contains `needle`, case-insensitively. Collections
/// that can be explored come first, in catalog order.
pub fn matching<'a>(sources: &'a [TimeSource], needle: &str) -> Vec<&'a TimeSource> {
    let n = needle.trim().to_lowercase();
    let mut found: Vec<&TimeSource> = sources
        .iter()
        .filter(|s| s.collection.to_lowercase().contains(&n))
        .collect();
    found.sort_by_key(|s| !s.eligible());
    found
}

#[component]
pub fn CollectionPicker(
    sources: Vec<TimeSource>,
    current: String,
    open: bool,
    on_toggle: EventHandler<bool>,
    on_pick: EventHandler<String>,
) -> Element {
    let mut filter = use_signal(String::new);
    let mut selected = use_context::<Signal<Option<Selected>>>();
    let nav = use_navigator();
    let found = matching(&sources, &filter.read());
    let total = sources.len();

    rsx! {
        div { class: "anchor ts-picker",
            button {
                class: "select",
                id: "ts-collection",
                "aria-haspopup": "listbox",
                "aria-expanded": "{open}",
                onclick: move |_| on_toggle.call(!open),
                span { "{current}" }
                Ico { icon: Icon::ChevronDown, size: 16 }
            }
            if open {
                div { class: "ts-scrim", onclick: move |_| on_toggle.call(false) }
                div {
                    class: "popover align-left ts-picker-pop",
                    role: "listbox",
                    "aria-label": "Collections",
                    onkeydown: move |e| {
                        if e.key() == Key::Escape {
                            e.stop_propagation();
                            on_toggle.call(false);
                        }
                    },
                    onclick: move |e| e.stop_propagation(),
                    div { class: "search-field",
                        Ico { icon: Icon::Search, size: 16 }
                        input {
                            class: "input",
                            r#type: "search",
                            placeholder: "Search collections…",
                            "aria-label": "Search collections",
                            value: "{filter}",
                            autofocus: true,
                            oninput: move |e| filter.set(e.value()),
                        }
                    }
                    div { class: "list ts-picker-list",
                        for s in found.iter() {
                            {
                                let name = s.collection.clone();
                                let eligible = s.eligible();
                                let is_current = s.collection == current;
                                let hint = time_hint(s);
                                rsx! {
                                    button {
                                        key: "{name}",
                                        class: if is_current { "list-item ts-pick selected" } else { "list-item ts-pick" },
                                        role: "option",
                                        "aria-selected": "{is_current}",
                                        "aria-disabled": "{!eligible}",
                                        disabled: !eligible,
                                        title: "{hint}",
                                        onclick: {
                                            let name = name.clone();
                                            move |_| on_pick.call(name.clone())
                                        },
                                        Ico { icon: s.profile.icon(), size: 16 }
                                        span { class: "ts-pick-text",
                                            span { class: "ts-pick-name", "{name}" }
                                            span { class: "ts-pick-sub", "{hint}" }
                                        }
                                        if is_current {
                                            Ico { icon: Icon::Check, size: 16 }
                                        }
                                    }
                                }
                            }
                        }
                        if found.is_empty() {
                            div { class: "ts-pick-none", "No collection matches. {total} in this database." }
                        }
                    }
                    div { class: "menu-sep" }
                    button {
                        class: "menu-item",
                        role: "menuitem",
                        title: "Show {current} in the Explorer",
                        onclick: {
                            let current = current.clone();
                            move |_| {
                                selected.set(Some(Selected { name: current.clone() }));
                                on_toggle.call(false);
                                nav.push(Route::Explorer {});
                            }
                        },
                        Ico { icon: Icon::FolderOpen, size: 16 }
                        span { "Open collection in Explorer" }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockConnectionService;
    use crate::services::timeseries_data::TimeseriesData;

    async fn all() -> Vec<TimeSource> {
        MockConnectionService::ready()
            .time_sources()
            .await
            .expect("catalog")
            .sources
    }

    #[tokio::test]
    async fn the_time_key_is_named_and_other_timestamps_are_listed() {
        let all = all().await;
        let sales = all.iter().find(|s| s.collection == "sales").expect("sales");
        assert_eq!(time_hint(sales), "sold_at · TIME_KEY");
        let shipments = all
            .iter()
            .find(|s| s.collection == "shipments")
            .expect("shipments");
        assert_eq!(time_hint(shipments), "scheduled_at, updated_at");
    }

    #[tokio::test]
    async fn an_unsupported_collection_shows_its_reason() {
        let all = all().await;
        let kv = all
            .iter()
            .find(|s| s.collection == "sessions")
            .expect("sessions");
        assert!(time_hint(kv).contains("key-value"));
    }

    #[tokio::test]
    async fn search_narrows_the_list_and_eligible_collections_come_first() {
        let all = all().await;
        let first = matching(&all, "");
        let eligible_len = first.iter().take_while(|s| s.eligible()).count();
        assert!(first[eligible_len..].iter().all(|s| !s.eligible()));
        let found = matching(&all, "SALES");
        assert_eq!(found.len(), 1);
        assert!(matching(&all, "zzz").is_empty());
    }
}
