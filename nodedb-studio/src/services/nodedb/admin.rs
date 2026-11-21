//! Administration reads and changes over `SHOW` and DDL statements.
//!
//! Wired: the access directory and subject access, the policy list, the
//! audit trail and its filter lists, and access changes. Every other
//! method returns a typed error that names what NodeDB 0.6.2 lacks (see
//! `unavailable`).
//!
//! The server has no revision on users or roles. A review's revision is a
//! hash of the rows the change read, and apply reads them again and refuses
//! when the hash differs. Another administrator's change that lands between
//! that read and the statement is not caught.

use std::collections::{BTreeMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};

use async_trait::async_trait;
use nodedb_client::{NodeDb, NodeDbError};
use nodedb_types::Value;

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
use crate::services::nodedb::admin_access::{self as access, Grant, PermRow, RoleRow, UserRow};
use crate::services::nodedb::admin_audit as audit;
use crate::services::nodedb::admin_grants as grants;
use crate::services::nodedb::admin_plan::plan;
use crate::services::nodedb::admin_policy as policy;
use crate::services::nodedb::admin_user::Statement;
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

fn int(n: u64) -> Value {
    Value::Integer(i64::try_from(n).unwrap_or(i64::MAX))
}

/// The users and custom roles of the caller's tenant.
struct TenantRows {
    users: Vec<UserRow>,
    roles: Vec<RoleRow>,
    tenant: u64,
    /// Over every row the server listed, of every tenant.
    revision: u64,
}

impl NodeDbConnectionService {
    async fn caller(&self) -> Result<access::Caller, StudioError> {
        access::session(&self.table("SHOW SESSION").await?)
    }

    /// A superuser reads every tenant's users and roles. The views cover
    /// the caller's tenant only, so a name repeated in another tenant never
    /// lends its roles or grants.
    async fn tenant_rows(&self) -> Result<TenantRows, StudioError> {
        let users = self.table("SHOW USERS").await?;
        let roles = self.table("SHOW ROLES").await?;
        let tenant = self.caller().await?.tenant;
        let (u, r) = access::of_tenant(access::users(&users)?, access::roles(&roles)?, tenant);
        Ok(TenantRows {
            users: u,
            roles: r,
            tenant,
            revision: revision(&[&users, &roles]),
        })
    }

    async fn permission_check(&self, required: &str) -> Result<PermissionCheck, StudioError> {
        Ok(PermissionCheck {
            required: required.to_string(),
            granted: self.caller().await?.admin,
        })
    }

    async fn database_list(&self) -> Result<Vec<String>, StudioError> {
        access::database_names(&self.table("SHOW DATABASES").await?)
    }

    /// `sql` in `db`, or `None` when `db` refused the caller or is gone:
    /// `skipped` then names it with the reason. A skipped database is not
    /// asked again.
    async fn in_database(
        &self,
        db: &str,
        sql: &str,
        skipped: &mut BTreeMap<String, String>,
    ) -> Result<Option<Table>, StudioError> {
        if skipped.contains_key(db) {
            return Ok(None);
        }
        match self.client(Some(db))?.query(sql).await {
            Ok(r) => Ok(Some(to_table(&r))),
            Err(e) => {
                let e = StudioError::from(e);
                if !access::refused(&e) {
                    return Err(e);
                }
                skipped.insert(db.to_string(), e.to_string());
                Ok(None)
            }
        }
    }

    /// The grants made to `wanted`, each in its database. Bare `SHOW
    /// PERMISSIONS` names no database, so each name is asked again in every
    /// database. Tenant-wide grants show only under `FOR <role>`.
    async fn grants_to(
        &self,
        wanted: &HashSet<String>,
        databases: &[String],
        tenant: u64,
        skipped: &mut BTreeMap<String, String>,
    ) -> Result<Vec<Grant>, StudioError> {
        let ours = |rows: Vec<PermRow>| -> Vec<PermRow> {
            rows.into_iter()
                .filter(|p| wanted.contains(&p.grantee))
                .collect()
        };
        let bare = ours(access::permissions(&self.table("SHOW PERMISSIONS").await?)?);
        let mut names: Vec<&str> = bare
            .iter()
            .map(|p| p.target.as_str())
            .filter(|n| access::ident(n, "collection").is_ok())
            .collect();
        names.sort_unstable();
        names.dedup();
        let mut per_db = Vec::new();
        for db in databases {
            let mut rows = Vec::new();
            for name in &names {
                let sql = format!("SHOW PERMISSIONS ON {name}");
                let Some(t) = self.in_database(db, &sql, skipped).await? else {
                    break;
                };
                rows.extend(ours(access::permissions(&t)?));
            }
            per_db.push((db.clone(), rows));
        }
        let mut wide = Vec::new();
        for role in wanted.iter().filter(|g| access::ident(g, "role").is_ok()) {
            let t = self.table(&format!("SHOW PERMISSIONS FOR {role}")).await?;
            wide.extend(ours(access::permissions(&t)?));
        }
        Ok(access::placed(&bare, &per_db, &wide, tenant))
    }

    /// `(database, collection)` for each collection `user` owns.
    async fn owned_by(
        &self,
        user: &str,
        databases: &[String],
        skipped: &mut BTreeMap<String, String>,
    ) -> Result<Vec<(String, String)>, StudioError> {
        let mut out = Vec::new();
        for db in databases {
            let Some(t) = self.in_database(db, "SHOW COLLECTIONS", skipped).await? else {
                continue;
            };
            out.extend(
                access::owners(&t)?
                    .into_iter()
                    .filter(|(_, o)| o == user)
                    .map(|(c, _)| (db.clone(), c)),
            );
        }
        Ok(out)
    }

    /// The newest events `query` asks for, from both audit reads.
    async fn audit_fetch(
        &self,
        query: &AuditQuery,
        now: i64,
    ) -> Result<audit::Fetched, StudioError> {
        let caller = self.caller().await?;
        let client = self.client(None)?;
        let n = audit::FETCH_LIMIT as u64 + 1;
        let from = now
            .saturating_sub(query.range.seconds())
            .saturating_mul(1_000_000);
        let mut params = vec![Value::Integer(from), int(n)];
        let sql = match &query.action {
            Some(action) => {
                params.push(Value::String(action.clone()));
                audit::FRESH_ACTION_SQL
            }
            None => audit::FRESH_SQL,
        };
        // The fresh read runs first: a flush between the two reads moves
        // rows into the catalog, where the stored read finds them.
        let fresh = audit::events(&to_table(&client.execute_sql(sql, &params).await?), caller)?;
        let stored = audit::events(
            &self.table(&format!("SHOW AUDIT LOG LIMIT {n}")).await?,
            caller,
        )?;
        let gap = match stored.first() {
            Some((newest, _)) if stored.len() > audit::FETCH_LIMIT => {
                let t = to_table(&client.execute_sql(audit::ONE_SQL, &[int(*newest)]).await?);
                t.rows.is_empty()
            }
            _ => false,
        };
        Ok(audit::Fetched { fresh, stored, gap })
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
        let rows = self.tenant_rows().await?;
        Ok(access::directory(
            &rows.users,
            &rows.roles,
            self.database_list().await?,
            Some(rows.tenant),
            now(),
        ))
    }

    async fn subject_access(&self, subject: &AccessSubject) -> Result<SubjectAccess, StudioError> {
        let rows = self.tenant_rows().await?;
        let wanted = grants::grantees(subject, &rows.users, &rows.roles)?;
        let databases = self.database_list().await?;
        // A tenant administrator sees every database and opens only some.
        let mut skipped = BTreeMap::new();
        let held = self
            .grants_to(&wanted, &databases, rows.tenant, &mut skipped)
            .await?;
        let owned = match subject {
            AccessSubject::Principal(id) => self.owned_by(id, &databases, &mut skipped).await?,
            AccessSubject::Role(_) => vec![],
        };
        let mut held =
            grants::subject_access(subject, &rows.users, &rows.roles, &held, &owned, now())?;
        held.unreported.extend(access::skipped_notes(&skipped));
        Ok(held)
    }

    async fn policy_set(&self, database: &str) -> Result<PolicySet, StudioError> {
        let databases = self.database_list().await?;
        if !databases.iter().any(|d| d == database) {
            return Err(StudioError::UnknownDatabase(database.to_string()));
        }
        // A client bound to `database`: policies and collections are per database.
        let client = self.client(Some(database))?;
        let collections =
            policy::collection_names(&to_table(&client.query("SHOW COLLECTIONS").await?))?;
        // Bare `SHOW RLS POLICIES` lists every database of the tenant, by
        // collection name only. `ON <collection>` asks this database.
        let listed = to_table(&client.query("SHOW RLS POLICIES").await?);
        let mut policies = Vec::new();
        for name in policy::policy_collections(&listed, &collections)? {
            let t = to_table(
                &client
                    .query(&format!("SHOW RLS POLICIES ON {name}"))
                    .await?,
            );
            policies.extend(policy::policies(&t, database)?);
        }
        let rows = self.tenant_rows().await?;
        let dir = access::directory(&rows.users, &rows.roles, vec![], None, 0);
        Ok(PolicySet {
            database: database.to_string(),
            databases,
            collections,
            roles: dir.roles.into_iter().map(|r| r.name).collect(),
            // The server cannot run a filter as another user, so it offers none.
            test_users: vec![],
            policies,
            observed_at: now(),
        })
    }

    async fn test_policy(&self, _test: &PolicyTest) -> Result<PolicyTestResult, StudioError> {
        Err(unavailable(
            "cannot run a filter as another user without that user's credentials: IMPERSONATE changes a server-wide registry, so a test would write",
        ))
    }

    async fn audit_facets(&self) -> Result<AuditFacets, StudioError> {
        let caller = self.caller().await?;
        let mut actors: Vec<String> = access::users(&self.table("SHOW USERS").await?)?
            .into_iter()
            .filter(|u| caller.superuser || u.tenant == caller.tenant)
            .map(|u| audit::actor_label(&u.name, u.tenant, caller))
            .collect();
        actors.sort();
        actors.dedup();
        Ok(AuditFacets {
            actors,
            actions: audit::actions(),
            // The server reads `audit_retention_days` from its config file
            // and reports it nowhere.
            retention_days: None,
            scope: "superuser".to_string(),
        })
    }

    async fn audit_events(&self, query: &AuditQuery) -> Result<AuditPage, StudioError> {
        let now = now();
        let fetched = self.audit_fetch(query, now).await?;
        Ok(audit::page(fetched, query, now))
    }

    async fn audit_event(&self, id: &str) -> Result<AuditEvent, StudioError> {
        let unknown = || StudioError::UnknownObject {
            kind: "audit event",
            id: id.to_string(),
        };
        let seq: u64 = id.parse().map_err(|_| unknown())?;
        let caller = self.caller().await?;
        let client = self.client(None)?;
        let fresh = audit::events(
            &to_table(&client.execute_sql(audit::ONE_SQL, &[int(seq)]).await?),
            caller,
        )?;
        let n = audit::FETCH_LIMIT + 1;
        let stored = audit::events(
            &self.table(&format!("SHOW AUDIT LOG LIMIT {n}")).await?,
            caller,
        )?;
        audit::find(fresh, stored, seq).ok_or_else(unknown)
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
            let rows = self.tenant_rows().await?;
            let planned = plan(changes, &rows.users, &rows.roles);
            return Ok(AdminReview {
                title: "Change access".to_string(),
                target: format!("{} change(s)", planned.len()),
                notes: vec![],
                warnings: planned.iter().flat_map(|p| p.warnings.clone()).collect(),
                errors: planned.iter().flat_map(|p| p.errors.clone()).collect(),
                lines: planned.into_iter().map(|p| p.line).collect(),
                permission: self.permission_check(ADMIN_PRIVILEGE).await?,
                revision: rows.revision,
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
        let rows = self.tenant_rows().await?;
        if rows.revision != revision_seen {
            return Err(StudioError::StaleRevision {
                object: "access".to_string(),
                expected: revision_seen,
                stored: rows.revision,
            });
        }
        let planned = plan(changes, &rows.users, &rows.roles);
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
        let statements: Vec<Statement> = planned.into_iter().filter_map(|p| p.statement).collect();
        let client = self.client(None)?;
        for (done, statement) in statements.iter().enumerate() {
            if let Err(e) = client.query(statement.sql()).await {
                // A statement with a password never puts it in the error.
                let e = statement.scrubbed(StudioError::from(e));
                return Err(if done == 0 {
                    e
                } else {
                    StudioError::Rejected {
                        reason: format!(
                            "applied {done} of {} statements, then `{}` failed: {e}",
                            statements.len(),
                            statement.label
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::admin_access::{
        AccessChange, EVERY, HOLDER_DATABASES, PrincipalKind, Privilege,
    };
    use crate::models::admin_audit::AuditRange;
    use crate::models::admin_policy::{PolicyChange, PolicyDraft};
    use crate::services::nodedb::admin_user::live::users_lock;
    use crate::services::nodedb::service::{live, live_as};

    const USER: &str = "studio_probe_a_t_user";
    const ROLE: &str = "studio_probe_a_t_role";
    const COLL: &str = "studio_probe_a_t_coll";
    /// A second database, kept between runs.
    const DB2: &str = "studio_probe_a_db2";

    /// Statements that must not fail a test: leftovers from an earlier run.
    async fn quietly_in(svc: &NodeDbConnectionService, db: Option<&str>, statements: &[&str]) {
        let c = svc.client(db).expect("a session");
        for sql in statements {
            let _ = c.query(sql).await;
        }
    }

    async fn quietly(svc: &NodeDbConnectionService, statements: &[&str]) {
        quietly_in(svc, None, statements).await;
    }

    fn refs(v: &[String]) -> Vec<&str> {
        v.iter().map(String::as_str).collect()
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
        let e = svc.credentials().await.expect_err("no credentials");
        assert!(e.to_string().contains("0.6.2"), "{e}");
    }

    #[tokio::test]
    #[ignore = "needs a live server: see service::live"]
    async fn live_admin_reads_directory_access_and_audit() {
        let _lock = users_lock();
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
        assert_eq!(
            (me.credential_expiry, me.created_at, me.password),
            (None, None, None),
            "nothing the server does not report"
        );
        assert_eq!(dir.tenant, Some(0));
        assert!(dir.principals.iter().any(|p| p.name == USER));
        assert!(dir.roles.iter().any(|r| r.name == "readonly"));
        let custom = dir.roles.iter().find(|r| r.name == ROLE).expect("custom");
        assert!(custom.created_at.is_some(), "SHOW ROLES reports it");
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
        assert!(a.effective.iter().any(|g| g.target == COLL
            && g.source == ROLE
            && g.database.as_deref() == Some("default")));
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
        let facets = svc.audit_facets().await.expect("facets");
        assert!(facets.actors.iter().any(|a| a == "nodedb"));
        assert!(facets.actions.iter().any(|a| a == "PermissionDenied"));
        assert_eq!(facets.retention_days, None);

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
        let other = "studio_probe_a_t_pol2";
        quietly(&svc, &[&format!("CREATE DATABASE {DB2}")]).await;
        quietly_in(
            &svc,
            Some(DB2),
            &[
                &format!("DROP RLS POLICY {other} ON {COLL}"),
                &format!("CREATE COLLECTION {COLL}"),
                &format!("CREATE RLS POLICY {other} ON {COLL} FOR READ USING (id = $auth.id)"),
            ],
        )
        .await;
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
        assert_eq!(p.using_expr, None, "the server reports no filter text");
        assert_eq!(p.role, None);
        assert!(
            !set.policies.iter().any(|p| p.name == other),
            "a policy of another database is not listed here"
        );
        let in_db2 = svc.policy_set(DB2).await.expect("policy set");
        assert!(in_db2.policies.iter().any(|p| p.name == other));
        assert!(
            !in_db2
                .policies
                .iter()
                .any(|p| p.name == "studio_probe_a_t_pol")
        );
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
        quietly_in(
            &svc,
            Some(DB2),
            &[
                &format!("DROP RLS POLICY {other} ON {COLL}"),
                &format!("DROP COLLECTION IF EXISTS {COLL}"),
            ],
        )
        .await;
        clean(&svc).await;
    }

    #[tokio::test]
    #[ignore = "needs a live server: see service::live"]
    async fn live_admin_grants_show_every_right_and_their_database() {
        let _lock = users_lock();
        let svc = live().await;
        let (role, owner, coll) = (
            "studio_probe_a_g_role",
            "studio_probe_a_g_owner",
            "studio_probe_a_g_coll",
        );
        let reset = [
            format!("REVOKE READ ON TENANT 0 FROM {role}"),
            format!("DROP USER IF EXISTS {owner}"),
            format!("DROP ROLE IF EXISTS {role}"),
            format!("DROP COLLECTION IF EXISTS {coll}"),
        ];
        let in_db2 = [
            format!("REVOKE DROP ON {coll} FROM {role}"),
            format!("REVOKE ADMIN ON {coll} FROM {role}"),
            format!("DROP COLLECTION IF EXISTS {coll}"),
        ];
        quietly_in(&svc, Some(DB2), &refs(&in_db2)).await;
        quietly(&svc, &refs(&reset)).await;
        quietly(&svc, &[&format!("CREATE DATABASE {DB2}")]).await;
        quietly(
            &svc,
            &[
                &format!("CREATE ROLE {role}"),
                &format!(
                    "CREATE USER {owner} WITH PASSWORD 'studio-probe-fixture-3' ROLE readonly"
                ),
                // The same name in the default database, with no grant.
                &format!("CREATE COLLECTION {coll}"),
                &format!("GRANT READ ON TENANT 0 TO {role}"),
            ],
        )
        .await;
        quietly_in(
            &svc,
            Some(DB2),
            &[
                &format!("CREATE COLLECTION {coll}"),
                &format!("GRANT DROP ON {coll} TO {role}"),
                &format!("GRANT ADMIN ON {coll} TO {role}"),
                &format!("ALTER COLLECTION {coll} OWNER TO {owner}"),
            ],
        )
        .await;

        let r = svc
            .subject_access(&AccessSubject::Role(role.into()))
            .await
            .expect("role");
        let held: Vec<_> = r
            .grants
            .iter()
            .map(|g| (g.database.as_deref(), g.collection.as_str(), g.privilege))
            .collect();
        for want in [
            (Some(DB2), coll, Privilege::Drop),
            (Some(DB2), coll, Privilege::Admin),
            // A tenant-wide grant reaches the databases a holder can open.
            (Some(HOLDER_DATABASES), EVERY, Privilege::Select),
        ] {
            assert!(held.contains(&want), "{want:?} in {held:?}");
        }
        assert!(
            !held.iter().any(|(db, _, _)| *db == Some("default")),
            "the grant binds {DB2}, not the same name in default: {held:?}"
        );

        let o = svc
            .subject_access(&AccessSubject::Principal(owner.into()))
            .await
            .expect("owner");
        assert!(o.effective.iter().any(|e| e.privilege == Privilege::All
            && e.source == "owner"
            && e.target == coll
            && e.database.as_deref() == Some(DB2)));
        let me = svc
            .subject_access(&AccessSubject::Principal("nodedb".into()))
            .await
            .expect("superuser");
        assert!(me.effective.iter().any(|e| e.privilege == Privilege::All
            && e.database.as_deref() == Some(EVERY)
            && e.source == "superuser account"));

        quietly_in(&svc, Some(DB2), &refs(&in_db2)).await;
        quietly(&svc, &refs(&reset)).await;
    }

    /// A user with `readwrite` and no grant on `DB2`, and a tenant
    /// administrator with no grant on `DB2` either.
    #[tokio::test]
    #[ignore = "needs a live server: see service::live"]
    async fn live_admin_a_user_reaches_only_the_databases_it_can_open() {
        use crate::models::admin_access::USER_DATABASES;
        use crate::models::admin_audit::TargetType;
        let _lock = users_lock();
        let svc = live().await;
        let (bob, ta) = ("studio_probe_a_bob", "studio_probe_a_ta");
        let (bob_pw, ta_pw) = ("studio-probe-fixture-5", "studio-probe-fixture-6");
        let reset = [
            format!("REVOKE ALL ON DATABASE {DB2} FROM {bob}"),
            format!("DROP USER IF EXISTS {bob}"),
            format!("DROP USER IF EXISTS {ta}"),
        ];
        quietly(&svc, &refs(&reset)).await;
        quietly(
            &svc,
            &[
                &format!("CREATE DATABASE {DB2}"),
                &format!("CREATE USER {bob} WITH PASSWORD '{bob_pw}' ROLE readwrite"),
                &format!("CREATE USER {ta} WITH PASSWORD '{ta_pw}' ROLE tenant_admin"),
            ],
        )
        .await;

        // The server refuses bob in DB2: he holds no grant on it.
        let (as_bob, _) = live_as(bob, bob_pw).await;
        let refusal = as_bob
            .client(Some(DB2))
            .expect("a session")
            .query("SHOW COLLECTIONS")
            .await
            .map(|_| ())
            .map_err(StudioError::from);
        println!("bob in {DB2}: {refusal:?}");
        let refusal = refusal.expect_err("bob cannot open DB2");
        assert!(access::refused(&refusal), "{refusal}");

        let a = svc
            .subject_access(&AccessSubject::Principal(bob.into()))
            .await
            .expect("access");
        assert!(
            a.effective
                .iter()
                .any(|e| e.source == "readwrite" && e.privilege == Privilege::Insert)
        );
        assert!(
            a.effective
                .iter()
                .filter(|e| e.source == "readwrite")
                .all(|e| e.database.as_deref() == Some(USER_DATABASES)),
            "readwrite reaches the databases bob can open, not every one"
        );
        assert!(
            a.unreported
                .iter()
                .any(|n| n.contains("does not list database grants")),
            "{:?}",
            a.unreported
        );

        // No statement lists a database grant.
        quietly(
            &svc,
            &[&format!(
                "GRANT CREATE COLLECTION ON DATABASE {DB2} TO {bob}"
            )],
        )
        .await;
        for sql in [
            format!("SHOW GRANTS FOR {bob}"),
            format!("SHOW PERMISSIONS FOR {bob}"),
            format!("SHOW PERMISSIONS FOR \"user:{bob}\""),
            "SHOW PERMISSIONS".to_string(),
        ] {
            let listed = svc.table(&sql).await.map(|t| t.rows.concat().join(" "));
            println!("{sql}: {listed:?}");
            assert!(
                !listed.as_deref().is_ok_and(|t| t.contains(DB2)),
                "{sql} lists the database grant: {listed:?}"
            );
        }
        quietly(&svc, &[&format!("REVOKE ALL ON DATABASE {DB2} FROM {bob}")]).await;

        // A tenant administrator lists DB2 but cannot open it: the panel
        // still renders and names it.
        let (as_ta, _) = live_as(ta, ta_pw).await;
        let seen = as_ta
            .subject_access(&AccessSubject::Principal(bob.into()))
            .await
            .expect("the panel renders");
        let skipped = format!("Could not read database {DB2}: ");
        assert!(
            seen.unreported.iter().any(|n| n.starts_with(&skipped)),
            "{:?}",
            seen.unreported
        );

        // Bob's refusal is a database denial in the trail.
        let denied = svc
            .audit_events(&AuditQuery {
                range: AuditRange::Last24Hours,
                action: Some("PermissionDenied".into()),
                target: Some(TargetType::Database),
                page_size: 200,
                ..AuditQuery::default()
            })
            .await
            .expect("audit");
        let mine = format!("user '{bob}' does not have access to");
        let e = denied
            .events
            .iter()
            .find(|e| e.object.contains(&mine))
            .expect("bob's refusal is filed under Database");
        assert!(e.target.starts_with("database "), "{}", e.target);
        assert_eq!(e.tenant, Some(0));

        quietly(&svc, &refs(&reset)).await;
    }

    #[tokio::test]
    #[ignore = "needs a live server: see service::live"]
    async fn live_admin_audit_shows_the_newest_event_at_once() {
        let _lock = users_lock();
        let svc = live().await;
        let role = "studio_probe_a_au_role";
        quietly(&svc, &[&format!("DROP ROLE IF EXISTS {role}")]).await;
        quietly(&svc, &[&format!("CREATE ROLE {role}")]).await;
        let created = format!("created role '{role}'");
        let stored = svc.table("SHOW AUDIT LOG LIMIT 20").await.expect("raw");
        // Shown, not asserted: the server moves events to its catalog every
        // 10 seconds, so the raw read misses them until then.
        println!(
            "SHOW AUDIT LOG already holds the event: {}",
            stored.rows.iter().flatten().any(|c| *c == created)
        );
        let page = svc
            .audit_events(&AuditQuery {
                range: AuditRange::Last24Hours,
                action: Some("PrivilegeChange".into()),
                page_size: 50,
                ..AuditQuery::default()
            })
            .await
            .expect("audit");
        assert!(
            page.events.iter().any(|e| e.object == created),
            "the event written just now is listed"
        );
        let all = svc.audit_events(&week()).await.expect("audit");
        assert!(all.total <= audit::FETCH_LIMIT);
        if all.total == audit::FETCH_LIMIT {
            assert!(all.truncated, "a full list says it is cut");
        }
        // Unflushed events lack a database, and they are the newest: no
        // event without one follows an event with one.
        let first_stored = all.events.iter().position(|e| e.database.is_some());
        assert!(
            first_stored.is_none_or(|i| all.events[i..].iter().all(|e| e.database.is_some())),
            "only events younger than the last flush lack a database"
        );
        quietly(&svc, &[&format!("DROP ROLE IF EXISTS {role}")]).await;
    }

    #[tokio::test]
    #[ignore = "needs a live server: see service::live"]
    async fn live_admin_access_change_previews_applies_and_goes_stale() {
        let _lock = users_lock();
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

        clean(&svc).await;
    }
}
