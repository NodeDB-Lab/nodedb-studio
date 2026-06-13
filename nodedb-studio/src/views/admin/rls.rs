//! Admin · Row-level security: policy list + policy editor with a test-as-user
//! affordance.

use dioxus::prelude::*;

#[component]
pub fn RlsView() -> Element {
    let policies = [
        ("orders_own_only", true),
        ("events_tenant_scoped", false),
        ("users_self_or_admin", false),
        ("documents_team_read", false),
    ];
    rsx! {
        div { class: "subview active",
            div { class: "view-header",
                div { h1 { style: "font-size:16px;", "Row-level security" } p { "Per-table policies. Test as any user before saving." } }
                div { class: "view-header-actions", button { class: "btn primary", "+ Policy" } }
            }
            div { style: "display:grid; grid-template-columns: 280px 1fr; gap: 14px; height: calc(100vh - 200px);",
                div { class: "card", style: "padding:8px; overflow:auto;",
                    div { class: "eyebrow", style: "padding:6px 10px;", "Policies" }
                    for (name, active) in policies {
                        div { class: if active { "collection active" } else { "collection" },
                            span { class: "ico", "P" }
                            " {name}"
                        }
                    }
                }
                div { class: "card", style: "padding:18px;",
                    div { style: "display:flex; align-items:center; justify-content:space-between; margin-bottom:14px;",
                        strong { "orders_own_only" }
                        span { class: "pill ok", span { class: "dot" } "enabled" }
                    }
                    div { class: "form-field", label { "Table" } input { value: "orders" } }
                    div { class: "form-field", label { "Operation" } input { value: "SELECT, UPDATE, DELETE" } }
                    div { class: "form-field",
                        label { "Expression" }
                        pre { style: "background:var(--bg-secondary); border:0.5px solid var(--border-mid); border-radius:5px; padding:10px; margin:0; font-family:var(--font-mono); font-size:12px; line-height:1.6;",
                            span { class: "sql-kw", "USING" }
                            " (user_id = current_user_id()\n       "
                            span { class: "sql-kw", "OR" }
                            " current_role() = "
                            span { class: "sql-str", "'admin'" }
                            ")"
                        }
                    }
                    div { class: "form-field",
                        label { "Test as user" }
                        div { style: "display:flex; gap:8px;",
                            select { style: "flex:1;", option { "u_44182 (hatta)" } option { "u_77103 (farhan)" } }
                            button { class: "btn", "Run test" }
                        }
                    }
                    div { style: "background:var(--bg-secondary); border-left: 2px solid var(--text-success); padding: 10px 14px; font-family: var(--font-mono); font-size: 11px;",
                        "✓ test passed · 14 rows visible (of 442,003 total)"
                    }
                }
            }
        }
    }
}
