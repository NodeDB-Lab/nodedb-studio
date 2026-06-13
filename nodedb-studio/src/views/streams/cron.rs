//! Streams · Scheduled jobs: a table of cron-style schedules.

use dioxus::prelude::*;

#[component]
pub fn StreamsCron() -> Element {
    // (name, cron, last_run, pill-css, status, next_run, avg_duration)
    let rows = [
        (
            "nightly_rollup",
            "0 2 * * *",
            "22h ago",
            "ok",
            "success",
            "in 2h 14m",
            "1m 22s",
        ),
        (
            "session_cleanup",
            "*/15 * * * *",
            "4m ago",
            "ok",
            "success",
            "in 11m",
            "812 ms",
        ),
        (
            "vector_reindex",
            "0 4 * * 0",
            "3d ago",
            "err",
            "failed · oom",
            "in 4d 8h",
            "22m",
        ),
        (
            "kv_ttl_sweep",
            "*/5 * * * *",
            "2m ago",
            "ok",
            "success",
            "in 3m",
            "102 ms",
        ),
        (
            "cluster_health_report",
            "0 9 * * 1",
            "2d ago",
            "ok",
            "success",
            "in 5d 4h",
            "8s",
        ),
    ];
    rsx! {
        div { style: "overflow:auto;",
            div { class: "view-padded",
                div { class: "view-header",
                    div {
                        h1 { style: "font-size:16px;", "Scheduled jobs" }
                        p { "Cron-style jobs that run server-side queries on a schedule." }
                    }
                    div { class: "view-header-actions",
                        button { class: "btn primary", "+ Schedule" }
                    }
                }
                div { class: "card", style: "padding: 0;",
                    table { class: "table",
                        thead { tr {
                            th { "Name" } th { "Cron" } th { "Last run" } th { "Status" }
                            th { "Next run" } th { "Avg duration" } th {}
                        } }
                        tbody {
                            for r in rows {
                                tr {
                                    td { strong { "{r.0}" } }
                                    td { class: "mono", "{r.1}" }
                                    td { class: "mono", "{r.2}" }
                                    td { span { class: "pill {r.3}", "{r.4}" } }
                                    td { class: "mono", "{r.5}" }
                                    td { class: "mono", "{r.6}" }
                                    td { button { class: "btn small", "Run now" } }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
