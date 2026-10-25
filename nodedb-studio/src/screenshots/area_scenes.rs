//! The scenes of each area, in the order the design lists them.

use crate::models::workbench::TxAccess;
use crate::routes::Route;
use crate::routes::tabs::{
    AdminTab, ClusterTab, EventsTab, SearchTab, SpatialTab, SyncTab, TimeseriesTab, VectorTab,
};
use crate::screenshots::admin_scenes;
use crate::screenshots::cluster_scenes;
use crate::screenshots::designer_scenes::{self, DesignerSeed};
use crate::screenshots::events_scenes;
use crate::screenshots::scene::{Overlay, Scene, at, scene};
use crate::screenshots::search_scenes;
use crate::screenshots::spatial_scenes;
use crate::screenshots::sync_scenes;
use crate::screenshots::timeseries_scenes;
use crate::screenshots::vector_scenes;
use crate::screenshots::workbench_scenes;
use crate::state::admin::AdminSeed;
use crate::state::cluster::ClusterSeed;
use crate::state::events::EventsSeed;
use crate::state::search::SearchSeed;
use crate::state::spatial::SpatialSeed;
use crate::state::sync::SyncSeed;
use crate::state::timeseries::TimeseriesSeed;
use crate::state::ui::Popover;
use crate::state::vector::VectorSeed;

/// Query and Console, in the order the design lists them. The first two
/// leave the Saved queries panel hidden, as the Query mockup does.
pub fn workbench() -> Vec<Scene> {
    let query = |name, overlay| Scene {
        hidden: &["library"],
        query: Some(Box::new(workbench_scenes::query())),
        ..scene(name, at(Route::Query {}), overlay)
    };
    vec![
        query("query", Overlay::None),
        query("query-view-menu", Overlay::Popover(Popover::View)),
        Scene {
            hidden: &["library", "schema"],
            sizes: &[("editor", 250.0)],
            query: Some(Box::new(workbench_scenes::query_plan())),
            transaction: Some(TxAccess::ReadOnly),
            ..scene("query-plan", at(Route::Query {}), Overlay::None)
        },
        Scene {
            sizes: &[("editor", 392.0)],
            query: Some(Box::new(workbench_scenes::query_library())),
            ..scene("query-library", at(Route::Query {}), Overlay::None)
        },
        Scene {
            console: Some(Box::new(workbench_scenes::console())),
            ..scene("console-session", at(Route::Console {}), Overlay::None)
        },
    ]
}

/// Designer: the workbench, the Create collection drawer, and the review.
pub fn designer() -> Vec<Scene> {
    let at_designer = |name, seed: DesignerSeed| Scene {
        designer: Some(Box::new(seed)),
        ..scene(name, at(Route::Designer {}), Overlay::None)
    };
    vec![
        at_designer("designer", designer_scenes::workbench()),
        at_designer("designer-create", designer_scenes::create()),
        at_designer("designer-review", designer_scenes::review()),
    ]
}

/// Spatial: Map, Analyze, and Raster.
pub fn spatial() -> Vec<Scene> {
    let at_tab = |name, tab, seed: SpatialSeed| Scene {
        spatial: Some(Box::new(seed)),
        ..scene(name, at(Route::Spatial { tab }), Overlay::None)
    };
    vec![
        at_tab("spatial-map", SpatialTab::Map, spatial_scenes::map()),
        at_tab(
            "spatial-analyze",
            SpatialTab::Analyze,
            spatial_scenes::analyze(),
        ),
        at_tab(
            "spatial-raster",
            SpatialTab::Raster,
            spatial_scenes::raster(),
        ),
    ]
}

/// Search: Search, Analyze, and Indexes.
pub fn search() -> Vec<Scene> {
    let at_tab = |name, tab, seed: SearchSeed| Scene {
        search: Some(Box::new(seed)),
        ..scene(name, at(Route::Search { tab }), Overlay::None)
    };
    vec![
        at_tab(
            "search-results",
            SearchTab::Results,
            search_scenes::results(),
        ),
        at_tab(
            "search-analyze",
            SearchTab::Analyze,
            search_scenes::analyze(),
        ),
        at_tab(
            "search-indexes",
            SearchTab::Indexes,
            search_scenes::indexes(),
        ),
    ]
}

/// Vector Space: Search, Projection, and Index.
pub fn vector() -> Vec<Scene> {
    let at_tab = |name, tab, seed: VectorSeed| Scene {
        vector: Some(Box::new(seed)),
        ..scene(name, at(Route::Vector { tab }), Overlay::None)
    };
    vec![
        at_tab("vector-search", VectorTab::Search, vector_scenes::search()),
        at_tab(
            "vector-projection",
            VectorTab::Projection,
            vector_scenes::projection(),
        ),
        at_tab("vector-index", VectorTab::Index, vector_scenes::index()),
    ]
}

/// Timeseries: Explore with the picker open, Dashboards, the panel editor,
/// Lifecycle, and the backfill review.
pub fn timeseries() -> Vec<Scene> {
    let at_tab = |name, tab, seed: TimeseriesSeed| Scene {
        timeseries: Some(Box::new(seed)),
        ..scene(name, at(Route::Timeseries { tab }), Overlay::None)
    };
    vec![
        at_tab(
            "timeseries-explore",
            TimeseriesTab::Explore,
            timeseries_scenes::explore(),
        ),
        at_tab(
            "timeseries-dashboards",
            TimeseriesTab::Dashboards,
            timeseries_scenes::dashboards(),
        ),
        at_tab(
            "timeseries-dashboard-editor",
            TimeseriesTab::Dashboards,
            timeseries_scenes::dashboard_editor(),
        ),
        at_tab(
            "timeseries-lifecycle",
            TimeseriesTab::Lifecycle,
            timeseries_scenes::lifecycle(),
        ),
        at_tab(
            "timeseries-backfill",
            TimeseriesTab::Backfill,
            timeseries_scenes::backfill(),
        ),
    ]
}

/// Events & Jobs: each tab, and the Create change feed drawer.
pub fn events() -> Vec<Scene> {
    let at_tab = |name, tab, seed: EventsSeed| Scene {
        events: Some(Box::new(seed)),
        ..scene(name, at(Route::Events { tab }), Overlay::None)
    };
    vec![
        at_tab(
            "events-changefeeds",
            EventsTab::ChangeFeeds,
            events_scenes::changefeeds(),
        ),
        at_tab(
            "events-feed-editor",
            EventsTab::ChangeFeeds,
            events_scenes::feed_editor(),
        ),
        at_tab("events-topics", EventsTab::Topics, events_scenes::topics()),
        at_tab(
            "events-materialized-views",
            EventsTab::MaterializedViews,
            events_scenes::materialized_views(),
        ),
        at_tab(
            "events-notifications",
            EventsTab::Notifications,
            events_scenes::notifications(),
        ),
        at_tab(
            "events-scheduled-jobs",
            EventsTab::ScheduledJobs,
            events_scenes::scheduled_jobs(),
        ),
    ]
}

/// Cluster: each tab, and the two reviews a membership operation opens.
pub fn cluster() -> Vec<Scene> {
    let at_tab = |name, tab, seed: ClusterSeed| Scene {
        cluster: Some(Box::new(seed)),
        ..scene(name, at(Route::Cluster { tab }), Overlay::None)
    };
    vec![
        at_tab("cluster-nodes", ClusterTab::Nodes, ClusterSeed::default()),
        at_tab(
            "cluster-raft-groups",
            ClusterTab::RaftGroups,
            cluster_scenes::raft_groups(),
        ),
        at_tab(
            "cluster-raft-transfer-review",
            ClusterTab::RaftGroups,
            cluster_scenes::transfer_review(),
        ),
        at_tab(
            "cluster-placement",
            ClusterTab::Placement,
            cluster_scenes::placement(),
        ),
        at_tab(
            "cluster-placement-review",
            ClusterTab::Placement,
            cluster_scenes::plan_review(),
        ),
        at_tab(
            "cluster-monitoring",
            ClusterTab::Monitoring,
            cluster_scenes::monitoring(),
        ),
        at_tab(
            "cluster-activity",
            ClusterTab::Activity,
            cluster_scenes::activity(),
        ),
    ]
}

/// Sync: each tab, the enrollment sheet, and the two reviews a repair opens.
pub fn sync() -> Vec<Scene> {
    let at_tab = |name, tab, seed: SyncSeed| Scene {
        sync: Some(Box::new(seed)),
        ..scene(name, at(Route::Sync { tab }), Overlay::None)
    };
    vec![
        at_tab("sync-devices", SyncTab::Devices, sync_scenes::devices()),
        at_tab(
            "sync-enrollment",
            SyncTab::Devices,
            sync_scenes::enrollment(),
        ),
        at_tab(
            "sync-subscriptions",
            SyncTab::Subscriptions,
            sync_scenes::subscriptions(),
        ),
        at_tab(
            "sync-subscription-review",
            SyncTab::Subscriptions,
            sync_scenes::subscription_review(),
        ),
        at_tab(
            "sync-rejected-writes",
            SyncTab::RejectedWrites,
            sync_scenes::rejected_writes(),
        ),
        at_tab(
            "sync-rejected-edit",
            SyncTab::RejectedWrites,
            sync_scenes::reject_edit(),
        ),
        at_tab("sync-activity", SyncTab::Activity, sync_scenes::activity()),
    ]
}

/// Administration: each tab, and the two reviews that apply a change.
pub fn admin() -> Vec<Scene> {
    let at_tab = |name, tab, seed: AdminSeed| Scene {
        admin: Some(Box::new(seed)),
        ..scene(name, at(Route::Admin { tab }), Overlay::None)
    };
    vec![
        at_tab("admin-access", AdminTab::Access, admin_scenes::access()),
        at_tab(
            "admin-access-review",
            AdminTab::Access,
            admin_scenes::access_review(),
        ),
        at_tab(
            "admin-row-policies",
            AdminTab::RowPolicies,
            admin_scenes::row_policies(),
        ),
        at_tab("admin-audit", AdminTab::Audit, admin_scenes::audit()),
        at_tab("admin-backups", AdminTab::Backups, admin_scenes::backups()),
        at_tab(
            "admin-restore-review",
            AdminTab::Backups,
            admin_scenes::restore_review(),
        ),
        at_tab("admin-config", AdminTab::Config, admin_scenes::config()),
        at_tab(
            "admin-credentials",
            AdminTab::Credentials,
            admin_scenes::credentials(),
        ),
    ]
}
