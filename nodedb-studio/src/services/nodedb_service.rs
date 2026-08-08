//! The real-client-backed `ConnectionService` impl. Currently an inert stub:
//! every method returns `StudioError::NotConnected` until a real connection is
//! opened — no panic or `todo!()` in the interim.
//!
//! The future `ConnectionBuilder` wiring will add the held client (an
//! `Option<NativeClient>`, gated behind nodedb-client's `native` feature) in the
//! same change that first reads it. That feature pulls a TLS + C-toolchain build
//! tree (tokio-rustls, aws-lc-rs, cmake), so it is deliberately left disabled
//! until a live consumer exists rather than carried here for a field nothing reads.

use async_trait::async_trait;

use crate::models::admin::{AuditEntry, ClusterNode, RaftGroup, RlsPolicy, ShardRange, UserRow};
use crate::models::cdc::CdcRow;
use crate::models::explorer::{CollectionGroup, RecordDetail, RecordRow};
use crate::models::notification::Notification;
use crate::models::shell::{NavBadges, SessionInfo};
use crate::models::streams::{
    MaterializedView, NotifyChannel, NotifyMessage, ScheduledJob, StreamSession, Topic,
};
use crate::models::viewers::{
    FtsHit, SeriesPoint, SpatialFeature, SubGraph, SyncPeer, VectorPoint,
};
use crate::models::workbench::{QueryPlan, ResultSet, SchemaNode};
use crate::services::admin_data::AdminData;
use crate::services::connection_service::ConnectionService;
use crate::services::error::StudioError;
use crate::services::explorer_data::ExplorerData;
use crate::services::streams_data::StreamsData;
use crate::services::viewers_data::ViewersData;
use crate::services::workbench_data::WorkbenchData;
use crate::state::connection::ActiveConnection;
use crate::state::connections_registry::{Credentials, SavedConnection};

// The Phase-2 seam impl: its trait conformance and object-safety are proven by
// the tests below, but the mock is still the active injected service, so this
// type is not constructed in non-test code yet. The phase that wires it in as
// the real service removes this allow.
#[allow(dead_code)]
#[derive(Default)]
pub struct NodeDbConnectionService;

#[async_trait(?Send)]
impl ConnectionService for NodeDbConnectionService {
    async fn list_connections(&self) -> Result<Vec<SavedConnection>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn notifications(&self) -> Result<Vec<Notification>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn connect(
        &self,
        _name: &str,
        _creds: &Credentials,
    ) -> Result<ActiveConnection, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn mark_all_read(&self) -> Result<(), StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn nav_badges(&self) -> Result<NavBadges, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn session_info(&self) -> Result<SessionInfo, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn databases(&self) -> Result<Vec<String>, StudioError> {
        Err(StudioError::NotConnected)
    }
}

#[async_trait(?Send)]
impl StreamsData for NodeDbConnectionService {
    async fn cdc_feed(&self) -> Result<Vec<CdcRow>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn open_stream_session(&self, _stream: &str) -> Result<StreamSession, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn cdc_batch(
        &self,
        _session: &StreamSession,
        _limit: usize,
    ) -> Result<Vec<CdcRow>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn commit_stream_offsets(&self, _session: &StreamSession) -> Result<(), StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn close_stream_session(&self, _session: &StreamSession) -> Result<(), StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn materialized_views(&self) -> Result<Vec<MaterializedView>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn topics(&self) -> Result<Vec<Topic>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn scheduled_jobs(&self) -> Result<Vec<ScheduledJob>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn notify_channels(&self) -> Result<Vec<NotifyChannel>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn notify_messages(&self) -> Result<Vec<NotifyMessage>, StudioError> {
        Err(StudioError::NotConnected)
    }
}

#[async_trait(?Send)]
impl ExplorerData for NodeDbConnectionService {
    async fn collection_groups(&self) -> Result<Vec<CollectionGroup>, StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn records(&self, _collection: &str) -> Result<Vec<RecordRow>, StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn record_detail(
        &self,
        _collection: &str,
        _id: &str,
    ) -> Result<RecordDetail, StudioError> {
        Err(StudioError::NotConnected)
    }
}

#[async_trait(?Send)]
impl AdminData for NodeDbConnectionService {
    async fn cluster_nodes(&self) -> Result<Vec<ClusterNode>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn raft_groups(&self) -> Result<Vec<RaftGroup>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn shard_ranges(&self) -> Result<Vec<ShardRange>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn users(&self) -> Result<Vec<UserRow>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn rls_policies(&self) -> Result<Vec<RlsPolicy>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn audit_entries(&self) -> Result<Vec<AuditEntry>, StudioError> {
        Err(StudioError::NotConnected)
    }
}

#[async_trait(?Send)]
impl WorkbenchData for NodeDbConnectionService {
    async fn run_query(&self, _sql: &str) -> Result<ResultSet, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn explain(&self, _sql: &str) -> Result<QueryPlan, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn schema_tree(&self) -> Result<Vec<SchemaNode>, StudioError> {
        Err(StudioError::NotConnected)
    }
}

#[async_trait(?Send)]
impl ViewersData for NodeDbConnectionService {
    async fn sub_graph(&self) -> Result<SubGraph, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn vector_points(&self) -> Result<Vec<VectorPoint>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn series(&self, _metric: &str) -> Result<Vec<SeriesPoint>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn spatial_features(&self) -> Result<Vec<SpatialFeature>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn fts_hits(&self, _query: &str) -> Result<Vec<FtsHit>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn sync_peers(&self) -> Result<Vec<SyncPeer>, StudioError> {
        Err(StudioError::NotConnected)
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;

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
    async fn stub_streams_lifecycle_and_lists_are_not_connected() {
        let svc = NodeDbConnectionService;
        let session = StreamSession {
            stream: "cdc".into(),
            group: "studio_cdc".into(),
        };
        assert!(matches!(
            svc.open_stream_session("cdc").await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.cdc_batch(&session, 10).await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.commit_stream_offsets(&session).await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.close_stream_session(&session).await,
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
        assert!(matches!(
            svc.notify_messages().await,
            Err(StudioError::NotConnected)
        ));
    }

    #[tokio::test]
    async fn stub_workbench_reads_are_not_connected() {
        let svc = NodeDbConnectionService;
        assert!(matches!(
            svc.run_query("SELECT 1").await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.explain("SELECT 1").await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.schema_tree().await,
            Err(StudioError::NotConnected)
        ));
    }

    #[tokio::test]
    async fn stub_viewer_reads_are_not_connected() {
        let svc = NodeDbConnectionService;
        assert!(matches!(
            svc.sub_graph().await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.vector_points().await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.series("qps").await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.spatial_features().await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.fts_hits("nodedb").await,
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
