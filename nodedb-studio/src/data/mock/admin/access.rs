//! Principals, roles, grants, and memberships, with the changes that edit them.

use crate::data::mock::admin::store::{ADMIN, AdminStore, DATABASE, missing};
use crate::models::admin::{AdminReview, ReviewLine};
use crate::models::admin_access::{
    AccessChange, AccessDirectory, AccessSubject, EffectiveGrant, Member, Membership, Mfa,
    Principal, PrincipalKind, PrincipalStatus, Privilege, Role, RoleGrant, SubjectAccess,
    sort_effective,
};
use crate::models::admin_audit::TargetType;
use crate::models::format::{date, epoch};
use crate::services::error::StudioError;

/// The privilege every access change needs.
const ACCESS_WRITE: &str = "auth.principal.write";

type Seed = (
    Vec<Principal>,
    Vec<Role>,
    Vec<RoleGrant>,
    Vec<(String, String, String)>,
);

fn principal(
    id: &str,
    name: &str,
    kind: PrincipalKind,
    description: &str,
    expiry: i64,
    created: i64,
) -> Principal {
    let user = kind == PrincipalKind::User;
    Principal {
        id: id.into(),
        name: name.into(),
        kind,
        status: PrincipalStatus::Active,
        description: description.into(),
        credential_expiry: expiry,
        created_at: created,
        password: user,
        mfa: if user {
            Mfa::NotRequired
        } else {
            Mfa::NotApplicable
        },
    }
}

fn role(name: &str, description: &str, parents: &[&str]) -> Role {
    Role {
        name: name.into(),
        description: description.into(),
        parents: parents.iter().map(|p| p.to_string()).collect(),
        created_at: epoch(2024, 11, 3, 10, 20, 0),
    }
}

fn grant(role: &str, collection: &str, privilege: Privilege) -> RoleGrant {
    RoleGrant {
        role: role.into(),
        database: DATABASE.into(),
        collection: collection.into(),
        privilege,
        granted_by: ADMIN.into(),
        granted_at: epoch(2025, 3, 18, 9, 30, 0),
    }
}

pub(super) fn seeded_access() -> Seed {
    let principals = vec![
        principal(
            "pr_01",
            "admin",
            PrincipalKind::User,
            "Cluster administrator",
            epoch(2026, 12, 31, 0, 0, 0),
            epoch(2024, 1, 15, 8, 0, 0),
        ),
        principal(
            "pr_02",
            "analyst",
            PrincipalKind::User,
            "Analytics analyst",
            epoch(2026, 8, 15, 0, 0, 0),
            epoch(2024, 11, 3, 10, 24, 17),
        ),
        principal(
            "pr_03",
            "ingest_bot",
            PrincipalKind::ServiceAccount,
            "Ingestion pipeline",
            epoch(2027, 1, 10, 0, 0, 0),
            epoch(2025, 2, 6, 14, 5, 0),
        ),
    ];
    let roles = vec![
        role(
            "cluster_admin",
            "Manages principals, config, and backups",
            &[],
        ),
        role(
            "analytics",
            "Reads the analytics collections",
            &["catalog_reader"],
        ),
        role("catalog_reader", "Reads the product catalog", &[]),
        role("ingest_writer", "Writes ingested rows", &[]),
        role("tenant_reader", "Reads the rows of one tenant", &[]),
        role("tenant_writer", "Writes the rows of one tenant", &[]),
    ];
    let grants = vec![
        grant("analytics", "sales", Privilege::Select),
        grant("analytics", "articles", Privilege::Select),
        grant("catalog_reader", "inventory", Privilege::Select),
        grant("ingest_writer", "sales", Privilege::Insert),
        grant("ingest_writer", "articles", Privilege::Insert),
        grant("tenant_reader", "sales", Privilege::Select),
        grant("tenant_writer", "sales", Privilege::Insert),
        grant("tenant_writer", "sales", Privilege::Update),
    ];
    let m = |p: &str, r: &str| (p.to_string(), r.to_string(), DATABASE.to_string());
    let memberships = vec![
        m("pr_01", "cluster_admin"),
        m("pr_02", "analytics"),
        m("pr_03", "ingest_writer"),
    ];
    (principals, roles, grants, memberships)
}

impl AdminStore {
    pub fn directory(&self) -> AccessDirectory {
        AccessDirectory {
            principals: self.principals.clone(),
            roles: self.roles.clone(),
            databases: self.databases.clone(),
            observed_at: self.clock,
        }
    }

    fn role_grants(&self, role: &str, database: &str) -> Vec<&RoleGrant> {
        self.grants
            .iter()
            .filter(|g| g.role == role && g.database == database)
            .collect()
    }

    /// Every role `role` inherits from, nearest first. A cycle stops at the
    /// first repeat.
    fn ancestors(&self, role: &str) -> Vec<String> {
        let mut seen: Vec<String> = Vec::new();
        let mut queue = vec![role.to_string()];
        while let Some(next) = queue.pop() {
            let parents = self
                .roles
                .iter()
                .find(|r| r.name == next)
                .map(|r| r.parents.clone())
                .unwrap_or_default();
            for p in parents {
                if p != role && !seen.contains(&p) {
                    seen.push(p.clone());
                    queue.push(p);
                }
            }
        }
        seen
    }

    /// What holding `role` on `database` grants: direct rows, then inherited.
    fn role_effective(&self, role: &str, database: &str) -> Vec<EffectiveGrant> {
        let row = |g: &RoleGrant, via: Option<&str>| EffectiveGrant {
            database: g.database.clone(),
            target: g.collection.clone(),
            privilege: g.privilege,
            source: role.to_string(),
            via: via.map(str::to_string),
        };
        let mut out: Vec<EffectiveGrant> = self
            .role_grants(role, database)
            .into_iter()
            .map(|g| row(g, None))
            .collect();
        for parent in self.ancestors(role) {
            for g in self.role_grants(&parent, database) {
                let held = out.iter().any(|e| {
                    e.target == g.collection && e.privilege == g.privilege && e.is_direct()
                });
                if !held {
                    out.push(row(g, Some(&parent)));
                }
            }
        }
        out
    }

    fn membership(&self, principal: &str, role: &str, database: &str) -> Membership {
        let mut covered: Vec<&str> = self
            .role_grants(role, database)
            .iter()
            .map(|g| g.collection.as_str())
            .collect();
        covered.sort_unstable();
        covered.dedup();
        Membership {
            principal: principal.to_string(),
            role: role.to_string(),
            database: database.to_string(),
            collections: covered.len() as u32,
        }
    }

    pub fn subject_access(&self, subject: &AccessSubject) -> Result<SubjectAccess, StudioError> {
        let mut out = SubjectAccess {
            subject: subject.clone(),
            memberships: Vec::new(),
            members: Vec::new(),
            grants: Vec::new(),
            effective: Vec::new(),
            observed_at: self.clock,
        };
        match subject {
            AccessSubject::Principal(id) => {
                if !self.principals.iter().any(|p| p.id == *id) {
                    return Err(missing("principal", id));
                }
                for (p, r, db) in self.memberships.iter().filter(|(p, _, _)| p == id) {
                    out.memberships.push(self.membership(p, r, db));
                    out.grants
                        .extend(self.role_grants(r, db).into_iter().cloned());
                    out.effective.extend(self.role_effective(r, db));
                }
            }
            AccessSubject::Role(name) => {
                if !self.roles.iter().any(|r| r.name == *name) {
                    return Err(missing("role", name));
                }
                for (p, _, db) in self.memberships.iter().filter(|(_, r, _)| r == name) {
                    let who = self.principals.iter().find(|x| x.id == *p);
                    out.members.push(Member {
                        principal: p.clone(),
                        name: who.map(|x| x.name.clone()).unwrap_or_else(|| p.clone()),
                        database: db.clone(),
                    });
                }
                out.grants
                    .extend(self.grants.iter().filter(|g| g.role == *name).cloned());
                out.effective.extend(self.role_effective(name, DATABASE));
            }
        }
        sort_effective(&mut out.effective);
        Ok(out)
    }

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

pub(super) fn plural(n: u32, word: &str) -> String {
    if n == 1 {
        word.to_string()
    } else {
        format!("{word}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyst_holds_two_direct_grants_and_one_inherited() {
        let s = AdminStore::seeded();
        let a = s
            .subject_access(&AccessSubject::Principal("pr_02".into()))
            .expect("analyst");
        let direct: Vec<_> = a
            .effective
            .iter()
            .filter(|e| e.is_direct())
            .map(|e| e.target.as_str())
            .collect();
        assert_eq!(direct, ["sales", "articles"]);
        let inherited = a.effective.iter().find(|e| !e.is_direct()).expect("one");
        assert_eq!(inherited.target, "inventory");
        assert_eq!(inherited.via.as_deref(), Some("catalog_reader"));
        assert_eq!(a.memberships[0].collections, 2);
    }

    #[test]
    fn the_admin_holds_no_collection_privilege() {
        let s = AdminStore::seeded();
        let a = s
            .subject_access(&AccessSubject::Principal("pr_01".into()))
            .expect("admin");
        assert!(a.effective.is_empty(), "no blanket access");
    }
}
