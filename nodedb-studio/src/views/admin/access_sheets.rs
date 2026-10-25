//! The sheets over Access: the forms that stage an edit, and the review
//! that applies the staged edits.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::sheet::{Sheet, SheetFooter, SheetWidth};
use crate::models::admin_access::{
    AccessChange, AccessDirectory, PrincipalDraft, PrincipalKind, RoleDraft,
};
use crate::services::backend::Backend;
use crate::state::admin::{AccessMode, AccessSheet, AdminUi};
use crate::views::admin::common::ReviewSheet;
use crate::views::admin::tasks;
use crate::views::events::common::FormField;

#[component]
pub fn AccessSheets(dir: AccessDirectory) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut ui = use_context::<AdminUi>().access;
    let u = ui.read().clone();
    if let Some(state) = u.review.clone() {
        return rsx! {
            ReviewSheet {
                state,
                back_label: "Back to changes",
                on_back: move |_| ui.write().review = None,
                on_apply: move |_| {
                    let backend = backend.clone();
                    spawn_forever(async move { tasks::apply_review(ui, backend).await });
                },
            }
        };
    }
    match u.sheet.clone() {
        Some(AccessSheet::Principal(d)) => rsx! { PrincipalSheet { draft: d, dir } },
        Some(AccessSheet::Role(d)) => rsx! { RoleSheet { draft: d, dir } },
        Some(AccessSheet::Membership {
            principal,
            role,
            database,
        }) => rsx! { MembershipSheet { principal, role, database, dir, mode: u.mode } },
        None => rsx! {},
    }
}

/// Names that already exist or are staged, so a draft cannot repeat one.
fn taken_principals(dir: &AccessDirectory, staged: &[AccessChange]) -> Vec<String> {
    let mut out: Vec<String> = dir.principals.iter().map(|p| p.name.clone()).collect();
    out.extend(staged.iter().filter_map(|c| match c {
        AccessChange::CreatePrincipal { name, .. } => Some(name.clone()),
        _ => None,
    }));
    out
}

fn taken_roles(dir: &AccessDirectory, staged: &[AccessChange]) -> Vec<String> {
    let mut out: Vec<String> = dir.roles.iter().map(|r| r.name.clone()).collect();
    out.extend(staged.iter().filter_map(|c| match c {
        AccessChange::CreateRole { name, .. } => Some(name.clone()),
        _ => None,
    }));
    out
}

fn edit_principal(
    mut ui: Signal<crate::state::admin::AccessUi>,
    f: impl FnOnce(&mut PrincipalDraft),
) {
    if let Some(AccessSheet::Principal(d)) = ui.write().sheet.as_mut() {
        f(d);
    }
}

#[component]
fn PrincipalSheet(draft: PrincipalDraft, dir: AccessDirectory) -> Element {
    let mut ui = use_context::<AdminUi>().access;
    let taken = taken_principals(&dir, &ui.read().staged);
    let name_err = draft.tried.then(|| draft.name_issue(&taken)).flatten();
    let expiry_err = draft.tried.then(|| draft.expiry_issue()).flatten();
    let stage = {
        let (draft, taken) = (draft.clone(), taken.clone());
        move |_| match draft.change(&taken) {
            Some(change) => {
                let mut w = ui.write();
                w.staged.push(change);
                w.sheet = None;
                w.mode = AccessMode::Principals;
            }
            None => edit_principal(ui, |d| d.tried = true),
        }
    };
    rsx! {
        Sheet {
            title: "New principal".to_string(),
            width: SheetWidth::Narrow,
            on_close: move |_| ui.write().sheet = None,
            footer: rsx! {
                SheetFooter {
                    error: None,
                    hint: Some("Staging adds it to the changes you review. Nothing is created yet.".to_string()),
                    button { class: "btn", onclick: move |_| ui.write().sheet = None, "Cancel" }
                    button { class: "btn primary", onclick: stage, "Stage principal" }
                }
            },
            div { class: "sheet-form ad-form",
                FormField { label: "Name", id: "ad-p-name", error: name_err, hint: "Lowercase letters, digits, and underscores.".to_string(),
                    input {
                        id: "ad-p-name",
                        class: "input",
                        value: "{draft.name}",
                        oninput: move |e| edit_principal(ui, |d| d.name = e.value()),
                    }
                }
                FormField { label: "Type", id: "ad-p-kind",
                    select {
                        id: "ad-p-kind",
                        class: "select native",
                        onchange: move |e| {
                            let v = e.value();
                            if let Some(k) = PrincipalKind::ALL.iter().copied().find(|k| k.label() == v) {
                                edit_principal(ui, |d| d.kind = k);
                            }
                        },
                        for k in PrincipalKind::ALL {
                            option { key: "{k.label()}", value: "{k.label()}", selected: k == draft.kind, "{k.label()}" }
                        }
                    }
                }
                FormField { label: "Description", id: "ad-p-desc",
                    input {
                        id: "ad-p-desc",
                        class: "input",
                        value: "{draft.description}",
                        oninput: move |e| edit_principal(ui, |d| d.description = e.value()),
                    }
                }
                FormField {
                    label: "Credential expiry",
                    id: "ad-p-expiry",
                    error: expiry_err,
                    hint: "The server holds the credential. Studio never sees it.".to_string(),
                    input {
                        id: "ad-p-expiry",
                        class: "input",
                        r#type: "date",
                        value: "{draft.expiry}",
                        oninput: move |e| edit_principal(ui, |d| d.expiry = e.value()),
                    }
                }
            }
        }
    }
}

fn edit_role(mut ui: Signal<crate::state::admin::AccessUi>, f: impl FnOnce(&mut RoleDraft)) {
    if let Some(AccessSheet::Role(d)) = ui.write().sheet.as_mut() {
        f(d);
    }
}

#[component]
fn RoleSheet(draft: RoleDraft, dir: AccessDirectory) -> Element {
    let mut ui = use_context::<AdminUi>().access;
    let taken = taken_roles(&dir, &ui.read().staged);
    let name_err = draft.tried.then(|| draft.name_issue(&taken)).flatten();
    let stage = {
        let (draft, taken) = (draft.clone(), taken.clone());
        move |_| match draft.change(&taken) {
            Some(change) => {
                let mut w = ui.write();
                w.staged.push(change);
                w.sheet = None;
                w.mode = AccessMode::Roles;
            }
            None => edit_role(ui, |d| d.tried = true),
        }
    };
    rsx! {
        Sheet {
            title: "New role".to_string(),
            width: SheetWidth::Narrow,
            on_close: move |_| ui.write().sheet = None,
            footer: rsx! {
                SheetFooter {
                    error: None,
                    hint: Some("A new role grants nothing until it has grants.".to_string()),
                    button { class: "btn", onclick: move |_| ui.write().sheet = None, "Cancel" }
                    button { class: "btn primary", onclick: stage, "Stage role" }
                }
            },
            div { class: "sheet-form ad-form",
                FormField { label: "Name", id: "ad-r-name", error: name_err, hint: "Lowercase letters, digits, and underscores.".to_string(),
                    input {
                        id: "ad-r-name",
                        class: "input",
                        value: "{draft.name}",
                        oninput: move |e| edit_role(ui, |d| d.name = e.value()),
                    }
                }
                FormField { label: "Description", id: "ad-r-desc",
                    input {
                        id: "ad-r-desc",
                        class: "input",
                        value: "{draft.description}",
                        oninput: move |e| edit_role(ui, |d| d.description = e.value()),
                    }
                }
            }
        }
    }
}

fn edit_membership(
    mut ui: Signal<crate::state::admin::AccessUi>,
    f: impl FnOnce(&mut String, &mut String, &mut String),
) {
    if let Some(AccessSheet::Membership {
        principal,
        role,
        database,
    }) = ui.write().sheet.as_mut()
    {
        f(principal, role, database);
    }
}

#[component]
fn MembershipSheet(
    principal: String,
    role: String,
    database: String,
    dir: AccessDirectory,
    mode: AccessMode,
) -> Element {
    let mut ui = use_context::<AdminUi>().access;
    let roles_mode = mode == AccessMode::Roles;
    let who = dir.name_of(&principal);
    let change = AccessChange::AddMembership {
        principal: principal.clone(),
        role: role.clone(),
        database: database.clone(),
    };
    let duplicate = ui.read().staged.contains(&change);
    let title = if roles_mode {
        format!("Add a member to {role}")
    } else {
        format!("Add a role to {who}")
    };
    let stage = {
        let change = change.clone();
        move |_| {
            let mut w = ui.write();
            if !w.staged.contains(&change) {
                w.staged.push(change.clone());
            }
            w.sheet = None;
        }
    };
    rsx! {
        Sheet {
            title,
            width: SheetWidth::Narrow,
            on_close: move |_| ui.write().sheet = None,
            footer: rsx! {
                SheetFooter {
                    error: None,
                    hint: Some("Staging adds it to the changes you review. Nothing is granted yet.".to_string()),
                    button { class: "btn", onclick: move |_| ui.write().sheet = None, "Cancel" }
                    button { class: "btn primary", disabled: duplicate, onclick: stage,
                        if roles_mode { "Stage member" } else { "Stage role" }
                    }
                }
            },
            div { class: "sheet-form ad-form",
                if roles_mode {
                    FormField { label: "Principal", id: "ad-m-principal",
                        select {
                            id: "ad-m-principal",
                            class: "select native",
                            onchange: move |e| edit_membership(ui, |p, _, _| *p = e.value()),
                            for p in dir.principals.iter() {
                                option { key: "{p.id}", value: "{p.id}", selected: p.id == principal, "{p.name}" }
                            }
                        }
                    }
                    FormField { label: "Role", id: "ad-m-role-fixed",
                        input { id: "ad-m-role-fixed", class: "input", value: "{role}", readonly: true }
                    }
                } else {
                    FormField { label: "Principal", id: "ad-m-principal-fixed",
                        input { id: "ad-m-principal-fixed", class: "input", value: "{who}", readonly: true }
                    }
                    FormField { label: "Role", id: "ad-m-role",
                        select {
                            id: "ad-m-role",
                            class: "select native",
                            onchange: move |e| edit_membership(ui, |_, r, _| *r = e.value()),
                            for r in dir.roles.iter() {
                                option { key: "{r.name}", value: "{r.name}", selected: r.name == role, "{r.name}" }
                            }
                        }
                    }
                }
                FormField {
                    label: "Database",
                    id: "ad-m-db",
                    hint: "The role applies to this database only.".to_string(),
                    select {
                        id: "ad-m-db",
                        class: "select native",
                        onchange: move |e| edit_membership(ui, |_, _, d| *d = e.value()),
                        for d in dir.databases.iter() {
                            option { key: "{d}", value: "{d}", selected: *d == database, "{d}" }
                        }
                    }
                }
                if duplicate {
                    div { class: "callout warn",
                        span { "This change is already staged." }
                    }
                }
            }
        }
    }
}
