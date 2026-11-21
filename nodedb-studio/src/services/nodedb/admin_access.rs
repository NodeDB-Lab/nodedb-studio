//! Access reads and changes: users, roles, grants.
//!
//! NodeDB 0.6.2 facts this file maps (checked live, see the tests in
//! `admin.rs`):
//! - `SHOW USERS` lists `username, tenant_id, roles, is_superuser`. It has
//!   no id, description, expiry, creation time, or status.
//! - `SHOW ROLES` lists custom roles only: `name, tenant_id, parent,
//!   created_at`. Built-in roles are fixed and never listed.
//! - Role membership and grants are not scoped to a database.
//! - `SHOW PERMISSIONS` reports `Read` or `Write` only. `Write` covers
//!   INSERT, UPDATE and DELETE.
//! - Role and user names cannot be bound as `$n`: DDL takes identifiers as
//!   text, so `ident` validates them first.

use std::collections::HashSet;

use crate::models::admin::{AdminChange, ReviewLine};
use crate::models::admin_access::{
    AccessChange, AccessDirectory, AccessSubject, EffectiveGrant, Member, Membership, Mfa,
    Principal, PrincipalKind, PrincipalStatus, Privilege, Role, RoleGrant, SubjectAccess,
};
use crate::services::decode::{Table, decode_rows};
use crate::services::error::StudioError;

use nodedb_client::NodeDbError;

/// The roles the server has built in. `SHOW ROLES` never lists them.
pub(super) const BUILTIN_ROLES: [&str; 6] = [
    "readonly",
    "readwrite",
    "monitor",
    "tenant_admin",
    "cluster_admin",
    "superuser",
];

/// What the membership and grant rows show where the server reports no
/// database: role membership and grants are not database-scoped.
pub(super) const NO_DATABASE: &str = "";

/// A name the server accepts as SQL text: ASCII letters, digits, and
/// underscores, 1 to 63 characters.
pub(super) fn ident<'a>(name: &'a str, what: &str) -> Result<&'a str, StudioError> {
    let ok = !name.is_empty()
        && name.len() <= 63
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if ok {
        Ok(name)
    } else {
        Err(StudioError::from(NodeDbError::bad_request(format!(
            "{what} `{name}` cannot be used in a statement: use letters, digits and underscores, up to 63 characters"
        ))))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UserRow {
    pub name: String,
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RoleRow {
    pub name: String,
    pub parent: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PermRow {
    pub grantee: String,
    pub permission: String,
    pub target: String,
}

pub(super) fn users(table: &Table) -> Result<Vec<UserRow>, StudioError> {
    decode_rows(table, &["username", "roles"], |r| {
        Ok(UserRow {
            name: r.field("username")?.to_string(),
            roles: r
                .field("roles")?
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect(),
        })
    })
}

pub(super) fn roles(table: &Table) -> Result<Vec<RoleRow>, StudioError> {
    decode_rows(table, &["name", "parent", "created_at"], |r| {
        Ok(RoleRow {
            name: r.field("name")?.to_string(),
            parent: Some(r.field("parent")?.to_string()).filter(|p| !p.is_empty()),
            created_at: r
                .field("created_at")?
                .parse()
                .map_err(|_| StudioError::MalformedRows {
                    reason: format!(
                        "role created_at `{}` is not a number",
                        r.field("created_at").unwrap_or_default()
                    ),
                })?,
        })
    })
}

pub(super) fn permissions(table: &Table) -> Result<Vec<PermRow>, StudioError> {
    decode_rows(table, &["grantee", "permission", "target"], |r| {
        Ok(PermRow {
            grantee: r.field("grantee")?.to_string(),
            permission: r.field("permission")?.to_string(),
            target: r.field("target")?.to_string(),
        })
    })
}

/// Database names from `SHOW DATABASES`.
pub(super) fn database_names(table: &Table) -> Result<Vec<String>, StudioError> {
    decode_rows(table, &["name"], |r| Ok(r.field("name")?.to_string()))
}

/// Whether `SHOW SESSION` names a superuser or a tenant administrator.
pub(super) fn can_administer(table: &Table) -> Result<bool, StudioError> {
    let rows = decode_rows(table, &["roles", "is_superuser"], |r| {
        let superuser = matches!(r.field("is_superuser")?, "t" | "true");
        let admin = r
            .field("roles")?
            .split(',')
            .any(|x| x.trim() == "tenant_admin");
        Ok(superuser || admin)
    })?;
    Ok(rows.into_iter().any(|b| b))
}

pub(super) fn directory(
    users: &[UserRow],
    custom: &[RoleRow],
    databases: Vec<String>,
    now: i64,
) -> AccessDirectory {
    let principals = users
        .iter()
        .map(|u| Principal {
            // The server lists no user id outside the caller's own session.
            id: u.name.clone(),
            name: u.name.clone(),
            kind: PrincipalKind::User,
            status: PrincipalStatus::Active,
            description: String::new(),
            credential_expiry: 0,
            created_at: 0,
            password: true,
            mfa: Mfa::NotRequired,
        })
        .collect();
    let builtin = BUILTIN_ROLES.iter().map(|n| Role {
        name: (*n).to_string(),
        description: "Built-in role".to_string(),
        parents: vec![],
        created_at: 0,
    });
    let custom = custom.iter().map(|r| Role {
        name: r.name.clone(),
        description: String::new(),
        parents: r.parent.iter().cloned().collect(),
        created_at: r.created_at,
    });
    AccessDirectory {
        principals,
        roles: builtin.chain(custom).collect(),
        databases,
        observed_at: now,
    }
}

/// The privileges one permission name holds. `Write` is all three writes.
/// Create, Drop, Alter and the rest have no `Privilege`, so they are left out.
fn privileges(permission: &str) -> Vec<Privilege> {
    match permission {
        "Read" => vec![Privilege::Select],
        "Write" => vec![Privilege::Insert, Privilege::Update, Privilege::Delete],
        _ => vec![],
    }
}

/// `role` and the parents it inherits from, nearest first.
fn chain(role: &str, custom: &[RoleRow]) -> Vec<String> {
    let mut out = vec![role.to_string()];
    let mut seen: HashSet<&str> = HashSet::from([role]);
    let mut at = role;
    while let Some(parent) = custom
        .iter()
        .find(|r| r.name == at)
        .and_then(|r| r.parent.as_deref())
    {
        if !seen.insert(parent) {
            break;
        }
        out.push(parent.to_string());
        at = parent;
    }
    out
}

/// The rows a built-in role grants without a stored grant.
fn builtin_privileges(role: &str) -> Vec<Privilege> {
    match role {
        "readonly" => vec![Privilege::Select],
        "readwrite" => vec![
            Privilege::Select,
            Privilege::Insert,
            Privilege::Update,
            Privilege::Delete,
        ],
        _ => vec![],
    }
}

fn role_grants(role: &str, perms: &[PermRow]) -> Vec<RoleGrant> {
    let mut out = Vec::new();
    for p in perms.iter().filter(|p| p.grantee == role) {
        for privilege in privileges(&p.permission) {
            out.push(RoleGrant {
                role: role.to_string(),
                database: NO_DATABASE.to_string(),
                collection: p.target.clone(),
                privilege,
                granted_by: String::new(),
                granted_at: 0,
            });
        }
    }
    for privilege in builtin_privileges(role) {
        out.push(RoleGrant {
            role: role.to_string(),
            database: NO_DATABASE.to_string(),
            collection: "*".to_string(),
            privilege,
            granted_by: String::new(),
            granted_at: 0,
        });
    }
    out
}

pub(super) fn subject_access(
    subject: &AccessSubject,
    users: &[UserRow],
    custom: &[RoleRow],
    perms: &[PermRow],
    now: i64,
) -> Result<SubjectAccess, StudioError> {
    let unknown = |kind: &'static str, id: &str| StudioError::UnknownObject {
        kind,
        id: id.to_string(),
    };
    let (held, memberships, members) = match subject {
        AccessSubject::Principal(id) => {
            let user = users
                .iter()
                .find(|u| &u.name == id)
                .ok_or_else(|| unknown("principal", id))?;
            let memberships = user
                .roles
                .iter()
                .map(|r| Membership {
                    principal: user.name.clone(),
                    role: r.clone(),
                    database: NO_DATABASE.to_string(),
                    collections: u32::try_from(
                        role_grants(r, perms)
                            .iter()
                            .map(|g| g.collection.as_str())
                            .collect::<HashSet<_>>()
                            .len(),
                    )
                    .unwrap_or(u32::MAX),
                })
                .collect();
            (user.roles.clone(), memberships, vec![])
        }
        AccessSubject::Role(name) => {
            if !BUILTIN_ROLES.contains(&name.as_str()) && !custom.iter().any(|r| &r.name == name) {
                return Err(unknown("role", name));
            }
            let members = users
                .iter()
                .filter(|u| u.roles.iter().any(|r| r == name))
                .map(|u| Member {
                    principal: u.name.clone(),
                    name: u.name.clone(),
                    database: NO_DATABASE.to_string(),
                })
                .collect();
            (vec![name.clone()], vec![], members)
        }
    };
    let mut grants = Vec::new();
    let mut effective = Vec::new();
    let mut listed: HashSet<String> = HashSet::new();
    for source in &held {
        for (depth, role) in chain(source, custom).iter().enumerate() {
            let via = (depth > 0).then(|| role.clone());
            for g in role_grants(role, perms) {
                effective.push(EffectiveGrant {
                    database: g.database.clone(),
                    target: g.collection.clone(),
                    privilege: g.privilege,
                    source: source.clone(),
                    via: via.clone(),
                });
                if listed.insert(format!(
                    "{}/{}/{}",
                    g.role,
                    g.collection,
                    g.privilege.label()
                )) {
                    grants.push(g);
                }
            }
        }
    }
    if let AccessSubject::Principal(id) = subject {
        // A grant made to the user itself, not through a role.
        let own = format!("user:{id}");
        for p in perms
            .iter()
            .filter(|p| p.grantee == own || p.grantee == *id)
        {
            for privilege in privileges(&p.permission) {
                effective.push(EffectiveGrant {
                    database: NO_DATABASE.to_string(),
                    target: p.target.clone(),
                    privilege,
                    source: id.clone(),
                    via: None,
                });
            }
        }
    }
    Ok(SubjectAccess {
        subject: subject.clone(),
        memberships,
        members,
        grants,
        effective,
        observed_at: now,
    })
}

/// What one access change does: its review line, any errors and warnings,
/// and the statement that applies it. `users` and `roles` are the state
/// after the earlier changes of the same batch.
pub(super) struct Planned {
    pub line: ReviewLine,
    pub statement: Option<String>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

pub(super) fn plan(
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
    changes
        .iter()
        .map(|c| plan_one(c, &mut users, &mut roles))
        .collect()
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
        AccessChange::CreatePrincipal { name, .. } => failed(
            format!("Principal {name}"),
            "created",
            "NodeDB creates a user with a password and this change carries none. Studio cannot create the user until the change has a password field."
                .to_string(),
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
                warnings.push("NodeDB does not store a role description. It is dropped.".to_string());
            }
            Planned {
                line: ReviewLine::new(label, None, "created"),
                statement: Some(format!("CREATE ROLE {name}")),
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
                return failed(label, "held", format!("`{principal}` holds `{role}` already."));
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
                statement: Some(format!("GRANT {role} TO {principal}")),
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
                return failed(label, "not held", format!("`{principal}` does not hold `{role}`."));
            }
            user.roles.retain(|r| r != role);
            Planned {
                line: ReviewLine::new(label, Some("held"), "not held"),
                statement: Some(format!("REVOKE {role} FROM {principal}")),
                errors: vec![],
                warnings: vec![],
            }
        }
    }
}

/// The access changes of `change`, or `None` for any other kind.
pub(super) fn access_changes(change: &AdminChange) -> Option<&[AccessChange]> {
    match change {
        AdminChange::Access(c) => Some(c),
        AdminChange::Policy(_)
        | AdminChange::Backup(_)
        | AdminChange::Config(_)
        | AdminChange::Credential(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(cols: &[&str], rows: &[&[&str]]) -> Table {
        Table {
            columns: cols.iter().map(|s| (*s).to_string()).collect(),
            rows: rows
                .iter()
                .map(|r| r.iter().map(|s| (*s).to_string()).collect())
                .collect(),
        }
    }

    fn role(name: &str, parent: Option<&str>) -> RoleRow {
        RoleRow {
            name: name.into(),
            parent: parent.map(str::to_string),
            created_at: 5,
        }
    }

    fn perm(grantee: &str, permission: &str, target: &str) -> PermRow {
        PermRow {
            grantee: grantee.into(),
            permission: permission.into(),
            target: target.into(),
        }
    }

    #[test]
    fn identifiers_take_letters_digits_and_underscores_only() {
        assert!(ident("studio_probe_a_r1", "role").is_ok());
        for bad in ["", "a b", "a;b", "a-b", "a'b", "é", &"x".repeat(64)] {
            assert!(ident(bad, "role").is_err(), "{bad}");
        }
    }

    #[test]
    fn users_split_the_role_line() {
        let t = table(
            &["username", "tenant_id", "roles", "is_superuser"],
            &[&["a", "0", "readwrite, r2", "f"], &["b", "0", "", "f"]],
        );
        let u = users(&t).expect("decodes");
        assert_eq!(u[0].roles, ["readwrite", "r2"]);
        assert!(u[1].roles.is_empty());
    }

    #[test]
    fn the_setting_fallback_is_an_error() {
        assert!(users(&table(&["setting"], &[&[""]])).is_err());
        assert!(roles(&table(&["setting"], &[&[""]])).is_err());
    }

    #[test]
    fn a_role_chain_walks_parents_and_stops_on_a_cycle() {
        let r = [
            role("a", Some("b")),
            role("b", Some("c")),
            role("c", Some("a")),
        ];
        assert_eq!(chain("a", &r), ["a", "b", "c"]);
        assert_eq!(chain("zzz", &r), ["zzz"]);
    }

    #[test]
    fn write_is_three_privileges_and_other_permissions_are_left_out() {
        assert_eq!(privileges("Read"), [Privilege::Select]);
        assert_eq!(privileges("Write").len(), 3);
        assert!(privileges("Create").is_empty());
    }

    #[test]
    fn inherited_grants_name_their_parent() {
        let users = [UserRow {
            name: "u".into(),
            roles: vec!["child".into()],
        }];
        let custom = [role("child", Some("parent")), role("parent", None)];
        let perms = [perm("parent", "Read", "orders")];
        let a = subject_access(
            &AccessSubject::Principal("u".into()),
            &users,
            &custom,
            &perms,
            9,
        )
        .expect("reads");
        assert_eq!(a.effective.len(), 1);
        assert_eq!(a.effective[0].source, "child");
        assert_eq!(a.effective[0].via.as_deref(), Some("parent"));
        assert_eq!(a.memberships[0].collections, 0);
        assert!(matches!(
            subject_access(
                &AccessSubject::Principal("zz".into()),
                &users,
                &custom,
                &perms,
                9
            ),
            Err(StudioError::UnknownObject { .. })
        ));
    }

    #[test]
    fn a_role_lists_its_members() {
        let users = [
            UserRow {
                name: "u".into(),
                roles: vec!["r".into()],
            },
            UserRow {
                name: "v".into(),
                roles: vec![],
            },
        ];
        let a = subject_access(
            &AccessSubject::Role("r".into()),
            &users,
            &[role("r", None)],
            &[],
            1,
        )
        .expect("reads");
        assert_eq!(a.members.len(), 1);
        assert_eq!(a.members[0].principal, "u");
    }

    #[test]
    fn a_batch_sees_its_own_earlier_changes() {
        let users = [UserRow {
            name: "u".into(),
            roles: vec![],
        }];
        let plans = plan(
            &[
                AccessChange::CreateRole {
                    name: "r".into(),
                    description: String::new(),
                },
                AccessChange::AddMembership {
                    principal: "u".into(),
                    role: "r".into(),
                    database: "analytics".into(),
                },
                AccessChange::AddMembership {
                    principal: "u".into(),
                    role: "r".into(),
                    database: String::new(),
                },
            ],
            &users,
            &[],
        );
        assert_eq!(plans[0].statement.as_deref(), Some("CREATE ROLE r"));
        assert_eq!(plans[1].statement.as_deref(), Some("GRANT r TO u"));
        assert_eq!(plans[1].warnings.len(), 1, "the database scope is not kept");
        assert!(plans[2].statement.is_none(), "already held");
    }

    #[test]
    fn a_hostile_name_never_reaches_a_statement() {
        let plans = plan(
            &[AccessChange::CreateRole {
                name: "r; DROP USER nodedb".into(),
                description: String::new(),
            }],
            &[],
            &[],
        );
        assert!(plans[0].statement.is_none());
        assert!(!plans[0].errors.is_empty());
    }

    #[test]
    fn a_user_needs_a_password_so_creating_one_is_refused() {
        let plans = plan(
            &[AccessChange::CreatePrincipal {
                name: "x".into(),
                kind: PrincipalKind::User,
                description: String::new(),
                credential_expiry: 0,
            }],
            &[],
            &[],
        );
        assert!(plans[0].statement.is_none());
    }

    #[test]
    fn administering_needs_superuser_or_tenant_admin() {
        let t = |roles: &str, su: &str| table(&["roles", "is_superuser"], &[&[roles, su]]);
        assert!(can_administer(&t("superuser", "t")).expect("reads"));
        assert!(can_administer(&t("readonly, tenant_admin", "f")).expect("reads"));
        assert!(!can_administer(&t("readonly", "f")).expect("reads"));
    }
}
