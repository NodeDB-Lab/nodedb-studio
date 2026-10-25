//! The three lower Access panels: the grants of the roles involved, the
//! memberships (or members), and the access in effect.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::admin_access::{AccessChange, AccessDirectory, AccessSubject, SubjectAccess};
use crate::state::admin::{AccessMode, AccessSheet, AdminUi};
use crate::views::admin::common::Cut;
use crate::views::events::common::InfoTip;

fn waiting(loading: bool, error: Option<String>) -> Element {
    match (loading, error) {
        (true, _) => rsx! { div { class: "async-loading", "Loading…" } },
        (false, Some(e)) => rsx! { div { class: "async-error-msg", "{e}" } },
        (false, None) => rsx! { div { class: "async-empty", "Choose a principal or role." } },
    }
}

#[component]
pub fn GrantsBody(access: Option<SubjectAccess>, loading: bool) -> Element {
    let mut ui = use_context::<AdminUi>().access;
    let needle = ui.read().grants_filter.trim().to_lowercase();
    let Some(a) = access else {
        return waiting(loading, None);
    };
    let rows: Vec<_> = a
        .grants
        .iter()
        .filter(|g| {
            needle.is_empty()
                || [&g.role, &g.collection, &g.granted_by]
                    .iter()
                    .any(|f| f.to_lowercase().contains(&needle))
        })
        .collect();
    rsx! {
        div { class: "toolbar nowrap cl-filters",
            div { class: "search-field grow",
                Ico { icon: Icon::Search, size: 16 }
                input {
                    class: "input",
                    placeholder: "Filter role grants…",
                    "aria-label": "Filter role grants",
                    value: "{ui.read().grants_filter}",
                    oninput: move |e| ui.write().grants_filter = e.value(),
                }
            }
        }
        div { class: "dt-wrap",
            table { class: "dt cl-table ad-table ad-grants",
                thead {
                    tr {
                        th { "Role" }
                        th { "Target collection" }
                        th { "Privileges" }
                        th { "Grant type" }
                        th { "Granted by" }
                    }
                }
                tbody {
                    for (i , g) in rows.iter().enumerate() {
                        tr { key: "{i}-{g.role}-{g.collection}-{g.privilege.label()}",
                            td { class: "key", Cut { text: g.role.clone() } }
                            td { title: "{g.database}.{g.collection}", Cut { text: g.collection.clone() } }
                            td { "{g.privilege.label()}" }
                            td { "Direct" }
                            td { Cut { text: g.granted_by.clone() } }
                        }
                    }
                }
            }
            if rows.is_empty() {
                div { class: "async-empty",
                    if a.grants.is_empty() {
                        "These roles carry no collection grants."
                    } else {
                        "No grant matches the filter."
                    }
                }
            }
        }
    }
}

#[component]
pub fn MembershipBody(
    dir: AccessDirectory,
    access: Option<SubjectAccess>,
    loading: bool,
) -> Element {
    let mut ui = use_context::<AdminUi>().access;
    let u = ui.read().clone();
    let needle = u.members_filter.trim().to_lowercase();
    let Some(a) = access else {
        return waiting(loading, None);
    };
    match a.subject.clone() {
        AccessSubject::Principal(pid) => {
            let who = dir.name_of(&pid);
            let added = |role: &str, db: &str| {
                u.staged.iter().position(|c| {
                    matches!(c, AccessChange::AddMembership { principal, role: r, database }
                        if *principal == pid && r == role && database == db)
                })
            };
            let removed = |role: &str, db: &str| {
                u.staged.iter().position(|c| {
                    matches!(c, AccessChange::RemoveMembership { principal, role: r, database }
                        if *principal == pid && r == role && database == db)
                })
            };
            // Stored memberships, then the ones staged to be added.
            let mut rows: Vec<(String, String, Option<u32>, bool)> = a
                .memberships
                .iter()
                .map(|m| {
                    (
                        m.database.clone(),
                        m.role.clone(),
                        Some(m.collections),
                        false,
                    )
                })
                .collect();
            for c in &u.staged {
                if let AccessChange::AddMembership {
                    principal,
                    role,
                    database,
                } = c
                    && *principal == pid
                {
                    rows.push((database.clone(), role.clone(), None, true));
                }
            }
            rows.retain(|(_, role, _, _)| {
                needle.is_empty() || role.to_lowercase().contains(&needle)
            });
            let mut databases: Vec<String> = rows.iter().map(|r| r.0.clone()).collect();
            databases.sort();
            databases.dedup();
            let open = {
                let dir = dir.clone();
                let pid = pid.clone();
                let held: Vec<String> = a.memberships.iter().map(|m| m.role.clone()).collect();
                move |_| {
                    let role = dir
                        .roles
                        .iter()
                        .find(|r| !held.contains(&r.name))
                        .or(dir.roles.first())
                        .map(|r| r.name.clone())
                        .unwrap_or_default();
                    ui.write().sheet = Some(AccessSheet::Membership {
                        principal: pid.clone(),
                        role,
                        database: dir.databases.first().cloned().unwrap_or_default(),
                    });
                }
            };
            rsx! {
                div { class: "toolbar nowrap cl-filters",
                    div { class: "search-field grow",
                        Ico { icon: Icon::Search, size: 16 }
                        input {
                            class: "input",
                            placeholder: "Filter roles…",
                            "aria-label": "Filter roles",
                            value: "{u.members_filter}",
                            oninput: move |e| ui.write().members_filter = e.value(),
                        }
                    }
                    button { class: "btn primary", onclick: open,
                        Ico { icon: Icon::Plus, size: 16 }
                        "Role"
                    }
                }
                div { class: "dt-wrap",
                    for db in databases {
                        {
                            let group: Vec<_> = rows.iter().filter(|r| r.0 == db).collect();
                            let count: u32 = group.iter().filter_map(|r| r.2).sum();
                            rsx! {
                                div { key: "{db}", class: "ad-group-head",
                                    Ico { icon: Icon::Database, size: 16 }
                                    span { class: "muted", "Database" }
                                    strong { "{db}" }
                                    span { class: "muted", "({count} " if count == 1 { "collection" } else { "collections" } ")" }
                                    InfoTip { text: "Collections the roles below grant privileges on in this database.".to_string() }
                                }
                                table { class: "dt cl-table ad-table ad-members",
                                    thead {
                                        tr {
                                            th { "Role" }
                                            th { "Scope" }
                                            th { class: "ad-x", "" }
                                        }
                                    }
                                    tbody {
                                        for (_ , role , n , is_new) in group {
                                            {
                                                let (r1, d1) = (role.clone(), db.clone());
                                                let gone = removed(role, &db).is_some();
                                                let add_at = added(role, &db);
                                                let remove_name = format!("Remove role {role} from {who}");
                                                let pid = pid.clone();
                                                rsx! {
                                                    tr {
                                                        key: "{role}",
                                                        class: if gone { "ad-staged ad-gone" } else if *is_new { "ad-staged" } else { "" },
                                                        td { class: "key", Cut { text: role.clone() } }
                                                        td {
                                                            match n {
                                                                Some(n) => rsx! { "{n} " if *n == 1 { "collection" } else { "collections" } },
                                                                None => rsx! { span { class: "badge amber", "Staged" } },
                                                            }
                                                            if gone {
                                                                span { class: "badge amber", "Removal staged" }
                                                            }
                                                        }
                                                        td { class: "ad-x",
                                                            button {
                                                                class: "icon-btn sm",
                                                                title: if gone { "Withdraw the removal" } else if *is_new { "Withdraw the addition" } else { "Stage removing this role" },
                                                                "aria-label": if gone { "Withdraw removing role {role}" } else { "{remove_name}" },
                                                                onclick: move |_| {
                                                                    let mut w = ui.write();
                                                                    if let Some(i) = add_at {
                                                                        w.staged.remove(i);
                                                                    } else if let Some(i) = w.staged.iter().position(|c| matches!(c,
                                                                        AccessChange::RemoveMembership { principal, role, database }
                                                                        if *principal == pid && *role == r1 && *database == d1))
                                                                    {
                                                                        w.staged.remove(i);
                                                                    } else {
                                                                        w.staged.push(AccessChange::RemoveMembership {
                                                                            principal: pid.clone(),
                                                                            role: r1.clone(),
                                                                            database: d1.clone(),
                                                                        });
                                                                    }
                                                                },
                                                                Ico { icon: if gone || *is_new { Icon::RotateCcw } else { Icon::X }, size: 16 }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if rows.is_empty() {
                        div { class: "async-empty",
                            if a.memberships.is_empty() { "{who} holds no role." } else { "No role matches the filter." }
                        }
                    }
                }
            }
        }
        AccessSubject::Role(name) => {
            let members: Vec<_> = a
                .members
                .iter()
                .filter(|m| needle.is_empty() || m.name.to_lowercase().contains(&needle))
                .collect();
            let open = {
                let dir = dir.clone();
                let held: Vec<String> = a.members.iter().map(|m| m.principal.clone()).collect();
                let name = name.clone();
                move |_| {
                    let principal = dir
                        .principals
                        .iter()
                        .find(|p| !held.contains(&p.id))
                        .or(dir.principals.first())
                        .map(|p| p.id.clone())
                        .unwrap_or_default();
                    ui.write().sheet = Some(AccessSheet::Membership {
                        principal,
                        role: name.clone(),
                        database: dir.databases.first().cloned().unwrap_or_default(),
                    });
                }
            };
            rsx! {
                div { class: "toolbar nowrap cl-filters",
                    div { class: "search-field grow",
                        Ico { icon: Icon::Search, size: 16 }
                        input {
                            class: "input",
                            placeholder: "Filter members…",
                            "aria-label": "Filter members",
                            value: "{u.members_filter}",
                            oninput: move |e| ui.write().members_filter = e.value(),
                        }
                    }
                    button { class: "btn primary", onclick: open,
                        Ico { icon: Icon::Plus, size: 16 }
                        "Principal"
                    }
                }
                div { class: "dt-wrap",
                    table { class: "dt cl-table ad-table ad-members",
                        thead {
                            tr {
                                th { "Principal" }
                                th { "Database" }
                                th { class: "ad-x", "" }
                            }
                        }
                        tbody {
                            for m in members.iter() {
                                {
                                    let (pid, role, db) = (m.principal.clone(), name.clone(), m.database.clone());
                                    let label = format!("Remove {} from role {name}", m.name);
                                    rsx! {
                                        tr { key: "{m.principal}-{m.database}",
                                            td { class: "key", Cut { text: m.name.clone() } }
                                            td { "{m.database}" }
                                            td { class: "ad-x",
                                                button {
                                                    class: "icon-btn sm",
                                                    title: "Stage removing this member",
                                                    "aria-label": "{label}",
                                                    onclick: move |_| {
                                                        let change = AccessChange::RemoveMembership {
                                                            principal: pid.clone(),
                                                            role: role.clone(),
                                                            database: db.clone(),
                                                        };
                                                        let mut w = ui.write();
                                                        if !w.staged.contains(&change) {
                                                            w.staged.push(change);
                                                        }
                                                    },
                                                    Ico { icon: Icon::X, size: 16 }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if members.is_empty() {
                        div { class: "async-empty",
                            if a.members.is_empty() { "No principal holds this role." } else { "No member matches the filter." }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn EffectiveBody(
    access: Option<SubjectAccess>,
    loading: bool,
    error: Option<String>,
) -> Element {
    let mode = use_context::<AdminUi>().access.read().mode;
    let Some(a) = access else {
        return waiting(loading, error);
    };
    rsx! {
        div { class: "dt-wrap",
            table { class: "dt cl-table ad-table ad-effective",
                thead {
                    tr {
                        th { "Target" }
                        th { "Privilege" }
                        th { "Source" }
                        th { "Type" }
                    }
                }
                tbody {
                    for (i , e) in a.effective.iter().enumerate() {
                        tr { key: "{i}-{e.target}-{e.privilege.label()}",
                            td { class: "key", title: "{e.database}.{e.target}", Cut { text: e.target.clone() } }
                            td { "{e.privilege.label()}" }
                            td { Cut { text: e.source.clone() } }
                            td {
                                title: match e.via.as_ref() {
                                    Some(v) => format!("Inherited from role {v}"),
                                    None => "Granted to the role itself".to_string(),
                                },
                                "{e.kind_label()}"
                            }
                        }
                    }
                }
            }
            if a.effective.is_empty() {
                div { class: "async-empty",
                    if mode == AccessMode::Roles {
                        "This role grants no collection privilege. Administrative permissions are separate."
                    } else {
                        "No collection privilege is held. Administrative permissions are separate."
                    }
                }
            }
        }
    }
}
