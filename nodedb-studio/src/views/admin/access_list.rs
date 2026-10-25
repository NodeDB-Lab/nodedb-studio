//! The lists at the top of Access: principals, or roles. Staged edits show
//! in them as rows that cannot be chosen yet, so the person sees what the
//! review will apply.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::admin_access::{
    AccessChange, AccessDirectory, PrincipalDraft, PrincipalKind, RoleDraft,
};
use crate::models::format::date;
use crate::state::admin::{AccessSheet, AccessUi, AdminUi};
use crate::views::admin::common::Cut;
use crate::views::events::common::StatusText;

#[component]
pub fn PrincipalsList(dir: AccessDirectory, chosen: Option<String>) -> Element {
    let mut ui = use_context::<AdminUi>().access;
    let u: AccessUi = ui.read().clone();
    let needle = u.filter.trim().to_lowercase();
    let rows: Vec<_> = dir
        .principals
        .iter()
        .filter(|p| u.kind.is_none_or(|k| p.kind == k))
        .filter(|p| p.matches(&needle))
        .collect();
    let staged: Vec<(String, PrincipalKind, String, i64)> = u
        .staged
        .iter()
        .filter_map(|c| match c {
            AccessChange::CreatePrincipal {
                name,
                kind,
                description,
                credential_expiry,
            } => Some((name.clone(), *kind, description.clone(), *credential_expiry)),
            _ => None,
        })
        .collect();
    rsx! {
        div { class: "toolbar nowrap cl-filters",
            div { class: "search-field grow",
                Ico { icon: Icon::Search, size: 16 }
                input {
                    class: "input",
                    placeholder: "Filter principals…",
                    "aria-label": "Filter principals",
                    value: "{u.filter}",
                    oninput: move |e| ui.write().filter = e.value(),
                }
            }
            select {
                class: "select native",
                "aria-label": "Principal type",
                onchange: move |e| {
                    let v = e.value();
                    ui.write().kind = PrincipalKind::ALL.iter().copied().find(|k| k.label() == v);
                },
                option { value: "", selected: u.kind.is_none(), "All types" }
                for k in PrincipalKind::ALL {
                    option { key: "{k.label()}", value: "{k.label()}", selected: Some(k) == u.kind, "{k.label()}" }
                }
            }
            button {
                class: "btn primary",
                onclick: move |_| ui.write().sheet = Some(AccessSheet::Principal(PrincipalDraft::new())),
                Ico { icon: Icon::Plus, size: 16 }
                "Principal"
            }
            button {
                class: "btn primary",
                title: "Create a role. It grants nothing until it has grants.",
                onclick: move |_| ui.write().sheet = Some(AccessSheet::Role(RoleDraft::default())),
                Ico { icon: Icon::Plus, size: 16 }
                "Role"
            }
        }
        div { class: "dt-wrap",
            table { class: "dt cl-table ad-table ad-principals",
                thead {
                    tr {
                        th { "Name" }
                        th { "Type" }
                        th { "Status" }
                        th { "Credential expiry" }
                        th { "Description" }
                    }
                }
                tbody {
                    for p in rows.iter() {
                        {
                            let id = p.id.clone();
                            let is_selected = chosen.as_deref() == Some(p.id.as_str());
                            let icon = match p.kind {
                                PrincipalKind::User => Icon::User,
                                PrincipalKind::ServiceAccount => Icon::Settings,
                            };
                            rsx! {
                                tr {
                                    key: "{p.id}",
                                    class: if is_selected { "clickable selected" } else { "clickable" },
                                    "aria-selected": "{is_selected}",
                                    tabindex: "0",
                                    onclick: move |_| ui.write().principal = Some(id.clone()),
                                    td { class: "key",
                                        span { class: "ad-name",
                                            Ico { icon, size: 16 }
                                            Cut { text: p.name.clone() }
                                        }
                                    }
                                    td { "{p.kind.label()}" }
                                    td { StatusText { tone: p.status.tone(), text: p.status.label().to_string() } }
                                    td { "{date(p.credential_expiry, 0)}" }
                                    td { Cut { text: p.description.clone() } }
                                }
                            }
                        }
                    }
                    for (name , kind , description , expiry) in staged {
                        tr { key: "staged-{name}", class: "ad-staged", "aria-disabled": "true",
                            td { class: "key",
                                span { class: "ad-name",
                                    Ico { icon: if kind == PrincipalKind::User { Icon::User } else { Icon::Settings }, size: 16 }
                                    Cut { text: name.clone() }
                                }
                            }
                            td { "{kind.label()}" }
                            td { span { class: "badge amber", "Staged" } }
                            td { "{date(expiry, 0)}" }
                            td { Cut { text: description.clone() } }
                        }
                    }
                }
            }
            if rows.is_empty() && !dir.principals.is_empty() {
                div { class: "async-empty", "No principal matches these filters." }
            }
        }
        div { class: "dt-footer", "{dir.principals.len()} principals" }
    }
}

#[component]
pub fn RolesList(dir: AccessDirectory, chosen: Option<String>) -> Element {
    let mut ui = use_context::<AdminUi>().access;
    let u: AccessUi = ui.read().clone();
    let needle = u.filter.trim().to_lowercase();
    let rows: Vec<_> = dir.roles.iter().filter(|r| r.matches(&needle)).collect();
    let staged: Vec<(String, String)> = u
        .staged
        .iter()
        .filter_map(|c| match c {
            AccessChange::CreateRole { name, description } => {
                Some((name.clone(), description.clone()))
            }
            _ => None,
        })
        .collect();
    rsx! {
        div { class: "toolbar nowrap cl-filters",
            div { class: "search-field grow",
                Ico { icon: Icon::Search, size: 16 }
                input {
                    class: "input",
                    placeholder: "Filter roles…",
                    "aria-label": "Filter roles",
                    value: "{u.filter}",
                    oninput: move |e| ui.write().filter = e.value(),
                }
            }
            button {
                class: "btn primary",
                onclick: move |_| ui.write().sheet = Some(AccessSheet::Principal(PrincipalDraft::new())),
                Ico { icon: Icon::Plus, size: 16 }
                "Principal"
            }
            button {
                class: "btn primary",
                title: "Create a role. It grants nothing until it has grants.",
                onclick: move |_| ui.write().sheet = Some(AccessSheet::Role(RoleDraft::default())),
                Ico { icon: Icon::Plus, size: 16 }
                "Role"
            }
        }
        div { class: "dt-wrap",
            table { class: "dt cl-table ad-table ad-roles",
                thead {
                    tr {
                        th { "Name" }
                        th { "Inherits from" }
                        th { "Description" }
                    }
                }
                tbody {
                    for r in rows.iter() {
                        {
                            let name = r.name.clone();
                            let is_selected = chosen.as_deref() == Some(r.name.as_str());
                            rsx! {
                                tr {
                                    key: "{r.name}",
                                    class: if is_selected { "clickable selected" } else { "clickable" },
                                    "aria-selected": "{is_selected}",
                                    tabindex: "0",
                                    onclick: move |_| ui.write().role = Some(name.clone()),
                                    td { class: "key",
                                        span { class: "ad-name",
                                            Ico { icon: Icon::Shield, size: 16 }
                                            Cut { text: r.name.clone() }
                                        }
                                    }
                                    td {
                                        if r.parents.is_empty() {
                                            span { class: "muted", "—" }
                                        } else {
                                            Cut { text: r.parents.join(", ") }
                                        }
                                    }
                                    td { Cut { text: r.description.clone() } }
                                }
                            }
                        }
                    }
                    for (name , description) in staged {
                        tr { key: "staged-{name}", class: "ad-staged", "aria-disabled": "true",
                            td { class: "key",
                                span { class: "ad-name",
                                    Ico { icon: Icon::Shield, size: 16 }
                                    Cut { text: name.clone() }
                                    span { class: "badge amber", "Staged" }
                                }
                            }
                            td { span { class: "muted", "—" } }
                            td { Cut { text: description.clone() } }
                        }
                    }
                }
            }
            if rows.is_empty() && !dir.roles.is_empty() {
                div { class: "async-empty", "No role matches the filter." }
            }
        }
        div { class: "dt-footer", "{dir.roles.len()} roles" }
    }
}
