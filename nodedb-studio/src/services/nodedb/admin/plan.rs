//! Plans access changes: each change becomes a review line, its errors and
//! warnings, and the statement that applies it.

use std::collections::HashSet;

use crate::models::admin::ReviewLine;
use crate::models::admin::access::AccessChange;
use crate::services::nodedb::admin::access::{BUILTIN_ROLES, RoleRow, UserRow, ident};
use crate::services::nodedb::admin::user::{NewUser, Statement, plan_create};

/// What one access change does: its review line, any errors and warnings,
/// and the statement that applies it. `users` and `roles` are the state
/// after the earlier changes of the same batch.
pub(in crate::services::nodedb) struct Planned {
    pub line: ReviewLine,
    pub statement: Option<Statement>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

pub(in crate::services::nodedb) fn plan(
    changes: &[AccessChange],
    users: &[UserRow],
    custom: &[RoleRow],
) -> Vec<Planned> {
    let mut users: Vec<UserRow> = users.to_vec();
    let mut roles: Vec<String> = BUILTIN_ROLES
        .iter()
        .map(|r| (*r).to_string())
        .chain(custom.iter().map(|r| r.name.clone()))
        .collect();
    // Grants a `CREATE USER ... ROLE` already covers run no statement.
    let mut folded = HashSet::new();
    let mut out = Vec::with_capacity(changes.len());
    for (at, c) in changes.iter().enumerate() {
        let mut planned = match c {
            AccessChange::CreatePrincipal {
                name,
                kind,
                description,
                credential_expiry,
                password,
            } => plan_create(
                NewUser {
                    name,
                    kind: *kind,
                    description,
                    credential_expiry: *credential_expiry,
                    password: password.as_ref(),
                },
                (at, changes),
                &mut users,
                &roles,
                &mut folded,
            ),
            AccessChange::CreateRole { .. }
            | AccessChange::AddMembership { .. }
            | AccessChange::RemoveMembership { .. } => plan_one(c, &mut users, &mut roles),
        };
        if folded.contains(&at) {
            planned.statement = None;
        }
        out.push(planned);
    }
    out
}

fn failed(label: String, after: &str, error: String) -> Planned {
    Planned {
        line: ReviewLine::new(label, None, after),
        statement: None,
        errors: vec![error],
        warnings: vec![],
    }
}

fn plan_one(change: &AccessChange, users: &mut [UserRow], roles: &mut Vec<String>) -> Planned {
    match change {
        // `plan` sends it to `plan_create`.
        AccessChange::CreatePrincipal { name, .. } => failed(
            format!("Principal {name}"),
            "created",
            "Studio planned this change in the wrong place.".to_string(),
        ),
        AccessChange::CreateRole { name, description } => {
            let label = format!("Role {name}");
            if let Err(e) = ident(name, "role") {
                return failed(label, "created", e.to_string());
            }
            if roles.iter().any(|r| r == name) {
                return failed(label, "created", format!("Role `{name}` exists."));
            }
            roles.push(name.clone());
            let mut warnings = vec![];
            if !description.trim().is_empty() {
                warnings
                    .push("NodeDB does not store a role description. It is dropped.".to_string());
            }
            Planned {
                line: ReviewLine::new(label, None, "created"),
                statement: Some(Statement::plain(format!("CREATE ROLE {name}"))),
                errors: vec![],
                warnings,
            }
        }
        AccessChange::AddMembership {
            principal,
            role,
            database,
        } => {
            let label = format!("Role {role} for {principal}");
            let checked = ident(principal, "principal").and(ident(role, "role"));
            if let Err(e) = checked {
                return failed(label, "held", e.to_string());
            }
            let Some(user) = users.iter_mut().find(|u| &u.name == principal) else {
                return failed(label, "held", format!("No principal `{principal}`."));
            };
            if !roles.iter().any(|r| r == role) {
                return failed(label, "held", format!("No role `{role}`."));
            }
            if user.roles.iter().any(|r| r == role) {
                return failed(
                    label,
                    "held",
                    format!("`{principal}` holds `{role}` already."),
                );
            }
            user.roles.push(role.clone());
            let mut warnings = vec![];
            if !database.is_empty() {
                warnings.push(format!(
                    "NodeDB roles are not scoped to a database. `{role}` applies on every database, not only `{database}`."
                ));
            }
            Planned {
                line: ReviewLine::new(label, Some("not held"), "held"),
                statement: Some(Statement::plain(format!("GRANT {role} TO {principal}"))),
                errors: vec![],
                warnings,
            }
        }
        AccessChange::RemoveMembership {
            principal, role, ..
        } => {
            let label = format!("Role {role} for {principal}");
            let checked = ident(principal, "principal").and(ident(role, "role"));
            if let Err(e) = checked {
                return failed(label, "not held", e.to_string());
            }
            let Some(user) = users.iter_mut().find(|u| &u.name == principal) else {
                return failed(label, "not held", format!("No principal `{principal}`."));
            };
            if !user.roles.iter().any(|r| r == role) {
                return failed(
                    label,
                    "not held",
                    format!("`{principal}` does not hold `{role}`."),
                );
            }
            user.roles.retain(|r| r != role);
            Planned {
                line: ReviewLine::new(label, Some("held"), "not held"),
                statement: Some(Statement::plain(format!("REVOKE {role} FROM {principal}"))),
                errors: vec![],
                warnings: vec![],
            }
        }
    }
}
