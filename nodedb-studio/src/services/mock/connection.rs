//! Connections, notifications, and session reads and writes.

use async_trait::async_trait;

use crate::data::mock;
use crate::models::connections::{
    AuthMethod, ConnStatus, ConnectionDraft, ConnectionEvent, ConnectionTarget, ConnectionTest,
};
use crate::models::notification::Notification;
use crate::models::shell::{NavBadges, RecentItem, SessionInfo};
use crate::services::connection_service::ConnectionService;
use crate::services::error::StudioError;
use crate::services::mock::MockConnectionService;
use crate::services::mock_behavior::{apply, apply_one, apply_one_or_empty};
use crate::state::connection::ActiveConnection;
use crate::state::connections_registry::{Credentials, SavedConnection};

fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

fn unreachable(host: &str) -> StudioError {
    StudioError::from(nodedb_client::NodeDbError::node_unreachable(host))
}

/// The first of `name copy`, `name copy 2`, ... that no entry uses.
fn free_copy_name(saved: &[SavedConnection], name: &str) -> String {
    let taken = |n: &str| saved.iter().any(|c| c.name == n);
    let first = format!("{name} copy");
    if !taken(&first) {
        return first;
    }
    (2u32..)
        .map(|i| format!("{name} copy {i}"))
        .find(|n| !taken(n))
        .unwrap_or(first)
}

impl MockConnectionService {
    /// The databases of the session `connect` opened.
    fn session_databases(&self) -> Option<Vec<String>> {
        let session = self.session.borrow().clone()?;
        self.saved
            .borrow()
            .iter()
            .find(|c| c.name == session)
            .and_then(|c| c.profile.as_ref())
            .map(|p| p.databases.clone())
    }
}

#[async_trait(?Send)]
impl ConnectionService for MockConnectionService {
    async fn list_connections(&self) -> Result<Vec<SavedConnection>, StudioError> {
        let saved = self.saved.borrow().clone();
        apply(self.behavior, move || saved).await
    }

    async fn connection_activity(&self) -> Result<Vec<ConnectionEvent>, StudioError> {
        apply(self.behavior, mock::connection_activity).await
    }

    async fn notifications(&self) -> Result<Vec<Notification>, StudioError> {
        let all_read = self.all_read.get();
        let read = self.read.borrow().clone();
        apply(self.behavior, move || {
            let mut notifs = mock::notifications();
            for n in &mut notifs {
                if all_read || read.contains(&n.id) {
                    n.unread = false;
                }
            }
            notifs
        })
        .await
    }

    async fn recent_items(&self) -> Result<Vec<RecentItem>, StudioError> {
        apply(self.behavior, mock::recent_items).await
    }

    async fn connect(
        &self,
        name: &str,
        creds: &Credentials,
    ) -> Result<ActiveConnection, StudioError> {
        // The blank-username guard must run before any behaviour branch: a
        // caller with bad input gets `MissingUsername`, never a behaviour
        // error, even when the service is configured `Erroring`.
        if creds.username.trim().is_empty() {
            return Err(StudioError::MissingUsername);
        }
        let entry = self.saved.borrow().iter().find(|c| c.name == name).cloned();
        let entry = apply_one(self.behavior, move || entry)
            .await?
            .ok_or(StudioError::NotConnected)?;
        if entry.status == ConnStatus::Offline {
            return Err(unreachable(entry.target.host().unwrap_or(&entry.name)));
        }
        let session = entry.open().ok_or(StudioError::NotConnected)?;
        *self.session.borrow_mut() = Some(session.name.clone());
        Ok(session)
    }

    async fn test_connection(
        &self,
        draft: &ConnectionDraft,
        _creds: &Credentials,
    ) -> Result<ConnectionTest, StudioError> {
        if draft.auth == AuthMethod::UserPassword && draft.username.trim().is_empty() {
            return Err(StudioError::MissingUsername);
        }
        // A saved entry with the same host answers for it: an offline one
        // does not reach, an observed one reports what its last check saw.
        let known = draft.target.host().and_then(|host| {
            self.saved
                .borrow()
                .iter()
                .find(|c| c.target.host() == Some(host))
                .cloned()
        });
        let tls_verified = draft.tls.required && draft.target.host().is_some();
        apply_one(self.behavior, move || match known {
            Some(c) if c.status == ConnStatus::Offline => {
                Err(unreachable(c.target.host().unwrap_or(&c.name)))
            }
            Some(c) => Ok(c.observation.map_or(
                ConnectionTest {
                    latency_ms: 22,
                    tls_verified,
                    server_version: "1.4.2".into(),
                    db_count: 12,
                },
                |o| ConnectionTest {
                    latency_ms: o.ping_ms,
                    tls_verified,
                    server_version: o.version,
                    db_count: o.db_count,
                },
            )),
            None => Ok(match draft.target {
                ConnectionTarget::Lite { .. } => ConnectionTest {
                    latency_ms: 8,
                    tls_verified: false,
                    server_version: "1.4.2".into(),
                    db_count: 3,
                },
                ConnectionTarget::Origin { .. } => ConnectionTest {
                    latency_ms: 22,
                    tls_verified,
                    server_version: "1.4.2".into(),
                    db_count: 12,
                },
            }),
        })
        .await?
    }

    async fn save_connection(
        &self,
        draft: ConnectionDraft,
        // The mock holds no keychain: the secret is dropped here, as a
        // `keychain: false` save drops it in the real client.
        _creds: Credentials,
        replacing: Option<String>,
    ) -> Result<SavedConnection, StudioError> {
        apply_one(self.behavior, || ()).await?;
        let mut saved = self.saved.borrow_mut();
        if saved
            .iter()
            .any(|c| c.name == draft.name && replacing.as_deref() != Some(c.name.as_str()))
        {
            return Err(StudioError::NameTaken(draft.name));
        }
        match replacing {
            Some(old) => {
                let entry = saved
                    .iter_mut()
                    .find(|c| c.name == old)
                    .ok_or(StudioError::UnknownConnection(old))?;
                entry.apply_draft(&draft);
                Ok(entry.clone())
            }
            None => {
                let entry = mock::saved_from_draft(&draft, now_epoch());
                saved.push(entry.clone());
                Ok(entry)
            }
        }
    }

    async fn duplicate_connection(&self, name: &str) -> Result<SavedConnection, StudioError> {
        apply_one(self.behavior, || ()).await?;
        let mut saved = self.saved.borrow_mut();
        let source = saved
            .iter()
            .find(|c| c.name == name)
            .cloned()
            .ok_or_else(|| StudioError::UnknownConnection(name.to_string()))?;
        let now = now_epoch();
        let copy = SavedConnection {
            name: free_copy_name(&saved, name),
            last_used: now,
            created: now,
            ..source
        };
        saved.push(copy.clone());
        Ok(copy)
    }

    async fn mark_all_read(&self) -> Result<(), StudioError> {
        // Route the write through the same behaviour switch as every read, so
        // the error path is reachable in tests.
        apply_one(self.behavior, || ()).await?;
        self.all_read.set(true);
        Ok(())
    }

    async fn mark_read(&self, id: &str) -> Result<(), StudioError> {
        apply_one(self.behavior, || ()).await?;
        self.read.borrow_mut().insert(id.to_string());
        Ok(())
    }

    async fn nav_badges(&self) -> Result<NavBadges, StudioError> {
        apply_one(self.behavior, mock::nav_badges).await
    }

    async fn session_info(&self) -> Result<SessionInfo, StudioError> {
        apply_one(self.behavior, mock::session_info).await
    }

    async fn databases(&self) -> Result<Vec<String>, StudioError> {
        // The list belongs to the session `connect` opened, as on a server, so
        // switching connections switches lists. Behaviour applies first: an
        // `Erroring` mock fails the same way connected or not.
        let listed = self.session_databases();
        apply_one_or_empty(self.behavior, move || listed, || Some(Vec::new()))
            .await?
            .ok_or(StudioError::NotConnected)
    }

    async fn select_database(&self, name: &str) -> Result<(), StudioError> {
        let listed = self.session_databases();
        let name = name.to_string();
        apply_one(self.behavior, move || match listed {
            None => Err(StudioError::NotConnected),
            Some(dbs) if dbs.contains(&name) => Ok(()),
            Some(_) => Err(StudioError::UnknownDatabase(name)),
        })
        .await?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::cdc::CdcRow;
    use crate::services::async_state::AsyncState;
    use crate::services::mock_behavior::MockBehavior;
    use crate::services::streams_data::StreamsData;

    #[tokio::test]
    async fn mark_all_read_persists_across_reads() {
        let svc = MockConnectionService::ready();
        let before = svc.notifications().await.expect("mock infallible");
        assert!(before.iter().any(|n| n.unread), "fixture has unread items");
        svc.mark_all_read()
            .await
            .expect("ready mock write succeeds");
        let after = svc.notifications().await.expect("mock infallible");
        assert!(
            after.iter().all(|n| !n.unread),
            "all read after mark_all_read"
        );
    }

    #[tokio::test]
    async fn mock_notifications_returns_data() {
        let svc = MockConnectionService::ready();
        let notifs = svc
            .notifications()
            .await
            .expect("mock notifications are infallible");
        assert!(!notifs.is_empty());
    }

    #[tokio::test]
    async fn mock_list_connections_returns_data() {
        let svc = MockConnectionService::ready();
        let conns = svc
            .list_connections()
            .await
            .expect("mock list_connections is infallible");
        assert!(!conns.is_empty());
    }

    #[tokio::test]
    async fn list_connections_empty_behavior_returns_no_rows() {
        let svc = MockConnectionService::empty();
        let conns = svc.list_connections().await.expect("empty behaviour is Ok");
        assert!(conns.is_empty());
    }

    #[tokio::test]
    async fn list_connections_erroring_is_retriable_error() {
        let svc = MockConnectionService::erroring();
        let err = svc
            .list_connections()
            .await
            .expect_err("erroring must fail");
        assert!(err.is_retriable());
    }

    #[tokio::test]
    async fn notifications_empty_behavior_returns_no_rows() {
        let svc = MockConnectionService::empty();
        let notifs = svc.notifications().await.expect("empty behaviour is Ok");
        assert!(notifs.is_empty());
    }

    #[tokio::test]
    async fn notifications_erroring_is_retriable_error() {
        let svc = MockConnectionService::erroring();
        let err = svc.notifications().await.expect_err("erroring must fail");
        assert!(err.is_retriable());
    }

    #[tokio::test]
    async fn mock_connect_known_name_returns_session() {
        let svc = MockConnectionService::ready();
        let creds = Credentials {
            username: "alice".into(),
            password: None,
        };
        // `staging-cluster` is a connectable Online mock connection (data/mock.rs).
        let session = svc.connect("staging-cluster", &creds).await;
        assert!(session.is_ok());
    }

    #[tokio::test]
    async fn mock_connect_unknown_name_is_not_connected() {
        let svc = MockConnectionService::ready();
        let creds = Credentials {
            username: "alice".into(),
            password: None,
        };
        assert!(matches!(
            svc.connect("does-not-exist", &creds).await,
            Err(StudioError::NotConnected)
        ));
    }

    #[tokio::test]
    async fn connect_rejects_blank_username() {
        let svc = MockConnectionService::ready();
        let creds = Credentials {
            username: "   ".into(),
            password: None,
        };
        let out = svc.connect("local-dev", &creds).await;
        assert!(
            matches!(out, Err(StudioError::MissingUsername)),
            "blank username must be rejected, never defaulted to admin"
        );
    }

    #[tokio::test]
    async fn connect_accepts_explicit_username() {
        let svc = MockConnectionService::ready();
        let name = mock::connections()
            .first()
            .map(|c| c.name.clone())
            .expect("fixture must have at least one connection");
        let creds = Credentials {
            username: "alice".into(),
            password: None,
        };
        assert!(svc.connect(&name, &creds).await.is_ok());
    }

    #[tokio::test]
    async fn connect_erroring_is_retriable_error() {
        let svc = MockConnectionService::erroring();
        let creds = Credentials {
            username: "alice".into(),
            password: None,
        };
        let err = svc
            .connect("staging-cluster", &creds)
            .await
            .expect_err("erroring must fail");
        assert!(err.is_retriable());
    }

    #[tokio::test]
    async fn connect_delayed_still_resolves() {
        let svc = MockConnectionService::delayed(std::time::Duration::from_millis(5));
        let creds = Credentials {
            username: "alice".into(),
            password: None,
        };
        assert!(svc.connect("staging-cluster", &creds).await.is_ok());
    }

    #[tokio::test]
    async fn connect_checks_missing_username_before_consulting_behavior() {
        // A blank username must surface `MissingUsername` even when the
        // service is configured `Erroring` — the guard must not become
        // skippable by routing `connect` through the behaviour matcher.
        let svc = MockConnectionService::erroring();
        let creds = Credentials {
            username: "   ".into(),
            password: None,
        };
        let out = svc.connect("staging-cluster", &creds).await;
        assert!(
            matches!(out, Err(StudioError::MissingUsername)),
            "blank username must win over the configured Erroring behavior"
        );
    }

    #[tokio::test]
    async fn session_info_populates_the_statusbar() {
        let svc = MockConnectionService::ready();
        let s = svc.session_info().await.expect("session info");
        assert!(!s.server_version.is_empty());
    }

    #[tokio::test]
    async fn session_info_reports_the_fixture_values() {
        // Independently-authored expectations (not derived from the call under
        // test), so this can actually fail if the fixture drifts.
        let svc = MockConnectionService::ready();
        let s = svc.session_info().await.expect("session info");
        assert_eq!(s.server_version, "dev");
    }

    #[tokio::test]
    async fn session_info_empty_behavior_still_returns_the_fixture() {
        // Single-value reads have no "empty" shape, so Empty folds into Ready
        // — matching the `record_detail` precedent.
        let svc = MockConnectionService::empty();
        let s = svc.session_info().await.expect("empty folds into ready");
        assert_eq!(s.server_version, "dev");
    }

    #[tokio::test]
    async fn session_info_erroring_is_retriable_error() {
        let svc = MockConnectionService::erroring();
        let err = svc.session_info().await.expect_err("erroring must fail");
        assert!(err.is_retriable());
    }

    #[tokio::test]
    async fn session_info_delayed_still_resolves() {
        let svc = MockConnectionService::delayed(std::time::Duration::from_millis(5));
        let s = svc.session_info().await.expect("delayed still resolves");
        assert_eq!(s.server_version, "dev");
    }

    #[tokio::test]
    async fn nav_badges_come_from_the_seam() {
        let svc = MockConnectionService::ready();
        let b = svc.nav_badges().await.expect("badges");
        assert!(b.query > 0, "fixture should show badges");
    }

    #[tokio::test]
    async fn nav_badges_reports_the_fixture_counts() {
        let svc = MockConnectionService::ready();
        let b = svc.nav_badges().await.expect("badges");
        assert_eq!(b.query, 3);
    }

    #[tokio::test]
    async fn nav_badges_empty_behavior_still_returns_the_fixture() {
        let svc = MockConnectionService::empty();
        let b = svc.nav_badges().await.expect("empty folds into ready");
        assert!(b.query > 0);
    }

    #[tokio::test]
    async fn nav_badges_erroring_is_retriable_error() {
        let svc = MockConnectionService::erroring();
        let err = svc.nav_badges().await.expect_err("erroring must fail");
        assert!(err.is_retriable());
    }

    #[tokio::test]
    async fn nav_badges_delayed_still_resolves() {
        let svc = MockConnectionService::delayed(std::time::Duration::from_millis(5));
        let b = svc.nav_badges().await.expect("delayed still resolves");
        assert_eq!(b.query, 3);
    }

    #[tokio::test]
    async fn databases_are_unique() {
        let svc = connected(MockConnectionService::ready(), "Local development").await;
        let mut dbs = svc.databases().await.expect("databases");
        let total = dbs.len();
        dbs.sort();
        dbs.dedup();
        assert_eq!(total, dbs.len());
    }

    #[tokio::test]
    async fn databases_fixture_has_more_than_one_entry() {
        // Pins the precondition `databases_are_unique` relies on: with a
        // single-entry fixture the dedup check above could never fire.
        let svc = connected(MockConnectionService::ready(), "Local development").await;
        let dbs = svc.databases().await.expect("databases");
        assert!(
            dbs.len() > 1,
            "fixture must list more than one database for the dedup check to be meaningful"
        );
        assert!(dbs.contains(&"analytics".to_string()));
    }

    /// Opens `name` on `svc` so session-scoped reads have a session.
    async fn connected(svc: MockConnectionService, name: &str) -> MockConnectionService {
        let creds = Credentials {
            username: "alice".into(),
            password: None,
        };
        svc.connect(name, &creds)
            .await
            .expect("fixture connection opens");
        svc
    }

    /// Each connection lists its own databases: switching sessions switches
    /// the list. A single shared fixture shows one server's databases on
    /// another.
    #[tokio::test]
    async fn databases_follow_the_connected_session() {
        let svc = connected(MockConnectionService::ready(), "Local development").await;
        let local = svc.databases().await.expect("local databases");
        let svc = connected(svc, "staging-cluster").await;
        let staging = svc.databases().await.expect("staging databases");
        assert_eq!(local.len(), 3);
        assert_eq!(staging.len(), 12);
        assert_ne!(local, staging);
    }

    #[tokio::test]
    async fn databases_before_connect_is_not_connected() {
        let svc = MockConnectionService::ready();
        let err = svc.databases().await.expect_err("no session yet");
        assert!(matches!(err, StudioError::NotConnected));
    }

    #[tokio::test]
    async fn databases_empty_behavior_returns_no_rows() {
        let svc = MockConnectionService::empty();
        let dbs = svc.databases().await.expect("mock databases is infallible");
        assert!(dbs.is_empty());
    }

    #[tokio::test]
    async fn databases_erroring_is_retriable_error() {
        let svc = MockConnectionService::erroring();
        let err = svc.databases().await.expect_err("erroring must fail");
        assert!(err.is_retriable());
    }

    #[tokio::test]
    async fn databases_delayed_still_returns_the_fixture() {
        let svc = connected(
            MockConnectionService::delayed(std::time::Duration::from_millis(5)),
            "Local development",
        )
        .await;
        let dbs = svc.databases().await.expect("delayed still resolves");
        assert!(!dbs.is_empty());
    }

    #[tokio::test]
    async fn cdc_feed_ready_is_loaded() {
        let svc = MockConnectionService::ready();
        let state = AsyncState::from_value(Some(svc.cdc_feed().await));
        assert!(matches!(state, AsyncState::Loaded(_)));
    }

    #[tokio::test]
    async fn cdc_feed_empty_is_empty() {
        let svc = MockConnectionService::empty();
        let state = AsyncState::from_value(Some(svc.cdc_feed().await));
        assert!(state.is_empty());
    }

    #[tokio::test]
    async fn cdc_feed_erroring_is_retriable_error() {
        let svc = MockConnectionService::erroring();
        let state = AsyncState::from_value(Some(svc.cdc_feed().await));
        assert!(state.error_message().is_some());
        assert!(state.is_retriable());
    }

    // Verifies the two-step transition pattern: set Loading, await the read,
    // set Loaded from the result. This test has no Dioxus signals/guards (unit-level),
    // so it only demonstrates the transition logic; the actual no-guard-across-await
    // discipline is exercised by real streaming views and enforced by the
    // AGENTS.md convention.
    #[tokio::test]
    async fn delayed_read_transitions_loading_to_loaded_without_guard() {
        let svc = MockConnectionService::ready();
        let mut latest: AsyncState<Vec<CdcRow>> = AsyncState::Loading;
        assert!(latest.is_loading());
        // simulate the scheduler interleaving other work before the read resolves
        tokio::task::yield_now().await;
        let result = svc.cdc_feed().await;
        latest = AsyncState::from_value(Some(result));
        assert!(matches!(latest, AsyncState::Loaded(_)));
    }
    /// The write must be able to fail, or the popover's failure path is dead
    /// code. Erroring must reject the write AND leave the read unchanged, so a
    /// caller that mutates local state only after Ok cannot end up ahead of the
    /// backend.
    #[tokio::test]
    async fn mark_all_read_erroring_rejects_and_leaves_feed_unread() {
        let svc = MockConnectionService::erroring();
        assert!(
            svc.mark_all_read().await.is_err(),
            "erroring write must fail"
        );
        // The read also errors under this behaviour, so probe persisted state
        // through a ready view of the same shared cell instead.
        let ready = svc.with_shared_state(MockBehavior::Ready);
        let feed = ready.notifications().await.expect("ready read");
        assert!(
            feed.iter().any(|n| n.unread),
            "a failed write must not clear the persisted unread flags"
        );
    }

    fn alice() -> Credentials {
        Credentials {
            username: "alice".into(),
            password: Some("never-printed".into()),
        }
    }

    fn draft(name: &str, host: &str) -> ConnectionDraft {
        ConnectionDraft {
            name: name.into(),
            target: ConnectionTarget::Origin {
                host: host.into(),
                port: 7432,
            },
            database: "analytics".into(),
            auth: AuthMethod::UserPassword,
            username: "admin".into(),
            keychain: true,
            tls: crate::models::connections::TlsConfig {
                required: true,
                ..Default::default()
            },
        }
    }

    #[tokio::test]
    async fn connect_to_an_offline_entry_is_a_retriable_connection_error() {
        let svc = MockConnectionService::ready();
        let err = svc
            .connect("test-cluster", &alice())
            .await
            .expect_err("test-cluster is offline");
        assert!(matches!(err, StudioError::Connection(_)));
        assert!(err.is_retriable());
    }

    #[tokio::test]
    async fn save_adds_an_entry_that_connects() {
        let svc = MockConnectionService::ready();
        let saved = svc
            .save_connection(draft("Staging", "db.new.example"), alice(), None)
            .await
            .expect("save");
        assert_eq!(saved.name, "Staging");
        let listed = svc.list_connections().await.expect("list");
        assert_eq!(listed.len(), 5);
        let session = svc
            .connect("Staging", &saved.credentials())
            .await
            .expect("a saved entry connects");
        assert_eq!(session.name, "Staging");
    }

    #[tokio::test]
    async fn save_rejects_a_taken_name() {
        let svc = MockConnectionService::ready();
        let err = svc
            .save_connection(draft("staging-cluster", "db.new.example"), alice(), None)
            .await
            .expect_err("name is taken");
        assert!(matches!(err, StudioError::NameTaken(n) if n == "staging-cluster"));
        assert_eq!(svc.list_connections().await.expect("list").len(), 4);
    }

    /// An edit may keep its own name, and a rename frees the old one.
    #[tokio::test]
    async fn edit_replaces_in_place() {
        let svc = MockConnectionService::ready();
        let saved = svc
            .save_connection(
                draft("staging-cluster", "db.staging.example"),
                alice(),
                Some("staging-cluster".into()),
            )
            .await
            .expect("an edit keeps its own name");
        assert_eq!(saved.name, "staging-cluster");
        let renamed = svc
            .save_connection(
                draft("staging-eu", "db.staging.example"),
                alice(),
                Some("staging-cluster".into()),
            )
            .await
            .expect("rename");
        assert_eq!(renamed.name, "staging-eu");
        let names: Vec<_> = svc
            .list_connections()
            .await
            .expect("list")
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert_eq!(names.len(), 4);
        assert!(!names.contains(&"staging-cluster".to_string()));
        // An edit keeps the entry's profile, so it still connects.
        assert!(svc.connect("staging-eu", &alice()).await.is_ok());
    }

    #[tokio::test]
    async fn edit_of_an_unknown_entry_is_an_error() {
        let svc = MockConnectionService::ready();
        let err = svc
            .save_connection(draft("x", "h"), alice(), Some("ghost".into()))
            .await
            .expect_err("unknown");
        assert!(matches!(err, StudioError::UnknownConnection(n) if n == "ghost"));
    }

    #[tokio::test]
    async fn duplicate_picks_a_free_name_each_time() {
        let svc = MockConnectionService::ready();
        let a = svc
            .duplicate_connection("staging-cluster")
            .await
            .expect("copy");
        let b = svc
            .duplicate_connection("staging-cluster")
            .await
            .expect("copy");
        assert_eq!(a.name, "staging-cluster copy");
        assert_eq!(b.name, "staging-cluster copy 2");
        assert!(matches!(
            svc.duplicate_connection("ghost").await,
            Err(StudioError::UnknownConnection(_))
        ));
    }

    #[tokio::test]
    async fn test_connection_reports_what_it_saw() {
        let svc = MockConnectionService::ready();
        let ok = svc
            .test_connection(&draft("Staging", "db.staging.example"), &alice())
            .await
            .expect("staging is reachable");
        assert_eq!(ok.latency_ms, 22);
        assert!(ok.tls_verified);
        assert_eq!(ok.db_count, 12);
    }

    #[tokio::test]
    async fn test_connection_to_an_offline_host_fails() {
        let svc = MockConnectionService::ready();
        let err = svc
            .test_connection(&draft("T", "test-db.example.com"), &alice())
            .await
            .expect_err("offline");
        assert!(matches!(err, StudioError::Connection(_)));
    }

    /// Test never persists: the registry is as it was.
    #[tokio::test]
    async fn test_connection_saves_nothing() {
        let svc = MockConnectionService::ready();
        svc.test_connection(&draft("Staging", "db.new.example"), &alice())
            .await
            .expect("reachable");
        assert_eq!(svc.list_connections().await.expect("list").len(), 4);
    }

    #[tokio::test]
    async fn test_connection_needs_a_username_for_password_auth() {
        let svc = MockConnectionService::ready();
        let mut d = draft("Staging", "db.new.example");
        d.username = "  ".into();
        let err = svc.test_connection(&d, &alice()).await.expect_err("blank");
        assert!(matches!(err, StudioError::MissingUsername));
    }

    #[tokio::test]
    async fn select_database_checks_the_session_list() {
        let svc = connected(MockConnectionService::ready(), "staging-cluster").await;
        assert!(svc.select_database("commerce").await.is_ok());
        assert!(matches!(
            svc.select_database("nope").await,
            Err(StudioError::UnknownDatabase(n)) if n == "nope"
        ));
        let idle = MockConnectionService::ready();
        assert!(matches!(
            idle.select_database("commerce").await,
            Err(StudioError::NotConnected)
        ));
    }

    #[tokio::test]
    async fn select_database_erroring_is_retriable() {
        let svc = MockConnectionService::erroring();
        let err = svc.select_database("x").await.expect_err("erroring");
        assert!(err.is_retriable());
    }

    #[tokio::test]
    async fn mark_read_persists_one_entry() {
        let svc = MockConnectionService::ready();
        svc.mark_read("index-build-embedding").await.expect("write");
        let feed = svc.notifications().await.expect("read");
        let unread: Vec<_> = feed
            .iter()
            .filter(|n| n.unread)
            .map(|n| n.id.as_str())
            .collect();
        assert_eq!(unread.len(), 3);
        assert!(!unread.contains(&"index-build-embedding"));
    }

    #[tokio::test]
    async fn mark_read_erroring_is_retriable_and_changes_nothing() {
        let svc = MockConnectionService::erroring();
        assert!(svc.mark_read("x").await.is_err());
        let feed = svc
            .with_shared_state(MockBehavior::Ready)
            .notifications()
            .await
            .expect("read");
        assert_eq!(feed.iter().filter(|n| n.unread).count(), 4);
    }

    #[tokio::test]
    async fn activity_and_recent_items_follow_the_behavior() {
        assert!(
            !MockConnectionService::ready()
                .connection_activity()
                .await
                .expect("rows")
                .is_empty()
        );
        assert!(
            MockConnectionService::empty()
                .connection_activity()
                .await
                .expect("empty")
                .is_empty()
        );
        assert!(
            MockConnectionService::erroring()
                .connection_activity()
                .await
                .is_err()
        );
        assert_eq!(
            MockConnectionService::ready()
                .recent_items()
                .await
                .expect("rows")
                .len(),
            1
        );
        assert!(
            MockConnectionService::empty()
                .recent_items()
                .await
                .expect("empty")
                .is_empty()
        );
    }
}
