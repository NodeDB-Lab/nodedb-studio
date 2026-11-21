//! Access reads and changes: users, roles, grants.
//!
//! NodeDB 0.6.2 facts this file maps (server source and live checks, see
//! the tests here and in `admin.rs`):
//! - `SHOW USERS` lists the active users of every tenant to a superuser:
//!   `username, tenant_id, roles, is_superuser` (`t`/`f`). It has no id,
//!   description, expiry, creation time, or sign-in method.
//! - `SHOW ROLES` lists custom roles only: `name, tenant_id, parent,
//!   created_at` (seconds). Built-in roles are fixed and never listed.
//! - Role membership is not scoped to a database. A grant is: the server
//!   stores `collection:<db>:<tenant>:<name>`. Bare `SHOW PERMISSIONS` lists
//!   the caller's tenant's collection and function grants with the bare name
//!   only, so the database comes from `SHOW PERMISSIONS ON <name>` asked in
//!   each database. A tenant-wide grant shows only under `SHOW PERMISSIONS
//!   FOR <role>`, as `tenant:<id>`.
//! - Permission names are the server's closed set, `Read` to `Backup`.
//!   `Write` covers INSERT, UPDATE and DELETE.
//! - Access in order: the superuser flag, collection ownership, built-in
//!   roles (only those the user holds itself, not a custom role's parent),
//!   then explicit grants to `user:<name>` or to a role and its parents.
//! - Role and user names cannot be bound as `$n`: DDL takes identifiers as
//!   text, so `ident` validates them first.

use crate::models::admin::AdminChange;
use crate::models::admin::access::{
    AccessChange, AccessDirectory, EVERY, Mfa, Principal, PrincipalKind, PrincipalStatus, Role,
};
use crate::services::decode::{Row, Table, decode_rows};
use crate::services::error::StudioError;

use nodedb_client::NodeDbError;
use nodedb_types::error::ErrorDetails;

/// The roles the server has built in. `SHOW ROLES` never lists them.
/// `database_owner:<id>`, `database_editor:<id>` and `database_reader:<id>`
/// are built in too, one per database.
pub(in crate::services::nodedb) const BUILTIN_ROLES: [&str; 6] = [
    "readonly",
    "readwrite",
    "monitor",
    "tenant_admin",
    "cluster_admin",
    "superuser",
];

/// What a membership row shows: role membership applies in every database.
pub(in crate::services::nodedb) const NO_DATABASE: &str = "";

/// A name the server accepts as SQL text: ASCII letters, digits, and
/// underscores, 1 to 63 characters.
pub(in crate::services::nodedb) fn ident<'a>(
    name: &'a str,
    what: &str,
) -> Result<&'a str, StudioError> {
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
pub(in crate::services::nodedb) struct UserRow {
    pub name: String,
    pub tenant: u64,
    pub roles: Vec<String>,
    pub superuser: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::services::nodedb) struct RoleRow {
    pub name: String,
    pub tenant: u64,
    pub parent: Option<String>,
    pub created_at: i64,
}

/// One row of `SHOW PERMISSIONS`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::services::nodedb) struct PermRow {
    pub grantee: String,
    pub permission: String,
    pub target: String,
}

/// A stored grant and the database it binds. `None`: no statement tells
/// it, e.g. a function grant. `EVERY` for a tenant-wide grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::services::nodedb) struct Grant {
    pub grantee: String,
    pub permission: String,
    pub target: String,
    pub database: Option<String>,
}

/// Who `SHOW SESSION` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::services::nodedb) struct Caller {
    pub tenant: u64,
    /// A superuser or a tenant administrator.
    pub admin: bool,
    pub superuser: bool,
}

fn number(r: &Row<'_>, col: &str, what: &str) -> Result<i64, StudioError> {
    let v = r.field(col)?;
    v.parse().map_err(|_| StudioError::MalformedRows {
        reason: format!("{what} {col} `{v}` is not a number"),
    })
}

fn tenant_of(r: &Row<'_>, what: &str) -> Result<u64, StudioError> {
    u64::try_from(number(r, "tenant_id", what)?).map_err(|_| StudioError::MalformedRows {
        reason: format!("{what} tenant_id is negative"),
    })
}

fn flag(r: &Row<'_>, col: &str) -> Result<bool, StudioError> {
    Ok(matches!(r.field(col)?, "t" | "true"))
}

fn role_list(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

pub(in crate::services::nodedb) fn users(table: &Table) -> Result<Vec<UserRow>, StudioError> {
    decode_rows(
        table,
        &["username", "tenant_id", "roles", "is_superuser"],
        |r| {
            Ok(UserRow {
                name: r.field("username")?.to_string(),
                tenant: tenant_of(&r, "user")?,
                roles: role_list(r.field("roles")?),
                superuser: flag(&r, "is_superuser")?,
            })
        },
    )
}

pub(in crate::services::nodedb) fn roles(table: &Table) -> Result<Vec<RoleRow>, StudioError> {
    decode_rows(table, &["name", "tenant_id", "parent", "created_at"], |r| {
        Ok(RoleRow {
            name: r.field("name")?.to_string(),
            tenant: tenant_of(&r, "role")?,
            parent: Some(r.field("parent")?.to_string()).filter(|p| !p.is_empty()),
            created_at: number(&r, "created_at", "role")?,
        })
    })
}

/// The users and custom roles of `tenant`. A superuser reads every
/// tenant's, and a name can repeat across tenants.
pub(in crate::services::nodedb) fn of_tenant(
    users: Vec<UserRow>,
    roles: Vec<RoleRow>,
    tenant: u64,
) -> (Vec<UserRow>, Vec<RoleRow>) {
    (
        users.into_iter().filter(|u| u.tenant == tenant).collect(),
        roles.into_iter().filter(|r| r.tenant == tenant).collect(),
    )
}

/// The grant rows of `SHOW PERMISSIONS`. An owner row is left out:
/// ownership comes from `SHOW COLLECTIONS`.
pub(in crate::services::nodedb) fn permissions(table: &Table) -> Result<Vec<PermRow>, StudioError> {
    let rows = decode_rows(table, &["grantee", "permission", "target", "type"], |r| {
        Ok((
            r.field("type")? == "grant",
            PermRow {
                grantee: r.field("grantee")?.to_string(),
                permission: r.field("permission")?.to_string(),
                target: r.field("target")?.to_string(),
            },
        ))
    })?;
    Ok(rows
        .into_iter()
        .filter(|(g, _)| *g)
        .map(|(_, p)| p)
        .collect())
}

/// Whether `e` says a database refused the caller or no longer exists. A
/// read across the databases `SHOW DATABASES` lists skips such a database
/// and names it: a tenant administrator sees every database but opens only
/// the default one and those granted to it. Any other error fails the read.
pub(in crate::services::nodedb) fn refused(e: &StudioError) -> bool {
    if let StudioError::Auth(n) | StudioError::NotFound(n) = e {
        n.is_auth_denied() || matches!(n.details(), ErrorDetails::DatabaseNotFound { .. })
    } else {
        false
    }
}

/// The note for each database a read skipped, `(database, reason)`.
pub(in crate::services::nodedb) fn skipped_notes(
    skipped: &std::collections::BTreeMap<String, String>,
) -> Vec<String> {
    skipped
        .iter()
        .map(|(db, why)| {
            format!("Could not read database {db}: {why}. Grants and owned collections in it do not show here.")
        })
        .collect()
}

/// Database names from `SHOW DATABASES`.
pub(in crate::services::nodedb) fn database_names(
    table: &Table,
) -> Result<Vec<String>, StudioError> {
    decode_rows(table, &["name"], |r| Ok(r.field("name")?.to_string()))
}

/// `(collection, owner)` from `SHOW COLLECTIONS`. A row with no owner (an
/// array) is left out.
pub(in crate::services::nodedb) fn owners(
    table: &Table,
) -> Result<Vec<(String, String)>, StudioError> {
    let rows = decode_rows(table, &["name", "owner"], |r| {
        Ok((r.field("name")?.to_string(), r.field("owner")?.to_string()))
    })?;
    Ok(rows.into_iter().filter(|(_, o)| !o.is_empty()).collect())
}

/// The caller `SHOW SESSION` names.
pub(in crate::services::nodedb) fn session(table: &Table) -> Result<Caller, StudioError> {
    decode_rows(table, &["tenant_id", "roles", "is_superuser"], |r| {
        let admin = role_list(r.field("roles")?)
            .iter()
            .any(|x| x == "tenant_admin");
        let superuser = flag(&r, "is_superuser")?;
        Ok(Caller {
            tenant: tenant_of(&r, "session")?,
            admin: superuser || admin,
            superuser,
        })
    })?
    .into_iter()
    .next()
    .ok_or_else(|| StudioError::MalformedRows {
        reason: "SHOW SESSION returned no row".to_string(),
    })
}

/// Bare `SHOW PERMISSIONS` rows placed in their database. `per_db` holds
/// each database's `SHOW PERMISSIONS ON <name>` rows. A bare row no
/// database lists keeps no database. `tenant_wide` holds `SHOW PERMISSIONS
/// FOR <role>` rows: those on `tenant:<tenant>` apply everywhere.
pub(in crate::services::nodedb) fn placed(
    bare: &[PermRow],
    per_db: &[(String, Vec<PermRow>)],
    tenant_wide: &[PermRow],
    tenant: u64,
) -> Vec<Grant> {
    let mut out: Vec<Grant> = per_db
        .iter()
        .flat_map(|(db, rows)| {
            rows.iter().map(|p| Grant {
                grantee: p.grantee.clone(),
                permission: p.permission.clone(),
                target: p.target.clone(),
                database: Some(db.clone()),
            })
        })
        .collect();
    let mut claimed = vec![false; out.len()];
    for b in bare {
        let same = |g: &Grant| {
            g.grantee == b.grantee && g.permission == b.permission && g.target == b.target
        };
        match (0..claimed.len()).find(|&i| !claimed[i] && same(&out[i])) {
            Some(i) => claimed[i] = true,
            None => out.push(Grant {
                grantee: b.grantee.clone(),
                permission: b.permission.clone(),
                target: b.target.clone(),
                database: None,
            }),
        }
    }
    let ours = format!("tenant:{tenant}");
    out.extend(
        tenant_wide
            .iter()
            .filter(|p| p.target == ours)
            .map(|p| Grant {
                grantee: p.grantee.clone(),
                permission: p.permission.clone(),
                target: EVERY.to_string(),
                database: Some(EVERY.to_string()),
            }),
    );
    out
}

pub(in crate::services::nodedb) fn directory(
    users: &[UserRow],
    custom: &[RoleRow],
    databases: Vec<String>,
    tenant: Option<u64>,
    now: i64,
) -> AccessDirectory {
    let principals = users
        .iter()
        .map(|u| Principal {
            // The server lists no user id outside the caller's own session.
            id: u.name.clone(),
            name: u.name.clone(),
            kind: PrincipalKind::User,
            // `SHOW USERS` lists active users only.
            status: PrincipalStatus::Active,
            description: String::new(),
            credential_expiry: None,
            created_at: None,
            password: None,
            mfa: Mfa::NotReported,
        })
        .collect();
    let builtin = BUILTIN_ROLES.iter().map(|n| Role {
        name: (*n).to_string(),
        description: "Built-in role".to_string(),
        parents: vec![],
        created_at: None,
    });
    let custom = custom.iter().map(|r| Role {
        name: r.name.clone(),
        description: String::new(),
        parents: r.parent.iter().cloned().collect(),
        created_at: Some(r.created_at),
    });
    AccessDirectory {
        principals,
        roles: builtin.chain(custom).collect(),
        databases,
        tenant,
        observed_at: now,
    }
}

/// The access changes of `change`, or `None` for any other kind.
pub(in crate::services::nodedb) fn access_changes(change: &AdminChange) -> Option<&[AccessChange]> {
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
    use crate::models::admin::password::Secret;
    use crate::services::nodedb::admin::plan::{Planned, plan};

    fn table(cols: &[&str], rows: &[&[&str]]) -> Table {
        Table {
            columns: cols.iter().map(|s| (*s).to_string()).collect(),
            rows: rows
                .iter()
                .map(|r| r.iter().map(|s| (*s).to_string()).collect())
                .collect(),
        }
    }

    fn user(name: &str, roles: &[&str]) -> UserRow {
        UserRow {
            name: name.into(),
            tenant: 0,
            roles: roles.iter().map(|r| (*r).to_string()).collect(),
            superuser: false,
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
    fn users_read_the_role_line_the_tenant_and_the_superuser_flag() {
        let t = table(
            &["username", "tenant_id", "roles", "is_superuser"],
            &[&["a", "0", "readwrite, r2", "f"], &["b", "2", "", "t"]],
        );
        let u = users(&t).expect("decodes");
        assert_eq!(u[0].roles, ["readwrite", "r2"]);
        assert!(!u[0].superuser);
        assert!(u[1].roles.is_empty());
        assert!(u[1].superuser, "`t` is a superuser");
        assert_eq!(u[1].tenant, 2);
    }

    #[test]
    fn the_setting_fallback_is_an_error() {
        assert!(users(&table(&["setting"], &[&[""]])).is_err());
        assert!(roles(&table(&["setting"], &[&[""]])).is_err());
    }

    #[test]
    fn only_the_callers_tenant_is_kept() {
        let mut other = user("analyst", &["analyst"]);
        other.tenant = 2;
        let role = |tenant| RoleRow {
            name: "analyst".into(),
            tenant,
            parent: None,
            created_at: 5,
        };
        let (u, r) = of_tenant(
            vec![user("analyst", &["readonly"]), other],
            vec![role(0), role(2)],
            0,
        );
        assert_eq!(u.len(), 1);
        assert_eq!(u[0].roles, ["readonly"]);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].tenant, 0);
    }

    #[test]
    fn an_owner_row_is_not_a_grant() {
        let t = table(
            &["grantee", "permission", "target", "type"],
            &[
                &["ann", "ALL (owner)", "c", "ownership"],
                &["r", "Drop", "c", "grant"],
            ],
        );
        assert_eq!(permissions(&t).expect("decodes"), [perm("r", "Drop", "c")]);
    }

    #[test]
    fn a_grant_takes_the_database_that_lists_it() {
        let bare = [
            perm("r", "Read", "users"),
            perm("r", "Read", "users"),
            perm("r", "Execute", "score"),
        ];
        let per_db = [
            ("default".to_string(), vec![perm("r", "Read", "users")]),
            ("analytics".to_string(), vec![perm("r", "Read", "users")]),
        ];
        let wide = [perm("r", "Read", "tenant:0"), perm("r", "Read", "tenant:2")];
        let g = placed(&bare, &per_db, &wide, 0);
        let dbs: Vec<_> = g
            .iter()
            .map(|g| (g.target.as_str(), g.database.as_deref()))
            .collect();
        assert_eq!(
            dbs,
            [
                ("users", Some("default")),
                ("users", Some("analytics")),
                ("score", None),
                (EVERY, Some(EVERY)),
            ],
            "a function grant keeps no database; another tenant's grant is left out"
        );
    }

    #[test]
    fn the_session_names_the_tenant_and_whether_it_administers() {
        let t = |roles: &str, su: &str| {
            table(
                &["username", "tenant_id", "roles", "is_superuser"],
                &[&["me", "3", roles, su]],
            )
        };
        let c = session(&t("superuser", "t")).expect("reads");
        assert_eq!(c.tenant, 3);
        assert!(c.admin && c.superuser);
        let ta = session(&t("readonly, tenant_admin", "f")).expect("reads");
        assert!(ta.admin && !ta.superuser);
        assert!(!session(&t("readonly", "f")).expect("reads").admin);
    }

    #[test]
    fn only_a_refusal_or_a_missing_database_is_skipped() {
        use nodedb_types::error::ErrorCode;
        let skip = |e: NodeDbError| refused(&StudioError::from(e));
        assert!(skip(NodeDbError::authorization_denied(
            "permission denied for database"
        )));
        assert!(skip(NodeDbError::from_wire(
            ErrorCode::DATABASE_NOT_FOUND,
            "database 'x' does not exist"
        )));
        for other in [
            NodeDbError::authentication_failed("bad password"),
            NodeDbError::bad_request("syntax"),
            NodeDbError::internal("disk"),
            NodeDbError::collection_not_found("c"),
        ] {
            assert!(!skip(other), "only a refusal is skipped");
        }
        assert!(!refused(&StudioError::NotConnected));
        let skipped = [("db2".to_string(), "denied".to_string())].into();
        assert_eq!(
            skipped_notes(&skipped),
            [
                "Could not read database db2: denied. Grants and owned collections in it do not show here."
            ]
        );
    }

    #[test]
    fn a_user_reports_no_expiry_creation_password_or_second_factor() {
        let d = directory(&[user("ann", &[])], &[], vec![], Some(0), 1);
        let p = &d.principals[0];
        assert_eq!(
            (p.credential_expiry, p.created_at, p.password),
            (None, None, None)
        );
        assert_eq!(p.mfa, Mfa::NotReported);
        assert_eq!(d.tenant, Some(0));
        assert!(d.roles.iter().all(|r| r.created_at.is_none()));
        let custom = RoleRow {
            name: "r".into(),
            tenant: 0,
            parent: None,
            created_at: 1_795_258_002,
        };
        let d = directory(&[], &[custom], vec![], Some(0), 1);
        let r = d.roles.iter().find(|r| r.name == "r").expect("custom");
        assert_eq!(r.created_at, Some(1_795_258_002));
    }

    #[test]
    fn a_batch_sees_its_own_earlier_changes() {
        let users = [user("u", &[])];
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
        let sql = |i: usize| plans[i].statement.as_ref().map(|s| s.sql().to_string());
        assert_eq!(sql(0).as_deref(), Some("CREATE ROLE r"));
        assert_eq!(sql(1).as_deref(), Some("GRANT r TO u"));
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

    fn new_user(name: &str, password: Option<&str>) -> AccessChange {
        AccessChange::CreatePrincipal {
            name: name.into(),
            kind: PrincipalKind::User,
            description: String::new(),
            credential_expiry: 0,
            password: password.map(Secret::new),
        }
    }

    fn grant(user: &str, role: &str) -> AccessChange {
        AccessChange::AddMembership {
            principal: user.into(),
            role: role.into(),
            database: String::new(),
        }
    }

    fn sqls(plans: &[Planned]) -> Vec<String> {
        plans
            .iter()
            .filter_map(|p| p.statement.as_ref().map(|s| s.sql().to_string()))
            .collect()
    }

    #[test]
    fn a_user_is_created_with_its_password() {
        let plans = plan(&[new_user("ann", Some("pw'1"))], &[], &[]);
        assert!(plans[0].errors.is_empty(), "{:?}", plans[0].errors);
        assert_eq!(sqls(&plans), ["CREATE USER ann WITH PASSWORD 'pw''1'"]);
        assert!(
            plans[0].warnings.iter().any(|w| w.contains("readwrite")),
            "the default role is said"
        );
    }

    #[test]
    fn a_user_without_a_usable_password_or_name_is_refused() {
        for (change, users) in [
            (new_user("ann", None), vec![]),
            (new_user("ann", Some("")), vec![]),
            (new_user("ann", Some("a\0b")), vec![]),
            (new_user("a; DROP USER nodedb", Some("x")), vec![]),
            (new_user("ann", Some("x")), vec![user("ann", &[])]),
        ] {
            let plans = plan(&[change], &users, &[]);
            assert!(plans[0].statement.is_none());
            assert!(!plans[0].errors.is_empty());
            assert!(plans[0].errors.iter().all(|e| !e.contains("a\0b")));
        }
    }

    #[test]
    fn a_service_account_is_not_created() {
        let change = AccessChange::CreatePrincipal {
            name: "bot".into(),
            kind: PrincipalKind::ServiceAccount,
            description: String::new(),
            credential_expiry: 0,
            password: None,
        };
        let plans = plan(&[change], &[], &[]);
        assert!(plans[0].statement.is_none());
        assert!(!plans[0].errors.is_empty());
    }

    #[test]
    fn a_staged_role_replaces_the_default_role_instead_of_joining_it() {
        let plans = plan(
            &[
                new_user("ann", Some("pw")),
                grant("ann", "readonly"),
                grant("ann", "monitor"),
            ],
            &[],
            &[],
        );
        assert!(plans.iter().all(|p| p.errors.is_empty()));
        assert_eq!(
            sqls(&plans),
            [
                "CREATE USER ann WITH PASSWORD 'pw' ROLE readonly",
                "GRANT monitor TO ann"
            ]
        );
        assert!(plans[0].warnings.iter().all(|w| !w.contains("`readwrite`")));
    }

    #[test]
    fn a_role_created_after_the_user_cannot_replace_the_default_role() {
        let plans = plan(
            &[
                new_user("ann", Some("pw")),
                AccessChange::CreateRole {
                    name: "later".into(),
                    description: String::new(),
                },
                grant("ann", "later"),
            ],
            &[],
            &[],
        );
        assert!(plans[0].statement.is_none());
        assert!(!plans[0].errors.is_empty());
    }

    #[test]
    fn a_role_created_before_the_user_goes_in_the_create() {
        let plans = plan(
            &[
                AccessChange::CreateRole {
                    name: "auditor".into(),
                    description: String::new(),
                },
                new_user("ann", Some("pw")),
                grant("ann", "auditor"),
            ],
            &[],
            &[],
        );
        assert_eq!(
            sqls(&plans),
            [
                "CREATE ROLE auditor",
                "CREATE USER ann WITH PASSWORD 'pw' ROLE auditor"
            ]
        );
    }
}
