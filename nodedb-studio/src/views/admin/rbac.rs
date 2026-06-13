//! Admin · RBAC: users, roles, and grants.

use dioxus::prelude::*;

struct Row {
    name: &'static str,
    email: &'static str,
    service: bool,
    analyst: bool,
    writer: bool,
    ops: bool,
    admin: bool,
    last_login: &'static str,
}

fn dot(on: bool) -> &'static str {
    if on { "●" } else { "" }
}

#[component]
pub fn RbacView() -> Element {
    let rows = [
        Row {
            name: "hatta",
            email: "hatta@nodedb.io",
            service: false,
            analyst: true,
            writer: true,
            ops: true,
            admin: true,
            last_login: "2m ago",
        },
        Row {
            name: "farhan",
            email: "",
            service: false,
            analyst: true,
            writer: true,
            ops: true,
            admin: true,
            last_login: "14m ago",
        },
        Row {
            name: "aisha",
            email: "",
            service: false,
            analyst: true,
            writer: true,
            ops: false,
            admin: false,
            last_login: "1h ago",
        },
        Row {
            name: "mkhairi",
            email: "",
            service: false,
            analyst: true,
            writer: true,
            ops: true,
            admin: false,
            last_login: "22m ago",
        },
        Row {
            name: "read-only-bi",
            email: "",
            service: true,
            analyst: true,
            writer: false,
            ops: false,
            admin: false,
            last_login: "just now",
        },
        Row {
            name: "backup-svc",
            email: "",
            service: true,
            analyst: false,
            writer: false,
            ops: true,
            admin: false,
            last_login: "4m ago",
        },
    ];
    rsx! {
        div { class: "subview active",
            div { class: "view-header",
                div { h1 { style: "font-size:16px;", "RBAC" } p { "Users, roles, and grants." } }
                div { class: "view-header-actions", button { class: "btn", "+ Role" } button { class: "btn primary", "+ User" } }
            }
            div { class: "card", style: "padding:0; overflow:auto;",
                table { class: "table",
                    thead { tr { th { "User" } th { "analyst" } th { "writer" } th { "ops" } th { "admin" } th { "Last login" } } }
                    tbody {
                        for r in rows {
                            tr {
                                td {
                                    strong { "{r.name}" }
                                    if !r.email.is_empty() {
                                        br {}
                                        span { style: "color:var(--text-tertiary);font-family:var(--font-mono);font-size:11px;", "{r.email}" }
                                    }
                                    if r.service {
                                        " "
                                        span { class: "pill", "service" }
                                    }
                                }
                                td { "{dot(r.analyst)}" }
                                td { "{dot(r.writer)}" }
                                td { "{dot(r.ops)}" }
                                td { "{dot(r.admin)}" }
                                td { class: "mono", "{r.last_login}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
