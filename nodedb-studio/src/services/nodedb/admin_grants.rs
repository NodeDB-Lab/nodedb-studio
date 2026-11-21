//! What one principal or role holds: its memberships or members, the
//! grants of the roles involved, and the access in effect.
//!
//! The order follows the server's `PermissionStore::check`: the superuser
//! flag, collection ownership, built-in roles (only those the user holds
//! itself: a custom role's built-in parent passes on its stored grants
//! only), then explicit grants to `user:<name>`, to a role, or to a parent.

use std::collections::HashSet;

use crate::models::admin_access::{
    AccessSubject, EVERY, EffectiveGrant, HOLDER_DATABASES, Member, Membership, Privilege,
    RoleGrant, SubjectAccess, USER_DATABASES,
};
use crate::services::error::StudioError;
use crate::services::nodedb::admin_access::{Grant, NO_DATABASE, RoleRow, UserRow};

/// What the access lists leave out because NodeDB 0.6.2 reports it nowhere.
const USER_TENANT_GRANTS: &str = "NodeDB 0.6.2 does not list a tenant-wide grant made to a user itself (GRANT … ON TENANT … TO <user>). Such a grant does not show here.";
/// The server lists no database grant, and a user who is not a superuser
/// opens only the default database and the databases granted to it
/// (`session_auth/identity.rs`, `authorization/service.rs`).
const USER_DATABASE_GRANTS: &str = "NodeDB 0.6.2 does not list database grants (GRANT … ON DATABASE … TO <user>). This user opens the default database and each database granted to it, and no other. A row in another database applies only if such a grant exists.";
const HOLDER_DATABASE_GRANTS: &str = "NodeDB 0.6.2 does not list database grants (GRANT … ON DATABASE … TO <user>). A holder who is not a superuser opens the default database and each database granted to it, and no other.";

/// The privileges one server permission name holds. The server's set is
/// closed, so any other name is an error, never a silent drop.
fn privileges(permission: &str) -> Result<Vec<Privilege>, StudioError> {
    Ok(match permission {
        "Read" => vec![Privilege::Select],
        "Write" => vec![Privilege::Insert, Privilege::Update, Privilege::Delete],
        "Create" => vec![Privilege::Create],
        "Drop" => vec![Privilege::Drop],
        "Alter" => vec![Privilege::Alter],
        "Admin" => vec![Privilege::Admin],
        "Monitor" => vec![Privilege::Monitor],
        "Execute" => vec![Privilege::Execute],
        "Backup" => vec![Privilege::Backup],
        other => {
            return Err(StudioError::MalformedRows {
                reason: format!(
                    "permission `{other}` is not one NodeDB 0.6.2 defines, so Studio cannot show what it allows"
                ),
            });
        }
    })
}

/// What a built-in role holds on every collection without a stored grant,
/// and the database it covers (`EVERY`, or one database by id). From the
/// server's `role_grants_permission`. `None` for a custom role.
/// `subject_access` narrows `EVERY` to the databases the holder can open.
fn builtin(role: &str) -> Option<(String, Vec<Privilege>)> {
    use Privilege::{All, Create, Delete, Execute, Insert, Monitor, Select, Update};
    let every = |p: Vec<Privilege>| Some((EVERY.to_string(), p));
    match role {
        "superuser" | "tenant_admin" => every(vec![All]),
        "readwrite" => every(vec![Select, Insert, Update, Delete, Execute]),
        "readonly" => every(vec![Select, Execute]),
        "monitor" => every(vec![Monitor]),
        // Cluster operations only: no right on any collection.
        "cluster_admin" => every(vec![]),
        _ => {
            let (kind, id) = role.split_once(':')?;
            id.parse::<u64>().ok()?;
            let privileges = match kind {
                "database_owner" => vec![All],
                "database_editor" => vec![Select, Insert, Update, Delete, Create, Execute],
                "database_reader" => vec![Select, Execute],
                _ => return None,
            };
            Some((format!("database id {id}"), privileges))
        }
    }
}

/// Whether `name` is a role of this tenant: built in, or listed.
fn is_role(name: &str, custom: &[RoleRow]) -> bool {
    builtin(name).is_some() || custom.iter().any(|r| r.name == name)
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

fn unknown(kind: &'static str, id: &str) -> StudioError {
    StudioError::UnknownObject {
        kind,
        id: id.to_string(),
    }
}

/// Every grantee whose grants reach `subject`: the user itself
/// (`user:<name>`), the roles it holds, and their parents.
pub(super) fn grantees(
    subject: &AccessSubject,
    users: &[UserRow],
    custom: &[RoleRow],
) -> Result<HashSet<String>, StudioError> {
    match subject {
        AccessSubject::Principal(id) => {
            let user = users
                .iter()
                .find(|u| &u.name == id)
                .ok_or_else(|| unknown("principal", id))?;
            let mut out = HashSet::from([format!("user:{id}")]);
            for r in &user.roles {
                out.extend(chain(r, custom));
            }
            Ok(out)
        }
        AccessSubject::Role(name) if is_role(name, custom) => {
            Ok(chain(name, custom).into_iter().collect())
        }
        AccessSubject::Role(name) => Err(unknown("role", name)),
    }
}

/// The grants stored for `role` itself.
fn stored(role: &str, grants: &[Grant]) -> Result<Vec<RoleGrant>, StudioError> {
    let mut out = Vec::new();
    for g in grants.iter().filter(|g| g.grantee == role) {
        for privilege in privileges(&g.permission)? {
            out.push(RoleGrant {
                role: role.to_string(),
                database: g.database.clone(),
                collection: g.target.clone(),
                privilege,
                granted_by: None,
                granted_at: None,
            });
        }
    }
    Ok(out)
}

/// The rows a built-in role holds with no stored grant.
fn implied(role: &str) -> Vec<RoleGrant> {
    let Some((database, privileges)) = builtin(role) else {
        return vec![];
    };
    privileges
        .into_iter()
        .map(|privilege| RoleGrant {
            role: role.to_string(),
            database: Some(database.clone()),
            collection: EVERY.to_string(),
            privilege,
            granted_by: None,
            granted_at: None,
        })
        .collect()
}

fn effective(g: &RoleGrant, source: &str, via: Option<String>) -> EffectiveGrant {
    EffectiveGrant {
        database: g.database.clone(),
        target: g.collection.clone(),
        privilege: g.privilege,
        source: source.to_string(),
        via,
    }
}

/// Everything `subject` holds. `owned` is `(database, collection)` for each
/// collection a principal owns.
pub(super) fn subject_access(
    subject: &AccessSubject,
    users: &[UserRow],
    custom: &[RoleRow],
    grants: &[Grant],
    owned: &[(String, String)],
    now: i64,
) -> Result<SubjectAccess, StudioError> {
    grantees(subject, users, custom)?;
    let mut superuser = false;
    let mut effective_rows = Vec::new();
    let mut unreported = vec![];
    let (held, memberships, members) = match subject {
        AccessSubject::Principal(id) => {
            let user = users
                .iter()
                .find(|u| &u.name == id)
                .ok_or_else(|| unknown("principal", id))?;
            superuser = user.superuser;
            if user.superuser {
                effective_rows.push(EffectiveGrant {
                    database: Some(EVERY.to_string()),
                    target: EVERY.to_string(),
                    privilege: Privilege::All,
                    source: "superuser account".to_string(),
                    via: None,
                });
            }
            effective_rows.extend(owned.iter().map(|(db, c)| EffectiveGrant {
                database: Some(db.clone()),
                target: c.clone(),
                privilege: Privilege::All,
                source: "owner".to_string(),
                via: None,
            }));
            unreported.push(USER_TENANT_GRANTS.to_string());
            let mut memberships = Vec::new();
            for r in &user.roles {
                let mut covered: Vec<RoleGrant> = implied(r);
                covered.extend(stored(r, grants)?);
                let scopes: HashSet<_> = covered
                    .iter()
                    .map(|g| (g.database.clone(), g.collection.clone()))
                    .collect();
                memberships.push(Membership {
                    principal: user.name.clone(),
                    role: r.clone(),
                    database: NO_DATABASE.to_string(),
                    collections: u32::try_from(scopes.len()).unwrap_or(u32::MAX),
                });
            }
            (user.roles.clone(), memberships, vec![])
        }
        AccessSubject::Role(name) => {
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
    let mut role_grants = Vec::new();
    let mut listed: HashSet<String> = HashSet::new();
    let mut keep = |g: RoleGrant, rows: &mut Vec<RoleGrant>| {
        let key = format!(
            "{}/{:?}/{}/{}",
            g.role,
            g.database,
            g.collection,
            g.privilege.label()
        );
        if listed.insert(key) {
            rows.push(g);
        }
    };
    for source in &held {
        // A built-in role grants its rights only to who holds it itself.
        for g in implied(source) {
            effective_rows.push(effective(&g, source, None));
            keep(g, &mut role_grants);
        }
        for (depth, role) in chain(source, custom).iter().enumerate() {
            let via = (depth > 0).then(|| role.clone());
            for g in stored(role, grants)? {
                effective_rows.push(effective(&g, source, via.clone()));
                keep(g, &mut role_grants);
            }
        }
    }
    if let AccessSubject::Principal(id) = subject {
        // A grant to the user itself. The server stores it as `user:<name>`:
        // a bare name is a role.
        let own = format!("user:{id}");
        for g in grants.iter().filter(|g| g.grantee == own) {
            for privilege in privileges(&g.permission)? {
                effective_rows.push(EffectiveGrant {
                    database: g.database.clone(),
                    target: g.target.clone(),
                    privilege,
                    source: id.clone(),
                    via: None,
                });
            }
        }
    }
    // Only a superuser opens every database. Anyone else reaches a built-in
    // role's or a tenant-wide grant's collections in the databases it can
    // open, which the server does not list.
    let reach = match subject {
        AccessSubject::Principal(_) if superuser => None,
        AccessSubject::Principal(_) => Some((USER_DATABASES, USER_DATABASE_GRANTS)),
        AccessSubject::Role(_) => Some((HOLDER_DATABASES, HOLDER_DATABASE_GRANTS)),
    };
    if let Some((databases, note)) = reach {
        let rows = role_grants.iter_mut().map(|g| &mut g.database);
        for db in rows.chain(effective_rows.iter_mut().map(|e| &mut e.database)) {
            if db.as_deref() == Some(EVERY) {
                *db = Some(databases.to_string());
            }
        }
        unreported.push(note.to_string());
    }
    Ok(SubjectAccess {
        subject: subject.clone(),
        memberships,
        members,
        grants: role_grants,
        effective: effective_rows,
        unreported,
        observed_at: now,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(name: &str, roles: &[&str]) -> UserRow {
        UserRow {
            name: name.into(),
            tenant: 0,
            roles: roles.iter().map(|r| (*r).to_string()).collect(),
            superuser: false,
        }
    }

    fn role(name: &str, parent: Option<&str>) -> RoleRow {
        RoleRow {
            name: name.into(),
            tenant: 0,
            parent: parent.map(str::to_string),
            created_at: 5,
        }
    }

    fn grant(grantee: &str, permission: &str, target: &str, db: Option<&str>) -> Grant {
        Grant {
            grantee: grantee.into(),
            permission: permission.into(),
            target: target.into(),
            database: db.map(str::to_string),
        }
    }

    fn of(
        subject: AccessSubject,
        users: &[UserRow],
        custom: &[RoleRow],
        grants: &[Grant],
        owned: &[(String, String)],
    ) -> SubjectAccess {
        subject_access(&subject, users, custom, grants, owned, 9).expect("reads")
    }

    fn rows(a: &SubjectAccess) -> Vec<(Option<&str>, &str, &'static str, &str)> {
        a.effective
            .iter()
            .map(|e| {
                (
                    e.database.as_deref(),
                    e.target.as_str(),
                    e.privilege.label(),
                    e.source.as_str(),
                )
            })
            .collect()
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
    fn every_server_permission_has_a_privilege_and_no_other_name_passes() {
        for (name, n) in [
            ("Read", 1),
            ("Write", 3),
            ("Create", 1),
            ("Drop", 1),
            ("Alter", 1),
            ("Admin", 1),
            ("Monitor", 1),
            ("Execute", 1),
            ("Backup", 1),
        ] {
            assert_eq!(privileges(name).expect(name).len(), n, "{name}");
        }
        assert!(privileges("Teleport").is_err(), "never dropped in silence");
    }

    #[test]
    fn a_role_with_drop_and_admin_shows_both() {
        let custom = [role("ops", None)];
        let grants = [
            grant("ops", "Drop", "orders", Some("default")),
            grant("ops", "Admin", "orders", Some("default")),
        ];
        let a = of(
            AccessSubject::Role("ops".into()),
            &[],
            &custom,
            &grants,
            &[],
        );
        let labels: Vec<_> = a.grants.iter().map(|g| g.privilege.label()).collect();
        assert_eq!(labels, ["DROP", "ADMIN"]);
        assert_eq!(a.effective.len(), 2);
        assert!(
            a.grants
                .iter()
                .all(|g| g.granted_by.is_none() && g.granted_at.is_none())
        );
    }

    #[test]
    fn superusers_tenant_admins_and_owners_hold_an_explicit_all_row() {
        let mut flagged = user("root", &[]);
        flagged.superuser = true;
        let users = [
            flagged,
            user("ta", &["tenant_admin"]),
            user("ann", &["readonly"]),
        ];
        let root = of(
            AccessSubject::Principal("root".into()),
            &users,
            &[],
            &[],
            &[],
        );
        assert_eq!(
            rows(&root),
            [(Some(EVERY), EVERY, "ALL", "superuser account")]
        );
        let ta = of(AccessSubject::Principal("ta".into()), &users, &[], &[], &[]);
        assert_eq!(
            rows(&ta),
            [(Some(USER_DATABASES), EVERY, "ALL", "tenant_admin")]
        );
        let owned = [("db2".to_string(), "orders".to_string())];
        let ann = of(
            AccessSubject::Principal("ann".into()),
            &users,
            &[],
            &[],
            &owned,
        );
        assert_eq!(
            rows(&ann),
            [
                (Some("db2"), "orders", "ALL", "owner"),
                (Some(USER_DATABASES), EVERY, "SELECT", "readonly"),
                (Some(USER_DATABASES), EVERY, "EXECUTE", "readonly"),
            ]
        );
        let su = of(
            AccessSubject::Role("superuser".into()),
            &users,
            &[],
            &[],
            &[],
        );
        assert_eq!(su.grants[0].privilege, Privilege::All);
        assert_eq!(su.grants[0].database.as_deref(), Some(HOLDER_DATABASES));
    }

    #[test]
    fn only_a_superuser_reaches_every_database() {
        let mut root = user("root", &["readwrite"]);
        root.superuser = true;
        let users = [root, user("bob", &["readwrite", "wide"])];
        let custom = [role("wide", None)];
        let grants = [grant("wide", "Read", EVERY, Some(EVERY))];
        let bob = of(
            AccessSubject::Principal("bob".into()),
            &users,
            &custom,
            &grants,
            &[],
        );
        assert!(!bob.effective.is_empty());
        assert!(
            bob.effective
                .iter()
                .map(|e| &e.database)
                .chain(bob.grants.iter().map(|g| &g.database))
                .all(|d| d.as_deref() == Some(USER_DATABASES)),
            "a built-in role and a tenant-wide grant reach the databases bob can open"
        );
        assert!(bob.unreported.iter().any(|n| n == USER_DATABASE_GRANTS));
        let root = of(
            AccessSubject::Principal("root".into()),
            &users,
            &custom,
            &grants,
            &[],
        );
        assert!(
            root.effective
                .iter()
                .all(|e| e.database.as_deref() == Some(EVERY))
        );
        assert!(!root.unreported.iter().any(|n| n == USER_DATABASE_GRANTS));
        let wide = of(
            AccessSubject::Role("wide".into()),
            &users,
            &custom,
            &grants,
            &[],
        );
        assert_eq!(wide.grants[0].database.as_deref(), Some(HOLDER_DATABASES));
        assert_eq!(wide.unreported, [HOLDER_DATABASE_GRANTS]);
    }

    #[test]
    fn a_database_role_covers_its_database_only() {
        let users = [user("ed", &["database_editor:1024"])];
        let a = of(AccessSubject::Principal("ed".into()), &users, &[], &[], &[]);
        assert!(
            a.effective
                .iter()
                .all(|e| e.database.as_deref() == Some("database id 1024"))
        );
        assert!(a.effective.iter().any(|e| e.privilege == Privilege::Create));
        assert!(!a.effective.iter().any(|e| e.privilege == Privilege::Drop));
        assert_eq!(builtin("database_editor:x"), None);
    }

    #[test]
    fn a_built_in_parent_passes_on_its_stored_grants_only() {
        let users = [user("u", &["child"])];
        let custom = [role("child", Some("readonly"))];
        let grants = [grant("readonly", "Read", "orders", Some("default"))];
        let a = of(
            AccessSubject::Principal("u".into()),
            &users,
            &custom,
            &grants,
            &[],
        );
        assert_eq!(rows(&a), [(Some("default"), "orders", "SELECT", "child")]);
        assert_eq!(a.effective[0].via.as_deref(), Some("readonly"));
    }

    #[test]
    fn a_role_named_like_the_user_lends_it_nothing() {
        let users = [user("ann", &[])];
        let custom = [role("ann", None)];
        let grants = [
            grant("ann", "Drop", "orders", Some("default")),
            grant("user:ann", "Read", "items", Some("default")),
        ];
        let a = of(
            AccessSubject::Principal("ann".into()),
            &users,
            &custom,
            &grants,
            &[],
        );
        assert_eq!(rows(&a), [(Some("default"), "items", "SELECT", "ann")]);
        assert_eq!(
            a.unreported,
            [USER_TENANT_GRANTS, USER_DATABASE_GRANTS],
            "tenant-wide user grants and database grants are named"
        );
    }

    #[test]
    fn inherited_grants_name_their_parent() {
        let users = [user("u", &["child"])];
        let custom = [role("child", Some("parent")), role("parent", None)];
        let grants = [grant("parent", "Read", "orders", None)];
        let a = of(
            AccessSubject::Principal("u".into()),
            &users,
            &custom,
            &grants,
            &[],
        );
        assert_eq!(a.effective.len(), 1);
        assert_eq!(a.effective[0].source, "child");
        assert_eq!(a.effective[0].via.as_deref(), Some("parent"));
        assert_eq!(a.effective[0].database, None, "not reported stays so");
        assert_eq!(a.memberships[0].collections, 0);
        assert!(matches!(
            subject_access(
                &AccessSubject::Principal("zz".into()),
                &users,
                &custom,
                &grants,
                &[],
                9
            ),
            Err(StudioError::UnknownObject { .. })
        ));
    }

    #[test]
    fn a_role_lists_its_members() {
        let users = [user("u", &["r"]), user("v", &[])];
        let a = of(
            AccessSubject::Role("r".into()),
            &users,
            &[role("r", None)],
            &[],
            &[],
        );
        assert_eq!(a.members.len(), 1);
        assert_eq!(a.members[0].principal, "u");
    }
}
