//! Principals, roles, grants, and memberships: the seed data and the reads
//! that resolve what a principal or role holds.

use crate::data::mock::admin::store::{ADMIN, AdminStore, DATABASE, missing};
use crate::models::admin_access::{
    AccessDirectory, AccessSubject, EffectiveGrant, Member, Membership, Mfa, Principal,
    PrincipalKind, PrincipalStatus, Privilege, Role, RoleGrant, SubjectAccess, sort_effective,
};
use crate::models::format::epoch;
use crate::services::error::StudioError;

type Seed = (
    Vec<Principal>,
    Vec<Role>,
    Vec<RoleGrant>,
    Vec<(String, String, String)>,
);

pub(super) fn principal(
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
    pub(super) fn role_effective(&self, role: &str, database: &str) -> Vec<EffectiveGrant> {
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

    pub(super) fn membership(&self, principal: &str, role: &str, database: &str) -> Membership {
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
