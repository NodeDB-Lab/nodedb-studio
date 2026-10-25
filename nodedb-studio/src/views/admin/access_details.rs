//! The details of the chosen principal or role, the staged edits waiting
//! for review, and the button that reads their preview.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::admin::AdminChange;
use crate::models::admin_access::{AccessDirectory, Principal, PrincipalKind, Role, SubjectAccess};
use crate::models::format::{date, date_second};
use crate::services::backend::Backend;
use crate::state::admin::AdminUi;
use crate::views::admin::common::Cut;
use crate::views::admin::tasks;
use crate::views::cluster::common::NameChips;
use crate::views::events::common::StatusBadge;

/// The edits waiting for review, each with a control that withdraws it,
/// and the Review changes button.
#[component]
fn StagedChanges(dir: AccessDirectory) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut ui = use_context::<AdminUi>().access;
    let staged = ui.read().staged.clone();
    let n = staged.len();
    let review = {
        let staged = staged.clone();
        move |_| {
            let (backend, change) = (backend.clone(), AdminChange::Access(staged.clone()));
            spawn_forever(async move { tasks::open_review(ui, backend, change).await });
        }
    };
    rsx! {
        if n > 0 {
            h4 { class: "inspector-section", "Staged changes" }
            ul { class: "ad-staged-list",
                for (i , c) in staged.iter().enumerate() {
                    li { key: "{i}",
                        Cut { text: c.describe(|id| dir.name_of(id)) }
                        button {
                            class: "icon-btn sm",
                            title: "Withdraw this change",
                            "aria-label": "Withdraw: {c.describe(|id| dir.name_of(id))}",
                            onclick: move |_| {
                                let mut w = ui.write();
                                if i < w.staged.len() {
                                    w.staged.remove(i);
                                }
                            },
                            Ico { icon: Icon::X, size: 16 }
                        }
                    }
                }
            }
        }
        div { class: "inspector-actions ad-review-action",
            button {
                class: "btn",
                disabled: n == 0,
                title: if n == 0 { "Stage a change first: add or remove a role, or add a principal or role." } else { "Read what the staged changes would do" },
                onclick: review,
                Ico { icon: Icon::Clock, size: 16 }
                if n == 0 {
                    "Review changes"
                } else {
                    "Review changes ({n})"
                }
            }
        }
    }
}

#[component]
pub fn PrincipalDetails(dir: AccessDirectory, principal: Principal) -> Element {
    let p = &principal;
    let icon = match p.kind {
        PrincipalKind::User => Icon::User,
        PrincipalKind::ServiceAccount => Icon::Settings,
    };
    rsx! {
        div { class: "inspector ad-details",
            div { class: "inspector-head",
                Ico { icon, size: 24 }
                h3 { class: "inspector-title", title: "{p.name}", "{p.name}" }
                StatusBadge { tone: p.status.tone(), text: p.status.label().to_string() }
            }
            div { class: "kv",
                div { class: "kv-row", span { "Name" } Cut { text: p.name.clone() } }
                div { class: "kv-row", span { "Type" } span { "{p.kind.label()}" } }
                div { class: "kv-row", span { "Description" } Cut { text: p.description.clone() } }
                div { class: "kv-row",
                    span { "Status" }
                    span { class: "status",
                        span { class: p.status.tone().dot() }
                        "{p.status.label()}"
                    }
                }
                div { class: "kv-row", span { "Credential expiry" } span { "{date(p.credential_expiry, 0)}" } }
                div { class: "kv-row", span { "Created" } span { "{date_second(p.created_at, 0)} UTC" } }
            }
            h4 { class: "inspector-section", "Authentication" }
            div { class: "kv",
                div { class: "kv-row",
                    span { "Password" }
                    span { if p.password { "Enabled" } else { "Not enabled" } }
                }
                div { class: "kv-row", span { "MFA" } span { "{p.mfa.label()}" } }
            }
            p { class: "inspector-note", "Studio never reads a credential. The server holds it." }
            StagedChanges { dir }
        }
    }
}

#[component]
pub fn RoleDetails(dir: AccessDirectory, role: Role, access: Option<SubjectAccess>) -> Element {
    let r = &role;
    let members = access.as_ref().map(|a| a.members.len());
    rsx! {
        div { class: "inspector ad-details",
            div { class: "inspector-head",
                Ico { icon: Icon::Shield, size: 24 }
                h3 { class: "inspector-title", title: "{r.name}", "{r.name}" }
            }
            div { class: "kv",
                div { class: "kv-row", span { "Name" } Cut { text: r.name.clone() } }
                div { class: "kv-row", span { "Description" } Cut { text: r.description.clone() } }
                div { class: "kv-row",
                    span { "Inherits from" }
                    if r.parents.is_empty() {
                        span { class: "muted", "—" }
                    } else {
                        NameChips { names: r.parents.clone() }
                    }
                }
                div { class: "kv-row", span { "Created" } span { "{date_second(r.created_at, 0)} UTC" } }
                div { class: "kv-row",
                    span { "Members" }
                    match members {
                        Some(n) => rsx! { span { class: "num", "{n}" } },
                        None => rsx! { span { class: "muted", "Unknown" } },
                    }
                }
            }
            p { class: "inspector-note",
                "A role grants only the privileges listed in Role grants. It holds no global administrator access."
            }
            StagedChanges { dir }
        }
    }
}
