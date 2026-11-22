//! Connections, notifications, and session reads and writes.

use std::rc::Rc;
use std::time::Instant;

use async_trait::async_trait;
use nodedb_client::native::pool::PoolConfig;
use nodedb_client::{NativeClient, NodeDb};

use crate::models::connections::{
    ConnStatus, ConnectionDraft, ConnectionEvent, ConnectionTest, Observation, TlsStatus,
};
use crate::models::notification::Notification;
use crate::models::shell::{NavBadges, RecentItem, SessionInfo};
use crate::services::connection_service::ConnectionService;
use crate::services::decode::decode_rows;
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;
use crate::services::nodedb::service::{Session, client_for, pool_config, to_table};
use crate::state::connection::{ActiveConnection, Capabilities};
use crate::state::connections_registry::{Credentials, SavedConnection};

/// The database a session opens when the saved connection names none.
const DEFAULT_DATABASE: &str = "default";

fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

/// What one round trip to a server saw.
struct Probe {
    client: Rc<NativeClient>,
    ping_ms: u32,
    version: String,
    who: Identity,
    databases: Vec<String>,
}

/// Who the server says the session is.
struct Identity {
    user: String,
    /// The tenant the user belongs to. Two tenants can each have a user of
    /// the same name.
    tenant_id: String,
    /// The role line: `superuser`, or the roles the user holds.
    role: String,
    /// A superuser or a tenant admin. Only they may list databases.
    admin: bool,
}

/// Reach the server `config` names, authenticate, and list its databases.
async fn probe(config: &PoolConfig, database: &str) -> Result<Probe, StudioError> {
    let client = client_for(config, database);
    let start = Instant::now();
    client.ping().await?;
    let ping_ms = u32::try_from(start.elapsed().as_millis()).unwrap_or(u32::MAX);
    let who = identity_of(&client).await?;
    let databases = databases_of(&client, database, who.admin).await?;
    Ok(Probe {
        // The handshake reports `NodeDB/0.6.2`. The chrome shows the number.
        version: client
            .server_version()
            .trim_start_matches("NodeDB/")
            .to_string(),
        client,
        ping_ms,
        who,
        databases,
    })
}

/// The databases the session can list. The server refuses `SHOW DATABASES`
/// to a user who is not an admin: that user sees `current`, the one
/// database the session is on.
async fn databases_of(
    client: &NativeClient,
    current: &str,
    admin: bool,
) -> Result<Vec<String>, StudioError> {
    if !admin {
        return Ok(vec![database_name(current)]);
    }
    let table = to_table(&client.query("SHOW DATABASES").await?);
    decode_rows(&table, &["name"], |r| Ok(r.field("name")?.to_string()))
}

/// The name of the database a saved connection opens: its own, or the
/// server default when it names none.
fn database_name(saved: &str) -> String {
    match saved.trim() {
        "" => DEFAULT_DATABASE.to_string(),
        name => name.to_string(),
    }
}

async fn identity_of(client: &NativeClient) -> Result<Identity, StudioError> {
    let table = to_table(&client.query("SHOW SESSION").await?);
    let rows = decode_rows(
        &table,
        &["username", "tenant_id", "roles", "is_superuser"],
        |r| {
            Ok(identity(
                r.field("username")?,
                r.field("tenant_id")?,
                r.field("roles")?,
                r.field("is_superuser")?,
            ))
        },
    )?;
    rows.into_iter()
        .next()
        .ok_or_else(|| StudioError::MalformedRows {
            reason: "SHOW SESSION returned no row".to_string(),
        })
}

/// One `SHOW SESSION` row as an identity. The native result prints a
/// boolean as `t` or `f`.
fn identity(username: &str, tenant_id: &str, roles: &str, is_superuser: &str) -> Identity {
    let superuser = matches!(is_superuser, "t" | "true");
    let tenant_admin = roles
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .any(|r| r == "tenant_admin");
    Identity {
        user: username.to_string(),
        tenant_id: tenant_id.to_string(),
        role: if superuser {
            "superuser".to_string()
        } else {
            roles.to_string()
        },
        admin: superuser || tenant_admin,
    }
}

#[async_trait(?Send)]
impl ConnectionService for NodeDbConnectionService {
    async fn list_connections(&self) -> Result<Vec<SavedConnection>, StudioError> {
        Ok(self.saved.borrow().clone())
    }

    // The activity log, notifications, recent items, and tab badges are
    // Studio's own records, not server data. Nothing records them yet, so
    // each one is empty.
    async fn connection_activity(&self) -> Result<Vec<ConnectionEvent>, StudioError> {
        Ok(Vec::new())
    }

    async fn notifications(&self) -> Result<Vec<Notification>, StudioError> {
        Ok(Vec::new())
    }

    async fn recent_items(&self) -> Result<Vec<RecentItem>, StudioError> {
        Ok(Vec::new())
    }

    async fn connect(
        &self,
        name: &str,
        creds: &Credentials,
    ) -> Result<ActiveConnection, StudioError> {
        // A new session would drop the open transaction's connection, and
        // the server would roll it back with no word to the user.
        if self.tx_open() {
            return Err(StudioError::TransactionOpen);
        }
        let entry = self
            .saved
            .borrow()
            .iter()
            .find(|c| c.name == name)
            .cloned()
            .ok_or(StudioError::NotConnected)?;
        let secret = creds
            .password
            .clone()
            .or_else(|| self.secrets.borrow().get(name).cloned());
        let config = pool_config(
            &entry.target,
            entry.auth,
            &creds.username,
            secret.as_deref(),
            &entry.tls,
            &entry.database,
        )?;
        let seen = match probe(&config, &entry.database).await {
            Ok(seen) => seen,
            Err(e) => {
                if matches!(e, StudioError::Connection(_)) {
                    self.update(name, |c| c.status = ConnStatus::Offline);
                }
                return Err(e);
            }
        };
        let Identity {
            user,
            tenant_id,
            role,
            ..
        } = seen.who;
        let database = database_name(&entry.database);
        // Multi-node only: a single-node server refuses `SHOW CLUSTER`.
        let cluster = seen.client.query("SHOW CLUSTER").await.is_ok();

        let now = now_epoch();
        let observation = Observation {
            at: now,
            ping_ms: seen.ping_ms,
            db_count: u32::try_from(seen.databases.len()).unwrap_or(u32::MAX),
            version: seen.version.clone(),
            // The client does not expose the server certificate.
            tls: if entry.tls.required {
                TlsStatus::NotChecked
            } else {
                TlsStatus::NotUsed
            },
        };
        let sub = self
            .update(name, |c| {
                c.status = ConnStatus::Available;
                c.last_used = now;
                c.observation = Some(observation);
                c.summary()
            })
            .unwrap_or_default();
        let mut session = Session::new(config, database.clone(), seen.version, &user);
        // Studio's own records belong to the saved connection and the
        // tenant, so a reconnect finds them.
        session.connection = name.to_string();
        session.tenant_id = tenant_id;
        // A BEGIN can have finished while the server answered.
        if self.tx_open() {
            return Err(StudioError::TransactionOpen);
        }
        *self.session.borrow_mut() = Some(session);
        Ok(ActiveConnection {
            name: name.to_string(),
            sub,
            display_name: user.clone(),
            user,
            role,
            capabilities: Capabilities {
                graph: true,
                vector: true,
                events: true,
                timeseries: true,
                spatial: true,
                search: true,
                // The server has no statement that lists Lite devices.
                sync: false,
                cluster,
                // ponytail: assumes the user can write. Read the session's
                // grants when a read-only login must hide the write actions.
                readonly: false,
                // 0.6.2 cannot do these, so the UI must not offer them.
                // Records carry a content fingerprint, not a revision.
                record_revisions: false,
                // It accepts BEGIN READ ONLY, then runs writes inside it.
                read_only_transactions: false,
                // No statement describes an array.
                arrays: false,
                // No source for the Backups, Config and Credentials tabs.
                backups: false,
                config: false,
                credentials: false,
                // A parse error: 0.6.2 has no EXPLAIN ANALYZE.
                explain_analyze: false,
                cluster_caps: crate::state::caps::cluster::ClusterCaps::nodedb_0_6(),
            },
            current_database: database,
        })
    }

    async fn test_connection(
        &self,
        draft: &ConnectionDraft,
        creds: &Credentials,
    ) -> Result<ConnectionTest, StudioError> {
        let config = pool_config(
            &draft.target,
            draft.auth,
            &draft.username,
            creds.password.as_deref(),
            &draft.tls,
            &draft.database,
        )?;
        let seen = probe(&config, &draft.database).await?;
        Ok(ConnectionTest {
            latency_ms: seen.ping_ms,
            tls_verified: draft.tls.required,
            server_version: seen.version,
            db_count: u32::try_from(seen.databases.len()).unwrap_or(u32::MAX),
        })
    }

    async fn save_connection(
        &self,
        draft: ConnectionDraft,
        creds: Credentials,
        replacing: Option<String>,
    ) -> Result<SavedConnection, StudioError> {
        let mut saved = self.saved.borrow_mut();
        if saved
            .iter()
            .any(|c| c.name == draft.name && replacing.as_deref() != Some(c.name.as_str()))
        {
            return Err(StudioError::NameTaken(draft.name));
        }
        let mut secrets = self.secrets.borrow_mut();
        let entry = match replacing {
            Some(old) => {
                let entry = saved
                    .iter_mut()
                    .find(|c| c.name == old)
                    .ok_or_else(|| StudioError::UnknownConnection(old.clone()))?;
                entry.apply_draft(&draft);
                // A rename keeps the secret the entry had.
                if let Some(secret) = secrets.remove(&old) {
                    secrets.insert(draft.name.clone(), secret);
                }
                entry.clone()
            }
            None => {
                let now = now_epoch();
                let entry = SavedConnection {
                    name: draft.name.clone(),
                    description: String::new(),
                    target: draft.target.clone(),
                    database: draft.database.clone(),
                    auth: draft.auth,
                    username: draft.username.clone(),
                    keychain: draft.keychain,
                    tls: draft.tls.clone(),
                    status: ConnStatus::NotConnected,
                    last_used: now,
                    created: now,
                    observation: None,
                    profile: None,
                };
                saved.push(entry.clone());
                entry
            }
        };
        if let Some(secret) = creds.password.filter(|s| !s.is_empty()) {
            secrets.insert(entry.name.clone(), secret);
        }
        Ok(entry)
    }

    async fn duplicate_connection(&self, _name: &str) -> Result<SavedConnection, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn mark_all_read(&self) -> Result<(), StudioError> {
        Ok(())
    }

    async fn mark_read(&self, _id: &str) -> Result<(), StudioError> {
        Ok(())
    }

    async fn nav_badges(&self) -> Result<NavBadges, StudioError> {
        Ok(NavBadges { query: 0 })
    }

    async fn session_info(&self) -> Result<SessionInfo, StudioError> {
        let session = self.session.borrow();
        let session = session.as_ref().ok_or(StudioError::NotConnected)?;
        Ok(SessionInfo {
            server_version: session.server_version.clone(),
        })
    }

    async fn databases(&self) -> Result<Vec<String>, StudioError> {
        let client = self.client(None)?;
        let current = self
            .session
            .borrow()
            .as_ref()
            .map(|s| s.database.clone())
            .ok_or(StudioError::NotConnected)?;
        let who = identity_of(&client).await?;
        databases_of(&client, &current, who.admin).await
    }

    async fn select_database(&self, name: &str) -> Result<(), StudioError> {
        if !self.databases().await?.iter().any(|d| d == name) {
            return Err(StudioError::UnknownDatabase(name.to_string()));
        }
        let mut session = self.session.borrow_mut();
        let session = session.as_mut().ok_or(StudioError::NotConnected)?;
        session.database = name.to_string();
        Ok(())
    }
}

impl NodeDbConnectionService {
    /// Change the saved entry `name` and return what `f` returns. `None`
    /// when no entry has the name.
    fn update<T>(&self, name: &str, f: impl FnOnce(&mut SavedConnection) -> T) -> Option<T> {
        self.saved
            .borrow_mut()
            .iter_mut()
            .find(|c| c.name == name)
            .map(f)
    }
}
