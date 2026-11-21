//! Administration reads and changes over `SHOW` and DDL statements.
//!
//! Wired: the access directory and subject access, the policy list, the
//! audit trail, and access changes. Every other method returns a typed error
//! that names what NodeDB 0.6.2 lacks (see `unavailable`).
//!
//! The server has no revision on users or roles. A review's revision is a
//! hash of the rows the change read, and apply reads them again and refuses
//! when the hash differs. Another administrator's change that lands between
//! that read and the statement is not caught.

use std::hash::{DefaultHasher, Hash, Hasher};

use async_trait::async_trait;
use nodedb_client::NodeDbError;

use crate::models::admin::{AdminChange, AdminReview};
use crate::models::admin_access::{AccessDirectory, AccessSubject, SubjectAccess};
use crate::models::admin_audit::{AuditEvent, AuditFacets, AuditPage, AuditQuery};
use crate::models::admin_backup::BackupOverview;
use crate::models::admin_config::{ConfigProposal, ConfigRequest, ConfigTarget, ConfigView};
use crate::models::admin_credential::{CredentialList, CredentialTest};
use crate::models::admin_policy::{PolicySet, PolicyTest, PolicyTestResult};
use crate::models::cluster_op::{OpResult, PermissionCheck};
use crate::services::admin_data::AdminData;
use crate::services::decode::Table;
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;
use crate::services::nodedb::admin_access as access;
use crate::services::nodedb::admin_audit as audit;
use crate::services::nodedb::admin_policy as policy;
use crate::services::nodedb::service::{next_id, to_table};

/// A typed error for something NodeDB 0.6.2 cannot supply.
fn unavailable(what: &str) -> StudioError {
    StudioError::from(NodeDbError::bad_request(format!("NodeDB 0.6.2 {what}")))
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

/// A number that changes when any row of `tables` changes.
fn revision(tables: &[&Table]) -> u64 {
    let mut h = DefaultHasher::new();
    for t in tables {
        t.columns.hash(&mut h);
        t.rows.hash(&mut h);
    }
    h.finish()
}

impl NodeDbConnectionService {
    async fn principals(&self) -> Result<(Table, Table), StudioError> {
        Ok((
            self.table("SHOW USERS").await?,
            self.table("SHOW ROLES").await?,
        ))
    }

    async fn permission_check(&self, required: &str) -> Result<PermissionCheck, StudioError> {
        let session = self.table("SHOW SESSION").await?;
        Ok(PermissionCheck {
            required: required.to_string(),
            granted: access::can_administer(&session)?,
        })
    }

    async fn database_list(&self) -> Result<Vec<String>, StudioError> {
        access::database_names(&self.table("SHOW DATABASES").await?)
    }
}

const NO_BACKUPS: &str = "keeps no snapshot list, restore history, or schedule: there is no SHOW BACKUPS, and BACKUP DATABASE writes to a server-side URI the change does not carry";
const NO_CREDENTIALS: &str =
    "has no credential aliases: no statement lists or tests provider credentials";
const NO_CONFIG: &str = "cannot change server config at runtime: ALTER SYSTEM SET accepts one field, deactivated_collection_retention_days, and nothing reads it back";
const ADMIN_PRIVILEGE: &str = "superuser or tenant_admin";

#[async_trait(?Send)]
impl AdminData for NodeDbConnectionService {
    async fn access_directory(&self) -> Result<AccessDirectory, StudioError> {
        let (users, roles) = self.principals().await?;
        Ok(access::directory(
            &access::users(&users)?,
            &access::roles(&roles)?,
            self.database_list().await?,
            now(),
        ))
    }

    async fn subject_access(&self, subject: &AccessSubject) -> Result<SubjectAccess, StudioError> {
        let (users, roles) = self.principals().await?;
        let perms = self.table("SHOW PERMISSIONS").await?;
        access::subject_access(
            subject,
            &access::users(&users)?,
            &access::roles(&roles)?,
            &access::permissions(&perms)?,
            now(),
        )
    }

    async fn policy_set(&self, database: &str) -> Result<PolicySet, StudioError> {
        let databases = self.database_list().await?;
        if !databases.iter().any(|d| d == database) {
            return Err(StudioError::UnknownDatabase(database.to_string()));
        }
        // A client bound to `database`: policies and collections are per database.
        let client = self.client(Some(database))?;
        let listed = to_table(&client.query("SHOW RLS POLICIES").await?);
        let collections = to_table(&client.query("SHOW COLLECTIONS").await?);
        let (users, roles) = self.principals().await?;
        let dir = access::directory(&access::users(&users)?, &access::roles(&roles)?, vec![], 0);
        Ok(PolicySet {
            database: database.to_string(),
            databases,
            collections: policy::collection_names(&collections)?,
            roles: dir.roles.into_iter().map(|r| r.name).collect(),
            // The server cannot run a filter as another user, so it offers none.
            test_users: vec![],
            policies: policy::policies(&listed, database)?,
            observed_at: now(),
        })
    }

    async fn test_policy(&self, _test: &PolicyTest) -> Result<PolicyTestResult, StudioError> {
        Err(unavailable(
            "cannot run a filter as another user without that user's credentials: IMPERSONATE changes a server-wide registry, so a test would write",
        ))
    }

    async fn audit_facets(&self) -> Result<AuditFacets, StudioError> {
        Err(unavailable(
            "reports no audit retention period, and the facets state one",
        ))
    }

    async fn audit_events(&self, query: &AuditQuery) -> Result<AuditPage, StudioError> {
        let all = self.audit_log().await?;
        Ok(audit::page(all, query, now()))
    }

    async fn audit_event(&self, id: &str) -> Result<AuditEvent, StudioError> {
        self.audit_log()
            .await?
            .into_iter()
            .find(|e| e.id == id)
            .ok_or_else(|| StudioError::UnknownObject {
                kind: "audit event",
                id: id.to_string(),
            })
    }

    async fn backup_overview(&self) -> Result<BackupOverview, StudioError> {
        Err(unavailable(NO_BACKUPS))
    }

    async fn config_view(&self, _target: &ConfigTarget) -> Result<ConfigView, StudioError> {
        Err(unavailable(
            "has no statement that reads server config: SHOW CONFIG does not exist",
        ))
    }

    async fn preview_config(
        &self,
        _request: &ConfigRequest,
    ) -> Result<ConfigProposal, StudioError> {
        Err(unavailable(NO_CONFIG))
    }

    async fn credentials(&self) -> Result<CredentialList, StudioError> {
        Err(unavailable(NO_CREDENTIALS))
    }

    async fn test_credential(&self, _alias: &str) -> Result<CredentialTest, StudioError> {
        Err(unavailable(
            "has no credential aliases: no statement tests a provider",
        ))
    }

    async fn preview_admin_change(&self, change: &AdminChange) -> Result<AdminReview, StudioError> {
        if let Some(changes) = access::access_changes(change) {
            let (users, roles) = self.principals().await?;
            let planned = access::plan(changes, &access::users(&users)?, &access::roles(&roles)?);
            return Ok(AdminReview {
                title: "Change access".to_string(),
                target: format!("{} change(s)", planned.len()),
                notes: vec![],
                warnings: planned.iter().flat_map(|p| p.warnings.clone()).collect(),
                errors: planned.iter().flat_map(|p| p.errors.clone()).collect(),
                lines: planned.into_iter().map(|p| p.line).collect(),
                permission: self.permission_check(ADMIN_PRIVILEGE).await?,
                revision: revision(&[&users, &roles]),
                apply_label: "Apply access changes".to_string(),
            });
        }
        match change {
            AdminChange::Policy(p) => {
                let listed = self.table("SHOW RLS POLICIES").await?;
                Ok(AdminReview {
                    title: "Change row policy".to_string(),
                    target: p.draft().name.clone(),
                    lines: vec![crate::models::admin::ReviewLine::new(
                        format!("Policy {}", p.draft().name),
                        None,
                        "changed",
                    )],
                    notes: vec![],
                    warnings: vec![],
                    errors: policy::refusals(p),
                    permission: self.permission_check(ADMIN_PRIVILEGE).await?,
                    revision: revision(&[&listed]),
                    apply_label: "Apply policy".to_string(),
                })
            }
            AdminChange::Backup(_) => Err(unavailable(NO_BACKUPS)),
            AdminChange::Config(_) => Err(unavailable(NO_CONFIG)),
            AdminChange::Credential(_) => Err(unavailable(NO_CREDENTIALS)),
            AdminChange::Access(_) => Err(unavailable("access change was not planned")),
        }
    }

    async fn apply_admin_change(
        &self,
        change: &AdminChange,
        revision_seen: u64,
    ) -> Result<OpResult, StudioError> {
        let Some(changes) = access::access_changes(change) else {
            return Err(StudioError::Rejected {
                reason: "NodeDB 0.6.2 cannot apply this change: the review lists why".to_string(),
            });
        };
        let (users, roles) = self.principals().await?;
        let stored = revision(&[&users, &roles]);
        if stored != revision_seen {
            return Err(StudioError::StaleRevision {
                object: "access".to_string(),
                expected: revision_seen,
                stored,
            });
        }
        let planned = access::plan(changes, &access::users(&users)?, &access::roles(&roles)?);
        let errors: Vec<String> = planned.iter().flat_map(|p| p.errors.clone()).collect();
        if !errors.is_empty() {
            return Err(StudioError::Rejected {
                reason: errors.join(" "),
            });
        }
        let check = self.permission_check(ADMIN_PRIVILEGE).await?;
        if !check.granted {
            return Err(StudioError::PermissionDenied {
                privilege: "superuser or tenant_admin",
                collection: "access".to_string(),
            });
        }
        let statements: Vec<String> = planned.into_iter().filter_map(|p| p.statement).collect();
        let client = self.client(None)?;
        for (done, sql) in statements.iter().enumerate() {
            if let Err(e) = client.query(sql).await {
                let e = StudioError::from(e);
                return Err(if done == 0 {
                    e
                } else {
                    StudioError::Rejected {
                        reason: format!(
                            "applied {done} of {} statements, then `{sql}` failed: {e}",
                            statements.len()
                        ),
                    }
                });
            }
        }
        Ok(OpResult {
            operation: format!("op_{}", next_id()),
            summary: format!("{} statement(s) applied", statements.len()),
        })
    }
}

impl NodeDbConnectionService {
    async fn audit_log(&self) -> Result<Vec<AuditEvent>, StudioError> {
        let sql = format!("SHOW AUDIT LOG LIMIT {}", audit::FETCH_LIMIT);
        audit::events(&self.table(&sql).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::admin_access::{AccessChange, PrincipalKind};
    use crate::models::admin_audit::AuditRange;
    use crate::models::admin_policy::{PolicyChange, PolicyDraft};
    use crate::services::nodedb::service::live;

    const USER: &str = "studio_probe_a_t_user";
    const ROLE: &str = "studio_probe_a_t_role";
    const COLL: &str = "studio_probe_a_t_coll";

    /// Statements that must not fail a test: leftovers from an earlier run.
    async fn quietly(svc: &NodeDbConnectionService, statements: &[&str]) {
        let c = svc.client(None).expect("a session");
        for sql in statements {
            let _ = c.query(sql).await;
        }
    }

    async fn clean(svc: &NodeDbConnectionService) {
        quietly(
            svc,
            &[
                &format!("DROP RLS POLICY studio_probe_a_t_pol ON {COLL}"),
                &format!("DROP COLLECTION IF EXISTS {COLL}"),
                &format!("REVOKE {ROLE} FROM {USER}"),
                &format!("DROP USER IF EXISTS {USER}"),
                &format!("DROP ROLE IF EXISTS {ROLE}"),
            ],
        )
        .await;
    }

    fn week() -> AuditQuery {
        AuditQuery {
            range: AuditRange::Last30Days,
            page_size: 10,
            ..AuditQuery::default()
        }
    }

    #[tokio::test]
    async fn no_session_is_not_connected() {
        let svc = NodeDbConnectionService::default();
        assert!(matches!(
            svc.access_directory().await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.audit_events(&week()).await,
            Err(StudioError::NotConnected)
        ));
    }

    #[tokio::test]
    async fn what_the_server_cannot_supply_is_a_typed_error() {
        let svc = NodeDbConnectionService::default();
        assert!(svc.backup_overview().await.is_err());
        assert!(svc.credentials().await.is_err());
        assert!(svc.audit_facets().await.is_err());
        let e = svc.credentials().await.expect_err("no credentials");
        assert!(e.to_string().contains("0.6.2"), "{e}");
    }

    #[tokio::test]
    #[ignore = "needs a live server: see service::live"]
    async fn live_admin_reads_directory_access_and_audit() {
        let svc = live().await;
        clean(&svc).await;
        quietly(
            &svc,
            &[
                &format!("CREATE ROLE {ROLE}"),
                &format!("CREATE USER {USER} WITH PASSWORD 'studio-probe-fixture-1'"),
                &format!("GRANT {ROLE} TO {USER}"),
                &format!("CREATE COLLECTION {COLL}"),
                &format!("GRANT SELECT ON COLLECTION {COLL} TO {ROLE}"),
            ],
        )
        .await;

        let dir = svc.access_directory().await.expect("directory");
        let me = dir
            .principals
            .iter()
            .find(|p| p.name == "nodedb")
            .expect("the login");
        assert_eq!(me.kind, PrincipalKind::User);
        assert!(dir.principals.iter().any(|p| p.name == USER));
        assert!(dir.roles.iter().any(|r| r.name == "readonly"));
        assert!(dir.roles.iter().any(|r| r.name == ROLE));
        assert!(dir.databases.iter().any(|d| d == "default"));

        let a = svc
            .subject_access(&AccessSubject::Principal(USER.into()))
            .await
            .expect("access");
        assert!(
            a.memberships
                .iter()
                .any(|m| m.role == ROLE && m.collections == 1)
        );
        assert!(
            a.effective
                .iter()
                .any(|g| g.target == COLL && g.source == ROLE)
        );
        let r = svc
            .subject_access(&AccessSubject::Role(ROLE.into()))
            .await
            .expect("role");
        assert_eq!(r.members.len(), 1);
        assert!(matches!(
            svc.subject_access(&AccessSubject::Role("studio_probe_a_t_none".into()))
                .await,
            Err(StudioError::UnknownObject { .. })
        ));

        let page = svc.audit_events(&week()).await.expect("audit");
        assert!(page.total > 0 && !page.events.is_empty());
        let first = &page.events[0];
        let again = svc.audit_event(&first.id).await.expect("one event");
        assert_eq!(again.id, first.id);
        let grants = svc
            .audit_events(&AuditQuery {
                action: Some("PrivilegeChange".into()),
                ..week()
            })
            .await
            .expect("filtered");
        assert!(
            grants.events.iter().any(|e| e.actor == "nodedb"),
            "{:?}",
            grants.events.first()
        );

        let set = svc.policy_set("default").await.expect("policy set");
        assert!(set.collections.iter().any(|c| c == COLL));
        assert!(matches!(
            svc.policy_set("studio_probe_a_t_nodb").await,
            Err(StudioError::UnknownDatabase(_))
        ));
        clean(&svc).await;
    }

    #[tokio::test]
    #[ignore = "needs a live server: see service::live"]
    async fn live_admin_policy_lists_what_the_server_reports() {
        let svc = live().await;
        clean(&svc).await;
        quietly(
            &svc,
            &[
                &format!("CREATE COLLECTION {COLL}"),
                &format!("CREATE RLS POLICY studio_probe_a_t_pol ON {COLL} FOR READ USING (id = $auth.id)"),
            ],
        )
        .await;
        let set = svc.policy_set("default").await.expect("policy set");
        let p = set
            .policies
            .iter()
            .find(|p| p.name == "studio_probe_a_t_pol")
            .expect("the policy");
        assert_eq!(p.collection, COLL);
        assert!(p.enabled);
        assert_eq!(p.using_expr, "", "the server reports no filter text");
        let change = AdminChange::Policy(PolicyChange::Create(PolicyDraft::blank(
            "default", COLL, "r",
        )));
        let review = svc.preview_admin_change(&change).await.expect("preview");
        assert!(!review.can_apply(), "{:?}", review.errors);
        assert!(
            svc.apply_admin_change(&change, review.revision)
                .await
                .is_err()
        );
        clean(&svc).await;
    }

    #[tokio::test]
    #[ignore = "needs a live server: see service::live"]
    async fn live_admin_access_change_previews_applies_and_goes_stale() {
        let svc = live().await;
        clean(&svc).await;
        quietly(
            &svc,
            &[&format!(
                "CREATE USER {USER} WITH PASSWORD 'studio-probe-fixture-1'"
            )],
        )
        .await;

        let create = AdminChange::Access(vec![
            AccessChange::CreateRole {
                name: ROLE.into(),
                description: String::new(),
            },
            AccessChange::AddMembership {
                principal: USER.into(),
                role: ROLE.into(),
                database: String::new(),
            },
        ]);
        let review = svc.preview_admin_change(&create).await.expect("preview");
        assert!(review.can_apply(), "{:?}", review.errors);
        assert_eq!(review.lines.len(), 2);

        // Another administrator's change between preview and apply.
        quietly(&svc, &["CREATE ROLE studio_probe_a_t_other"]).await;
        let stale = svc.apply_admin_change(&create, review.revision).await;
        quietly(&svc, &["DROP ROLE IF EXISTS studio_probe_a_t_other"]).await;
        assert!(
            matches!(stale, Err(StudioError::StaleRevision { .. })),
            "{stale:?}"
        );
        let dir = svc.access_directory().await.expect("directory");
        assert!(
            !dir.roles.iter().any(|r| r.name == ROLE),
            "the stale apply wrote nothing"
        );

        let review = svc.preview_admin_change(&create).await.expect("preview");
        svc.apply_admin_change(&create, review.revision)
            .await
            .expect("apply");
        let held = svc
            .subject_access(&AccessSubject::Principal(USER.into()))
            .await
            .expect("access");
        assert!(held.memberships.iter().any(|m| m.role == ROLE));
        let trail = svc
            .audit_events(&AuditQuery {
                action: Some("PrivilegeChange".into()),
                page_size: 50,
                ..week()
            })
            .await
            .expect("audit");
        assert!(
            trail
                .events
                .iter()
                .any(|e| e.object.contains(ROLE) && e.object.contains(USER)),
            "the grant is in the trail"
        );

        let remove = AdminChange::Access(vec![AccessChange::RemoveMembership {
            principal: USER.into(),
            role: ROLE.into(),
            database: String::new(),
        }]);
        let review = svc.preview_admin_change(&remove).await.expect("preview");
        svc.apply_admin_change(&remove, review.revision)
            .await
            .expect("apply");
        let after = svc
            .subject_access(&AccessSubject::Principal(USER.into()))
            .await
            .expect("access");
        assert!(!after.memberships.iter().any(|m| m.role == ROLE));
        let again = svc.preview_admin_change(&remove).await.expect("preview");
        assert!(!again.can_apply(), "nothing left to remove");

        let user = AdminChange::Access(vec![AccessChange::CreatePrincipal {
            name: "studio_probe_a_t_new".into(),
            kind: PrincipalKind::User,
            description: String::new(),
            credential_expiry: 0,
        }]);
        assert!(
            !svc.preview_admin_change(&user)
                .await
                .expect("preview")
                .can_apply()
        );
        clean(&svc).await;
    }
}
