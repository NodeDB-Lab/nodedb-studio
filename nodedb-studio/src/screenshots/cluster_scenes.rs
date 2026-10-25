//! The Cluster states the design shows. Each seed names every tab's state,
//! so one scene's selection or open review never leaks into the next image.

use crate::data::mock::ClusterStore;
use crate::models::cluster_place::MoveRequest;
use crate::models::ts_dashboard::Refresh;
use crate::state::cluster::{
    ActivityUi, ClusterSeed, MonitorUi, PlaceUi, PlanReview, RaftUi, TransferReview,
};

/// Raft groups with the group needing attention selected.
pub fn raft_groups() -> ClusterSeed {
    ClusterSeed {
        raft: Some(RaftUi {
            group: Some("data-12".into()),
            ..RaftUi::default()
        }),
        ..ClusterSeed::default()
    }
}

/// The leadership transfer review, open on data-12 with node-4 as target.
pub fn transfer_review() -> ClusterSeed {
    let preview = ClusterStore::seeded()
        .preview_transfer("data-12", "nd_04")
        .ok();
    ClusterSeed {
        raft: Some(RaftUi {
            group: Some("data-12".into()),
            target: Some("nd_04".into()),
            review: preview.map(|preview| TransferReview {
                preview,
                busy: false,
                error: None,
            }),
            ..RaftUi::default()
        }),
        ..ClusterSeed::default()
    }
}

fn move_17() -> MoveRequest {
    MoveRequest {
        vshard: 17,
        from_id: "nd_05".into(),
        to_id: "nd_01".into(),
    }
}

/// analytics.orders, vShard 17 selected, with its replica on node-5 staged
/// to move to node-1.
pub fn placement() -> ClusterSeed {
    ClusterSeed {
        place: Some(PlaceUi {
            database: Some("analytics".into()),
            collection: Some("orders".into()),
            vshard: Some(17),
            staged: vec![move_17()],
            ..PlaceUi::default()
        }),
        ..ClusterSeed::default()
    }
}

/// The rebalance plan review, open on the staged move.
pub fn plan_review() -> ClusterSeed {
    let plan = ClusterStore::seeded().preview_rebalance(&[move_17()]).ok();
    let mut seed = placement();
    if let Some(p) = seed.place.as_mut() {
        p.review = plan.map(|plan| PlanReview {
            plan,
            busy: false,
            error: None,
        });
    }
    seed
}

/// node-1 over the last hour, refreshing every 30 seconds.
pub fn monitoring() -> ClusterSeed {
    ClusterSeed {
        monitor: Some(MonitorUi {
            node: Some("nd_01".into()),
            refresh: Refresh::Seconds30,
            ..MonitorUi::default()
        }),
        ..ClusterSeed::default()
    }
}

/// The leader election of data-12 selected.
pub fn activity() -> ClusterSeed {
    ClusterSeed {
        activity: Some(ActivityUi {
            selected: Some("evt_98142".into()),
            ..ActivityUi::default()
        }),
        ..ClusterSeed::default()
    }
}
