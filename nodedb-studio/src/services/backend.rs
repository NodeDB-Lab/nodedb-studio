//! The single front door. `Backend` composes every per-topic data trait plus the
//! connection/session/notification trait, so the app provides ONE `Rc<dyn Backend>`
//! and a screen reads the one method it needs without knowing which trait owns it.
//!
//! Adding a new domain trait later means extending this bound and implementing the
//! trait on the mock + stub — additive, never a reshape of existing methods.

use crate::services::admin_data::AdminData;
use crate::services::cluster_data::ClusterData;
use crate::services::connection_service::ConnectionService;
use crate::services::designer_data::DesignerData;
use crate::services::explorer_data::ExplorerData;
use crate::services::feed_data::FeedData;
use crate::services::graph_data::GraphData;
use crate::services::job_data::JobData;
use crate::services::listen_data::ListenData;
use crate::services::matview_data::MatViewData;
use crate::services::spatial_data::SpatialData;
use crate::services::timeseries_data::TimeseriesData;
use crate::services::topic_data::TopicData;
use crate::services::transfer_data::TransferData;
use crate::services::vector_data::VectorData;
use crate::services::viewers_data::ViewersData;
use crate::services::workbench_data::WorkbenchData;

pub trait Backend:
    ConnectionService
    + FeedData
    + TopicData
    + MatViewData
    + ListenData
    + JobData
    + ExplorerData
    + AdminData
    + ClusterData
    + WorkbenchData
    + ViewersData
    + TimeseriesData
    + VectorData
    + SpatialData
    + GraphData
    + TransferData
    + DesignerData
{
}

impl<T> Backend for T where
    T: ConnectionService
        + FeedData
        + TopicData
        + MatViewData
        + ListenData
        + JobData
        + ExplorerData
        + AdminData
        + ClusterData
        + WorkbenchData
        + ViewersData
        + TimeseriesData
        + TimeseriesData
        + VectorData
        + SpatialData
        + GraphData
        + TransferData
        + DesignerData
{
}
