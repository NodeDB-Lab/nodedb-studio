//! The real-client backend: one live session over `nodedb-client`'s native
//! transport.
//!
//! Wired today: the connection registry, connect, test, databases, the
//! catalog, and the query workbench. Every other seam method returns
//! `StudioError::NotConnected` until it is wired.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use nodedb_client::native::pool::PoolConfig;
use nodedb_client::{NativeClient, NodeDbError, QueryResult};
use nodedb_types::protocol::AuthMethod as WireAuth;

use crate::data::mock::WorkbenchStore;
use crate::models::connections::{AuthMethod, ConnectionTarget, TlsConfig};
use crate::services::decode::Table;
use crate::services::error::StudioError;
use crate::services::nodedb::workbench_tx::Tx;
use crate::state::connections_registry::SavedConnection;

/// The backend that talks to a NodeDB server.
#[derive(Default)]
pub struct NodeDbConnectionService {
    /// The saved-connection registry.
    // ponytail: memory only, so a restart starts empty. Persist it with
    // `state::persist` when connections must survive a restart.
    pub(super) saved: RefCell<Vec<SavedConnection>>,
    /// The password or token of each saved connection, by name.
    // ponytail: memory only and never written to disk, so a restart asks
    // again. The OS keychain replaces this map.
    pub(super) secrets: RefCell<HashMap<String, String>>,
    pub(super) session: RefCell<Option<Session>>,
    /// Studio's own records, saved queries and console history, by saved
    /// connection name and signed-in user. A reconnect keeps them.
    // ponytail: memory only, so a restart starts empty, and a renamed
    // connection starts a new library. Persist it with `state::persist`
    // when saved queries must survive a restart.
    libraries: RefCell<HashMap<(String, String), WorkbenchStore>>,
}

/// The last statement `run_query` executed, kept so a page change reads the
/// buffered rows and never executes the statement a second time.
pub(super) struct LastRun {
    pub database: String,
    pub sql: String,
    pub result: QueryResult,
    pub elapsed_ms: u32,
}

/// A number no other session or transaction of this process holds.
pub(super) fn next_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// One open session: who and where, plus a client per database it has used.
pub(super) struct Session {
    /// Tells this session from the one a reconnect replaces it with.
    pub id: u64,
    /// The saved connection it opened. Blank outside `connect`.
    pub connection: String,
    /// The user the server signed in.
    pub user: String,
    pub config: PoolConfig,
    /// The database session-scoped reads answer for.
    pub database: String,
    pub server_version: String,
    clients: HashMap<String, Rc<NativeClient>>,
    pub last_run: Option<LastRun>,
    /// The open transaction. Dropping the session closes its connection,
    /// and the server rolls back what a closed connection left open.
    pub tx: Option<Tx>,
}

impl Session {
    pub fn new(config: PoolConfig, database: String, server_version: String, user: &str) -> Self {
        Self {
            id: next_id(),
            connection: String::new(),
            user: user.to_string(),
            config,
            database,
            server_version,
            clients: HashMap::new(),
            last_run: None,
            tx: None,
        }
    }

    /// `database`, or the session's database when blank.
    pub fn database_or_default(&self, database: &str) -> String {
        database_of(database).unwrap_or_else(|| self.database.clone())
    }
}

fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

fn setup(detail: &str) -> StudioError {
    StudioError::from(NodeDbError::config(detail))
}

/// The pool config a target and an identity describe. `database` blank means
/// the server default.
pub(super) fn pool_config(
    target: &ConnectionTarget,
    auth: AuthMethod,
    username: &str,
    secret: Option<&str>,
    tls: &TlsConfig,
    database: &str,
) -> Result<PoolConfig, StudioError> {
    let ConnectionTarget::Origin { host, port } = target else {
        return Err(setup("NodeDB-Lite files cannot be opened yet"));
    };
    let secret = secret.filter(|s| !s.is_empty());
    let wire = match auth {
        AuthMethod::UserPassword => {
            let username = username.trim();
            if username.is_empty() {
                return Err(StudioError::MissingUsername);
            }
            match secret {
                Some(password) => WireAuth::Password {
                    username: username.to_string(),
                    password: password.to_string(),
                },
                // No password: the server accepts this only in trust mode.
                None => WireAuth::Trust {
                    username: username.to_string(),
                },
            }
        }
        AuthMethod::Token => WireAuth::ApiKey {
            token: secret
                .ok_or_else(|| setup("this connection needs a token"))?
                .to_string(),
        },
        AuthMethod::SystemCredentials => {
            return Err(setup("system credentials cannot be used yet"));
        }
    };
    // The client has no client-certificate support. Refuse instead of
    // connecting without the certificate the user asked for.
    if !tls.client_cert.is_empty() || !tls.client_key.is_empty() {
        return Err(setup("client certificates cannot be used yet"));
    }
    let mut config = PoolConfig::new(format!("{host}:{port}"), wire);
    config.max_size = 4;
    config.database = database_of(database);
    config.tls.enabled = tls.required;
    config.tls.server_name = Some(tls.server_name.clone()).filter(|s| !s.is_empty());
    config.tls.ca_cert_path = Some(tls.ca_file.clone())
        .filter(|s| !s.is_empty())
        .map(Into::into);
    Ok(config)
}

fn database_of(name: &str) -> Option<String> {
    Some(name.trim().to_string()).filter(|s| !s.is_empty())
}

/// A client for `database` of the server `config` names.
pub(super) fn client_for(config: &PoolConfig, database: &str) -> Rc<NativeClient> {
    Rc::new(NativeClient::new(PoolConfig {
        database: database_of(database),
        ..config.clone()
    }))
}

/// A server result as display strings.
pub(super) fn to_table(result: &QueryResult) -> Table {
    Table {
        columns: result.columns.clone(),
        rows: result
            .rows
            .iter()
            .map(|row| row.iter().map(ToString::to_string).collect())
            .collect(),
    }
}

impl NodeDbConnectionService {
    /// The client for `database`, or for the session's database when `None`.
    /// The caller awaits on the returned client, never on a `session` borrow.
    pub(super) fn client(&self, database: Option<&str>) -> Result<Rc<NativeClient>, StudioError> {
        let mut session = self.session.borrow_mut();
        let session = session.as_mut().ok_or(StudioError::NotConnected)?;
        let database = database
            .filter(|d| !d.trim().is_empty())
            .unwrap_or(&session.database)
            .to_string();
        let config = &session.config;
        Ok(session
            .clients
            .entry(database)
            .or_insert_with_key(|db| client_for(config, db))
            .clone())
    }

    /// Run `sql` on the session's database and return its rows as strings.
    pub(super) async fn table(&self, sql: &str) -> Result<Table, StudioError> {
        let client = self.client(None)?;
        Ok(to_table(&client.query(sql).await?))
    }

    /// Run `f` on Studio's own records for the session's connection and
    /// user. Each user owns the queries they save.
    pub(super) fn local<T>(
        &self,
        f: impl FnOnce(&mut WorkbenchStore) -> Result<T, StudioError>,
    ) -> Result<T, StudioError> {
        let key = {
            let session = self.session.borrow();
            let s = session.as_ref().ok_or(StudioError::NotConnected)?;
            (s.connection.clone(), s.user.clone())
        };
        let mut libraries = self.libraries.borrow_mut();
        f(libraries
            .entry(key)
            .or_insert_with_key(|(_, user)| WorkbenchStore::empty(user, now_epoch)))
    }
}

/// A service connected to a live server, for the `#[ignore]` live tests.
/// `NODEDB_PROBE_ADDR` is `host:port` of the native listener.
/// `NODEDB_PROBE_USER` and `NODEDB_PROBE_PASSWORD` are the login.
/// Run them with `cargo nextest run --run-ignored ignored-only -E 'test(live_)'`.
#[cfg(test)]
pub(super) async fn live() -> NodeDbConnectionService {
    let var = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("set {name}"));
    live_as(&var("NODEDB_PROBE_USER"), &var("NODEDB_PROBE_PASSWORD"))
        .await
        .0
}

/// `live`, as another user of the same server. Also returns the session
/// `connect` opened.
#[cfg(test)]
pub(super) async fn live_as(
    username: &str,
    password: &str,
) -> (
    NodeDbConnectionService,
    crate::state::connection::ActiveConnection,
) {
    use crate::models::connections::ConnectionDraft;
    use crate::services::connection_service::ConnectionService;
    use crate::state::connections_registry::Credentials;

    let addr = std::env::var("NODEDB_PROBE_ADDR").expect("set NODEDB_PROBE_ADDR");
    let (host, port) = addr
        .rsplit_once(':')
        .expect("NODEDB_PROBE_ADDR is host:port");
    let creds = Credentials {
        username: username.to_string(),
        password: Some(password.to_string()),
    };
    let draft = ConnectionDraft {
        name: "probe".into(),
        target: ConnectionTarget::Origin {
            host: host.into(),
            port: port.parse().expect("the port is a number"),
        },
        database: String::new(),
        auth: AuthMethod::UserPassword,
        username: creds.username.clone(),
        keychain: false,
        tls: TlsConfig::default(),
    };
    let svc = NodeDbConnectionService::default();
    svc.save_connection(draft, creds.clone(), None)
        .await
        .expect("the draft saves");
    let session = svc
        .connect("probe", &creds)
        .await
        .expect("the probe connects");
    (svc, session)
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;
    use crate::models::connections::ConnectionDraft;
    use crate::models::feeds::FeedSession;
    use crate::services::connection_service::ConnectionService;
    use crate::services::error::StudioError;
    use crate::services::feed_data::FeedData;
    use crate::services::job_data::JobData;
    use crate::services::listen_data::ListenData;
    use crate::services::matview_data::MatViewData;
    use crate::services::search_data::SearchData;
    use crate::services::spatial_data::SpatialData;
    use crate::services::sync_data::SyncData;
    use crate::services::timeseries_data::TimeseriesData;
    use crate::services::topic_data::TopicData;
    use crate::services::vector_data::VectorData;
    use crate::services::workbench_data::WorkbenchData;
    use crate::state::connections_registry::Credentials;

    fn draft(name: &str) -> ConnectionDraft {
        ConnectionDraft {
            name: name.into(),
            target: ConnectionTarget::Origin {
                host: "localhost".into(),
                port: 6433,
            },
            database: String::new(),
            auth: AuthMethod::UserPassword,
            username: "alice".into(),
            keychain: false,
            tls: TlsConfig::default(),
        }
    }

    fn creds() -> Credentials {
        Credentials {
            username: "alice".into(),
            password: Some("pw".into()),
        }
    }

    #[tokio::test]
    async fn no_session_reads_the_empty_answers() {
        let svc = NodeDbConnectionService::default();
        assert!(svc.list_connections().await.expect("lists").is_empty());
        assert!(svc.notifications().await.expect("reads").is_empty());
        assert!(svc.recent_items().await.expect("reads").is_empty());
        assert!(svc.connection_activity().await.expect("reads").is_empty());
        assert!(svc.mark_all_read().await.is_ok());
        assert!(svc.mark_read("id").await.is_ok());
        assert_eq!(svc.nav_badges().await.expect("reads").query, 0);
    }

    #[tokio::test]
    async fn no_session_is_not_connected() {
        let svc = NodeDbConnectionService::default();
        assert!(matches!(
            svc.connect("anything", &creds()).await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.session_info().await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.databases().await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.select_database("x").await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.duplicate_connection("x").await,
            Err(StudioError::NotConnected)
        ));
    }

    #[tokio::test]
    async fn registry_saves_then_lists() {
        let svc = NodeDbConnectionService::default();
        let saved = svc
            .save_connection(draft("a"), creds(), None)
            .await
            .expect("saves");
        assert_eq!(saved.name, "a");
        let list = svc.list_connections().await.expect("lists");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "a");
    }

    #[tokio::test]
    async fn registry_refuses_a_duplicate_name() {
        let svc = NodeDbConnectionService::default();
        svc.save_connection(draft("a"), creds(), None)
            .await
            .expect("saves");
        assert!(matches!(
            svc.save_connection(draft("a"), creds(), None).await,
            Err(StudioError::NameTaken(n)) if n == "a"
        ));
        assert_eq!(svc.list_connections().await.expect("lists").len(), 1);
    }

    #[tokio::test]
    async fn registry_edit_renames_and_keeps_the_entry() {
        let svc = NodeDbConnectionService::default();
        svc.save_connection(draft("a"), creds(), None)
            .await
            .expect("saves");
        let renamed = svc
            .save_connection(draft("b"), Credentials::default(), Some("a".into()))
            .await
            .expect("edits");
        assert_eq!(renamed.name, "b");
        let list = svc.list_connections().await.expect("lists");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "b");
        // The rename keeps the secret the entry had.
        assert_eq!(
            svc.secrets.borrow().get("b").map(String::as_str),
            Some("pw")
        );
        assert!(!svc.secrets.borrow().contains_key("a"));
    }

    #[tokio::test]
    async fn registry_edit_may_keep_its_own_name() {
        let svc = NodeDbConnectionService::default();
        svc.save_connection(draft("a"), creds(), None)
            .await
            .expect("saves");
        assert!(
            svc.save_connection(draft("a"), creds(), Some("a".into()))
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn registry_edit_of_a_missing_entry_is_unknown() {
        let svc = NodeDbConnectionService::default();
        assert!(matches!(
            svc.save_connection(draft("a"), creds(), Some("ghost".into()))
                .await,
            Err(StudioError::UnknownConnection(n)) if n == "ghost"
        ));
    }

    fn config_of(
        target: &ConnectionTarget,
        auth: AuthMethod,
        username: &str,
        secret: Option<&str>,
        tls: &TlsConfig,
    ) -> Result<PoolConfig, StudioError> {
        pool_config(target, auth, username, secret, tls, "")
    }

    fn origin() -> ConnectionTarget {
        ConnectionTarget::Origin {
            host: "localhost".into(),
            port: 6433,
        }
    }

    #[test]
    fn pool_config_needs_a_username() {
        let r = config_of(
            &origin(),
            AuthMethod::UserPassword,
            "  ",
            Some("pw"),
            &TlsConfig::default(),
        );
        assert!(matches!(r, Err(StudioError::MissingUsername)));
    }

    #[test]
    fn pool_config_refuses_a_lite_target() {
        let lite = ConnectionTarget::Lite {
            path: "/tmp/x".into(),
        };
        let r = config_of(
            &lite,
            AuthMethod::UserPassword,
            "alice",
            Some("pw"),
            &TlsConfig::default(),
        );
        assert!(r.is_err());
    }

    #[test]
    fn pool_config_token_needs_a_token() {
        let tls = TlsConfig::default();
        assert!(config_of(&origin(), AuthMethod::Token, "", None, &tls).is_err());
        assert!(config_of(&origin(), AuthMethod::Token, "", Some(""), &tls).is_err());
        assert!(config_of(&origin(), AuthMethod::Token, "", Some("tok"), &tls).is_ok());
    }

    #[test]
    fn pool_config_refuses_a_client_certificate() {
        let tls = TlsConfig {
            client_cert: "/c.pem".into(),
            client_key: "/k.pem".into(),
            ..TlsConfig::default()
        };
        let r = config_of(
            &origin(),
            AuthMethod::UserPassword,
            "alice",
            Some("pw"),
            &tls,
        );
        assert!(r.is_err());
    }

    #[test]
    fn pool_config_builds_the_address() {
        let config = config_of(
            &origin(),
            AuthMethod::UserPassword,
            "alice",
            Some("pw"),
            &TlsConfig::default(),
        )
        .expect("builds");
        assert_eq!(config.addr, "localhost:6433");
    }

    #[tokio::test]
    async fn stub_events_reads_and_session_are_not_connected() {
        let svc = NodeDbConnectionService::default();
        let session = FeedSession {
            feed: "feed_01".into(),
            group: "studio_feed_01".into(),
        };
        assert!(matches!(
            svc.open_feed_session("feed_01").await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.read_feed_events(&session, 10).await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.commit_feed_offsets(&session).await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.close_feed_session(&session).await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.change_feeds().await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.materialized_views().await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(svc.topics().await, Err(StudioError::NotConnected)));
        assert!(matches!(
            svc.scheduled_jobs().await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.notify_channels().await,
            Err(StudioError::NotConnected)
        ));
    }

    #[tokio::test]
    async fn stub_workbench_reads_are_not_connected() {
        let svc = NodeDbConnectionService::default();
        let request = crate::models::workbench::QueryRequest {
            sql: "SELECT 1".into(),
            params: Vec::new(),
            database: "analytics".into(),
            page: 0,
            page_size: 50,
        };
        assert!(matches!(
            svc.run_query(&request).await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.explain(&request, crate::models::query_plan::ExplainMode::Explain)
                .await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.schema_tree().await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.transaction().await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.query_library().await,
            Err(StudioError::NotConnected)
        ));
    }

    #[tokio::test]
    async fn stub_viewer_reads_are_not_connected() {
        let svc = NodeDbConnectionService::default();
        assert!(matches!(
            svc.vector_setup().await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.vector_index_status("articles", "embedding").await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.time_sources().await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.backfill_runs(None).await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.spatial_setup().await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.search_setup().await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.search_indexes("articles").await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.sync_devices().await,
            Err(StudioError::NotConnected)
        ));
    }

    #[test]
    fn stub_is_object_safe_behind_rc() {
        // Compile-time guarantee: the stub coerces to the seam trait object,
        // exactly as `app.rs` provides it via context.
        let _s: Rc<dyn ConnectionService> = Rc::new(NodeDbConnectionService::default());
    }

    #[test]
    fn stub_is_object_safe_behind_backend() {
        use crate::services::backend::Backend;
        let _b: Rc<dyn Backend> = Rc::new(NodeDbConnectionService::default());
    }

    #[tokio::test]
    #[ignore = "needs a live NodeDB 0.6.2 server: see `live`"]
    async fn live_connect_reads_the_session_and_its_databases() {
        let svc = super::live().await;
        let saved = svc.list_connections().await.expect("the registry reads");
        println!("saved: {saved:#?}");
        let dbs = svc.databases().await.expect("databases read");
        println!("databases: {dbs:?}");
        assert!(dbs.iter().any(|d| d == "default"), "got {dbs:?}");
        let info = svc.session_info().await.expect("session info reads");
        assert!(info.server_version.starts_with("0.6"), "got {info:?}");
        assert!(matches!(
            svc.select_database("no_such_db").await,
            Err(StudioError::UnknownDatabase(_))
        ));
    }

    #[tokio::test]
    #[ignore = "needs a live NodeDB 0.6.2 server: see `live`"]
    async fn live_connect_works_for_a_user_who_cannot_list_databases() {
        let admin = super::live().await;
        // Fails when the user exists from an earlier run: that is fine.
        let _ = admin
            .client(None)
            .expect("a session")
            .query("CREATE USER studio_probe_c_ro WITH PASSWORD 'cropass1' ROLE readonly")
            .await;
        let (svc, session) = super::live_as("studio_probe_c_ro", "cropass1").await;
        assert_eq!(session.user, "studio_probe_c_ro");
        assert_eq!(session.role, "readonly");
        assert_eq!(session.current_database, "default");
        let dbs = svc.databases().await.expect("databases read");
        assert_eq!(dbs, ["default"]);

        let (_, admin_session) = super::live_as(
            &std::env::var("NODEDB_PROBE_USER").expect("set NODEDB_PROBE_USER"),
            &std::env::var("NODEDB_PROBE_PASSWORD").expect("set NODEDB_PROBE_PASSWORD"),
        )
        .await;
        assert_eq!(admin_session.role, "superuser");
    }
}
