//! Previews and applies staged access changes: new principals and roles,
//! and role memberships added or removed.

use crate::data::mock::admin::access::principal;
use crate::data::mock::admin::store::AdminStore;
use crate::models::admin::{AdminReview, ReviewLine};
use crate::models::admin_access::{AccessChange, PrincipalKind, Role};
use crate::models::admin_audit::TargetType;
use crate::models::format::date;
use crate::services::error::StudioError;

/// The privilege every access change needs.
const ACCESS_WRITE: &str = "auth.principal.write";

impl AdminStore {
    fn name_of(&self, id: &str) -> String {
        self.principals
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| id.to_string())
    }

    fn roles_of(&self, principal: &str) -> Vec<String> {
        self.memberships
            .iter()
            .filter(|(p, _, _)| p == principal)
            .map(|(_, r, _)| r.clone())
            .collect()
    }

    fn privilege_lines(&self, principal: &str) -> Vec<String> {
        let mut out = Vec::new();
        for (_, r, db) in self.memberships.iter().filter(|(p, _, _)| p == principal) {
            for e in self.role_effective(r, db) {
                let line = format!(
                    "  - {}: {}.{}{}",
                    e.privilege.label().to_lowercase(),
                    e.database,
                    e.target,
                    e.via.as_deref().map(|_| " (inherited)").unwrap_or("")
                );
                if !out.contains(&line) {
                    out.push(line);
                }
            }
        }
        out
    }

    /// `Roles: ["analytics"]`, then the permission lines.
    fn state_lines(&self, principal: &str) -> Vec<String> {
        let roles: Vec<String> = self
            .roles_of(principal)
            .iter()
            .map(|r| format!("\"{r}\""))
            .collect();
        let mut out = vec![format!("Roles: [{}]", roles.join(", "))];
        let perms = self.privilege_lines(principal);
        if perms.is_empty() {
            out.push("Effective permissions: []".to_string());
        } else {
            out.push("Effective permissions:".to_string());
            out.extend(perms);
        }
        out
    }

    fn target_of(&self, principal: &str) -> String {
        let kind = self
            .principals
            .iter()
            .find(|p| p.id == principal)
            .map(|p| p.kind)
            .unwrap_or(PrincipalKind::User);
        let prefix = match kind {
            PrincipalKind::User => "user",
            PrincipalKind::ServiceAccount => "service",
        };
        format!("{prefix}:{}", self.name_of(principal))
    }

    pub(super) fn preview_access(&self, changes: &[AccessChange]) -> AdminReview {
        let mut lines = Vec::new();
        let mut notes = Vec::new();
        let mut warnings = Vec::new();
        let mut errors = Vec::new();
        if changes.is_empty() {
            errors.push("No change is staged.".to_string());
        }
        // Names created earlier in the same batch count as taken.
        let mut names: Vec<String> = self.principals.iter().map(|p| p.name.clone()).collect();
        let mut roles: Vec<String> = self.roles.iter().map(|r| r.name.clone()).collect();
        let mut held = self.memberships.clone();
        for c in changes {
            match c {
                AccessChange::CreatePrincipal {
                    name,
                    kind,
                    credential_expiry,
                    ..
                } => {
                    if names.contains(name) {
                        errors.push(format!("A principal named {name} exists."));
                    }
                    names.push(name.clone());
                    lines.push(ReviewLine::new(
                        format!("Principal {name}"),
                        None,
                        format!(
                            "{} · credential expires {}",
                            kind.label(),
                            date(*credential_expiry, 0)
                        ),
                    ));
                    notes.push(format!(
                        "{name} holds no role until you add one. Studio never sees its credential."
                    ));
                }
                AccessChange::CreateRole { name, .. } => {
                    if roles.contains(name) {
                        errors.push(format!("A role named {name} exists."));
                    }
                    roles.push(name.clone());
                    lines.push(ReviewLine::new(
                        format!("Role {name}"),
                        None,
                        "Created with no grants",
                    ));
                    notes.push(format!("Role {name} grants nothing until it has grants."));
                }
                AccessChange::AddMembership {
                    principal,
                    role,
                    database,
                } => {
                    let who = self.name_of(principal);
                    let key = (principal.clone(), role.clone(), database.clone());
                    if !self.principals.iter().any(|p| p.id == *principal) {
                        errors.push(format!("No principal {principal}."));
                    } else if !roles.contains(role) {
                        errors.push(format!("No role {role}."));
                    } else if held.contains(&key) {
                        errors.push(format!("{who} already holds {role} on {database}."));
                    } else {
                        let n = self.membership(principal, role, database).collections;
                        lines.push(ReviewLine::new(
                            format!("{who} · role {role} on {database}"),
                            Some("Not held"),
                            format!("Held · {n} {}", plural(n, "collection")),
                        ));
                        let adds: Vec<String> = self
                            .role_effective(role, database)
                            .iter()
                            .map(|e| {
                                format!("{} on {}.{}", e.privilege.label(), e.database, e.target)
                            })
                            .collect();
                        if !adds.is_empty() {
                            notes.push(format!("{who} gains {}.", adds.join(", ")));
                        }
                        held.push(key);
                    }
                }
                AccessChange::RemoveMembership {
                    principal,
                    role,
                    database,
                } => {
                    let who = self.name_of(principal);
                    let key = (principal.clone(), role.clone(), database.clone());
                    if let Some(i) = held.iter().position(|h| *h == key) {
                        held.remove(i);
                        lines.push(ReviewLine::new(
                            format!("{who} · role {role} on {database}"),
                            Some("Held"),
                            "Not held",
                        ));
                        if !held.iter().any(|(p, _, _)| p == principal) {
                            warnings.push(format!("{who} holds no role after this change."));
                        }
                    } else {
                        errors.push(format!("{who} does not hold {role} on {database}."));
                    }
                }
            }
        }
        notes.push("Applying records each change in the audit trail.".to_string());
        AdminReview {
            title: "Review access changes".to_string(),
            target: format!(
                "{} {}",
                changes.len(),
                plural(changes.len() as u32, "change")
            ),
            lines,
            notes,
            warnings,
            errors,
            permission: self.permission(ACCESS_WRITE),
            revision: self.rev.access,
            apply_label: if changes.len() == 1 {
                "Apply 1 change".to_string()
            } else {
                format!("Apply {} changes", changes.len())
            },
        }
    }

    pub(super) fn apply_access(&mut self, changes: &[AccessChange]) -> Result<String, StudioError> {
        let mut ids = Vec::new();
        for c in changes {
            let id = match c {
                AccessChange::CreatePrincipal {
                    name,
                    kind,
                    description,
                    credential_expiry,
                } => {
                    let id = format!("pr_{:02}", self.next_principal);
                    self.next_principal += 1;
                    self.principals.push(principal(
                        &id,
                        name,
                        *kind,
                        description,
                        *credential_expiry,
                        self.clock,
                    ));
                    let (label, prefix) = match kind {
                        PrincipalKind::User => ("Create user", "user"),
                        PrincipalKind::ServiceAccount => ("Create service account", "service"),
                    };
                    self.record(
                        label,
                        TargetType::Principal,
                        &format!("{prefix}:{name}"),
                        name,
                        vec!["Roles: []".to_string()],
                        vec![
                            "Roles: []".to_string(),
                            format!("Credential expires: {}", date(*credential_expiry, 0)),
                        ],
                    )
                }
                AccessChange::CreateRole { name, description } => {
                    self.roles.push(Role {
                        name: name.clone(),
                        description: description.clone(),
                        parents: Vec::new(),
                        created_at: self.clock,
                    });
                    self.record(
                        "Create role",
                        TargetType::Role,
                        &format!("role:{name}"),
                        name,
                        vec!["Grants: []".to_string()],
                        vec!["Grants: []".to_string()],
                    )
                }
                AccessChange::AddMembership {
                    principal,
                    role,
                    database,
                } => {
                    let before = self.state_lines(principal);
                    self.memberships
                        .push((principal.clone(), role.clone(), database.clone()));
                    let after = self.state_lines(principal);
                    let target = self.target_of(principal);
                    self.record(
                        "Grant role",
                        TargetType::Principal,
                        &target,
                        database,
                        before,
                        after,
                    )
                }
                AccessChange::RemoveMembership {
                    principal,
                    role,
                    database,
                } => {
                    let before = self.state_lines(principal);
                    self.memberships
                        .retain(|m| *m != (principal.clone(), role.clone(), database.clone()));
                    let after = self.state_lines(principal);
                    let target = self.target_of(principal);
                    self.record(
                        "Revoke role",
                        TargetType::Principal,
                        &target,
                        database,
                        before,
                        after,
                    )
                }
            };
            ids.push(id);
        }
        self.rev.access += 1;
        Ok(format!(
            "Applied {} access {}. Recorded as {}.",
            changes.len(),
            plural(changes.len() as u32, "change"),
            ids.join(", ")
        ))
    }
}

fn plural(n: u32, word: &str) -> String {
    if n == 1 {
        word.to_string()
    } else {
        format!("{word}s")
    }
}
