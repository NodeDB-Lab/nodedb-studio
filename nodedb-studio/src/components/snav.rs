//! Streams lateral navigation: a back-to-landing arrow + a tab strip across the
//! five sub-screens. Selecting a tab navigates directly between sub-screens
//! without returning to the landing.

use dioxus::prelude::*;

use crate::routes::Route;

const TABS: [(&str, &str); 5] = [
    ("cdc", "CDC"),
    ("mv", "Materialized views"),
    ("topics", "Durable topics"),
    ("notify", "LISTEN/NOTIFY"),
    ("cron", "Scheduled jobs"),
];

#[component]
pub fn Snav(active: String) -> Element {
    let nav = use_navigator();
    rsx! {
        div { class: "snav",
            button {
                class: "snav-back",
                title: "Back to landing",
                onclick: move |_| { nav.push(Route::Streams { tab: "landing".to_string() }); },
                "←"
            }
            for (key, label) in TABS {
                {
                    let is_active = active == key;
                    let k = key.to_string();
                    let item_class = if is_active { "snav-item active" } else { "snav-item" };
                    rsx! {
                        div {
                            class: "{item_class}",
                            onclick: move |_| { nav.push(Route::Streams { tab: k.clone() }); },
                            "{label}"
                            if is_active {
                                span { class: "pulse-dot" }
                            }
                        }
                    }
                }
            }
        }
    }
}
