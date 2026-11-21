//! What the session may do to a collection.
//!
//! The server reports a session's roles in `SHOW SESSION`. It does not report
//! what a custom role allows, or a `GRANT .. ON TENANT`: `SHOW PERMISSIONS`
//! answers a non-superuser with no grants of its own (checked live on 0.6.2).
//! It does list, to anyone, the owner of a collection and the grants made on
//! it (`SHOW PERMISSIONS ON`). So the answer comes from the superuser flag,
//! the built-in role names, and for a `readonly` session the write right
//! listed on the collection. Any other role set is an error, never a guess.
//! A grant to `INSERT`, `UPDATE` or `DELETE` is one `Write` permission on the
//! server: the three move together.

use nodedb_client::NodeDbError;

use crate::models::collection::Profile;
use crate::models::records::{CollectionAccess, Denial, Grant};
use crate::services::decode::decode_rows;
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;

/// Built-in roles that read and write every collection of their tenant
/// (`role_grants_permission`, 0.6.2 server source). `cluster_admin` and
/// `monitor` grant no data access. A database-scoped role such as
/// `database_editor:<id>` holds only in its own database, which `SHOW
/// SESSION` does not name. Any other name, `admin` included, is a custom role
/// that grants nothing by itself.
const WRITERS: [&str; 2] = ["readwrite", "tenant_admin"];
/// The built-in role that reads and writes only through a grant.
const READER: &str = "readonly";

fn undetermined(roles: &str) -> StudioError {
    StudioError::from(NodeDbError::config(format!(
        "the session's rights cannot be read: roles `{roles}` include a role whose \
         permissions NodeDB 0.6.2 does not report to a non-superuser"
    )))
}

/// The role names of a `SHOW SESSION` role line.
fn role_names(roles: &str) -> Vec<&str> {
    roles
        .split(',')
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .collect()
}

/// Whether every role reads and none writes: the one role set whose write
/// right depends on grants.
fn reads_only(names: &[&str]) -> bool {
    !names.is_empty() && names.iter().all(|r| *r == READER)
}

/// Whether a `SHOW PERMISSIONS ON` listing gives `user` a write: ownership, or
/// a `Write` grant to the user or to one of its roles. A grant on the tenant
/// is not listed, so `false` is no denial.
pub(in crate::services::nodedb) fn write_listed(
    rows: &[(String, String, String)],
    user: &str,
    roles: &[&str],
) -> bool {
    let me = format!("user:{user}");
    rows.iter()
        .any(|(grantee, permission, kind)| match kind.as_str() {
            "ownership" => grantee == user,
            "grant" => {
                permission == "Write" && (*grantee == me || roles.contains(&grantee.as_str()))
            }
            _ => false,
        })
}

/// The rights a role set gives, for a collection of `profile`.
/// `granted_write` is what `write_listed` found for a `readonly` session.
pub(in crate::services::nodedb) fn rights(
    roles: &str,
    superuser: bool,
    granted_write: bool,
    profile: Profile,
) -> Result<CollectionAccess, StudioError> {
    let names = role_names(roles);
    let by_role = superuser || names.iter().any(|r| WRITERS.contains(r));
    // `readonly` writes through a collection grant, a tenant grant or
    // ownership, and a tenant grant is not listed: with no listed write, the
    // answer is unknown, never a denial.
    let by_grant = reads_only(&names) && granted_write;
    if !(by_role || by_grant) {
        return Err(undetermined(roles));
    }
    // The server refuses UPDATE and DELETE on timeseries whoever asks.
    let change = if profile == Profile::Timeseries {
        Grant::Denied(Denial::Policy("Timeseries data is append-only".to_string()))
    } else {
        Grant::Allowed
    };
    Ok(CollectionAccess {
        role: if superuser {
            "superuser".to_string()
        } else {
            names.join(", ")
        },
        create: Grant::Allowed,
        read: Grant::Allowed,
        update: change.clone(),
        delete: change,
    })
}

impl NodeDbConnectionService {
    pub(in crate::services::nodedb) async fn access_of(
        &self,
        collection: &str,
    ) -> Result<CollectionAccess, StudioError> {
        let meta = self.meta(collection).await?;
        let session = self.table("SHOW SESSION").await?;
        let rows = decode_rows(&session, &["username", "roles", "is_superuser"], |r| {
            // The native result prints a boolean as `t` or `f`.
            let flag = r.field("is_superuser")?;
            Ok((
                r.field("username")?.to_string(),
                r.field("roles")?.to_string(),
                flag == "t" || flag == "true",
            ))
        })?;
        let (user, roles, superuser) =
            rows.into_iter()
                .next()
                .ok_or_else(|| StudioError::MalformedRows {
                    reason: "SHOW SESSION returned no row".to_string(),
                })?;
        let names = role_names(&roles);
        let granted_write = if !superuser && reads_only(&names) {
            // `meta` checked the name: it is a bare identifier.
            let listed = self
                .table(&format!("SHOW PERMISSIONS ON {}", meta.collection))
                .await?;
            let grants = decode_rows(&listed, &["grantee", "permission", "type"], |r| {
                Ok((
                    r.field("grantee")?.to_string(),
                    r.field("permission")?.to_string(),
                    r.field("type")?.to_string(),
                ))
            })?;
            write_listed(&grants, &user, &names)
        } else {
            false
        };
        rights(&roles, superuser, granted_write, meta.profile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_superuser_may_do_everything_but_change_a_timeseries() {
        let a = rights("superuser", true, false, Profile::Document).expect("rights");
        assert!(a.create.allowed() && a.read.allowed() && a.update.allowed() && a.delete.allowed());
        assert_eq!(a.role, "superuser");
        let t = rights("superuser", true, false, Profile::Timeseries).expect("rights");
        assert!(t.create.allowed() && !t.update.allowed() && !t.delete.allowed());
        assert!(matches!(t.update, Grant::Denied(Denial::Policy(_))));
    }

    #[test]
    fn readwrite_writes_and_readonly_writes_only_through_a_listed_grant() {
        let rw = rights("readwrite, readonly", false, false, Profile::KeyValue).expect("rights");
        assert!(rw.create.allowed() && rw.update.allowed());
        assert_eq!(rw.role, "readwrite, readonly");
        let ta = rights("tenant_admin", false, false, Profile::Document).expect("rights");
        assert!(ta.delete.allowed());
        // a tenant grant is not listed: no listed write is no denial
        assert!(rights("readonly", false, false, Profile::DocumentStrict).is_err());
        let granted = rights("readonly", false, true, Profile::DocumentStrict).expect("rights");
        assert!(granted.read.allowed() && granted.create.allowed() && granted.delete.allowed());
        assert_eq!(granted.role, "readonly");
    }

    #[test]
    fn an_unknown_or_empty_role_set_is_an_error_not_a_guess() {
        // `admin` is no built-in role on 0.6.2: it parses as a custom role
        for roles in [
            "",
            "analyst",
            "readonly, analyst",
            "monitor",
            "admin",
            "cluster_admin",
        ] {
            assert!(
                rights(roles, false, true, Profile::Document).is_err(),
                "{roles}"
            );
        }
    }

    #[test]
    fn a_write_is_listed_by_ownership_or_a_grant_to_the_user_or_its_role() {
        let row = |g: &str, p: &str, k: &str| (g.to_string(), p.to_string(), k.to_string());
        let owner = [row("ro", "ALL (owner)", "ownership")];
        assert!(write_listed(&owner, "ro", &["readonly"]));
        assert!(!write_listed(&owner, "other", &["readonly"]));
        let to_user = [row("user:ro", "Write", "grant")];
        assert!(write_listed(&to_user, "ro", &["readonly"]));
        assert!(!write_listed(
            &[row("user:ro", "Read", "grant")],
            "ro",
            &["readonly"]
        ));
        assert!(write_listed(
            &[row("readonly", "Write", "grant")],
            "ro",
            &["readonly"]
        ));
        assert!(!write_listed(
            &[row("ro", "Write", "grant")],
            "x",
            &["readonly"]
        ));
        assert!(!write_listed(&[], "ro", &["readonly"]));
    }
}

#[cfg(test)]
pub(in crate::services::nodedb) mod live {
    use super::*;
    use crate::models::connections::{AuthMethod, ConnectionTarget, TlsConfig};
    use crate::services::explorer_data::ExplorerData;
    use crate::services::nodedb::service::{Session, live, pool_config};

    /// A service whose session logs in as `user`. `connect` is not used: it
    /// runs `SHOW DATABASES`, which a non-admin cannot.
    pub(in crate::services::nodedb) fn session_as(
        user: &str,
        password: &str,
    ) -> NodeDbConnectionService {
        let addr = std::env::var("NODEDB_PROBE_ADDR").expect("set NODEDB_PROBE_ADDR");
        let (host, port) = addr.rsplit_once(':').expect("host:port");
        let target = ConnectionTarget::Origin {
            host: host.into(),
            port: port.parse().expect("a port"),
        };
        let config = pool_config(
            &target,
            AuthMethod::UserPassword,
            user,
            Some(password),
            &TlsConfig::default(),
            "",
        )
        .expect("a pool config");
        let svc = NodeDbConnectionService::default();
        *svc.session.borrow_mut() =
            Some(Session::new(config, "default".into(), String::new(), user));
        svc
    }

    async fn seed(admin: &NodeDbConnectionService) {
        let c = admin.client(None).expect("a client");
        // Idempotent: an object that exists already makes the statement error.
        for sql in [
            "CREATE COLLECTION IF NOT EXISTS studio_probe_x_acc_doc",
            "CREATE USER studio_probe_x_ro_u WITH PASSWORD 'xropass1' ROLE readonly",
            "CREATE USER studio_probe_x_rw_u WITH PASSWORD 'xrwpass1' ROLE readwrite",
            "CREATE USER studio_probe_x_none_u WITH PASSWORD 'xnonepass1'",
            "REVOKE readwrite FROM studio_probe_x_none_u",
            "REVOKE WRITE ON studio_probe_x_acc_doc FROM studio_probe_x_ro_u",
        ] {
            let _ = c.query(sql).await;
        }
    }

    #[tokio::test]
    #[ignore = "needs a live server: NODEDB_PROBE_ADDR"]
    async fn live_explorer_access_follows_the_session_roles() {
        let admin = live().await;
        seed(&admin).await;
        let su = admin
            .collection_access("studio_probe_x_acc_doc")
            .await
            .expect("superuser access");
        assert_eq!(su.role, "superuser");
        assert!(su.create.allowed() && su.update.allowed() && su.delete.allowed());

        // readonly with no listed write: a tenant grant could still allow it
        let ro = session_as("studio_probe_x_ro_u", "xropass1");
        let none_listed = ro.collection_access("studio_probe_x_acc_doc").await;
        assert!(none_listed.is_err(), "{none_listed:?}");
        let c = admin.client(None).expect("a client");
        c.query("GRANT WRITE ON studio_probe_x_acc_doc TO studio_probe_x_ro_u")
            .await
            .expect("grants");
        let granted = ro.collection_access("studio_probe_x_acc_doc").await;
        c.query("REVOKE WRITE ON studio_probe_x_acc_doc FROM studio_probe_x_ro_u")
            .await
            .expect("revokes");
        let granted = granted.expect("readonly access with a grant");
        assert!(granted.read.allowed() && granted.create.allowed() && granted.delete.allowed());
        assert_eq!(granted.role, "readonly");

        let rw = session_as("studio_probe_x_rw_u", "xrwpass1")
            .collection_access("studio_probe_x_acc_doc")
            .await
            .expect("readwrite access");
        assert!(rw.create.allowed() && rw.delete.allowed());

        // No role: the server hides a collection-level GRANT from the session.
        let none = session_as("studio_probe_x_none_u", "xnonepass1")
            .collection_access("studio_probe_x_acc_doc")
            .await;
        assert!(none.is_err(), "{none:?}");
    }
}
