//! What the session may do to a collection.
//!
//! The server reports a session's roles in `SHOW SESSION`. It does not report
//! what a custom role or a collection-level `GRANT` allows: `SHOW PERMISSIONS`
//! and `SHOW GRANTS` answer a non-superuser with its own role names and no
//! collection grants (checked live on 0.6.2). So the answer comes from the
//! superuser flag and the built-in role names alone. Any other role set is an
//! error, never a guess. A grant to `INSERT`, `UPDATE` or `DELETE` is one
//! `Write` permission on the server: the three move together.

use nodedb_client::NodeDbError;

use crate::models::collection::Profile;
use crate::models::records::{CollectionAccess, Denial, Grant};
use crate::services::decode::decode_rows;
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;

/// Built-in roles that read and write data.
const WRITERS: [&str; 3] = ["readwrite", "admin", "tenant_admin"];
/// The built-in role that reads and never writes.
const READER: &str = "readonly";

fn undetermined(roles: &str) -> StudioError {
    StudioError::from(NodeDbError::config(format!(
        "the session's rights cannot be read: roles `{roles}` include a role whose \
         permissions NodeDB 0.6.2 does not report to a non-superuser"
    )))
}

/// The rights a role line gives, for a collection of `profile`.
pub(super) fn rights(
    roles: &str,
    superuser: bool,
    profile: Profile,
) -> Result<CollectionAccess, StudioError> {
    let names: Vec<&str> = roles
        .split(',')
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .collect();
    let writes = superuser || names.iter().any(|r| WRITERS.contains(r));
    let reads_only = !names.is_empty() && names.iter().all(|r| *r == READER);
    if !writes && !reads_only {
        return Err(undetermined(roles));
    }
    let (read, write) = if writes {
        (Grant::Allowed, Grant::Allowed)
    } else {
        (Grant::Allowed, Grant::Denied(Denial::Privilege))
    };
    // The server refuses UPDATE and DELETE on timeseries whoever asks.
    let change = if profile == Profile::Timeseries {
        Grant::Denied(Denial::Policy("Timeseries data is append-only".to_string()))
    } else {
        write.clone()
    };
    Ok(CollectionAccess {
        role: if superuser {
            "superuser".to_string()
        } else {
            names.join(", ")
        },
        create: write,
        read,
        update: change.clone(),
        delete: change,
    })
}

impl NodeDbConnectionService {
    pub(super) async fn access_of(
        &self,
        collection: &str,
    ) -> Result<CollectionAccess, StudioError> {
        let meta = self.meta(collection).await?;
        let session = self.table("SHOW SESSION").await?;
        let rows = decode_rows(&session, &["roles", "is_superuser"], |r| {
            // The native result prints a boolean as `t` or `f`.
            let flag = r.field("is_superuser")?;
            Ok((r.field("roles")?.to_string(), flag == "t" || flag == "true"))
        })?;
        let (roles, superuser) =
            rows.into_iter()
                .next()
                .ok_or_else(|| StudioError::MalformedRows {
                    reason: "SHOW SESSION returned no row".to_string(),
                })?;
        rights(&roles, superuser, meta.profile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_superuser_may_do_everything_but_change_a_timeseries() {
        let a = rights("superuser", true, Profile::Document).expect("rights");
        assert!(a.create.allowed() && a.read.allowed() && a.update.allowed() && a.delete.allowed());
        assert_eq!(a.role, "superuser");
        let t = rights("superuser", true, Profile::Timeseries).expect("rights");
        assert!(t.create.allowed() && !t.update.allowed() && !t.delete.allowed());
        assert!(matches!(t.update, Grant::Denied(Denial::Policy(_))));
    }

    #[test]
    fn readonly_reads_and_readwrite_writes() {
        let ro = rights("readonly", false, Profile::DocumentStrict).expect("rights");
        assert!(ro.read.allowed());
        assert_eq!(ro.create, Grant::Denied(Denial::Privilege));
        assert_eq!(ro.delete, Grant::Denied(Denial::Privilege));
        let rw = rights("readwrite, readonly", false, Profile::KeyValue).expect("rights");
        assert!(rw.create.allowed() && rw.update.allowed());
        assert_eq!(rw.role, "readwrite, readonly");
    }

    #[test]
    fn an_unknown_or_empty_role_set_is_an_error_not_a_guess() {
        for roles in ["", "analyst", "readonly, analyst", "monitor"] {
            assert!(rights(roles, false, Profile::Document).is_err(), "{roles}");
        }
    }
}

#[cfg(test)]
pub(super) mod live {
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

        let ro = session_as("studio_probe_x_ro_u", "xropass1")
            .collection_access("studio_probe_x_acc_doc")
            .await
            .expect("readonly access");
        assert!(ro.read.allowed());
        assert_eq!(ro.create, Grant::Denied(Denial::Privilege));
        assert_eq!(ro.role, "readonly");

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
