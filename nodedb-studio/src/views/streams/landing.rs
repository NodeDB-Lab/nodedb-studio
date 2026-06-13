//! Streams landing: the five capability cards. Clicking a card opens its
//! sub-screen.

use dioxus::prelude::*;

use crate::routes::Route;

#[component]
pub fn StreamsLanding() -> Element {
    let nav = use_navigator();
    let open = move |tab: &'static str| {
        move |_| {
            nav.push(Route::Streams {
                tab: tab.to_string(),
            });
        }
    };

    rsx! {
        div { class: "view-padded",
            div { class: "view-header",
                div {
                    h1 { "Streams & Events" }
                    p { "CDC streams, materialized views, durable topics, pub/sub, and scheduled jobs — all from one place." }
                }
                div { class: "view-header-actions",
                    button { class: "btn", "Docs" }
                    button { class: "btn primary", "+ New stream" }
                }
            }
            div { class: "se-grid",
                div { class: "card hover se-card", onclick: open("cdc"),
                    div { class: "head",
                        div { class: "icon-box", "CDC" }
                        span { class: "pill ok", span { class: "dot" } "live" }
                    }
                    div { class: "title", "Change data capture" }
                    div { class: "desc", "Stream every insert, update, and delete with row-level diffs." }
                    div { class: "stats",
                        div { class: "s", div { class: "v", "4" } div { class: "l", "streams" } }
                        div { class: "s", div { class: "v", "2.1k/s" } div { class: "l", "events" } }
                    }
                }
                div { class: "card hover se-card", onclick: open("mv"),
                    div { class: "head",
                        div { class: "icon-box", "MV" }
                        span { class: "pill ok", span { class: "dot" } "live" }
                    }
                    div { class: "title", "Materialized views" }
                    div { class: "desc", "Continuous query results that update as the underlying data changes." }
                    div { class: "stats",
                        div { class: "s", div { class: "v", "7" } div { class: "l", "views" } }
                        div { class: "s", div { class: "v", "12ms" } div { class: "l", "p50 lag" } }
                    }
                }
                div { class: "card hover se-card", onclick: open("topics"),
                    div { class: "head",
                        div { class: "icon-box", "TOP" }
                        span { class: "pill", span { class: "dot", style: "background:var(--text-secondary)" } "idle" }
                    }
                    div { class: "title", "Durable topics" }
                    div { class: "desc", "Kafka-style replayable topics with offsets and consumer groups." }
                    div { class: "stats",
                        div { class: "s", div { class: "v", "3" } div { class: "l", "topics" } }
                        div { class: "s", div { class: "v", "128MB" } div { class: "l", "retained" } }
                    }
                }
                div { class: "card hover se-card", onclick: open("notify"),
                    div { class: "head",
                        div { class: "icon-box", "N!" }
                        span { class: "pill info", span { class: "dot" } "14ch" }
                    }
                    div { class: "title", "LISTEN / NOTIFY" }
                    div { class: "desc", "Lightweight pub/sub channels for application-level events." }
                    div { class: "stats",
                        div { class: "s", div { class: "v", "14" } div { class: "l", "channels" } }
                        div { class: "s", div { class: "v", "42" } div { class: "l", "listeners" } }
                    }
                }
                div { class: "card hover se-card", onclick: open("cron"),
                    div { class: "head",
                        div { class: "icon-box", "CR" }
                        span { class: "pill warn", span { class: "dot" } "1 failing" }
                    }
                    div { class: "title", "Scheduled jobs" }
                    div { class: "desc", "Cron-style jobs that run server-side queries on a schedule." }
                    div { class: "stats",
                        div { class: "s", div { class: "v", "9" } div { class: "l", "schedules" } }
                        div { class: "s", div { class: "v", "2m" } div { class: "l", "next run" } }
                    }
                }
            }
        }
    }
}
