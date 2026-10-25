//! The real-client-backed `ConnectionService` impl. Currently an inert stub:
//! every method returns `StudioError::NotConnected` until a real connection is
//! opened — no panic or `todo!()` in the interim.
//!
//! The future `ConnectionBuilder` wiring will add the held client (an
//! `Option<NativeClient>`, gated behind nodedb-client's `native` feature) in the
//! same change that first reads it. That feature pulls a TLS + C-toolchain build
//! tree (tokio-rustls, aws-lc-rs, cmake), so it is deliberately left disabled
//! until a live consumer exists rather than carried here for a field nothing reads.

// The Phase-2 seam impl: its trait conformance and object-safety are proven by
// the tests below, but the mock is still the active injected service, so this
// type is not constructed in non-test code yet. The phase that wires it in as
// the real service removes this allow.
#[allow(dead_code)]
#[derive(Default)]
pub struct NodeDbConnectionService;

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;
    use crate::models::feeds::FeedSession;
    use crate::services::connection_service::ConnectionService;
    use crate::services::error::StudioError;
    use crate::services::feed_data::FeedData;
    use crate::services::job_data::JobData;
    use crate::services::listen_data::ListenData;
    use crate::services::matview_data::MatViewData;
    use crate::services::topic_data::TopicData;
    use crate::services::vector_data::VectorData;
    use crate::services::viewers_data::ViewersData;
    use crate::services::workbench_data::WorkbenchData;
    use crate::state::connections_registry::Credentials;

    #[tokio::test]
    async fn stub_returns_not_connected() {
        let svc = NodeDbConnectionService;
        assert!(matches!(
            svc.list_connections().await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.notifications().await,
            Err(StudioError::NotConnected)
        ));
        let creds = Credentials {
            username: "alice".into(),
            password: None,
        };
        assert!(matches!(
            svc.connect("anything", &creds).await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.mark_all_read().await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.mark_read("id").await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.connection_activity().await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.recent_items().await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.duplicate_connection("x").await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.select_database("x").await,
            Err(StudioError::NotConnected)
        ));
    }

    #[tokio::test]
    async fn stub_shell_chrome_reads_are_not_connected() {
        let svc = NodeDbConnectionService;
        assert!(matches!(
            svc.nav_badges().await,
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
    }

    #[tokio::test]
    async fn stub_events_reads_and_session_are_not_connected() {
        let svc = NodeDbConnectionService;
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
        let svc = NodeDbConnectionService;
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
        let svc = NodeDbConnectionService;
        assert!(matches!(
            svc.vector_setup().await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.vector_index_status("articles", "embedding").await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.series("qps").await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.spatial_features("places").await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.fts_hits("articles", "nodedb").await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.sync_peers().await,
            Err(StudioError::NotConnected)
        ));
    }

    #[test]
    fn stub_is_object_safe_behind_rc() {
        // Compile-time guarantee: the stub coerces to the seam trait object,
        // exactly as `app.rs` provides it via context.
        let _s: Rc<dyn ConnectionService> = Rc::new(NodeDbConnectionService);
    }

    #[test]
    fn stub_is_object_safe_behind_backend() {
        use crate::services::backend::Backend;
        let _b: Rc<dyn Backend> = Rc::new(NodeDbConnectionService);
    }
}
