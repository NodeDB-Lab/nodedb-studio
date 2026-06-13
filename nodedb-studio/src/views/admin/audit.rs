//! Admin · Audit log: DDL, RBAC changes, and admin actions.

use dioxus::prelude::*;

#[component]
pub fn AuditView() -> Element {
    // (when, actor, action, target, pill-css, result)
    let rows = [
        (
            "04:18:02",
            "hatta",
            "GRANT",
            "role:writer → aisha",
            "ok",
            "ok",
        ),
        (
            "03:42:14",
            "farhan",
            "CREATE TABLE",
            "analytics.cohorts",
            "ok",
            "ok",
        ),
        (
            "03:14:08",
            "backup-svc",
            "SNAPSHOT",
            "full · 2.4GB",
            "ok",
            "ok",
        ),
        (
            "02:51:22",
            "mkhairi",
            "ALTER POLICY",
            "orders_own_only",
            "ok",
            "ok",
        ),
        (
            "02:10:01",
            "hatta",
            "LOGIN FAIL",
            "—",
            "err",
            "denied · bad pw",
        ),
        ("01:08:55", "hatta", "LOGIN", "—", "ok", "ok"),
    ];
    rsx! {
        div { class: "subview active",
            div { class: "view-header",
                div { h1 { style: "font-size:16px;", "Audit log" } p { "All DDL, RBAC changes, and admin actions." } }
                div { class: "view-header-actions",
                    input { placeholder: "filter…", style: "padding:5px 10px; background:var(--bg-primary); border:0.5px solid var(--border-mid); border-radius:5px; font-size:12px;" }
                    button { class: "btn", "Export" }
                }
            }
            div { class: "card", style: "padding:0;",
                table { class: "table",
                    thead { tr { th { "When" } th { "Actor" } th { "Action" } th { "Target" } th { "Result" } } }
                    tbody {
                        for r in rows {
                            tr {
                                td { class: "mono", "{r.0}" }
                                td { "{r.1}" }
                                td { "{r.2}" }
                                td { "{r.3}" }
                                td { span { class: "pill {r.4}", "{r.5}" } }
                            }
                        }
                    }
                }
            }
        }
    }
}
