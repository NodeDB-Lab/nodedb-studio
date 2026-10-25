//! The Test policy panel: run a policy filter as a named user with a
//! session setting, and see how many rows it keeps. A test reads. It never
//! writes, it never counts the rows it hides, and it never assumes a bypass.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::admin_policy::{PolicySet, PolicyTest};
use crate::models::format::thousands;
use crate::services::backend::Backend;
use crate::state::admin::{AdminUi, PolicyUi, TestRun};
use crate::views::admin::policies::Editing;
use crate::views::events::common::FormField;

const TENANT: &str = "app.tenant_id";

async fn run(mut ui: Signal<PolicyUi>, backend: Rc<dyn Backend>, test: PolicyTest) {
    ui.write().test = TestRun::Running;
    let result = backend.test_policy(&test).await;
    ui.write().test = match result {
        Ok(r) => TestRun::Done(r),
        Err(e) => TestRun::Failed(e.to_string()),
    };
}

#[component]
pub fn PolicyTestPanel(editing: Editing, set: PolicySet) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut ui = use_context::<AdminUi>().policies;
    let u = ui.read().clone();
    let user = u
        .user
        .as_deref()
        .and_then(|n| set.test_users.iter().find(|t| t.name == n))
        .or(set.test_users.first())
        .cloned();
    let Some(user) = user else {
        return rsx! { div { class: "async-empty", "No test user is available." } };
    };
    let default_tenant = user
        .session
        .iter()
        .find(|(k, _)| k == TENANT)
        .map(|(_, v)| v.clone())
        .unwrap_or_default();
    let tenant = u.tenant.clone().unwrap_or(default_tenant);
    let d = &editing.draft;
    let can_run = !d.using_expr.trim().is_empty() && !matches!(u.test, TestRun::Running);
    let start = {
        let test = PolicyTest {
            database: d.database.clone(),
            collection: d.collection.clone(),
            using_expr: d.using_expr.clone(),
            user: user.name.clone(),
            session: vec![(TENANT.to_string(), tenant.clone())],
        };
        move |_| {
            let (backend, test) = (backend.clone(), test.clone());
            spawn_forever(async move { run(ui, backend, test).await });
        }
    };
    rsx! {
        div { class: "ad-test",
            div { class: "ad-form",
                FormField { label: "Run as user", id: "pt-user",
                    select {
                        id: "pt-user",
                        class: "select native",
                        onchange: move |e| {
                            let mut w = ui.write();
                            w.user = Some(e.value());
                            w.tenant = None;
                            w.test = TestRun::Idle;
                        },
                        for t in set.test_users.iter() {
                            option { key: "{t.name}", value: "{t.name}", selected: t.name == user.name, "{t.name}" }
                        }
                    }
                }
                div { class: "field",
                    span { class: "field-label", "Session variable" }
                    div { class: "ad-session",
                        span { class: "mono", "{TENANT}" }
                        input {
                            class: "input",
                            "aria-label": "Value of {TENANT}",
                            value: "{tenant}",
                            oninput: move |e| {
                                let mut w = ui.write();
                                w.tenant = Some(e.value());
                                w.test = TestRun::Idle;
                            },
                        }
                    }
                    div { class: "hint", "This session setting applies for the test." }
                }
            }
            button { class: "btn primary ad-run", disabled: !can_run, onclick: start,
                Ico { icon: Icon::Play, size: 16 }
                "Run test (read-only)"
            }
            h4 { class: "inspector-section", "Results" }
            match u.test.clone() {
                TestRun::Idle => rsx! {
                    p { class: "muted", "Run the test to see which rows the policy keeps for {user.name}." }
                },
                TestRun::Running => rsx! { div { class: "async-loading", "Running…" } },
                TestRun::Failed(msg) => rsx! {
                    div { class: "callout err", role: "alert",
                        Ico { icon: Icon::CircleAlert, size: 16 }
                        span { "{msg}" }
                    }
                },
                TestRun::Done(r) => rsx! {
                    div { class: "callout ok", role: "status",
                        Ico { icon: Icon::CircleCheck, size: 16 }
                        span { "Policy filter applied successfully" }
                    }
                    div { class: "ad-result",
                        div { class: "ad-result-row",
                            span { class: "ad-result-label", "Matched rows" }
                            div {
                                strong { class: "ad-result-num", "{thousands(r.matched_rows)}" }
                                div { class: "hint", "Rows visible to {r.user} ({TENANT} = {tenant})" }
                            }
                        }
                        div { class: "ad-result-row",
                            span { class: "ad-result-label", "Other rows" }
                            div {
                                span { class: "ad-hidden",
                                    Ico { icon: Icon::EyeOff, size: 16 }
                                    strong { "Excluded by policy" }
                                }
                                div { class: "hint", "Rows for other tenants are not visible." }
                            }
                        }
                    }
                    div { class: "callout info",
                        Ico { icon: Icon::Info, size: 16 }
                        span {
                            if r.wrote { "This test changed data." } else { "This is a read-only test. No data was modified." }
                            br {}
                            if r.bypass_assumed { "A bypass privilege was assumed." } else { "Bypass privilege is not assumed." }
                        }
                    }
                },
            }
        }
    }
}
